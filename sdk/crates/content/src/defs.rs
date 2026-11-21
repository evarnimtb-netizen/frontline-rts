//! Def files of a content mod (§7.1, §7.4): squads, weapons, items,
//! factions, maps and rules, one TOML file each under `defs/<kind>/`.
//!
//! Every def has a namespaced id (`ww2:kar98k`, §7.3). Values are integers
//! with the unit in the field name (`range_m`, `reload_ms`, `spread_mrad`),
//! so lowering into the fixed-point sim never depends on float rounding.
//! Unknown keys are errors. [`validate`](SquadDef::validate) methods check one
//! file on its own; cross-file references are checked by
//! [`crate::ModDefs`].

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::issue::Issue;
use crate::setup::DigProfile;
use crate::tuning::{
    AiAmmoTuning, AiArmourTuning, AiArtyTuning, AiAssaultTuning, AiFortTuning, AiGunsTuning,
    AiInfantryTuning, AiMortarTuning, AiObstacleTuning, AiReconTuning, AiSupplyTuning,
    AiTransportTuning, AiTruckTuning, BailTuning, BlastTuning, BodyTuning, CallInTuning,
    CoverTuning, DigCommonTuning, DrivingTuning, EscortTuning, FireMissionTuning, FireTuning,
    FormationTuning, GunneryTuning, GunsTuning, MortarTuning, RecrewTuning, ShareTuning,
    SupplyTuning, SuppressTuning, SuppressionTuning, ThrowTuning, TransportTuning, TuningRefs,
    VehicleDamageTuning, VehiclePathTuning, check_tuning,
};
use crate::tuning::{AiMedicTuning, WoundsTuning};

/// Map side limits (the sim's 64 m tiling).
pub const MAP_SIZE_STEP_M: u32 = 64;
pub const MAP_SIZE_MAX_M: u32 = 8192;
/// Most men in one squad.
pub const MAX_SQUAD_MEN: u8 = 16;

/// What a squad does on the field. Drives AI choices and which orders apply
/// (only squads that can dig take `Dig`, only mortars take `FireMission`).
/// A vehicle is a squad of one (`Vehicle`, S3): its squad def names the
/// vehicle def, which holds cost, pop, call-in time and sight.
#[derive(
    Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SquadRole {
    Rifle,
    Mg,
    Engineer,
    Mortar,
    /// Infantry anti-tank team: a launcher gunner and a loader (S3).
    AntiTank,
    /// A vehicle (S3): one per squad, see `SquadDef::vehicle`.
    Vehicle,
    /// S3c: a tank crew that bailed out (`VehicleDef::crew_squad`). Never
    /// bought: validation refuses it in a faction roster or start squads.
    Crew,
}

impl Default for SquadRole {
    /// Only a placeholder while a unit file without `role` is read: the
    /// loader derives the role from its tags and weapons (S4).
    fn default() -> SquadRole {
        SquadRole::Rifle
    }
}

/// A unit card (`defs/units/*.toml`): a buyable squad or vehicle, or a crew.
///
/// S4: a file may `extends` another unit (see `docs/modding/variants.md`);
/// what [`crate::ModDefs`] holds is the resolved card. Soldiers are given
/// either as `[[members]]` slots or by the legacy keys (`men`, `weapon`,
/// `weapon_men`, `sidearm`, `loadout`); after loading, a legacy card also
/// carries its expanded `members`, and every card its `tags`,
/// `capabilities`, `cooldown_s` and `unlock_s`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SquadDef {
    /// Namespaced id, e.g. `ww2:ger_grenadiergruppe`.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Faction id that fields this squad.
    pub faction: String,
    /// Legacy role (S1-S3). S4: optional; it fills `tags` and
    /// `capabilities` left empty. Without it the loader derives the role
    /// from the tags (crew, vehicle, at, mortar, engineer, mg, line) or the
    /// weapons; set `tags` or `role`.
    #[serde(default)]
    pub role: SquadRole,
    /// `vehicle` squads: the vehicle def id. Its def holds cost, pop,
    /// re-call cooldowns, speed and sight, so the soldier fields below
    /// stay out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vehicle: Option<String>,
    /// Soldiers in the squad, 1..=16 (soldier squads; required).
    #[serde(default)]
    pub men: u8,
    /// Weapon id the first `weapon_men` men carry (soldier squads;
    /// required).
    #[serde(default)]
    pub weapon: String,
    /// How many men carry `weapon` (default: all). The rest carry
    /// `sidearm`; an AT team has one launcher gunner and a rifleman.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weapon_men: Option<u8>,
    /// Weapon id of the men without `weapon` (a rifle, SMG or MG).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sidearm: Option<String>,
    /// Manpower cost of a call-in (soldier squads). A call-in spawns the
    /// squad at once at the base (S3b, owner 2026-11-01: no queue, no
    /// delivery time).
    #[serde(default)]
    pub cost: u32,
    /// Seconds after a call-in before this squad type can be called in
    /// again (soldier squads; S3b). 0 = no cooldown.
    #[serde(default)]
    pub recall_cooldown_s: u32,
    /// Seconds from the match start before the first call-in of this type
    /// (soldier squads; S3b). 0 = available at once.
    #[serde(default)]
    pub initial_cooldown_s: u32,
    /// Squad types naming the same group (a namespaced id, e.g.
    /// `ww2:support`) share one re-call timer: a call-in of any of them
    /// starts its own cooldown on all of them (S3b, optional; vehicle
    /// squads too).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cooldown_group: Option<String>,
    /// Hit points per soldier.
    #[serde(default = "default_hp")]
    pub hp: u32,
    /// Walking speed in cm/s (soldier squads; required).
    #[serde(default)]
    #[schemars(range(min = 10, max = 1000))]
    pub speed_cm_s: u32,
    /// Sight range in metres (soldier squads; required).
    #[serde(default)]
    pub sight_m: u32,
    /// Formation spacing between men in decimetres (soldier squads;
    /// required).
    #[serde(default)]
    #[schemars(range(min = 5, max = 200))]
    pub spacing_dm: u32,
    /// Digging speed per man in percent of the base rate; 0 = cannot dig.
    #[serde(default)]
    pub dig_rate_pct: u32,
    /// S3b (soldier squads): how steady the men are under fire, in percent.
    /// The duck, duck-release and pin suppression thresholds are scaled by
    /// it (150: they duck and pin at 1.5 times the suppression). The hook
    /// for veterancy (owner question S3b-9 (A)); every current squad 100.
    #[serde(default = "default_steadiness")]
    #[schemars(range(min = 25, max = 400))]
    pub steadiness_pct: u32,
    /// Items the men carry (S1b abilities); at most one entry per item
    /// kind. Empty: nothing to throw.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub loadout: Vec<LoadoutDef>,

    // ------------------------------------------------ S4 card block --
    /// S4: seconds after a call-in before this card can be called in again
    /// (the unit spawns at the base at once; nothing is queued). Alias:
    /// `recall_cooldown_s` (one of the two per file). Vehicle cards: unset
    /// = the vehicle def's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cooldown_s: Option<u32>,
    /// S4: match seconds before the first call-in. Alias:
    /// `initial_cooldown_s`. Vehicle cards: unset = the vehicle def's; the
    /// rules' `[vehicles] unlock_s` stays a floor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unlock_s: Option<u32>,
    /// S4: most squads of this card alive at once per player; 0 = no cap.
    #[serde(default, skip_serializing_if = "is_zero_u16")]
    pub max_alive: u16,
    /// S4: a group from the rules' `[limit_groups]`; alive units of every
    /// card in the group count against its limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit_group: Option<String>,
    /// S4: first and last month (`"YYYY-MM"`) the card is fielded; a card
    /// outside the rules' `[call_in] date` leaves the roster. Unset: always.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub available_from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub available_to: Option<String>,
    /// S4: free labels for the AI mix and HUD filters (`[a-z0-9_]`, at most
    /// 8). Reserved: `vehicle`, `crew`, `at`, `understrength`. Empty: from
    /// `role`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// S4: what the unit can do (orders follow): `dig_basic`, `dig_full`,
    /// `recrew`, `vehicle_crew`; `fire_mission` and `at_hunt` are derived from
    /// the members' weapons. Empty: from `role`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<String>,
    /// S4: training profile id (`defs/training/`); unset = the rules'
    /// `[training] default`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub training: Option<String>,
    /// S4: the AI's weight among ready cards of the same mix tag
    /// (default 100). S7: 0 = the AI never buys the card.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_weight: Option<u32>,
    /// S13 H1 (docs/slices/S13-heavy-wave.md 5): a heavy card the AI buys
    /// by its own path once enemy armour was seen (with `[ai.armour]
    /// heavy_buy` on), not by weight. Vehicle cards only; default false.
    #[serde(default, skip_serializing_if = "is_false")]
    pub ai_heavy: bool,
    /// S9: the vehicle card that comes with this gun card: its price adds
    /// to the card's and the tow arrives hitched with the crew riding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tow: Option<String>,
    /// S4: HUD tile name, at most 14 characters (default `name`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short_name: Option<String>,
    /// S4: citations for the card's strength and weapons.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<String>,
    /// S4: the men by slot (replaces the legacy soldier keys).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub members: Vec<crate::variant::MemberDef>,
    /// S4: a template other units extend; it resolves and validates but
    /// is never in a roster, start squads or the match. Not inherited.
    #[serde(rename = "abstract", default, skip_serializing_if = "is_false")]
    pub is_abstract: bool,
}

fn is_zero_u16(v: &u16) -> bool {
    *v == 0
}

fn is_false(v: &bool) -> bool {
    !*v
}

impl SquadDef {
    /// Re-call cooldown of a soldier card: `cooldown_s` or its alias.
    pub fn card_cooldown_s(&self) -> u32 {
        self.cooldown_s.unwrap_or(self.recall_cooldown_s)
    }

    /// First call-in time of a soldier card: `unlock_s` or its alias.
    pub fn card_unlock_s(&self) -> u32 {
        self.unlock_s.unwrap_or(self.initial_cooldown_s)
    }

    /// Whether the soldiers are given by the legacy keys.
    pub fn is_legacy(&self) -> bool {
        self.men != 0 || !self.weapon.is_empty()
    }

    /// Whether the card carries the tag.
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    /// Whether the card has the capability.
    pub fn has_capability(&self, cap: &str) -> bool {
        self.capabilities.iter().any(|c| c == cap)
    }
}

/// How many of one item a squad's men carry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LoadoutDef {
    /// Item id, e.g. `ww2:stielhandgranate`.
    pub item: String,
    /// Carried by every man but the squad leader.
    #[serde(default)]
    pub count: u8,
    /// Carried by the squad leader (the first man at spawn).
    #[serde(default)]
    pub leader_count: u8,
}

/// Most of one item a man carries.
pub const MAX_ITEM_COUNT: u8 = 10;
/// Longest re-call cooldown of a squad or vehicle type (s; S3b).
pub const MAX_COOLDOWN_S: u32 = 600;
/// Longest cooldown from the match start (s; S3b).
pub const MAX_INITIAL_COOLDOWN_S: u32 = 1800;

fn default_hp() -> u32 {
    100
}

/// S3b: steadiness of a squad type without `steadiness_pct` (percent).
pub const DEFAULT_STEADINESS_PCT: u32 = 100;

fn default_steadiness() -> u32 {
    DEFAULT_STEADINESS_PCT
}

/// What a thrown item does. The sim keeps per-soldier counts by kind, and
/// the presenter's ability buttons (G, T, F) map to kinds.
#[derive(
    Copy,
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Default,
    Serialize,
    Deserialize,
    JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum ItemKind {
    /// Fragmentation grenade: a small HE blast.
    #[default]
    Frag = 0,
    /// Smoke grenade: a cloud that blocks spotting.
    Smoke = 1,
    /// Satchel charge: a big HE blast whose crater cuts trenches.
    Satchel = 2,
    /// S4: a single-shot anti-tank weapon (Panzerfaust): not thrown, fired
    /// at a vehicle like a launcher (`weapon` names the launcher def it
    /// fires as); the tube is spent.
    SingleShotAt = 3,
    /// S10a: a Bangalore torpedo: placed, not thrown; its `[he]` bursts
    /// `bangalore.bursts` times along `bangalore.length_cm`.
    Bangalore = 4,
    /// S10b: a mine: laid by a minefield blueprint (`FortDef::mine_item`),
    /// never thrown, never in a kit; `[mine]` says what sets it off.
    Mine = 5,
    /// S11b G1: an HE rifle grenade: not thrown; the man fits the launcher
    /// (`windup_ms`) and fires it as `weapon` (a launcher def with HE
    /// ammunition) on the low arc at a seen enemy man; `stow_ms` to take
    /// the launcher off again. Issued only with `[items] rifle_grenades` 1.
    RifleGrenade = 6,
}

/// Number of [`ItemKind`]s.
pub const ITEM_KINDS: usize = 7;

impl ItemKind {
    pub const ALL: [ItemKind; ITEM_KINDS] = [
        ItemKind::Frag,
        ItemKind::Smoke,
        ItemKind::Satchel,
        ItemKind::SingleShotAt,
        ItemKind::Bangalore,
        ItemKind::Mine,
        ItemKind::RifleGrenade,
    ];

    /// Whether the item is thrown (frag, smoke, satchel).
    #[inline]
    pub const fn thrown(self) -> bool {
        !matches!(
            self,
            ItemKind::SingleShotAt | ItemKind::Bangalore | ItemKind::Mine | ItemKind::RifleGrenade
        )
    }

    /// Position in [`ItemKind::ALL`] (per-soldier count arrays).
    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }
}

/// A throwable item (`defs/items/*.toml`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ItemDef {
    pub id: String,
    pub name: String,
    pub kind: ItemKind,
    /// Closest target, in metres.
    #[serde(default)]
    pub min_range_m: u32,
    /// Farthest target, in metres.
    pub range_m: u32,
    /// The thrower stands still this long before the item leaves his hand.
    pub windup_ms: u32,
    /// Fuse from the start of the throw (wind-up included). The item never
    /// goes off before it lands; 0 = on landing.
    #[serde(default)]
    pub fuse_ms: u32,
    /// Scatter of the landing point, in percent of the throw distance.
    pub scatter_pct: u32,
    /// Frag and satchel: the blast.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub he: Option<ItemHeDef>,
    /// Smoke: the cloud.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smoke: Option<SmokeDef>,
    /// S4 `single_shot_at`: the launcher weapon def it fires as (range,
    /// engagement distances, spread, round, fire reveal). `windup_ms` is
    /// the aim time before the shot; `range_m` the farthest shot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weapon: Option<String>,
    /// S4: citations.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<String>,
    /// S5a: what one item weighs and which supply kind refills it (priced
    /// in the base; required in a mod with a rules `[supply]` table).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supply: Option<SupplyDef>,
    /// S10a `bangalore` items: the charge's length and bursts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bangalore: Option<BangaloreDef>,
    /// S10b `mine` items: what sets the mine off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mine: Option<MineDef>,
    /// S11b G1 `rifle_grenade` items: the man takes the launcher off his
    /// rifle this long after the shot; his rifle is silent meanwhile (ms).
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub stow_ms: u32,
}

