use core::fmt;
use std::{borrow::Borrow, cell::Cell, hash::Hash, ops::Deref};

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

impl Hash for ObjString {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.str.hash(state);
    }
}

impl Borrow<str> for Box<ObjString> {
    fn borrow(&self) -> &str {
        &self.str
    }
}
#[derive(Debug, Trace)]
pub struct ObjFunction {
    pub chunk: Chunk,
    pub name: Option<Gc<ObjString>>,
    pub arity: u8,
    pub upvalue_count: u8,
}

impl Default for ObjFunction {
    fn default() -> Self {
        Self {
            chunk: Chunk::new(),
            name: Default::default(),
            arity: Default::default(),
            upvalue_count: 0,
        }
    }
}

impl ObjFunction {
    pub fn new(name: Option<Gc<ObjString>>) -> ObjFunction {
        Self {
            chunk: Chunk::new(),
            name,
            arity: 0,
            upvalue_count: 0,
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
