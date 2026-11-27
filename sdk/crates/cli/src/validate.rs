//! `validate <scenario.toml | mod dir>...`: parse, check and lower scenario
//! files and content mods, reporting every problem at once (§8.2).
//!
//! A mod folder (one holding `mod.toml`) is loaded with every def checked
//! on its own and against the others; then each rules override in its
//! `tests/rules_*.toml` is applied, each tuning override in
//! `tests/overrides/*.toml` is applied to a copy (S3b), S4 example variants
//! (`tests/variants/<kind>/*.toml` plus the override files beside them) load
//! on a copy, and every rules def is resolved with every map for the first
//! two factions, with and without the variants. Asset checks arrive with the mod
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
    /// S16a (R5): findings that do not fail the file (a point-symmetric
    /// map's hand-placed items without a mirror).
    pub warnings: Vec<Problem>,
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
    let mut warnings: Vec<Problem> = Vec::new();
    match ModDefs::load(root) {
        Err(e) => problems.extend(content_problems(e, "")),
        Ok(mut defs) => {
            warnings = map_warnings(&defs);
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
            // S3b tuning overrides (tests/overrides/*.toml) must apply too.
            let mut patches: Vec<_> = std::fs::read_dir(root.join("tests/overrides"))
                .into_iter()
                .flatten()
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|x| x == "toml"))
                .collect();
            patches.sort();
            for o in &patches {
                let name = o
                    .strip_prefix(root)
                    .unwrap_or(o)
                    .to_string_lossy()
                    .replace('\\', "/");
                if let Err(e) = defs.clone().apply_override(o, crate::overrides::BASE_RULES) {
                    problems.extend(content_problems(e, &name));
                }
            }
            // S4: example variants (tests/variants/<kind>/*.toml) load on
            // top of the mod, with their override files (tests/variants/
            // *.toml), and resolve like the mod.
            let variants = root.join("tests/variants");
            let mut checked = vec![defs.clone()];
            if variants.is_dir() {
                let mut with = defs.clone();
                match with.add_defs_dir(&variants) {
                    Err(e) => problems.extend(content_problems(e, "tests/variants")),
                    Ok(_) => {
                        let mut files: Vec<_> = std::fs::read_dir(&variants)
                            .into_iter()
                            .flatten()
                            .filter_map(|e| e.ok().map(|e| e.path()))
                            .filter(|p| p.extension().is_some_and(|x| x == "toml"))
                            .collect();
                        files.sort();
                        for o in &files {
                            let name = o
                                .strip_prefix(root)
                                .unwrap_or(o)
                                .to_string_lossy()
                                .replace('\\', "/");
                            if let Err(e) = with.apply_override(o, crate::overrides::BASE_RULES) {
                                problems.extend(content_problems(e, &name));
                            }
                        }
                        checked.push(with);
                    }
                }
            }
            for defs in &checked {
                resolve_all(defs, &mut problems);
            }
        }
    }
    FileResult {
        ok: problems.is_empty(),
        file,
        problems,
        warnings,
    }
}

/// S16a (R5): the mirror warnings of every map (`MapFileDef::mirror_warnings`;
/// S17a: and `side_warnings`), each path prefixed with the map id.
pub fn map_warnings(defs: &ModDefs) -> Vec<Problem> {
    defs.maps
        .iter()
        .flat_map(|(id, m)| {
            let mut w = m.mirror_warnings();
            // S17a: the bases' `side` values.
            w.extend(m.side_warnings());
            w.into_iter().map(move |i| {
                let mut p = Problem::from(i);
                p.path = format!("maps.{id}.{}", p.path);
                p
            })
        })
        .collect()
}

/// S15 M (docs/slices/S15-ussr.md 4): the faction pairs rules `r` lets
/// fight: its `[matchups] pairs` (with `free` 1, every two enabled
/// factions); without the table or pairs, the first two factions.
pub fn allowed_pairs(defs: &ModDefs, r: &frontline_content::RulesDef) -> Vec<[String; 2]> {
    let all: Vec<String> = defs.factions.keys().cloned().collect();
    match &r.matchups {
        Some(m) if m.free == 1 => {
            let on: Vec<&String> = all.iter().filter(|f| m.enabled(f)).collect();
            let mut out = Vec::new();
            for (i, a) in on.iter().enumerate() {
                for b in &on[i + 1..] {
                    out.push([(*a).clone(), (*b).clone()]);
                }
            }
            out
        }
        Some(m) if !m.pairs.is_empty() => m.pairs.clone(),
        _ if all.len() >= 2 => vec![[all[0].clone(), all[1].clone()]],
        _ => Vec::new(),
    }
}

/// Resolves every rules def with every map for every allowed pair.
fn resolve_all(defs: &ModDefs, problems: &mut Vec<Problem>) {
    for (rid, rules) in &defs.rules {
        for pair in allowed_pairs(defs, rules) {
            for map in defs.maps.keys() {
                let req = MatchRequest {
                    rules: rid.clone(),
                    map: map.clone(),
                    seed: 1,
                    players: pair
                        .iter()
                        .map(|f| PlayerRequest {
                            faction: f.clone(),
                            ai: true,
                            difficulty: Default::default(),
                            team: None,
                        })
                        .collect(),
                };
                if let Err(e) = defs.resolve(&req) {
                    let at = format!("{rid} on {map} ({} vs {})", pair[0], pair[1]);
                    problems.extend(content_problems(e, &at));
                }
            }
        }
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
        warnings: Vec::new(),
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

#[cfg(test)]
mod s15_tests {
    use super::*;

    /// S15 M (plan 4 tools): `validate` resolves every allowed pair, not
    /// "the first two factions". S15-ai: the shipped table holds
    /// Germany-USA and Germany-USSR (the real USSR defs), and both resolve.
    /// Control: `free` 1 adds the USA-USSR pair.
    #[test]
    fn validate_resolves_every_pair() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../mods/ww2");
        let mut defs = ModDefs::load(&root).unwrap();
        let r = &defs.rules["ww2:skirmish"];
        let pair = |a: &str, b: &str| [a.to_string(), b.to_string()];
        assert_eq!(
            allowed_pairs(&defs, r),
            [
                pair("ww2:germany", "ww2:usa"),
                pair("ww2:germany", "ww2:ussr")
            ]
        );
        let mut problems = Vec::new();
        resolve_all(&defs, &mut problems);
        assert!(problems.is_empty(), "{problems:?}");
        defs.rules
            .get_mut("ww2:skirmish")
            .unwrap()
            .matchups
            .as_mut()
            .unwrap()
            .free = 1;
        assert_eq!(allowed_pairs(&defs, &defs.rules["ww2:skirmish"]).len(), 3);
    }
}
