//! `desync-diff A.sav B.sav`: field-by-field difference of two sim saves,
//! for a multiplayer desync dump (S2 "Hash check and desync dump").
//!
//! Both saves are loaded with `World::load` and their full state is
//! walked as a serde tree, so every saved field is compared without a
//! hand-kept list. Differences are named by what they belong to:
//! `entity 412 transform.pos.x`, `squad 7 order`, `player 1 tickets`,
//! `point 2 progress`, or the global path. Q16.16 fields are shown both
//! raw and in metres. Output is sorted by category (globals, players,
//! squads, points, entities) and then by id, and capped.
//!
//! `World::load` rebuilds derived data and the hash cache, so a desync only
//! in derived data cannot show in a save diff, and two identical saves
//! always reload to identical hashes. The hashes that matter are the ones
//! the running worlds had: each dump's `manifest.json` (next to the save)
//! records `state_hash` at the pause turn. When both saves are a dump's
//! `paused.sav` and the saves match but those hashes differ, the report
//! says so; the per-tick hash ring in the manifest narrows the tick, and
//! the planned `--replay` finds the field.

use std::fmt::Write as _;
use std::path::Path;

use anyhow::{Result, anyhow};
use frontline_sim::World;
use serde_json::Value;

/// Default number of differences printed.
pub const DEFAULT_LIMIT: usize = 50;
/// Differences counted at most (the walk stops there).
const COUNT_CAP: usize = 1_000_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldDiff {
    /// Category rank (0 globals, 1 players, 2 squads, 3 points,
    /// 4 entities), for sorting.
    pub rank: u8,
    pub id: u64,
    /// Human name, e.g. `entity 412 transform.pos.x`.
    pub name: String,
    pub a: String,
    pub b: String,
}

#[derive(Clone, Debug)]
pub struct DiffReport {
    /// The first `limit` differences in report order.
    pub diffs: Vec<FieldDiff>,
    /// All differences found (capped at a million).
    pub total: usize,
    pub ticks: (u64, u64),
    /// `state_hash` after load (the cached hash recomputed from scratch on
    /// load) and `full_state_hash`.
    pub hashes: (u64, u64),
    pub full_hashes: (u64, u64),
    /// From the dumps' manifests, when both saves are a dump's
    /// `paused.sav`: the running worlds' `state_hash` at the pause turn.
    pub live_hashes: Option<(String, String)>,
    /// From the manifests: the tick whose reported hashes differed, and
    /// those hashes (slot order).
    pub desync: Option<(u64, [String; 2])>,
}

impl DiffReport {
    /// True when the saves differ in any field or hash, or the dumps'
    /// manifests say the running worlds' hashes differed.
    pub fn differs(&self) -> bool {
        self.total > 0
            || self.hashes.0 != self.hashes.1
            || self.full_hashes.0 != self.full_hashes.1
            || self.live_hashes.as_ref().is_some_and(|(a, b)| a != b)
    }

    /// Reads the `manifest.json` next to each save (a desync dump's
    /// `slot<N>/`), if both have one, and fills [`DiffReport::live_hashes`]
    /// (only for saves at the manifests' pause tick) and
    /// [`DiffReport::desync`].
    pub fn add_manifests(&mut self, a: &Path, b: &Path) {
        let read = |p: &Path| -> Option<Value> {
            let text = std::fs::read_to_string(p.parent()?.join("manifest.json")).ok()?;
            serde_json::from_str(&text).ok()
        };
        let (Some(ma), Some(mb)) = (read(a), read(b)) else {
            return;
        };
        let paused = |m: &Value, tick: u64| -> Option<String> {
            (m.get("paused_tick")?.as_u64()? == tick)
                .then(|| m.get("state_hash")?.as_str().map(str::to_owned))?
        };
        if let (Some(x), Some(y)) = (paused(&ma, self.ticks.0), paused(&mb, self.ticks.1)) {
            self.live_hashes = Some((x, y));
        }
        let desync = |m: &Value| -> Option<(u64, [String; 2])> {
            let h = m.get("hashes")?.as_array()?;
            let s = |i: usize| h.get(i)?.as_str().map(str::to_owned);
            Some((m.get("desync_tick")?.as_u64()?, [s(0)?, s(1)?]))
        };
        self.desync = desync(&ma).or_else(|| desync(&mb));
    }
}

/// Loads both saves and diffs them.
pub fn diff_saves(a: &[u8], b: &[u8], limit: usize) -> Result<DiffReport> {
    let wa = World::load(a, 1).map_err(|e| anyhow!("first save: {e}"))?;
    let wb = World::load(b, 1).map_err(|e| anyhow!("second save: {e}"))?;
    diff_worlds(&wa, &wb, limit)
}

