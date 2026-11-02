//! S4 variant system: the parts of a unit card beyond the S1-S3 squad
//! fields (`docs/slices/S4-variants.md`, "Data model").
//!
//! * [`MemberDef`]: `[[members]]` slots, per-man weapon, items, ammo and
//!   traits. Legacy squads (`men`, `weapon`, `weapon_men`, `sidearm`,
//!   `loadout`) expand into slots ([`expand_legacy`]); a slot list that
//!   today's sim can run projects back onto the legacy fields
//!   ([`project_members`]).
//! * Capabilities ([`CAPABILITIES`]) are engine meanings that gate orders
//!   and sim rules; tags are free labels for the AI mix and HUD filters,
//!   four of them reserved ([`RESERVED_TAGS`]).
//! * [`TrainingDef`]: training profiles (`defs/training/*.toml`).
//!
//! S4 Part A: the content crate parses, validates and resolves all of this;
//! the sim still runs today's `SquadTypeSetup`, so `ModDefs::resolve`
//! refuses a card it cannot express there ("needs S4 Part B").

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::defs::{LoadoutDef, MAX_ITEM_COUNT, MAX_SQUAD_MEN, SquadDef, SquadRole};
use crate::issue::Issue;
use crate::modset::did_you_mean;

/// One slot of a squad: `count` men with the same weapon, items and traits.
/// Slots are in spawn and formation order; the first man of the first slot
/// is the squad leader.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MemberDef {
    /// Free name shown on the squad card (`leader`, `mg_gunner`,
    /// `rifleman`): lower-case letters, digits and `_`, 1..=24 characters.
    pub slot: String,
    /// Men in this slot, 1..=16; all slots together 1..=16.
    #[serde(default = "one")]
    pub count: u8,
    /// Weapon id each man of the slot carries.
    pub weapon: String,
    /// Weapon he fights with after handing a crew weapon on (a rifle, SMG,
    /// MG or pistol). S4 Part B (crew-weapon pickup).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sidearm: Option<String>,
    /// S4 form of `rounds`: rounds of his weapon's feed he carries (S5a:
    /// read as `rounds = { <his weapon's feed> = N }`). Not with `rounds`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ammo: Option<u32>,
    /// S5a: rounds each man of the slot carries, by cartridge or ammo id,
    /// counting those in his weapon, e.g. `{ "ww2:792x57_clip" = 60 }`.
    /// A feed of his weapon (or sidearm) not listed gets that weapon's
    /// `load`. At most 4 feeds per unit; each 0..=65535.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub rounds: BTreeMap<String, u32>,
    /// Items each man of the slot carries, by item id, e.g.
    /// `{ "ww2:stielhandgranate" = 1 }`; at most one item of each kind per
    /// man, at most 10 of one item.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub items: BTreeMap<String, u8>,
    /// S4 traits: `leader` (first slot only), `crew_gunner`,
    /// `crew_assistant`, `ammo_bearer`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub traits: Vec<String>,
}

fn one() -> u8 {
    1
}

/// Traits a member may have in S4.
pub const TRAITS: [&str; 4] = ["leader", "crew_gunner", "crew_assistant", "ammo_bearer"];
/// Traits reserved for later slices (refused until then).
pub const RESERVED_TRAITS: [&str; 4] = ["officer", "radio", "medic", "sniper"];

/// Capabilities in S4, in canonical order.
pub const CAPABILITIES: [&str; 6] = [
    "dig_basic",
    "dig_full",
    "barrage",
    "recrew",
    "vehicle_crew",
    "at_hunt",
];
/// Capabilities the content crate derives from the members' weapons
/// (setting them is allowed only when the weapons give them).
pub const DERIVED_CAPABILITIES: [&str; 2] = ["barrage", "at_hunt"];
/// Capabilities reserved for later slices.
pub const RESERVED_CAPABILITIES: [&str; 14] = [
    "build_structure",
    "build_heavy",
    "place_wire",
    "clear_wire",
    "place_mines",
    "clear_mines",
    "breach",
    "demolish",
    "observe",
    "radio",
    "medic",
    "gun_crew",
    "issue_crates",
    "flame",
];

