//! `validate <scenario.toml | mod dir>...`: parse, check and lower scenario
//! files and content mods, reporting every problem at once (§8.2).
//!
//! A mod folder (one holding `mod.toml`) is loaded with every def checked
//! on its own and against the others; then each rules override in its
//! `tests/rules_*.toml` is applied, and every rules def is resolved with
//! every map for the first two factions. Asset checks arrive with the mod
//! loader (M3).

use std::path::Path;

use frontline_content::{ContentError, Issue, MatchRequest, ModDefs, PlayerRequest, Scenario};
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

/// Checks a scenario file or a mod folder.
pub fn path(p: &Path) -> FileResult {
    if p.join("mod.toml").is_file() {
        mod_dir(p)
    } else {
        scenario_file(p)
    }
}

/// Checks one mod folder (see the module docs).
pub fn mod_dir(root: &Path) -> FileResult {
    let file = root.display().to_string();
    let mut problems: Vec<Problem> = Vec::new();
    match ModDefs::load(root) {
        Err(e) => problems.extend(content_problems(e, "")),
        Ok(mut defs) => {
            let mut overrides: Vec<_> = std::fs::read_dir(root.join("tests"))
                .into_iter()
                .flatten()
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| {
                    p.extension().is_some_and(|x| x == "toml")
                        && p.file_name()
                            .and_then(|n| n.to_str())
                            .is_some_and(|n| n.starts_with("rules_"))
                })
                .collect();
            overrides.sort();
            for o in &overrides {
                let name = o
                    .strip_prefix(root)
                    .unwrap_or(o)
                    .to_string_lossy()
                    .replace('\\', "/");
                if let Err(e) = defs.add_rules_override(o) {
                    problems.extend(content_problems(e, &name));
                }
            }
            let factions: Vec<String> = defs.factions.keys().take(2).cloned().collect();
            if factions.len() == 2 {
                for rules in defs.rules.keys() {
                    for map in defs.maps.keys() {
                        let req = MatchRequest {
                            rules: rules.clone(),
                            map: map.clone(),
                            seed: 1,
                            players: factions
                                .iter()
                                .map(|f| PlayerRequest {
                                    faction: f.clone(),
                                    ai: true,
                                    difficulty: Default::default(),
                                })
                                .collect(),
                        };
                        if let Err(e) = defs.resolve(&req) {
                            problems.extend(content_problems(e, &format!("{rules} on {map}")));
                        }
                    }
                }
            }
        }
    }
    FileResult {
        ok: problems.is_empty(),
        file,
        problems,
    }
}

/// Problems from a content error, each path prefixed with `origin` when
/// given.
fn content_problems(e: ContentError, origin: &str) -> Vec<Problem> {
    let prefix = |p: String| match (origin.is_empty(), p.is_empty()) {
        (true, _) => p,
        (false, true) => origin.to_owned(),
        (false, false) => format!("{origin}: {p}"),
    };
    match e {
        ContentError::Invalid { issues, .. } => issues
            .into_iter()
            .map(|i| {
                let mut p = Problem::from(i);
                p.path = prefix(p.path);
                p
            })
            .collect(),
        ContentError::Parse { origin: o, message } => vec![Problem {
            path: prefix(o),
            message,
            fix: None,
        }],
        e => vec![Problem {
            path: prefix(String::new()),
            message: e.to_string(),
            fix: None,
        }],
    }
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
