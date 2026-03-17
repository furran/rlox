use std::{
    collections::HashMap,
    fmt::Debug,
    io::Write,
    mem::{self},
    ops::{Deref, DerefMut},
};

use rlox_gc::{Gc, Trace};

use crate::{
    common::{OpCode, Value},
    compiler::{Compiler, compiler::CompileError},
    object::{ObjClosure, ObjFunction, ObjString, Upvalue},
    vm::heap::LoxHeap,
};

#[derive(Debug)]
pub enum VMError {
    RuntimeError(String),
    CompileError(Vec<CompileError>),
}

pub type VMResult = Result<(), VMError>;

#[derive(Debug, Trace)]
pub struct Stack<T: Trace, const N: usize> {
    data: [T; N],
    top: usize,
}

impl<T: Trace, const N: usize> Deref for Stack<T, N> {
    type Target = [T; N];

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T: Trace, const N: usize> DerefMut for Stack<T, N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

#[allow(dead_code)]
impl<T: Trace, const N: usize> Stack<T, N>
where
    T: Default,
    T: Debug,
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

    fn truncate(&mut self, len: usize) {
        self.top = len;
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
        println!("Stack [ {:?} ]", &self.data[0..self.top + 1])
    }
}

#[derive(Debug, Default)]
pub struct GlobalIndices(HashMap<Gc<ObjString>, u8>);

impl GlobalIndices {
    pub fn new() -> Self {
        Self(HashMap::new())
    }
}

impl Deref for GlobalIndices {
    type Target = HashMap<Gc<ObjString>, u8>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for GlobalIndices {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Debug, Trace)]
struct CallFrame {
    closure: Gc<ObjClosure>,
    ip: usize,
    slot_offset: usize,
}

impl CallFrame {
    pub fn get_closure(&self) -> &ObjClosure {
        &self.closure
    }

    pub fn get_function(&self) -> &ObjFunction {
        &self.closure.function
    }
}

#[derive(Debug)]
pub struct VM<W: Write> {
    stack: Stack<Value, 256>,
    heap: LoxHeap,
    globals: Vec<Option<Value>>,
    frames: Vec<CallFrame>,
    open_upvalues: HashMap<usize, Gc<Upvalue>>,
    output: W,
}

impl<W: Write> VM<W> {
    pub fn new(output: W) -> Self {
        Self {
            stack: Stack::new(),
            heap: LoxHeap::new(),
            globals: Vec::new(),
            frames: Vec::with_capacity(64),
            open_upvalues: HashMap::new(),
            output,
        }
    }

    pub fn run_file(_source: &String) {}

    pub fn interpret(&mut self, source: &str) -> VMResult {
        let function = Compiler::compile(source, &mut self.heap)?;
        let func_ref = self.allocate(function);

        let closure = ObjClosure::new(func_ref);
        let closure_ref = self.allocate(closure);

        self.stack.push(Value::Closure(closure_ref));

        self.frames.push(CallFrame {
            closure: closure_ref,
            ip: 0,
            slot_offset: 0,
        });

        self.run()
    }