/// Tags with an engine meaning: `vehicle` (must equal "has a vehicle"),
/// `crew` (never in a roster or start squads), `at` (AI gate after the
/// vehicle unlock) and `understrength` (HUD marker).
pub const RESERVED_TAGS: [&str; 4] = ["vehicle", "crew", "at", "understrength"];
/// Most tags per card.
pub const MAX_TAGS: usize = 8;
/// Longest tag.
pub const MAX_TAG_LEN: usize = 24;
/// Longest HUD tile name (`short_name`).
pub const MAX_SHORT_NAME: usize = 14;
/// Most squads of one card alive at once (`max_alive`).
pub const MAX_ALIVE_CAP: u16 = 100;
/// Default AI weight of a card among ready cards of its mix tag.
pub const DEFAULT_AI_WEIGHT: u32 = 100;

/// The legacy role's tag (`role` fills `tags` when they are left empty).
pub fn role_tag(role: SquadRole) -> &'static str {
    match role {
        SquadRole::Rifle => "line",
        SquadRole::Mg => "mg",
        SquadRole::Engineer => "engineer",
        SquadRole::Mortar => "mortar",
        SquadRole::AntiTank => "at",
        SquadRole::Vehicle => "vehicle",
        SquadRole::Crew => "crew",
    }
}

/// The legacy role of a tag list (S4 Part A lowering and the presenter's
/// `type`): the first match in the order crew, vehicle, at, mortar,
/// engineer, mg, line.
pub fn role_of_tags(tags: &[String]) -> Option<SquadRole> {
    [
        SquadRole::Crew,
        SquadRole::Vehicle,
        SquadRole::AntiTank,
        SquadRole::Mortar,
        SquadRole::Engineer,
        SquadRole::Mg,
        SquadRole::Rifle,
    ]
    .into_iter()
    .find(|&r| tags.iter().any(|t| t == role_tag(r)))
}

/// Capabilities a legacy role gives (the non-derived ones), canonical
/// order. `dig_basic` needs `dig_rate_pct > 0`.
pub fn role_capabilities(role: SquadRole, dig_rate_pct: u32) -> Vec<String> {
    let mut out = Vec::new();
    let infantry = !matches!(role, SquadRole::Vehicle);
    if infantry && dig_rate_pct > 0 {
        out.push("dig_basic");
    }
    if role == SquadRole::Engineer {
        out.push("dig_full");
    }
    if matches!(
        role,
        SquadRole::Rifle | SquadRole::Mg | SquadRole::Engineer | SquadRole::Mortar
    ) {
        out.push("recrew");
    }
    if role == SquadRole::Crew {
        out.push("vehicle_crew");
    }
    out.into_iter().map(String::from).collect()
}

/// Sorts capabilities into [`CAPABILITIES`] order and drops duplicates.
pub fn canonical_capabilities(caps: &mut Vec<String>) {
    caps.sort_by_key(|c| {
        CAPABILITIES
            .iter()
            .position(|k| k == c)
            .unwrap_or(CAPABILITIES.len())
    });
    caps.dedup();
}

