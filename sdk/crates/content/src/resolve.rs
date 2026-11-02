//! S4 `extends`: a def file may name another def of the same kind as its
//! parent and give only what differs (`docs/modding/variants.md`).
//!
//! Every def file is first read into a raw TOML table ([`RawDefs`]); a def
//! resolves by merging its chain from the root down:
//!
//! * the chain is at most [`MAX_EXTENDS_DEPTH`] parents long; an unknown
//!   parent (with did-you-mean) or a cycle is an error on the child;
//! * tables merge key by key, arrays and scalars replace; `id` and
//!   `abstract` are never inherited and `extends` is dropped;
//! * units: a level that sets `[[members]]` drops the inherited legacy
//!   soldier keys (`men`, `weapon`, `weapon_men`, `sidearm`, `loadout`), and
//!   a level that sets a legacy soldier key over a parent with `[[members]]`
//!   is an error; a level that sets one name of an aliased card key
//!   (`recall_cooldown_s` / `cooldown_s`, `initial_cooldown_s` / `unlock_s`)
//!   drops the other name inherited from its parents, and one file setting
//!   both is an error.
//!
//! The merged table is then read into the def struct (unknown keys stay
//! errors) and validated as a whole by `ModDefs`; issues name the child's
//! file and, for a value a parent set, "(from `<parent id>`)". Tuning
//! overrides patch the raw tables and every def resolves again, so a patch
//! on a parent reaches its variants.

use std::collections::BTreeMap;

use crate::issue::Issue;
use crate::modset::did_you_mean;

/// Longest `extends` chain (parents, the def itself not counted).
pub const MAX_EXTENDS_DEPTH: usize = 4;

/// Def kinds, by their folder under `defs/` (also the `[defs.<kind>]` names
/// of a tuning override).
pub const DEF_KINDS: [&str; 9] = [
    "units", "weapons", "items", "factions", "maps", "rules", "vehicles", "ammo", "training",
];

/// Legacy soldier keys of a unit (replaced by `[[members]]`).
pub const LEGACY_SOLDIER_KEYS: [&str; 5] = ["men", "weapon", "weapon_men", "sidearm", "loadout"];

/// Aliased unit card keys: (legacy name, S4 name).
pub const CARD_ALIASES: [(&str, &str); 2] = [
    ("recall_cooldown_s", "cooldown_s"),
    ("initial_cooldown_s", "unlock_s"),
];

/// One def file as parsed TOML.
#[derive(Clone, Debug, PartialEq)]
pub struct RawDef {
    /// Where it came from: `defs/units/x.toml` (relative to the mod root),
    /// or an override file's path.
    pub file: String,
    /// The file's text, while `table` is still exactly the file (parse
    /// errors then quote the line).
    pub src: Option<String>,
    pub table: toml::Table,
}

impl RawDef {
    /// The parent this def names, if any.
    pub fn extends(&self) -> Option<&str> {
        self.table.get("extends").and_then(|v| v.as_str())
    }
}

/// Every def file of a mod as parsed TOML, by kind and id: the SDK view
/// before `extends` (schema tools, the Godot data reload, overrides).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RawDefs {
    pub kinds: BTreeMap<String, BTreeMap<String, RawDef>>,
}

impl RawDefs {
    pub fn get(&self, kind: &str, id: &str) -> Option<&RawDef> {
        self.kinds.get(kind)?.get(id)
    }

    pub fn get_mut(&mut self, kind: &str, id: &str) -> Option<&mut RawDef> {
        self.kinds.get_mut(kind)?.get_mut(id)
    }

    /// Ids of one kind, sorted.
    pub fn ids(&self, kind: &str) -> Vec<String> {
        self.kinds
            .get(kind)
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default()
    }

    pub fn insert(&mut self, kind: &str, id: String, def: RawDef) {
        self.kinds
            .entry(kind.to_owned())
            .or_default()
            .insert(id, def);
    }
}

/// A def's merged table and where each top-level key came from.
#[derive(Clone, Debug)]
pub struct Resolved {
    pub table: toml::Table,
    /// Top-level key → id of the def in the chain that set it last.
    pub from: BTreeMap<String, String>,
    /// The parents, nearest first.
    pub parents: Vec<String>,
}

impl Resolved {
    /// `" (from `<parent>`)"` when the issue's first key came from a parent
    /// of `id`, else `""`.
    pub fn origin_note(&self, id: &str, issue_path: &str) -> Option<String> {
        let key = issue_path
            .split(['.', '[', ':'])
            .next()
            .unwrap_or_default()
            .trim();
        match self.from.get(key) {
            Some(p) if p != id => Some(p.clone()),
            _ => None,
        }
    }
}

