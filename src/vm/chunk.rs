use core::fmt;

use rlox_gc::Trace;

use crate::common::{OpCode, Value};

#[derive(Debug, Trace)]
struct LineStart {
    offset: usize,
    line: usize,
}

#[derive(Debug, Trace)]
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

    pub fn write_byte(&mut self, byte: u8, line: usize) {
        self.code.push(byte);
        if self
            .line_starts
            .last()
            .map(|ls| ls.line != line)
            .unwrap_or(true)
        {
            self.line_starts.push(LineStart {
                offset: self.code.len() - 1,
                line,
            });
        }
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

    pub fn instruction_size(&self, offset: usize) -> usize {
        let opcode = OpCode::from(self.code[offset]);
        match opcode {
            OpCode::Closure => {
                let func_idx = self.code[offset + 1];
                if let Value::Function(func) = self.constants[func_idx as usize] {
                    1 + (func.upvalue_count as usize)
                } else {
                    1
                }
            }
            other => other.operand_count(),
        }
    }

    pub fn disassemble(&self, name: &str) {
        println!("== {} start ==", name);
        let mut offset = 0;

        while offset < self.code.len() {
            offset += self.disassemble_instruction(offset);
        }
        println!("==  {} end  ==", name);
    }

    pub fn disassemble_instruction(&self, offset: usize) -> usize {
        let line = self.get_line(offset);
        print!("{:04x} ", offset);

        if offset > 0 && line == self.get_line(offset - 1) {
            print!("   | ");
        } else {
            print!("{:4} ", line);
        }

        let opcode = OpCode::from(self.code[offset]);
        let operand_count = self.instruction_size(offset);
        print!("{:?}", opcode);
        for op in 0..operand_count {
            print!(" {}", op);
        }
        println!();

        operand_count + 1
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
