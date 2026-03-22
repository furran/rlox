use rlox::{common::Value, object::NativeFn, vm::vm::repl};
use rlox_gc::Gc;

fn main() {
    println!("Value size: {}", std::mem::size_of::<Value>());
    dbg!(std::mem::size_of::<NativeFn>()); // 8 — fn pointer
    dbg!(std::mem::size_of::<&'static str>()); // 16 — fat pointer
    dbg!(std::mem::size_of::<Gc<()>>()); // 8 — thin pointer
    dbg!(std::mem::size_of::<f64>());
    let _ = repl();
}
