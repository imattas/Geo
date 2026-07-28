#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageKind {
    HostCompiler,
    Runtime,
    StandardLibrary,
    Examples,
    Distribution,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapStage {
    pub name: &'static str,
    pub kind: StageKind,
    pub path: &'static str,
    pub description: &'static str,
}

pub fn bootstrap_stages() -> Vec<BootstrapStage> {
    vec![
        BootstrapStage {
            name: "host-compiler",
            kind: StageKind::HostCompiler,
            path: "compiler/geo",
            description: "build the Rust implementation of the Geo compiler",
        },
        BootstrapStage {
            name: "runtime",
            kind: StageKind::Runtime,
            path: "library/geo_runtime",
            description: "compile and link compiler-managed native runtime symbols",
        },
        BootstrapStage {
            name: "std",
            kind: StageKind::StandardLibrary,
            path: "library/std",
            description: "check and package source-level standard library modules",
        },
        BootstrapStage {
            name: "self-hosting-examples",
            kind: StageKind::Examples,
            path: "examples/v1",
            description: "compile compiler-shaped Geo examples for supported targets",
        },
        BootstrapStage {
            name: "dist",
            kind: StageKind::Distribution,
            path: "target/dist",
            description:
                "assemble release artifacts for the compiler, runtime, and standard library",
        },
    ]
}