/// S10b: what sets a mine off (`[mine]` of a `mine` item).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MineDef {
    /// `foot` (an enemy man on foot) or `vehicle` (an enemy vehicle's track
    /// or wheel).
    pub trigger: String,
    /// How close the man or the track centreline must come (cm).
    pub trigger_cm: u32,
}

/// S10b: the `[mine] trigger` values, by setup index.
pub const MINE_TRIGGERS: [&str; 2] = ["foot", "vehicle"];

/// S10a: a Bangalore torpedo's charge (`[bangalore]` of a `bangalore` item).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BangaloreDef {
    /// Length of the laid charge (cm).
    pub length_cm: u32,
    /// HE bursts spaced evenly along it, all in one tick.
    pub bursts: u32,
    /// The man works this long to lay it (s); the fuse is `fuse_ms` after.
    pub place_s: u32,
}

/// An item's blast (like a weapon's [`HeDef`], plus damage and suppression).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ItemHeDef {
    /// Damage of the overpressure inside `blast_radius_cm`.
    pub damage: u32,
    /// Suppression at the blast point (out of 100), falling to zero at the
    /// suppression radius.
    pub suppression: u32,
    /// Overpressure (S3b, physical-contact rule): a man within this
    /// radius whose body point has a clear line from the burst takes full
    /// `damage`; track anchors within it take `vs_vehicle_pct` of it.
    pub blast_radius_cm: u32,
    /// Fragment rays per burst (at most 256): each stops at the first man,
    /// vehicle, wreck or earth it meets.
    pub frag_count: u32,
    /// Fragment ray length.
    pub frag_reach_cm: u32,
    /// Damage of one fragment striking a man at the burst ...
    pub frag_damage: u32,
    /// ... falling linearly to this percent of it at full reach.
    pub frag_far_pct: u32,
    /// Ground bursts throw fragments between these elevations above the
    /// ground plane, `[min, max]` degrees (a burst on a vehicle or a man,
    /// well above the ground, throws them within `[-max, max]`).
    pub frag_elev_deg: [u32; 2],
    /// A fragment striking a hull side's track band deals this to the track.
    pub frag_track_damage: u32,
    pub suppression_radius_m: u32,
    pub crater_radius_cm: u32,
    pub crater_depth_cm: u32,
    /// The crater levels what it hits instead of deepening it: a trench
    /// inside it fills in and a parapet is blown away (satchel charges).
    #[serde(default)]
    pub cuts_trenches: bool,
    /// Armour it defeats on contact, in mm, by the plate rule (S3).
    pub pen_mm: u32,
    /// When it defeats a plate (S3b): `inside_rays` fragment rays from the
    /// contact point into the hull, `inside_damage` each to the first box
    /// they cross within `inside_reach_cm`.
    #[serde(default)]
    pub inside_damage: u32,
    #[serde(default)]
    pub inside_rays: u32,
    #[serde(default)]
    pub inside_reach_cm: u32,
    /// Damage to track anchors within `blast_radius_cm`, in percent of the
    /// blast's damage. Blasts never reach a closed vehicle's insides.
    pub vs_vehicle_pct: u32,
    /// S8a thin-plate rule (S7-8): a fragment pierces a plate whose
    /// effective thickness is below this (mm RHA at the burst, falling with
    /// distance as `frag_damage` does). 0: never.
    #[serde(default)]
    pub frag_pen_mm: u32,
}

/// Most fragment rays one burst may throw (S3b; the per-tick budget is the
/// sim's).
pub const MAX_FRAG_RAYS: u32 = 256;
/// Most inner fragment rays when HE beats a plate (S3b).
pub const MAX_INSIDE_RAYS: u32 = 32;

/// Checks the fragment and overpressure keys every HE block has (S3b).
/// `v`: blast_radius_cm, frag_count, frag_reach_cm, frag_damage,
/// frag_far_pct, frag_track_damage; `elev` the band.
fn check_frag(out: &mut Vec<Issue>, prefix: &str, v: [u32; 6], elev: [u32; 2]) {
    let [blast, count, reach, damage, far, track] = v;
    range(out, &format!("{prefix}blast_radius_cm"), blast, 0, 2000);
    range(out, &format!("{prefix}frag_count"), count, 0, MAX_FRAG_RAYS);
    range(out, &format!("{prefix}frag_reach_cm"), reach, 0, 5000);
    range(out, &format!("{prefix}frag_damage"), damage, 0, 10_000);
    range(out, &format!("{prefix}frag_far_pct"), far, 0, 100);
    range(out, &format!("{prefix}frag_track_damage"), track, 0, 1000);
    range(out, &format!("{prefix}frag_elev_deg[0]"), elev[0], 0, 89);
    range(out, &format!("{prefix}frag_elev_deg[1]"), elev[1], 0, 89);
    if elev[0] > elev[1] {
        out.push(
            Issue::new(
                format!("{prefix}frag_elev_deg"),
                format!("[{}, {}] is not [min, max]", elev[0], elev[1]),
            )
            .with_hint(format!(
                "{prefix}frag_elev_deg = [{}, {}]",
                elev[1], elev[0]
            )),
        );
    }
}

/// Checks the S3 vehicle fields every HE block has.
fn check_he_armour(out: &mut Vec<Issue>, prefix: &str, v: [u32; 5]) {
    let [
        pen_mm,
        inside_damage,
        inside_rays,
        inside_reach_cm,
        vs_vehicle_pct,
    ] = v;
    range(out, &format!("{prefix}pen_mm"), pen_mm, 0, 2000);
    range(
        out,
        &format!("{prefix}inside_damage"),
        inside_damage,
        0,
        10_000,
    );
    range(
        out,
        &format!("{prefix}inside_rays"),
        inside_rays,
        0,
        MAX_INSIDE_RAYS,
    );
    range(
        out,
        &format!("{prefix}inside_reach_cm"),
        inside_reach_cm,
        0,
        1000,
    );
    range(
        out,
        &format!("{prefix}vs_vehicle_pct"),
        vs_vehicle_pct,
        0,
        100,
    );
}

/// A smoke cloud: a cylinder that grows, holds, then thins out.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SmokeDef {
    /// Full radius (the cloud is as tall as it is wide).
    pub radius_m: u32,
    /// Growth from nothing to the full radius.
    pub grow_ms: u32,
    /// Full density after growing.
    pub hold_ms: u32,
    /// Thinning out: sight lines get through more and more often.
    pub fade_ms: u32,
}

/// Weapon family; decides how the sim fires it.
#[derive(
    Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum WeaponClass {
    Rifle,
    Smg,
    Mg,
    /// Indirect fire at a ground position; needs `[he]`.
    Mortar,
    /// A vehicle's main gun (S3); it fires the rounds its mount carries.
    Cannon,
    /// An infantry anti-tank launcher (S3); it fires `ammo` and targets
    /// only vehicles.
    Launcher,
    /// S3c: a handgun (tank crews' sidearm); fires like a rifle.
    Pistol,
}

/// A weapon (`defs/weapons/*.toml`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WeaponDef {
    pub id: String,
    pub name: String,
    pub class: WeaponClass,
    /// Shortest range it can engage at, in metres (mortars).
    #[serde(default)]
    pub min_range_m: u32,
    /// Longest range it engages at, in metres.
    pub range_m: u32,
    /// Damage per hit (direct) or at the blast centre (HE).
    pub damage: u32,
    /// Cyclic rate in rounds per minute.
    pub rounds_per_min: u32,
    /// Rounds between reloads.
    pub magazine: u32,
    pub reload_ms: u32,
    /// Dispersion half-angle in milliradians ...
    #[serde(default)]
    pub spread_mrad: u32,
    /// ... or in microradians, for guns finer than 1 mrad (cannons). Set
    /// one of the two.
    #[serde(default)]
    pub spread_urad: u32,
    /// Cannons and launchers: nominal; their rounds fly at the ammo's
    /// `muzzle_velocity_m_s` (a launcher's must match its ammo's).
    pub muzzle_velocity_m_s: u32,
    /// Time after moving before it can fire (MG, mortar).
    #[serde(default)]
    pub setup_ms: u32,
    /// Suppression added to a soldier by one near miss (out of 100).
    pub suppression: u32,
    /// Firing reveals the shooter: sight range against him in percent, for
    /// how long (default: the rules' `firing_spot_pct` and `reveal_ms`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fire_reveal_pct: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fire_reveal_ms: Option<u32>,
    /// Launchers: opens fire on a vehicle standing still within this range,
    /// on a moving one (over 1 m/s) within `engage_moving_m` (default
    /// `range_m`). Applies to every player, human or AI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engage_still_m: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engage_moving_m: Option<u32>,
    /// Launchers: the round it fires (`defs/ammo/`) ...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ammo: Option<String>,
    /// ... and how many the team carries (resupplied in base like items).
    #[serde(default)]
    pub rounds_carried: u32,
    /// Launchers: when the gunner dies, the next living man takes the
    /// weapon after this long.
    #[serde(default)]
    pub handover_ms: u32,
    /// High-explosive effect of each round.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub he: Option<HeDef>,
    /// S4: a crew weapon (an MG, launcher or mortar): when its carrier
    /// falls, the nearest man of his squad picks it up after `handover_ms`
    /// (assistants and ammo bearers first). Default: true for the `mg`,
    /// `mortar` and `launcher` classes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crew_weapon: Option<bool>,
    /// S4: name on the squad card (at most 14 characters; default `name`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short_name: Option<String>,
    /// S4: citations for the values.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<String>,
    /// S5a: what each shot takes from the man's rounds: a cartridge
    /// (`defs/cartridges/`) or an ammo def with `supply`. Required in a mod
    /// with a rules `[supply]` table for every weapon a soldier carries or
    /// a vehicle mounts as an MG (launchers fire their `ammo`, cannons
    /// their mount's rounds).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feed: Option<String>,
    /// S5a: rounds a man carrying it has when his slot gives no `rounds`
    /// for its feed (default: one `magazine`), counting the rounds in the
    /// weapon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub load: Option<u32>,
    /// S5a: MG fire discipline: rounds per burst (0: fire the magazine
    /// through) and the pause after a burst (ms). Parsed in S5a Part A,
    /// used from Part B.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub burst_rounds: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub burst_pause_ms: u32,
    /// S5a fire discipline per weapon (S5a-18): a man with this weapon in
    /// `discipline` mode opens fire within this share of its range (%;
    /// 10-100; absent or 0: the rules' `combat.fire.hold_beyond_pct`).
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub hold_beyond_pct: u32,
    /// S5b: high-angle indirect fire (mortars): the charges and laying
    /// limits the charge solver picks from. Without it (legacy content) a
    /// mortar keeps the S1 flight rule (`[combat.mortar] flight_milli`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub indirect: Option<IndirectDef>,
    /// S8a (vehicles.md 6.4): small arms against thin armour: rows of
    /// `[range_m, mm]` (mm RHA beaten at 0 deg; linear between rows, flat
    /// past the last), ranges strictly rising. Empty: the round never
    /// beats a plate.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub penetration: Vec<[u32; 2]>,
    /// S8a: what a penetrating round does inside (needs `penetration`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub core: Option<SmallCoreDef>,
}

/// S8a: a small-arms round past a plate: it goes on `length_cm` and hits at
/// most `max_boxes` inner boxes (crew, riders, components) for `damage`
/// each. No spall, no filler.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SmallCoreDef {
    pub damage: u32,
    pub length_cm: u32,
    pub max_boxes: u32,
}

/// S8a: most rows of a small-arms penetration table.
pub const MAX_PEN_ROWS: usize = 8;

/// S5b `[indirect]` of a mortar: muzzle velocity per charge (m/s, strictly
/// rising, the lowest first) and the elevation limits (whole degrees, high
/// angle: 45 to 89). The solver takes the lowest charge that reaches the
/// target at an elevation inside the limits.
///
/// S11a1: a gun's table sets `low_angle = true`: its band may start below
/// 45 deg (`[0, 73]`), the solver tries each charge's high root and then
/// its low root, and the weapon may be a cannon. `range_m` (default the
/// weapon's `range_m`) is the indirect range; `spread_mrad` (default the
/// weapon's spread) the round scatter of a mission.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IndirectDef {
    pub charges_m_s: Vec<u32>,
    pub elevation_deg: [u32; 2],
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub low_angle: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range_m: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spread_mrad: Option<u32>,
}

/// Most charges in an `[indirect]` table.
pub const MAX_CHARGES: usize = 12;

/// Drag-free range (m) of muzzle velocity `v` (m/s) at elevation `deg`
/// (whole degrees) on flat ground: `v² sin 2θ / g`. Content-side check only
/// (the sim solves in fixed point).
pub fn indirect_range_m(v: u32, deg: u32) -> f64 {
    let t = (deg as f64).to_radians();
    (v as f64) * (v as f64) * (2.0 * t).sin() / 9.81
}

impl IndirectDef {
    /// Checks the table against the weapon's ranges; `path` is `indirect`.
    pub fn check(&self, w: &WeaponDef, out: &mut Vec<Issue>) {
        let c = &self.charges_m_s;
        if c.is_empty() || c.len() > MAX_CHARGES {
            out.push(
                Issue::new(
                    "indirect.charges_m_s",
                    format!("needs 1..={MAX_CHARGES} charges, got {}", c.len()),
                )
                .with_hint("charges_m_s = [75, 105, 130, 152, 174]"),
            );
            return;
        }
        for (k, &v) in c.iter().enumerate() {
            if !(20..=600).contains(&v) {
                out.push(Issue::new(
                    format!("indirect.charges_m_s[{k}]"),
                    format!("must be in 20..=600 m/s, got {v}"),
                ));
                return;
            }
        }
        if c.windows(2).any(|p| p[0] >= p[1]) {
            out.push(
                Issue::new(
                    "indirect.charges_m_s",
                    "charges must rise strictly (the lowest charge first)",
                )
                .with_hint("charges_m_s = [75, 105, 130, 152, 174]"),
            );
            return;
        }
        let [lo, hi] = self.elevation_deg;
        if self.low_angle {
            self.check_gun(w, out);
            return;
        }
        if !(45..=89).contains(&lo) || !(45..=89).contains(&hi) || lo >= hi {
            out.push(
                Issue::new(
                    "indirect.elevation_deg",
                    format!("must be [min, max] within 45..=89 with min < max, got [{lo}, {hi}]"),
                )
                .with_hint("high angle only in S5b; set elevation_deg = [45, 85]"),
            );
            return;
        }
        if w.class != WeaponClass::Mortar || w.he.is_none() {
            out.push(Issue::new(
                "indirect",
                "only a mortar (class = \"mortar\" with an [he] table) has [indirect]",
            ));
        }
        let shortest = indirect_range_m(c[0], hi);
        if shortest > w.min_range_m as f64 * 1.1 {
            out.push(
                Issue::new(
                    "indirect",
                    format!(
                        "the shortest high-angle range is {shortest:.0} m (lowest charge at {hi} deg), min_range_m is {}",
                        w.min_range_m
                    ),
                )
                .with_hint(format!(
                    "raise the top elevation or add a lower charge, or set min_range_m = {}",
                    shortest.ceil() as u32
                )),
            );
        }
        let longest = indirect_range_m(*c.last().expect("non-empty"), 45);
        if longest < w.range_m as f64 {
            out.push(
                Issue::new(
                    "indirect.charges_m_s",
                    format!(
                        "no charge reaches {} m (the highest at 45 deg reaches {longest:.0} m)",
                        w.range_m
                    ),
                )
                .with_hint(format!(
                    "add a charge of at least {} m/s, or lower range_m",
                    ((w.range_m as f64) * 9.81).sqrt().ceil() as u32
                )),
            );
        }
    }
}

