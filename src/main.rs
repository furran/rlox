use crate::common::Value;
use crate::vm::VM;
use crate::vm::vm::VMResult;

pub mod common;
pub mod compiler;
pub mod vm;

fn main() {
    let _ = VM::repl();
}
