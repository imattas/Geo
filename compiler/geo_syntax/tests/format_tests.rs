use geo_syntax::format::format_program;
use geo_syntax::lexer::lex;
use geo_syntax::parser::parse;

fn format(source: &str) -> String {
    let tokens = lex(source).expect("source should lex");
    let program = parse(&tokens).expect("source should parse");
    format_program(&program)
}

#[test]
fn formats_declarations_and_nested_blocks() {
    let formatted = format(
        r#"import std.io
fn main(){let value:int=1+2*3;if value>0{println("ok")}else{println("no")}}"#,
    );

    assert_eq!(
        formatted,
        "import std.io\n\nfn main() {\n    let value: int = 1 + 2 * 3\n    if value > 0 {\n        println(\"ok\")\n    } else {\n        println(\"no\")\n    }\n}\n"
    );
}

#[test]
fn formatter_is_idempotent_for_supported_syntax() {
    let source = "fn main() -> int { let value: int = (1 + 2) * 3; return value }";
    let once = format(source);
    let twice = format(&once);
    assert_eq!(once, twice);
}

#[test]
fn formats_literals_and_types_without_losing_values() {
    let formatted = format(
        r#"struct Pair{left:i32,right:*u8}
enum State{Ready=1,Done}
fn make()->Pair{let pair=Pair{left:-4,right:null};return pair}"#,
    );

    assert!(formatted.contains("struct Pair {\n    left: i32\n    right: *u8\n}"));
    assert!(formatted.contains("enum State {\n    Ready = 1,\n    Done,\n}"));
    assert!(
        formatted.contains("Pair { left: -4, right: null }")
            || formatted.contains("Pair { left: -4, right: null }")
    );
}
