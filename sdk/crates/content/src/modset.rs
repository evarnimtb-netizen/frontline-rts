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
    CartridgeDef, FactionDef, ITEM_KINDS, ItemDef, MapFileDef, RulesDef, SquadDef, SquadRole,
    WeaponClass, WeaponDef, check_id,
};
use crate::issue::{ContentError, Issue};
use crate::resolve::{DEF_KINDS, RawDef, RawDefs, Resolved, merge_level, merge_tables};
use crate::variant::{
    DERIVED_CAPABILITIES, TrainingDef, canonical_capabilities, expand_legacy, role_capabilities,
    role_of_tags, role_tag,
};
use crate::vehicle::{AmmoDef, AmmoMechanism, CrewModel, VehicleDef};

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

/// Every def of one mod, resolved (`extends`, S4) and validated, keyed by
/// id.
///
/// `Serialize` exists for [`ModDefs::content_hash`] only; nothing reads the
/// encoding back. Equality and the hash cover the resolved defs, not
/// [`ModDefs::raw`].
#[derive(Clone, Debug, Default, Serialize)]
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
    /// Training profiles (S4, `defs/training/`).
    pub training: BTreeMap<String, TrainingDef>,
    /// S5a: counted rounds without ballistics (`defs/cartridges/`).
    pub cartridges: BTreeMap<String, CartridgeDef>,
    /// S6: HQ, dump and crate defs (`defs/supply/`).
    pub supply: BTreeMap<String, crate::defs::StockDef>,
    /// S10a: fortification blueprints (`defs/fortifications/`).
    pub forts: BTreeMap<String, crate::defs::FortDef>,
    /// S4: every def file as parsed TOML before `extends` (the SDK view).
    /// Tuning overrides patch these and every def resolves again.
    #[serde(skip)]
    pub raw: RawDefs,
}

impl PartialEq for ModDefs {
    fn eq(&self, o: &ModDefs) -> bool {
        self.manifest == o.manifest
            && self.squads == o.squads
            && self.weapons == o.weapons
            && self.items == o.items
            && self.factions == o.factions
            && self.maps == o.maps
            && self.rules == o.rules
            && self.vehicles == o.vehicles
            && self.ammo == o.ammo
            && self.training == o.training
            && self.cartridges == o.cartridges
            && self.supply == o.supply
            && self.forts == o.forts
    }
}

impl Eq for ModDefs {}

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

    /// Reads every `*.toml` in `dir/<kind>` (sorted) into `raw`: parse
    /// errors, ids outside the namespace and duplicate ids are issues.
    fn read_kind(&mut self, raw: &mut RawDefs, dir: &Path, kind: &str, ns: &str) {
        let path = dir.join(kind);
        let Ok(rd) = std::fs::read_dir(&path) else {
            return;
        };
        let mut files: Vec<PathBuf> = rd
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "toml"))
            .collect();
        files.sort();
        for f in files {
            let file = self.rel(&f);
            let src = match std::fs::read_to_string(&f) {
                Ok(s) => s,
                Err(e) => {
                    self.push(&file, Issue::new("", format!("cannot read: {e}")));
                    continue;
                }
            };
            let table: toml::Table = match toml::from_str(&src) {
                Ok(t) => t,
                Err(e) => {
                    self.push(&file, Issue::new("", e.to_string().trim_end().to_owned()));
                    continue;
                }
            };
            let def_id = match table.get("id") {
                Some(toml::Value::String(id)) => id.clone(),
                _ => {
                    #[derive(Deserialize)]
                    #[allow(dead_code)]
                    struct IdOnly {
                        id: String,
                    }
                    let msg = match toml::from_str::<IdOnly>(&src) {
                        Err(e) => e.to_string().trim_end().to_owned(),
                        Ok(_) => "missing field `id`".to_owned(),
                    };
                    self.push(&file, Issue::new("", msg));
                    continue;
                }
            };
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
            if raw.get(kind, &def_id).is_some() {
                self.push(
                    &file,
                    Issue::new("id", format!("`{def_id}` is defined twice in defs/{kind}")),
                );
                continue;
            }
            raw.insert(
                kind,
                def_id,
                RawDef {
                    file,
                    src: Some(src),
                    table,
                },
            );
        }
    }
}

/// Reads one resolved table into a def. A def without `extends` that fails
/// is read again from its file text, so the error quotes the line.
fn read_def<T: DeserializeOwned>(rd: &RawDef, res: &Resolved) -> Result<T, String> {
    match toml::Value::Table(res.table.clone()).try_into::<T>() {
        Ok(d) => Ok(d),
        Err(e) => {
            if res.parents.is_empty()
                && let Some(src) = &rd.src
                && let Err(e2) = toml::from_str::<T>(src)
            {
                return Err(e2.to_string().trim_end().to_owned());
            }
            let after = if res.parents.is_empty() {
                String::new()
            } else {
                format!(" (after extends {})", res.parents.join(" -> "))
            };
            Err(format!("{}{after}", e.to_string().trim_end()))
        }
    }
}

/// Prefixes an issue's path with its file (the loader's format).
fn at_file(file: &str, issue: Issue) -> Issue {
    let path = if issue.path.is_empty() {
        file.to_owned()
    } else {
        format!("{file}: {}", issue.path)
    };
    Issue { path, ..issue }
}

/// Resolves every def of `kind` from `raw`; issues go to `issues` with the
/// file prefix. `prepare` runs before `validate` (units: the role),
/// `finish` after it (units: the canonical view).
fn resolve_kind<T: DeserializeOwned>(
    raw: &RawDefs,
    kind: &str,
    validate: impl Fn(&T) -> Vec<Issue>,
    mut prepare: impl FnMut(&mut T, &Resolved) -> Vec<Issue>,
    mut finish: impl FnMut(&mut T) -> Vec<Issue>,
    issues: &mut Vec<Issue>,
) -> BTreeMap<String, T> {
    let mut out = BTreeMap::new();
    for id in raw.ids(kind) {
        let rd = raw.get(kind, &id).expect("listed");
        let res = match crate::resolve::resolve(raw, kind, &id) {
            Ok(r) => r,
            Err(e) => {
                issues.extend(e.into_iter().map(|i| at_file(&rd.file, i)));
                continue;
            }
        };
        let mut def: T = match read_def(rd, &res) {
            Ok(d) => d,
            Err(msg) => {
                issues.push(at_file(&rd.file, Issue::new("", msg)));
                continue;
            }
        };
        let mut local = prepare(&mut def, &res);
        local.extend(validate(&def));
        local.extend(finish(&mut def));
        for mut i in local {
            if let Some(p) = res.origin_note(&id, &i.path) {
                i.message = format!("{} (from `{p}`)", i.message);
            }
            issues.push(at_file(&rd.file, i));
        }
        out.insert(id, def);
    }
    out
}