impl IndirectDef {
    /// S11a1: the checks of a `low_angle` table (a gun's): the band within
    /// 0..=89 with min < max, a mortar with `[he]` or a cannon, `range_m`
    /// and `spread_mrad` in range, the highest charge reaching `range_m`.
    fn check_gun(&self, w: &WeaponDef, out: &mut Vec<Issue>) {
        let [lo, hi] = self.elevation_deg;
        if lo > 89 || hi > 89 || lo >= hi {
            out.push(Issue::new(
                "indirect.elevation_deg",
                format!("must be [min, max] within 0..=89 with min < max, got [{lo}, {hi}]"),
            ));
            return;
        }
        let ok_class =
            (w.class == WeaponClass::Mortar && w.he.is_some()) || w.class == WeaponClass::Cannon;
        if !ok_class {
            out.push(Issue::new(
                "indirect",
                "a low_angle table needs a cannon or a mortar with an [he] table",
            ));
        }
        let range = self.range_m.unwrap_or(w.range_m);
        if !(1..=20_000).contains(&range) {
            out.push(Issue::new(
                "indirect.range_m",
                format!("must be in 1..=20000 m, got {range}"),
            ));
            return;
        }
        if self.spread_mrad.is_some_and(|v| v > 100) {
            out.push(Issue::new(
                "indirect.spread_mrad",
                "must be in 0..=100 mrad",
            ));
        }
        let top = hi.min(45);
        let longest = indirect_range_m(*self.charges_m_s.last().expect("non-empty"), top);
        if longest < range as f64 {
            out.push(Issue::new(
                "indirect.range_m",
                format!(
                    "no charge reaches {range} m (the highest at {top} deg reaches {longest:.0} m)"
                ),
            ));
        }
    }
}

fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}

/// S5a supply kinds (`supply.kind`): what a round is charged as when the
/// base (later a crate, dump or HQ) hands it out.
#[derive(
    Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum SupplyKind {
    /// Rifle, SMG, pistol and MG rounds, vehicle MG belts.
    SmallArms = 0,
    /// Grenades, satchels, AT rockets, Panzerfausts.
    Explosives = 1,
    /// Mortar bombs, main-gun rounds.
    Shells = 2,
    /// S10a: construction material (sandbags, timber, wire). Never in the
    /// three-wide stock, cargo and account arrays (`ALL`, `NAMES`): only
    /// crates, carried kg and `VehicleState::material_g` hold it.
    Construction = 3,
}

impl SupplyKind {
    /// Every kind, in index order.
    pub const ALL: [SupplyKind; 3] = [
        SupplyKind::SmallArms,
        SupplyKind::Explosives,
        SupplyKind::Shells,
    ];
    /// TOML and report names, by index.
    pub const NAMES: [&'static str; 3] = ["small_arms", "explosives", "shells"];

    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }

    /// The ammunition kind named `s` (never `construction`).
    pub fn from_name(s: &str) -> Option<SupplyKind> {
        Self::NAMES
            .iter()
            .position(|n| *n == s)
            .map(|i| Self::ALL[i])
    }

    /// S10a: a crate's kind: an ammunition kind or `construction`.
    pub fn from_crate_name(s: &str) -> Option<SupplyKind> {
        if s == "construction" {
            return Some(SupplyKind::Construction);
        }
        Self::from_name(s)
    }

    /// Whether the kind indexes the three-wide ammunition arrays.
    #[inline]
    pub const fn is_ammo(self) -> bool {
        !matches!(self, SupplyKind::Construction)
    }
}

/// S5a: a round's (or an item's) supply table: `supply = { kind =
/// "small_arms", g = 28 }`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SupplyDef {
    /// `small_arms`, `explosives` or `shells` (`construction` comes with
    /// S6).
    pub kind: String,
    /// Grams per round, its share of clip, belt, link, box or carton
    /// included (what a crate or the base hands out); 1..=100000.
    pub g: u32,
}

impl SupplyDef {
    /// Checks one supply table; `path` is its key (`supply`).
    pub fn check(&self, path: &str, out: &mut Vec<Issue>) {
        if SupplyKind::from_name(&self.kind).is_none() {
            let msg = if self.kind == "construction" {
                "`construction` supplies come with S6 (HQ, dumps, crates); not before S6".to_owned()
            } else {
                format!(
                    "unknown supply kind `{}`; one of small_arms, explosives, shells",
                    self.kind
                )
            };
            out.push(Issue::new(format!("{path}.kind"), msg).with_hint("kind = \"small_arms\""));
        }
        range(out, &format!("{path}.g"), self.g, 1, 100_000);
    }

    /// The parsed kind (after [`SupplyDef::check`]).
    pub fn kind(&self) -> SupplyKind {
        SupplyKind::from_name(&self.kind).unwrap_or(SupplyKind::SmallArms)
    }
}

/// S5a: a counted round whose flight lives on the weapon (small-arms
/// rounds, mortar bombs): `defs/cartridges/*.toml`. It has an id, a supply
/// kind and a weight, no ballistics.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CartridgeDef {
    pub id: String,
    pub name: String,
    /// Calibre (mm), for the docs and the HUD.
    pub caliber_mm: u32,
    pub supply: SupplyDef,
    /// Citations.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<String>,
}

impl CartridgeDef {
    pub fn validate(&self) -> Vec<Issue> {
        let mut out = Vec::new();
        check_id(&mut out, "id", &self.id);
        check_name(&mut out, "name", &self.name);
        range(&mut out, "caliber_mm", self.caliber_mm, 1, 300);
        self.supply.check("supply", &mut out);
        out
    }
}

/// S10a: a blueprint's shape (`FortDef::shape`).
#[derive(
    Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum FortShape {
    /// One site at a point, facing a second point.
    #[default]
    Point = 0,
    /// One piece (or wire section) per segment along a dragged line.
    Line = 1,
    /// Snaps to an own built HQ (the HQ dugout roof).
    OverHq = 2,
}

/// S10a: piece part names (`PieceDef::part`; colour only), by setup index.
pub const PIECE_PARTS: [&str; 4] = ["sandbag", "roof", "timber", "steel"];
/// S10a: the capabilities a blueprint may need (`FortDef::capability`);
/// S10b adds `build_heavy` (hedgehog, bunker) and `place_mines`.
pub const FORT_CAPABILITIES: [&str; 4] = [
    "build_structure",
    "place_wire",
    "build_heavy",
    "place_mines",
];
/// S10b: most pieces of one blueprint (the bunker has six).
pub const MAX_FORT_PIECES: usize = 6;

/// S10a: one box of a blueprint (sandbags, a roof).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PieceDef {
    /// `sandbag`, `roof`, `timber` or `steel`.
    pub part: String,
    /// Along the facing, across, up (cm).
    pub size_cm: [u32; 3],
    /// Centre offset in the facing frame (cm); z of the box bottom.
    #[serde(default)]
    pub offset_cm: [i32; 3],
    /// Armour a bullet or fragment must beat to damage it (mm).
    pub armour_mm: u32,
    pub hp: u32,
    /// `base`: z from the undug surface (a roof over a pit); default the
    /// ground under it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub z_from: Option<String>,
}

/// S10a: a wire blueprint's band.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WireDef {
    /// Width of the band men are slowed in (cm).
    pub band_cm: u32,
    /// Height (cm): fragments below it hit it.
    pub height_cm: u32,
    pub hp: u32,
}

/// S10a: a fortification blueprint (`defs/fortifications/*.toml`), the
/// same for every faction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FortDef {
    pub id: String,
    pub name: String,
    pub shape: FortShape,
    /// A dig profile dug before the pieces are built (`mg_nest_pit`,
    /// `dugout_pit`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pit: Option<DigProfile>,
    /// `build_structure` or `place_wire`.
    pub capability: String,
    /// Point: the whole work; line: per segment.
    pub materials_kg: u32,
    /// Seconds of work at 100 % with `work_men` men (point: whole; line:
    /// per segment).
    pub work_s: u32,
    pub work_men: u32,
    /// Sites (point, over_hq) or pieces / sections (line) per player,
    /// built or building.
    pub limit_per_player: u32,
    /// Point: no other own site of this def closer (m).
    #[serde(default)]
    pub min_spacing_m: u32,
    pub max_slope_pct: u32,
    /// Line: segment length (cm) and the longest line (m).
    #[serde(default)]
    pub segment_cm: u32,
    #[serde(default)]
    pub max_length_m: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pieces: Vec<PieceDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wire: Option<WireDef>,
    /// S10b minefields: the `mine` item laid, one per segment end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mine_item: Option<String>,
    /// S10b obstacles: the pieces block vehicle path nodes.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub blocks_vehicles: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<String>,
}

impl FortDef {
    pub fn validate(&self) -> Vec<Issue> {
        let mut out = Vec::new();
        check_id(&mut out, "id", &self.id);
        check_name(&mut out, "name", &self.name);
        let wire = self.capability == "place_wire";
        let mines = self.capability == "place_mines";
        if !FORT_CAPABILITIES.contains(&self.capability.as_str()) {
            out.push(Issue::new(
                "capability",
                format!(
                    "`{}` is not one of {}",
                    self.capability,
                    FORT_CAPABILITIES.join(", ")
                ),
            ));
        }
        if wire {
            if self.wire.is_none() || !self.pieces.is_empty() {
                out.push(Issue::new(
                    "wire",
                    "a place_wire blueprint has a `wire` table and no pieces",
                ));
            }
        } else if mines {
            // S10b: a minefield lays mines, nothing else.
            if self.mine_item.is_none()
                || !self.pieces.is_empty()
                || self.wire.is_some()
                || self.shape != FortShape::Line
            {
                out.push(Issue::new(
                    "mine_item",
                    "a place_mines blueprint is a line with `mine_item`, no pieces and no `wire`",
                ));
            }
        } else if self.wire.is_some()
            || self.pieces.is_empty()
            || self.pieces.len() > MAX_FORT_PIECES
        {
            out.push(Issue::new(
                "pieces",
                format!("needs 1..={MAX_FORT_PIECES} pieces and no `wire`"),
            ));
        }
        if self.mine_item.is_some() && !mines {
            out.push(Issue::new(
                "mine_item",
                "only a place_mines blueprint lays mines",
            ));
        }
        if self.blocks_vehicles && self.pieces.is_empty() {
            out.push(Issue::new("blocks_vehicles", "an obstacle needs pieces"));
        }
        if self.capability == "build_heavy"
            && (self.shape == FortShape::OverHq || self.pieces.is_empty())
        {
            out.push(Issue::new(
                "capability",
                "build_heavy only on point or line blueprints with pieces",
            ));
        }
        if self.shape == FortShape::OverHq && self.pit.is_some() {
            out.push(Issue::new("pit", "an over_hq blueprint digs no pit"));
        }
        if let Some(p) = self.pit
            && !matches!(
                p,
                DigProfile::MgNestPit | DigProfile::DugoutPit | DigProfile::BunkerPit
            )
        {
            out.push(Issue::new(
                "pit",
                "one of mg_nest_pit, dugout_pit, bunker_pit",
            ));
        }
        if self.shape == FortShape::Line {
            range(&mut out, "segment_cm", self.segment_cm, 50, 1000);
            range(&mut out, "max_length_m", self.max_length_m, 1, 200);
            if self.pit.is_some() {
                out.push(Issue::new("pit", "a line blueprint digs no pit"));
            }
        }
        range(&mut out, "materials_kg", self.materials_kg, 0, 10_000);
        range(&mut out, "work_s", self.work_s, 1, 3600);
        range(&mut out, "work_men", self.work_men, 1, 12);
        range(&mut out, "limit_per_player", self.limit_per_player, 1, 200);
        range(&mut out, "min_spacing_m", self.min_spacing_m, 0, 200);
        range(&mut out, "max_slope_pct", self.max_slope_pct, 0, 100);
        for (k, b) in self.pieces.iter().enumerate() {
            if !PIECE_PARTS.contains(&b.part.as_str()) {
                out.push(Issue::new(
                    format!("pieces[{k}].part"),
                    format!("`{}` is not one of {}", b.part, PIECE_PARTS.join(", ")),
                ));
            }
            for (j, v) in b.size_cm.iter().enumerate() {
                range(&mut out, &format!("pieces[{k}].size_cm[{j}]"), *v, 10, 600);
            }
            for (j, o) in b.offset_cm.iter().enumerate() {
                if o.unsigned_abs() > 1000 {
                    out.push(Issue::new(
                        format!("pieces[{k}].offset_cm[{j}]"),
                        "must be in -1000..=1000",
                    ));
                }
            }
            range(
                &mut out,
                &format!("pieces[{k}].armour_mm"),
                b.armour_mm,
                1,
                2000,
            );
            range(&mut out, &format!("pieces[{k}].hp"), b.hp, 1, 10_000);
            if b.z_from.as_deref().is_some_and(|z| z != "base") {
                out.push(Issue::new(
                    format!("pieces[{k}].z_from"),
                    "the only value is `base`",
                ));
            }
        }
        if let Some(w) = &self.wire {
            range(&mut out, "wire.band_cm", w.band_cm, 10, 1000);
            range(&mut out, "wire.height_cm", w.height_cm, 10, 500);
            range(&mut out, "wire.hp", w.hp, 1, 10_000);
        }
        out
    }
}

/// S6: what a supply def is (`kind`).
#[derive(
    Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum StockKind {
    /// The platoon HQ (one per player): a pit, a stock box and a radio box.
    #[default]
    Hq = 0,
    /// An ammunition dump (up to the rules' `dump_max`).
    Dump = 1,
    /// A crate one man carries.
    Crate = 2,
    /// S12 W8: an aid post (up to the rules' `aid_max`; Build B).
    Aid = 3,
}

/// S6: one oriented box of a supply def (`boxes = [...]`): what stops
/// bullets and fragments and holds hit points.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StockBoxDef {
    /// `stock` (holds the kg; breaking it loses them), `radio` (HQ: the
    /// radio state) or `crate`.
    pub part: String,
    /// Length (along the facing), width, height (cm).
    pub size_cm: [u32; 3],
    /// Centre offset from the def's centre, in its facing frame (cm; z
    /// from the ground).
    #[serde(default)]
    pub offset_cm: [i32; 3],
    /// Hit points (fragment and bullet damage points).
    pub hp: u32,
}

