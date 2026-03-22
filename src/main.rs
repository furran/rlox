use std::io::Write;

use rlox::vm::{VM, vm::VMError};

pub fn repl() {
    let stdin = std::io::stdin();
    let mut vm = VM::new(std::io::stdout());

    loop {
        print!(">>> ");
        vm.output.flush().unwrap();

        let mut buffer = String::new();
        stdin.read_line(&mut buffer).unwrap();

        while !is_complete(&buffer) {
            print!("... ");
            vm.output.flush().unwrap();
            stdin.read_line(&mut buffer).unwrap();
        }

        if let Err(e) = vm.interpret(&buffer) {
            match e {
                VMError::CompileError(errors) => {
                    for error in errors {
                        eprintln!("[CompileError] {}", error);
                    }
                }
                VMError::RuntimeError(msg) => eprintln!("[RuntimeError] {}", msg),
            }
        }
    }
}

fn is_complete(source: &str) -> bool {
    let mut depth = 0;
    let mut in_string = false;

    for c in source.chars() {
        match c {
            '"' => in_string = !in_string,
            '{' if !in_string => depth += 1,
            '}' if !in_string => depth -= 1,
            _ => {}
        }
    }
    depth == 0
}

pub fn run_file(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let source = std::fs::read_to_string(path)?;
    let output = std::io::stdout();
    let mut vm = VM::new(output);
    vm.interpret(&source)?;
    Ok(())
}

fn main() {
    let args = std::env::args().collect::<Vec<String>>();
    match args.len() {
        1 => repl(),
        2 => {
            if let Err(e) = run_file(&args[1]) {
                eprintln!("{}", e);
            }
        }
        _ => {
            eprintln!("Usage: rlox [file]");
            std::process::exit(64)
        }
    }
}