pub fn diff_worlds(a: &World, b: &World, limit: usize) -> Result<DiffReport> {
    let va = serde_json::to_value(a.state())?;
    let vb = serde_json::to_value(b.state())?;
    let mut all = Vec::new();
    walk(&mut String::new(), &va, &vb, &mut all);
    let total = all.len();
    let mut diffs: Vec<FieldDiff> = all.into_iter().map(|(p, x, y)| name(&p, x, y)).collect();
    diffs.sort_by(|x, y| (x.rank, x.id, &x.name).cmp(&(y.rank, y.id, &y.name)));
    diffs.truncate(limit);
    Ok(DiffReport {
        diffs,
        total,
        ticks: (a.tick(), b.tick()),
        hashes: (a.state_hash(), b.state_hash()),
        full_hashes: (a.full_state_hash(), b.full_state_hash()),
        live_hashes: None,
        desync: None,
    })
}

/// Collects `(path, a, b)` for every differing leaf. Paths are dotted, with
/// `[i]` for sequence indices.
fn walk(path: &mut String, a: &Value, b: &Value, out: &mut Vec<(String, String, String)>) {
    if out.len() >= COUNT_CAP || a == b {
        return;
    }
    let len = path.len();
    match (a, b) {
        (Value::Object(ma), Value::Object(mb)) => {
            for (k, va) in ma {
                path.truncate(len);
                if !path.is_empty() {
                    path.push('.');
                }
                path.push_str(k);
                match mb.get(k) {
                    Some(vb) => walk(path, va, vb, out),
                    None => out.push((path.clone(), short(va), "(missing)".into())),
                }
            }
            for (k, vb) in mb {
                if !ma.contains_key(k) {
                    path.truncate(len);
                    if !path.is_empty() {
                        path.push('.');
                    }
                    path.push_str(k);
                    out.push((path.clone(), "(missing)".into(), short(vb)));
                }
            }
        }
        (Value::Array(xa), Value::Array(xb)) => {
            if xa.len() != xb.len() {
                out.push((
                    format!("{path}.len"),
                    xa.len().to_string(),
                    xb.len().to_string(),
                ));
            }
            for (i, (va, vb)) in xa.iter().zip(xb).enumerate() {
                path.truncate(len);
                let _ = write!(path, "[{i}]");
                walk(path, va, vb, out);
            }
        }
        _ => out.push((path.clone(), short(a), short(b))),
    }
    path.truncate(len);
}

fn short(v: &Value) -> String {
    let s = v.to_string();
    if s.len() > 80 {
        format!("{}…", &s[..s.floor_char_boundary(77)])
    } else {
        s
    }
}

/// Leaf names that hold Q16.16 values (shown in metres too).
const FX_LEAVES: &[&str] = &[
    "pos",
    "yaw",
    "lin",
    "waypoint",
    "speed",
    "height",
    "fire_height",
    "x",
    "y",
    "z",
];

/// Turns a raw path into a categorised, readable name.
fn name(path: &str, a: String, b: String) -> FieldDiff {
    // `[i]` right after a known collection becomes the id.
    let (rank, id, rest, label) = if let Some(r) = path.strip_prefix("entities.") {
        // entities.<column>[id]<rest>
        match split_index(r) {
            Some((col, id, rest)) => (4, id, format!("{col}{rest}"), format!("entity {id}")),
            None => (4, 0, r.to_owned(), "entities".to_owned()),
        }
    } else if let Some(r) = path.strip_prefix("game.players") {
        match split_index(r) {
            Some((_, id, rest)) => (1, id, trim_dot(&rest), format!("player {id}")),
            None => (1, 0, r.to_owned(), "players".to_owned()),
        }
    } else if let Some(r) = path.strip_prefix("game.points") {
        match split_index(r) {
            Some((_, id, rest)) => (3, id, trim_dot(&rest), format!("point {id}")),
            None => (3, 0, r.to_owned(), "points".to_owned()),
        }
    } else if let Some(r) = path.strip_prefix("game.squads.") {
        let (id, rest) = r.split_once('.').unwrap_or((r, ""));
        (
            2,
            id.parse().unwrap_or(0),
            rest.to_owned(),
            format!("squad {id}"),
        )
    } else {
        (0, 0, path.to_owned(), String::new())
    };
    let leaf = rest
        .rsplit(['.', '['])
        .next()
        .unwrap_or("")
        .trim_end_matches(']');
    let fx = FX_LEAVES.contains(&leaf) || rest.contains("pos.") || rest.contains("lin.");
    let show = |v: String| match (fx, v.parse::<i64>()) {
        (true, Ok(raw)) => format!("{:.3} ({raw})", raw as f64 / 65536.0),
        _ => v,
    };
    let name = match (label.is_empty(), rest.is_empty()) {
        (true, _) => rest,
        (false, true) => label,
        (false, false) => format!("{label} {rest}"),
    };
    FieldDiff {
        rank,
        id,
        name,
        a: show(a),
        b: show(b),
    }
}

fn trim_dot(s: &str) -> String {
    s.strip_prefix('.').unwrap_or(s).to_owned()
}

