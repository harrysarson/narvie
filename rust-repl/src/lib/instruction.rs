use lib::immediate::{self, Immediate, InvalidImmediate};
use lib::register::{GetRegisterError, Rd, Register, Rs1, Rs2};
use std::fmt;
use std::marker::PhantomData;
use std::str::FromStr;
use std::string::String;

#[derive(Debug)]
pub enum GetMemoryLocationError {
    MissingOpenParentheses,
    MissingCloseParentheses,
    TextAfterCloseParenthesis,
}

#[derive(Debug)]
pub enum Error {
    WrongNumberOfArgs {
        actual: usize,
        expected: &'static [usize],
    },
    InvalidRegisterArg(GetRegisterError),
    InvalidImmediateArg(InvalidImmediate),
    InvalidMemoryLocationArg(GetMemoryLocationError),
    InvalidInstructionName(String),
    InvalidFenceArgument(String),
}

struct Opcode(u32);

impl Opcode {
    pub fn from_u32(opcode: u32) -> Option<Opcode> {
        if opcode & !0x7F == 0 {
            Some(Opcode(opcode))
        } else {
            None
        }
    }
    pub fn to_u32(&self) -> u32 {
        self.0
    }
}

#[derive(Debug)]
struct Funct3(u32);

impl Funct3 {
    pub fn from_u32(funct3: u32) -> Option<Self> {
        if funct3 & !0x7 == 0 {
            Some(Funct3(funct3))
        } else {
            None
        }
    }
    pub fn to_u32(&self) -> u32 {
        self.0
    }
}

#[derive(Debug)]
struct Funct7(u32);

impl Funct7 {
    pub fn from_u32(funct7: u32) -> Option<Self> {
        if funct7 & !0x7F == 0 {
            Some(Funct7(funct7))
        } else {
            None
        }
    }
    pub fn to_u32(&self) -> u32 {
        self.0
    }
}

#[derive(Debug)]
struct Fm(u32);

impl Fm {
    pub fn from_u32(funct7: u32) -> Option<Self> {
        if funct7 & !0xF == 0 {
            Some(Self(funct7))
        } else {
            None
        }
    }
    pub fn to_u32(&self) -> u32 {
        self.0
    }
}

#[derive(Debug)]
struct FenceArg<T>(u32, PhantomData<T>);

impl<T> FenceArg<T> {
    pub fn from_u32(arg: u32) -> Option<Self> {
        if arg & !0xF == 0 {
            Some(Self(arg, PhantomData))
        } else {
            None
        }
    }
    pub fn to_u32(&self) -> u32 {
        self.0
    }
}

#[derive(Debug)]
struct FencePredecessor;

#[derive(Debug)]
struct FenceSuccessor;

#[derive(Debug)]
pub struct U {
    args: (Register<Rd>, Immediate<immediate::U>),
}

#[derive(Debug)]
pub struct J {
    args: (Register<Rd>, Immediate<immediate::J>),
}

#[derive(Debug)]
pub struct I {
    args: (Register<Rd>, Register<Rs1>, Immediate<immediate::I>),
}

#[derive(Debug)]
pub struct Fence {
    args: (
        Register<Rs1>,
        Register<Rs1>,
        FenceArg<FenceSuccessor>,
        FenceArg<FencePredecessor>,
        Fm,
    ),
}

#[derive(Debug)]
pub struct S {
    args: (Register<Rs1>, Register<Rs2>, Immediate<immediate::S>),
}

#[derive(Debug)]
pub struct R {
    args: (Register<Rd>, Register<Rs1>, Register<Rs2>),
}

#[derive(Debug)]
pub struct B {
    args: (Register<Rs1>, Register<Rs2>, Immediate<immediate::B>),
}

#[derive(Debug)]
pub enum Instruction {
    Lui(U),
    Auipc(U),
    Jal(J),
    Jalr(I),
    Beq(B),
    Bne(B),
    Blt(B),
    Bge(B),
    Bltu(B),
    Bgeu(B),
    Lb(I),
    Lh(I),
    Lw(I),
    Lbu(I),
    Lhu(I),
    Sb(S),
    Sh(S),
    Sw(S),
    Addi(I),
    Slti(I),
    Sltiu(I),
    Xori(I),
    Ori(I),
    Andi(I),
    Add(R),
    Sub(R),
    Sll(R),
    Slt(R),
    Sltu(R),
    Xor(R),
    Srl(R),
    Sra(R),
    Or(R),
    And(R),
    Fence(Fence),
    FenceI(I),
}

