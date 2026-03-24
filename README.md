# rlox

A bytecode virtual machine interpreter for the Lox programming language implemented in Rust. Based on the third section of [Crafting Interpreters](https://craftinginterpreters.com/a-bytecode-virtual-machine.html).

## Features

### Core language

- Dynamic typing with `nil`, `bool`, `number`, and `string` primitives
- Arithmethic, comparison and logical operatos
- Control flow: `if` / `else`, `while`, `for`
- Classes with methods, fields and single inheritence
- `super` and `this`
- native functions: `clock()`

### Extensions beyond clox

- `delete` keyword for removing fields from instances
- `switch` / `case` / `default` statements (no fallthrough)
- Access to undefined instance fields are `nil` instead of runtime error
- Support for dynamic field names for instances (i.e. `x["hello world"] = 1`)

### Some implementation details

- Generic `Gc<T>` 'smart' pointer with type-erased GC header
- Interior mutability via `Cell` and `RefCell` for `Gc` objects
- Globals are resolved at compile time, and indexed as a vector at runtime, avoiding hash map lookups
- The VM assumes compiler correctness, so we avoid bounds checking at runtime (safe-ishly)

## Usage

```bash
# Run lox script
cargo run --release -- script.lox
```

```bash
# start the REPL
cargo run --release
```

```bash
# run the basic test suit
cargo test --test vm_tests
```

```bash
# or run the crafting interpreters test suite (more reliable, really)
cargo test --test lox_tests
```

```bash
# Compare performance against clox (requires a compiled clox binary in project root)
cargo test --test compare --release --features bench_compare -- --no-capture
```

## The GC

Heavily inspired by this great blogpost: [Designing a GC in Rust](https://manishearth.github.io/blog/2015/09/01/designing-a-gc-in-rust/)

The garbage collector is implemented as a separate crate (`rlox_gc`) and is generic-ish\* over any type implementing or deriving `Trace`. It's not really safe and it relies on the VM to uphold it's invariants. It works like this:

- **Type-erased intrusive linked list**: all allocations are linked through a `GcHeader` embedded at offset 0 of every `Gc Object<T>`. The heap only sees headers; typed access goes through `Gc<T>` handles.
- **`Gc<T>` is `Copy`**: Gc handles are plain pointer with no ownership semantics
- **Manual `Drop` vtable**: type-erased `drop` and `trace` fn pointers are stamped into `GcHeader` allowing the gc to call the correct destructor without knowing `T`
- **#`[derive(Trace)]`**: proc macro auto generates `Trace` implementations that recursively mark all `Gc<T>` fields.

\* not really generic since for this specific use case, we have to have access to the internals. More specifically, marked `Gc<ObjString>`'s so our interner (non-root) doesn't hold onto dangling pointers.

## Performance

Benchmarked against the reference clox implementation (compiled using `gcc -O3 -o clox *.c`) on a selection of the official benchmarks:

| benchmark           | clox     | rlox     | ratio |
| ------------------- | -------- | -------- | ----- |
| binary_trees.lox    | 2.50s    | 4.85s    | 1.94x |
| equality.lox        | 2.80s    | 4.98s    | 1.78x |
| fib.lox             | 728.00ms | 1.49s    | 2.05x |
| instantiation.lox   | 851.00ms | 1.10s    | 1.30x |
| invocation.lox      | 228.00ms | 675.90ms | 2.96x |
| method_call.lox     | 148.00ms | 410.95ms | 2.78x |
| properties.lox      | 374.00ms | 961.07ms | 2.57x |
| string_equality.lox | 0.00ns   | 152.26ms | 0.00x |
| trees.lox           | 4.23s    | 9.66s    | 2.29x |
| zoo.lox             | 270.00ms | 669.76ms | 2.48x |
| zoo_batch.lox       | 10.00s   | 10.00s   | 1.00x |
