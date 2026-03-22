use rlox::{
    common::Value,
    vm::{
        VM,
        vm::{VMError, VMResult},
    },
};

fn run(source: &str) -> String {
    let mut output = Vec::new();
    let mut vm = VM::new(&mut output);
    vm.interpret(source).unwrap();
    drop(vm);
    String::from_utf8(output).unwrap()
}

fn run_repl(lines: &[&str]) -> String {
    let mut output = Vec::new();
    let mut vm = VM::new(&mut output);
    for line in lines {
        vm.interpret(line).unwrap();
    }
    drop(vm);
    String::from_utf8(output).unwrap()
}

fn run_repl_results(lines: &[&str]) -> Vec<VMResult> {
    let mut output = Vec::new();
    let mut vm = VM::new(&mut output);
    let mut results = vec![];
    for line in lines {
        results.push(vm.interpret(line));
    }
    drop(vm);
    results
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
    assert_eq!(output.trim(), "nil")
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
fn test_closure_global_variable() {
    let output = run(r#"
        var x = "global";
        fun outer() {
            fun inner() {
                print x;
            }
            inner();
        }
        outer();
    "#);

    assert_eq!(output.trim(), "global");
}

#[test]
fn test_closure_global_variable_and_local_variables() {
    let output = run(r#"
        var x = "global";
        fun outer() {
            var x = "outer";
            fun inner() {
                print x;
            }
            inner();
        }
        outer();
    "#);

    assert_eq!(output.trim(), "outer");
}

#[test]
fn test_closure_undefined_variable() {
    let mut vm = VM::new(Vec::new());
    let result = vm.interpret(
        r#"
        fun outer() {
            fun inner() {
                print x;
            }
            inner();
        }
        outer();
    "#,
    );

    assert!(result.is_err());
}

#[test]
fn test_closure_closed_upvalues() {
    let output = run(r#"
        fun outer() {
            var x = "outside";
            fun inner() {
                print x;
            }

            return inner;
        }
        var closure = outer();
        closure();
    "#);

    assert_eq!(output.trim(), "outside");
}

#[test]
fn test_closure_mutate_shared_upvalue() {
    let output = run(r#"
        var globalSet;
        var globalGet;

        fun main() {
            var a = "initial";

            fun set() { a = "updated"; }
            fun get() { print a; }

            globalSet = set;
            globalGet = get;
        }

        main();
        globalSet();
        globalGet();
    "#);
    assert_eq!(output.trim(), "updated");
}

#[test]
fn test_independent_closures_dont_share_state() {
    let output = run(r#"
        fun makeCounter() {
            var count = 0;
            fun increment() {
                count = count + 1;
                return count;
            }
            return increment;
        }
        var a = makeCounter();
        var b = makeCounter();
        print a();
        print a();
        print b();
    "#);
    assert_eq!(output, "1\n2\n1\n");
}

#[test]
fn test_nested_closures() {
    let output = run(r#"
        fun outer() {
            var x = 1;
            fun middle() {
                var y = 2;
                fun inner() {
                    return x + y;
                }
                return inner;
            }
            return middle;
        }
        print outer()()();
    "#);
    assert_eq!(output.trim(), "3");
}

#[test]
fn test_class_add_and_set_fields() {
    let output = run(r#"
        class Pair {}
        var pair = Pair();
        pair.first = 1;
        pair.second = 2;
        print pair.first + pair.second;
    "#);

    assert_eq!(output.trim(), "3");
}

#[test]
fn test_class_persistence_across_repl_lines() {
    let output = run_repl(&[
        "class Pair {}",
        "var pair = Pair();",
        "pair.first = 1;",
        "pair.second = 2;",
        "print pair.first + pair.second;",
    ]);

    assert_eq!(output.trim(), "3");
}

#[test]
fn test_locally_defined_class() {
    let output = run(r#"
        {
            class Local {}
            var local = Local();
            local.hello = "hello";
            print local.hello;
        }
    "#);

    assert_eq!(output.trim(), "hello");
}

#[test]
fn test_locally_defined_class_is_out_of_scope() {
    let output = Vec::new();
    let mut vm = VM::new(output);
    let result = vm.interpret(
        r#"
        {
            class Local {}
            var local = Local();
            local.hello = "hello";
            print local.hello;
        }
        var global = Local();
    "#,
    );
    assert!(result.is_err());
}

#[test]
fn test_return_class_from_function() {
    let output = run(r#"
        fun test() {
            class A {}
            return A;
        }
        var x = test();
        var y = x();
        print y;
    "#);

    assert_eq!(output.trim(), "<A instance>");
}

#[test]
fn test_get_undefined_property() {
    let output = run(r#"
        class A {}
        var x = A();
        print x.fish;
    "#);

    assert_eq!(output.trim(), format!("{}", Value::Nil))
}

#[test]
fn test_set_property_of_non_instance() {
    let output = Vec::new();
    let mut vm = VM::new(output);
    let result = vm.interpret(
        r#"
        var x = "fish";
        x.fish = 1;
    "#,
    );

    assert!(result.is_err());
}

#[test]
fn test_dynamic_field_names_set_get_basic() {
    let output = run(r#"
        class A{}
        var x = A();
        x["a"] = 1;
        print x["a"];
        print x.a;
    "#);

    assert_eq!(output.trim(), "1\n1");
}

#[test]
fn test_dynamic_field_names_dynamic_key() {
    let output = run(r#"
        class A{}
        var x = A();
        var hello = "hello";
        var world = " world";
        x[hello + world] = 1;
        print x["hello world"];
    "#);

    assert_eq!(output.trim(), "1");
}

#[test]
fn test_dynamic_field_names_dot_same_field() {
    let output = run(r#"
        class A {}
        var x = A();
        x.field = 1;
        print x["field"];
        x["field"] = 2;
        print x.field;
    "#);
    assert_eq!(output.trim(), "1\n2");
}

#[test]
fn test_dynamic_field_names_missing_field_returns_nil() {
    let output = run(r#"
        class A {}
        var x = A();
        print x["nonexistent"];
    "#);
    assert_eq!(output.trim(), format!("{}", Value::Nil));
}

#[test]
fn test_dynamic_field_names_non_string_key_errors() {
    let output = Vec::new();
    let mut vm = VM::new(output);
    let result = vm.interpret(
        r#"
        class A {}
        var x = A();
        x[42] = 1;
    "#,
    );
    assert!(result.is_err());
}

#[test]
fn test_dynamic_field_names_non_instance_errors() {
    let output = Vec::new();
    let mut vm = VM::new(output);
    let result = vm.interpret(
        r#"
        var x = "not an instance";
        x["field"] = 1;
    "#,
    );
    assert!(result.is_err());
}

#[test]
fn test_delete_field_basic() {
    let output = run(r#"
        class A {}
        var x = A();
        x.fish = "fish";
        delete x.fish;
        print x.fish;
    "#);
    assert_eq!(output.trim(), format!("{}", Value::Nil));
}

#[test]
fn test_delete_undefined_field() {
    let output = run(r#"
        class A {}
        var x = A();
        delete x.fish;
        print x.fish;
    "#);
    assert_eq!(output.trim(), format!("{}", Value::Nil));
}

#[test]
fn test_delete_field_is_expression() {
    let output = run(r#"
        class A {}
        var x = A();
        x.fish = "fish";
        var f = delete x.fish;
        print f;
    "#);
    assert_eq!(output.trim(), "fish");
}

#[test]
fn test_delete_field_from_non_instance_errors() {
    let output = Vec::new();
    let mut vm = VM::new(output);
    let result = vm.interpret(
        r#"
        var x = "fish";
        delete x.fish;
    "#,
    );
    assert!(result.is_err());
}

#[test]
fn test_access_method() {
    let output = run(r#"
        class Brunch {  
            eggs() {}
        }

        var brunch = Brunch();
        var eggs = brunch.eggs;
        print eggs;
    "#);
    assert_eq!(output.trim(), "<fn eggs>");
}

#[test]
fn test_call_method() {
    let output = run(r#"
        class Scone {
            topping(first, second) {
                print "scone with " + first + " and " + second;
            }
        }

        var scone = Scone();
        scone.topping("berries", "cream");
    "#);
    assert_eq!(output.trim(), "scone with berries and cream");
}

#[test]
fn test_call_nested_method_with_this() {
    let output = run(r#"
        class Nested {
            method() {
                fun function() {
                    print this;
                }
                function();
            }
        }
        Nested().method();
    "#);
    assert_eq!(output.trim(), "<Nested instance>");
}

#[test]
fn test_instance_initializer() {
    let output = run(r#"
        class CoffeeMaker {
            init(coffee) {
                this.coffee = coffee;
            }

            brew() {
                print "Enjoy your cup of " + this.coffee;
                this.coffee = nil;
            }
        }

        var maker = CoffeeMaker("coffee and chicory");
        maker.brew();
    "#);
    assert_eq!(output.trim(), "Enjoy your cup of coffee and chicory");
}

#[test]
fn test_instance_initializer_errors_on_return() {
    let mut vm = VM::new(Vec::new());
    let result = vm.interpret(
        r#"
        class CoffeeMaker {
            init(coffee) {
                this.coffee = coffee;
                return this;
            }
        }
    "#,
    );
    assert!(result.is_err());
}

#[test]
fn test_use_this_at_top_level_is_error() {
    let output = Vec::new();
    let mut vm = VM::new(output);
    let result = vm.interpret(
        r#"
        print this;
    "#,
    );
    assert!(result.is_err());
}

#[test]
fn test_use_this_in_function_is_error() {
    let output = Vec::new();
    let mut vm = VM::new(output);
    let result = vm.interpret(
        r#"
        fun notMethod() {
            print this;
        }
    "#,
    );
    assert!(result.is_err());
}

#[test]
fn test_invoke_field_access() {
    let output = run(r#"
        class Oops {
            init() {
                fun f() {
                    print "not a method";
                }

                this.field = f;
            }
        }
        var oops = Oops();
        var f = oops.field();
    "#);

    assert_eq!(output.trim(), "not a method");
}

#[test]
fn test_inheritance_basic() {
    let output = run(r#"
        class Doughnut {
            cook() {
                print "Dunk in the fryer.";
            }
        }

        class Cruller < Doughnut {
            finish() {
                print "Glaze with icing.";
            }
        }

        var x = Cruller();
        x.cook();
        x.finish();
  "#);

    assert_eq!(output.trim(), "Dunk in the fryer.\nGlaze with icing.");
}

#[test]
fn test_inherit_from_non_class() {
    let output = Vec::new();
    let mut vm = VM::new(output);
    let result = vm.interpret(
        r#"
        var x = "fish";
        class Doughnut < x {
            cook() {
                print "Dunk in the fryer.";
            }
        }
    "#,
    );
    assert!(result.is_err());
}

#[test]
fn test_super() {
    let output = run(r#"
        class Doughnut {
            cook() {
                print "Dunk in the fryer.";
                this.finish("sprinkles");
            }
            finish(ingredient) {
                print "Finish with " + ingredient;
            }
        }
        class Cruller < Doughnut {
            finish(ingredient) {
                super.finish(ingredient + " icing");
            }
        }
        var x = Cruller();
        x.finish("fish");
    "#);
    assert_eq!(output.trim(), "Finish with fish icing")
}

#[test]
fn test_super_out_of_class_is_error() {
    let results = run_repl_results(&[
        "var super = 1;",
        "print super;",
        "super.x = 2;",
        "super();",
        "var x = super;",
    ]);
    for res in results {
        assert!(res.is_err());
    }
}

#[test]
fn test_super_in_non_subclass_is_error() {
    let mut vm = VM::new(Vec::new());
    let result = vm.interpret(
        r#"
        class Cruller {
            finish(ingredient) {
                super.finish(ingredient + " icing");
            }
        }
        var x = Cruller();
        x.finish("fish");
    "#,
    );
    assert!(result.is_err());
}

#[test]
fn test_super_init() {
    let output = run(r#"
        class Doughnut {
            init(a) {
                this.a = a;
            }
        }
        class Cruller < Doughnut {
            init(a,b) {
                super.init(a);
                this.b = b;
            }
        }
        var x = Cruller(1,2);
        print x.a;
        print x.b;
    "#);
    assert_eq!(output.trim(), "1\n2");
}