/// Resolves def `id` of `kind`: walks its `extends` chain and merges it
/// (module docs). Errors are issues on the child's `extends` (or the key
/// that breaks a unit rule), without the file prefix.
pub fn resolve(raw: &RawDefs, kind: &str, id: &str) -> Result<Resolved, Vec<Issue>> {
    let def = raw
        .get(kind, id)
        .ok_or_else(|| vec![Issue::new("id", format!("unknown {kind} def `{id}`"))])?;
    let mut chain = vec![id.to_owned()];
    let mut cur = def;
    loop {
        let parent = match cur.table.get("extends") {
            None => break,
            Some(toml::Value::String(p)) => p.as_str(),
            Some(_) => {
                return Err(vec![
                    Issue::new(
                        "extends",
                        format!(
                            "`extends` of `{}` must be a def id string",
                            chain[chain.len() - 1]
                        ),
                    )
                    .with_hint("extends = \"ww2:rifle_squad\""),
                ]);
            }
        };
        if let Some(at) = chain.iter().position(|c| c == parent) {
            let mut cycle: Vec<&str> = chain[at..].iter().map(String::as_str).collect();
            cycle.push(parent);
            return Err(vec![
                Issue::new(
                    "extends",
                    format!("`extends` makes a cycle: {}", cycle.join(" -> ")),
                )
                .with_hint("remove `extends` from one def of the cycle"),
            ]);
        }
        let Some(p) = raw.get(kind, parent) else {
            let elsewhere = DEF_KINDS
                .iter()
                .find(|k| **k != kind && raw.get(k, parent).is_some());
            let msg = match elsewhere {
                Some(k) => format!(
                    "`{parent}` is a {k} def; `extends` names a def of the same kind ({kind})"
                ),
                None => format!(
                    "unknown {kind} def `{parent}`{}",
                    did_you_mean(parent, raw.ids(kind).iter())
                ),
            };
            return Err(vec![Issue::new("extends", msg)]);
        };
        chain.push(parent.to_owned());
        if chain.len() - 1 > MAX_EXTENDS_DEPTH {
            return Err(vec![
                Issue::new(
                    "extends",
                    format!(
                        "the chain {} is {} parents deep; at most {MAX_EXTENDS_DEPTH}",
                        chain.join(" -> "),
                        chain.len() - 1
                    ),
                )
                .with_hint("extend a def nearer the root, or copy the keys you need"),
            ]);
        }
        cur = p;
    }
    let mut table = toml::Table::new();
    let mut from = BTreeMap::new();
    let n = chain.len();
    let mut issues = Vec::new();
    for (k, level_id) in chain.iter().rev().enumerate() {
        let last = k + 1 == n;
        let level = &raw.get(kind, level_id).expect("in chain").table;
        let mut level = level.clone();
        level.remove("extends");
        if !last {
            level.remove("id");
            level.remove("abstract");
        }
        if let Err(mut e) = merge_level(kind, &mut table, &mut from, level, level_id) {
            issues.append(&mut e);
        }
    }
    if !issues.is_empty() {
        return Err(issues);
    }
    table.insert("id".into(), toml::Value::String(id.to_owned()));
    chain.remove(0);
    Ok(Resolved {
        table,
        from,
        parents: chain,
    })
}

/// Merges one level of a chain (or a tuning override's patch) onto `acc`,
/// applying the unit rules of the module docs.
pub fn merge_level(
    kind: &str,
    acc: &mut toml::Table,
    from: &mut BTreeMap<String, String>,
    level: toml::Table,
    level_id: &str,
) -> Result<(), Vec<Issue>> {
    let mut issues = Vec::new();
    if kind == "units" {
        if level.contains_key("members") {
            for k in LEGACY_SOLDIER_KEYS {
                acc.remove(k);
                from.remove(k);
            }
        } else if acc.contains_key("members") {
            let parent = from.get("members").cloned().unwrap_or_default();
            for k in LEGACY_SOLDIER_KEYS {
                if level.contains_key(k) {
                    issues.push(
                        Issue::new(
                            k,
                            format!("parent `{parent}` uses [[members]]; set [[members]] here"),
                        )
                        .with_hint(format!(
                            "replace `{k}` with a [[members]] list (it replaces the parent's)"
                        )),
                    );
                }
            }
        }
        for (old, new) in CARD_ALIASES {
            match (level.contains_key(old), level.contains_key(new)) {
                (true, true) => issues.push(
                    Issue::new(
                        old,
                        format!("`{old}` and `{new}` are one key; set one of them"),
                    )
                    .with_hint(format!("remove `{old}` (S4 name: `{new}`)")),
                ),
                (true, false) => {
                    acc.remove(new);
                    from.remove(new);
                }
                (false, true) => {
                    acc.remove(old);
                    from.remove(old);
                }
                (false, false) => {}
            }
        }
    }
    if !issues.is_empty() {
        return Err(issues);
    }
    for k in level.keys() {
        from.insert(k.clone(), level_id.to_owned());
    }
    merge_tables(acc, level);
    Ok(())
}