#[derive(Debug)]
pub enum Format {
    U,
    J,
    I,
    B,
    R,
    S,
    Fence,
}

trait Placeable {
    const MASK: u32;
    fn place_unchecked(&self) -> u32;
    fn place(&self) -> u32 {
        let bits = self.place_unchecked();
        if bits & !Self::MASK == 0 {
            bits
        } else {
            panic!("Placed bits do not fit within the mask.");
        }
    }
}

pub trait RegisterPlacement {
    const OFFSET: u32;
}

impl<R: RegisterPlacement> Placeable for Register<R> {
    const MASK: u32 = 0b11111 << R::OFFSET;

    fn place_unchecked(&self) -> u32 {
        self.to_u32() << R::OFFSET
    }
}

impl RegisterPlacement for Rd {
    const OFFSET: u32 = 7;
}

impl RegisterPlacement for Rs1 {
    const OFFSET: u32 = 15;
}

impl RegisterPlacement for Rs2 {
    const OFFSET: u32 = 22;
}

impl Placeable for Immediate<immediate::U> {
    const MASK: u32 = 0xFFFFF << 12;

    fn place_unchecked(&self) -> u32 {
        (self.to_i32() as u32) << 12
    }
}

impl Placeable for Immediate<immediate::I> {
    const MASK: u32 = 0xFFF << 20;

    fn place_unchecked(&self) -> u32 {
        (self.to_i32() as u32 & 0xFFF) << 20
    }
}

impl Placeable for Immediate<immediate::S> {
    const MASK: u32 = 0xFE000F80;

    fn place_unchecked(&self) -> u32 {
        let imm_ = self.to_i32() as u32;

        // imm[11:5] -> inst[31:25]
        // imm[4:0]  -> inst[11:7]
        ((imm_ & 0b111111100000) << 20) | ((imm_ & 0b000000011111) << 07)
    }
}

impl Placeable for Immediate<immediate::J> {
    const MASK: u32 = 0xFFFFF000;

    fn place_unchecked(&self) -> u32 {
        let imm_ = self.to_i32() as u32;
        // imm[20]    -> inst[31]
        // imm[10:1]  -> inst[30:21]
        // imm[11]    -> inst[20]
        // imm[19:12] ->, inst[19:12]
        000 | ((imm_ & 0x100000) << 11)
            | ((imm_ & 0x0007FE) << 20)
            | ((imm_ & 0x000800) << 09)
            | ((imm_ & 0x0FF000) << 00)
    }
}

impl Placeable for Immediate<immediate::B> {
    const MASK: u32 = 0xFE000F80;

    fn place_unchecked(&self) -> u32 {
        let imm_ = self.to_i32() as u32;
        // imm[12]   -> inst[31]
        // imm[10:5] -> inst[30:25]
        // imm[4:1]  -> inst[11:8]
        // imm[11]   -> inst[7]
        000 | ((imm_ & 0b1000000000000) << 19)
            | ((imm_ & 0b0011111100000) << 20)
            | ((imm_ & 0b0000000011110) << 07)
            | ((imm_ & 0b0100000000000) >> 04)
    }
}

impl Placeable for Opcode {
    const MASK: u32 = 0x3F;

    fn place_unchecked(&self) -> u32 {
        self.to_u32()
    }
}

impl Placeable for Funct3 {
    const MASK: u32 = 0x3000;

    fn place_unchecked(&self) -> u32 {
        self.to_u32() << 12
    }
}

impl Placeable for Funct7 {
    const MASK: u32 = 0xF3000000;

    fn place_unchecked(&self) -> u32 {
        self.to_u32() << 25
    }
}

impl Placeable for Fm {
    const MASK: u32 = 0xF0000000;

    fn place_unchecked(&self) -> u32 {
        self.to_u32() << 28
    }
}

impl Placeable for FenceArg<FenceSuccessor> {
    const MASK: u32 = 0x00F00000;

    fn place_unchecked(&self) -> u32 {
        self.to_u32() << 20
    }
}

impl Placeable for FenceArg<FencePredecessor> {
    const MASK: u32 = 0x0F000000;

    fn place_unchecked(&self) -> u32 {
        self.to_u32() << 24
    }
}

