use rlox::vm::VM;

fn expects_error(path: &str) -> bool {
    let source = std::fs::read_to_string(path).unwrap();
    source.lines().any(|line| {
        line.contains("// Error")
            || line.contains("// expect runtime error")
            || line.contains("// [line")
    })
}

fn expected_output(path: &str) -> String {
    let source = std::fs::read_to_string(path).unwrap();
    source
        .lines()
        .filter_map(|line| line.split_once("expect: ").map(|(_, rest)| rest.trim()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn run_test(path: &str) {
    let source = std::fs::read_to_string(path).unwrap();
    let mut output = Vec::new();
    let mut vm = VM::new(&mut output);
    let result = vm.interpret(&source);
    drop(vm);
    let output = String::from_utf8(output)
        .unwrap()
        .trim()
        .to_string()
        .replace("\r\n", "\n");
    if expects_error(path) {
        assert!(
            result.is_err(),
            "Test expected error but succeeded: {}",
            path
        )
    } else {
        if let Err(e) = result {
            assert!(false, "Test failed with error: {}\n {}", path, e);
        }
        let expected = expected_output(path);
        assert_eq!(output, expected, "Test failed: {}", path);
    }
}

#[test]
fn test_lox_files() {
    let test_dir = "./test";
    let skip_dirs = ["benchmark", "expressions", "scanning"];

    for entry in walkdir::WalkDir::new(test_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "lox"))
    {
        let path = entry.path().to_str().unwrap();

        if skip_dirs.iter().any(|d| path.contains(d)) {
            continue;
        }

        println!("Testing: {path}");

        run_test(path);
    }
}
