use core::fmt;
use std::{
    borrow::Borrow,
    hash::{Hash, Hasher},
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

use crate::vm::Chunk;

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
    chunk: Chunk,
    name: NonNull<ObjString>,
    arity: u32,
}

#[derive(Debug)]
pub enum Object {
    String(ObjString),
    Function(ObjFunction),
}

#[derive(Debug, Copy, Clone)]
pub struct ObjRef(NonNull<Object>);

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
