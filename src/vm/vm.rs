use std::{
    collections::HashMap,
    fmt::{Debug, Display},
    io::Write,
    mem::{self},
    ops::{Deref, DerefMut},
};

use rlox_gc::{Gc, Trace};

use crate::{
    common::{OpCode, Value},
    compiler::{Compiler, compiler::CompileError},
    object::{
        NativeFn, ObjBoundMethod, ObjClass, ObjClosure, ObjFunction, ObjInstance, ObjNative,
        ObjString, Upvalue, native_clock,
    },
    vm::heap::LoxHeap,
};

#[derive(Debug)]
pub enum VMError {
    RuntimeError(String),
    CompileError(Vec<CompileError>),
}

impl Display for VMError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VMError::RuntimeError(msg) => write!(f, "{msg}"),
            VMError::CompileError(errors) => {
                for e in errors {
                    writeln!(f, "{e}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for VMError {}

pub type VMResult = Result<(), VMError>;

#[derive(Debug)]
pub struct Stack<T: Trace, const N: usize> {
    data: [T; N],
    top: usize,
}

impl<T: Trace, const N: usize> Trace for Stack<T, N> {
    fn trace(&self) {
        for item in &self.data[..self.top] {
            item.trace();
        }
    }
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

    fn clear(&mut self) {
        for v in &mut self.data {
            *v = Default::default();
        }
    }
}

#[derive(Debug, Default)]
pub struct GlobalIndices(pub HashMap<Gc<ObjString>, u8>);

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
    global_indices: GlobalIndices,
    frames: Vec<CallFrame>,
    open_upvalues: HashMap<usize, Gc<Upvalue>>,
    init_string: Gc<ObjString>,
    output: W,
}

impl<W: Write> VM<W> {
    pub fn new(output: W) -> Self {
        let mut heap = LoxHeap::new();
        let init_string = heap.intern("init");
        let mut vm = Self {
            stack: Stack::new(),
            heap,
            globals: Vec::new(),
            global_indices: GlobalIndices::new(),
            frames: Vec::with_capacity(64),
            open_upvalues: HashMap::new(),
            init_string,
            output,
        };

        vm.define_native("clock", 0, native_clock);
        vm
    }

    pub fn run_file(path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let source = std::fs::read_to_string(path)?;
        let output = std::io::stdout();
        let mut vm = VM::new(output);
        vm.interpret(&source)?;
        Ok(())
    }

    pub fn interpret(&mut self, source: &str) -> VMResult {
        let function = Compiler::compile(source, &mut self.heap, &mut self.global_indices)?;
        // avoid collection
        let func_ref = self.heap.alloc_raw(function);
        let closure = ObjClosure::new(func_ref);
        let closure_ref = self.heap.alloc_raw(closure);
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
                    let constant = self.read_constant();
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
                OpCode::Invoke => {
                    let name = self.read_constant().unwrap_string();
                    let arg_count = self.read_byte() as usize;

                    self.invoke(name, arg_count)?;
                }
                OpCode::SuperInvoke => {
                    let method = self.read_constant().unwrap_string();
                    let arg_count = self.read_byte() as usize;
                    let Value::Class(superclass) = self.stack.pop() else {
                        return self.runtime_error("Expected class.");
                    };

                    self.invoke_from_class(superclass, method, arg_count)?;
                }
                OpCode::Closure => {
                    let func = self.read_constant().unwrap_function();
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
                OpCode::CloseUpvalue => {
                    self.close_upvalues(self.stack.top - 1);
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
                OpCode::Class => {
                    let name = self.read_constant().unwrap_string();
                    let class = self.allocate(ObjClass::new(name));
                    self.stack.push(Value::Class(class));
                }
                OpCode::Inherit => {
                    let subclass = self.stack.pop().unwrap_class();
                    let Value::Class(superclass) = self.stack.peek(0) else {
                        return self.runtime_error("Superclass must be a class.");
                    };
                    subclass
                        .methods
                        .borrow_mut()
                        .extend(superclass.methods.borrow().iter().map(|(&k, &v)| (k, v)));
                }
                OpCode::Method => {
                    let name = self.read_constant().unwrap_string();
                    let method = self.stack.pop().unwrap_closure();
                    let class = self.stack.peek(0).unwrap_class();
                    if name == self.init_string {
                        class.initializer.set(Some(method));
                    }
                    class.methods.borrow_mut().insert(name, method);
                }
                OpCode::GetSuper => {
                    let name = self.read_constant().unwrap_string();
                    let superclass = self.stack.pop().unwrap_class();

                    let val = self.bind_method(superclass, name);
                    self.stack.push(val);
                }
                OpCode::SetProperty => {
                    let val = self.stack.peek(1);
                    if let Value::Instance(inst) = val {
                        let val = self.read_constant();
                        if let Value::String(name) = val {
                            let arg = self.stack.pop();
                            inst.fields.borrow_mut().insert(name, arg);
                            self.stack.pop();
                            self.stack.push(arg);
                        }
                    } else {
                        return self.runtime_error("Only instances have fields.");
                    }
                }
                OpCode::GetProperty => {
                    let Value::Instance(instance) = self.stack.peek(0) else {
                        return self.runtime_error("Only instances have properties.");
                    };
                    let name = self.read_constant().unwrap_string();

                    if let Some(field) = instance.fields.borrow().get(&name) {
                        self.stack.pop();
                        self.stack.push(*field);
                    } else {
                        let value = self.bind_method(instance.class, name);
                        self.stack.push(value);
                    }
                }
                OpCode::DeleteProperty => {
                    let name = self.read_constant().unwrap_string();
                    let Value::Instance(instance) = self.stack.pop() else {
                        return self.runtime_error("Can only delete fields on instances.");
                    };
                    let removed = instance
                        .fields
                        .borrow_mut()
                        .remove(&name)
                        .unwrap_or(Value::Nil);
                    self.stack.push(removed);
                }
                OpCode::SetIndex => {
                    let value = self.stack.pop();
                    let Value::String(name) = self.stack.pop() else {
                        return self.runtime_error("Field name must be a string.");
                    };
                    let Value::Instance(instance) = self.stack.pop() else {
                        return self.runtime_error("Only instances have fields.");
                    };

                    instance.fields.borrow_mut().insert(name, value);
                    self.stack.push(value);
                }
                OpCode::GetIndex => {
                    let name = self.stack.pop();
                    let instance = self.stack.pop();
                    if let (Value::String(name), Value::Instance(inst)) = (name, instance) {
                        let value = inst
                            .fields
                            .borrow()
                            .get(&name)
                            .copied()
                            .unwrap_or(Value::Nil);
                        self.stack.push(value);
                    } else {
                        return self.runtime_error("Field name must be a string");
                    }
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

    fn read_constant(&mut self) -> Value {
        let idx = self.read_byte();
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
            Value::Class(class) => {
                let obj_instance = ObjInstance::new(class);
                let instance = Value::Instance(self.allocate(obj_instance));
                let top = self.stack.top;
                self.stack[top - arg_count - 1] = instance;
                if let Some(initializer) = class.initializer.get() {
                    return self.call(initializer, arg_count);
                } else if arg_count != 0 {
                    return self
                        .runtime_error(format!("Expected 0 arguments but got {}", arg_count));
                }
                Ok(())
            }
            Value::BoundMethod(bound) => {
                let top = self.stack.top;
                self.stack[top - arg_count - 1] = bound.receiver;
                self.call(bound.method, arg_count)
            }
            Value::Native(native) => {
                let arity = native.arity as usize;
                if arg_count != arity {
                    return self.runtime_error(format!(
                        "Expected {} arguments but got {}",
                        native.arity, arg_count
                    ));
                }
                let arg_start = self.stack.top - arg_count;
                let result = (native.function)(&self.stack[arg_start..self.stack.top]);
                self.stack.top -= arg_count + 1;
                self.stack.push(result);
                Ok(())
            }
            _ => self.runtime_error("Can only call functions and classes."),
        }
    }

    fn invoke(&mut self, name: Gc<ObjString>, arg_count: usize) -> VMResult {
        let receiver = self.stack.peek(arg_count);
        let Value::Instance(instance) = receiver else {
            return self.runtime_error("Only instances have methods.");
        };

        if let Some(&value) = instance.fields.borrow().get(&name) {
            let top = self.stack.top;
            self.stack[top - arg_count - 1] = value;
            return self.call_value(value, arg_count);
        }

        self.invoke_from_class(instance.class, name, arg_count)
    }

    fn invoke_from_class(
        &mut self,
        class: Gc<ObjClass>,
        name: Gc<ObjString>,
        arg_count: usize,
    ) -> VMResult {
        if let Some(&method) = class.methods.borrow().get(&name) {
            self.call(method, arg_count)
        } else {
            self.runtime_error(format!("Undefined property '{}'", name))
        }
    }

    fn bind_method(&mut self, class: Gc<ObjClass>, name: Gc<ObjString>) -> Value {
        if let Some(&method) = class.methods.borrow().get(&name) {
            let bound = ObjBoundMethod::new(self.stack.peek(0), method);
            let bound_ref = self.allocate(bound);
            // we pop after allocation to avoid collection of stack value
            self.stack.pop();
            Value::BoundMethod(bound_ref)
        } else {
            self.stack.pop();
            Value::Nil
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

        self.reset();
        Err(VMError::RuntimeError(error))
    }

    fn reset(&mut self) {
        self.stack.top = 0;
        self.frames.clear();
        self.open_upvalues.clear();
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
                &self.global_indices,
                &self.init_string,
            );
            self.heap.collect(&roots);
        }
        self.heap.alloc_raw(value)
    }

    fn intern(&mut self, s: &str) -> Gc<ObjString> {
        if self.heap.should_collect() {
            let roots = Roots(
                &self.stack,
                &self.frames,
                &self.open_upvalues,
                &self.globals,
                &self.global_indices,
                &self.init_string,
            );
            self.heap.collect(&roots);
        }

        self.heap.intern(s)
    }

    fn define_native(&mut self, name: &'static str, arity: u8, function: NativeFn) {
        let native = self.heap.alloc_raw(ObjNative {
            function,
            arity,
            name,
        });
        let name_ref = self.heap.intern(name);
        let idx = self.global_indices.len() as u8;
        let idx = *self.global_indices.entry(name_ref).or_insert(idx) as usize;
        if idx >= self.globals.len() {
            self.globals.resize(idx + 1, None);
        }
        self.globals[idx] = Some(Value::Native(native));
    }
}

impl<W: Write> Drop for VM<W> {
    fn drop(&mut self) {
        struct NoRoots;
        impl Trace for NoRoots {
            fn trace(&self) {}
        }
        self.heap.collect(&[NoRoots]);

        // sanity check
        debug_assert_eq!(self.heap.get_bytes_alloc(), 0, "GC leak detected!");
        debug_assert!(self.stack.is_empty());
    }
}

#[derive(Trace)]
struct Roots<'a>(
    &'a Stack<Value, 256>,
    &'a Vec<CallFrame>,
    &'a HashMap<usize, Gc<Upvalue>>,
    &'a Vec<Option<Value>>,
    &'a GlobalIndices,
    &'a Gc<ObjString>,
);

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
