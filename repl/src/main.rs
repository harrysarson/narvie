extern crate byteorder;
extern crate clap;
extern crate directories;
extern crate log;
extern crate prettytable;
extern crate rustyline;
extern crate serialport;
extern crate stderrlog;
extern crate time;

mod lib;

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use clap::{App, Arg};
use directories::ProjectDirs;
use lib::instruction::{self, Instruction};
use lib::register::{self, Register};
use log::{debug, error, info, warn};
use prettytable::*;

use rustyline::error::ReadlineError;
use rustyline::Editor;
use std::error::Error;
use std::fmt::{self, Debug, Display};
use std::fs::{self, File};
use std::io;
use std::net::TcpStream;
use std::path::Path;
use std::process;
use std::str::FromStr;
use std::time::Duration;

#[derive(Debug)]
enum EvalInstructionError {
    Parse(instruction::Error),
    Write(io::Error),
    Read(io::Error),
}

struct RunArgs<'a, F>
where
    F: FnMut(&'static str) -> Result<(), EvalInstructionError>,
{
    history_file_path: Option<&'a Path>,
    evaluator: F,
}

struct SerialLogger<'a, S: io::Read + io::Write> {
    stream: S,
    logger: Option<&'a mut io::Write>,
}

trait ReadWrite: io::Read + io::Write {}
impl<T: io::Read + io::Write> ReadWrite for T {}

struct NarviePortError {}

impl Display for NarviePortError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Narvie Port Error!")
    }
}

impl Debug for NarviePortError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self)
    }
}
impl Error for NarviePortError {}

impl<'a, S: io::Read + io::Write> io::Read for SerialLogger<'a, S> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let res = self.stream.read(buf);
        if let Some(ref mut logger) = self.logger {
            logger.write_all(buf)?;
        }
        res
    }
}
impl<'a, S: io::Read + io::Write> io::Write for SerialLogger<'a, S> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stream.write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}

/* narvie will use these as headers when displaying binary.
 */
fn format_headers(f: &instruction::Format) -> &'static [&'static str] {
    match f {
        instruction::Format::U => &["imm[31:12]", "rd", "opcode"],
        instruction::Format::J => &["imm[20|10:1|11|19:12]", "rd", "opcode"],
        instruction::Format::I => &["imm[11:0]", "rs1", "funct3", "rd", "opcode"],
        instruction::Format::B => &[
            "imm[12|10:5]",
            "rs2",
            "rs1",
            "funct3",
            "imm[4:1|11]",
            "opcode",
        ],
        instruction::Format::R => &["funct7", "rs2", "rs1", "funct3", "rd", "opcode"],
        instruction::Format::S => &["imm[11:5]", "rs2", "rs1", "funct3", "imm[4:0]", "opcode"],
        instruction::Format::Fence => {
            &["fm", "pred", "succ", "rs1", "funct3", "imm[4:0]", "opcode"]
        }
    }
}

/* narvie will split the instruction into blocks of binary.
 * These arrays indicate how wide each block should be.
 *
 * It is assumed in the code that these are positive integers
 * and that they sum to 32
 */
fn binary_block_widths(f: &instruction::Format) -> &'static [u32] {
    match f {
        instruction::Format::U => &[20, 5, 7],
        instruction::Format::J => &[20, 5, 7],
        instruction::Format::I => &[12, 5, 3, 5, 7],
        instruction::Format::B => &[7, 5, 5, 3, 5, 7],
        instruction::Format::R => &[7, 5, 5, 3, 5, 7],
        instruction::Format::S => &[7, 5, 5, 3, 5, 7],
        instruction::Format::Fence => &[4, 4, 4, 5, 3, 5, 7],
    }
}

fn format_binary_instruction(inst: &Instruction) -> Vec<String> {
    let widths = binary_block_widths(&inst.to_format());
    assert!(widths.into_iter().sum::<u32>() == 32);

    let binary_str = format!("{:032b}", inst.to_u32());

    widths
        .into_iter()
        .fold((vec![], binary_str.as_str()), |(mut p, string), w| {
            let (a, rest) = string.split_at(*w as usize);
            p.push(a.to_string());
            (p, rest)
        })
        .0
}

fn assembly_table(instruction: &Instruction) -> prettytable::Table {
    let mut header_row = prettytable::row!["Mnemonic"];
    header_row.extend(
        format_headers(&instruction.to_format())
            .into_iter()
            .map(|s| prettytable::Cell::new(s)),
    );

    let mut instruction_row = prettytable::row![instruction];
    instruction_row.extend(
        format_binary_instruction(&instruction)
            .into_iter()
            .map(|s| prettytable::Cell::new(&s)),
    );

    let mut table = prettytable::Table::init(vec![header_row, instruction_row]);

    table.set_format(*prettytable::format::consts::FORMAT_BOX_CHARS);
    table
}

