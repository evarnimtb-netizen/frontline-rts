//! `validate <scenario.toml>...`: parse, check and lower scenario files,
//! reporting every problem at once (§8.2).
//!
//! Mod folders, reference resolution and asset checks arrive with the mod
//! loader (M3); for now this covers scenario files.

use std::path::Path;

use frontline_content::{ContentError, Issue, Scenario};
use serde::Serialize;

use crate::lower;

/// Result for one file, also the `--json` output shape.
#[derive(Clone, Debug, Serialize)]
pub struct FileResult {
    pub file: String,
    pub ok: bool,
    pub problems: Vec<Problem>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Problem {
    /// Dotted field path; empty for syntax and I/O errors.
    pub path: String,
    pub message: String,
    pub fix: Option<String>,
}

/// Checks one scenario file: syntax, content rules, and whether the current
/// sim can run it (at the scenario's own seed).
pub fn scenario_file(path: &Path) -> FileResult {
    let file = path.display().to_string();
    let problems = match std::fs::read_to_string(path) {
        Err(e) => vec![Problem::bare(e.to_string())],
        Ok(src) => match Scenario::parse_toml(&src, &file) {
            Ok(s) => lower::check(&s, s.seed)
                .into_iter()
                .map(Problem::from)
                .collect(),
            Err(ContentError::Parse { message, .. }) => vec![Problem::bare(message)],
            Err(e) => vec![Problem::bare(e.to_string())],
        },
    };
    FileResult {
        ok: problems.is_empty(),
        file,
        problems,
    }
}

impl Problem {
    fn bare(message: String) -> Problem {
        Problem {
            path: String::new(),
            message,
            fix: None,
        }
    }
}

impl From<Issue> for Problem {
    fn from(i: Issue) -> Problem {
        Problem {
            path: i.path,
            message: i.message,
            fix: i.hint,
        }
    }
}