impl U {
    fn from_args(args: &[&str]) -> Result<Self, Error> {
        if args.len() != 2 {
            Result::Err(Error::WrongNumberOfArgs {
                actual: args.len(),
                expected: &[2],
            })
        } else {
            let rd = Register::from_str(args[0]).map_err(Error::InvalidRegisterArg)?;
            let imm = Immediate::from_str(args[1]).map_err(Error::InvalidImmediateArg)?;

            Ok(U { args: (rd, imm) })
        }
    }

    fn to_u32(&self, opcode: &Opcode) -> u32 {
        let (rd, imm) = &self.args;
        opcode.place() | rd.place() | imm.place()
    }
}

impl fmt::Display for U {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let (rd, imm) = &self.args;

        write!(
            f,
            "x{rd},0x{imm:x}", // TODO: limited to 12 bits?
            rd = rd.to_u32(),
            imm = imm.to_i32()
        )
    }
}

impl J {
    fn from_args(args: &[&str]) -> Result<Self, Error> {
        if args.len() != 2 {
            Result::Err(Error::WrongNumberOfArgs {
                actual: args.len(),
                expected: &[2],
            })
        } else {
            let rd = Register::from_str(args[0]).map_err(Error::InvalidRegisterArg)?;
            let imm = Immediate::from_str(args[1]).map_err(Error::InvalidImmediateArg)?;

            Ok(J { args: (rd, imm) })
        }
    }

    fn to_u32(&self, opcode: &Opcode) -> u32 {
        let (rd, imm) = &self.args;
        opcode.place() | rd.place() | imm.place()
    }
}

impl fmt::Display for J {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let (rd, imm) = &self.args;

        write!(
            f,
            "x{rd},{imm}", // TODO: limited to 12 bits?
            rd = rd.to_u32(),
            imm = imm.to_i32()
        )
    }
}

fn get_memory_argument(
    memory_location: &str,
) -> Result<(Register<Rs1>, Immediate<immediate::I>), Error> {
    let open_bracket_index = memory_location
        .find('(')
        .ok_or(Error::InvalidMemoryLocationArg(
            GetMemoryLocationError::MissingOpenParentheses,
        ))?;

    let close_bracket_index = memory_location
        .find(')')
        .ok_or(Error::InvalidMemoryLocationArg(
            GetMemoryLocationError::MissingCloseParentheses,
        ))?;

    if close_bracket_index != memory_location.len() - 1 {
        dbg!(memory_location);
        return Err(Error::InvalidMemoryLocationArg(
            GetMemoryLocationError::TextAfterCloseParenthesis,
        ));
    }

    let rs1 = Register::from_str(&memory_location[open_bracket_index + 1..close_bracket_index])
        .map_err(Error::InvalidRegisterArg)?;

    let offset = Immediate::from_str(&memory_location[0..open_bracket_index])
        .map_err(Error::InvalidImmediateArg)?;
    Ok((rs1, offset))
}

impl I {
    fn from_args(args: &[&str]) -> Result<Self, Error> {
        if args.len() != 3 {
            Result::Err(Error::WrongNumberOfArgs {
                actual: args.len(),
                expected: &[3],
            })
        } else {
            let rd = Register::from_str(args[0]).map_err(Error::InvalidRegisterArg)?;
            let rs1 = Register::from_str(args[1]).map_err(Error::InvalidRegisterArg)?;
            let imm = Immediate::from_str(args[2]).map_err(Error::InvalidImmediateArg)?;

            Ok(I {
                args: (rd, rs1, imm),
            })
        }
    }

    fn load_from_args(args: &[&str]) -> Result<Self, Error> {
        if args.len() != 2 {
            Result::Err(Error::WrongNumberOfArgs {
                actual: args.len(),
                expected: &[2],
            })
        } else {
            let rd = Register::from_str(args[0]).map_err(Error::InvalidRegisterArg)?;
            let (rs1, offset) = get_memory_argument(args[1])?;

            Ok(I {
                args: (rd, rs1, offset),
            })
        }
    }

    fn to_u32(&self, opcode: &Opcode, funct3: &Funct3) -> u32 {
        let (rd, rs1, imm) = &self.args;
        opcode.place() | rd.place() | funct3.place() | rs1.place() | imm.place()
    }
}

impl fmt::Display for I {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let (rd, rs1, imm) = &self.args;