/// Merges `over` into `base`: tables merge key by key, anything else
/// (arrays included) replaces.
pub fn merge_tables(base: &mut toml::Table, over: toml::Table) {
    for (k, v) in over {
        match (base.get_mut(&k), v) {
            (Some(toml::Value::Table(b)), toml::Value::Table(o)) => merge_tables(b, o),
            (_, v) => {
                base.insert(k, v);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(defs: &[(&str, &str)]) -> RawDefs {
        let mut r = RawDefs::default();
        for (id, text) in defs {
            let mut table: toml::Table = toml::from_str(text).unwrap();
            table.insert("id".into(), toml::Value::String((*id).into()));
            r.insert(
                "units",
                (*id).into(),
                RawDef {
                    file: format!("defs/units/{id}.toml"),
                    src: None,
                    table,
                },
            );
        }
        r
    }

    fn msg(e: Vec<Issue>) -> String {
        e.iter()
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn chain_of_four_resolves_five_is_refused() {
        let r = raw(&[
            ("t:a", "x = 1"),
            ("t:b", "extends = \"t:a\"\ny = 2"),
            ("t:c", "extends = \"t:b\""),
            ("t:d", "extends = \"t:c\""),
            ("t:e", "extends = \"t:d\"\nx = 5"),
            ("t:f", "extends = \"t:e\""),
        ]);
        let e = resolve(&r, "units", "t:e").unwrap();
        assert_eq!(e.parents, ["t:d", "t:c", "t:b", "t:a"]);
        assert_eq!(e.table["x"].as_integer(), Some(5));
        assert_eq!(e.table["y"].as_integer(), Some(2));
        assert_eq!(e.from["y"], "t:b");
        assert!(!e.table.contains_key("extends"));
        assert_eq!(e.table["id"].as_str(), Some("t:e"));
        let f = msg(resolve(&r, "units", "t:f").unwrap_err());
        assert!(f.contains("5 parents deep"), "{f}");
    }

    #[test]
    fn cycles_are_named() {
        let r = raw(&[
            ("t:a", "extends = \"t:b\""),
            ("t:b", "extends = \"t:a\""),
            ("t:c", "extends = \"t:d\""),
            ("t:d", "extends = \"t:e\""),
            ("t:e", "extends = \"t:c\""),
        ]);
        let a = msg(resolve(&r, "units", "t:a").unwrap_err());
        assert!(a.contains("t:a -> t:b -> t:a"), "{a}");
        let c = msg(resolve(&r, "units", "t:c").unwrap_err());
        assert!(c.contains("t:c -> t:d -> t:e -> t:c"), "{c}");
    }

    #[test]
    fn unknown_parent_suggests_a_near_id() {
        let r = raw(&[("t:rifle", "x = 1"), ("t:b", "extends = \"t:rifel\"")]);
        let e = msg(resolve(&r, "units", "t:b").unwrap_err());
        assert!(e.contains("did you mean `t:rifle`"), "{e}");
    }

    #[test]
    fn tables_merge_arrays_replace() {
        let r = raw(&[
            (
                "t:a",
                "tags = [\"line\", \"x\"]\n[sub]\na = 1\nb = 2\n[[loadout]]\nitem = \"g\"",
            ),
            ("t:b", "extends = \"t:a\"\ntags = [\"y\"]\n[sub]\nb = 3"),
        ]);
        let b = resolve(&r, "units", "t:b").unwrap();
        assert_eq!(b.table["tags"].as_array().unwrap().len(), 1);
        let sub = b.table["sub"].as_table().unwrap();
        assert_eq!(
            (sub["a"].as_integer(), sub["b"].as_integer()),
            (Some(1), Some(3))
        );
        assert!(b.table.contains_key("loadout"));
    }

    #[test]
    fn members_drop_legacy_keys_and_legacy_over_members_is_refused() {
        let r = raw(&[
            ("t:a", "men = 8\nweapon = \"w\"\nrecall_cooldown_s = 20"),
            (
                "t:b",
                "extends = \"t:a\"\ncooldown_s = 30\n[[members]]\nslot = \"r\"\nweapon = \"w\"",
            ),
            ("t:c", "extends = \"t:b\"\nmen = 4"),
            ("t:d", "cooldown_s = 1\nrecall_cooldown_s = 2"),
        ]);
        let b = resolve(&r, "units", "t:b").unwrap();
        assert!(!b.table.contains_key("men") && !b.table.contains_key("weapon"));
        assert!(!b.table.contains_key("recall_cooldown_s"));
        assert_eq!(b.table["cooldown_s"].as_integer(), Some(30));
        let c = msg(resolve(&r, "units", "t:c").unwrap_err());
        assert!(c.contains("parent `t:b` uses [[members]]"), "{c}");
        let d = msg(resolve(&r, "units", "t:d").unwrap_err());
        assert!(d.contains("one key"), "{d}");
    }

    #[test]
    fn id_and_abstract_are_not_inherited() {
        let r = raw(&[
            ("t:a", "abstract = true\nx = 1"),
            ("t:b", "extends = \"t:a\""),
        ]);
        let b = resolve(&r, "units", "t:b").unwrap();
        assert!(!b.table.contains_key("abstract"));
        assert_eq!(b.table["id"].as_str(), Some("t:b"));
        assert_eq!(b.origin_note("t:b", "x"), Some("t:a".into()));
        assert_eq!(b.origin_note("t:b", "id"), None);
    }
}
