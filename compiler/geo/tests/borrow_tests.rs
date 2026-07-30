use geo::borrow;
use geo::lexer::lex;
use geo::parser::parse;
use geo::typecheck;

fn borrow_check(source: &str) -> Result<(), Vec<geo::diagnostics::Diagnostic>> {
    let tokens = lex(source).unwrap();
    let program = parse(&tokens).unwrap();
    typecheck::check(&program).unwrap();
    borrow::check(&program)
}

#[test]
fn accepts_owned_value_moved_once() {
    let source = r#"
        fn main() -> int {
            let name: string = "Geo"
            let moved: string = name
            return 0
        }
    "#;

    borrow_check(source).unwrap();
}

#[test]
fn rejects_string_use_after_move() {
    let source = r#"
        fn main() -> int {
            let name: string = "Geo"
            let moved: string = name
            let again: string = name
            return 0
        }
    "#;

    let err = borrow_check(source).unwrap_err();
    assert!(err[0].message.contains("use of moved value 'name'"));
}

#[test]
fn rejects_struct_use_after_move() {
    let source = r#"
        struct Token {
            kind: int
        }

        fn main() -> int {
            let token: Token = Token { kind: 1 }
            let moved: Token = token
            return token.kind
        }
    "#;

    let err = borrow_check(source).unwrap_err();
    assert!(err[0].message.contains("use of moved value 'token'"));
}

#[test]
fn accepts_repeated_scalar_use() {
    let source = r#"
        fn main() -> int {
            let x: int = 1
            let y: int = x
            return x + y
        }
    "#;

    borrow_check(source).unwrap();
}

#[test]
fn accepts_repeated_string_queries_without_moving_the_source() {
    let source = r#"
        import std.string

        fn scan(source: string) -> int {
            var total: int = 0
            for index in 0usize..string_len(source) {
                total += string_byte_at(source, index)
            }
            return total
        }

        fn main() -> int {
            return scan("Geo")
        }
    "#;

    borrow_check(source).unwrap();
}

#[test]
fn rejects_move_while_value_is_borrowed() {
    let source = r#"
        fn take(value: string) -> int {
            return 0
        }

        fn main() -> int {
            let name: string = "Geo"
            let borrowed: &string = &name
            return take(name)
        }
    "#;

    let err = borrow_check(source).unwrap_err();
    assert!(err[0].message.contains("cannot move borrowed value 'name'"));
}

#[test]
fn rejects_mutable_borrow_while_shared_borrow_exists() {
    let source = r#"
        fn main() -> int {
            let x: int = 1
            let shared: &int = &x
            let unique: &mut int = &mut x
            return x
        }
    "#;

    let err = borrow_check(source).unwrap_err();
    assert!(err[0]
        .message
        .contains("cannot mutably borrow 'x' while it is already borrowed"));
}

#[test]
fn rejects_borrow_return_escape() {
    let source = r#"
        fn main() -> &int {
            let x: int = 1
            return &x
        }
    "#;

    let err = borrow_check(source).unwrap_err();
    assert!(err[0].message.contains("borrow of 'x' escapes"));
}

#[test]
fn accepts_temporary_borrow_after_call_returns() {
    let source = r#"
        fn inspect(value: &int) {
        }

        fn main() -> int {
            var x: int = 1
            inspect(&x)
            x = 2
            return x
        }
    "#;

    borrow_check(source).unwrap();
}

#[test]
fn rejects_reference_local_return_escape() {
    let source = r#"
        fn main() -> &int {
            let x: int = 1
            let borrowed: &int = &x
            return borrowed
        }
    "#;

    let err = borrow_check(source).unwrap_err();
    assert!(err[0]
        .message
        .contains("borrow of 'x' escapes through reference 'borrowed'"));
}

#[test]
fn accepts_move_that_only_occurs_on_one_if_path() {
    let source = r#"
        fn main() -> int {
            let name: string = "Geo"
            if true {
                let moved: string = name
            }
            let still_available: string = name
            return 0
        }
    "#;

    borrow_check(source).unwrap();
}

#[test]
fn rejects_move_that_occurs_on_every_if_path() {
    let source = r#"
        fn main() -> int {
            let name: string = "Geo"
            if true {
                let first: string = name
            } else {
                let second: string = name
            }
            let moved_again: string = name
            return 0
        }
    "#;

    let err = borrow_check(source).unwrap_err();
    assert!(err[0].message.contains("use of moved value 'name'"));
}

#[test]
fn accepts_move_inside_a_loop_that_may_not_run() {
    let source = r#"
        fn main() -> int {
            let name: string = "Geo"
            while false {
                let moved: string = name
            }
            let still_available: string = name
            return 0
        }
    "#;

    borrow_check(source).unwrap();
}

#[test]
fn releases_reference_borrow_when_inner_scope_ends() {
    let source = r#"
        fn main() -> int {
            var value: int = 1
            if true {
                let view: &int = &value
            }
            value = 2
            return value
        }
    "#;

    borrow_check(source).unwrap();
}

#[test]
fn releases_old_borrow_when_reference_is_reassigned() {
    let source = r#"
        fn main() -> int {
            var first: int = 1
            var second: int = 2
            var view: &int = &first
            view = &second
            first = 3
            return first + second
        }
    "#;

    borrow_check(source).unwrap();
}

#[test]
fn releases_shadowed_reference_at_scope_exit() {
    let source = r#"
        fn main() -> int {
            var value: int = 1
            if true {
                let value: &int = &value
            }
            value = 2
            return value
        }
    "#;

    borrow_check(source).unwrap();
}

#[test]
fn reports_root_source_for_chained_reference_escape() {
    let source = r#"
        fn main() -> & &int {
            let value: int = 1
            let first: &int = &value
            let second: & &int = &first
            return second
        }
    "#;

    let err = borrow_check(source).unwrap_err();
    assert!(err[0]
        .message
        .contains("borrow of 'value' escapes through reference 'second'"));
}

#[test]
fn reports_root_source_for_dereference_borrow_escape() {
    let source = r#"
        fn main() -> &int {
            let value: int = 1
            let view: &int = &value
            return &*view
        }
    "#;

    let err = borrow_check(source).unwrap_err();
    assert!(err[0].message.contains("borrow of 'value' escapes"));
}

#[test]
fn releases_all_possible_branch_origins_on_reassignment() {
    let source = r#"
        fn main() -> int {
            var first: int = 1
            var second: int = 2
            var replacement: int = 3
            var view: &int = &first
            if true {
                view = &second
            }
            view = &replacement
            first = 4
            second = 5
            return replacement
        }
    "#;

    borrow_check(source).unwrap();
}
