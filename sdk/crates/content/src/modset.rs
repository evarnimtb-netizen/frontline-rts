//! Loading one content mod from disk (§7.1): `mod.toml` plus every def file
//! under `defs/`, with per-file and cross-file validation. Test overrides
//! of a rules def (`tests/rules_*.toml`) load on request
//! ([`ModDefs::add_rules_override`]).
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
use crate::vehicle::{AmmoDef, AmmoMechanism, MountKind, VehicleDef};

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
    /// Vehicles (S3, `defs/vehicles/`).
    pub vehicles: BTreeMap<String, VehicleDef>,
    /// Rounds for cannons and launchers (S3, `defs/ammo/`).
    pub ammo: BTreeMap<String, AmmoDef>,
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
            vehicles: l.load_kind(
                "vehicles",
                &ns,
                |d: &VehicleDef| &d.id,
                VehicleDef::validate,
            ),
            ammo: l.load_kind("ammo", &ns, |d: &AmmoDef| &d.id, AmmoDef::validate),
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

    /// Loads a test override of a rules def: a partial rules file whose
    /// `base` key names the rules def it starts from and whose `id` and
    /// `name` name the result. Every other key replaces the base's value;
    /// tables merge key by key, so `[vehicles] unlock_s = 0` changes one
    /// value. The result is validated like any rules def and added to
    /// [`ModDefs::rules`] (and so to the content hash). Returns its id.
    pub fn add_rules_override(&mut self, path: &Path) -> Result<String, ContentError> {
        let origin = path.display().to_string();
        let invalid = |issues: Vec<Issue>| ContentError::Invalid {
            origin: origin.clone(),
            issues,
        };
        let src = std::fs::read_to_string(path).map_err(|e| ContentError::Io {
            path: path.to_owned(),
            source: e,
        })?;
        let mut table: toml::Table = toml::from_str(&src).map_err(|e| ContentError::Parse {
            origin: origin.clone(),
            message: e.to_string().trim_end().to_owned(),
        })?;
        let base_id = match table.remove("base") {
            Some(toml::Value::String(b)) => b,
            _ => {
                return Err(invalid(vec![
                    Issue::new("base", "required: the rules id this file overrides")
                        .with_hint("base = \"ww2:skirmish\""),
                ]));
            }
        };
        let Some(base) = self.rules.get(&base_id) else {
            return Err(invalid(vec![Issue::new(
                "base",
                format!(
                    "unknown rules `{base_id}`{}",
                    did_you_mean(&base_id, self.rules.keys())
                ),
            )]));
        };
        for key in ["id", "name"] {
            if !table.contains_key(key) {
                return Err(invalid(vec![
                    Issue::new(key, "required: the override is a rules def of its own")
                        .with_hint(format!("{key} = \"...\"")),
                ]));
            }
        }
        let mut merged = match toml::Value::try_from(base) {
            Ok(toml::Value::Table(t)) => t,
            _ => unreachable!("a rules def serialises to a table"),
        };
        merge_tables(&mut merged, table);
        let def: RulesDef =
            toml::Value::Table(merged)
                .try_into()
                .map_err(|e: toml::de::Error| ContentError::Parse {
                    origin: origin.clone(),
                    message: e.to_string().trim_end().to_owned(),
                })?;
        let mut issues = def.validate();
        if let Some(m) = &self.manifest
            && !def.id.starts_with(&format!("{}:", m.id))
        {
            issues.push(Issue::new(
                "id",
                format!("`{}` is not in this mod's namespace `{}`", def.id, m.id),
            ));
        }
        if self.rules.contains_key(&def.id) {
            issues.push(Issue::new(
                "id",
                format!("`{}` is already a rules id", def.id),
            ));
        }
        if !issues.is_empty() {
            return Err(invalid(issues));
        }
        let id = def.id.clone();
        self.rules.insert(id.clone(), def);
        Ok(id)
    }

    /// Applies a tuning override file (S3b quick iteration; `frontline-cli
    /// --rules-override`). It is a partial TOML file:
    ///
    /// * top-level keys and tables merge onto the rules def `rules_id`, key
    ///   by key (`[combat] near_miss_cm = 250` changes one value);
    /// * `[defs.<kind>."<id>"]` tables merge onto one def of `kind` (`units`,
    ///   `weapons`, `items`, `factions`, `maps`, `vehicles`, `ammo`), e.g.
    ///   `[defs.units."ww2:ger_pz4_h_squad"] cost = 600`. Arrays (such as a
    ///   vehicle's `[[weapons]]`) replace the whole array.
    ///
    /// Every patched def is validated as on load, then the cross-file
    /// checks run again. Returns each changed value (dotted path, before,
    /// after) for the run's log. The defs (and so the content hash and
    /// match hashes) change; ids do not. On an error nothing changes.
    pub fn apply_override(
        &mut self,
        path: &Path,
        rules_id: &str,
    ) -> Result<Vec<OverrideChange>, ContentError> {
        let mut work = self.clone();
        let changes = work.apply_override_in_place(path, rules_id)?;
        *self = work;
        Ok(changes)
    }

    fn apply_override_in_place(
        &mut self,
        path: &Path,
        rules_id: &str,
    ) -> Result<Vec<OverrideChange>, ContentError> {
        let origin = path.display().to_string();
        let invalid = |issues: Vec<Issue>| ContentError::Invalid {
            origin: origin.clone(),
            issues,
        };
        let src = std::fs::read_to_string(path).map_err(|e| ContentError::Io {
            path: path.to_owned(),
            source: e,
        })?;
        let mut table: toml::Table = toml::from_str(&src).map_err(|e| ContentError::Parse {
            origin: origin.clone(),
            message: e.to_string().trim_end().to_owned(),
        })?;
        for key in ["base", "id", "name"] {
            if table.contains_key(key) {
                return Err(invalid(vec![
                    Issue::new(
                        key,
                        "an override patches the run's rules and keeps their id",
                    )
                    .with_hint(format!(
                        "remove `{key}` (files with `base`/`id` are rules defs of their own: \
                     tests/rules_*.toml)"
                    )),
                ]));
            }
        }
        let mut changes = Vec::new();
        if let Some(defs) = table.remove("defs") {
            let toml::Value::Table(defs) = defs else {
                return Err(invalid(vec![
                    Issue::new("defs", "must be a table of def kinds")
                        .with_hint("[defs.units.\"ww2:usa_rifle_squad\"]"),
                ]));
            };
            for (kind, ids) in defs {
                let toml::Value::Table(ids) = ids else {
                    return Err(invalid(vec![Issue::new(
                        format!("defs.{kind}"),
                        "must be a table of def ids",
                    )]));
                };
                for (id, patch) in ids {
                    let at = format!("defs.{kind}.\"{id}\"");
                    let toml::Value::Table(patch) = patch else {
                        return Err(invalid(vec![Issue::new(at, "must be a table of keys")]));
                    };
                    match kind.as_str() {
                        "units" => patch_in(&mut self.squads, &id, patch, &at, SquadDef::validate),
                        "weapons" => {
                            patch_in(&mut self.weapons, &id, patch, &at, WeaponDef::validate)
                        }
                        "items" => patch_in(&mut self.items, &id, patch, &at, ItemDef::validate),
                        "factions" => {
                            patch_in(&mut self.factions, &id, patch, &at, FactionDef::validate)
                        }
                        "maps" => patch_in(&mut self.maps, &id, patch, &at, MapFileDef::validate),
                        "vehicles" => {
                            patch_in(&mut self.vehicles, &id, patch, &at, VehicleDef::validate)
                        }
                        "ammo" => patch_in(&mut self.ammo, &id, patch, &at, AmmoDef::validate),
                        _ => Err(vec![
                            Issue::new(
                                format!("defs.{kind}"),
                                format!("unknown def kind `{kind}`"),
                            )
                            .with_hint(
                                "one of units, weapons, items, factions, maps, vehicles, ammo",
                            ),
                        ]),
                    }
                    .map(|c| changes.extend(c))
                    .map_err(invalid)?;
                }
            }
        }
        if !table.is_empty() {
            if !self.rules.contains_key(rules_id) {
                return Err(invalid(vec![Issue::new(
                    "",
                    format!(
                        "unknown rules `{rules_id}`{}",
                        did_you_mean(rules_id, self.rules.keys())
                    ),
                )]));
            }
            let c = patch_in(
                &mut self.rules,
                rules_id,
                table,
                &format!("rules \"{rules_id}\""),
                RulesDef::validate,
            )
            .map_err(invalid)?;
            changes.extend(c);
        }
        let issues = self.cross_check();
        if !issues.is_empty() {
            return Err(invalid(issues));
        }
        Ok(changes)
    }

    /// References between defs: squads → factions, weapons, items and
    /// vehicles, faction rosters → squads, role/weapon agreement, at most
    /// one item of each kind per loadout, and vehicle mounts → weapons and
    /// ammo.
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
            if s.role == SquadRole::Vehicle {
                if let Some(v) = &s.vehicle {
                    match self.vehicles.get(v) {
                        None => out.push(Issue::new(
                            format!("{}: vehicle", squad_file(id)),
                            format!(
                                "unknown vehicle `{v}`{}",
                                did_you_mean(v, self.vehicles.keys())
                            ),
                        )),
                        Some(vd) if vd.faction != s.faction => out.push(Issue::new(
                            format!("{}: faction", squad_file(id)),
                            format!("vehicle `{v}` belongs to faction `{}`", vd.faction),
                        )),
                        Some(_) => {}
                    }
                }
                continue;
            }
            match self.weapons.get(&s.weapon) {
                None => out.push(Issue::new(
                    format!("{}: weapon", squad_file(id)),
                    format!(
                        "unknown weapon `{}`{}",
                        s.weapon,
                        did_you_mean(&s.weapon, self.weapons.keys())
                    ),
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
                    let at_role = s.role == SquadRole::AntiTank;
                    let launcher = w.class == WeaponClass::Launcher;
                    if at_role != launcher {
                        out.push(Issue::new(
                            format!("{}: weapon", squad_file(id)),
                            "anti_tank squads carry a launcher, other squads must not",
                        ));
                    }
                    if w.class == WeaponClass::Cannon {
                        out.push(Issue::new(
                            format!("{}: weapon", squad_file(id)),
                            "a cannon is a vehicle weapon",
                        ));
                    }
                }
            }
            if let Some(side) = &s.sidearm {
                match self.weapons.get(side) {
                    None => out.push(Issue::new(
                        format!("{}: sidearm", squad_file(id)),
                        format!(
                            "unknown weapon `{side}`{}",
                            did_you_mean(side, self.weapons.keys())
                        ),
                    )),
                    Some(w)
                        if !matches!(
                            w.class,
                            WeaponClass::Rifle | WeaponClass::Smg | WeaponClass::Mg
                        ) =>
                    {
                        out.push(Issue::new(
                            format!("{}: sidearm", squad_file(id)),
                            "a sidearm is a rifle, SMG or MG",
                        ))
                    }
                    Some(_) => {}
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
        for (id, w) in &self.weapons {
            let Some(a) = &w.ammo else { continue };
            match self.ammo.get(a) {
                None => out.push(Issue::new(
                    format!("weapon `{id}`: ammo"),
                    format!("unknown ammo `{a}`{}", did_you_mean(a, self.ammo.keys())),
                )),
                Some(ad) if ad.muzzle_velocity_m_s != w.muzzle_velocity_m_s => out.push(
                    Issue::new(
                        format!("weapon `{id}`: muzzle_velocity_m_s"),
                        format!(
                            "{} m/s, but its ammo `{a}` flies at {} m/s",
                            w.muzzle_velocity_m_s, ad.muzzle_velocity_m_s
                        ),
                    )
                    .with_hint(format!("muzzle_velocity_m_s = {}", ad.muzzle_velocity_m_s)),
                ),
                Some(_) => {}
            }
        }
        for (id, v) in &self.vehicles {
            let file = format!("vehicle `{id}`");
            if !self.factions.contains_key(&v.faction) {
                out.push(Issue::new(
                    format!("{file}: faction"),
                    format!("unknown faction `{}`", v.faction),
                ));
            }
            for (i, m) in v.weapons.iter().enumerate() {
                let p = format!("{file}: weapons[{i}]");
                match self.weapons.get(&m.def) {
                    None => out.push(Issue::new(
                        format!("{p}.def"),
                        format!(
                            "unknown weapon `{}`{}",
                            m.def,
                            did_you_mean(&m.def, self.weapons.keys())
                        ),
                    )),
                    Some(w) => {
                        let want = if m.mount == MountKind::TurretMain {
                            WeaponClass::Cannon
                        } else {
                            WeaponClass::Mg
                        };
                        if w.class != want {
                            out.push(Issue::new(
                                format!("{p}.def"),
                                format!(
                                    "a `{:?}` mount takes a {want:?} weapon, `{}` is {:?}",
                                    m.mount, m.def, w.class
                                ),
                            ));
                        }
                    }
                }
                let mut kinds = [0u8; 2];
                for (k, a) in m.ammo.iter().enumerate() {
                    match self.ammo.get(&a.id) {
                        None => out.push(
                            Issue::new(
                                format!("{p}.ammo[{k}].id"),
                                format!(
                                    "unknown ammo `{}`{}",
                                    a.id,
                                    did_you_mean(&a.id, self.ammo.keys())
                                ),
                            )
                            .with_hint("add defs/ammo/<name>.toml or fix the id"),
                        ),
                        Some(ad) => kinds[(ad.mechanism == AmmoMechanism::He) as usize] += 1,
                    }
                }
                if kinds.iter().any(|&n| n > 1) {
                    out.push(Issue::new(
                        format!("{p}.ammo"),
                        "at most one armour-piercing (ap, aphe, heat) and one he round",
                    ));
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

/// One value an override changed (`ModDefs::apply_override`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OverrideChange {
    /// `rules "ww2:skirmish": combat.near_miss_cm`, or
    /// `defs.units."ww2:usa_rifle_squad": cost`.
    pub path: String,
    /// TOML text of the value before and after (`None`: absent).
    pub old: Option<String>,
    pub new: Option<String>,
}

impl std::fmt::Display for OverrideChange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let show = |v: &Option<String>| v.clone().unwrap_or_else(|| "(none)".into());
        write!(
            f,
            "{}: {} -> {}",
            self.path,
            show(&self.old),
            show(&self.new)
        )
    }
}

/// Merges `patch` onto def `id` of `defs` (TOML round trip), validates it
/// and stores it; returns the changed values. `at` names the def in issues.
fn patch_in<T: Serialize + DeserializeOwned>(
    defs: &mut BTreeMap<String, T>,
    id: &str,
    patch: toml::Table,
    at: &str,
    validate: impl Fn(&T) -> Vec<Issue>,
) -> Result<Vec<OverrideChange>, Vec<Issue>> {
    let Some(def) = defs.get(id) else {
        return Err(vec![Issue::new(
            at,
            format!("unknown id `{id}`{}", did_you_mean(id, defs.keys())),
        )]);
    };
    let before = match toml::Value::try_from(def) {
        Ok(toml::Value::Table(t)) => t,
        _ => return Err(vec![Issue::new(at, "this def cannot be patched as TOML")]),
    };
    let mut merged = before.clone();
    merge_tables(&mut merged, patch);
    let new: T = toml::Value::Table(merged.clone())
        .try_into()
        .map_err(|e: toml::de::Error| vec![Issue::new(at, e.to_string().trim_end().to_owned())])?;
    let issues: Vec<Issue> = validate(&new)
        .into_iter()
        .map(|i| Issue {
            path: format!("{at}: {}", i.path),
            ..i
        })
        .collect();
    if !issues.is_empty() {
        return Err(issues);
    }
    let (mut a, mut b) = (BTreeMap::new(), BTreeMap::new());
    flatten("", &before, &mut a);
    flatten("", &merged, &mut b);
    let keys: std::collections::BTreeSet<&String> = a.keys().chain(b.keys()).collect();
    let changes = keys
        .into_iter()
        .filter(|k| a.get(*k) != b.get(*k))
        .map(|k| OverrideChange {
            path: format!("{at}: {k}"),
            old: a.get(k).cloned(),
            new: b.get(k).cloned(),
        })
        .collect();
    defs.insert(id.to_owned(), new);
    Ok(changes)
}

/// Dotted path → TOML text of every non-table value under `t`.
fn flatten(prefix: &str, t: &toml::Table, out: &mut BTreeMap<String, String>) {
    for (k, v) in t {
        let p = if prefix.is_empty() {
            k.clone()
        } else {
            format!("{prefix}.{k}")
        };
        match v {
            toml::Value::Table(sub) => flatten(&p, sub, out),
            v => {
                out.insert(p, v.to_string());
            }
        }
    }
}

/// Merges `over` into `base`: tables merge key by key, anything else
/// replaces.
fn merge_tables(base: &mut toml::Table, over: toml::Table) {
    for (k, v) in over {
        match (base.get_mut(&k), v) {
            (Some(toml::Value::Table(b)), toml::Value::Table(o)) => merge_tables(b, o),
            (_, v) => {
                base.insert(k, v);
            }
        }
    }
}

/// `" (did you mean `x`?)"` for the closest known id, or `""` when nothing
/// is close (§8.2: actionable errors).
pub(crate) fn did_you_mean<'a>(id: &str, known: impl Iterator<Item = &'a String>) -> String {
    let best = known
        .map(|k| (edit_distance(id, k), k))
        .min_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(b.1)));
    match best {
        Some((d, k)) if d <= (id.len() / 3).max(2) => format!(" (did you mean `{k}`?)"),
        _ => String::new(),
    }
}

/// Levenshtein distance over bytes (ids are ASCII).
fn edit_distance(a: &str, b: &str) -> usize {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, &ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1; b.len() + 1];
        for (j, &cb) in b.iter().enumerate() {
            cur[j + 1] = (prev[j] + usize::from(ca != cb))
                .min(prev[j + 1] + 1)
                .min(cur[j] + 1);
        }
        prev = cur;
    }
    prev[b.len()]
}
