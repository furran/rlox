use core::fmt;
use std::ops::Neg;

use rlox_gc::{Gc, Trace};

use crate::object::{ObjBoundMethod, ObjClass, ObjClosure, ObjFunction, ObjInstance, ObjString};

macro_rules! define_instructions {
    (
        $(
            $variant:ident $(($($param:ident : $param_type:ty), *))? $(= $opcode:literal)?
        ), * $(,)?
    ) => {

        #[derive(Debug, PartialEq)]
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
    Call(arg_count: u8),
    Invoke(name_index: u8, arg_count: u8),
    SuperInvoke(name_index: u8, arg_count: u8),
    Closure,
    CloseUpvalue,
    Return,

    Class(name_index: u8),
    Inherit,
    Method(name_index: u8),
    GetSuper(name_index: u8),
    SetProperty(prop_index: u8),
    GetProperty(prop_index: u8),
    DeleteProperty(index: u8),

    SetIndex,
    GetIndex,

    SetLocal(index: u8),
    GetLocal(index: u8),

    DefineGlobal(index: u8),
    SetGlobal(index: u8),
    GetGlobal(index: u8),

    SetUpvalue(index: u8),
    GetUpvalue(index: u8),

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

#[derive(Debug, Copy, Clone, Default, Trace)]
pub enum Value {
    #[default]
    Nil,
    Number(f64),
    Bool(bool),
    String(Gc<ObjString>),
    Function(Gc<ObjFunction>),
    Closure(Gc<ObjClosure>),
    Class(Gc<ObjClass>),
    Instance(Gc<ObjInstance>),
    BoundMethod(Gc<ObjBoundMethod>),
}

impl Value {
    pub fn is_falsey(&self) -> bool {
        match self {
            Value::Bool(x) => !x,
            Value::Nil => true,
            _ => false,
        }
    }

    pub fn unwrap_string(self) -> Gc<ObjString> {
        match self {
            Value::String(s) => s,
            other => panic!("Expected Value::String, got {:?}.", other),
        }
    }

    pub fn unwrap_function(self) -> Gc<ObjFunction> {
        match self {
            Value::Function(f) => f,
            other => panic!("Expected Value::Function, got {:?}.", other),
        }
    }

    pub fn unwrap_closure(self) -> Gc<ObjClosure> {
        match self {
            Value::Closure(c) => c,
            other => panic!("Expected Value::Closure, got {:?}.", other),
        }
    }

    pub fn unwrap_class(self) -> Gc<ObjClass> {
        match self {
            Value::Class(c) => c,
            other => panic!("Expected Value::Class, got {:?}.", other),
        }
    }

    pub fn unwrap_instance(self) -> Gc<ObjInstance> {
        match self {
            Value::Instance(i) => i,
            other => panic!("Expected Value::Instance, got {:?}.", other),
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
            (Value::Function(a), Value::Function(b)) => a == b,
            (Value::Closure(a), Value::Closure(b)) => a == b,
            _ => false,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Value::Nil => write!(f, "nil"),
            Value::Number(x) => write!(f, "{}", x),
            Value::Bool(x) => write!(f, "{}", x),
            Value::String(x) => write!(f, "{}", x),
            Value::Function(x) => write!(f, "{}", x),
            Value::Closure(x) => write!(f, "{}", x),
            Value::Class(x) => write!(f, "{}", x),
            Value::Instance(x) => write!(f, "{}", x),
            Value::BoundMethod(x) => write!(f, "{}", x.method.function),
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
