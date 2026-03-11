use crate::vm::VM;

mod common;
pub mod compiler;
mod object;
pub mod vm;

fn main() {
    let _ = VM::repl();
}
