use core::fmt;
use std::ops::Neg;

use crate::object::{ObjRef, Object};

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

    JumpIfFalse(hi: u8, lo: u8),
    Jump(hi: u8, lo: u8),
    Loop(hi: u8, lo: u8),

    SwitchEq,
}

impl From<OpCode> for u8 {
    fn from(value: OpCode) -> Self {
        value as u8
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub enum Value {
    #[default]
    Nil,
    Number(f64),
    Bool(bool),
    Obj(ObjRef),
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Nil, Value::Nil) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Number(a), Value::Number(b)) => a == b,
            (Value::Obj(a), Value::Obj(b)) => a == b,
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
            Value::Obj(obj) => match &**obj {
                Object::String(obj_string) => write!(f, "{}", &(**obj_string)),
                _ => todo!(),
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
