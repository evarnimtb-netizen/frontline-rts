//! `--rules-override <file.toml>` (S3b quick iteration): partial TOML files
//! merged onto the mod's rules and defs before a run
//! ([`ModDefs::apply_override`]). `docs/modding/tuning.md` has the format;
//! `mods/ww2/tests/overrides/example.toml` is an example.

use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use frontline_content::ModDefs;

use crate::skirmish::mods_dir;

/// The rules id every override patches: the skirmish rules. The test rules
/// the armour cases use (`ww2:skirmish_test_unlock`) are derived from it
/// after the overrides, so they see the change too.
pub const BASE_RULES: &str = "ww2:skirmish";

/// Loads the mod (`--mods`, else the default) and applies `overrides` in
/// order. Returns the defs and one log line per changed value.
pub fn load_defs(mods: Option<&Path>, overrides: &[PathBuf]) -> Result<(ModDefs, Vec<String>)> {
    let dir = mods_dir(mods)?;
    let mut defs = ModDefs::load(&dir).map_err(|e| anyhow!("{e}"))?;
    let mut log = Vec::new();
    for path in overrides {
        let changes = defs
            .apply_override(path, BASE_RULES)
            .map_err(|e| anyhow!("--rules-override {e}"))?;
        if changes.is_empty() {
            log.push(format!("override {}: no value changed", path.display()));
        }
        for c in changes {
            log.push(format!("override {}: {c}", path.display()));
        }
    }
    Ok((defs, log))
}

/// Checks the overrides once and prints what they change to stderr (the
/// run's own output stays on stdout, unchanged in format).
pub fn announce(mods: Option<&Path>, overrides: &[PathBuf]) -> Result<()> {
    if overrides.is_empty() {
        return Ok(());
    }
    let (_, log) = load_defs(mods, overrides)?;
    for line in log {
        eprintln!("{line}");
    }
    Ok(())
}
