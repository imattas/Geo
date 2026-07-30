fn workspace_path(path: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(path)
}

#[test]
fn cli_emit_obj_writes_elf64_relocatable_without_external_assembler() {
    let input = workspace_path("examples/return_42.geo");
    let output = std::env::temp_dir().join(format!("geo-return-42-{}.o", std::process::id()));

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args([
            "emit-obj",
            input.to_string_lossy().as_ref(),
            "-o",
            output.to_string_lossy().as_ref(),
            "--target",
            "x86_64-linux",
        ])
        .status()
        .expect("failed to run geo emit-obj");

    let bytes = std::fs::read(&output).unwrap_or_default();
    let _ = std::fs::remove_file(&output);

    assert!(status.success());
    assert_eq!(&bytes[0..4], b"\x7fELF");
    assert_eq!(read_u16(&bytes, 16), 1);
    assert!(contains_bytes(&bytes, b".symtab"));
    assert!(contains_bytes(&bytes, b"main"));
}

#[test]
fn cli_emit_obj_writes_win64_coff_relocatable_without_external_assembler() {
    let input = workspace_path("examples/return_42.geo");
    let output = std::env::temp_dir().join(format!("geo-return-42-{}.obj", std::process::id()));

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args([
            "emit-obj",
            input.to_string_lossy().as_ref(),
            "-o",
            output.to_string_lossy().as_ref(),
            "--target",
            "x86_64-windows",
        ])
        .status()
        .expect("failed to run geo emit-obj");
    let bytes = std::fs::read(&output).unwrap_or_default();
    let _ = std::fs::remove_file(&output);

    assert!(status.success());
    assert_eq!(read_u16(&bytes, 0), 0x8664);
    assert_eq!(read_u16(&bytes, 2), 1);
    assert!(contains_bytes(&bytes, b".text"));
    assert!(contains_bytes(&bytes, b"main"));
    assert!(contains_bytes(
        &bytes,
        &[0x48, 0xb8, 42, 0, 0, 0, 0, 0, 0, 0]
    ));
    assert!(contains_bytes(&bytes, &[0xc9, 0xc3]));
}

#[test]
fn cli_build_writes_compiler_owned_linux_executable_subset() {
    let input = workspace_path("examples/return_42.geo");
    let output = std::env::temp_dir().join(format!("geo-return-42-{}.elf", std::process::id()));

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args([
            "build",
            input.to_string_lossy().as_ref(),
            "-o",
            output.to_string_lossy().as_ref(),
            "--target",
            "x86_64-linux",
        ])
        .status()
        .expect("failed to run geo build");
    let bytes = std::fs::read(&output).unwrap_or_default();
    let _ = std::fs::remove_file(&output);

    assert!(status.success());
    assert_eq!(&bytes[0..4], b"\x7fELF");
    assert_eq!(read_u16(&bytes, 16), 2);
    assert!(contains_bytes(
        &bytes,
        &[0x89, 0xc7, 0xb8, 60, 0, 0, 0, 0x0f, 0x05]
    ));
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}
