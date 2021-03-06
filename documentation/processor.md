# Processor

The processor originally support the [RV32I instruction set](https://riscv.org/specifications) and has a five-stage pipeline, forwarding unit and branch predictor.
It was modified to be used as the backend of the riscv-repl.

## Processor Modifications

The `cpu` module has an output `inst_in` which is set to the address of the next instruction to be evaluated (the program counter) and an input `inst_out` from which the instruction is read.
The modified processor entirely ignores the value of `inst_in`.
The input to `cpu`, `inst_in` is normally set to `0x13000000` (riscv `nop` instruction) except for the first processor clock cycle after an instruction is read from UART when is it set to that instruction.

Additionally, the processor clock input is normally held high.
Once an instruction is read from UART, the processor clock oscillates for six cycles (there are six rising edges) and then is held high again.
During these cycles, the instruction moves down the stages of the processor and the registers are updated.

The final modification is to add a 2024 bit output from the `register_file` module which allows the register file to be read and sent over UART.

## Processor Operation

When not executing an instruction processor clock `proc_clk` is held high and the processor sits idle.
The FPGA listens for bytes transmitted to it, and when four bytes have been received ([see UART Interface](#uart-interface)), sets `instruction_rcv` high for one clock cycle and stores the received instruction in `instruction_buffer`.

On the next rising edge of the clock, `do_execute` is driven high and the instruction is outputted to the processor.
The processor clock then cycles five times causing the processor to fully evaluate the instruction.
After five rising edges of `proc_clk`, `do_execute` is driven low and `proc_clk` is kept high.
For the first processor clock cycle of each instruction evaluated, `inst_out` is assigned to the instruction received by UART, at all other times it is assigned the noop instruction.
`inst_out` is read by the processor during the fetch stage and the processor will evaluate what ever instruction it reads.

Once the instruction has been executed `send_regfile` is driven high for one clock cycle which starts transmission of the register file from the FPGA to the computer.

The execution of instructions is summarised by the following [timing diagram](https://wavedrom.com/editor.html?{signal%3A%20[%20[%20%27Inputs%27%2C%0A%20%20{name%3A%20%27clk12%27%2C%20wave%3A%20%27p.............%27}%2C%0A%20%20{name%3A%20%27instruction_rcv%27%2C%20wave%3A%20%270.10..........%27}%2C%0A%20%20]%2C%20[%20%27Registers%27%2C%0A%20%20{name%3A%20%27do_execute%27%2C%20wave%3A%20%270.1........0..%27}%2C%0A%20%20{name%3A%20%27clocks_counter%27%2C%20wave%3A%20%27xx%3D%3D%3D%3D%3D%3D%3D%3D%3Dxxx%27%2C%20data%3A%20[0%2C%201%2C%202%2C%203%2C%204%2C%205%2C%206%2C%207%2C%208]}%2C%0A%20%20]%2C%20[%20%27Outputs%27%2C%0A%20%20{name%3A%20%27clk_proc%27%2C%20wave%3A%20%271.0101010101..%27}%2C%0A%20%20{name%3A%20%27inst_out%27%2C%20wave%3A%20%27%3D.%3D.%3D.........%27%2C%20data%3A%20[%20%27noop%27%2C%20%27instruction%27%2C%20%27noop%27%20]}%2C%0A%20%20{name%3A%20%27send_regfile%27%2C%20wave%3A%20%270..........10.%27%2C%20data%3A%20[%20%27noop%27%2C%20%27instruction%27%2C%20%27noop%27%20]}%2C%0A]%20]}%0A).

![RISCV REPL demo](/images/timing-diagram.png)

## UART Interface

As RISC-V is a little-endian all UART transmissions will also be little-endian.

### Input

The processor waits until it has read four bytes our UART.
These bytes should contain the instruction in little endian form.
For example the instruction `add t0, a0, a1` is `00B502B3` in binary and should be transmitted over UART as `0xB3`, `0x02`, `0xB5`, `0x00`.
Note that programs like `gcc` will generate machine that uses little endian ordering.
When sending an instruction generated by `gcc` to the processor send the bytes in the order that `gcc` stores them on disk.

| Index | Byte expected        |
|-------|----------------------|
| 0     | `inst & 0x000000FF`  |
| 1     | `inst & 0x0000FF00`  |
| 2     | `inst & 0x00FF0000`  |
| 3     | `inst & 0xFF000000`  |

## Output

After the instruction is evaluated the register file is transmitted over UART.
 and as such the the least significant byte of `x0` is transmitted first and the most significant byte of `x31` is transmitted last.
In all 128 bytes are transmitted.

| Index | Register name | Transmitted byte   |
|-------|---------------|--------------------|
| 0     | `x0`          | `x0 & 0x000000FF`  |
| 1     | `x0`          | `x0 & 0x0000FF00`  |
| 2     | `x0`          | `x0 & 0x00FF0000`  |
| 3     | `x0`          | `x0 & 0xFF000000`  |
| 4     | `x1`          | `x1 & 0x000000FF`  |
| 5     | `x1`          | `x1 & 0x0000FF00`  |
| 6     | `x1`          | `x1 & 0x00FF0000`  |
| 7     | `x1`          | `x1 & 0xFF000000`  |
| ...   | ...           | ...                |
| 124   | `x31`         | `x31 & 0x000000FF` |
| 125   | `x31`         | `x31 & 0x0000FF00` |
| 126   | `x31`         | `x31 & 0x00FF0000` |
| 127   | `x31`         | `x31 & 0xFF000000` |
