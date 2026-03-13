use rlox::vm::{VM, vm::VMError};

fn run(source: &str) -> String {
    let mut output = Vec::new();
    let mut vm = VM::new(&mut output);
    vm.interpret(source).unwrap();
    String::from_utf8(output).unwrap()
}

fn run_repl(lines: &[&str]) -> String {
    let mut output = Vec::new();
    let mut vm = VM::new(&mut output);
    for line in lines {
        vm.interpret(line).unwrap();
    }
    String::from_utf8(output).unwrap()
}

#[test]
fn test_string_interning() {
    let output = run(r#"
        var x = "hello";
        var y = "hello";
        print x == y;
    "#);
    assert_eq!(output.trim(), "true");
}

#[test]
fn test_global_variable_definition() {
    let output = run_repl(&["var x = 1+2;", "print x;"]);
    assert_eq!(output.trim(), "3");
}

#[test]
fn test_global_variable_reassignment() {
    let output = run_repl(&["var x = 1;", "x = 2;", "print x;"]);
    assert_eq!(output.trim(), "2");
}

#[test]
fn test_global_variable_uninitialized() {
    let output = run(r#"
        var x;
        print x;
    "#);
    assert_eq!(output.trim(), "Nil")
}

#[test]
fn test_local_variable_scoping() {
    let output = run(r#"
        var x = "global";
        {
            var x = "local";
            print x;
        }
        print x;
    "#);
    assert_eq!(output, "local\nglobal\n");
}

#[test]
fn test_local_variable_not_visible_outside_scope() {
    let mut output = Vec::new();
    let mut vm = VM::new(&mut output);
    let result = vm.interpret(
        r#"
        {
            var x = "local";
        }
        print x;
    "#,
    );
    assert!(result.is_err());
}

#[test]
fn test_string_concatenation() {
    let output = run(r#"
        var a = "hello";
        var b = " world";
        print a + b;
    "#);
    assert_eq!(output.trim(), "hello world");
}

#[test]
fn test_if_else() {
    let output = run(r#"
        if (true) print "yes"; else print "no";
        if (false) print "yes"; else print "no";
    "#);
    assert_eq!(output, "yes\nno\n");
}

#[test]
fn test_while_loop() {
    let output = run(r#"
        var i = 0;
        while (i < 3) {
            print i;
            i = i + 1;
        }
    "#);
    assert_eq!(output, "0\n1\n2\n");
}

#[test]
fn test_for_loop() {
    let output = run(r#"
        for (var i = 0; i < 3; i = i + 1) {
            print i;
        }
    "#);
    assert_eq!(output, "0\n1\n2\n");
}

#[test]
fn test_switch_basic_match() {
    let output = run(r#"
        switch(1+2) {
            case 1: print "one";
            case 2: print "two";
            case 3: print "three";
            case 4: print "four";
            default: print "default";
        }
    "#);
    assert_eq!(output.trim(), "three");
}

#[test]
fn test_switch_no_fallthrough() {
    let output = run(r#"
        switch (1) {
            case 1: print "one";
            case 1: print "also one";
        }
    "#);
    assert_eq!(output.trim(), "one");
}

#[test]
fn test_switch_no_match_no_default() {
    let output = run(r#"
        switch (3) {
            case 1: print "one";
            case 2: print "two";
        }
    "#);
    assert_eq!(output.trim(), "");
}

#[test]
fn test_switch_default() {
    let output = run(r#"
        switch (3) {
            case 1: print "one";
            case 2: print "two";
            default: print "other";
        }
    "#);
    assert_eq!(output.trim(), "other");
}

#[test]
fn test_switch_only_default() {
    let output = run(r#"
        switch (1) {
            default: print "default";
        }
    "#);
    assert_eq!(output.trim(), "default");
}

#[test]
fn test_switch_multiple_statements_in_case() {
    let output = run(r#"
        switch (1) {
            case 1:
                print "a";
                print "b";
                print "c";
            case 2: print "two";
        }
    "#);
    assert_eq!(output, "a\nb\nc\n");
}

#[test]
fn test_switch_does_not_affect_stack() {
    let output = run(r#"
        var x = 1;
        switch (x) {
            case 1: print "one";
        }
        print x;
    "#);
    assert_eq!(output, "one\n1\n");
}

#[test]
fn test_basic_function_call() {
    let output = run(r#"
        fun greet() {
            print "hello";
        }
        greet();
    "#);
    assert_eq!(output.trim(), "hello");
}

#[test]
fn test_function_with_return_value() {
    let output = run(r#"
        fun answer() {
            return 42;
        }
        print answer();
    "#);
    assert_eq!(output.trim(), "42");
}

#[test]
fn test_function_with_args() {
    let output = run(r#"
        fun add(a, b) {
            return a + b;
        }
        print add(1, 2);
    "#);
    assert_eq!(output.trim(), "3");
}

#[test]
fn test_function_wrong_arg_count() {
    let mut vm = VM::new(Vec::new());
    let result = vm.interpret(
        r#"
        fun add(a, b) {
            return a + b;
        }
        add(1);
    "#,
    );
    assert!(result.is_err());
}

#[test]
fn test_function_as_value() {
    let output = run(r#"
        fun greet() {
            print "hello";
        }
        var f = greet;
        f();
    "#);
    assert_eq!(output.trim(), "hello");
}

#[test]
fn test_recursive_function() {
    let output = run(r#"
        fun fib(n) {
            if (n <= 1) return n;
            return fib(n - 1) + fib(n - 2);
        }
        print fib(10);
    "#);
    assert_eq!(output.trim(), "55");
}

#[test]
fn test_nested_function_calls() {
    let output = run(r#"
        fun inner() {
            return 1;
        }
        fun outer() {
            return inner() + 1;
        }
        print outer();
    "#);
    assert_eq!(output.trim(), "2");
}

#[test]
fn test_function_stack_trace_on_error() {
    let mut vm = VM::new(Vec::new());
    let result = vm.interpret(
        r#"
        fun inner() {
            return undefined;
        }
        fun outer() {
            return inner();
        }
        outer();
    "#,
    );
    match result {
        Err(VMError::RuntimeError(msg)) => {
            assert!(msg.contains("inner"));
            assert!(msg.contains("outer"));
            assert!(msg.contains("script"));
        }
        _ => panic!("Expected runtime error"),
    }
}

#[test]
fn test_undefined_variable_is_runtime_error() {
    let mut output = Vec::new();
    let mut vm = VM::new(&mut output);
    let result = vm.interpret("print undefined;");
    assert!(result.is_err());
}

#[test]
fn test_type_error_in_arithmetic() {
    let mut output = Vec::new();
    let mut vm = VM::new(&mut output);
    let result = vm.interpret(r#"print "string" - 1;"#);
    assert!(result.is_err());
}
