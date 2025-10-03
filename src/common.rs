use core::fmt;
use std::{
    borrow::Cow,
    fmt::write,
    ops::{Add, Div, Mul, Neg, Sub},
};

use crate::vm::Chunk;

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
    OpReturn,
}

#[derive(Clone, Copy, Debug)]
pub enum ObjectRef<'src> {
    String(*const ObjString<'src>),
}

impl<'src> PartialEq for ObjectRef<'src> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ObjectRef::String(a), ObjectRef::String(b)) => unsafe { (**a).chars == (**b).chars },
            _ => false,
        }
    }
}

#[derive(Debug)]
pub struct ObjString<'src> {
    pub chars: Cow<'src, str>,
}

#[derive(Debug, Copy, Clone)]
pub enum Value<'src> {
    Nil,
    Number(f64),
    Bool(bool),
    Object(ObjectRef<'src>),
}

impl<'src> PartialEq for Value<'src> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Nil, Value::Nil) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Number(a), Value::Number(b)) => a == b,
            (Value::Object(a), Value::Object(b)) => a == b,
            _ => false,
        }
    }
}

impl Default for Value<'_> {
    fn default() -> Self {
        Value::Nil
    }
}

impl fmt::Display for Value<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Nil => write!(f, "Nil"),
            Value::Number(x) => write!(f, "{}", x),
            Value::Bool(x) => write!(f, "{}", x),
            Value::Object(obj) => write!(f, "{}", obj),
        }
    }
}

impl fmt::Display for ObjectRef<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ObjectRef::String(obj_str_ptr) => {
                let str = unsafe { &**obj_str_ptr };
                write!(f, "\"{}\"", str.chars)
            }
            _ => write!(f, ""),
        }
    }
}

impl<'src> Neg for Value<'src> {
    type Output = Value<'src>;

    fn neg(self) -> Value<'src> {
        match self {
            Value::Number(a) => Value::Number(-a),
            _ => panic!("Operand must be a number."),
        }
    }
}
