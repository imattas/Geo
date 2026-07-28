use geo::target::{ObjectFormat, Target, TargetTriple};

#[test]
fn parses_linux_target() {
    let target = Target::parse("x86_64-linux").unwrap();
    assert_eq!(target.triple, TargetTriple::X86_64Linux);
    assert_eq!(target.object_format, ObjectFormat::Elf64);
    assert_eq!(target.nasm_format, "elf64");
    assert_eq!(target.default_linker, "gcc");
}

#[test]
fn parses_windows_target() {
    let target = Target::parse("x86_64-windows").unwrap();
    assert_eq!(target.triple, TargetTriple::X86_64Windows);
    assert_eq!(target.object_format, ObjectFormat::Win64);
    assert_eq!(target.nasm_format, "win64");
    assert_eq!(target.executable_extension, "exe");
}

#[test]
fn rejects_unknown_target() {
    let err = Target::parse("wasm32-browser").unwrap_err();
    assert!(err.message.contains("unsupported target"));
}
