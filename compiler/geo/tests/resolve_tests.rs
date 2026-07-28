use geo::ast::{Expr, Stmt};
use geo::resolve::load_package_entry;

#[test]
fn loads_imported_module_functions() {
    let dir = std::env::temp_dir().join(format!("geo-resolve-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("failed to create resolver fixture directory");
    let main = dir.join("main.geo");
    std::fs::write(
        &main,
        r#"
            import math

            fn main() -> int {
                return forty_two()
            }
        "#,
    )
    .expect("failed to write main fixture");
    std::fs::write(
        dir.join("math.geo"),
        r#"
            fn forty_two() -> int {
                return 42
            }
        "#,
    )
    .expect("failed to write math fixture");

    let program = load_package_entry(&main).expect("failed to load package entry");
    let _ = std::fs::remove_dir_all(&dir);

    assert!(program
        .functions
        .iter()
        .any(|function| function.name == "main"));
    assert!(program
        .functions
        .iter()
        .any(|function| function.name == "forty_two"));
}

#[test]
fn rejects_circular_imports() {
    let dir = std::env::temp_dir().join(format!("geo-cycle-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("failed to create cycle fixture directory");
    let a = dir.join("a.geo");
    std::fs::write(
        &a,
        r#"
            import b

            fn main() -> int {
                return b_value()
            }
        "#,
    )
    .expect("failed to write a fixture");
    std::fs::write(
        dir.join("b.geo"),
        r#"
            import a

            fn b_value() -> int {
                return 1
            }
        "#,
    )
    .expect("failed to write b fixture");

    let err = load_package_entry(&a).unwrap_err();
    let _ = std::fs::remove_dir_all(&dir);

    assert!(err
        .iter()
        .any(|diagnostic| diagnostic.message.contains("circular import")));
}

#[test]
fn loads_directory_module_entry_and_imported_structs() {
    let dir = std::env::temp_dir().join(format!("geo-dir-module-{}", std::process::id()));
    let model_dir = dir.join("model");
    std::fs::create_dir_all(&model_dir).expect("failed to create directory module fixture");
    let main = dir.join("main.geo");
    std::fs::write(
        &main,
        r#"
            import model

            fn main() -> int {
                let token: Token = Token { kind: 7 }
                return token.kind
            }
        "#,
    )
    .expect("failed to write main fixture");
    std::fs::write(
        model_dir.join("mod.geo"),
        r#"
            struct Token {
                kind: int
            }
        "#,
    )
    .expect("failed to write directory module fixture");

    let program = load_package_entry(&main).expect("failed to load package entry");
    let _ = std::fs::remove_dir_all(&dir);

    assert!(program
        .structs
        .iter()
        .any(|struct_decl| struct_decl.name == "Token"));
    assert!(program
        .functions
        .iter()
        .any(|function| function.name == "main"));
}

#[test]
fn resolves_qualified_imported_struct_types() {
    let dir = std::env::temp_dir().join(format!("geo-qualified-type-{}", std::process::id()));
    let model_dir = dir.join("model");
    std::fs::create_dir_all(&model_dir).expect("failed to create qualified type fixture");
    let main = dir.join("main.geo");
    std::fs::write(
        &main,
        r#"
            import model

            fn main() -> int {
                let token: model.Token = model.Token { kind: 7 }
                return token.kind
            }
        "#,
    )
    .expect("failed to write qualified type main fixture");
    std::fs::write(
        model_dir.join("mod.geo"),
        r#"
            struct Token {
                kind: int
            }
        "#,
    )
    .expect("failed to write qualified type model fixture");

    let program = load_package_entry(&main).expect("failed to load package entry");
    let _ = std::fs::remove_dir_all(&dir);

    assert!(program
        .structs
        .iter()
        .any(|struct_decl| struct_decl.name == "Token"));
}

#[test]
fn resolves_qualified_imported_enum_variants() {
    let dir = std::env::temp_dir().join(format!("geo-qualified-enum-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("failed to create qualified enum fixture");
    let main = dir.join("main.geo");
    std::fs::write(
        &main,
        r#"
            import model

            fn main() -> int {
                let kind: model.TokenKind = model.TokenKind.Number
                return match kind {
                    model.TokenKind.Eof => 0
                    model.TokenKind.Number => 42
                }
            }
        "#,
    )
    .expect("failed to write qualified enum main fixture");
    std::fs::write(
        dir.join("model.geo"),
        r#"
            enum TokenKind {
                Eof
                Number
            }
        "#,
    )
    .expect("failed to write qualified enum model fixture");

    let program = load_package_entry(&main).expect("failed to load package entry");
    let _ = std::fs::remove_dir_all(&dir);

    assert!(program
        .enums
        .iter()
        .any(|enum_decl| enum_decl.name == "TokenKind"));
}

#[test]
fn resolves_qualified_imported_constants() {
    let dir = std::env::temp_dir().join(format!("geo-qualified-const-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("failed to create qualified const fixture");
    let main = dir.join("main.geo");
    std::fs::write(
        &main,
        r#"
            import config

            fn main() -> int {
                return config.LIMIT
            }
        "#,
    )
    .expect("failed to write qualified const main fixture");
    std::fs::write(
        dir.join("config.geo"),
        r#"
            const LIMIT: int = 42
        "#,
    )
    .expect("failed to write qualified const module fixture");

    let program = load_package_entry(&main).expect("failed to load package entry");
    let _ = std::fs::remove_dir_all(&dir);

    assert!(program
        .consts
        .iter()
        .any(|const_decl| const_decl.name == "LIMIT"));
    let main = program
        .functions
        .iter()
        .find(|function| function.name == "main")
        .expect("main should be present");
    assert!(matches!(
        &main.body[0],
        Stmt::Return(Some(Expr::Var(name))) if name == "LIMIT"
    ));
}

#[test]
fn resolves_aliased_imported_names() {
    let dir = std::env::temp_dir().join(format!("geo-aliased-import-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("failed to create aliased import fixture");
    let main = dir.join("main.geo");
    std::fs::write(
        &main,
        r#"
            import model as m

            fn main() -> int {
                let kind: m.TokenKind = m.TokenKind.Number
                return match kind {
                    m.TokenKind.Eof => 0
                    m.TokenKind.Number => m.score()
                }
            }
        "#,
    )
    .expect("failed to write aliased import main fixture");
    std::fs::write(
        dir.join("model.geo"),
        r#"
            enum TokenKind {
                Eof
                Number
            }

            fn score() -> int {
                return 42
            }
        "#,
    )
    .expect("failed to write aliased import model fixture");

    let program = load_package_entry(&main).expect("failed to load package entry");
    let _ = std::fs::remove_dir_all(&dir);

    assert!(program
        .enums
        .iter()
        .any(|enum_decl| enum_decl.name == "TokenKind"));
    assert!(program
        .functions
        .iter()
        .any(|function| function.name == "score"));
}

#[test]
fn loads_imported_type_aliases() {
    let dir = std::env::temp_dir().join(format!("geo-alias-module-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("failed to create alias module fixture");
    let main = dir.join("main.geo");
    std::fs::write(
        &main,
        r#"
            import model

            fn main() -> Byte {
                return 255
            }
        "#,
    )
    .expect("failed to write alias main fixture");
    std::fs::write(
        dir.join("model.geo"),
        r#"
            type Byte = u8
        "#,
    )
    .expect("failed to write alias module fixture");

    let program = load_package_entry(&main).expect("failed to load package entry");
    let _ = std::fs::remove_dir_all(&dir);

    assert!(program
        .type_aliases
        .iter()
        .any(|alias| alias.name == "Byte"));
}

#[test]
fn qualifies_imported_module_functions_for_calls() {
    let dir = std::env::temp_dir().join(format!("geo-qualified-call-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("failed to create qualified call fixture");
    let main = dir.join("main.geo");
    std::fs::write(
        &main,
        r#"
            import math

            fn main() -> int {
                return math.forty_two()
            }
        "#,
    )
    .expect("failed to write qualified call main fixture");
    std::fs::write(
        dir.join("math.geo"),
        r#"
            fn forty_two() -> int {
                return 42
            }
        "#,
    )
    .expect("failed to write qualified call module fixture");

    let program = load_package_entry(&main).expect("failed to load package entry");
    let _ = std::fs::remove_dir_all(&dir);

    assert!(program
        .functions
        .iter()
        .any(|function| function.name == "forty_two"));
}
