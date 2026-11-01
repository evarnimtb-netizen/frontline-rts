//! Loading one content mod from disk (§7.1): `mod.toml` plus every def file
//! under `defs/`, with per-file and cross-file validation.
//!
//! Files are read in sorted path order and defs are kept in `BTreeMap`s keyed
//! by id, so everything derived from a [`ModDefs`] (including the sim's
//! [`crate::MatchSetup`]) is independent of directory listing order.
//! Problems from every file are collected into one
//! [`ContentError::Invalid`]; each issue's path starts with the file, e.g.
//! `defs/units/ger_rifle_squad.toml: men`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::defs::{
    FactionDef, ITEM_KINDS, ItemDef, MapFileDef, RulesDef, SquadDef, SquadRole, WeaponClass,
    WeaponDef, check_id,
};
use crate::issue::{ContentError, Issue};

/// A mod's manifest (`mod.toml`, §7.2).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ModManifest {
    /// Namespace for every def id in the mod (`<id>:<name>`).
    pub id: String,
    pub name: String,
    pub version: String,
    /// `content` or `cosmetic`.
    pub kind: String,
    pub api_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub godot: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<String>,
    #[serde(default = "default_true")]
    pub deterministic: bool,
}

fn default_true() -> bool {
    true
}

/// Every def of one mod, validated, keyed by id.
///
/// `Serialize` exists for [`ModDefs::content_hash`] only; nothing reads the
/// encoding back.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct ModDefs {
    pub manifest: Option<ModManifest>,
    pub squads: BTreeMap<String, SquadDef>,
    pub weapons: BTreeMap<String, WeaponDef>,
    pub items: BTreeMap<String, ItemDef>,
    pub factions: BTreeMap<String, FactionDef>,
    pub maps: BTreeMap<String, MapFileDef>,
    pub rules: BTreeMap<String, RulesDef>,
}

/// Collects issues for one mod directory.
struct Loader {
    root: PathBuf,
    issues: Vec<Issue>,
}

impl Loader {
    fn rel(&self, p: &Path) -> String {
        p.strip_prefix(&self.root)
            .unwrap_or(p)
            .to_string_lossy()
            .replace('\\', "/")
    }

    fn push(&mut self, file: &str, issue: Issue) {
        let path = if issue.path.is_empty() {
            file.to_owned()
        } else {
            format!("{file}: {}", issue.path)
        };
        self.issues.push(Issue { path, ..issue });
    }

    /// Parses one file; parse errors become issues.
    fn parse<T: DeserializeOwned>(&mut self, path: &Path) -> Option<T> {
        let file = self.rel(path);
        let src = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                self.push(&file, Issue::new("", format!("cannot read: {e}")));
                return None;
            }
        };
        match toml::from_str(&src) {
            Ok(v) => Some(v),
            Err(e) => {
                self.push(&file, Issue::new("", e.to_string().trim_end().to_owned()));
                None
            }
        }
    }

    /// Loads every `*.toml` in `defs/<dir>` (sorted), validating each and
    /// rejecting duplicate ids.
    fn load_kind<T: DeserializeOwned>(
        &mut self,
        dir: &str,
        ns: &str,
        id: impl Fn(&T) -> &str,
        validate: impl Fn(&T) -> Vec<Issue>,
    ) -> BTreeMap<String, T> {
        let mut out = BTreeMap::new();
        let path = self.root.join("defs").join(dir);
        let Ok(rd) = std::fs::read_dir(&path) else {
            return out;
        };
        let mut files: Vec<PathBuf> = rd
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "toml"))
            .collect();
        files.sort();
        for f in files {
            let Some(def) = self.parse::<T>(&f) else {
                continue;
            };
            let file = self.rel(&f);
            for i in validate(&def) {
                self.push(&file, i);
            }
            let def_id = id(&def).to_owned();
            if !ns.is_empty() && !def_id.starts_with(&format!("{ns}:")) {
                self.push(
                    &file,
                    Issue::new(
                        "id",
                        format!("`{def_id}` is not in this mod's namespace `{ns}`"),
                    )
                    .with_hint(format!("id = \"{ns}:...\"")),
                );
            }
            if out.contains_key(&def_id) {
                self.push(
                    &file,
                    Issue::new("id", format!("`{def_id}` is defined twice in defs/{dir}")),
                );
                continue;
            }
            out.insert(def_id, def);
        }
        out
    }
}