        write!(
            f,
            "x{rd},x{rs1},{imm}", // TODO: limited to 12 bits?
            rd = rd.to_u32(),
            rs1 = rs1.to_u32(),
            imm = imm.to_i32()
        )
    }
}

impl S {
    fn from_args(args: &[&str]) -> Result<Self, Error> {
        if args.len() != 2 {
            Result::Err(Error::WrongNumberOfArgs {
                actual: args.len(),
                expected: &[2],
            })
        } else {
            let rs2 = Register::from_str(args[0]).map_err(Error::InvalidRegisterArg)?;
            let (rs1, offset) = get_memory_argument(args[1])?;
            Ok(S {
                args: (rs1, rs2, Immediate::from_i32(offset.to_i32()).unwrap()),
            })
        }
    }

    fn to_u32(&self, opcode: &Opcode, funct3: &Funct3) -> u32 {
        let (rs1, rs2, imm) = &self.args;
        opcode.place() | funct3.place() | rs1.place() | rs2.place() | imm.place()
    }
}

impl fmt::Display for S {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let (rs1, rs2, imm) = &self.args;

        write!(
            f,
            "x{rs2},{imm}({rs1})", // TODO: limited to 12 bits?
            rs1 = rs1.to_u32(),
            rs2 = rs2.to_u32(),
            imm = imm.to_i32(),
        )
    }
}

impl R {
    fn from_args(args: &[&str]) -> Result<Self, Error> {
        if args.len() != 3 {
            Result::Err(Error::WrongNumberOfArgs {
                actual: args.len(),
                expected: &[3],
            })
        } else {
            let rd = Register::from_str(args[0]).map_err(Error::InvalidRegisterArg)?;
            let rs1 = Register::from_str(args[1]).map_err(Error::InvalidRegisterArg)?;
            let rs2 = Register::from_str(args[2]).map_err(Error::InvalidRegisterArg)?;

            Ok(R {
                args: (rd, rs1, rs2),
            })
        }
    }

    fn to_u32(&self, opcode: &Opcode, funct3: &Funct3, funct7: &Funct7) -> u32 {
        let (rd, rs1, rs2) = &self.args;
        opcode.place() | rd.place() | funct3.place() | rs1.place() | rs2.place() | funct7.place()
    }
}

impl fmt::Display for R {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let (rd, rs1, rs2) = &self.args;

        write!(
            f,
            "x{rd},x{rs1},{rs2}",
            rd = rd.to_u32(),
            rs1 = rs1.to_u32(),
            rs2 = rs2.to_u32()
        )
    }
}

impl B {
    fn from_args(args: &[&str]) -> Result<Self, Error> {
        if args.len() != 3 {
            Result::Err(Error::WrongNumberOfArgs {
                actual: args.len(),
                expected: &[3],
            })
        } else {
            let rs1 = Register::from_str(args[0]).map_err(Error::InvalidRegisterArg)?;
            let rs2 = Register::from_str(args[1]).map_err(Error::InvalidRegisterArg)?;
            let imm = Immediate::from_str(args[2]).map_err(Error::InvalidImmediateArg)?;

            Ok(B {
                args: (rs1, rs2, imm),
            })
        }
    }

    fn to_u32(&self, opcode: &Opcode, funct3: &Funct3) -> u32 {
        let (rs1, rs2, imm) = &self.args;
        opcode.place() | imm.place() | funct3.place() | rs1.place() | rs2.place()
    }
}

impl fmt::Display for B {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let (rs1, rs2, imm) = &self.args;

        write!(
            f,
            "x{rs1},x{rs2},{imm}",
            rs1 = rs1.to_u32(),
            rs2 = rs2.to_u32(),
            imm = imm.to_i32()
        )
    }
}

impl<T> FromStr for FenceArg<T> {
    type Err = ();

    fn from_str<'a>(mut arg: &str) -> Result<Self, ()> {
        // TODO: surely we can make this nicer?
        let mut fence_arg = 0;
        let mut possible_characters: &[char] = &['i', 'o', 'r', 'w'];

        while let Some((c, rest)) = possible_characters.split_first() {
            possible_characters = rest;
            fence_arg <<= 1;
            if arg.contains(*c) {
                if arg.starts_with(*c) {
                    fence_arg |= 1;
                    let tuple = arg.split_at(1);
                    arg = tuple.1;
                } else {
                    return Err(());
                }
            }
        }
        Ok(FenceArg::from_u32(fence_arg).unwrap())
    }
}

