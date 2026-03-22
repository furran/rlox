use core::fmt;
use std::{
    cell::{Cell, RefCell},
    clone,
    collections::HashMap,
    fmt::Display,
    ops::Deref,
    sync::OnceLock,
};

use rlox_gc::{Gc, Trace};

use crate::{
    common::Value,
    vm::{Chunk, vm::Stack},
};

#[derive(Debug, Clone, Trace)]
pub struct ObjString {
    pub str: String,
}

impl Deref for ObjString {
    type Target = str;
    fn deref(&self) -> &str {
        &self.str
    }
}

impl fmt::Display for ObjString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.str.fmt(f)
    }
}

impl PartialEq for ObjString {
    fn eq(&self, other: &Self) -> bool {
        self.str == other.str
    }
}

impl Eq for ObjString {}

#[derive(Debug, Trace)]
pub struct ObjFunction {
    pub chunk: Chunk,
    pub name: Option<Gc<ObjString>>,
    pub arity: u8,
    pub upvalue_count: u8,
    pub max_locals: u8,
}

impl ObjFunction {
    pub fn new(name: Option<Gc<ObjString>>) -> ObjFunction {
        Self {
            chunk: Chunk::new(),
            name,
            arity: 0,
            upvalue_count: 0,
            max_locals: 0,
        }
    }
}

impl fmt::Display for ObjFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.name {
            Some(name) => write!(f, "<fn {}>", name),
            None => write!(f, "<script>"),
        }
    }
}

#[derive(Debug, Trace)]
pub struct ObjClosure {
    pub function: Gc<ObjFunction>,
    pub upvalues: Vec<Gc<Upvalue>>,
}

impl ObjClosure {
    pub fn new(function: Gc<ObjFunction>) -> Self {
        Self {
            function,
            upvalues: Vec::new(),
        }
    }
}

impl fmt::Display for ObjClosure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.function.name {
            Some(name) => write!(f, "<fn {}>", name),
            None => write!(f, "<script>"),
        }
    }
}

pub type NativeFn = fn(&[Value]) -> Value;

#[derive(Debug, Copy, Clone)]
pub struct ObjNative {
    pub function: NativeFn,
    pub arity: u8,
    pub name: &'static str,
}

impl Trace for ObjNative {
    fn trace(&self) {}
}

impl Display for ObjNative {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<native {}>", self.name)
    }
}

static START_TIME: OnceLock<std::time::Instant> = OnceLock::new();

pub fn native_clock(_args: &[Value]) -> Value {
    let start = START_TIME.get_or_init(std::time::Instant::now);
    Value::Number(start.elapsed().as_secs_f64())
}

#[derive(Debug, Trace)]
pub struct ObjClass {
    pub name: Gc<ObjString>,
    pub methods: RefCell<HashMap<Gc<ObjString>, Gc<ObjClosure>>>,
    pub initializer: Cell<Option<Gc<ObjClosure>>>,
}

impl ObjClass {
    pub fn new(name: Gc<ObjString>) -> Self {
        Self {
            name,
            methods: RefCell::new(HashMap::new()),
            initializer: Cell::new(None),
        }
    }
}

impl Display for ObjClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[derive(Debug, Trace)]
pub struct ObjInstance {
    pub class: Gc<ObjClass>,
    pub fields: RefCell<HashMap<Gc<ObjString>, Value>>,
}

impl ObjInstance {
    pub fn new(class: Gc<ObjClass>) -> Self {
        Self {
            class,
            fields: RefCell::new(HashMap::new()),
        }
    }
}

impl Display for ObjInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<{} instance>", self.class.name)
    }
}

#[derive(Debug, Trace)]
pub struct ObjBoundMethod {
    pub receiver: Value,
    pub method: Gc<ObjClosure>,
}

impl ObjBoundMethod {
    pub fn new(receiver: Value, method: Gc<ObjClosure>) -> Self {
        Self { receiver, method }
    }
}

#[derive(Debug, Trace)]
pub struct Upvalue {
    pub state: Cell<UpvalueState>,
}

#[derive(Debug, Clone, Copy)]
pub enum UpvalueState {
    Open(usize),
    Closed(Value),
}

impl Trace for UpvalueState {
    fn trace(&self) {
        match self {
            UpvalueState::Open(_) => {} // not a gc reference -> skip
            UpvalueState::Closed(value) => value.trace(),
        }
    }
}

impl Upvalue {
    pub fn open_upvalue(slot: usize) -> Self {
        Self {
            state: Cell::new(UpvalueState::Open(slot)),
        }
    }

    pub fn get<const N: usize>(&self, stack: &Stack<Value, N>) -> Value {
        match self.state.get() {
            UpvalueState::Open(slot) => stack[slot],
            UpvalueState::Closed(v) => v,
        }
    }

    pub fn set<const N: usize>(&self, value: Value, stack: &mut Stack<Value, N>) {
        match self.state.get() {
            UpvalueState::Open(slot) => stack[slot] = value,
            UpvalueState::Closed(_) => self.state.set(UpvalueState::Closed(value)),
        }
    }

    pub fn close(&self, stack: &Stack<Value, 256>) {
        if let UpvalueState::Open(slot) = self.state.get() {
            self.state.set(UpvalueState::Closed(stack[slot]));
        }
    }
}
