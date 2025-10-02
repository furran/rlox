use core::fmt;
use std::{
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ObjectRef {
    String(*const ObjString),
    Function(*const ObjFunction),
}

pub struct ObjString {
    pub chars: String,
}

pub struct ObjFunction {
    pub name: Option<String>,
    pub arity: u8,
    pub chunk: Chunk,
}

pub fn alloc_string(chars: String) -> *const ObjString {
    let boxed = Box::new(ObjString { chars });
    Box::leak(boxed) as *const ObjString
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum Value {
    Nil,
    Number(f64),
    Bool(bool),
    Object(ObjectRef),
}

impl Default for Value {
    fn default() -> Self {
        Value::Nil
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Nil => write!(f, "Nil"),
            Value::Number(x) => write!(f, "{:4}", x),
            Value::Bool(x) => write!(f, "{}", x),
            Value::Object(obj) => match obj {
                ObjectRef::String(obj_str_ptr) => {
                    let str = unsafe { &**obj_str_ptr };
                    write!(f, "{}", str.chars)
                }
                _ => write!(f, ""),
            },
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