impl Fence {
    fn from_args(args: &[&str]) -> Result<Self, Error> {
        if args.len() == 0 {
            Ok(Fence {
                args: (
                    Register::from_u32(0).unwrap(),
                    Register::from_u32(0).unwrap(),
                    FenceArg::from_u32(0b1111).unwrap(),
                    FenceArg::from_u32(0b1111).unwrap(),
                    Fm::from_u32(0).unwrap(),
                ),
            })
        } else if args.len() == 2 {
            let pred: FenceArg<FencePredecessor> = args[0]
                .parse()
                .or(Err(Error::InvalidFenceArgument(args[0].to_string())))?;
            let succ: FenceArg<FenceSuccessor> = args[1]
                .parse()
                .or(Err(Error::InvalidFenceArgument(args[0].to_string())))?;

            Ok(Fence {
                args: (
                    Register::from_u32(0).unwrap(),
                    Register::from_u32(0).unwrap(),
                    succ,
                    pred,
                    Fm::from_u32(0).unwrap(),
                ),
            })
        } else {
            Err(Error::WrongNumberOfArgs {
                actual: args.len(),
                expected: &[0, 2],
            })
        }
    }

    fn to_u32(&self, opcode: &Opcode, funct3: &Funct3) -> u32 {
        let (rd, rs1, succ, pred, rm) = &self.args;
        opcode.place()
            | rd.place()
            | funct3.place()
            | rs1.place()
            | succ.place()
            | pred.place()
            | rm.place()
    }
}

impl fmt::Display for Fence {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let (_, _, succ, pred, _) = &self.args;

        let to_fence_arg = |bits| {
            [
                if (bits & 0b1000) == 0 { "" } else { "i" },
                if (bits & 0b0100) == 0 { "" } else { "o" },
                if (bits & 0b0010) == 0 { "" } else { "r" },
                if (bits & 0b0001) == 0 { "" } else { "w" },
            ]
            .concat()
        };

        write!(
            f,
            "{pred}, {succ}",
            pred = to_fence_arg(pred.to_u32()),
            succ = to_fence_arg(succ.to_u32())
        )
    }
}

fn parse_no_args(args: &[&str]) -> Result<I, Error> {
    if args.len() == 0 {
        Ok(I {
            args: (
                Register::from_u32(0).unwrap(),
                Register::from_u32(0).unwrap(),
                Immediate::from_i32(0).unwrap(),
            ),
        })
    } else {
        Err(Error::WrongNumberOfArgs {
            actual: args.len(),
            expected: &[0],
        })
    }
}

impl FromStr for Instruction {
    type Err = Error;

    fn from_str<'a>(mnemonic: &str) -> Result<Self, Error> {
        let mnemonic = mnemonic.trim();

        let first_space_index = mnemonic.find(' ').unwrap_or(mnemonic.len());

        let (name, args) = mnemonic.split_at(first_space_index);

        let args: Vec<&str> = if args.is_empty() {
            vec![]
        } else {
            args.split(',').map(str::trim).collect()
        };

