use core::{fmt, panic};

use crate::common::{OpCode, Value};

#[derive(Debug)]
struct LineStart {
    offset: usize,
    line: usize,
}

#[derive(Debug)]
pub struct Chunk {
    pub code: Vec<u8>,
    pub constants: Vec<Value>,
    line_starts: Vec<LineStart>,
}

impl Chunk {
    pub fn new() -> Self {
        Chunk {
            code: Vec::new(),
            constants: Vec::new(),
            line_starts: Vec::new(),
        }
    }

    pub fn write_byte(&mut self, byte: u8) {
        self.code.push(byte);
    }

    pub fn add_constant(&mut self, value: Value) -> u8 {
        self.constants.push(value);
        (self.constants.len() - 1) as u8
    }

    pub fn get_line(&self, offset: usize) -> usize {
        self.line_starts
            .iter()
            .rev()
            .find(|line_start| offset >= line_start.offset)
            .map(|line_start| line_start.line)
            .unwrap_or_default()
    }

    pub fn disassemble(&self, name: &str) -> fmt::Result {
        println!("== {} start ==", name);
        let mut offset = 0;

        while offset < self.code.len() {
            offset = self.disassemble_instruction(offset)?;
        }
        println!("==  {} end  ==", name);
        Ok(())
    }

    pub fn disassemble_instruction(&self, offset: usize) -> Result<usize, fmt::Error> {
        if offset >= self.code.len() {
            return Err(fmt::Error);
        }

        let line = self.get_line(offset);
        print!("{:04x} ", offset);

        if offset > 0 && line == self.get_line(offset - 1) {
            print!("   | ");
        } else {
            print!("{:4} ", line);
        }

        let opcode = OpCode::from(self.code[offset]);
        let operand_count = opcode.operand_count();

        match operand_count {
            0 => {
                println!("{:?}", opcode);
                Ok(offset + 1)
            }
            1 => {
                if offset + 1 >= self.code.len() {
                    panic!("ERROR: {:?} missing operand", opcode);
                }
                let operand = self.code[offset + 1];
                match opcode {
                    OpCode::Constant => {
                        let value = self.constants[operand as usize];
                        println!("{:?} {} ", opcode, value);
                    }
                    _ => println!("{:?} {:4} ", opcode, operand),
                }
                Ok(offset + 2)
            }
            2 => {
                if offset + 2 >= self.code.len() {
                    println!("ERROR: {:?} missing operands", opcode);
                    return Ok(offset + 1);
                }
                let operand1 = self.code[offset + 1];
                let operand2 = self.code[offset + 2];

                println!("{:?} {:02x}{:02x}", opcode, operand1, operand2);
                Ok(offset + 3)
            }
            _ => {
                panic!(
                    "{:?} (unsupported operand count: {})",
                    opcode, operand_count
                )
            }
        }
    }
}

impl fmt::Display for Chunk {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        writeln!(f, "=== Chunk ===")?;
        for (i, byte) in self.code.iter().enumerate() {
            writeln!(f, "{:04} {:?}", i, byte)?;
        }
        Ok(())
    }
}
