use std::{io::Write, mem};

use crate::{
    common::{ObjString, ObjectRef, OpCode, Value},
    compiler::Compiler,
    vm::{chunk::Chunk, heap::Heap},
};

#[derive(Debug)]
pub enum VMError {
    RuntimeError(String),
}

pub type VMResult = Result<(), VMError>;

#[derive(Debug)]
struct Stack<T, const N: usize> {
    data: [T; N],
    top: usize,
}

#[allow(dead_code)]
impl<T, const N: usize> Stack<T, N>
where
    T: Default,
    T: Copy,
    T: std::fmt::Display,
{
    fn new() -> Self {
        Self {
            data: std::array::from_fn(|_| Default::default()),
            top: 0,
        }
    }

    fn push(&mut self, item: T) {
        self.data[self.top] = item;
        self.top += 1;
    }

    fn pop(&mut self) -> T {
        self.top -= 1;
        mem::take(&mut self.data[self.top])
    }

    fn top(&self) -> T {
        self.data[self.top - 1]
    }

    fn peek(&self, offset: usize) -> T {
        self.data[self.top - 1 - offset]
    }

    fn is_empty(&self) -> bool {
        self.top == 0
    }

    fn is_full(&self) -> bool {
        self.top == N
    }

    fn len(&self) -> usize {
        self.top
    }

    fn capacity(&self) -> usize {
        N
    }

    fn print(&self)
    where
        T: std::fmt::Debug,
    {
        print!("Stack [");
        for i in 0..self.top {
            let val = &self.data[i];
            print!(" {}", val);
            if i != self.top - 1 {
                print!(", ")
            }
        }
        println!(" ]");
    }
}

#[derive(Debug)]
pub struct VM<'src> {
    chunk: Chunk<'src>,
    ip: usize,
    stack: Stack<Value<'src>, 256>,
    heap: Heap<'src>,
}

impl<'src> VM<'src> {
    pub fn repl() -> std::io::Result<()> {
        let stdin = std::io::stdin();
        let mut stdout = std::io::stdout();

        loop {
            print!("> ");
            stdout.flush()?;

            let mut line = String::new();

            match stdin.read_line(&mut line) {
                Ok(0) => {
                    println!();
                    break;
                }
                Ok(_) => {
                    let line = line.trim_end();
                    if let Err(e) = VM::interpret(line) {
                        eprintln!("Error: {:?}", e);
                    }
                }
                Err(e) => {
                    eprintln!("Error reading input: {}", e);
                    break;
                }
            }
        }

        Ok(())
    }

    pub fn run_file(source: &String) {}

    pub fn interpret(source: &str) -> VMResult {
        let mut chunk = Chunk::new();
        let mut heap = Heap::new();

        Compiler::compile(source, &mut chunk, &mut heap)?;

        let mut vm = VM {
            chunk,
            ip: 0,
            stack: Stack::new(),
            heap,
        };

        vm.run()
    }

    pub fn run(&mut self) -> VMResult {
        loop {
            let instruction = self.chunk.disassemble_instruction(self.ip);
            println!("{:?}", instruction);
            self.stack.print();
            let opcode = OpCode::from(self.read_byte()?);
            match opcode {
                OpCode::OpConstant => {
                    let idx = self.read_byte()?;
                    let constant = self.read_constant(idx);
                    self.stack.push(constant);
                    println!("{:?}", constant);
                }
                OpCode::OpNegate => {
                    let value = self.stack.pop();
                    if let Value::Number(val) = value {
                        self.stack.push(Value::Number(-val));
                    } else {
                        return Err(VMError::RuntimeError(
                            "Operand must be a number.".to_string(),
                        ));
                    }
                }
                OpCode::OpAdd => {
                    let b = self.stack.pop();
                    let a = self.stack.pop();
                    let result = match (a, b) {
                        (Value::Number(a), Value::Number(b)) => Value::Number(a + b),
                        (
                            Value::Object(ObjectRef::String(a)),
                            Value::Object(ObjectRef::String(b)),
                        ) => self.concatenate(&a, &b),
                        _ => {
                            return self
                                .runtime_error("Operands must be two numbers or two strings.");
                        }
                    };
                    self.stack.push(result);
                }
                OpCode::OpSubtract => self.binary_op(|a, b| Value::Number(a - b))?,
                OpCode::OpMultiply => self.binary_op(|a, b| Value::Number(a * b))?,
                OpCode::OpDivide => self.binary_op(|a, b| Value::Number(a / b))?,
                OpCode::OpFalse => self.stack.push(Value::Bool(false)),
                OpCode::OpTrue => self.stack.push(Value::Bool(true)),
                OpCode::OpNil => self.stack.push(Value::Nil),
                OpCode::OpNot => {
                    let value = self.stack.pop();
                    self.stack.push(Value::Bool(VM::is_falsey(value)));
                }
                OpCode::OpEqual => {
                    let b = self.stack.pop();
                    let a = self.stack.pop();
                    self.stack.push(Value::Bool(a == b));
                }
                OpCode::OpGreater => self.binary_op(|a, b| Value::Bool(a > b))?,
                OpCode::OpLess => self.binary_op(|a, b| Value::Bool(a < b))?,
                OpCode::OpReturn => return Ok(()),
            }
        }
    }

    fn read_byte(&mut self) -> Result<u8, VMError> {
        let chunk = &self.chunk;

        let byte = chunk.code[self.ip];
        self.ip += 1;

        Ok(byte)
    }

    fn binary_op<F>(&mut self, op: F) -> VMResult
    where
        F: FnOnce(f64, f64) -> Value<'src>,
    {
        let b = self.stack.pop();
        let a = self.stack.pop();
        match (a, b) {
            (Value::Number(a), Value::Number(b)) => {
                self.stack.push(op(a, b));
                Ok(())
            }
            _ => self.runtime_error("Operands must be numbers."),
        }
    }

    fn concatenate(&mut self, a: &*const ObjString, b: &*const ObjString) -> Value<'src> {
        let conc = unsafe { format!("{}{}", &(**a).chars, &(**b).chars) };
        Value::Object(self.heap.alloc_owned_string(conc))
    }

    fn is_falsey(v: Value) -> bool {
        match v {
            Value::Bool(x) => !x,
            Value::Nil => true,
            _ => false,
        }
    }

    fn read_constant(&self, idx: u8) -> Value<'src> {
        self.chunk.constants[idx as usize]
    }

    fn runtime_error(&mut self, message: &str) -> VMResult {
        eprintln!("{}", message);
        let ip = self.ip - 1;
        let line = self.chunk.get_line(ip);
        eprintln!("[line {line}] in script");
        self.reset_stack();
        Err(VMError::RuntimeError(format!("{}", message)))
    }

    fn reset_stack(&mut self) {
        self.stack.top = 0;
    }
}