/// S6: an HQ, an ammunition dump or a crate (`defs/supply/*.toml`;
/// docs/slices/S6-hq-crates.md "Data model").
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StockDef {
    pub id: String,
    pub name: String,
    pub faction: String,
    pub kind: StockKind,
    /// HQ and dump: manpower paid when the build order is accepted.
    #[serde(default)]
    pub cost: u32,
    /// HQ and dump: the dig profile dug before it stands (`hq_pit`,
    /// `dump_pit`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pit: Option<DigProfile>,
    /// HQ and dump: men, mortars and tanks inside this radius are served (m).
    #[serde(default)]
    pub radius_m: u32,
    /// HQ and dump: the most stock it holds, kg by supply kind.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub cap_kg: BTreeMap<String, u32>,
    /// HQ: seconds after its destruction before a new one may be ordered.
    #[serde(default)]
    pub rebuild_s: u32,
    /// HQ and dump: unit tags that may build it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub built_by: Vec<String>,
    /// Crate: its supply kind (`small_arms`, `explosives`, `shells`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supply: Option<String>,
    /// Crate: kg it holds, paid at issue (kg x the kind's price, D1).
    #[serde(default)]
    pub kg: u32,
    /// Crate: its carrier's squad walks at this share of its pace (%).
    #[serde(default = "full_pct")]
    pub carry_speed_pct: u32,
    /// The HE round (an ammo def with `blast`) whose bursts a broken box
    /// holding shells or explosives sets off (cook-off).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cookoff: Option<String>,
    pub boxes: Vec<StockBoxDef>,
    /// S12 W8 aid post: beds, 1..=8 (men treated or evacuated at once).
    #[serde(default)]
    pub beds: u32,
    /// S12 W8 aid post: own medics standing in its radius it needs to work,
    /// 1..=8.
    #[serde(default)]
    pub staff_min: u32,
    /// Citations.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<String>,
}

fn full_pct() -> u32 {
    100
}

/// S6: box part names (`StockBoxDef::part`), by their setup index.
pub const STOCK_BOX_PARTS: [&str; 4] = ["stock", "radio", "crate", "tent"];

impl StockDef {
    pub fn validate(&self) -> Vec<Issue> {
        let mut out = Vec::new();
        check_id(&mut out, "id", &self.id);
        check_name(&mut out, "name", &self.name);
        check_id(&mut out, "faction", &self.faction);
        if self.boxes.is_empty() || self.boxes.len() > 4 {
            out.push(Issue::new("boxes", "needs 1..=4 boxes"));
        }
        for (k, b) in self.boxes.iter().enumerate() {
            if !STOCK_BOX_PARTS.contains(&b.part.as_str()) {
                out.push(Issue::new(
                    format!("boxes[{k}].part"),
                    format!("`{}` is not one of stock, radio, crate, tent", b.part),
                ));
            }
            for (j, s) in b.size_cm.iter().enumerate() {
                range(&mut out, &format!("boxes[{k}].size_cm[{j}]"), *s, 10, 1000);
            }
            for (j, o) in b.offset_cm.iter().enumerate() {
                if o.unsigned_abs() > 1000 {
                    out.push(Issue::new(
                        format!("boxes[{k}].offset_cm[{j}]"),
                        "must be in -1000..=1000",
                    ));
                }
            }
            range(&mut out, &format!("boxes[{k}].hp"), b.hp, 1, 10_000);
        }
        if let Some(c) = &self.cookoff {
            check_id(&mut out, "cookoff", c);
        }
        match self.kind {
            StockKind::Crate => {
                match self.supply.as_deref().and_then(SupplyKind::from_crate_name) {
                    Some(_) => {}
                    None => out.push(
                        Issue::new("supply", "a crate names its supply kind")
                            .with_hint("supply = \"shells\""),
                    ),
                }
                range(&mut out, "kg", self.kg, 1, 200);
                range(&mut out, "carry_speed_pct", self.carry_speed_pct, 10, 100);
            }
            StockKind::Hq | StockKind::Dump => {
                if !matches!(self.pit, Some(DigProfile::HqPit | DigProfile::DumpPit)) {
                    out.push(
                        Issue::new("pit", "an HQ or dump names its pit profile")
                            .with_hint("pit = \"hq_pit\""),
                    );
                }
                range(&mut out, "radius_m", self.radius_m, 1, 100);
                range(&mut out, "cost", self.cost, 0, 10_000);
                range(&mut out, "rebuild_s", self.rebuild_s, 0, 3600);
                for (k, v) in &self.cap_kg {
                    if SupplyKind::from_name(k).is_none() {
                        out.push(Issue::new(
                            format!("cap_kg.{k}"),
                            "unknown supply kind; one of small_arms, explosives, shells",
                        ));
                    }
                    range(&mut out, &format!("cap_kg.{k}"), *v, 0, 10_000);
                }
                if self.built_by.is_empty() {
                    out.push(
                        Issue::new("built_by", "names at least one unit tag")
                            .with_hint("built_by = [\"hq\"]"),
                    );
                }
            }
            // S12 W8: the aid post: a shallow pit, beds and staff, no stock.
            StockKind::Aid => {
                if self.pit != Some(DigProfile::AidPit) {
                    out.push(
                        Issue::new("pit", "an aid post names the aid pit profile")
                            .with_hint("pit = \"aid_pit\""),
                    );
                }
                range(&mut out, "radius_m", self.radius_m, 1, 100);
                range(&mut out, "cost", self.cost, 0, 10_000);
                range(&mut out, "beds", self.beds, 1, 8);
                range(&mut out, "staff_min", self.staff_min, 1, 8);
                if !self.cap_kg.is_empty() {
                    out.push(Issue::new("cap_kg", "an aid post holds no stock"));
                }
                if self.built_by.is_empty() {
                    out.push(
                        Issue::new("built_by", "names at least one unit tag")
                            .with_hint("built_by = [\"medic\"]"),
                    );
                }
            }
        }
        if self.kind != StockKind::Aid && (self.beds != 0 || self.staff_min != 0) {
            out.push(Issue::new(
                "beds",
                "only an aid post has beds and staff_min",
            ));
        }
        out
    }
}

impl WeaponDef {
    /// Whether the weapon is a crew weapon (S4; see `crew_weapon`).
    pub fn is_crew_weapon(&self) -> bool {
        self.crew_weapon.unwrap_or(matches!(
            self.class,
            WeaponClass::Mg | WeaponClass::Mortar | WeaponClass::Launcher
        ))
    }
}

/// High-explosive blast.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HeDef {
    /// Overpressure (S3b, physical-contact rule): a man within this
    /// radius whose body point has a clear line from the burst takes full
    /// `damage`; track anchors within it take `vs_vehicle_pct` of it.
    pub blast_radius_cm: u32,
    /// Fragment rays per burst (at most 256): each stops at the first man,
    /// vehicle, wreck or earth it meets.
    pub frag_count: u32,
    /// Fragment ray length.
    pub frag_reach_cm: u32,
    /// Damage of one fragment striking a man at the burst ...
    pub frag_damage: u32,
    /// ... falling linearly to this percent of it at full reach.
    pub frag_far_pct: u32,
    /// Ground bursts throw fragments between these elevations above the
    /// ground plane, `[min, max]` degrees (a burst on a vehicle or a man,
    /// well above the ground, throws them within `[-max, max]`).
    pub frag_elev_deg: [u32; 2],
    /// A fragment striking a hull side's track band deals this to the track.
    pub frag_track_damage: u32,
    pub suppression_radius_m: u32,
    pub crater_radius_cm: u32,
    pub crater_depth_cm: u32,
    /// Armour it defeats on contact, in mm, by the plate rule (S3).
    pub pen_mm: u32,
    /// When it defeats a plate (S3b): `inside_rays` fragment rays from the
    /// contact point into the hull, `inside_damage` each to the first box
    /// they cross within `inside_reach_cm`.
    #[serde(default)]
    pub inside_damage: u32,
    #[serde(default)]
    pub inside_rays: u32,
    #[serde(default)]
    pub inside_reach_cm: u32,
    /// Damage to track anchors within `blast_radius_cm`, in percent of the
    /// blast's damage. Blasts never reach a closed vehicle's insides.
    pub vs_vehicle_pct: u32,
    /// S8a thin-plate rule (S7-8): a fragment pierces a plate whose
    /// effective thickness is below this (mm RHA at the burst, falling with
    /// distance as `frag_damage` does). 0: never.
    #[serde(default)]
    pub frag_pen_mm: u32,
}

/// A playable side (`defs/factions/*.toml`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FactionDef {
    pub id: String,
    pub name: String,
    /// Team colour as `#rrggbb`.
    pub color: String,
    /// Squad ids this faction can call in.
    pub roster: Vec<String>,
    /// Squad ids on the field at tick 0. Must be in `roster`.
    #[serde(default)]
    pub start_squads: Vec<String>,
}

/// A skirmish map (`defs/maps/*.toml`). The heightfield is generated from
/// `[terrain]` by the sim; nothing here is an image.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MapFileDef {
    pub id: String,
    pub name: String,
    /// Side of the square map: a multiple of 64 in 64..=8192.
    pub size_m: u32,
    pub terrain: TerrainDef,
    /// Woods patches: they cut spotting range inside them, not bullets.
    #[serde(default)]
    pub woods: Vec<WoodsDef>,
    pub capture_points: Vec<CapturePointDef>,
    /// One base per player slot, in slot order.
    pub bases: Vec<BaseDef>,
}

/// Heightfield shape. Height = base + value-noise octaves + ridges − valleys.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TerrainDef {
    /// Terrain seed: fixed per map, independent of the match seed.
    pub seed: u64,
    pub base_height_m: u32,
    #[serde(default)]
    pub octaves: Vec<OctaveDef>,
    #[serde(default)]
    pub ridges: Vec<RidgeDef>,
    #[serde(default)]
    pub valleys: Vec<ValleyDef>,
    /// S3c (S3c-8 (B)): `"point"` makes the terrain point-symmetric about
    /// the map centre: the half on base 0's side (x + y below the map
    /// size) is generated and mirrored onto the other half, so neither
    /// base side gets better ground. Woods, points and bases are placed by
    /// hand and are not mirrored. Default `"none"`.
    #[serde(default)]
    pub symmetry: TerrainSymmetry,
}

/// How a map's terrain is mirrored (S3c).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum TerrainSymmetry {
    #[default]
    None,
    Point,
}

/// One octave of smooth value noise.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OctaveDef {
    /// Lattice spacing in metres (a multiple of 4).
    pub period_m: u32,
    /// Peak deviation in centimetres.
    pub amplitude_cm: u32,
}

/// A raised band along a segment, smooth falloff to both sides.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RidgeDef {
    pub from: [u32; 2],
    pub to: [u32; 2],
    pub height_m: u32,
    /// Full width at the foot, in metres.
    pub width_m: u32,
}

/// A lowered band along a segment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ValleyDef {
    pub from: [u32; 2],
    pub to: [u32; 2],
    pub depth_m: u32,
    pub width_m: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WoodsDef {
    pub center: [u32; 2],
    pub radius_m: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CapturePointDef {
    /// Short label shown in the HUD, e.g. `A`.
    pub name: String,
    pub pos: [u32; 2],
    /// Weight when counting held points for income and ticket drain.
    #[serde(default = "default_point_value")]
    pub value: u32,
}

fn default_point_value() -> u32 {
    1
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BaseDef {
    pub pos: [u32; 2],
    pub radius_m: u32,
}

/// Game-mode rules (`defs/rules/*.toml`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RulesDef {
    pub id: String,
    pub name: String,
    /// 30 (default) or 20.
    #[serde(default = "default_tick_hz")]
    pub tick_hz: u32,
    #[serde(default = "default_hash_interval")]
    pub hash_interval: u32,
    pub tickets: u32,
    /// Soldiers per side, alive or called in.
    pub pop_cap: u32,
    pub projectile_cap: u32,
    pub economy: EconomyDef,
    pub tickets_drain: TicketsDrainDef,
    pub capture: CaptureDef,
    pub movement: MovementDef,
    pub dig: DigDef,
    pub combat: CombatDef,
    pub items: ItemRulesDef,
    /// Vehicles (S3).
    pub vehicles: VehicleRulesDef,
    pub difficulty: DifficultyTableDef,
    pub ai: AiDef,
    /// Where arriving men look for cover (S3b tuning table).
    pub cover: CoverTuning,
    /// Where bought squads spawn in the base (S3b tuning table), and (S4)
    /// the match date for card `available_from` / `available_to`.
    pub call_in: CallInRulesDef,
    /// S4: the training profile of cards that name none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub training: Option<TrainingRulesDef>,
    /// S4: named limits on alive units, by group id (e.g.
    /// `"ww2:heavy_armour" = 2`); a card joins one with `limit_group`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub limit_groups: BTreeMap<String, u32>,
    /// S5a: ammunition prices and base refill rates. Without it (legacy
    /// content) rounds are unlimited and refills free (S5a-22).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supply: Option<SupplyTuning>,
    /// S5b: fire missions, observers, ranging and the knowledge map.
    /// Without it (legacy content) a mission runs as the old barrage
    /// (S5b-1 (A)).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fire_mission: Option<FireMissionTuning>,
    /// S12: wounds, medics, aid posts and reinforcements (`[wounds]`).
    /// Without it: the S12a values, every switch off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wounds: Option<WoundsTuning>,
    /// S13: rule O1, obliquity by plate-to-calibre ratio (`[armour]`).
    /// Without it: the S13 values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub armour: Option<crate::tuning::ArmourTuning>,
    /// S10a: fortification rules (`[fortify]`). Without it no blueprint
    /// can be built.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fortify: Option<crate::tuning::FortTuning>,
}

/// `[call_in]` of a rules def: the S3b spawn tuning ([`CallInTuning`],
/// which the match setup carries) plus the S4 match date.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CallInRulesDef {
    /// Six spawn spots sit on a ring at this share of the base radius
    /// (the seventh is the centre) (%).
    pub spawn_ring_pct: u32,
    /// A spot with a man or vehicle this close is passed over for the
    /// least crowded one (m).
    pub spawn_clear_m: u32,
    /// A vehicle counts as this many men when spots are compared.
    pub spawn_vehicle_weight: u32,
    /// S4: the match month, `"YYYY-MM"`: cards whose `available_from` /
    /// `available_to` exclude it leave the rosters. Empty (default): no
    /// date filter (skirmish).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub date: String,
}

impl CallInRulesDef {
    /// The spawn tuning table the match setup carries.
    pub fn tuning(&self) -> CallInTuning {
        CallInTuning {
            spawn_ring_pct: self.spawn_ring_pct,
            spawn_clear_m: self.spawn_clear_m,
            spawn_vehicle_weight: self.spawn_vehicle_weight,
        }
    }
}