    fn run(&mut self) -> VMResult {
        loop {
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
                        (Value::String(a), Value::String(b)) => self.concatenate(a, b),
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
                OpCode::Call => {
                    let arg_count = self.read_byte() as usize;
                    self.call_value(self.stack.peek(arg_count), arg_count)?;
                }
                OpCode::Closure => {
                    let idx = self.read_byte();
                    let value = self.read_constant(idx);
                    if let Value::Function(func) = value {
                        let mut closure = ObjClosure::new(func);
                        let upvalue_count = func.upvalue_count;
                        for _ in 0..upvalue_count {
                            let is_local = self.read_byte() != 0;
                            let index = self.read_byte() as usize;
                            let uv = if is_local {
                                self.capture_upvalue(self.current_frame().slot_offset + index)
                            } else {
                                self.current_frame().get_closure().upvalues[index].clone()
                            };

                            closure.upvalues.push(uv);
                        }
                        let closure_ref = self.allocate(closure);
                        self.stack.push(Value::Closure(closure_ref));
                    }
                }
                OpCode::CloseUpvalue => {
                    self.close_upvalues(self.stack.top);
                    self.stack.pop();
                }
                OpCode::Return => {
                    let result = self.stack.pop();
                    let slot_offset = self.current_frame().slot_offset;
                    self.close_upvalues(slot_offset);
                    self.frames.pop();

                    if self.frames.is_empty() {
                        self.stack.pop();
                        return Ok(());
                    }
                    self.stack.truncate(slot_offset);
                    self.stack.push(result);
                }
                OpCode::SetLocal => {
                    let slot = self.read_byte() as usize;
                    let slot_offset = self.current_frame().slot_offset;
                    self.stack[slot + slot_offset] = self.stack.peek(0);
                }
                OpCode::GetLocal => {
                    let slot = self.read_byte() as usize;
                    let slot_offset = self.current_frame().slot_offset;
                    self.stack.push(self.stack[slot + slot_offset]);
                }
                OpCode::SetUpvalue => {
                    let slot = self.read_byte() as usize;
                    let upvalue = self.current_frame().get_closure().upvalues[slot];
                    let value = self.stack.peek(0);
                    upvalue.set(value, &mut self.stack);
                }
                OpCode::GetUpvalue => {
                    let slot = self.read_byte() as usize;
                    let value = self.current_frame().get_closure().upvalues[slot].get(&self.stack);
                    self.stack.push(value);
                }
                OpCode::DefineGlobal => {
                    let slot = self.read_byte() as usize;
                    let value = self.stack.pop();
                    if slot >= self.globals.len() {
                        self.globals.resize(slot + 1, None);
                    }
                    self.globals[slot] = Some(value);
                }
                OpCode::SetGlobal => {
                    let slot = self.read_byte() as usize;
                    if slot >= self.globals.len() {
                        return self.runtime_error("Undefined variable.");
                    }
                    self.globals[slot] = Some(self.stack.peek(0));
                }
                OpCode::GetGlobal => {
                    let slot = self.read_byte() as usize;
                    match self.globals.get(slot) {
                        Some(Some(value)) => self.stack.push(*value),
                        _ => return self.runtime_error("Undefined variable."),
                    }
                }
                OpCode::JumpIfFalse => {
                    let offset = self.read_short() as usize;
                    if self.stack.peek(0).is_falsey() {
                        self.current_frame_mut().ip += offset;
                    }
                }
                OpCode::Jump => {
                    let offset = self.read_short() as usize;
                    self.current_frame_mut().ip += offset;
                }
                OpCode::Loop => {
                    let offset = self.read_short() as usize;
                    self.current_frame_mut().ip -= offset;
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
        let frame = self.current_frame_mut();
        let byte = frame.get_function().chunk.code[frame.ip];
        frame.ip += 1;
        byte
    }

    fn read_short(&mut self) -> u16 {
        let hi = self.read_byte() as u16;
        let lo = self.read_byte() as u16;
        (hi << 8) | lo
    }

    fn read_constant(&self, idx: u8) -> Value {
        let frame = self.current_frame();
        frame.get_function().chunk.constants[idx as usize]
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

    fn call(&mut self, closure: Gc<ObjClosure>, arg_count: usize) -> VMResult {
        let arity = closure.function.arity as usize;
        if arg_count != arity {
            return self.runtime_error(format!(
                "Expected {} argument(s) but got {}.",
                arity, arg_count
            ));
        }

        self.stack.print();
        self.frames.push(CallFrame {
            closure,
            ip: 0,
            slot_offset: self.stack.len() - arg_count - 1,
        });
        Ok(())
    }

    fn call_value(&mut self, callee: Value, arg_count: usize) -> VMResult {
        match callee {
            Value::Closure(c) => self.call(c, arg_count),
            _ => self.runtime_error("Can only call functions and classes."),
        }
    }

    fn capture_upvalue(&mut self, slot: usize) -> Gc<Upvalue> {
        if let Some(&existing) = self.open_upvalues.get(&slot) {
            return existing;
        }
        let upvalue = self.allocate(Upvalue::open_upvalue(slot));
        self.open_upvalues.insert(slot, upvalue);
        upvalue
    }

    fn close_upvalues(&mut self, last: usize) {
        let slots_to_close = self
            .open_upvalues
            .keys()
            .filter(|&&slot| slot >= last)
            .copied()
            .collect::<Vec<usize>>();
        for slot in slots_to_close {
            if let Some(upvalue) = self.open_upvalues.remove(&slot) {
                upvalue.close(&mut self.stack);
            }
        }
    }

    fn concatenate(&mut self, a: Gc<ObjString>, b: Gc<ObjString>) -> Value {
        let conc = format!("{}{}", (*a).str, (*b).str);
        let str = ObjString { str: conc };
        let str_ref = self.intern(&str);
        Value::String(str_ref)
    }

    fn runtime_error(&mut self, message: impl Into<String>) -> VMResult {
        let message = message.into();
        let mut error = format!("{}\n", message);

        for frame in self.frames.iter().rev() {
            let function = frame.get_function();
            let ip = frame.ip - 1;
            let line = function.chunk.get_line(ip);
            match &function.name {
                Some(name) => error.push_str(&format!("[line {}] in {}\n", line, name)),
                None => error.push_str(&format!("[line {}] in script.\n", line)),
            }
        }

        self.reset_stack();
        Err(VMError::RuntimeError(error))
    }

    fn reset_stack(&mut self) {
        self.stack.top = 0;
    }

    fn current_frame(&self) -> &CallFrame {
        self.frames.last().unwrap()
    }

    fn current_frame_mut(&mut self) -> &mut CallFrame {
        self.frames.last_mut().unwrap()
    }

    fn allocate<T: Trace>(&mut self, value: T) -> Gc<T> {
        if self.heap.should_collect() {
            let roots = Roots(
                &self.stack,
                &self.frames,
                &self.open_upvalues,
                &self.globals,
            );
            self.heap.collect(&roots);
        }
        self.heap.allocate(value)
    }

    fn intern(&mut self, s: &str) -> Gc<ObjString> {
        if self.heap.should_collect() {
            let roots = Roots(
                &self.stack,
                &self.frames,
                &self.open_upvalues,
                &self.globals,
            );
            self.heap.collect(&roots);
        }

        self.heap.intern(s)
    }
}

#[derive(Trace)]
struct Roots<'a>(
    &'a Stack<Value, 256>,
    &'a Vec<CallFrame>,
    &'a HashMap<usize, Gc<Upvalue>>,
    &'a Vec<Option<Value>>,
);

impl<W: Write> Trace for VM<W> {
    fn trace(&self) {
        self.stack.trace();
        self.frames.trace();
        self.open_upvalues.trace();
        self.globals.trace();
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
