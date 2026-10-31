//! `schema --out <dir>`: write a JSON Schema per def type.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Writes `<dir>/<name>.schema.json` for every def type (creating `dir`) and
/// returns the paths written, in a stable order.
pub fn export(dir: &Path) -> Result<Vec<PathBuf>> {
    fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    let mut written = Vec::new();
    for doc in frontline_content::schemas() {
        let path = dir.join(format!("{}.schema.json", doc.name));
        let mut json = serde_json::to_string_pretty(&doc.schema)?;
        json.push('\n');
        fs::write(&path, json).with_context(|| format!("writing {}", path.display()))?;
        written.push(path);
    }
    Ok(written)
}