/// `[training]` of a rules def (S4).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TrainingRulesDef {
    /// Training profile id of cards without `training`, e.g.
    /// `ww2:training_regular`.
    pub default: String,
}

/// Vehicle rules (S3, `docs/slices/S3-armour.md`). Per-vehicle numbers
/// (speeds, armour, bog chance) are in the vehicle defs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VehicleRulesDef {
    /// Vehicles can be called in from this much match time on (0 = from
    /// the start; the test override).
    pub unlock_s: u32,
    /// Most vehicles per player, alive or queued.
    pub limit: u32,
    /// Most wrecks on the map; beyond it the oldest is removed.
    pub wreck_cap: u32,
    /// A crewman moving into an empty driver or gunner seat takes this
    /// long; both seats count as empty meanwhile.
    pub seat_swap_s: u32,
    /// Crew hit points per seat, and below which a crewman is wounded.
    pub crew_hp: u32,
    pub wounded_hp: u32,
    /// A vehicle faster than this counts as moving (spotting, spread,
    /// launcher fire discipline).
    pub moving_cm_s: u32,
    /// Sight range against a moving vehicle, in percent (the size factor
    /// is the vehicle def's `spot_size_pct`; the highest applies).
    pub moving_spot_pct: u32,
    /// Enemy soldiers this close spot a vehicle without a sight line.
    pub near_spot_m: u32,
    /// Main-gun spread against a moving target, in percent.
    pub moving_target_spread_pct: u32,
    /// Each further shot at the same target is this much tighter ...
    pub ranging_step_pct: u32,
    /// ... down to this much of the first shot's spread.
    pub ranging_floor_pct: u32,
    /// Spread of HE fired at infantry (replaces the gun's AP spread).
    pub he_area_spread_mrad: u32,
    /// Tank HE is never fired within this distance of own men.
    pub he_clear_m: u32,
    /// Track samples over ground dug at least this deep count as crossing a
    /// trench ...
    pub crossing_dug_cm: u32,
    /// ... which caps speed at this ...
    pub crossing_speed_cm_s: u32,
    /// ... and a new crossing (one bog roll) starts after this long with no
    /// sample over dug ground.
    pub crossing_gap_s: u32,
    /// An engine hit for at least this much damage may start a fire ...
    pub fire_min_damage: u32,
    /// ... with this chance. A fire burns the engine ...
    pub fire_start_pct: u32,
    pub fire_burn_hp_s: u32,
    /// ... and every `fire_check_s` a living crew puts it out with
    /// `fire_out_pct`. A fire still burning after `fire_burnout_s` knocks
    /// the vehicle out.
    pub fire_check_s: u32,
    pub fire_out_pct: u32,
    pub fire_burnout_s: u32,
    /// Each hit on the ammo rack for at least this much damage detonates
    /// it with `rack_detonate_pct` (a rack destroyed by damage always
    /// does).
    pub rack_min_damage: u32,
    pub rack_detonate_pct: u32,
    /// Inside the own base: percent of every component's HP back per
    /// second, one main-gun round back every `base_rearm_s`, and one dead
    /// crew seat back every `base_crew_s`.
    pub base_repair_pct_s: u32,
    pub base_rearm_s: u32,
    pub base_crew_s: u32,
    /// Path following, yielding and ground pose (S3b tuning table).
    pub driving: DrivingTuning,
    /// Vehicle path costs and blocked ground (S3b tuning table).
    pub path: VehiclePathTuning,
    /// The infantry file behind a vehicle (S3b tuning table).
    pub escort: EscortTuning,
    /// Vehicle weapons and hits on vehicles (S3b tuning table).
    pub gunnery: GunneryTuning,
    /// Ammo fires and rack outcomes (S3b tuning table).
    pub damage: VehicleDamageTuning,
    /// Bail-out (S3c tuning table).
    pub bail: BailTuning,
    /// Re-crewing abandoned vehicles (S3c tuning table).
    pub recrew: RecrewTuning,
    /// Infantry carriers (S8a tuning table; defaults when absent).
    #[serde(default)]
    pub transport: TransportTuning,
    /// S9 `[vehicles.guns]`.
    #[serde(default)]
    pub guns: GunsTuning,
}

/// Thrown items (S1b abilities).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ItemRulesDef {
    /// A soldier in his own base gets one item back this often, up to his
    /// loadout.
    pub resupply_s: u32,
    /// S4: a squad fires at most one single-shot AT item (Panzerfaust)
    /// this often (s), so it does not empty its kit at one tank.
    #[serde(default = "default_single_shot_gap_s")]
    pub single_shot_gap_s: u32,
    /// S4 (review L5): a fallen gunner's crew weapon (MG, launcher) is
    /// picked up only by a man of his squad within this distance (m) of
    /// it; it waits there until one is.
    #[serde(default = "default_crew_pickup_reach_m")]
    pub crew_pickup_reach_m: u32,
    /// S11b G1 switch: squads are issued their rifle grenades (0: the setup
    /// drops the `rifle_grenade` kind from every squad type).
    #[serde(default)]
    pub rifle_grenades: u32,
    /// S11b G3: a squad fires at most one rifle grenade this often (s).
    #[serde(default = "default_rifle_grenade_gap_s")]
    pub rifle_grenade_gap_s: u32,
    /// Throw flight and smoke reach (S3b tuning table).
    pub throw: ThrowTuning,
}

fn default_single_shot_gap_s() -> u32 {
    3
}
fn default_crew_pickup_reach_m() -> u32 {
    30
}
fn default_rifle_grenade_gap_s() -> u32 {
    15
}
fn default_tick_hz() -> u32 {
    30
}
fn default_hash_interval() -> u32 {
    30
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EconomyDef {
    pub start_manpower: u32,
    pub income_per_min: u32,
    pub point_income_per_min: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TicketsDrainDef {
    pub per_min_per_point: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CaptureDef {
    pub radius_m: u32,
    pub time_s: u32,
    pub max_men: u8,
    /// Each man beyond the first, up to `max_men`, adds this much capture
    /// speed (percent of one man's).
    pub extra_man_pct: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MovementDef {
    pub slope_cost_pct: u32,
    pub max_slope_pct: u32,
    pub arrive_cm: u32,
    pub retreat_speed_pct: u32,
    pub catch_up_speed_pct: u32,
    /// Walking speed of a man ordered prone (crawling), in percent.
    pub prone_speed_pct: u32,
    pub path_budget_per_tick: u32,
    pub path_max_expansions: u32,
    /// Path step cost between two trench cells, in percent of normal.
    pub trench_cost_pct: u32,
    /// Infantry path step cost into an AT-ditch cell, in percent of normal.
    pub ditch_cost_pct: u32,
    /// Squad walking, arrival and soldier movement details (S3b tuning
    /// table).
    pub formation: FormationTuning,
}

/// Dig profiles (S3 generalised the S1 trench to named profiles).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DigDef {
    pub profiles: DigProfilesDef,
    /// Spoil bank shape and digger reach (S3b tuning table).
    pub common: DigCommonTuning,
}

/// Every profile a `Dig` command can name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DigProfilesDef {
    /// The standard trench (S1).
    pub trench: DigProfileDef,
    /// The anti-tank ditch (S3): deep and wide enough to block vehicles.
    pub at_ditch: DigProfileDef,
    /// S6: pits (a rectangle with an all-round parapet; `pit_length_cm`
    /// set): the mortar pit, the HQ pit and the dump pit. Absent (legacy
    /// content): the trench profile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mortar_pit: Option<DigProfileDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hq_pit: Option<DigProfileDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dump_pit: Option<DigProfileDef>,
    /// S9: an AT gun pit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gun_pit: Option<DigProfileDef>,
    /// S10a: the MG nest's and the dugout's pits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mg_nest_pit: Option<DigProfileDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dugout_pit: Option<DigProfileDef>,
    /// S10b: the bunker's pit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bunker_pit: Option<DigProfileDef>,
    /// S12 W8: the aid post's pit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aid_pit: Option<DigProfileDef>,
}

/// A dug cross-section: a flat floor, walls sloping out to the lip, and a
/// spoil bank (parapet or berm) on the enemy side.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DigProfileDef {
    pub depth_cm: u32,
    pub floor_cm: u32,
    /// Horizontal run of each wall, floor edge to lip.
    pub wall_cm: u32,
    /// Height of the spoil bank.
    pub parapet_cm: u32,
    pub max_length_m: u32,
    pub work_per_m: u32,
    /// It deepens in this many equal passes.
    pub passes: u32,
    /// Only engineer squads dig it.
    #[serde(default)]
    pub engineers_only: bool,
    /// S6: a pit: the spoil goes all round, not to the front only.
    #[serde(default)]
    pub all_round: bool,
    /// S6: a pit's outside length along its long axis (cm; 0: a line
    /// profile). The dug line is this minus the floor and both walls, so
    /// the rounded ends make the pit's length.
    #[serde(default)]
    pub pit_length_cm: u32,
}

/// Spotting, accuracy, suppression and corpse rules. Suppression is in
/// points out of 100 (a weapon's `suppression` is added per near miss).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CombatDef {
    /// Spotting LOS rays per player per tick.
    pub spot_rays_per_tick: u32,
    /// Each enemy soldier is re-checked this often while the ray budget lasts.
    pub spot_period_ms: u32,
    /// Most observers tried per target (rays per target check).
    pub spot_observers: u32,
    /// Sight range against a prone target, in percent.
    pub prone_spot_pct: u32,
    /// Sight range against a target in a trench, in percent.
    pub trench_spot_pct: u32,
    /// Sight range against a target in woods, in percent.
    pub woods_spot_pct: u32,
    /// Sight range against a soldier who fired recently (replaces the
    /// prone, trench and woods factors), in percent.
    pub firing_spot_pct: u32,
    /// How long a shot gives the shooter away.
    pub reveal_ms: u32,
    /// A round passing this close suppresses.
    pub near_miss_cm: u32,
    /// Suppression added by being hit.
    pub hit_suppression: u32,
    pub suppression_decay_per_s: u32,
    /// At this suppression a soldier crouches ...
    pub crouch_suppression: u32,
    /// ... and at this one he is pinned (prone, does not move).
    pub pin_suppression: u32,
    /// Weapon spread while the shooter moves, in percent.
    pub moving_spread_pct: u32,
    /// Weapon spread while pinned, in percent.
    pub pinned_spread_pct: u32,
    /// Extra spread at the weapon's full range, in percent (linear in range).
    pub range_spread_pct: u32,
    /// A soldier firing from a trench shows this much above the parapet.
    pub fire_exposure_cm: u32,
    /// Corpses stay this long (for the presenter).
    pub corpse_s: u32,
    /// Hit capsule and posture points (S3b tuning table).
    pub body: BodyTuning,
    /// Mortar laying and flight (S3b tuning table).
    pub mortar: MortarTuning,
    /// HE bursts (S3b tuning table).
    pub blast: BlastTuning,
    /// Suppression details (S3b tuning table).
    pub suppression: SuppressionTuning,
    /// The Suppress order's area fire (S3b tuning table).
    pub suppress: SuppressTuning,
    /// S5a: sharing rounds inside a squad (default: the S5a starting
    /// values).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub share: Option<ShareTuning>,
    /// S5a: fire discipline (default: the S5a starting values).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fire: Option<FireTuning>,
}

/// AI commander tuning (weights in data, §9). Difficulty is separate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AiDef {
    /// Squads below this strength (percent of full men) fall back to base
    /// while enemies are near.
    pub retreat_strength_pct: u32,
    /// Spotted enemies this close to a point or squad count as a threat.
    pub threat_radius_m: u32,
    /// Target army mix by squad role, in percent of squads (legacy form;
    /// S4: or `[ai.mix]`, exactly one of the two).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mix_rifle_pct: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mix_mg_pct: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mix_engineer_pct: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mix_mortar_pct: Option<u32>,
    /// Rounds per mortar mission (legacy key; S5b: `[ai.mortar]
    /// mission_rounds`, set one of the two).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub barrage_rounds: Option<u32>,
    /// Trench length engineers dig at a held point ...
    pub dig_length_m: u32,
    /// ... this far in front of it, toward the nearest enemy.
    pub dig_offset_m: u32,
    /// A squad throws at most one item this often.
    pub throw_cooldown_s: u32,
    /// Frags go at enemies in a trench, or at a group of at least this many
    /// spotted men within 5 m of each other.
    pub frag_group_men: u32,
    /// S3 mix: vehicles after unlock (under the vehicle limit), and AT
    /// teams after unlock once an enemy vehicle has been seen, in percent
    /// of squads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mix_vehicle_pct: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mix_at_pct: Option<u32>,
    /// S4: the target army mix by card tag, in percent of squads, e.g.
    /// `line = 50`, `mg = 20`, `vehicle = 20`, `at = 20` (replaces the
    /// `mix_*_pct` keys; set one form). `vehicle` counts after the vehicle
    /// unlock, `at` once an enemy vehicle was seen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mix: Option<BTreeMap<String, u32>>,
    /// Tanks hold this far from a point where enemy AT was seen ...
    pub tank_standoff_m: u32,
    /// ... else advance with their escort to this far from the enemy
    /// trench, where the escort attacks.
    pub assault_close_m: u32,
    /// Enemy AT counts as gone from a point after this long unseen.
    pub at_clear_s: u32,
    /// An immobilised vehicle repairs when no seen enemy is this close.
    pub repair_clear_m: u32,
    /// Infantry, mortar and satchel rules (S3b tuning table).
    pub infantry: AiInfantryTuning,
    /// Vehicles and escorts (S3b tuning table).
    pub armour: AiArmourTuning,
    /// Fire-and-move, AT and satchel hunts (S3b tuning table).
    pub assault: AiAssaultTuning,
    /// S5a: ammunition reserve and refill trips (default: the S5a
    /// starting values).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ammo: Option<AiAmmoTuning>,
    /// S5b: the AI's fire missions (replaces `barrage_rounds`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mortar: Option<AiMortarTuning>,
    /// S6: the AI's HQ and crate runs (default: the S6 starting values).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supply: Option<AiSupplyTuning>,
    /// S8a: the AI's half-tracks (default: the S8a starting values).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transport: Option<AiTransportTuning>,
    /// S8b: `[ai.recon]`.
    pub recon: Option<AiReconTuning>,
    /// S8b: `[ai.truck]`.
    pub truck: Option<AiTruckTuning>,
    /// S9: `[ai.guns]`.
    pub guns: Option<AiGunsTuning>,
    /// S10a-ai: `[ai.fort]` (default: the S10a-ai starting values).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fort: Option<AiFortTuning>,
    /// S10b-ai: `[ai.obstacles]` (default: the S10b-ai starting values).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub obstacles: Option<AiObstacleTuning>,
    /// S11a1: `[ai.arty]` (default: the S11a1 starting values, switches off).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arty: Option<AiArtyTuning>,
    /// S12: `[ai.medic]` (default: the S12a values, switches off).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub medic: Option<AiMedicTuning>,
}

