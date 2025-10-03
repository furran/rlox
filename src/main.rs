use crate::vm::VM;
use crate::vm::vm::VMResult;

pub mod common;
pub mod compiler;
pub mod vm;

fn main() -> VMResult {
    let result = VM::interpret("3>4\n");
    VM::repl();
    result
}
