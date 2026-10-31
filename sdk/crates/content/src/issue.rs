//! Error and validation-issue types shared by all def kinds.

use std::fmt;
use std::path::PathBuf;

/// One validation problem in a def file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Issue {
    /// Dotted path of the offending field, e.g. `map.size_m`.
    pub path: String,
    /// What is wrong, including the expected type or range.
    pub message: String,
    /// A concrete fix, when one can be suggested.
    pub hint: Option<String>,
}

impl Issue {
    pub fn new(path: impl Into<String>, message: impl Into<String>) -> Issue {
        Issue {
            path: path.into(),
            message: message.into(),
            hint: None,
        }
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Issue {
        self.hint = Some(hint.into());
        self
    }
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path, self.message)?;
        if let Some(h) = &self.hint {
            write!(f, " (fix: {h})")?;
        }
        Ok(())
    }
}

/// Failure to load a def file.
#[derive(Debug, thiserror::Error)]
pub enum ContentError {
    #[error("{}: {source}", path.display())]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    /// Syntax or type error. The message carries the line, column and a
    /// source excerpt from the TOML parser.
    #[error("{origin}: {message}")]
    Parse { origin: String, message: String },
    #[error("{origin}: {} problem(s)\n{}", issues.len(), list(issues))]
    Invalid { origin: String, issues: Vec<Issue> },
}

fn list(issues: &[Issue]) -> String {
    issues
        .iter()
        .map(|i| format!("  - {i}"))
        .collect::<Vec<_>>()
        .join("\n")
}