impl AiDef {
    /// S5b: `[ai.mortar]`, else the legacy `barrage_rounds` (6 if absent)
    /// with the S5b starting values.
    pub fn mortar_tuning(&self) -> AiMortarTuning {
        self.mortar
            .clone()
            .unwrap_or_else(|| AiMortarTuning::legacy(self.barrage_rounds.unwrap_or(6)))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DifficultyTableDef {
    pub easy: DifficultyDef,
    pub normal: DifficultyDef,
    pub hard: DifficultyDef,
}

/// How an AI player at one difficulty differs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DifficultyDef {
    pub income_pct: u32,
    pub reaction_ms: u32,
    pub replan_ms: u32,
    pub decision_quality_pct: u32,
}

// ------------------------------------------------------------ validation --

/// Pushes an issue when `v` is outside `lo..=hi`.
fn range(out: &mut Vec<Issue>, path: &str, v: u32, lo: u32, hi: u32) {
    if v < lo || v > hi {
        out.push(Issue::new(path, format!("must be in {lo}..={hi}, got {v}")));
    }
}

/// Namespaced id check (`mod:name`, lower-case ASCII, digits and `_`).
pub(crate) fn check_id(out: &mut Vec<Issue>, path: &str, id: &str) {
    let ok = id.split_once(':').is_some_and(|(ns, name)| {
        let part = |s: &str| {
            !s.is_empty()
                && s.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        };
        part(ns) && part(name)
    });
    if !ok {
        out.push(
            Issue::new(path, format!("`{id}` is not a namespaced id"))
                .with_hint("id = \"mymod:my_thing\" (lower-case letters, digits, `_`)"),
        );
    }
}

fn check_name(out: &mut Vec<Issue>, path: &str, name: &str) {
    if name.trim().is_empty() {
        out.push(Issue::new(path, "must not be empty"));
    }
}

impl SquadDef {
    /// Checks one resolved unit card on its own (cross-file references:
    /// [`crate::ModDefs::cross_check`]).
    pub fn validate(&self) -> Vec<Issue> {
        let mut out = Vec::new();
        check_id(&mut out, "id", &self.id);
        check_name(&mut out, "name", &self.name);
        if let Some(g) = &self.cooldown_group {
            check_id(&mut out, "cooldown_group", g);
        }
        self.validate_card(&mut out);
        if self.role == SquadRole::Vehicle {
            // The vehicle def holds everything else.
            if self.vehicle.is_none() {
                out.push(
                    Issue::new("vehicle", "a `vehicle` squad names its vehicle def")
                        .with_hint("vehicle = \"mymod:my_tank\""),
                );
            }
            let soldier_fields = [
                ("men", self.men != 0),
                ("weapon", !self.weapon.is_empty()),
                ("weapon_men", self.weapon_men.is_some()),
                ("sidearm", self.sidearm.is_some()),
                ("recall_cooldown_s", self.recall_cooldown_s != 0),
                ("initial_cooldown_s", self.initial_cooldown_s != 0),
                ("hp", self.hp != default_hp()),
                ("speed_cm_s", self.speed_cm_s != 0),
                ("sight_m", self.sight_m != 0),
                ("spacing_dm", self.spacing_dm != 0),
                ("dig_rate_pct", self.dig_rate_pct != 0),
                (
                    "steadiness_pct",
                    self.steadiness_pct != default_steadiness(),
                ),
                ("loadout", !self.loadout.is_empty()),
                ("members", !self.members.is_empty()),
                ("training", self.training.is_some()),
            ];
            for (key, set) in soldier_fields {
                if set {
                    let hint = match key {
                        "recall_cooldown_s" => {
                            "a vehicle card sets cooldown_s (S4), or the vehicle def"
                        }
                        "initial_cooldown_s" => {
                            "a vehicle card sets unlock_s (S4), or the vehicle def"
                        }
                        _ => "set it in the vehicle def, or remove it",
                    };
                    out.push(Issue::new(key, "not allowed on a vehicle squad").with_hint(hint));
                }
            }
            return out;
        }
        if self.vehicle.is_some() {
            out.push(Issue::new(
                "vehicle",
                "only squads with role = \"vehicle\" (or the `vehicle` tag) name a vehicle",
            ));
        }
        if !self.members.is_empty() {
            // S4: the men are given by slots; the legacy keys stay out.
            let legacy = [
                ("men", self.men != 0),
                ("weapon", !self.weapon.is_empty()),
                ("weapon_men", self.weapon_men.is_some()),
                ("sidearm", self.sidearm.is_some()),
                ("loadout", !self.loadout.is_empty()),
            ];
            for (key, set) in legacy {
                if set {
                    out.push(
                        Issue::new(
                            key,
                            "set either [[members]] or the legacy men/weapon keys, not both",
                        )
                        .with_hint(format!(
                            "remove `{key}` (each slot has its weapon, items and count)"
                        )),
                    );
                }
            }
            crate::variant::check_members(&mut out, &self.members);
        } else {
            if self.men == 0 {
                out.push(
                    Issue::new(
                        "men",
                        "required: soldiers in the squad, 1..=16 (or [[members]] slots)",
                    )
                    .with_hint("[[members]] slot = \"rifleman\", count = 8, weapon = \"...\""),
                );
            } else {
                range(&mut out, "men", self.men as u32, 1, MAX_SQUAD_MEN as u32);
            }
            if self.weapon.is_empty() {
                out.push(Issue::new(
                    "weapon",
                    "required: the weapon id the men carry",
                ));
            }
            if let Some(n) = self.weapon_men {
                range(&mut out, "weapon_men", n as u32, 1, self.men.max(1) as u32);
                if n < self.men && self.sidearm.is_none() {
                    out.push(
                        Issue::new(
                            "sidearm",
                            format!(
                                "{} men carry no `weapon`; they need a sidearm",
                                self.men - n
                            ),
                        )
                        .with_hint("sidearm = \"ww2:kar98k\""),
                    );
                }
            }
            if self.sidearm.is_some() && self.weapon_men.is_none_or(|n| n >= self.men) {
                out.push(Issue::new(
                    "sidearm",
                    "every man carries `weapon`; set weapon_men below men or remove it",
                ));
            }
        }
        range(
            &mut out,
            "recall_cooldown_s",
            self.recall_cooldown_s,
            0,
            MAX_COOLDOWN_S,
        );
        range(
            &mut out,
            "initial_cooldown_s",
            self.initial_cooldown_s,
            0,
            MAX_INITIAL_COOLDOWN_S,
        );
        range(&mut out, "hp", self.hp, 1, 10_000);
        range(&mut out, "speed_cm_s", self.speed_cm_s, 10, 1000);
        range(&mut out, "sight_m", self.sight_m, 1, 2000);
        range(&mut out, "spacing_dm", self.spacing_dm, 5, 200);
        range(&mut out, "dig_rate_pct", self.dig_rate_pct, 0, 1000);
        range(&mut out, "steadiness_pct", self.steadiness_pct, 25, 400);
        range(&mut out, "cost", self.cost, 0, 100_000);
        for (i, l) in self.loadout.iter().enumerate() {
            let max = MAX_ITEM_COUNT as u32;
            range(
                &mut out,
                &format!("loadout[{i}].count"),
                l.count as u32,
                0,
                max,
            );
            range(
                &mut out,
                &format!("loadout[{i}].leader_count"),
                l.leader_count as u32,
                0,
                max,
            );
            if self.loadout[..i].iter().any(|o| o.item == l.item) {
                out.push(Issue::new(
                    format!("loadout[{i}].item"),
                    format!("`{}` is listed twice", l.item),
                ));
            }
        }
        for cap in ["dig_basic", "dig_full"] {
            if self.dig_rate_pct == 0 && self.has_capability(cap) {
                out.push(
                    Issue::new(
                        "capabilities",
                        format!("`{cap}` needs a digging speed (dig_rate_pct is 0)"),
                    )
                    .with_hint("dig_rate_pct = 100"),
                );
            }
        }
        out
    }