        match name.to_ascii_lowercase().as_str() {
            "lui" => U::from_args(&args).map(Instruction::Lui),
            "auipc" => U::from_args(&args).map(Instruction::Auipc),
            "jal" => J::from_args(&args).map(Instruction::Jal),
            "jalr" => I::from_args(&args).map(Instruction::Jalr),
            "beq" => B::from_args(&args).map(Instruction::Beq),
            "bne" => B::from_args(&args).map(Instruction::Bne),
            "blt" => B::from_args(&args).map(Instruction::Blt),
            "bge" => B::from_args(&args).map(Instruction::Bge),
            "bltu" => B::from_args(&args).map(Instruction::Bltu),
            "bgeu" => B::from_args(&args).map(Instruction::Bgeu),
            "lb" => I::load_from_args(&args).map(Instruction::Lb),
            "lh" => I::load_from_args(&args).map(Instruction::Lh),
            "lw" => I::load_from_args(&args).map(Instruction::Lw),
            "lbu" => I::load_from_args(&args).map(Instruction::Lbu),
            "lhu" => I::load_from_args(&args).map(Instruction::Lhu),
            "sb" => S::from_args(&args).map(Instruction::Sb),
            "sh" => S::from_args(&args).map(Instruction::Sh),
            "sw" => S::from_args(&args).map(Instruction::Sw),
            "addi" => I::from_args(&args).map(Instruction::Addi),
            "slti" => I::from_args(&args).map(Instruction::Slti),
            "sltiu" => I::from_args(&args).map(Instruction::Sltiu),
            "xori" => I::from_args(&args).map(Instruction::Xori),
            "ori" => I::from_args(&args).map(Instruction::Ori),
            "andi" => I::from_args(&args).map(Instruction::Andi),
            "add" => R::from_args(&args).map(Instruction::Add),
            "sub" => R::from_args(&args).map(Instruction::Sub),
            "sll" => R::from_args(&args).map(Instruction::Sll),
            "slt" => R::from_args(&args).map(Instruction::Slt),
            "sltu" => R::from_args(&args).map(Instruction::Sltu),
            "xor" => R::from_args(&args).map(Instruction::Xor),
            "srl" => R::from_args(&args).map(Instruction::Srl),
            "sra" => R::from_args(&args).map(Instruction::Sra),
            "or" => R::from_args(&args).map(Instruction::Or),
            "and" => R::from_args(&args).map(Instruction::And),
            "fence" => Fence::from_args(&args).map(Instruction::Fence),
            "fence.i" => parse_no_args(&args).map(Instruction::FenceI),
            _ => Err(Error::InvalidInstructionName(name.to_string())),
        }
    }
}

impl fmt::Display for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Instruction::Lui(u) => write!(f, "lui {}", u),
            Instruction::Auipc(u) => write!(f, "auipc {}", u),
            Instruction::Jal(j) => write!(f, "jal {}", j),
            Instruction::Jalr(i) => write!(f, "jalr {}", i),
            Instruction::Beq(b) => write!(f, "beq {}", b),
            Instruction::Bne(b) => write!(f, "bne {}", b),
            Instruction::Blt(b) => write!(f, "blt {}", b),
            Instruction::Bge(b) => write!(f, "bge {}", b),
            Instruction::Bltu(b) => write!(f, "bktu {}", b),
            Instruction::Bgeu(b) => write!(f, "bgeu {}", b),
            Instruction::Lb(i) => write!(f, "lb {}", i),
            Instruction::Lh(i) => write!(f, "lh {}", i),
            Instruction::Lw(i) => write!(f, "lw {}", i),
            Instruction::Lbu(i) => write!(f, "lbu {}", i),
            Instruction::Lhu(i) => write!(f, "lhu {}", i),
            Instruction::Sb(s) => write!(f, "sb {}", s),
            Instruction::Sh(s) => write!(f, "sh {}", s),
            Instruction::Sw(s) => write!(f, "sw {}", s),
            Instruction::Addi(i) => write!(f, "addi {}", i),
            Instruction::Slti(i) => write!(f, "slti {}", i),
            Instruction::Sltiu(i) => write!(f, "sltiu {}", i),
            Instruction::Xori(i) => write!(f, "xori {}", i),
            Instruction::Ori(i) => write!(f, "ori {}", i),
            Instruction::Andi(i) => write!(f, "andi {}", i),
            Instruction::Add(r) => write!(f, "add {}", r),
            Instruction::Sub(r) => write!(f, "sub {}", r),
            Instruction::Sll(r) => write!(f, "sll {}", r),
            Instruction::Slt(r) => write!(f, "slt {}", r),
            Instruction::Sltu(r) => write!(f, "sltu {}", r),
            Instruction::Xor(r) => write!(f, "xor {}", r),
            Instruction::Srl(r) => write!(f, "srl {}", r),
            Instruction::Sra(r) => write!(f, "sra {}", r),
            Instruction::Or(r) => write!(f, "or {}", r),
            Instruction::And(r) => write!(f, "and {}", r),
            Instruction::Fence(i) => write!(f, "fence {}", i),
            Instruction::FenceI(i) => write!(f, "fence.i {}", i),
        }
    }
}