impl ModDefs {
    /// xxh3 of the postcard encoding of every def (S2 "Session setup"): two
    /// game builds that load different defs get different hashes, so the
    /// relay can refuse the second player before the lobby. The maps are
    /// `BTreeMap`s keyed by id, so the hash never depends on file listing
    /// order. It is a guard against honest mistakes, not a signature.
    pub fn content_hash(&self) -> u64 {
        let bytes = postcard::to_allocvec(self).expect("encoding defs cannot fail");
        xxhash_rust::xxh3::xxh3_64(&bytes)
    }

    /// Reads and validates the mod at `root` (the folder holding `mod.toml`
    /// and `defs/`).
    pub fn load(root: &Path) -> Result<ModDefs, ContentError> {
        let mut l = Loader {
            root: root.to_owned(),
            issues: Vec::new(),
        };
        let manifest_path = root.join("mod.toml");
        let manifest: Option<ModManifest> = l.parse(&manifest_path);
        let ns = manifest.as_ref().map(|m| m.id.clone()).unwrap_or_default();
        if let Some(m) = &manifest {
            let mut id_issues = Vec::new();
            check_id(&mut id_issues, "id", &format!("{}:x", m.id));
            for i in id_issues {
                l.push("mod.toml", i);
            }
        }
        let mut defs = ModDefs {
            squads: l.load_kind("units", &ns, |d: &SquadDef| &d.id, SquadDef::validate),
            weapons: l.load_kind("weapons", &ns, |d: &WeaponDef| &d.id, WeaponDef::validate),
            items: l.load_kind("items", &ns, |d: &ItemDef| &d.id, ItemDef::validate),
            factions: l.load_kind(
                "factions",
                &ns,
                |d: &FactionDef| &d.id,
                FactionDef::validate,
            ),
            maps: l.load_kind("maps", &ns, |d: &MapFileDef| &d.id, MapFileDef::validate),
            rules: l.load_kind("rules", &ns, |d: &RulesDef| &d.id, RulesDef::validate),
            manifest,
        };
        l.issues.extend(defs.cross_check());
        if l.issues.is_empty() {
            Ok(defs)
        } else {
            defs.manifest = None;
            Err(ContentError::Invalid {
                origin: root.display().to_string(),
                issues: l.issues,
            })
        }
    }

    /// References between defs: squads → factions, weapons and items,
    /// faction rosters → squads, role/weapon agreement, and at most one item
    /// of each kind per loadout.
    pub fn cross_check(&self) -> Vec<Issue> {
        let mut out = Vec::new();
        let squad_file = |id: &str| format!("squad `{id}`");
        for (id, s) in &self.squads {
            if !self.factions.contains_key(&s.faction) {
                out.push(Issue::new(
                    format!("{}: faction", squad_file(id)),
                    format!("unknown faction `{}`", s.faction),
                ));
            }
            match self.weapons.get(&s.weapon) {
                None => out.push(Issue::new(
                    format!("{}: weapon", squad_file(id)),
                    format!("unknown weapon `{}`", s.weapon),
                )),
                Some(w) => {
                    let mortar_role = s.role == SquadRole::Mortar;
                    let mortar_weapon = w.class == WeaponClass::Mortar;
                    if mortar_role != mortar_weapon {
                        out.push(Issue::new(
                            format!("{}: weapon", squad_file(id)),
                            "mortar squads carry a mortar weapon, other squads must not",
                        ));
                    }
                }
            }
            let mut kinds = [false; ITEM_KINDS];
            for (i, l) in s.loadout.iter().enumerate() {
                match self.items.get(&l.item) {
                    None => out.push(Issue::new(
                        format!("{}: loadout[{i}].item", squad_file(id)),
                        format!("unknown item `{}`", l.item),
                    )),
                    Some(it) if kinds[it.kind.index()] => out.push(Issue::new(
                        format!("{}: loadout[{i}].item", squad_file(id)),
                        format!("a second {:?} item; carry one of each kind", it.kind),
                    )),
                    Some(it) => kinds[it.kind.index()] = true,
                }
            }
        }
        for (id, f) in &self.factions {
            for (i, s) in f.roster.iter().enumerate() {
                match self.squads.get(s) {
                    None => out.push(Issue::new(
                        format!("faction `{id}`: roster[{i}]"),
                        format!("unknown squad `{s}`"),
                    )),
                    Some(sq) if sq.faction != *id => out.push(Issue::new(
                        format!("faction `{id}`: roster[{i}]"),
                        format!("squad `{s}` belongs to faction `{}`", sq.faction),
                    )),
                    Some(_) => {}
                }
            }
        }
        out
    }
}
