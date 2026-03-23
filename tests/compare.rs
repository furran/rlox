use std::process::Command;
use std::time::Duration;

use walkdir::WalkDir;

fn get_benchmarks() -> Vec<(String, String)> {
    WalkDir::new("./test/benchmark/")
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "lox"))
        .map(|e| {
            (
                e.file_name().to_str().unwrap().to_string(),
                e.path().to_str().unwrap().to_string(),
            )
        })
        .collect()
}

fn run_clox(path: &str) -> Duration {
    let output = Command::new("./clox")
        .arg(path)
        .output()
        .expect("failed to run clox");
    let stdout = String::from_utf8(output.stdout).unwrap();
    stdout
        .lines()
        .last()
        .and_then(|line| line.trim().parse::<f64>().ok())
        .map(|secs| Duration::from_secs_f64(secs))
        .unwrap_or(Duration::ZERO)
}

fn run_rlox(path: &str) -> Duration {
    let source = std::fs::read_to_string(path).unwrap();
    let mut output = Vec::new();
    let mut vm = rlox::vm::VM::new(&mut output);
    let _ = vm.interpret(&source);
    drop(vm);
    let stdout = String::from_utf8(output).unwrap();
    stdout
        .lines()
        .last()
        .and_then(|line| line.trim().parse::<f64>().ok())
        .map(|secs| Duration::from_secs_f64(secs))
        .unwrap_or(Duration::ZERO)
}

fn main() {
    let benchmarks = get_benchmarks();

    let name_width = benchmarks.iter().map(|(n, _)| n.len()).max().unwrap_or(10);
    println!(
        "| {:<width$} | {:>10} | {:>10} | {:>8} |",
        "benchmark",
        "clox",
        "rlox",
        "ratio",
        width = name_width
    );

    println!(
        "|{:-<width$}|{:-<12}|{:-<12}|{:-<10}|",
        "",
        "",
        "",
        "",
        width = name_width + 2
    );

    for (name, path) in &benchmarks {
        let clox = run_clox(path);
        let rlox = run_rlox(path);
        let ratio = if clox > Duration::ZERO && rlox > Duration::ZERO {
            rlox.as_secs_f64() / clox.as_secs_f64()
        } else {
            0.0
        };

        println!(
            "| {:<4$} | {:>10.2?} | {:>10.2?} | {:>7.2}x |",
            name, clox, rlox, ratio, name_width
        );
    }
}