impl Instruction {
    pub fn to_u32(&self) -> u32 {
        match self {
            Instruction::Lui(u) => u.to_u32(&Opcode::from_u32(0b0110111).unwrap()),
            Instruction::Auipc(u) => u.to_u32(&Opcode::from_u32(0b0010111).unwrap()),
            Instruction::Jal(j) => j.to_u32(&Opcode::from_u32(0b1101111).unwrap()),
            Instruction::Jalr(i) => i.to_u32(
                &Opcode::from_u32(0b1100111).unwrap(),
                &Funct3::from_u32(0b000).unwrap(),
            ),
            Instruction::Beq(b) => b.to_u32(
                &Opcode::from_u32(0b1100011).unwrap(),
                &Funct3::from_u32(0b000).unwrap(),
            ),
            Instruction::Bne(b) => b.to_u32(
                &Opcode::from_u32(0b1100011).unwrap(),
                &Funct3::from_u32(0b001).unwrap(),
            ),
            Instruction::Blt(b) => b.to_u32(
                &Opcode::from_u32(0b1100011).unwrap(),
                &Funct3::from_u32(0b100).unwrap(),
            ),
            Instruction::Bge(b) => b.to_u32(
                &Opcode::from_u32(0b1100011).unwrap(),
                &Funct3::from_u32(0b101).unwrap(),
            ),
            Instruction::Bltu(b) => b.to_u32(
                &Opcode::from_u32(0b1100011).unwrap(),
                &Funct3::from_u32(0b110).unwrap(),
            ),
            Instruction::Bgeu(b) => b.to_u32(
                &Opcode::from_u32(0b1100011).unwrap(),
                &Funct3::from_u32(0b111).unwrap(),
            ),
            Instruction::Lb(i) => i.to_u32(
                &Opcode::from_u32(0b0000011).unwrap(),
                &Funct3::from_u32(0b000).unwrap(),
            ),
            Instruction::Lh(i) => i.to_u32(
                &Opcode::from_u32(0b0000011).unwrap(),
                &Funct3::from_u32(0b001).unwrap(),
            ),
            Instruction::Lw(i) => i.to_u32(
                &Opcode::from_u32(0b0000011).unwrap(),
                &Funct3::from_u32(0b010).unwrap(),
            ),
            Instruction::Lbu(i) => i.to_u32(
                &Opcode::from_u32(0b0000011).unwrap(),
                &Funct3::from_u32(0b100).unwrap(),
            ),
            Instruction::Lhu(i) => i.to_u32(
                &Opcode::from_u32(0b0000011).unwrap(),
                &Funct3::from_u32(0b101).unwrap(),
            ),
            Instruction::Sb(s) => s.to_u32(
                &Opcode::from_u32(0b0100011).unwrap(),
                &Funct3::from_u32(0b000).unwrap(),
            ),
            Instruction::Sh(s) => s.to_u32(
                &Opcode::from_u32(0b0100011).unwrap(),
                &Funct3::from_u32(0b001).unwrap(),
            ),
            Instruction::Sw(s) => s.to_u32(
                &Opcode::from_u32(0b0100011).unwrap(),
                &Funct3::from_u32(0b010).unwrap(),
            ),
            Instruction::Addi(i) => i.to_u32(
                &Opcode::from_u32(0b0010011).unwrap(),
                &Funct3::from_u32(0b000).unwrap(),
            ),
            Instruction::Slti(i) => i.to_u32(
                &Opcode::from_u32(0b0010011).unwrap(),
                &Funct3::from_u32(0b010).unwrap(),
            ),
            Instruction::Sltiu(i) => i.to_u32(
                &Opcode::from_u32(0b0010011).unwrap(),
                &Funct3::from_u32(0b011).unwrap(),
            ),
            Instruction::Xori(i) => i.to_u32(
                &Opcode::from_u32(0b0010011).unwrap(),
                &Funct3::from_u32(0b100).unwrap(),
            ),
            Instruction::Ori(i) => i.to_u32(
                &Opcode::from_u32(0b0010011).unwrap(),
                &Funct3::from_u32(0b110).unwrap(),
            ),
            Instruction::Andi(i) => i.to_u32(
                &Opcode::from_u32(0b0010011).unwrap(),
                &Funct3::from_u32(0b111).unwrap(),
            ),
            Instruction::Add(r) => r.to_u32(
                &Opcode::from_u32(0b0110011).unwrap(),
                &Funct3::from_u32(0b000).unwrap(),
                &Funct7::from_u32(0b0000000).unwrap(),
            ),
            Instruction::Sub(r) => r.to_u32(
                &Opcode::from_u32(0b0110011).unwrap(),
                &Funct3::from_u32(0b000).unwrap(),
                &Funct7::from_u32(0b0100000).unwrap(),
            ),
            Instruction::Sll(r) => r.to_u32(
                &Opcode::from_u32(0b0110011).unwrap(),
                &Funct3::from_u32(0b001).unwrap(),
                &Funct7::from_u32(0b0000000).unwrap(),
            ),
            Instruction::Slt(r) => r.to_u32(
                &Opcode::from_u32(0b0110011).unwrap(),
                &Funct3::from_u32(0b010).unwrap(),
                &Funct7::from_u32(0b0000000).unwrap(),
            ),
            Instruction::Sltu(r) => r.to_u32(
                &Opcode::from_u32(0b0110011).unwrap(),
                &Funct3::from_u32(0b011).unwrap(),
                &Funct7::from_u32(0b0000000).unwrap(),
            ),
            Instruction::Xor(r) => r.to_u32(
                &Opcode::from_u32(0b0110011).unwrap(),
                &Funct3::from_u32(0b100).unwrap(),
                &Funct7::from_u32(0b0000000).unwrap(),
            ),
            Instruction::Srl(r) => r.to_u32(
                &Opcode::from_u32(0b0110011).unwrap(),
                &Funct3::from_u32(0b101).unwrap(),
                &Funct7::from_u32(0b0000000).unwrap(),
            ),
            Instruction::Sra(r) => r.to_u32(
                &Opcode::from_u32(0b0110011).unwrap(),
                &Funct3::from_u32(0b101).unwrap(),
                &Funct7::from_u32(0b0100000).unwrap(),
            ),
            Instruction::Or(r) => r.to_u32(
                &Opcode::from_u32(0b0110011).unwrap(),
                &Funct3::from_u32(0b110).unwrap(),
                &Funct7::from_u32(0b0000000).unwrap(),
            ),
            Instruction::And(r) => r.to_u32(
                &Opcode::from_u32(0b0110011).unwrap(),
                &Funct3::from_u32(0b111).unwrap(),
                &Funct7::from_u32(0b0000000).unwrap(),
            ),
            Instruction::Fence(f) => f.to_u32(
                &Opcode::from_u32(0b0001111).unwrap(),
                &Funct3::from_u32(0b000).unwrap(),
            ),
            Instruction::FenceI(i) => i.to_u32(
                &Opcode::from_u32(0b0001111).unwrap(),
                &Funct3::from_u32(0b001).unwrap(),
            ),
        }
    }

