use core::fmt;
use std::{
    borrow::Borrow,
    cell::Cell,
    fmt::Display,
    hash::{Hash, Hasher},
    ops::{Deref, DerefMut},
    ptr::NonNull,
    rc::Rc,
};

use crate::{common::Value, vm::Chunk};

#[derive(Debug, Clone)]
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

#[derive(Debug, PartialEq, Eq)]
pub struct ObjStringPtr(NonNull<Object>);

impl Borrow<str> for ObjStringPtr {
    fn borrow(&self) -> &str {
        unsafe {
            match self.0.as_ref() {
                Object::String(s) => &s.str,
                _ => unreachable!(),
            }
        }
    }
}

impl From<*mut Object> for ObjStringPtr {
    fn from(ptr: *mut Object) -> Self {
        ObjStringPtr(NonNull::new(ptr).unwrap())
    }
}

impl From<NonNull<Object>> for ObjStringPtr {
    fn from(ptr: NonNull<Object>) -> Self {
        ObjStringPtr(ptr)
    }
}

impl From<ObjRef> for ObjStringPtr {
    fn from(value: ObjRef) -> Self {
        ObjStringPtr(value.0)
    }
}

impl Hash for ObjStringPtr {
    fn hash<H: Hasher>(&self, state: &mut H) {
        unsafe {
            if let Object::String(s) = self.0.as_ref() {
                s.hash(state);
            }
        }
    }
}

impl ObjStringPtr {
    pub fn as_ptr(&self) -> *mut Object {
        self.0.as_ptr()
    }
}

#[derive(Debug)]
pub struct ObjFunction {
    pub chunk: Chunk,
    pub name: Option<NonNull<ObjString>>,
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
    pub fn new(name: Option<NonNull<ObjString>>) -> ObjFunction {
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
        match self.name {
            Some(name) => write!(f, "<fn {}>", unsafe { name.as_ref() }),
            None => write!(f, "<script>"),
        }
    }
}

#[derive(Debug)]
pub struct ObjClosure {
    pub function: NonNull<ObjFunction>,
    pub upvalues: Vec<Upvalue>,
}

impl ObjClosure {
    pub fn new(function: NonNull<ObjFunction>) -> Self {
        Self {
            function,
            upvalues: Vec::new(),
        }
    }
}

pub type Upvalue = Rc<Cell<Value>>;

#[derive(Debug)]
pub enum Object {
    String(ObjString),
    Function(ObjFunction),
    Closure(ObjClosure),
    Upvalue(Upvalue),
}

impl Display for Object {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Object::String(s) => write!(f, "{}", s),
            Object::Function(fun) => write!(f, "{}", fun),
            Object::Closure(c) => {
                write!(f, "{}", unsafe { c.function.as_ref() })
            }
            Object::Upvalue(uv) => write!(f, "{}", uv.get()),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct ObjRef(NonNull<Object>);

impl ObjRef {
    pub fn as_function(&self) -> Option<NonNull<ObjFunction>> {
        match unsafe { self.0.as_ref() } {
            Object::Function(f) => Some(NonNull::from(f)),
            _ => None,
        }
    }

    pub fn as_closure(&self) -> Option<NonNull<ObjClosure>> {
        match unsafe { self.0.as_ref() } {
            Object::Closure(c) => Some(NonNull::from(c)),
            _ => None,
        }
    }
}

impl Deref for ObjRef {
    type Target = Object;
    fn deref(&self) -> &Self::Target {
        unsafe { self.0.as_ref() }
    }
}

impl DerefMut for ObjRef {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.0.as_mut() }
    }
}

impl From<*mut Object> for ObjRef {
    fn from(ptr: *mut Object) -> Self {
        ObjRef(NonNull::new(ptr).unwrap())
    }
}

impl PartialEq for ObjRef {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_ptr() == other.0.as_ptr()
    }
}
