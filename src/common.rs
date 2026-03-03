use core::fmt;
use std::{
    borrow::Borrow,
    hash::{Hash, Hasher},
    ops::{Deref, Neg},
};

macro_rules! define_instructions {
    (
        $(
            $variant:ident $(($($param:ident : $param_type:ty), *))? $(= $opcode:literal)?
        ), * $(,)?
    ) => {

        #[derive(Debug)]
        #[repr(u8)]
        pub enum OpCode {
            $(
                $variant
            ),*
        }

        impl From<u8> for OpCode {
            fn from(byte: u8) -> Self {
                $(
                    if byte == (OpCode::$variant as u8) {
                        return OpCode::$variant;
                    }
                )*
                panic!("Unknown OpCode {}", byte)
            }
        }

        impl OpCode {
            pub fn operand_count(&self) -> usize {
                match self {
                    $(
                        OpCode::$variant  => { count_params!($(  $($param), *)?)  }
                    ),*
                }
            }
        }
    };
}

macro_rules! count_params {
    () => { 0 };
    ($param:expr) => {
        1
    };
    ($param:expr, $($rest:expr),+) => { 1 + count_params!($($rest),+)};
}

define_instructions! {
    Constant(index: u8),
    Nil,
    True,
    False,
    Add,
    Subtract,
    Multiply,
    Divide,
    Negate,
    Not,
    Equal,
    Greater,
    Less,
    Print,
    Pop,
    Return,

    SetLocal(index: u8),
    GetLocal(index: u8),

    DefineGlobal(index: u8),
    SetGlobal(index: u8),
    GetGlobal(index: u8),
}

impl From<OpCode> for u8 {
    fn from(value: OpCode) -> Self {
        value as u8
    }
}

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
pub struct ObjStringPtr(pub *const ObjString);

impl Hash for ObjStringPtr {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub enum Value {
    #[default]
    Nil,
    Number(f64),
    Bool(bool),
    String(*const ObjString),
}

impl Value {
    pub fn as_obj_string(&self) -> Option<&ObjString> {
        if let Value::String(ptr) = self {
            Some(unsafe { &**ptr })
        } else {
            None
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Nil, Value::Nil) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Number(a), Value::Number(b)) => a == b,
            (Value::String(a), Value::String(b)) => a == b,
            _ => false,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Value::Nil => write!(f, "Nil"),
            Value::Number(x) => write!(f, "{}", x),
            Value::Bool(x) => write!(f, "{}", x),
            Value::String(obj) => write!(f, "{}", unsafe { &(**obj).str }),
        }
    }
}

impl Neg for Value {
    type Output = Value;

    fn neg(self) -> Value {
        match self {
            Value::Number(a) => Value::Number(-a),
            _ => panic!("Operand must be a number."),
        }
    }
}
