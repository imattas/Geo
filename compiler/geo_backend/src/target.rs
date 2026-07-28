use crate::diagnostics::Diagnostic;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetTriple {
    X86_64Linux,
    X86_64Windows,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Abi {
    SystemV,
    WindowsX64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectFormat {
    Elf64,
    Win64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub triple: TargetTriple,
    pub abi: Abi,
    pub object_format: ObjectFormat,
    pub nasm_format: &'static str,
    pub executable_extension: &'static str,
    pub default_linker: &'static str,
}

impl Target {
    pub fn host() -> Self {
        if cfg!(target_os = "windows") {
            Self::windows_x86_64()
        } else {
            Self::linux_x86_64()
        }
    }

    pub fn parse(value: &str) -> Result<Self, Diagnostic> {
        match value {
            "x86_64-linux" => Ok(Self::linux_x86_64()),
            "x86_64-windows" => Ok(Self::windows_x86_64()),
            other => Err(Diagnostic::error(format!(
                "unsupported target '{other}'; supported targets are x86_64-linux and x86_64-windows"
            ))),
        }
    }

    pub fn linux_x86_64() -> Self {
        Self {
            triple: TargetTriple::X86_64Linux,
            abi: Abi::SystemV,
            object_format: ObjectFormat::Elf64,
            nasm_format: "elf64",
            executable_extension: "",
            default_linker: "gcc",
        }
    }

    pub fn windows_x86_64() -> Self {
        Self {
            triple: TargetTriple::X86_64Windows,
            abi: Abi::WindowsX64,
            object_format: ObjectFormat::Win64,
            nasm_format: "win64",
            executable_extension: "exe",
            default_linker: "gcc",
        }
    }
}
