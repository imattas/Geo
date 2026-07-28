use crate::diagnostics::{Diagnostic, SourceLocation};
use std::fs;
use std::path::{Path, PathBuf};

pub fn module_path_to_file(root: &Path, path: &[String]) -> PathBuf {
    let mut file = root.to_path_buf();
    for segment in path {
        file.push(segment);
    }
    file.set_extension("geo");
    if file.exists() {
        return file;
    }

    let mut dir_module = root.to_path_buf();
    for segment in path {
        dir_module.push(segment);
    }
    dir_module.push("mod.geo");
    if dir_module.exists() {
        return dir_module;
    }

    file
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    pub path: PathBuf,
    pub text: String,
}

impl SourceFile {
    pub fn load(path: &Path) -> Result<Self, Vec<Diagnostic>> {
        if path.extension().and_then(|ext| ext.to_str()) != Some("geo") {
            return Err(vec![Diagnostic::error(
                "Geo source files must use the .geo extension",
            )]);
        }

        let text = fs::read_to_string(path).map_err(|err| {
            vec![Diagnostic::error(format!(
                "failed to read source file: {err}"
            ))]
        })?;

        Ok(Self {
            path: path.to_path_buf(),
            text,
        })
    }

    pub fn location(&self, offset: usize, len: usize) -> SourceLocation {
        let mut line_start = 0;
        let mut line = 1;

        for (idx, ch) in self.text.char_indices() {
            if idx >= offset {
                break;
            }
            if ch == '\n' {
                line += 1;
                line_start = idx + 1;
            }
        }

        let line_end = self.text[line_start..]
            .find('\n')
            .map(|relative| line_start + relative)
            .unwrap_or(self.text.len());
        let column = self.text[line_start..offset.min(self.text.len())]
            .chars()
            .count()
            + 1;

        SourceLocation {
            path: self.path.clone(),
            line,
            column,
            line_text: self.text[line_start..line_end].to_string(),
            underline_len: len.max(1),
        }
    }
}
