use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutKind {
    Directory,
    File,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutEntry {
    pub path: PathBuf,
    pub kind: LayoutKind,
    pub purpose: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutReport {
    pub missing: Vec<LayoutEntry>,
}

impl LayoutReport {
    pub fn is_ok(&self) -> bool {
        self.missing.is_empty()
    }
}

pub fn required_layout() -> Vec<LayoutEntry> {
    vec![
        dir("compiler", "compiler implementation area"),
        dir("compiler/geo", "current Geo compiler and CLI crate"),
        dir("compiler/geo/src", "compiler source modules"),
        dir("compiler/geo/tests", "compiler boundary tests"),
        dir("compiler/geo_layout", "workspace layout validation crate"),
        dir("library", "language libraries and runtime area"),
        dir(
            "library/geo_runtime",
            "compiler-managed native runtime implementation",
        ),
        dir("library/std", "Geo standard library source package"),
        dir("library/std/src", "standard library Geo modules"),
        dir(
            "library/std/src/platform",
            "target-specific standard library modules",
        ),
        dir("src", "repository-level bootstrap and tool area"),
        dir("src/bootstrap", "bootstrap stage model"),
        dir("src/tools", "developer tools"),
        dir("src/tools/xtask", "workspace automation tool"),
        dir("examples", "Geo source examples"),
        dir("docs", "project design and planning docs"),
        file("Cargo.toml", "workspace manifest"),
        file("README.md", "project overview"),
        file("STATUS.md", "current project status"),
        file("ROADMAP.md", "project roadmap"),
        file("IMPROVEMENTS.md", "candidate improvements"),
    ]
}

pub fn validate_workspace(root: &Path) -> LayoutReport {
    let missing = required_layout()
        .into_iter()
        .filter(|entry| {
            let path = root.join(&entry.path);
            match entry.kind {
                LayoutKind::Directory => !path.is_dir(),
                LayoutKind::File => !path.is_file(),
            }
        })
        .collect();

    LayoutReport { missing }
}

fn dir(path: &'static str, purpose: &'static str) -> LayoutEntry {
    LayoutEntry {
        path: PathBuf::from(path),
        kind: LayoutKind::Directory,
        purpose,
    }
}

fn file(path: &'static str, purpose: &'static str) -> LayoutEntry {
    LayoutEntry {
        path: PathBuf::from(path),
        kind: LayoutKind::File,
        purpose,
    }
}
