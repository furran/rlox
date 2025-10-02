use std::env;

use crate::vm::VM;
use crate::vm::vm::VMResult;

pub mod common;
pub mod compiler;
pub mod vm;

fn main() -> VMResult {
    // unsafe { env::set_var("RUST_BACKTRACE", "1") };
    let result = VM::interpret("3>4\n");
    VM::repl();
    result
}