/// The `[[members]]` a legacy squad expands to (exact rule of the spec):
/// `leader` (1 man: `weapon` if `weapon_men >= 1`, else `sidearm`; each
/// loadout item's `leader_count`), `weapon` (`weapon_men - 1` men with
/// `weapon`; each item's `count`) and `sidearm` (`men - weapon_men` men
/// with `sidearm`; each item's `count`). Empty slots and zero item counts
/// are left out.
pub fn expand_legacy(s: &SquadDef, launcher: bool) -> Vec<MemberDef> {
    let n = s.men;
    if n == 0 {
        return Vec::new();
    }
    let k = s.weapon_men.unwrap_or(n).min(n);
    // A launcher squad has one carrier (S3 `weapon_of`; S4 review M3).
    let k = if launcher { k.min(1) } else { k };
    let side = s.sidearm.clone().unwrap_or_default();
    let items = |leader: bool| -> BTreeMap<String, u8> {
        s.loadout
            .iter()
            .map(|l| {
                let c = if leader { l.leader_count } else { l.count };
                (l.item.clone(), c)
            })
            .filter(|&(_, c)| c > 0)
            .collect()
    };
    let slot = |name: &str, count: u8, weapon: String, leader: bool| MemberDef {
        slot: name.into(),
        count,
        weapon,
        sidearm: None,
        ammo: None,
        rounds: BTreeMap::new(),
        items: items(leader),
        traits: Vec::new(),
    };
    let lead_weapon = if k >= 1 {
        s.weapon.clone()
    } else {
        side.clone()
    };
    let mut out = vec![slot("leader", 1, lead_weapon, true)];
    if k > 1 {
        out.push(slot("weapon", k - 1, s.weapon.clone(), false));
    }
    if n > k.max(1) {
        out.push(slot("sidearm", n - k.max(1), side, false));
    }
    out
}

/// A slot list in the shape today's `SquadTypeSetup` runs: `men`, the
/// first `weapon_men` men with `weapon`, the rest with `sidearm`, leader
/// and per-man item counts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LegacySoldiers {
    pub men: u8,
    pub weapon: String,
    pub weapon_men: u8,
    pub sidearm: Option<String>,
    /// Sorted by item id.
    pub loadout: Vec<LoadoutDef>,
}

/// Projects `[[members]]` onto the legacy fields, or says why today's sim
/// cannot run them (S4 Part A; the reasons end in "needs S4 Part B").
pub fn project_members(members: &[MemberDef]) -> Result<LegacySoldiers, String> {
    let mut men: Vec<&MemberDef> = Vec::new();
    for m in members {
        if m.sidearm.is_some() {
            return Err(format!(
                "slot `{}` sets a sidearm (crew-weapon hand-over)",
                m.slot
            ));
        }
        for _ in 0..m.count {
            men.push(m);
        }
    }
    let Some(first) = men.first() else {
        return Err("no members".into());
    };
    let weapon = first.weapon.clone();
    let k = men.iter().take_while(|m| m.weapon == weapon).count();
    let sidearm = men.get(k).map(|m| m.weapon.clone());
    if let Some(s) = &sidearm
        && let Some(odd) = men[k..].iter().find(|m| m.weapon != *s)
    {
        return Err(format!(
            "slot `{}` carries `{}`: more than two weapons, or `{weapon}` after `{s}` \
             (today's squads carry one weapon, then one sidearm)",
            odd.slot, odd.weapon
        ));
    }
    let rest = &men[1..];
    if let Some(odd) = rest.iter().find(|m| m.items != rest[0].items) {
        return Err(format!(
            "slot `{}` carries other items than slot `{}` (today every man but the \
             leader carries the same items)",
            odd.slot, rest[0].slot
        ));
    }
    let mut ids: Vec<&String> = first.items.keys().collect();
    if let Some(r) = rest.first() {
        ids.extend(r.items.keys());
    }
    ids.sort();
    ids.dedup();
    let loadout = ids
        .into_iter()
        .map(|id| LoadoutDef {
            item: id.clone(),
            count: rest
                .first()
                .and_then(|r| r.items.get(id))
                .copied()
                .unwrap_or(0),
            leader_count: first.items.get(id).copied().unwrap_or(0),
        })
        .collect();
    Ok(LegacySoldiers {
        men: men.len() as u8,
        weapon,
        weapon_men: k as u8,
        sidearm,
        loadout,
    })
}

