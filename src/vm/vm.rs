use std::{
    collections::HashMap,
    io::Write,
    mem::{self},
    ops::{Deref, DerefMut},
};

use crate::{
    common::{OpCode, Value},
    compiler::{Compiler, compiler::CompileError},
    object::{ObjString, ObjStringPtr, Object},
    vm::{chunk::Chunk, heap::Heap},
};

#[derive(Debug)]
pub enum VMError {
    RuntimeError(String),
    CompileError(Vec<CompileError>),
}

pub type VMResult = Result<(), VMError>;

#[derive(Debug)]
struct Stack<T, const N: usize> {
    data: [T; N],
    top: usize,
}

impl<T, const N: usize> Deref for Stack<T, N> {
    type Target = [T; N];

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T, const N: usize> DerefMut for Stack<T, N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
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

    fn print(&self) {
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
pub struct GlobalIndices(HashMap<ObjStringPtr, u8>);

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
pub struct VM<W: Write> {
    chunk: Chunk,
    ip: usize,
    stack: Stack<Value, 256>,
    heap: Heap,
    globals: Vec<Value>,
    global_indices: GlobalIndices,
    output: W,
}

impl<W: Write> VM<W> {
    pub fn new(output: W) -> Self {
        Self {
            chunk: Chunk::new(),
            ip: 0,
            stack: Stack::new(),
            heap: Heap::new(),
            globals: Vec::new(),
            global_indices: GlobalIndices::default(),
            output,
        }
    }

    pub fn run_file(_source: &String) {}

    pub fn interpret(&mut self, source: &str) -> VMResult {
        self.chunk = Compiler::compile(source, &mut self.heap, &mut self.global_indices)?;
        self.ip = 0;

        self.run()
    }

    fn run(&mut self) -> VMResult {
        loop {
            self.stack.print();
            let opcode = self.read_opcode();
            match opcode {
                OpCode::Constant => {
                    let idx = self.read_byte();
                    let constant = self.read_constant(idx);
                    self.stack.push(constant);
                }
                OpCode::Negate => {
                    let value = self.stack.pop();
                    if let Value::Number(val) = value {
                        self.stack.push(Value::Number(-val));
                    } else {
                        return self.runtime_error("Operand must be a number.");
                    }
                }
                OpCode::Add => {
                    let b = self.stack.pop();
                    let a = self.stack.pop();
                    let result = match (a, b) {
                        (Value::Number(a), Value::Number(b)) => Value::Number(a + b),
                        (Value::Obj(a), Value::Obj(b)) => match (&*a, &*b) {
                            (Object::String(a), Object::String(b)) => self.concatenate(a, b),
                            _ => {
                                return self
                                    .runtime_error("Operands must be two numbers or two strings.");
                            }
                        },
                        _ => {
                            return self
                                .runtime_error("Operands must be two numbers or two strings.");
                        }
                    };
                    self.stack.push(result);
                }
                OpCode::Subtract => self.binary_op(|a, b| Value::Number(a - b))?,
                OpCode::Multiply => self.binary_op(|a, b| Value::Number(a * b))?,
                OpCode::Divide => self.binary_op(|a, b| Value::Number(a / b))?,
                OpCode::False => self.stack.push(Value::Bool(false)),
                OpCode::True => self.stack.push(Value::Bool(true)),
                OpCode::Nil => self.stack.push(Value::Nil),
                OpCode::Not => {
                    let value = self.stack.pop();
                    self.stack.push(Value::Bool(value.is_falsey()));
                }
                OpCode::Equal => {
                    let b = self.stack.pop();
                    let a = self.stack.pop();
                    self.stack.push(Value::Bool(a == b));
                }
                OpCode::Greater => self.binary_op(|a, b| Value::Bool(a > b))?,
                OpCode::Less => self.binary_op(|a, b| Value::Bool(a < b))?,
                OpCode::Print => writeln!(self.output, "{}", self.stack.pop())
                    .map_err(|e| VMError::RuntimeError(e.to_string()))?,
                OpCode::Pop => {
                    self.stack.pop();
                }
                OpCode::Return => {
                    return Ok(());
                }
                OpCode::DefineGlobal => {
                    let slot = self.read_byte() as usize;
                    let value = self.stack.pop();
                    if slot >= self.globals.len() {
                        self.globals.resize(slot + 1, Value::Nil);
                    }
                    self.globals[slot] = value;
                }
                OpCode::SetGlobal => {
                    let slot = self.read_byte() as usize;
                    if slot >= self.globals.len() {
                        return self.runtime_error("Undefined variable.");
                    }
                    self.globals[slot] = self.stack.peek(0);
                }
                OpCode::GetGlobal => {
                    let slot = self.read_byte() as usize;
                    if let Some(value) = self.globals.get(slot) {
                        self.stack.push(*value)
                    } else {
                        return self.runtime_error("Undefined variable.");
                    }
                }
                OpCode::SetLocal => {
                    let slot = self.read_byte() as usize;
                    self.stack[slot] = self.stack.peek(0);
                }
                OpCode::GetLocal => {
                    let slot = self.read_byte() as usize;
                    self.stack.push(self.stack[slot]);
                }
                OpCode::JumpIfFalse => {
                    let offset = self.read_short() as usize;
                    if self.stack.peek(0).is_falsey() {
                        self.ip += offset;
                    }
                }
                OpCode::Jump => {
                    let offset = self.read_short() as usize;
                    self.ip += offset;
                }
                OpCode::Loop => {
                    let offset = self.read_short() as usize;
                    self.ip -= offset;
                }
                OpCode::SwitchEq => {
                    let case = self.stack.pop();
                    let switch = self.stack.peek(0);
                    self.stack.push(Value::Bool(switch == case));
                }
            }
        }
    }

    fn read_byte(&mut self) -> u8 {
        let byte = self.chunk.code[self.ip];
        self.ip += 1;

        byte
    }

    fn read_short(&mut self) -> u16 {
        let hi = self.read_byte() as u16;
        let lo = self.read_byte() as u16;
        (hi << 8) | lo
    }

    fn read_constant(&self, idx: u8) -> Value {
        self.chunk.constants[idx as usize]
    }

    fn read_opcode(&mut self) -> OpCode {
        unsafe { std::mem::transmute(self.read_byte()) }
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
        let str = self.heap.alloc_string(&conc);
        Value::Obj(str)
    }

    fn runtime_error(&mut self, message: impl Into<String>) -> VMResult {
        let message = message.into();
        let ip = self.ip - 1;
        let line = self.chunk.get_line(ip);
        self.reset_stack();
        Err(VMError::RuntimeError(format!("[line {line}] {message}")))
    }

    fn reset_stack(&mut self) {
        self.stack.top = 0;
    }
}

pub fn repl() -> std::io::Result<()> {
    let stdin = std::io::stdin();
    let mut vm = VM::new(std::io::stdout());

    loop {
        print!(">>> ");
        vm.output.flush().unwrap();

        let mut buffer = String::new();
        stdin.read_line(&mut buffer).unwrap();

        while !is_complete(&buffer) {
            print!("... ");
            vm.output.flush().unwrap();
            stdin.read_line(&mut buffer).unwrap();
        }

        if let Err(e) = vm.interpret(&buffer) {
            match e {
                VMError::CompileError(errors) => {
                    for error in errors {
                        eprintln!("[CompileError] {}", error);
                    }
                }
                VMError::RuntimeError(msg) => eprintln!("[RuntimeError] {}", msg),
            }
        }

        println!("{:?}", vm.heap);
        println!("Globals vector: {:?}", vm.globals);
    }
}

fn is_complete(source: &str) -> bool {
    let mut depth = 0;
    let mut in_string = false;

    for c in source.chars() {
        match c {
            '"' => in_string = !in_string,
            '{' if !in_string => depth += 1,
            '}' if !in_string => depth -= 1,
            _ => {}
        }
    }
    depth == 0
}
