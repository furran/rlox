use std::{
    collections::HashMap,
    io::Write,
    mem::{self},
    ops::{Deref, DerefMut},
};

use crate::{
    common::{ObjString, ObjStringPtr, OpCode, Value},
    compiler::Compiler,
    vm::{Interner, chunk::Chunk},
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

#[derive(Debug, Default)]
pub struct GlobalIndices(pub HashMap<ObjStringPtr, u8>);

impl Deref for GlobalIndices {
    type Target = HashMap<ObjStringPtr, u8>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for GlobalIndices {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Debug)]
pub struct VM {
    chunk: Chunk,
    ip: usize,
    stack: Stack<Value, 256>,
    interner: Interner,
    globals: Vec<Value>,
    global_indices: GlobalIndices,
}

impl VM {
    fn new() -> Self {
        Self {
            chunk: Chunk::new(),
            ip: 0,
            stack: Stack::new(),
            interner: Interner::new(),
            globals: Vec::new(),
            global_indices: GlobalIndices::default(),
        }
    }

    pub fn repl() -> std::io::Result<()> {
        let stdin = std::io::stdin();
        let mut stdout = std::io::stdout();
        let mut vm = VM::new();

        loop {
            print!("> ");
            stdout.flush()?;

            let mut line = String::new();

            match stdin.read_line(&mut line) {
                Ok(0) => {
                    println!();
                    return Ok(());
                }
                Ok(_) => {
                    let line = line.trim_end();
                    if let Err(e) = vm.interpret(line) {
                        eprintln!("Error: {:?}", e);
                    }
                }
                Err(e) => {
                    eprintln!("Error reading input: {}", e);
                    return Err(e);
                }
            }
            println!("{:?}", vm.interner);
            println!("{:?}", vm.globals);
        }
    }

    pub fn run_file(_source: &String) {}

    pub fn interpret(&mut self, source: &str) -> VMResult {
        self.chunk = Compiler::compile(source, &mut self.interner, &mut self.global_indices);
        self.ip = 0;

        self.run()
    }

    fn run(&mut self) -> VMResult {
        loop {
            // let _ = self.chunk.disassemble_instruction(self.ip);
            self.stack.print();
            let opcode = self.read_opcode();
            match opcode {
                OpCode::OpConstant => {
                    let idx = self.read_byte();
                    let constant = self.read_constant(idx);
                    self.stack.push(constant);
                    println!("{:?}", constant);
                }
                OpCode::OpNegate => {
                    let value = self.stack.pop();
                    if let Value::Number(val) = value {
                        self.stack.push(Value::Number(-val));
                    } else {
                        return self.runtime_error("Operand must be a number.");
                    }
                }
                OpCode::OpAdd => {
                    let b = self.stack.pop();
                    let a = self.stack.pop();
                    let result = match (a, b) {
                        (Value::Number(a), Value::Number(b)) => Value::Number(a + b),
                        (Value::String(a), Value::String(b)) => self.concatenate(a, b),
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
                OpCode::OpPrint => println!("{}", self.stack.pop()),
                OpCode::OpPop => {
                    self.stack.pop();
                }
                OpCode::OpReturn => {
                    return Ok(());
                }
                OpCode::OpDefineGlobal => {
                    let slot = self.read_byte() as usize;
                    let value = self.stack.pop();
                    if slot >= self.globals.len() {
                        self.globals.resize(slot + 1, Value::Nil);
                    }
                    self.globals[slot] = value;
                }
                OpCode::OpGetGlobal => {
                    let slot = self.read_byte() as usize;
                    if let Some(value) = self.globals.get(slot) {
                        self.stack.push(*value)
                    } else {
                        return self.runtime_error("Undefined variable.");
                    }
                }
                OpCode::OpSetGlobal => {
                    let slot = self.read_byte() as usize;
                    if slot >= self.globals.len() {
                        return self.runtime_error("Undefined variable.");
                    }
                    self.globals[slot] = self.stack.peek(0);
                }
            }
        }
    }

    fn read_byte(&mut self) -> u8 {
        let byte = self.chunk.code[self.ip];
        self.ip += 1;

        byte
    }

    fn binary_op<F>(&mut self, op: F) -> VMResult
    where
        F: FnOnce(f64, f64) -> Value,
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

    fn concatenate(&mut self, a: *const ObjString, b: *const ObjString) -> Value {
        let conc = unsafe { format!("{}{}", (*a).str, (*b).str) };
        let str = self.intern(&conc);
        Value::String(str)
    }

    fn is_falsey(v: Value) -> bool {
        match v {
            Value::Bool(x) => !x,
            Value::Nil => true,
            _ => false,
        }
    }

    fn read_constant(&self, idx: u8) -> Value {
        self.chunk.constants[idx as usize]
    }

    fn read_opcode(&mut self) -> OpCode {
        unsafe { std::mem::transmute(self.read_byte()) }
    }

    fn runtime_error(&mut self, message: impl Into<String>) -> VMResult {
        let message = message.into();
        eprintln!("{}", message);
        let ip = self.ip - 1;
        let line = self.chunk.get_line(ip);
        eprintln!("[line {line}] in script");
        self.reset_stack();
        Err(VMError::RuntimeError(message))
    }

    fn reset_stack(&mut self) {
        self.stack.top = 0;
    }

    pub fn intern(&mut self, e: &str) -> *const ObjString {
        self.interner.intern(e)
    }
}
