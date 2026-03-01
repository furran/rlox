use crate::vm::VM;

pub mod common;
pub mod compiler;
pub mod vm;

fn main() {
    let _ = VM::repl();
}
