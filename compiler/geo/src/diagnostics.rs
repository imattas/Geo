use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
}

impl Severity {
    fn label(self) -> &'static str {
        match self {
            Severity::Error => "error",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLocation {
    pub path: PathBuf,
    pub line: usize,
    pub column: usize,
    pub line_text: String,
    pub underline_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub source: Option<SourceLocation>,
    pub notes: Vec<String>,
}

impl Diagnostic {
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            message: message.into(),
            source: None,
            notes: Vec::new(),
        }
    }

    pub fn with_source(mut self, source: SourceLocation) -> Self {
        self.source = Some(source);
        self
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    pub fn render(&self) -> String {
        let mut out = format!("{}: {}", self.severity.label(), self.message);

        if let Some(source) = &self.source {
            out.push('\n');
            out.push_str(&format!(
                " --> {}:{}:{}\n",
                source.path.display(),
                source.line,
                source.column
            ));
            out.push_str("  |\n");
            out.push_str(&format!("{} | {}\n", source.line, source.line_text));
            out.push_str("  | ");
            out.push_str(&" ".repeat(source.column.saturating_sub(1)));
            out.push_str(&"^".repeat(source.underline_len.max(1)));
        }

        for note in &self.notes {
            out.push('\n');
            out.push_str(&format!("note: {note}"));
        }

        out
    }
}
