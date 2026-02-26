use core::fmt;
use std::{
    borrow::{Borrow, Cow},
    hash::Hash,
    ops::Neg,
    ptr::NonNull,
};

macro_rules! define_instructions {
    (
        $(
            $variant:ident $(($($param:ident : $param_type:ty), *))? $(= $opcode:literal)?
        ), * $(,)?
    ) => {

        #[allow(non_upper_case_globals)]
        pub mod opcodes {
            $(
                pub const $variant: u8 = super::OpCode::$variant as u8;
            )*
        }

        #[derive(Debug)]
        #[repr(u8)]
        pub enum OpCode {
            $(
                $variant $(= $opcode)?
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
    OpConstant(index: u8),
    OpNil,
    OpTrue,
    OpFalse,
    OpAdd,
    OpSubtract,
    OpMultiply,
    OpDivide,
    OpNegate,
    OpNot,
    OpEqual,
    OpGreater,
    OpLess,
    OpPrint,
    OpPop,
    OpReturn,
}

#[derive(Debug, Clone)]
pub struct ObjString {
    pub str: String,
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

pub fn alloc_owned_string(chars: String) -> NonNull<ObjString> {
    let boxed = Box::new(ObjString { str: chars });

    let ptr = Box::leak(boxed);

    unsafe { NonNull::new_unchecked(ptr) }
}

#[derive(Debug, Copy, Clone, Default)]
pub enum Value {
    #[default]
    Nil,
    Number(f64),
    Bool(bool),
    String(*const ObjString),
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Nil, Value::Nil) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Number(a), Value::Number(b)) => a == b,
            (Value::String(a), Value::String(b)) => unsafe { (**a).str == (**b).str },
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