/// Lower-case letters, digits and `_`, 1..=`max` characters.
pub(crate) fn is_label(s: &str, max: usize) -> bool {
    !s.is_empty()
        && s.len() <= max
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// A `"YYYY-MM"` month as `year * 12 + month - 1`.
pub fn parse_month(s: &str) -> Option<u32> {
    let (y, m) = s.split_once('-')?;
    if y.len() != 4 || m.len() != 2 {
        return None;
    }
    let y: u32 = y.parse().ok()?;
    let m: u32 = m.parse().ok()?;
    (1..=12).contains(&m).then_some(y * 12 + m - 1)
}

/// Per-def checks of `[[members]]` (cross-file references are checked by
/// `ModDefs::cross_check`).
pub(crate) fn check_members(out: &mut Vec<Issue>, members: &[MemberDef]) {
    let mut total = 0u32;
    for (i, m) in members.iter().enumerate() {
        let p = format!("members[{i}]");
        if !is_label(&m.slot, MAX_TAG_LEN) {
            out.push(
                Issue::new(
                    format!("{p}.slot"),
                    format!(
                        "`{}` is not a slot name (lower-case letters, digits, `_`, 1..=24)",
                        m.slot
                    ),
                )
                .with_hint("slot = \"rifleman\""),
            );
        }
        if members[..i].iter().any(|o| o.slot == m.slot) {
            out.push(Issue::new(
                format!("{p}.slot"),
                format!(
                    "slot `{}` is listed twice; raise its count or rename one",
                    m.slot
                ),
            ));
        }
        if !(1..=MAX_SQUAD_MEN).contains(&m.count) {
            out.push(Issue::new(
                format!("{p}.count"),
                format!("must be in 1..={MAX_SQUAD_MEN}, got {}", m.count),
            ));
        }
        total += m.count as u32;
        if m.weapon.trim().is_empty() {
            out.push(
                Issue::new(format!("{p}.weapon"), "required: the weapon id he carries")
                    .with_hint("weapon = \"ww2:kar98k\""),
            );
        }
        if let Some(a) = m.ammo
            && a > 10_000
        {
            out.push(Issue::new(
                format!("{p}.ammo"),
                format!("must be in 0..=10000, got {a}"),
            ));
        }
        if m.ammo.is_some() && !m.rounds.is_empty() {
            out.push(
                Issue::new(
                    format!("{p}.ammo"),
                    "set `rounds` (S5a, by cartridge id) or the S4 `ammo`, not both",
                )
                .with_hint("remove `ammo` and list the weapon's feed in `rounds`"),
            );
        }
        for (id, &n) in &m.rounds {
            if n > u16::MAX as u32 {
                out.push(Issue::new(
                    format!("{p}.rounds.\"{id}\""),
                    format!("must be in 0..=65535, got {n}"),
                ));
            }
        }
        for (id, &c) in &m.items {
            if c > MAX_ITEM_COUNT {
                out.push(
                    Issue::new(
                        format!("{p}.items.\"{id}\""),
                        format!("must be in 0..={MAX_ITEM_COUNT}, got {c}"),
                    )
                    .with_hint(format!("\"{id}\" = {MAX_ITEM_COUNT}")),
                );
            }
        }
        for (k, t) in m.traits.iter().enumerate() {
            let tp = format!("{p}.traits[{k}]");
            if m.traits[..k].contains(t) {
                out.push(Issue::new(tp, format!("`{t}` is listed twice")));
            } else if RESERVED_TRAITS.contains(&t.as_str()) {
                out.push(
                    Issue::new(
                        tp,
                        format!("trait `{t}` is reserved for a later slice and has no effect yet"),
                    )
                    .with_hint(format!("remove `{t}` (S4 traits: {})", TRAITS.join(", "))),
                );
            } else if !TRAITS.contains(&t.as_str()) {
                let known: Vec<String> = TRAITS.iter().map(|s| s.to_string()).collect();
                out.push(
                    Issue::new(
                        tp,
                        format!("unknown trait `{t}`{}", did_you_mean(t, known.iter())),
                    )
                    .with_hint(format!("one of {}", TRAITS.join(", "))),
                );
            } else if t == "leader" && (i != 0 || m.count != 1) {
                out.push(
                    Issue::new(
                        tp,
                        "the squad leader is the first man of the first slot: a `leader` \
                         slot comes first and has count = 1",
                    )
                    .with_hint("move this slot to the top of [[members]] with count = 1"),
                );
            }
        }
    }
    if !members.is_empty() && !(1..=MAX_SQUAD_MEN as u32).contains(&total) {
        out.push(Issue::new(
            "members",
            format!("the slots hold {total} men; a squad has 1..={MAX_SQUAD_MEN}"),
        ));
    }
}

/// Per-def checks of the card's tags and capabilities.
pub(crate) fn check_labels(out: &mut Vec<Issue>, tags: &[String], caps: &[String]) {
    if tags.len() > MAX_TAGS {
        out.push(Issue::new(
            "tags",
            format!("{} tags; at most {MAX_TAGS}", tags.len()),
        ));
    }
    for (i, t) in tags.iter().enumerate() {
        if !is_label(t, MAX_TAG_LEN) {
            out.push(
                Issue::new(
                    format!("tags[{i}]"),
                    format!(
                        "`{t}` is not a tag (lower-case letters, digits, `_`, 1..={MAX_TAG_LEN})"
                    ),
                )
                .with_hint("tags = [\"line\"]"),
            );
        } else if tags[..i].contains(t) {
            out.push(Issue::new(
                format!("tags[{i}]"),
                format!("`{t}` is listed twice"),
            ));
        }
    }
    let known: Vec<String> = CAPABILITIES.iter().map(|s| s.to_string()).collect();
    for (i, c) in caps.iter().enumerate() {
        let p = format!("capabilities[{i}]");
        if caps[..i].contains(c) {
            out.push(Issue::new(p, format!("`{c}` is listed twice")));
        } else if RESERVED_CAPABILITIES.contains(&c.as_str()) {
            out.push(
                Issue::new(
                    p,
                    format!("capability `{c}` is reserved for a later slice and has no effect yet"),
                )
                .with_hint(format!(
                    "remove `{c}` (S4 capabilities: {})",
                    CAPABILITIES.join(", ")
                )),
            );
        } else if !CAPABILITIES.contains(&c.as_str()) {
            out.push(
                Issue::new(
                    p,
                    format!("unknown capability `{c}`{}", did_you_mean(c, known.iter())),
                )
                .with_hint(format!("one of {}", CAPABILITIES.join(", "))),
            );
        }
    }
}

/// A training profile (`defs/training/*.toml`, S4). It changes the man,
/// never the weapon; every value is a percentage of the regular soldier's
/// (100 = no change). No in-match veterancy: a card's profile is fixed.
/// S4 Part A: parsed and validated; the sim applies it from Part B on
/// (until then a card with a non-regular profile is refused at resolve).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TrainingDef {
    /// Namespaced id, e.g. `ww2:training_regular`.
    pub id: String,
    pub name: String,
    /// Aim error: multiplies the weapon's spread (%).
    #[serde(default = "hundred")]
    pub spread_pct: u32,
    /// Time to open fire on a new target (%).
    #[serde(default = "hundred")]
    pub aim_time_pct: u32,
    /// Reload time (%).
    #[serde(default = "hundred")]
    pub reload_pct: u32,
    /// Scatter of thrown items (%).
    #[serde(default = "hundred")]
    pub throw_scatter_pct: u32,
    /// Multiplies the card's `steadiness_pct` (duck and pin thresholds) (%).
    #[serde(default = "hundred")]
    pub steadiness_pct: u32,
    /// Suppression decay per second (%).
    #[serde(default = "hundred")]
    pub suppression_decay_pct: u32,
    /// The men's sight range (%).
    #[serde(default = "hundred")]
    pub spot_pct: u32,
    /// Multiplies `dig_rate_pct` (%).
    #[serde(default = "hundred")]
    pub build_rate_pct: u32,
}