/// `col[12].rest` → `("col", 12, ".rest")`.
fn split_index(s: &str) -> Option<(String, u64, String)> {
    let open = s.find('[')?;
    let close = open + s[open..].find(']')?;
    let id = s[open + 1..close].parse().ok()?;
    Some((s[..open].to_owned(), id, s[close + 1..].to_owned()))
}

/// The text `desync-diff` prints.
pub fn render(r: &DiffReport) -> String {
    let mut s = String::new();
    if r.ticks.0 != r.ticks.1 {
        let _ = writeln!(
            s,
            "warning: the saves are at different ticks ({} vs {}); a desync dump's \
             paused.sav files share a tick",
            r.ticks.0, r.ticks.1
        );
    }
    if let Some((tick, [h0, h1])) = &r.desync {
        let _ = writeln!(
            s,
            "dump: hashes first differed at tick {tick} ({h0} vs {h1})"
        );
    }
    let live_differ = r.live_hashes.as_ref().filter(|(a, b)| a != b);
    if r.total == 0 {
        if let Some((a, b)) = live_differ {
            let _ = writeln!(
                s,
                "the saves match field for field, but the running worlds' state hashes \
                 at tick {} differed ({a} vs {b}, from manifest.json). World::load \
                 rebuilds derived data and the hash cache, so the divergence is in \
                 derived data or the hash cache, which a save diff cannot show. Use the \
                 per-tick hash ring in manifest.json to find the first bad tick \
                 (desync-diff --replay is a planned follow-up).",
                r.ticks.0
            );
        } else if r.hashes.0 == r.hashes.1 && r.full_hashes.0 == r.full_hashes.1 {
            let _ = writeln!(
                s,
                "saves are identical (tick {}, hash {:016x})",
                r.ticks.0, r.hashes.0
            );
        } else {
            let _ = writeln!(
                s,
                "the saves match field for field, but their hashes differ \
                 ({:016x} vs {:016x}). World::load rebuilds derived data and the hash \
                 cache, so the divergence is in derived data or the hash cache, which a \
                 save diff cannot show. Use the per-tick hash ring in manifest.json to \
                 find the first bad tick (desync-diff --replay is a planned follow-up).",
                r.hashes.0, r.hashes.1
            );
        }
        return s;
    }
    let _ = writeln!(
        s,
        "{} difference{} at tick {} (hashes {:016x} vs {:016x}){}:",
        if r.total >= COUNT_CAP {
            format!("{COUNT_CAP}+")
        } else {
            r.total.to_string()
        },
        if r.total == 1 { "" } else { "s" },
        r.ticks.0,
        r.hashes.0,
        r.hashes.1,
        if r.total > r.diffs.len() {
            format!(", first {}", r.diffs.len())
        } else {
            String::new()
        }
    );
    for d in &r.diffs {
        let _ = writeln!(s, "  {}: {} vs {}", d.name, d.a, d.b);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_saves_with_differing_manifests_are_flagged() {
        let dir = std::env::temp_dir().join(format!("desync-diff-test-{}", std::process::id()));
        let w = World::new(
            frontline_sim::SimConfig {
                soldiers: 8,
                ..frontline_sim::SimConfig::default()
            },
            1,
        )
        .unwrap();
        let save = w.save();
        let mut paths = Vec::new();
        for (slot, hash) in [(0, "00000000000000aa"), (1, "00000000000000bb")] {
            let d = dir.join(format!("slot{slot}"));
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join("paused.sav"), &save).unwrap();
            let manifest = format!(
                "{{\"paused_tick\": {}, \"state_hash\": \"{hash}\", \"desync_tick\": 0, \
                 \"hashes\": [\"1\", \"2\"]}}",
                w.tick()
            );
            std::fs::write(d.join("manifest.json"), manifest).unwrap();
            paths.push(d.join("paused.sav"));
        }
        let mut r = diff_saves(&save, &save, 10).unwrap();
        assert!(!r.differs());
        r.add_manifests(&paths[0], &paths[1]);
        assert!(r.differs());
        let text = render(&r);
        assert!(text.contains("state hashes"), "{text}");
        assert!(text.contains("hashes first differed at tick 0"), "{text}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn names_are_categorised() {
        let d = name(
            "entities.transform[412].pos.x",
            "67731456".into(),
            "67731200".into(),
        );
        assert_eq!(d.name, "entity 412 transform.pos.x");
        assert_eq!(d.rank, 4);
        assert_eq!(d.a, "1033.500 (67731456)");
        let d = name("game.squads.7.order", "1".into(), "2".into());
        assert_eq!((d.name.as_str(), d.id), ("squad 7 order", 7));
        let d = name("game.players[1].tickets", "5".into(), "6".into());
        assert_eq!(d.name, "player 1 tickets");
        let d = name("stats.commands_applied", "5".into(), "6".into());
        assert_eq!((d.name.as_str(), d.rank), ("stats.commands_applied", 0));
    }
}