    /// S4 card-block keys shared by soldier and vehicle cards.
    fn validate_card(&self, out: &mut Vec<Issue>) {
        if let Some(c) = self.cooldown_s {
            range(out, "cooldown_s", c, 0, MAX_COOLDOWN_S);
        }
        if let Some(c) = self.unlock_s {
            range(out, "unlock_s", c, 0, MAX_INITIAL_COOLDOWN_S);
        }
        range(
            out,
            "max_alive",
            self.max_alive as u32,
            0,
            crate::variant::MAX_ALIVE_CAP as u32,
        );
        if let Some(g) = &self.limit_group {
            check_id(out, "limit_group", g);
        }
        if let Some(t) = &self.training {
            check_id(out, "training", t);
        }
        if let Some(w) = self.ai_weight {
            range(out, "ai_weight", w, 0, 1000);
        }
        if self.ai_heavy && self.vehicle.is_none() {
            out.push(
                Issue::new("ai_heavy", "only a vehicle card can be a heavy card")
                    .with_hint("remove ai_heavy"),
            );
        }
        if let Some(n) = &self.short_name {
            let len = n.chars().count();
            if n.trim().is_empty() || len > crate::variant::MAX_SHORT_NAME {
                out.push(Issue::new(
                    "short_name",
                    format!(
                        "must be 1..={} characters, got {len}",
                        crate::variant::MAX_SHORT_NAME
                    ),
                ));
            }
        }
        for (i, src) in self.sources.iter().enumerate() {
            if src.trim().is_empty() {
                out.push(Issue::new(format!("sources[{i}]"), "must not be empty"));
            }
        }
        let mut months = [None, None];
        for (k, (key, v)) in [
            ("available_from", &self.available_from),
            ("available_to", &self.available_to),
        ]
        .into_iter()
        .enumerate()
        {
            if let Some(v) = v {
                months[k] = crate::variant::parse_month(v);
                if months[k].is_none() {
                    out.push(
                        Issue::new(key, format!("`{v}` is not a month \"YYYY-MM\""))
                            .with_hint(format!("{key} = \"1944-06\"")),
                    );
                }
            }
        }
        if let [Some(a), Some(b)] = months
            && a > b
        {
            out.push(Issue::new(
                "available_from",
                "must not be after available_to",
            ));
        }
        crate::variant::check_labels(out, &self.tags, &self.capabilities);
        let has_vehicle_tag = self.has_tag("vehicle");
        if !self.tags.is_empty() && has_vehicle_tag != self.vehicle.is_some() {
            out.push(
                Issue::new(
                    "tags",
                    "the reserved tag `vehicle` marks exactly the cards that name a vehicle",
                )
                .with_hint(if self.vehicle.is_some() {
                    "add \"vehicle\" to tags"
                } else {
                    "remove \"vehicle\" from tags"
                }),
            );
        }
    }
}

impl ItemDef {
    pub fn validate(&self) -> Vec<Issue> {
        let mut out = Vec::new();
        check_id(&mut out, "id", &self.id);
        check_name(&mut out, "name", &self.name);
        // S11b G1: a rifle grenade reaches farther than a hand throw.
        let rg = self.kind == ItemKind::RifleGrenade;
        range(
            &mut out,
            "range_m",
            self.range_m,
            1,
            if rg { 300 } else { 100 },
        );
        range(
            &mut out,
            "stow_ms",
            self.stow_ms,
            0,
            if rg { 60_000 } else { 0 },
        );
        if self.min_range_m >= self.range_m {
            out.push(Issue::new(
                "min_range_m",
                format!(
                    "must be below range_m ({}), got {}",
                    self.range_m, self.min_range_m
                ),
            ));
        }
        range(&mut out, "windup_ms", self.windup_ms, 0, 10_000);
        range(&mut out, "fuse_ms", self.fuse_ms, 0, 30_000);
        range(&mut out, "scatter_pct", self.scatter_pct, 0, 100);
        let he = matches!(
            self.kind,
            ItemKind::Frag | ItemKind::Satchel | ItemKind::Bangalore | ItemKind::Mine
        );
        if he != self.he.is_some() {
            out.push(
                Issue::new(
                    "he",
                    "frag and satchel items need an [he] table, smoke must not",
                )
                .with_hint("[he] damage = 150 ..."),
            );
        }
        if (self.kind == ItemKind::Smoke) != self.smoke.is_some() {
            out.push(
                Issue::new("smoke", "smoke items need a [smoke] table, others must not")
                    .with_hint("[smoke] radius_m = 8 ..."),
            );
        }
        if matches!(self.kind, ItemKind::SingleShotAt | ItemKind::RifleGrenade)
            != self.weapon.is_some()
        {
            out.push(
                Issue::new(
                    "weapon",
                    "single_shot_at and rifle_grenade items need `weapon` (the launcher def they fire as), others must not",
                )
                .with_hint("weapon = \"ww2:pzf60\""),
            );
        }
        if let Some(h) = &self.he {
            for i in h.validate() {
                out.push(Issue {
                    path: format!("he.{}", i.path),
                    ..i
                });
            }
        }
        if let Some(sp) = &self.supply {
            sp.check("supply", &mut out);
        }
        if (self.kind == ItemKind::Bangalore) != self.bangalore.is_some() {
            out.push(
                Issue::new(
                    "bangalore",
                    "bangalore items need a [bangalore] table, others must not",
                )
                .with_hint("[bangalore] length_cm = 300 ..."),
            );
        }
        if (self.kind == ItemKind::Mine) != self.mine.is_some() {
            out.push(
                Issue::new("mine", "mine items need a [mine] table, others must not")
                    .with_hint("[mine] trigger = \"foot\"\ntrigger_cm = 30"),
            );
        }
        if let Some(m) = &self.mine {
            if !MINE_TRIGGERS.contains(&m.trigger.as_str()) {
                out.push(Issue::new(
                    "mine.trigger",
                    format!("`{}` is not one of foot, vehicle", m.trigger),
                ));
            }
            range(&mut out, "mine.trigger_cm", m.trigger_cm, 1, 200);
        }
        if let Some(b) = &self.bangalore {
            range(&mut out, "bangalore.length_cm", b.length_cm, 50, 1000);
            range(&mut out, "bangalore.bursts", b.bursts, 1, 8);
            range(&mut out, "bangalore.place_s", b.place_s, 0, 120);
        }
        if let Some(s) = &self.smoke {
            range(&mut out, "smoke.radius_m", s.radius_m, 1, 50);
            range(&mut out, "smoke.grow_ms", s.grow_ms, 0, 60_000);
            range(&mut out, "smoke.hold_ms", s.hold_ms, 0, 300_000);
            range(&mut out, "smoke.fade_ms", s.fade_ms, 0, 60_000);
        }
        out
    }
}

impl ItemHeDef {
    /// Checks one blast; paths are relative to the `[he]` table.
    pub fn validate(&self) -> Vec<Issue> {
        let mut out = Vec::new();
        range(&mut out, "damage", self.damage, 0, 10_000);
        range(&mut out, "suppression", self.suppression, 0, 100);
        check_frag(
            &mut out,
            "",
            [
                self.blast_radius_cm,
                self.frag_count,
                self.frag_reach_cm,
                self.frag_damage,
                self.frag_far_pct,
                self.frag_track_damage,
            ],
            self.frag_elev_deg,
        );
        range(
            &mut out,
            "suppression_radius_m",
            self.suppression_radius_m,
            1,
            200,
        );
        range(&mut out, "crater_radius_cm", self.crater_radius_cm, 0, 2000);
        range(&mut out, "crater_depth_cm", self.crater_depth_cm, 0, 500);
        check_he_armour(
            &mut out,
            "",
            [
                self.pen_mm,
                self.inside_damage,
                self.inside_rays,
                self.inside_reach_cm,
                self.vs_vehicle_pct,
            ],
        );
        range(&mut out, "frag_pen_mm", self.frag_pen_mm, 0, 100);
        out
    }
}

impl WeaponDef {
    pub fn validate(&self) -> Vec<Issue> {
        let mut out = Vec::new();
        check_id(&mut out, "id", &self.id);
        check_name(&mut out, "name", &self.name);
        range(&mut out, "range_m", self.range_m, 1, 5000);
        if self.min_range_m >= self.range_m {
            out.push(Issue::new(
                "min_range_m",
                format!(
                    "must be below range_m ({}), got {}",
                    self.range_m, self.min_range_m
                ),
            ));
        }
        range(&mut out, "damage", self.damage, 0, 10_000);
        range(&mut out, "rounds_per_min", self.rounds_per_min, 1, 1800);
        range(&mut out, "magazine", self.magazine, 1, 1000);
        range(&mut out, "reload_ms", self.reload_ms, 0, 60_000);
        range(&mut out, "spread_mrad", self.spread_mrad, 0, 500);
        range(&mut out, "spread_urad", self.spread_urad, 0, 500_000);
        if self.spread_mrad != 0 && self.spread_urad != 0 {
            out.push(
                Issue::new("spread_urad", "set spread_mrad or spread_urad, not both")
                    .with_hint("spread_urad = 2500 for 2.5 mrad"),
            );
        }
        if let Some(v) = self.fire_reveal_pct {
            range(&mut out, "fire_reveal_pct", v, 100, 1000);
        }
        if let Some(v) = self.fire_reveal_ms {
            range(&mut out, "fire_reveal_ms", v, 0, 60_000);
        }
        let launcher = self.class == WeaponClass::Launcher;
        for (key, v) in [
            ("engage_still_m", self.engage_still_m),
            ("engage_moving_m", self.engage_moving_m),
        ] {
            match v {
                Some(_) if !launcher => out.push(Issue::new(key, "only launchers have it")),
                Some(m) => range(&mut out, key, m, 1, self.range_m),
                None => {}
            }
        }
        match (&self.ammo, launcher) {
            (None, true) => out.push(
                Issue::new("ammo", "a launcher names the round it fires")
                    .with_hint("ammo = \"ww2:my_rocket\""),
            ),
            (Some(_), false) => out.push(Issue::new(
                "ammo",
                "only launchers name a round; a cannon's rounds are on its vehicle mount",
            )),
            _ => {}
        }
        if launcher {
            range(&mut out, "rounds_carried", self.rounds_carried, 1, 50);
        } else if self.rounds_carried != 0 {
            out.push(Issue::new(
                "rounds_carried",
                "only launchers have rounds_carried",
            ));
        }
        // S4: every crew weapon has a hand-over time (launchers since S3).
        if self.is_crew_weapon() {
            range(&mut out, "handover_ms", self.handover_ms, 0, 60_000);
        } else if self.handover_ms != 0 {
            out.push(
                Issue::new(
                    "handover_ms",
                    "only crew weapons (MGs, mortars, launchers or crew_weapon = true) have handover_ms",
                )
                .with_hint("crew_weapon = true"),
            );
        }
        if let Some(n) = &self.short_name
            && (n.trim().is_empty() || n.chars().count() > crate::variant::MAX_SHORT_NAME)
        {
            out.push(Issue::new(
                "short_name",
                format!(
                    "must be 1..={} characters, got {}",
                    crate::variant::MAX_SHORT_NAME,
                    n.chars().count()
                ),
            ));
        }
        if let Some(l) = self.load {
            range(&mut out, "load", l, 1, 65_535);
        }
        range(&mut out, "burst_rounds", self.burst_rounds, 0, 1000);
        range(&mut out, "burst_pause_ms", self.burst_pause_ms, 0, 10_000);
        if self.hold_beyond_pct != 0 {
            range(&mut out, "hold_beyond_pct", self.hold_beyond_pct, 10, 100);
        }
        if let Some(ind) = &self.indirect {
            ind.check(self, &mut out);
        }
        if self.class == WeaponClass::Cannon && self.he.is_some() {
            out.push(Issue::new(
                "he",
                "a cannon's HE is a round on its vehicle mount (defs/ammo), not [he]",
            ));
        }
        range(
            &mut out,
            "muzzle_velocity_m_s",
            self.muzzle_velocity_m_s,
            1,
            3000,
        );
        range(&mut out, "setup_ms", self.setup_ms, 0, 60_000);
        range(&mut out, "suppression", self.suppression, 0, 100);
        match (&self.he, self.class) {
            (None, WeaponClass::Mortar) => out.push(
                Issue::new("he", "a mortar needs an [he] table")
                    .with_hint("[he] blast_radius_cm = 150 ..."),
            ),
            (Some(he), _) => {
                check_he_armour(
                    &mut out,
                    "he.",
                    [
                        he.pen_mm,
                        he.inside_damage,
                        he.inside_rays,
                        he.inside_reach_cm,
                        he.vs_vehicle_pct,
                    ],
                );
                check_frag(
                    &mut out,
                    "he.",
                    [
                        he.blast_radius_cm,
                        he.frag_count,
                        he.frag_reach_cm,
                        he.frag_damage,
                        he.frag_far_pct,
                        he.frag_track_damage,
                    ],
                    he.frag_elev_deg,
                );
                range(
                    &mut out,
                    "he.suppression_radius_m",
                    he.suppression_radius_m,
                    1,
                    200,
                );
                range(
                    &mut out,
                    "he.crater_radius_cm",
                    he.crater_radius_cm,
                    0,
                    2000,
                );
                range(&mut out, "he.crater_depth_cm", he.crater_depth_cm, 0, 500);
            }
            (None, _) => {}
        }
        if let Some(he) = &self.he {
            range(&mut out, "he.frag_pen_mm", he.frag_pen_mm, 0, 100);
        }
        check_pen_table(&mut out, &self.penetration, self.core.as_ref());
        out
    }
}

/// S8a: a weapon's `penetration` rows and `core`.
fn check_pen_table(out: &mut Vec<Issue>, rows: &[[u32; 2]], core: Option<&SmallCoreDef>) {
    if rows.len() > MAX_PEN_ROWS {
        out.push(Issue::new(
            "penetration",
            format!("at most {MAX_PEN_ROWS} rows, got {}", rows.len()),
        ));
    }
    for (i, [r, mm]) in rows.iter().enumerate() {
        range(out, &format!("penetration[{i}][0]"), *r, 0, 5000);
        range(out, &format!("penetration[{i}][1]"), *mm, 0, 100);
        if i > 0 && rows[i - 1][0] >= *r {
            out.push(Issue::new(
                format!("penetration[{i}]"),
                "ranges must rise strictly",
            ));
        }
    }
    match (rows.is_empty(), core) {
        (false, None) => out.push(
            Issue::new("core", "a weapon with `penetration` needs a `core`")
                .with_hint("core = { damage = 35, length_cm = 40, max_boxes = 1 }"),
        ),
        (true, Some(_)) => out.push(Issue::new("core", "`core` needs `penetration`")),
        _ => {}
    }
    if let Some(c) = core {
        range(out, "core.damage", c.damage, 1, 1000);
        range(out, "core.length_cm", c.length_cm, 1, 500);
        range(out, "core.max_boxes", c.max_boxes, 1, 4);
    }
}

/// Parses `#rrggbb` into `0xRRGGBB`.
pub fn parse_color(s: &str) -> Option<u32> {
    let hex = s.strip_prefix('#')?;
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    u32::from_str_radix(hex, 16).ok()
}

impl FactionDef {
    pub fn validate(&self) -> Vec<Issue> {
        let mut out = Vec::new();
        check_id(&mut out, "id", &self.id);
        check_name(&mut out, "name", &self.name);
        if parse_color(&self.color).is_none() {
            out.push(
                Issue::new("color", format!("`{}` is not a #rrggbb colour", self.color))
                    .with_hint("color = \"#7d8b4a\""),
            );
        }
        if self.roster.is_empty() {
            out.push(Issue::new("roster", "must list at least one squad"));
        }
        for (i, s) in self.start_squads.iter().enumerate() {
            if !self.roster.contains(s) {
                out.push(
                    Issue::new(
                        format!("start_squads[{i}]"),
                        format!("`{s}` is not in the roster"),
                    )
                    .with_hint("add it to `roster` or remove it here"),
                );
            }
        }
        out
    }
}

impl MapFileDef {
    pub fn validate(&self) -> Vec<Issue> {
        let mut out = Vec::new();
        check_id(&mut out, "id", &self.id);
        check_name(&mut out, "name", &self.name);
        let size = self.size_m;
        if !(MAP_SIZE_STEP_M..=MAP_SIZE_MAX_M).contains(&size)
            || !size.is_multiple_of(MAP_SIZE_STEP_M)
        {
            out.push(Issue::new(
                "size_m",
                format!("must be a multiple of 64 in 64..=8192, got {size}"),
            ));
        }
        let inside = |out: &mut Vec<Issue>, path: String, p: [u32; 2]| {
            if p[0] > size || p[1] > size {
                out.push(Issue::new(
                    path,
                    format!("[{}, {}] is outside the {size} m map", p[0], p[1]),
                ));
            }
        };
        let t = &self.terrain;
        range(&mut out, "terrain.base_height_m", t.base_height_m, 0, 1000);
        for (i, o) in t.octaves.iter().enumerate() {
            let p = format!("terrain.octaves[{i}]");
            if o.period_m < 8 || o.period_m > size.max(8) || !o.period_m.is_multiple_of(4) {
                out.push(Issue::new(
                    format!("{p}.period_m"),
                    format!("must be a multiple of 4 in 8..=size_m, got {}", o.period_m),
                ));
            }
            range(
                &mut out,
                &format!("{p}.amplitude_cm"),
                o.amplitude_cm,
                0,
                10_000,
            );
        }
        for (i, r) in t.ridges.iter().enumerate() {
            let p = format!("terrain.ridges[{i}]");
            inside(&mut out, format!("{p}.from"), r.from);
            inside(&mut out, format!("{p}.to"), r.to);
            range(&mut out, &format!("{p}.height_m"), r.height_m, 0, 200);
            range(&mut out, &format!("{p}.width_m"), r.width_m, 8, 4000);
        }
        for (i, v) in t.valleys.iter().enumerate() {
            let p = format!("terrain.valleys[{i}]");
            inside(&mut out, format!("{p}.from"), v.from);
            inside(&mut out, format!("{p}.to"), v.to);
            range(&mut out, &format!("{p}.depth_m"), v.depth_m, 0, 200);
            range(&mut out, &format!("{p}.width_m"), v.width_m, 8, 4000);
        }
        for (i, w) in self.woods.iter().enumerate() {
            inside(&mut out, format!("woods[{i}].center"), w.center);
            range(
                &mut out,
                &format!("woods[{i}].radius_m"),
                w.radius_m,
                1,
                1000,
            );
        }
        if self.capture_points.is_empty() {
            out.push(Issue::new("capture_points", "need at least one point"));
        }
        if self.capture_points.len() > crate::MAX_CAPTURE_POINTS {
            out.push(Issue::new("capture_points", "too many points"));
        }
        for (i, c) in self.capture_points.iter().enumerate() {
            check_name(&mut out, &format!("capture_points[{i}].name"), &c.name);
            inside(&mut out, format!("capture_points[{i}].pos"), c.pos);
            range(
                &mut out,
                &format!("capture_points[{i}].value"),
                c.value,
                1,
                100,
            );
        }
        if self.bases.len() < 2 {
            out.push(Issue::new(
                "bases",
                "need at least two bases (one per player)",
            ));
        }
        for (i, b) in self.bases.iter().enumerate() {
            inside(&mut out, format!("bases[{i}].pos"), b.pos);
            range(
                &mut out,
                &format!("bases[{i}].radius_m"),
                b.radius_m,
                4,
                500,
            );
        }
        out
    }
}

impl RulesDef {
    pub fn validate(&self) -> Vec<Issue> {
        let mut out = Vec::new();
        check_id(&mut out, "id", &self.id);
        check_name(&mut out, "name", &self.name);
        if !crate::TICK_HZ_ALLOWED.contains(&self.tick_hz) {
            out.push(
                Issue::new("tick_hz", format!("must be 30 or 20, got {}", self.tick_hz))
                    .with_hint("tick_hz = 30"),
            );
        }
        range(&mut out, "hash_interval", self.hash_interval, 1, 10_000);
        range(&mut out, "tickets", self.tickets, 1, 1_000_000);
        range(&mut out, "pop_cap", self.pop_cap, 1, 10_000);
        range(&mut out, "projectile_cap", self.projectile_cap, 1, 1 << 20);
        range(
            &mut out,
            "economy.start_manpower",
            self.economy.start_manpower,
            0,
            1_000_000,
        );
        range(
            &mut out,
            "economy.income_per_min",
            self.economy.income_per_min,
            0,
            1_000_000,
        );
        range(
            &mut out,
            "economy.point_income_per_min",
            self.economy.point_income_per_min,
            0,
            1_000_000,
        );
        range(
            &mut out,
            "tickets_drain.per_min_per_point",
            self.tickets_drain.per_min_per_point,
            0,
            100_000,
        );
        range(&mut out, "capture.radius_m", self.capture.radius_m, 1, 100);
        range(&mut out, "capture.time_s", self.capture.time_s, 1, 600);
        range(
            &mut out,
            "capture.max_men",
            self.capture.max_men as u32,
            1,
            64,
        );
        let m = &self.movement;
        range(
            &mut out,
            "movement.slope_cost_pct",
            m.slope_cost_pct,
            0,
            10_000,
        );
        range(&mut out, "movement.max_slope_pct", m.max_slope_pct, 1, 1000);
        range(&mut out, "movement.arrive_cm", m.arrive_cm, 5, 500);
        range(
            &mut out,
            "movement.retreat_speed_pct",
            m.retreat_speed_pct,
            10,
            500,
        );
        range(
            &mut out,
            "movement.catch_up_speed_pct",
            m.catch_up_speed_pct,
            100,
            300,
        );
        range(
            &mut out,
            "movement.prone_speed_pct",
            m.prone_speed_pct,
            10,
            100,
        );
        range(
            &mut out,
            "movement.path_budget_per_tick",
            m.path_budget_per_tick,
            64,
            1_000_000,
        );
        range(
            &mut out,
            "movement.path_max_expansions",
            m.path_max_expansions,
            64,
            1_000_000,
        );
        range(
            &mut out,
            "movement.trench_cost_pct",
            m.trench_cost_pct,
            10,
            100,
        );
        let pits = &self.dig.profiles;
        for (name, d) in [
            ("trench", Some(&self.dig.profiles.trench)),
            ("at_ditch", Some(&self.dig.profiles.at_ditch)),
            ("mortar_pit", pits.mortar_pit.as_ref()),
            ("hq_pit", pits.hq_pit.as_ref()),
            ("dump_pit", pits.dump_pit.as_ref()),
            ("gun_pit", pits.gun_pit.as_ref()),
            ("mg_nest_pit", pits.mg_nest_pit.as_ref()),
            ("dugout_pit", pits.dugout_pit.as_ref()),
            ("bunker_pit", pits.bunker_pit.as_ref()),
            ("aid_pit", pits.aid_pit.as_ref()),
        ]
        .into_iter()
        .filter_map(|(n, d)| d.map(|d| (n, d)))
        {
            let p = format!("dig.profiles.{name}");
            range(&mut out, &format!("{p}.depth_cm"), d.depth_cm, 10, 1000);
            range(&mut out, &format!("{p}.floor_cm"), d.floor_cm, 10, 1000);
            range(&mut out, &format!("{p}.wall_cm"), d.wall_cm, 10, 500);
            range(&mut out, &format!("{p}.parapet_cm"), d.parapet_cm, 0, 300);
            range(
                &mut out,
                &format!("{p}.max_length_m"),
                d.max_length_m,
                1,
                1000,
            );
            range(
                &mut out,
                &format!("{p}.work_per_m"),
                d.work_per_m,
                1,
                10_000_000,
            );
            range(&mut out, &format!("{p}.passes"), d.passes, 1, 32);
            range(
                &mut out,
                &format!("{p}.pit_length_cm"),
                d.pit_length_cm,
                0,
                2000,
            );
            if d.pit_length_cm > 0 && d.pit_length_cm < d.floor_cm + 2 * d.wall_cm + 20 {
                out.push(Issue::new(
                    format!("{p}.pit_length_cm"),
                    "a pit is at least its floor and both walls plus 20 cm long",
                ));
            }
        }
        let v = &self.vehicles;
        for (name, val, lo, hi) in [
            ("unlock_s", v.unlock_s, 0, 3600),
            ("limit", v.limit, 0, 50),
            ("wreck_cap", v.wreck_cap, 1, 256),
            ("seat_swap_s", v.seat_swap_s, 0, 60),
            ("crew_hp", v.crew_hp, 1, 1000),
            ("wounded_hp", v.wounded_hp, 0, v.crew_hp),
            ("moving_cm_s", v.moving_cm_s, 1, 1000),
            ("moving_spot_pct", v.moving_spot_pct, 1, 1000),
            ("near_spot_m", v.near_spot_m, 0, 200),
            (
                "moving_target_spread_pct",
                v.moving_target_spread_pct,
                100,
                1000,
            ),
            ("ranging_step_pct", v.ranging_step_pct, 0, 100),
            ("ranging_floor_pct", v.ranging_floor_pct, 1, 100),
            ("he_area_spread_mrad", v.he_area_spread_mrad, 0, 100),
            ("he_clear_m", v.he_clear_m, 0, 500),
            ("crossing_dug_cm", v.crossing_dug_cm, 10, 1000),
            ("crossing_speed_cm_s", v.crossing_speed_cm_s, 10, 3000),
            ("crossing_gap_s", v.crossing_gap_s, 0, 60),
            ("fire_min_damage", v.fire_min_damage, 0, 1000),
            ("fire_start_pct", v.fire_start_pct, 0, 100),
            ("fire_burn_hp_s", v.fire_burn_hp_s, 0, 100),
            ("fire_check_s", v.fire_check_s, 1, 60),
            ("fire_out_pct", v.fire_out_pct, 0, 100),
            ("fire_burnout_s", v.fire_burnout_s, 1, 600),
            ("rack_min_damage", v.rack_min_damage, 0, 1000),
            ("rack_detonate_pct", v.rack_detonate_pct, 0, 100),
            ("base_repair_pct_s", v.base_repair_pct_s, 0, 100),
            ("base_rearm_s", v.base_rearm_s, 1, 600),
            ("base_crew_s", v.base_crew_s, 1, 600),
        ] {
            range(&mut out, &format!("vehicles.{name}"), val, lo, hi);
        }
        let c = &self.combat;
        for (name, v, lo, hi) in [
            ("spot_rays_per_tick", c.spot_rays_per_tick, 1, 100_000),
            ("spot_period_ms", c.spot_period_ms, 1, 10_000),
            ("spot_observers", c.spot_observers, 1, 16),
            ("prone_spot_pct", c.prone_spot_pct, 1, 1000),
            ("trench_spot_pct", c.trench_spot_pct, 1, 1000),
            ("woods_spot_pct", c.woods_spot_pct, 1, 1000),
            ("firing_spot_pct", c.firing_spot_pct, 1, 1000),
            ("reveal_ms", c.reveal_ms, 0, 60_000),
            ("near_miss_cm", c.near_miss_cm, 0, 2000),
            ("hit_suppression", c.hit_suppression, 0, 100),
            ("suppression_decay_per_s", c.suppression_decay_per_s, 0, 100),
            ("crouch_suppression", c.crouch_suppression, 1, 100),
            ("pin_suppression", c.pin_suppression, 1, 100),
            ("moving_spread_pct", c.moving_spread_pct, 1, 1000),
            ("pinned_spread_pct", c.pinned_spread_pct, 1, 1000),
            ("range_spread_pct", c.range_spread_pct, 0, 1000),
            ("fire_exposure_cm", c.fire_exposure_cm, 0, 200),
            ("corpse_s", c.corpse_s, 0, 600),
        ] {
            range(&mut out, &format!("combat.{name}"), v, lo, hi);
        }
        if c.pin_suppression < c.crouch_suppression {
            out.push(Issue::new(
                "combat.pin_suppression",
                "must be at least crouch_suppression",
            ));
        }
        range(&mut out, "items.resupply_s", self.items.resupply_s, 1, 600);
        range(
            &mut out,
            "items.crew_pickup_reach_m",
            self.items.crew_pickup_reach_m,
            1,
            1_000,
        );
        // S11b G1, G3.
        range(
            &mut out,
            "items.rifle_grenades",
            self.items.rifle_grenades,
            0,
            1,
        );
        range(
            &mut out,
            "items.rifle_grenade_gap_s",
            self.items.rifle_grenade_gap_s,
            0,
            600,
        );
        if c.spot_rays_per_tick < c.spot_observers {
            out.push(Issue::new(
                "combat.spot_rays_per_tick",
                "must be at least spot_observers",
            ));
        }
        for (name, t) in [
            ("easy", &self.difficulty.easy),
            ("normal", &self.difficulty.normal),
            ("hard", &self.difficulty.hard),
        ] {
            let p = format!("difficulty.{name}");
            range(&mut out, &format!("{p}.income_pct"), t.income_pct, 0, 1000);
            range(
                &mut out,
                &format!("{p}.reaction_ms"),
                t.reaction_ms,
                0,
                60_000,
            );
            range(
                &mut out,
                &format!("{p}.replan_ms"),
                t.replan_ms,
                100,
                60_000,
            );
            range(
                &mut out,
                &format!("{p}.decision_quality_pct"),
                t.decision_quality_pct,
                0,
                100,
            );
        }
        let a = &self.ai;
        for (name, v, lo, hi) in [
            ("retreat_strength_pct", a.retreat_strength_pct, 0, 100),
            ("threat_radius_m", a.threat_radius_m, 1, 500),
            ("dig_length_m", a.dig_length_m, 2, 1000),
            ("dig_offset_m", a.dig_offset_m, 0, 100),
            ("throw_cooldown_s", a.throw_cooldown_s, 1, 600),
            ("frag_group_men", a.frag_group_men, 1, 16),
            ("tank_standoff_m", a.tank_standoff_m, 1, 2000),
            ("assault_close_m", a.assault_close_m, 1, 500),
            ("at_clear_s", a.at_clear_s, 1, 600),
            ("repair_clear_m", a.repair_clear_m, 1, 2000),
        ] {
            range(&mut out, &format!("ai.{name}"), v, lo, hi);
        }
        a.check_mix(&mut out);
        if !self.call_in.date.is_empty()
            && crate::variant::parse_month(&self.call_in.date).is_none()
        {
            out.push(
                Issue::new(
                    "call_in.date",
                    format!("`{}` is not a month \"YYYY-MM\"", self.call_in.date),
                )
                .with_hint("[call_in] date = \"1944-06\" (or \"\" for no date filter)"),
            );
        }
        if let Some(t) = &self.training {
            check_id(&mut out, "training.default", &t.default);
        }
        for (g, &n) in &self.limit_groups {
            check_id(&mut out, &format!("limit_groups.\"{g}\""), g);
            range(&mut out, &format!("limit_groups.\"{g}\""), n, 0, 1000);
        }
        if a.dig_length_m > self.dig.profiles.trench.max_length_m {
            out.push(Issue::new(
                "ai.dig_length_m",
                "must be at most dig.profiles.trench.max_length_m",
            ));
        }
        range(
            &mut out,
            "capture.extra_man_pct",
            self.capture.extra_man_pct,
            0,
            1000,
        );
        range(
            &mut out,
            "movement.ditch_cost_pct",
            self.movement.ditch_cost_pct,
            1,
            10_000,
        );
        let call_in = self.call_in.tuning();
        check_tuning(
            &mut out,
            "",
            TuningRefs {
                body: &c.body,
                mortar: &c.mortar,
                blast: &c.blast,
                suppression: &c.suppression,
                suppress: &c.suppress,
                ai_infantry: &a.infantry,
                ai_armour: &a.armour,
                ai_assault: &a.assault,
                driving: &v.driving,
                vehicle_path: &v.path,
                escort: &v.escort,
                gunnery: &v.gunnery,
                bail: &v.bail,
                recrew: &v.recrew,
                transport: &v.transport,
                guns: &v.guns,
                vehicle_damage: &v.damage,
                formation: &self.movement.formation,
                cover: &self.cover,
                throw: &self.items.throw,
                dig_common: &self.dig.common,
                call_in: &call_in,
                supply: self.supply.as_ref(),
                share: &self.combat.share.clone().unwrap_or_default(),
                fire: &self.combat.fire.clone().unwrap_or_default(),
                ai_ammo: &a.ammo.clone().unwrap_or_default(),
                fire_mission: self.fire_mission.as_ref(),
                fortify: self.fortify.as_ref(),
                ai_mortar: &a.mortar_tuning(),
                ai_supply: &a.supply.clone().unwrap_or_default(),
                ai_transport: &a.transport.clone().unwrap_or_default(),
                ai_recon: &a.recon.clone().unwrap_or_default(),
                ai_truck: &a.truck.clone().unwrap_or_default(),
                ai_guns: &a.guns.clone().unwrap_or_default(),
                ai_fort: &a.fort.clone().unwrap_or_default(),
                ai_obstacles: &a.obstacles.clone().unwrap_or_default(),
                ai_arty: &a.arty.clone().unwrap_or_default(),
                wounds: &self.wounds.clone().unwrap_or_default(),
                ai_medic: &a.medic.clone().unwrap_or_default(),
                armour: &self.armour.clone().unwrap_or_default(),
            },
        );
        match (a.barrage_rounds, &a.mortar) {
            (Some(_), Some(_)) => out.push(
                Issue::new(
                    "ai.barrage_rounds",
                    "replaced by [ai.mortar] mission_rounds; set only that",
                )
                .with_hint("delete ai.barrage_rounds"),
            ),
            (Some(r), None) => range(&mut out, "ai.barrage_rounds", r, 1, 50),
            _ => {}
        }
        out
    }
}

/// The legacy `mix_*_pct` keys of `[ai]` and the `[ai.mix]` tag each maps
/// to (S4: the old form maps 1:1).
pub const LEGACY_MIX_KEYS: [(&str, &str); 6] = [
    ("mix_rifle_pct", "line"),
    ("mix_mg_pct", "mg"),
    ("mix_engineer_pct", "engineer"),
    ("mix_mortar_pct", "mortar"),
    ("mix_vehicle_pct", "vehicle"),
    ("mix_at_pct", "at"),
];

impl AiDef {
    fn legacy_mix(&self) -> [Option<u32>; 6] {
        [
            self.mix_rifle_pct,
            self.mix_mg_pct,
            self.mix_engineer_pct,
            self.mix_mortar_pct,
            self.mix_vehicle_pct,
            self.mix_at_pct,
        ]
    }