fn reg_file_table(reg_file: &[u32; 32]) -> prettytable::Table {
    const COLUMNS: u32 = 2;
    const ROWS: u32 = register::GPR_COUNT / 2;
    if ROWS * COLUMNS != register::GPR_COUNT {
        panic!("Expected an even number of registers");
    }
    let mut table = prettytable::Table::init(vec![prettytable::Row::new(
        (0..COLUMNS)
            .map(|i| {
                let mut side_table = prettytable::table!(["Name", "ABI", "Value"]);
                side_table.extend((0..ROWS).map(|j| {
                    let reg_index = i * ROWS + j;
                    prettytable::row![
                        format!("x{}", reg_index),
                        Register::<()>::from_u32(reg_index).unwrap().abi_name(),
                        format!("0x{:08X}", reg_file[reg_index as usize]),
                    ]
                }));

                side_table.set_format(*prettytable::format::consts::FORMAT_BOX_CHARS);
                prettytable::cell!(side_table)
            })
            .collect::<Vec<prettytable::Cell>>(),
    )]);
    table.set_format(*prettytable::format::consts::FORMAT_CLEAN);
    table
}

fn eval_instruction<S>(mnemonic: &str, port: &mut S) -> Result<(), EvalInstructionError>
where
    S: io::Read + io::Write,
{
    let instruction = Instruction::from_str(mnemonic).map_err(EvalInstructionError::Parse)?;

    assembly_table(&instruction).printstd();

    port.write_u32::<LittleEndian>(instruction.to_u32())
        .map_err(EvalInstructionError::Write)?;

    let mut reg_file = [0; 32];

    for i in 0..32 {
        reg_file[i] = port
            .read_u32::<LittleEndian>()
            .map_err(EvalInstructionError::Read)?;
    }

    reg_file_table(&reg_file).printstd();

    Ok(())
}

fn run<F>(args: RunArgs<F>) -> Result<(), Box<Error>>
where
    for<'b> F: FnMut(&'b str) -> Result<(), EvalInstructionError>,
{
    let RunArgs {
        history_file_path,
        mut evaluator,
    } = args;

    let mut rl = Editor::<()>::new();

    if let Some(file) = history_file_path {
        if rl.load_history(&file).is_err() {
            info!("No existing history file");
        }
        debug!("Saving history to {}", file.to_string_lossy())
    }

    loop {
        match rl.readline("> ") {
            Ok(line) => {
                rl.add_history_entry(line.as_ref());
                if let Some(file) = history_file_path {
                    if rl.save_history(file).is_err() {
                        warn!("Error saving history to file");
                    }
                }
                let line = line.trim();
                if !line.is_empty() {
                    if let Err(error) = evaluator(line) {
                        println!(
                            "Error {}:",
                            match error {
                                EvalInstructionError::Parse(_) => "parsing instruction mnemonic",
                                EvalInstructionError::Write(_) => "writing to serial port",
                                EvalInstructionError::Read(_) => "reading from serial port",
                            }
                        );

                        match error {
                            EvalInstructionError::Parse(parse_error) => {
                                println!("  {:?}", parse_error)
                            }
                            EvalInstructionError::Write(e) => {
                                println!("  {:?}", e);
                                return Err(Box::new(e));
                            }
                            EvalInstructionError::Read(e) => {
                                println!("  {:?}", e);
                                return Err(Box::new(e));
                            }
                        };
                    }
                }
            }
            Err(ReadlineError::Interrupted) => return Ok(()),
            Err(ReadlineError::Eof) => return Ok(()),
            Err(err) => return Err(Box::new(err)),
        }
    }
}

fn constrain<F>(f: F) -> F
where
    F: for<'a> FnMut(&'a str) -> Result<(), EvalInstructionError>,
{
    f
}