    pub fn to_format(&self) -> Format {
        match self {
            Instruction::Lui(_) => Format::U,
            Instruction::Auipc(_) => Format::U,
            Instruction::Jal(_) => Format::J,
            Instruction::Jalr(_) => Format::I,
            Instruction::Beq(_) => Format::B,
            Instruction::Bne(_) => Format::B,
            Instruction::Blt(_) => Format::B,
            Instruction::Bge(_) => Format::B,
            Instruction::Bltu(_) => Format::B,
            Instruction::Bgeu(_) => Format::B,
            Instruction::Lb(_) => Format::I,
            Instruction::Lh(_) => Format::I,
            Instruction::Lw(_) => Format::I,
            Instruction::Lbu(_) => Format::I,
            Instruction::Lhu(_) => Format::I,
            Instruction::Sb(_) => Format::S,
            Instruction::Sh(_) => Format::S,
            Instruction::Sw(_) => Format::S,
            Instruction::Addi(_) => Format::I,
            Instruction::Slti(_) => Format::I,
            Instruction::Sltiu(_) => Format::I,
            Instruction::Xori(_) => Format::I,
            Instruction::Ori(_) => Format::I,
            Instruction::Andi(_) => Format::I,
            Instruction::Add(_) => Format::R,
            Instruction::Sub(_) => Format::R,
            Instruction::Sll(_) => Format::R,
            Instruction::Slt(_) => Format::R,
            Instruction::Sltu(_) => Format::R,
            Instruction::Xor(_) => Format::R,
            Instruction::Srl(_) => Format::R,
            Instruction::Sra(_) => Format::R,
            Instruction::Or(_) => Format::R,
            Instruction::And(_) => Format::R,
            Instruction::Fence(_) => Format::Fence,
            Instruction::FenceI(_) => Format::I,
        }
    }
}