/// S4: units without `role` take it from their tags, their vehicle or their
/// weapons; `role` and the reserved `crew` tag must agree.
fn prepare_unit(
    s: &mut SquadDef,
    res: &Resolved,
    weapons: &BTreeMap<String, WeaponDef>,
) -> Vec<Issue> {
    let mut out = Vec::new();
    if res.table.contains_key("role") {
        let crew_tag = s.has_tag("crew");
        if !s.tags.is_empty() && crew_tag != (s.role == SquadRole::Crew) {
            out.push(
                Issue::new(
                    "tags",
                    "the reserved tag `crew` marks exactly the crew squads (role = \"crew\")",
                )
                .with_hint(if crew_tag {
                    "remove \"crew\" from tags"
                } else {
                    "add \"crew\" to tags"
                }),
            );
        }
        return out;
    }
    if s.tags.is_empty() && s.vehicle.is_none() {
        out.push(
            Issue::new(
                "tags",
                "required: what the unit is, for the AI mix and the HUD (or the legacy `role`)",
            )
            .with_hint("tags = [\"line\"]"),
        );
    }
    let class_of = |w: &str| weapons.get(w).map(|d| d.class);
    let carries = |c: WeaponClass| {
        s.members.iter().any(|m| class_of(&m.weapon) == Some(c))
            || (!s.weapon.is_empty() && class_of(&s.weapon) == Some(c))
    };
    s.role = if s.vehicle.is_some() {
        SquadRole::Vehicle
    } else if let Some(r) = role_of_tags(&s.tags) {
        r
    } else if carries(WeaponClass::Mortar) {
        SquadRole::Mortar
    } else if carries(WeaponClass::Launcher) {
        SquadRole::AntiTank
    } else {
        SquadRole::Rifle
    };
    out
}