fn narvie_port(matches: &clap::ArgMatches) -> Result<Box<dyn ReadWrite>, Box<Error>> {
    if let Some(tcp_port) = matches.value_of("tcp-port") {
        let tcp_port = tcp_port.parse::<u16>().map_err(|e| {
            error!("Port for tcp must be a positive integer");
            Box::new(e)
        })?;

        TcpStream::connect(("localhost", tcp_port))
            .map_err(|e| {
                error!(
                    "
narvie cannot connect to tcp port {}!

Check that a simulation of the narvie processor is running and the that you
are using the correct port address. Then run the narvie CLI again.",
                    tcp_port
                );
                debug!("Error details: {:?}", e);
                Box::new(e).into()
            })
            .map(Box::new)
            .map(|b| Box::<dyn ReadWrite>::from(b))
    } else {
        let address = matches.value_of("address").ok_or_else(|| {
            if let Ok(ports) = serialport::available_ports() {
                if ports.is_empty() {
                    println!(
                        "
The narvie CLI requires a connection to a narvie processor to evaluate RISC-V
instructions. However, your computer does not list any available serialport
connections. Ensure that a narvie processor is plugged into your computer and
try again.

If you don't have a narvie processor to hand, use --assembly-only to see how
narvie works!"
                    );
                } else {
                    println!(
                        "Please provide narvie with the address of your serial port!
Maybe try one of these:"
                    );
                    for (i, info) in ports.iter().enumerate() {
                        println!("    {}: {}", i + 1, info.port_name);
                    }
                }
            }
            println!("\n{}", matches.usage());
            Box::new(NarviePortError {})
        })?;

        let baud = matches
            .value_of("baud")
            .and_then(|input| input.parse::<u32>().ok())
            .ok_or_else(|| {
                error!("Parameter --baud must be an integer");
                Box::new(NarviePortError {})
            })?;

        serialport::open_with_settings(
            address,
            &serialport::SerialPortSettings {
                baud_rate: baud,
                data_bits: serialport::DataBits::Eight,
                flow_control: serialport::FlowControl::None,
                parity: serialport::Parity::None,
                stop_bits: serialport::StopBits::One,
                timeout: Duration::from_millis(500),
            },
        )
        .map_err(|e: serialport::Error| {
            let header = "Cannot connect to narvie processor!";
            if e.description == "Permission denied" {
                error!(
                    "{}
\tIt may be that narvie does not have permission to access your serial port.
\tTry running `$ sudo chmod 666 {}`",
                    header, address,
                );
            } else {
                error!(
                    "{}
\tCheck the processor is running and the that you are using the
\tcorrect address. Then run the narvie CLI again.",
                    header
                );
            }
            debug!("Error details: {:?}", e);
            Box::new(e).into()
        })
        .map(Box::new)
        .map(|b| Box::<dyn ReadWrite>::from(b))
    }
}

fn main() {
    stderrlog::new()
        .module(module_path!())
        .verbosity(3)
        .init()
        .unwrap();

    let (history_file, log_file) = ProjectDirs::from("", "physical-computation", "narvie")
        .map(|p| p.data_dir().to_owned())
        .ok_or_else(|| warn!("No project dir found"))
        .map(|dir| {
            if fs::create_dir_all(&dir).is_err() {
                warn!("Could not create project dir")
            }
            dir
        })
        .map(|dir| {
            (
                Some(dir.join("history.txt")),
                Some(dir.join(format!("log-{}", time::now_utc().rfc3339()))),
            )
        })
        .unwrap_or((None, None));

    let matches = App::new("narve CLI")
        .version("0.1.0")
        .author("Harry Sarson <harry.sarson@hotmail.co.uk>")
        .about("Native RISCV instruction evaluator")
        .arg(
            Arg::with_name("address")
                .value_name("address")
                .takes_value(true)
                .help("serial port port address."),
        )
        .arg(
            Arg::with_name("tcp-port")
                .value_name("PORT")
                .takes_value(true)
                .long("tcp")
                .help("Listen over tcp at port PORT instead of a serialport."),
        )
        .arg(
            Arg::with_name("baud")
                .default_value("9600")
                .value_name("baud")
                .takes_value(true)
                .long("baud")
                .help("baud rate"),
        )
        .arg(
            Arg::with_name("assemble-only")
                .long("assemble-only")
                .help("Only assemble mnemonics, do not evaluate them."),
        )
        .get_matches();

    if matches.is_present("assemble-only") {
        let evaluator = constrain(move |mnemonic| {
            let instruction =
                Instruction::from_str(mnemonic).map_err(EvalInstructionError::Parse)?;

            assembly_table(&instruction).printstd();
            Ok(())
        });

        run(RunArgs {
            history_file_path: history_file.as_ref().map(|p| p.as_path()),
            evaluator: evaluator,
        })
        .unwrap_or_else(|e| {
            error!("Unrecognised error: {}", e);
            process::exit(1)
        });
    } else {
        let mut logger = log_file.and_then(|p| {
            File::create(&p)
                .map_err(|e| warn!("Could not open log file: {:?}", e))
                .map(|l| {
                    debug!("Saving register logs to {}", p.to_string_lossy());
                    l
                })
                .ok()
        });

        let mut stream = SerialLogger {
            stream: narvie_port(&matches).unwrap_or_else(|_| process::exit(1)),
            logger: logger.as_mut().map(|f| (f as &mut io::Write)),
        };

        let evaluator = constrain(move |inst| eval_instruction(inst, &mut stream));

        run(RunArgs {
            history_file_path: history_file.as_ref().map(|p| p.as_path()),
            evaluator,
        })
        .unwrap_or_else(|e| {
            error!("Unrecognised error: {}", e);
            process::exit(1)
        })
    }
}