    /// The target mix by tag, from whichever form the rules use.
    pub fn mix_by_tag(&self) -> BTreeMap<String, u32> {
        if let Some(m) = &self.mix {
            return m.clone();
        }
        LEGACY_MIX_KEYS
            .iter()
            .zip(self.legacy_mix())
            .map(|((_, tag), v)| (tag.to_string(), v.unwrap_or(0)))
            .collect()
    }

    fn check_mix(&self, out: &mut Vec<Issue>) {
        let legacy = self.legacy_mix();
        let any_legacy = legacy.iter().any(Option::is_some);
        match &self.mix {
            Some(_) if any_legacy => {
                for ((key, tag), v) in LEGACY_MIX_KEYS.iter().zip(legacy) {
                    if v.is_some() {
                        out.push(
                            Issue::new(
                                format!("ai.{key}"),
                                "set the mix either as [ai.mix] or as the mix_*_pct keys",
                            )
                            .with_hint(format!("move it to [ai.mix] {tag} = ...")),
                        );
                    }
                }
            }
            Some(m) => {
                for (tag, &v) in m {
                    let p = format!("ai.mix.{tag}");
                    if !crate::variant::is_label(tag, crate::variant::MAX_TAG_LEN) {
                        out.push(Issue::new(&p, format!("`{tag}` is not a tag")));
                    }
                    range(out, &p, v, 0, 100);
                }
                if m.iter()
                    .filter(|(t, _)| *t != "vehicle" && *t != "at")
                    .all(|(_, &v)| v == 0)
                {
                    out.push(Issue::new(
                        "ai.mix",
                        "the infantry mix (tags other than vehicle and at) must not be all zero",
                    ));
                }
            }
            None => {
                for ((key, _), v) in LEGACY_MIX_KEYS.iter().zip(legacy) {
                    match v {
                        None => out.push(
                            Issue::new(
                                format!("ai.{key}"),
                                "required (or give the whole mix as [ai.mix] by tag)",
                            )
                            .with_hint(format!("{key} = 0")),
                        ),
                        Some(v) => range(out, &format!("ai.{key}"), v, 0, 100),
                    }
                }
                if legacy[..4].iter().map(|v| v.unwrap_or(0)).sum::<u32>() == 0 && any_legacy {
                    out.push(Issue::new(
                        "ai.mix_rifle_pct",
                        "the mix must not be all zero",
                    ));
                }
            }
        }
    }
}