/// S4 canonical view of a unit: legacy soldiers expanded into `members`,
/// `tags` and `capabilities` from the role when left empty, derived
/// capabilities added, and the card keys under their S4 names.
fn finish_unit(
    s: &mut SquadDef,
    weapons: &BTreeMap<String, WeaponDef>,
    items: &BTreeMap<String, ItemDef>,
) -> Vec<Issue> {
    let mut out = Vec::new();
    if s.role != SquadRole::Vehicle && s.members.is_empty() {
        // A legacy launcher squad: one man carries the launcher, the rest
        // the sidearm, whatever `weapon_men` says (the S3 `weapon_of`).
        let launcher = weapons
            .get(&s.weapon)
            .is_some_and(|w| w.class == WeaponClass::Launcher);
        s.members = expand_legacy(s, launcher);
    }
    if s.tags.is_empty() {
        s.tags = vec![role_tag(s.role).to_owned()];
    }
    let explicit = s.capabilities.clone();
    if s.capabilities.is_empty() {
        s.capabilities = role_capabilities(s.role, s.dig_rate_pct);
    }
    let class_of = |w: &str| weapons.get(w).map(|d| d.class);
    let has = |c: WeaponClass| s.members.iter().any(|m| class_of(&m.weapon) == Some(c));
    // S4 Part B: a single-shot AT item (Panzerfaust) in the kit gives
    // `at_hunt` too.
    let single_shot = s.members.iter().any(|m| {
        m.items.iter().any(|(id, &n)| {
            n > 0
                && items
                    .get(id)
                    .is_some_and(|d| d.kind == crate::defs::ItemKind::SingleShotAt)
        })
    });
    let derived = [
        ("fire_mission", has(WeaponClass::Mortar), "a mortar"),
        (
            "at_hunt",
            has(WeaponClass::Launcher) || single_shot,
            "a launcher or a single-shot AT item",
        ),
        // S14 (plan 4.2): any member, either side of a `when` pair (the
        // setup keeps it only when a kept member carries one).
        ("flame", has(WeaponClass::Flame), "a flame weapon"),
    ];
    for (cap, yes, what) in derived {
        debug_assert!(DERIVED_CAPABILITIES.contains(&cap));
        if yes {
            s.capabilities.push(cap.to_owned());
        } else if explicit.iter().any(|c| c == cap) {
            out.push(
                Issue::new(
                    "capabilities",
                    format!("`{cap}` comes from {what} among the members, and none carries one"),
                )
                .with_hint(format!("remove `{cap}`, or give a member {what}")),
            );
        }
    }
    canonical_capabilities(&mut s.capabilities);
    // S4 review M2: the sim keys crew rules (crew squad link, dismissal in
    // base, capture) on `vehicle_crew`, the content checks on the tag.
    let crew_tag = s.has_tag("crew");
    if s.role != SquadRole::Vehicle && crew_tag != s.has_capability("vehicle_crew") {
        out.push(
            Issue::new(
                "capabilities",
                "the tag `crew` and the capability `vehicle_crew` go together (a crew squad has \
                 both, other squads neither)",
            )
            .with_hint(if crew_tag {
                "add \"vehicle_crew\" to capabilities"
            } else {
                "remove \"vehicle_crew\", or add \"crew\" to tags"
            }),
        );
    }
    if s.role != SquadRole::Vehicle {
        s.cooldown_s = Some(s.card_cooldown_s());
        s.unlock_s = Some(s.card_unlock_s());
    }
    out
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
        let mut raw = RawDefs::default();
        for kind in DEF_KINDS {
            l.read_kind(&mut raw, &root.join("defs"), kind, &ns);
        }
        let mut defs = ModDefs {
            manifest,
            raw,
            ..ModDefs::default()
        };
        l.issues.extend(defs.resolve_all());
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

    /// Resolves every def from [`ModDefs::raw`] (`extends`, S4), replacing
    /// the resolved maps; returns the per-def issues, file first.
    pub fn resolve_all(&mut self) -> Vec<Issue> {
        fn plain<T: DeserializeOwned>(
            raw: &RawDefs,
            kind: &str,
            validate: impl Fn(&T) -> Vec<Issue>,
            issues: &mut Vec<Issue>,
        ) -> BTreeMap<String, T> {
            resolve_kind(
                raw,
                kind,
                validate,
                |_, _| Vec::new(),
                |_| Vec::new(),
                issues,
            )
        }
        let raw = &self.raw;
        let mut is: [Vec<Issue>; DEF_KINDS.len()] = Default::default();
        let k = |kind: &str| DEF_KINDS.iter().position(|x| *x == kind).expect("a kind");
        let weapons: BTreeMap<String, WeaponDef> =
            plain(raw, "weapons", WeaponDef::validate, &mut is[k("weapons")]);
        let items = plain(raw, "items", ItemDef::validate, &mut is[k("items")]);
        let ammo = plain(raw, "ammo", AmmoDef::validate, &mut is[k("ammo")]);
        let vehicles = plain(
            raw,
            "vehicles",
            VehicleDef::validate,
            &mut is[k("vehicles")],
        );
        let training = plain(
            raw,
            "training",
            TrainingDef::validate,
            &mut is[k("training")],
        );
        let cartridges = plain(
            raw,
            "cartridges",
            CartridgeDef::validate,
            &mut is[k("cartridges")],
        );
        let supply = plain(
            raw,
            "supply",
            crate::defs::StockDef::validate,
            &mut is[k("supply")],
        );
        let forts = plain(
            raw,
            "fortifications",
            crate::defs::FortDef::validate,
            &mut is[k("fortifications")],
        );
        let factions = plain(
            raw,
            "factions",
            FactionDef::validate,
            &mut is[k("factions")],
        );
        let maps = plain(raw, "maps", MapFileDef::validate, &mut is[k("maps")]);
        let rules = plain(raw, "rules", RulesDef::validate, &mut is[k("rules")]);
        let squads = resolve_kind(
            raw,
            "units",
            SquadDef::validate,
            |s: &mut SquadDef, r: &Resolved| prepare_unit(s, r, &weapons),
            |s: &mut SquadDef| finish_unit(s, &weapons, &items),
            &mut is[k("units")],
        );
        // The S1-S3 report order: units, weapons, items, factions, maps,
        // rules, vehicles, ammo (DEF_KINDS order).
        let out: Vec<Issue> = is.into_iter().flatten().collect();
        self.squads = squads;
        self.weapons = weapons;
        self.items = items;
        self.factions = factions;
        self.maps = maps;
        self.rules = rules;
        self.vehicles = vehicles;
        self.ammo = ammo;
        self.training = training;
        self.cartridges = cartridges;
        self.supply = supply;
        self.forts = forts;
        out
    }

    /// Adds the def files under `dir/<kind>/` (e.g. a mod's
    /// `tests/variants/units/x.toml`) to these defs: they may extend the
    /// mod's defs, must not reuse their ids, and are validated with
    /// everything else. On an error nothing changes.
    pub fn add_defs_dir(&mut self, dir: &Path) -> Result<Vec<String>, ContentError> {
        let mut work = self.clone();
        let mut l = Loader {
            root: dir.parent().unwrap_or(dir).to_owned(),
            issues: Vec::new(),
        };
        let ns = self
            .manifest
            .as_ref()
            .map(|m| m.id.clone())
            .unwrap_or_default();
        let mut extra = RawDefs::default();
        for kind in DEF_KINDS {
            l.read_kind(&mut extra, dir, kind, &ns);
        }
        let mut added = Vec::new();
        for (kind, defs) in extra.kinds {
            for (id, rd) in defs {
                if work.raw.get(&kind, &id).is_some() {
                    l.push(
                        &rd.file,
                        Issue::new("id", format!("`{id}` is already a {kind} def of the mod"))
                            .with_hint("give the variant an id of its own"),
                    );
                    continue;
                }
                added.push(id.clone());
                work.raw.insert(&kind, id, rd);
            }
        }
        l.issues.extend(work.resolve_all());
        l.issues.extend(work.cross_check());
        if !l.issues.is_empty() {
            return Err(ContentError::Invalid {
                origin: dir.display().to_string(),
                issues: l.issues,
            });
        }
        *self = work;
        Ok(added)
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
            toml::Value::Table(merged.clone())
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
        issues.extend(self.check_rules_refs(&def));
        if !issues.is_empty() {
            return Err(invalid(issues));
        }
        let id = def.id.clone();
        // S4: kept as a raw def too, so later overrides re-resolve it.
        self.raw.insert(
            "rules",
            id.clone(),
            RawDef {
                file: origin.clone(),
                src: None,
                table: merged,
            },
        );
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
        let before = self.clone();
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
                if !DEF_KINDS.contains(&kind.as_str()) || kind == "rules" {
                    return Err(invalid(vec![
                        Issue::new(format!("defs.{kind}"), format!("unknown def kind `{kind}`"))
                            .with_hint(
                                "one of units, weapons, items, factions, maps, vehicles, ammo, \
                             training (the rules are patched by top-level tables)",
                            ),
                    ]));
                }
                for (id, patch) in ids {
                    let at = format!("defs.{kind}.\"{id}\"");
                    let toml::Value::Table(patch) = patch else {
                        return Err(invalid(vec![Issue::new(at, "must be a table of keys")]));
                    };
                    self.patch_raw(&kind, &id, patch, &at).map_err(invalid)?;
                }
            }
        }
        if !table.is_empty() {
            if self.raw.get("rules", rules_id).is_none() {
                return Err(invalid(vec![Issue::new(
                    "",
                    format!(
                        "unknown rules `{rules_id}`{}",
                        did_you_mean(rules_id, self.rules.keys())
                    ),
                )]));
            }
            self.patch_raw("rules", rules_id, table, &format!("rules \"{rules_id}\""))
                .map_err(invalid)?;
        }
        let mut issues = self.resolve_all();
        issues.extend(self.cross_check());
        if !issues.is_empty() {
            return Err(invalid(issues));
        }
        Ok(self.changes_since(&before))
    }

    /// Merges a tuning override's `patch` onto the raw def `kind` / `id`
    /// (S4: as one more `extends` level, so `[[members]]` in a patch
    /// replaces a legacy squad's soldier keys). The def's issues are
    /// named `at` from then on.
    fn patch_raw(
        &mut self,
        kind: &str,
        id: &str,
        patch: toml::Table,
        at: &str,
    ) -> Result<(), Vec<Issue>> {
        for key in ["id", "extends", "abstract"] {
            if patch.contains_key(key) {
                return Err(vec![
                    Issue::new(
                        format!("{at}: {key}"),
                        "an override changes values, not a def's id or parent",
                    )
                    .with_hint(format!("remove `{key}`")),
                ]);
            }
        }
        let known = self.raw.ids(kind);
        let Some(rd) = self.raw.get_mut(kind, id) else {
            return Err(vec![Issue::new(
                at,
                format!("unknown id `{id}`{}", did_you_mean(id, known.iter())),
            )]);
        };
        let mut from = BTreeMap::new();
        merge_level(kind, &mut rd.table, &mut from, patch, "override").map_err(|e| {
            e.into_iter()
                .map(|i| Issue {
                    path: format!("{at}: {}", i.path),
                    ..i
                })
                .collect::<Vec<_>>()
        })?;
        rd.src = None;
        rd.file = at.to_owned();
        Ok(())
    }

    /// Every resolved value that differs from `before`: defs first (by
    /// kind and id), then rules. Canonical unit keys the loader fills
    /// (`members`, `tags`, `capabilities`, `cooldown_s`, `unlock_s`) are
    /// listed only where a file or patch sets them.
    fn changes_since(&self, before: &ModDefs) -> Vec<OverrideChange> {
        fn flat<T: Serialize>(d: Option<&T>) -> BTreeMap<String, String> {
            let mut out = BTreeMap::new();
            if let Some(d) = d
                && let Ok(toml::Value::Table(t)) = toml::Value::try_from(d)
            {
                flatten("", &t, &mut out);
            }
            out
        }
        fn diff<T: Serialize>(
            a: &BTreeMap<String, T>,
            b: &BTreeMap<String, T>,
            at: impl Fn(&str) -> String,
            keep: impl Fn(&str, &str) -> bool,
            out: &mut Vec<OverrideChange>,
        ) {
            let ids: std::collections::BTreeSet<&String> = a.keys().chain(b.keys()).collect();
            for id in ids {
                let (x, y) = (flat(a.get(id)), flat(b.get(id)));
                let keys: std::collections::BTreeSet<&String> = x.keys().chain(y.keys()).collect();
                for k in keys {
                    if x.get(k) != y.get(k) && keep(id, k) {
                        out.push(OverrideChange {
                            path: format!("{}: {k}", at(id)),
                            old: x.get(k).cloned(),
                            new: y.get(k).cloned(),
                        });
                    }
                }
            }
        }
        let mut out = Vec::new();
        let all = |_: &str, _: &str| true;
        let set_by_file = |id: &str, key: &str| {
            let top = key.split('.').next().unwrap_or_default();
            if !["members", "tags", "capabilities", "cooldown_s", "unlock_s"].contains(&top) {
                return true;
            }
            crate::resolve::resolve(&self.raw, "units", id).is_ok_and(|r| r.table.contains_key(top))
        };
        let at = |kind: &'static str| move |id: &str| format!("defs.{kind}.\"{id}\"");
        diff(
            &before.squads,
            &self.squads,
            at("units"),
            set_by_file,
            &mut out,
        );
        diff(&before.weapons, &self.weapons, at("weapons"), all, &mut out);
        diff(&before.items, &self.items, at("items"), all, &mut out);
        diff(
            &before.factions,
            &self.factions,
            at("factions"),
            all,
            &mut out,
        );
        diff(&before.maps, &self.maps, at("maps"), all, &mut out);
        diff(
            &before.vehicles,
            &self.vehicles,
            at("vehicles"),
            all,
            &mut out,
        );
        diff(&before.ammo, &self.ammo, at("ammo"), all, &mut out);
        diff(
            &before.training,
            &self.training,
            at("training"),
            all,
            &mut out,
        );
        diff(
            &before.cartridges,
            &self.cartridges,
            at("cartridges"),
            all,
            &mut out,
        );
        diff(&before.supply, &self.supply, at("supply"), all, &mut out);
        diff(
            &before.forts,
            &self.forts,
            at("fortifications"),
            all,
            &mut out,
        );
        diff(
            &before.rules,
            &self.rules,
            |id| format!("rules \"{id}\""),
            all,
            &mut out,
        );
        out
    }

    /// S4: a rules def's references (the training default).
    fn check_rules_refs(&self, r: &RulesDef) -> Vec<Issue> {
        let mut out = Vec::new();
        // S15 M1: every faction `[matchups]` names exists.
        if let Some(m) = &r.matchups {
            for (k, f) in m.factions.iter().enumerate() {
                if !self.factions.contains_key(f) {
                    out.push(Issue::new(
                        format!("matchups.factions[{k}]"),
                        format!("unknown faction `{f}`"),
                    ));
                }
            }
            for (k, pair) in m.pairs.iter().enumerate() {
                for f in pair {
                    if !self.factions.contains_key(f) {
                        out.push(Issue::new(
                            format!("matchups.pairs[{k}]"),
                            format!("unknown faction `{f}`"),
                        ));
                    }
                }
            }
        }
        if let Some(t) = &r.training
            && !self.training.contains_key(&t.default)
        {
            out.push(Issue::new(
                "training.default",
                format!(
                    "unknown training profile `{}`{}",
                    t.default,
                    did_you_mean(&t.default, self.training.keys())
                ),
            ));
        }
        out
    }

    /// References between defs: squads → factions, weapons, items and
    /// vehicles, faction rosters → squads, role/weapon agreement, at most
    /// one item of each kind per loadout, and vehicle mounts → weapons and
    /// ammo.
    pub fn cross_check(&self) -> Vec<Issue> {
        let mut out = Vec::new();
        // S11b G1: a rifle grenade fires as a launcher whose round is HE,
        // and is refilled as shells.
        for (id, it) in &self.items {
            if it.kind != crate::defs::ItemKind::RifleGrenade {
                continue;
            }
            let he_launcher = it
                .weapon
                .as_ref()
                .and_then(|w| self.weapons.get(w))
                .is_some_and(|w| {
                    w.class == WeaponClass::Launcher
                        && w.ammo
                            .as_ref()
                            .and_then(|a| self.ammo.get(a))
                            .is_some_and(|a| a.mechanism == AmmoMechanism::He && a.he.is_some())
                });
            if !he_launcher {
                out.push(
                    Issue::new(
                        format!("items/{}.toml: weapon", id.split_once(':').map_or(&id[..], |x| x.1)),
                        "a rifle_grenade item fires as a launcher weapon whose ammo is mechanism \"he\" with an [he] table",
                    )
                    .with_hint("weapon = \"ww2:m7_launcher\""),
                );
            }
            if it.supply.as_ref().is_some_and(|sp| sp.kind != "shells") {
                out.push(Issue::new(
                    format!(
                        "items/{}.toml: supply.kind",
                        id.split_once(':').map_or(&id[..], |x| x.1)
                    ),
                    "a rifle_grenade item is refilled as shells",
                ));
            }
        }
        // S10b: a minefield names a `mine` item.
        for (id, f) in &self.forts {
            if let Some(m) = &f.mine_item
                && self
                    .items
                    .get(m)
                    .is_none_or(|it| it.kind != crate::defs::ItemKind::Mine)
            {
                out.push(Issue::new(
                    format!("fortification `{id}`: mine_item"),
                    format!("`{m}` is not a mine item"),
                ));
            }
        }
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
            self.check_unit_refs(id, s, &mut out);
            if !s.is_legacy() || (s.weapon.is_empty() && !s.members.is_empty()) {
                // `[[members]]` cards: checked slot by slot above.
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
                            WeaponClass::Rifle
                                | WeaponClass::Smg
                                | WeaponClass::Mg
                                | WeaponClass::Pistol
                        ) =>
                    {
                        out.push(Issue::new(
                            format!("{}: sidearm", squad_file(id)),
                            "a sidearm is a rifle, SMG, MG or pistol",
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
                    Some(it) if it.kind == crate::defs::ItemKind::Mine => out.push(Issue::new(
                        format!("{}: loadout[{i}].item", squad_file(id)),
                        "a mine item is laid by a minefield blueprint, never carried in a kit",
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
                        let want = if m.mount.is_main() {
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
                        // S11a1: above +60 deg only with a low-angle table.
                        let low = w.indirect.as_ref().is_some_and(|i| i.low_angle);
                        let top = crate::vehicle::low_angle_top(low);
                        if let Some([_, hi]) = m.elevation_deg
                            && m.mount.is_main()
                            && hi > top
                        {
                            out.push(
                                Issue::new(
                                    format!("{p}.elevation_deg"),
                                    format!(
                                        "must be at most {top} deg: only a main weapon with a \
                                         `low_angle` [indirect] table elevates above 60, got {hi}"
                                    ),
                                )
                                .with_hint("add [indirect] low_angle = true to the weapon, or lower the limit"),
                            );
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
                // S3b: a second penetrator (a special round, e.g. APCR) only
                // with the HE round, so the indices stay standard 0, HE 1,
                // special 2.
                if kinds[1] > 1 || kinds[0] > 2 || (kinds[0] == 2 && kinds[1] == 0) {
                    out.push(Issue::new(
                        format!("{p}.ammo"),
                        "one armour-piercing round (ap, aphe, heat), at most one he round and, \
                         with it, at most one special armour-piercing round",
                    ));
                }
            }
        }
        for (id, r) in &self.rules {
            for i in self.check_rules_refs(r) {
                out.push(Issue {
                    path: format!("rules `{id}`: {}", i.path),
                    ..i
                });
            }
        }
        for (id, f) in &self.factions {
            for (i, s) in f.roster.iter().enumerate() {
                if self.squads.get(s).is_some_and(|sq| sq.is_abstract) {
                    out.push(
                        Issue::new(
                            format!("faction `{id}`: roster[{i}]"),
                            format!("squad `{s}` is abstract (a template for other units)"),
                        )
                        .with_hint("list a unit that extends it"),
                    );
                    continue;
                }
                match self.squads.get(s) {
                    None => out.push(Issue::new(
                        format!("faction `{id}`: roster[{i}]"),
                        format!("unknown squad `{s}`"),
                    )),
                    Some(sq) if sq.faction != *id => out.push(Issue::new(
                        format!("faction `{id}`: roster[{i}]"),
                        format!("squad `{s}` belongs to faction `{}`", sq.faction),
                    )),
                    Some(sq) if sq.role == SquadRole::Crew => out.push(Issue::new(
                        format!("faction `{id}`: roster[{i}]"),
                        format!("squad `{s}` is a crew (crews are not bought)"),
                    )),
                    Some(_) => {}
                }
            }
            for (i, s) in f.start_squads.iter().enumerate() {
                if self.squads.get(s).is_some_and(|sq| sq.is_abstract) {
                    out.push(Issue::new(
                        format!("faction `{id}`: start_squads[{i}]"),
                        format!("squad `{s}` is abstract (a template for other units)"),
                    ));
                }
                if self
                    .squads
                    .get(s)
                    .is_some_and(|sq| sq.role == SquadRole::Crew)
                {
                    out.push(Issue::new(
                        format!("faction `{id}`: start_squads[{i}]"),
                        format!("squad `{s}` is a crew (crews do not start on the field)"),
                    ));
                }
            }
        }
        // S9: a card's tow names a live card.
        for (id, s) in &self.squads {
            if let Some(t) = &s.tow
                && !self.squads.get(t).is_some_and(|q| q.vehicle.is_some())
            {
                out.push(Issue::new(
                    format!("squad `{id}`: tow"),
                    format!(
                        "unknown vehicle card `{t}`{}",
                        did_you_mean(t, self.squads.keys())
                    ),
                ));
            }
        }
        // S15 Fix: a card's tow seats the gun's crew (the crew rides with
        // the gun; a tow without seats leaves it to walk, S15 review 2).
        for (id, s) in &self.squads {
            let Some(t) = &s.tow else { continue };
            let crew = s
                .vehicle
                .as_ref()
                .and_then(|v| self.vehicles.get(v))
                .and_then(|g| g.crew_squad.as_ref())
                .and_then(|c| self.squads.get(c))
                .map(|c| c.members.iter().map(|m| u32::from(m.count)).sum::<u32>());
            let seats = self
                .squads
                .get(t)
                .and_then(|q| q.vehicle.as_ref())
                .and_then(|v| self.vehicles.get(v))
                .map(|tv| tv.transport.as_ref().map_or(0, |tr| tr.capacity));
            if let (Some(crew), Some(seats)) = (crew, seats)
                && seats < crew
            {
                out.push(Issue::new(
                    format!("squad `{id}`: tow"),
                    format!("the tow `{t}` seats {seats}, the gun's crew is {crew} men"),
                ));
            }
        }
        // S3c: a vehicle's crew squad is a crew of its faction.
        for (id, v) in &self.vehicles {
            if let Some(c) = &v.crew_squad {
                match self.squads.get(c) {
                    None => out.push(Issue::new(
                        format!("vehicle `{id}`: crew_squad"),
                        format!("unknown squad `{c}`{}", did_you_mean(c, self.squads.keys())),
                    )),
                    Some(sq)
                        if !(sq.role == SquadRole::Crew
                            || (v.crew_model == CrewModel::Served && sq.has_tag("gun_crew")))
                            || sq.faction != v.faction
                            || sq.is_abstract =>
                    {
                        out.push(Issue::new(
                            format!("vehicle `{id}`: crew_squad"),
                            format!("squad `{c}` is not a crew of faction `{}`", v.faction),
                        ))
                    }
                    Some(_) => {}
                }
            }
        }
        out.extend(self.check_supply());
        out.extend(self.check_stocks());
        out
    }

    /// S6 content checks: a supply def's faction exists, its cook-off
    /// round is an ammo def with a blast, its `built_by` tags are some
    /// unit's tags; at most one HQ, one dump and one crate per kind per
    /// faction.
    pub fn check_stocks(&self) -> Vec<Issue> {
        use crate::defs::StockKind;
        let mut out = Vec::new();
        let tags: std::collections::BTreeSet<&str> = self
            .squads
            .values()
            .flat_map(|s| s.tags.iter().map(|t| t.as_str()))
            .collect();
        let mut seen: std::collections::BTreeSet<(String, StockKind, String)> =
            std::collections::BTreeSet::new();
        for (id, d) in &self.supply {
            let at = |p: &str| format!("supply `{id}`: {p}");
            if !self.factions.contains_key(&d.faction) {
                out.push(Issue::new(
                    at("faction"),
                    format!(
                        "unknown faction `{}`{}",
                        d.faction,
                        did_you_mean(&d.faction, self.factions.keys())
                    ),
                ));
            }
            if let Some(c) = &d.cookoff {
                match self.ammo.get(c) {
                    Some(a) if a.he.is_some() => {}
                    Some(_) => out.push(Issue::new(
                        at("cookoff"),
                        format!("ammo `{c}` has no blast"),
                    )),
                    None => out.push(Issue::new(
                        at("cookoff"),
                        format!("unknown ammo `{c}`{}", did_you_mean(c, self.ammo.keys())),
                    )),
                }
            }
            for t in &d.built_by {
                if !tags.contains(t.as_str()) {
                    out.push(Issue::new(
                        at("built_by"),
                        format!("no unit has the tag `{t}`"),
                    ));
                }
            }
            let key = (
                d.faction.clone(),
                d.kind,
                d.supply.clone().unwrap_or_default(),
            );
            if !seen.insert(key) {
                out.push(Issue::new(
                    at("kind"),
                    "a faction has one def per kind (and one crate per supply kind)",
                ));
            }
        }
        out.extend(self.check_truck_supply());
        out
    }

    /// S8b: a truck's `[supply]` names crates of its own faction, one per
    /// kind, a crate for every kind it loads, and a cook-off round with a
    /// blast.
    pub fn check_truck_supply(&self) -> Vec<Issue> {
        use crate::defs::StockKind;
        let mut out = Vec::new();
        for (id, v) in &self.vehicles {
            let Some(t) = &v.supply else { continue };
            let at = |p: &str| format!("vehicle `{id}`: supply.{p}");
            match self.ammo.get(&t.cookoff) {
                Some(a) if a.he.is_some() => {}
                Some(_) => out.push(Issue::new(
                    at("cookoff"),
                    format!("ammo `{}` has no blast", t.cookoff),
                )),
                None => out.push(Issue::new(
                    at("cookoff"),
                    format!(
                        "unknown ammo `{}`{}",
                        t.cookoff,
                        did_you_mean(&t.cookoff, self.ammo.keys())
                    ),
                )),
            }
            let mut kinds: Vec<&str> = Vec::new();
            for c in &t.crates {
                let Some(d) = self.supply.get(c) else {
                    out.push(Issue::new(
                        at("crates"),
                        format!(
                            "unknown supply def `{c}`{}",
                            did_you_mean(c, self.supply.keys())
                        ),
                    ));
                    continue;
                };
                if d.kind != StockKind::Crate || d.faction != v.faction {
                    out.push(Issue::new(
                        at("crates"),
                        format!("`{c}` is not a crate of this vehicle's faction"),
                    ));
                }
                let kind = d.supply.as_deref().unwrap_or_default();
                if kinds.contains(&kind) {
                    out.push(Issue::new(at("crates"), "one crate def per supply kind"));
                }
                kinds.push(kind);
            }
            for (k, kg) in &t.load_kg {
                if *kg > 0 && !kinds.contains(&k.as_str()) {
                    out.push(Issue::new(
                        at("load_kg"),
                        format!("loads `{k}` but lists no crate of that kind"),
                    ));
                }
            }
        }
        out
    }

    /// S5a content checks (docs/modding/ammo.md): feeds name cartridges or
    /// ammo defs with `supply`, rare rounds are never a weapon's feed,
    /// member `rounds` name known feeds (at most 4 per unit), `rare_cap`
    /// stays within every rules def's `rare_cap_max` and holds the rare
    /// rounds stowed; in a mod with a `[supply]` table every carried
    /// weapon has a feed, every carried item and every launcher or main-gun
    /// round a `supply`, main-gun rounds are `shells`, and every vehicle
    /// sets `rare_cap`.
    pub fn check_supply(&self) -> Vec<Issue> {
        let mut out = Vec::new();
        let priced = self.rules.values().any(|r| r.supply.is_some());
        let cap_max = self
            .rules
            .values()
            .filter_map(|r| r.supply.as_ref().map(|s| (r.id.as_str(), s.rare_cap_max)))
            .min_by_key(|(_, c)| *c);
        let feed_ok = |id: &str| -> Result<bool, String> {
            if let Some(c) = self.cartridges.get(id) {
                let _ = c;
                return Ok(false);
            }
            match self.ammo.get(id) {
                Some(a) if a.supply.is_none() => Err(format!(
                    "ammo `{id}` has no `supply` table; add supply = {{ kind = \"...\", g = ... }}"
                )),
                Some(a) => Ok(a.rare),
                None => Err(format!(
                    "unknown cartridge or ammo `{id}`{}",
                    did_you_mean(id, self.cartridges.keys().chain(self.ammo.keys()))
                )),
            }
        };
        let small_arms: Vec<&String> = self
            .cartridges
            .iter()
            .filter(|(_, c)| c.supply.kind == "small_arms")
            .map(|(k, _)| k)
            .collect();
        for (id, w) in &self.weapons {
            // S15 X1: the foreign feed names a cartridge and a faction.
            if let Some(f) = &w.foreign_feed {
                if !self.cartridges.contains_key(&f.cartridge) {
                    out.push(Issue::new(
                        format!("weapon `{id}`: foreign_feed.cartridge"),
                        format!(
                            "unknown cartridge `{}`{}",
                            f.cartridge,
                            did_you_mean(&f.cartridge, self.cartridges.keys())
                        ),
                    ));
                }
                if !self.factions.contains_key(&f.faction) {
                    out.push(Issue::new(
                        format!("weapon `{id}`: foreign_feed.faction"),
                        format!(
                            "unknown faction `{}`{}",
                            f.faction,
                            did_you_mean(&f.faction, self.factions.keys())
                        ),
                    ));
                }
            }
            if let Some(f) = &w.feed {
                match feed_ok(f) {
                    Err(e) => out.push(Issue::new(format!("weapon `{id}`: feed"), e)),
                    Ok(true) => out.push(
                        Issue::new(
                            format!("weapon `{id}`: feed"),
                            format!("`{f}` is a rare round (D3); it is never a weapon's only feed"),
                        )
                        .with_hint("feed the standard round; rare rounds go in a vehicle's rack"),
                    ),
                    Ok(false) => {}
                }
            }
        }
        // Weapons soldiers carry and vehicles mount as MGs.
        let mut carried: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
        let mut items_carried: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
        for (uid, s) in &self.squads {
            if s.is_abstract {
                continue;
            }
            let mut feeds: Vec<String> = Vec::new();
            for (k, m) in s.members.iter().enumerate() {
                carried.insert(m.weapon.as_str());
                if let Some(sa) = &m.sidearm {
                    carried.insert(sa.as_str());
                }
                for (it, &n) in &m.items {
                    if n > 0 {
                        items_carried.insert(it.as_str());
                    }
                }
                for w in [Some(&m.weapon), m.sidearm.as_ref()].into_iter().flatten() {
                    if let Some(f) = self.weapons.get(w).and_then(|w| w.feed.clone())
                        && !feeds.contains(&f)
                    {
                        feeds.push(f);
                    }
                }
                for rid in m.rounds.keys() {
                    if let Err(e) = feed_ok(rid) {
                        out.push(Issue::new(
                            format!("squad `{uid}`: members[{k}].rounds.\"{rid}\""),
                            e,
                        ));
                    } else if !feeds.contains(rid) {
                        feeds.push(rid.clone());
                    }
                }
            }
            if feeds.len() > crate::setup::MAX_FEEDS {
                out.push(
                    Issue::new(
                        format!("squad `{uid}`: members"),
                        format!(
                            "uses {} different rounds ({}); at most {} per unit",
                            feeds.len(),
                            feeds.join(", "),
                            crate::setup::MAX_FEEDS
                        ),
                    )
                    .with_hint("give the men weapons that share rounds, or split the unit"),
                );
            }
        }
        for v in self.vehicles.values() {
            for m in v.weapons.iter().filter(|m| !m.mount.is_main()) {
                carried.insert(m.def.as_str());
            }
        }
        for (vid, v) in &self.vehicles {
            let at = |p: &str| format!("vehicle `{vid}`: {p}");
            let main = v.main_gun();
            let rare_n: u32 = main.map_or(0, |m| {
                m.ammo
                    .iter()
                    .filter(|a| self.ammo.get(&a.id).is_some_and(|d| d.rare))
                    .map(|a| a.count)
                    .sum()
            });
            if let (Some(c), Some((rid, mx))) = (v.rare_cap, cap_max)
                && c > mx
            {
                out.push(
                    Issue::new(
                        at("rare_cap"),
                        format!(
                            "rare_cap {c} is above the rules' supply.rare_cap_max ({mx}, rules `{rid}`)"
                        ),
                    )
                    .with_hint(format!(
                        "lower rare_cap to at most {mx}, or raise [supply] rare_cap_max in defs/rules"
                    )),
                );
            }
            let cap = v.rare_cap.unwrap_or(if rare_n > 0 {
                cap_max.map_or(10, |x| x.1)
            } else {
                0
            });
            if rare_n > cap {
                out.push(
                    Issue::new(
                        at("weapons[0].ammo"),
                        format!("{rare_n} rare rounds stowed, above rare_cap {cap}"),
                    )
                    .with_hint(format!("stow at most {cap} rare rounds, or raise rare_cap")),
                );
            }
            if priced {
                // S5a Fix (review L8): an MG mount without `rounds` would
                // fire without limit in a priced mod.
                for (k, m) in v.weapons.iter().enumerate() {
                    if !m.mount.is_main() && m.rounds.is_none() {
                        out.push(
                            Issue::new(
                                at(&format!("weapons[{k}].rounds")),
                                "required on coax and hull MG mounts in a mod with a [supply] table (S5a: every round counted)",
                            )
                            .with_hint("rounds = 3000"),
                        );
                    }
                }
                if v.rare_cap.is_none() {
                    out.push(
                        Issue::new(
                            at("rare_cap"),
                            "required in a mod with a [supply] table (D3: the most rare rounds it holds)",
                        )
                        .with_hint(format!("rare_cap = {rare_n}")),
                    );
                }
                for (k, a) in main.map_or(&[][..], |m| &m.ammo[..]).iter().enumerate() {
                    match self.ammo.get(&a.id).and_then(|d| d.supply.as_ref()) {
                        None => out.push(
                            Issue::new(
                                at(&format!("weapons[0].ammo[{k}]")),
                                format!("ammo `{}` has no `supply` table", a.id),
                            )
                            .with_hint("supply = { kind = \"shells\", g = 9000 }"),
                        ),
                        Some(sp) if sp.kind != "shells" => out.push(Issue::new(
                            at(&format!("weapons[0].ammo[{k}]")),
                            format!(
                                "ammo `{}`: a vehicle main gun's rounds are `shells` (no cheap tank rounds), got `{}`",
                                a.id, sp.kind
                            ),
                        )),
                        Some(_) => {}
                    }
                }
            }
        }
        if priced {
            for w in &carried {
                let Some(wd) = self.weapons.get(*w) else {
                    continue;
                };
                match wd.class {
                    WeaponClass::Launcher => {
                        if let Some(a) = &wd.ammo
                            && self.ammo.get(a).is_some_and(|d| d.supply.is_none())
                        {
                            out.push(
                                Issue::new(
                                    format!("ammo `{a}`: supply"),
                                    format!("the round of `{w}` needs a `supply` table (priced refill)"),
                                )
                                .with_hint("supply = { kind = \"explosives\", g = 3250 }"),
                            );
                        }
                    }
                    WeaponClass::Cannon => {}
                    _ if wd.feed.is_none() => out.push(
                        Issue::new(
                            format!("weapon `{w}`: feed"),
                            format!(
                                "{w} fires counted rounds since S5a: add `feed = \"<cartridge id>\"` (cartridges of kind small_arms: {})",
                                small_arms
                                    .iter()
                                    .map(|s| s.as_str())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            ),
                        )
                        .with_hint("feed = \"ww2:792x57_clip\""),
                    ),
                    _ => {}
                }
            }
            for it in &items_carried {
                if self.items.get(*it).is_some_and(|d| d.supply.is_none()) {
                    out.push(
                        Issue::new(
                            format!("item `{it}`: supply"),
                            "carried items need a `supply` table in a mod with [supply] (priced refill)",
                        )
                        .with_hint("supply = { kind = \"explosives\", g = 600 }"),
                    );
                }
            }
        }
        out
    }
}

impl ModDefs {
    /// S4 references of a unit card: training profile, limit group, and
    /// for `[[members]]` cards each slot's weapon, sidearm and items.
    fn check_unit_refs(&self, id: &str, s: &SquadDef, out: &mut Vec<Issue>) {
        let at = |p: String| format!("squad `{id}`: {p}");
        if let Some(t) = &s.training
            && !self.training.contains_key(t)
        {
            out.push(Issue::new(
                at("training".into()),
                format!(
                    "unknown training profile `{t}`{}",
                    did_you_mean(t, self.training.keys())
                ),
            ));
        }
        if let Some(g) = &s.limit_group
            && !self.rules.values().any(|r| r.limit_groups.contains_key(g))
        {
            out.push(
                Issue::new(
                    at("limit_group".into()),
                    format!("no rules def has the limit group `{g}`"),
                )
                .with_hint(format!("[limit_groups] \"{g}\" = 2 in the rules")),
            );
        }
        if s.is_legacy() {
            return;
        }
        let mut mortar = false;
        let mut launcher = false;
        let mut launcher_men = 0u32;
        for (i, m) in s.members.iter().enumerate() {
            let p = format!("members[{i}]");
            if self
                .weapons
                .get(&m.weapon)
                .is_some_and(|w| w.class == WeaponClass::Launcher)
            {
                launcher_men += m.count as u32;
            }
            match self.weapons.get(&m.weapon) {
                None => out.push(Issue::new(
                    at(format!("{p}.weapon")),
                    format!(
                        "unknown weapon `{}`{}",
                        m.weapon,
                        did_you_mean(&m.weapon, self.weapons.keys())
                    ),
                )),
                Some(w) if w.class == WeaponClass::Cannon => out.push(Issue::new(
                    at(format!("{p}.weapon")),
                    "a cannon is a vehicle weapon",
                )),
                Some(w) => {
                    mortar |= w.class == WeaponClass::Mortar;
                    launcher |= w.class == WeaponClass::Launcher;
                }
            }
            if let Some(side) = &m.sidearm {
                match self.weapons.get(side) {
                    None => out.push(Issue::new(
                        at(format!("{p}.sidearm")),
                        format!(
                            "unknown weapon `{side}`{}",
                            did_you_mean(side, self.weapons.keys())
                        ),
                    )),
                    Some(w)
                        if !matches!(
                            w.class,
                            WeaponClass::Rifle
                                | WeaponClass::Smg
                                | WeaponClass::Mg
                                | WeaponClass::Pistol
                        ) =>
                    {
                        out.push(Issue::new(
                            at(format!("{p}.sidearm")),
                            "a sidearm is a rifle, SMG, MG or pistol",
                        ))
                    }
                    Some(_) => {}
                }
            }
            let mut kinds = [false; ITEM_KINDS];
            for (item, &n) in &m.items {
                match self.items.get(item) {
                    None => out.push(Issue::new(
                        at(format!("{p}.items.\"{item}\"")),
                        format!(
                            "unknown item `{item}`{}",
                            did_you_mean(item, self.items.keys())
                        ),
                    )),
                    Some(it) if it.kind == crate::defs::ItemKind::Mine => out.push(Issue::new(
                        at(format!("{p}.items.\"{item}\"")),
                        "a mine item is laid by a minefield blueprint, never carried in a kit",
                    )),
                    Some(it) if n > 0 && kinds[it.kind.index()] => out.push(Issue::new(
                        at(format!("{p}.items.\"{item}\"")),
                        format!(
                            "a second {:?} item; a man carries one of each kind",
                            it.kind
                        ),
                    )),
                    Some(it) => kinds[it.kind.index()] |= n > 0,
                }
            }
        }
        if s.role == SquadRole::Mortar && !mortar {
            out.push(Issue::new(
                at("members".into()),
                "a mortar squad needs a member with a mortar weapon",
            ));
        }
        // S4 review M3: the sim fires a squad's launcher through one
        // carrier, and only when the launcher is the main weapon (the first
        // crew weapon among the members, else the leader's weapon).
        if launcher {
            // S14: a flamethrower is never the main weapon (setup.rs).
            let crew = |w: &str| {
                self.weapons
                    .get(w)
                    .is_some_and(|d| d.is_crew_weapon() && d.class != WeaponClass::Flame)
            };
            let main = s
                .members
                .iter()
                .map(|m| m.weapon.as_str())
                .find(|w| crew(w))
                .or_else(|| s.members.first().map(|m| m.weapon.as_str()));
            let main_launcher = main
                .and_then(|w| self.weapons.get(w))
                .is_some_and(|w| w.class == WeaponClass::Launcher);
            if launcher_men > 1 {
                out.push(
                    Issue::new(
                        at("members".into()),
                        format!(
                            "{launcher_men} men carry a launcher; a squad has one launcher \
                             carrier (the others would never fire)"
                        ),
                    )
                    .with_hint("give one member the launcher and the rest a rifle or pistol"),
                );
            }
            if !main_launcher {
                out.push(
                    Issue::new(
                        at("members".into()),
                        "a launcher must be the squad's main weapon (the first crew weapon among \
                         the members); here another crew weapon comes first, so the launcher \
                         man would never fire",
                    )
                    .with_hint(
                        "move the launcher slot before the MG or mortar slot, or use a separate \
                         AT team",
                    ),
                );
            }
            if s.has_tag("crew") {
                out.push(
                    Issue::new(
                        at("members".into()),
                        "a crew squad cannot carry a launcher (crews that bail out start \
                         without launcher state)",
                    )
                    .with_hint("remove the launcher from the crew card"),
                );
            }
        }
        // S15 Build B: an anti-tank rifle (a crew weapon of class rifle with
        // a `penetration` table, the PTRD-41) is the team's weapon in place
        // of a launcher.
        let at_rifle = s.members.iter().any(|m| {
            self.weapons.get(&m.weapon).is_some_and(|w| {
                w.class == WeaponClass::Rifle && w.is_crew_weapon() && !w.penetration.is_empty()
            })
        });
        if s.role == SquadRole::AntiTank && !launcher && !at_rifle {
            out.push(Issue::new(
                at("members".into()),
                "an anti-tank team (tag `at`) needs a member with a launcher or an anti-tank rifle",
            ));
        }
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

/// `" (did you mean `x`?)"` for the closest known id, or `""` when nothing
/// is close (§8.2: actionable errors).
pub fn did_you_mean<'a>(id: &str, known: impl Iterator<Item = &'a String>) -> String {
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