fn hundred() -> u32 {
    100
}

impl TrainingDef {
    /// Every percentage with its key and allowed range.
    pub fn values(&self) -> [(&'static str, u32, u32, u32); 8] {
        [
            ("spread_pct", self.spread_pct, 25, 400),
            ("aim_time_pct", self.aim_time_pct, 25, 400),
            ("reload_pct", self.reload_pct, 25, 400),
            ("throw_scatter_pct", self.throw_scatter_pct, 25, 400),
            ("steadiness_pct", self.steadiness_pct, 25, 400),
            ("suppression_decay_pct", self.suppression_decay_pct, 25, 400),
            ("spot_pct", self.spot_pct, 50, 200),
            ("build_rate_pct", self.build_rate_pct, 25, 400),
        ]
    }

    /// True when every value is 100 (a regular soldier).
    pub fn is_regular(&self) -> bool {
        self.values().iter().all(|v| v.1 == 100)
    }

    pub fn validate(&self) -> Vec<Issue> {
        let mut out = Vec::new();
        crate::defs::check_id(&mut out, "id", &self.id);
        if self.name.trim().is_empty() {
            out.push(Issue::new("name", "must not be empty"));
        }
        for (k, v, lo, hi) in self.values() {
            if v < lo || v > hi {
                out.push(
                    Issue::new(k, format!("must be in {lo}..={hi}, got {v}"))
                        .with_hint(format!("{k} = {}", v.clamp(lo, hi))),
                );
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn legacy(men: u8, k: Option<u8>, side: Option<&str>, loadout: &[(&str, u8, u8)]) -> SquadDef {
        let mut s: SquadDef = toml::from_str(
            r#"
            id = "t:x"
            name = "X"
            faction = "t:f"
            role = "rifle"
            weapon = "t:w"
            speed_cm_s = 300
            sight_m = 150
            spacing_dm = 30
            "#,
        )
        .unwrap();
        s.men = men;
        s.weapon_men = k;
        s.sidearm = side.map(String::from);
        s.loadout = loadout
            .iter()
            .map(|&(i, c, l)| LoadoutDef {
                item: i.into(),
                count: c,
                leader_count: l,
            })
            .collect();
        s
    }

    #[test]
    fn legacy_expansion_follows_the_spec_table() {
        // An MG team: no sidearm, every man carries the MG (three guns).
        let mg = expand_legacy(&legacy(3, None, None, &[]), false);
        assert_eq!(mg.len(), 2);
        assert_eq!((mg[0].slot.as_str(), mg[0].count), ("leader", 1));
        assert_eq!(
            (mg[1].slot.as_str(), mg[1].count, mg[1].weapon.as_str()),
            ("weapon", 2, "t:w")
        );
        // An AT team: the leader carries the launcher, one rifleman.
        let at = expand_legacy(&legacy(2, Some(1), Some("t:r"), &[]), false);
        assert_eq!(at.len(), 2);
        assert_eq!(at[0].weapon, "t:w");
        assert_eq!(
            (at[1].slot.as_str(), at[1].count, at[1].weapon.as_str()),
            ("sidearm", 1, "t:r")
        );
        // A crew of five: one MP 40, four pistols; items by leader_count.
        let crew = expand_legacy(
            &legacy(5, Some(1), Some("t:p"), &[("t:g", 1, 2), ("t:s", 0, 1)]),
            false,
        );
        assert_eq!(
            crew[0].items,
            BTreeMap::from([("t:g".into(), 2), ("t:s".into(), 1)])
        );
        assert_eq!(crew[1].items, BTreeMap::from([("t:g".into(), 1)]));
        assert_eq!(crew[1].count, 4);
        // S4 review M3: a legacy launcher squad has one carrier, whatever
        // `weapon_men` says; the others take the sidearm.
        let at3 = expand_legacy(&legacy(3, Some(2), Some("t:r"), &[]), true);
        assert_eq!(at3.len(), 2);
        assert_eq!((at3[0].count, at3[0].weapon.as_str()), (1, "t:w"));
        assert_eq!(
            (at3[1].slot.as_str(), at3[1].count, at3[1].weapon.as_str()),
            ("sidearm", 2, "t:r")
        );
    }

    #[test]
    fn projection_inverts_the_expansion() {
        for (men, k, side) in [
            (8, None, None),
            (3, None, None),
            (2, Some(1), Some("t:r")),
            (5, Some(1), Some("t:p")),
            (6, Some(4), Some("t:r")),
            (1, None, None),
        ] {
            let s = legacy(men, k, side, &[("t:g", 1, 2), ("t:s", 0, 1)]);
            let p = project_members(&expand_legacy(&s, false)).unwrap();
            assert_eq!(p.men, men);
            assert_eq!(p.weapon, "t:w");
            assert_eq!(p.weapon_men, k.unwrap_or(men));
            assert_eq!(p.sidearm.as_deref(), side);
            let want: Vec<(String, u8, u8)> = if men == 1 {
                vec![("t:g".into(), 0, 2), ("t:s".into(), 0, 1)]
            } else {
                vec![("t:g".into(), 1, 2), ("t:s".into(), 0, 1)]
            };
            let got: Vec<(String, u8, u8)> = p
                .loadout
                .iter()
                .map(|l| (l.item.clone(), l.count, l.leader_count))
                .collect();
            assert_eq!(got, want, "men {men}");
        }
    }

    #[test]
    fn projection_refuses_what_todays_sim_cannot_run() {
        let m = |slot: &str, count: u8, w: &str| MemberDef {
            slot: slot.into(),
            count,
            weapon: w.into(),
            sidearm: None,
            ammo: None,
            rounds: BTreeMap::new(),
            items: BTreeMap::new(),
            traits: Vec::new(),
        };
        // Three weapons.
        let e = project_members(&[m("a", 1, "x"), m("b", 2, "y"), m("c", 1, "z")]).unwrap_err();
        assert!(e.contains("slot `c`"), "{e}");
        // The first weapon again after the second.
        assert!(project_members(&[m("a", 1, "x"), m("b", 1, "y"), m("c", 1, "x")]).is_err());
        // A member sidearm.
        let mut s = m("a", 1, "x");
        s.sidearm = Some("p".into());
        assert!(project_members(&[s]).is_err());
        // Items differing between non-leaders.
        let mut b = m("b", 1, "x");
        b.items.insert("g".into(), 1);
        assert!(project_members(&[m("a", 1, "x"), m("c", 1, "x"), b]).is_err());
    }

    #[test]
    fn months_parse() {
        assert_eq!(parse_month("1944-06"), Some(1944 * 12 + 5));
        assert_eq!(parse_month("1944-13"), None);
        assert_eq!(parse_month("44-06"), None);
        assert_eq!(parse_month("1944-6"), None);
        assert!(parse_month("1944-06") < parse_month("1944-08"));
    }

    #[test]
    fn tags_map_back_to_roles() {
        let t = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            role_of_tags(&t(&["line", "understrength"])),
            Some(SquadRole::Rifle)
        );
        assert_eq!(role_of_tags(&t(&["line", "mg"])), Some(SquadRole::Mg));
        assert_eq!(role_of_tags(&t(&["assault"])), None);
        for r in [
            SquadRole::Rifle,
            SquadRole::Mg,
            SquadRole::Engineer,
            SquadRole::Mortar,
            SquadRole::AntiTank,
            SquadRole::Vehicle,
            SquadRole::Crew,
        ] {
            assert_eq!(role_of_tags(&[role_tag(r).to_string()]), Some(r));
        }
    }
}
