//! Gameplay tuning tables (S3b, the quick-iteration rule): values that used
//! to be Rust constants in the sim and now live in the rules TOML
//! (`defs/rules/*.toml`), so a tuning change is a data edit.
//!
//! Each table is one plain struct used both as the def (parsed from TOML)
//! and in the [`crate::MatchSetup`] (copied unchanged): every value stays an
//! integer in the unit its key names (`_mm`, `_m`, `_s`, `_pct`, `_deg`,
//! ...), and the sim converts it at the point of use exactly as the old
//! constant was converted, so moving a constant into data changes no
//! result. [`check`](BodyTuning::check) runs on the def (load) and again on
//! the setup (saves are untrusted).
//!
//! Engine and structural constants (pool and list caps, search sizes, RNG
//! draw counts, substep rate, grid sizes) stay in code; see
//! `docs/slices/S3b-balance.md` "Tuning surface".

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::issue::Issue;

/// Pushes an issue when `table.field` is outside `lo..=hi`.
fn check_range(out: &mut Vec<Issue>, table: &str, field: &str, v: u32, lo: u32, hi: u32) {
    if v < lo || v > hi {
        let fix = v.clamp(lo, hi);
        out.push(
            Issue::new(
                format!("{table}.{field}"),
                format!("must be in {lo}..={hi}, got {v}"),
            )
            .with_hint(format!("[{table}] {field} = {fix}")),
        );
    }
}

/// Pushes an issue unless `a <= b` (two keys of one table).
fn check_order(out: &mut Vec<Issue>, table: &str, a: (&str, u32), b: (&str, u32)) {
    if a.1 > b.1 {
        out.push(
            Issue::new(
                format!("{table}.{}", a.0),
                format!("must not exceed {} ({} > {})", b.0, a.1, b.1),
            )
            .with_hint(format!("[{table}] {} = {}", a.0, b.1)),
        );
    }
}

/// A tuning key: its name and allowed range (inclusive).
pub type TuningKey = (&'static str, u32, u32);

/// One tuning table: a struct of `u32` keys, each with its allowed range.
macro_rules! tuning_table {
    (
        $(#[$meta:meta])*
        pub struct $name:ident {
            $(
                $(#[doc = $doc:expr])*
                $field:ident : $lo:literal ..= $hi:literal $(= $def:literal)?,
            )*
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
        #[serde(deny_unknown_fields)]
        pub struct $name {
            $( $(#[doc = $doc])* $(#[serde(default = $def)])? pub $field: u32, )*
        }

        impl $name {
            /// Range checks; `table` is the table's dotted path (e.g.
            /// `combat.body`), used in the issue paths and fixes.
            pub fn check(&self, table: &str, out: &mut Vec<Issue>) {
                $( check_range(out, table, stringify!($field), self.$field, $lo, $hi); )*
                self.check_more(table, out);
            }

            /// Every key with its allowed range (docs and tests).
            pub const KEYS: &'static [TuningKey] =
                &[$( (stringify!($field), $lo, $hi), )*];
        }
    };
}

tuning_table! {
    /// `[combat.body]`: a soldier's hit capsule and posture points. Heights
    /// are from the feet.
    pub struct BodyTuning {
        /// Capsule height standing (mm).
        stand_mm: 1000..=2500,
        /// Capsule height crouched (mm).
        crouch_mm: 500..=2500,
        /// Capsule height prone (mm).
        prone_mm: 100..=1000,
        /// Capsule radius standing or crouched (mm).
        radius_mm: 50..=600,
        /// Prone: a horizontal capsule along the heading, this radius ...
        prone_radius_mm: 50..=600,
        /// ... and this half length (mm).
        prone_half_mm: 100..=1500,
        /// The muzzle sits this far below the top of the firing capsule (mm).
        muzzle_drop_mm: 0..=1000,
        /// Shooters aim at a man on a fire step this far below his top (mm).
        head_aim_mm: 0..=1000,
        /// Eyes (and the head spotting point) this far below the top (mm).
        eye_drop_mm: 0..=1000,
        /// A man this far below the base surface stands in a trench (mm).
        trench_depth_mm: 100..=3000,
        /// Slower than this counts as standing still (MG set-up, mortar
        /// set-up, moving spread) (mm/s).
        still_mm_s: 1..=5000,
    }
}

impl BodyTuning {
    fn check_more(&self, table: &str, out: &mut Vec<Issue>) {
        check_order(
            out,
            table,
            ("prone_mm", self.prone_mm),
            ("crouch_mm", self.crouch_mm),
        );
        check_order(
            out,
            table,
            ("crouch_mm", self.crouch_mm),
            ("stand_mm", self.stand_mm),
        );
    }
}

tuning_table! {
    /// `[combat.mortar]`: how mortar teams lay and fire a barrage.
    pub struct MortarTuning {
        /// Muzzle height above the crew's feet (mm).
        muzzle_mm: 0..=3000,
        /// Flight time is `sqrt(range · this / 1000)` seconds (350: about a
        /// 60° arc at every range) ...
        flight_milli: 1..=5000,
        /// ... and at least this (s).
        min_flight_s: 1..=30,
        /// A team holding out of range walks to 90 % of its range again
        /// unless that spot is within this of its last walk (m).
        barrage_retry_m: 1..=200,
    }
}

impl MortarTuning {
    fn check_more(&self, _: &str, _: &mut Vec<Issue>) {}
}

tuning_table! {
    /// `[combat.blast]`: HE bursts of shells, rockets and thrown items.
    pub struct BlastTuning {
        /// Damage and sight rays from a burst start this far above the
        /// burst point (mm).
        lift_mm: 0..=2000,
        /// Crater depth roll, percent of the weapon's crater depth: from ...
        crater_depth_min_pct: 0..=500,
        /// ... to (inclusive).
        crater_depth_max_pct: 0..=500,
        /// A burst more than this above the ground under it carves no
        /// crater (on a vehicle) (mm).
        crater_lift_mm: 0..=5000,
        /// S3c (B8, T9): a ground burst where the ground falls away (a
        /// parapet crest, a trench lip) also throws fragments downward over
        /// the fall: the ground is sampled this far around the burst, and
        /// the steepest drop extends the HE def's elevation band below its
        /// minimum, with extra rays at the band's density (cm; 0: off).
        frag_fall_probe_cm: 0..=1000,
    }
}

impl BlastTuning {
    fn check_more(&self, table: &str, out: &mut Vec<Issue>) {
        check_order(
            out,
            table,
            ("crater_depth_min_pct", self.crater_depth_min_pct),
            ("crater_depth_max_pct", self.crater_depth_max_pct),
        );
    }
}

tuning_table! {
    /// `[combat.suppression]`: how incoming fire suppresses (the thresholds
    /// and decay are the `[combat]` keys).
    pub struct SuppressionTuning {
        /// A near miss on a man crouched below cover (in a trench, off the
        /// fire step) adds this share of the weapon's suppression (%).
        covered_near_miss_pct: 0..=100,
        /// A burst with earth between it and a man suppresses him this
        /// much (%).
        blast_behind_cover_pct: 0..=100,
        /// Each point of suppression widens a shooter's spread by this
        /// many hundredths of a percent (100: 1 % per point) (%).
        spread_per_point_pct: 0..=1000,
        /// S3b duck state: a man in a trench at or above this suppression
        /// ducks below the lip (no fire, no throw) (points) ...
        duck_points: 0..=100,
        /// ... and comes back up only below this (points). Both scale with
        /// the squad type's `steadiness_pct`, as the pin threshold does.
        duck_release_points: 0..=100,
        /// A ducked man's suppression decays at this share of the normal
        /// rate: he stays down until the fire slackens (%).
        duck_decay_pct: 0..=100,
    }
}

impl SuppressionTuning {
    fn check_more(&self, table: &str, out: &mut Vec<Issue>) {
        check_order(
            out,
            table,
            ("duck_release_points", self.duck_release_points),
            ("duck_points", self.duck_points),
        );
    }
}

tuning_table! {
    /// `[combat.suppress]` (S3b): the Suppress order, area fire at a point
    /// or short line without a spotted target.
    pub struct SuppressTuning {
        /// Each shot aims this high above the ground on the line (the
        /// undug surface over a trench) (cm).
        aim_cm: 0..=500,
        /// The longest suppress line (m).
        line_max_m: 1..=200,
        /// A shot is fired only with a clear line from the muzzle to a
        /// point this far short of its aim point (so rounds aimed just over
        /// a parapet still go) (cm).
        clear_cm: 0..=2000,
        /// A vehicle on Suppress fires its main gun's HE at the line at
        /// most this often (s) ...
        tank_he_s: 1..=600,
        /// ... while more than this many HE rounds are left ...
        tank_he_reserve: 0..=200,
        /// ... aimed this high above the ground at the line's middle (cm;
        /// low, so the shell bursts on the parapet, not beyond it).
        tank_he_aim_cm: 0..=500,
    }
}

impl SuppressTuning {
    fn check_more(&self, _: &str, _: &mut Vec<Issue>) {}
}

tuning_table! {
    /// `[ai.assault]` (S3b): the AI's fire-and-move, AT-team hunts and
    /// satchel hunts.
    pub struct AiAssaultTuning {
        /// A base of fire stands at most this far from the line it
        /// suppresses: an MG team (m) ...
        support_mg_m: 10..=1000,
        /// ... a rifle squad (m) ...
        support_rifle_m: 10..=1000,
        /// ... a vehicle (m).
        support_tank_m: 10..=2000,
        /// Manoeuvre squads advance in bounds: a squad more than this far
        /// ahead of the rearmost one halts and fires (m).
        bound_m: 5..=500,
        /// The base lifts its fire once a manoeuvre squad's leader is this
        /// close to the line (m).
        lift_fire_m: 0..=200,
        /// The suppress line along an enemy trench is this long (m; at most
        /// `combat.suppress.line_max_m`).
        trench_line_m: 1..=200,
        /// An AT team hunts a known enemy vehicle this close to it or to
        /// its point (m) ...
        at_hunt_m: 0..=2000,
        /// ... unless a known enemy soldier is this close to the middle of
        /// its route (m) ...
        at_hunt_clear_m: 0..=500,
        /// ... and stops this much inside its launcher's `engage_still` (m).
        at_hunt_inset_m: 0..=200,
        /// An engineer squad with a satchel goes for a known enemy vehicle
        /// within this (m) ...
        satchel_hunt_m: 0..=500,
        /// ... that has been slower than `ai.infantry.satchel_slow_mm_s`
        /// (stopped, bogged, immobilised) for this long (s), walking to a
        /// spot this far outside the hull side nearer it (m).
        satchel_slow_s: 0..=120,
        satchel_stand_m: 1..=50,
        /// S3c sticky hunts: a hunt goes on while its vehicle was seen
        /// within this (s; meanwhile the squad goes to where it was last
        /// seen) ...
        satchel_stick_s: 0..=600,
        /// ... while it has not been faster than
        /// `ai.infantry.satchel_slow_mm_s` for longer than this (s) ...
        satchel_chase_s: 0..=600,
        /// ... and while it is within `satchel_hunt_m` plus this of the
        /// squad (m).
        satchel_leash_m: 0..=1000,
        /// Hunters crouch on the approach and crawl (prone) within this of
        /// the vehicle while its hull MG's arc covers them (m; 0: never
        /// crawl).
        satchel_crawl_m: 0..=200,
        /// A hunt ends after this long (s).
        satchel_hunt_max_s: 1..=1800,
        /// S3c second base of fire: an assault on a point with at least
        /// three squads gets a second base when its first base's fire
        /// weight (rounds per minute x suppression, summed over its men)
        /// is below this share of the enemy faction's strongest MG team
        /// weapon (%; 0: never) ...
        second_base_below_pct: 0..=1000,
        /// ... from a spot at least this far from the first base (m).
        second_base_gap_m: 0..=500,
        /// S3c rockets (B3): AT teams also hunt an immobile or bogged known
        /// enemy vehicle this close to the team (m; at most 2000; 0: only
        /// `at_hunt_m`).
        at_hunt_stuck_m: 0..=2000,
        /// S3c rockets (B3): an AT team without a hunt follows an own
        /// vehicle on an assault this close to it, staying this far behind
        /// it (m; 0: off).
        at_follow_m: 0..=1000,
        at_follow_back_m: 0..=500,
        /// S4 Panzerfaust hunts: a squad with a single-shot AT item goes
        /// for a known enemy vehicle this close to it (m; 0: off; absent in
        /// older rules: 100) ...
        pzf_hunt_m: 0..=500 = "default_pzf_hunt_m",
        /// ... unless a known enemy soldier is this close to the middle of
        /// its route (m; absent: 40).
        pzf_clear_m: 0..=500 = "default_pzf_clear_m",
        /// S5a Panzerfaust ambush hold (S4-42 / T12): a known enemy vehicle
        /// this close to the point a Panzerfaust squad holds (or to the
        /// squad) starts an ambush (m; 0: off; absent: 0, so older rules
        /// keep the S4 hunt, S5a-29 (A)) ...
        ambush_watch_m: 0..=1000 = "default_ambush_watch_m",
        /// ... the squad waits at a spot this far to the side of the
        /// vehicle's projected path (m; absent: 25) ...
        ambush_offset_m: 5..=200 = "default_ambush_offset_m",
        /// ... and gives up after this long (s; absent: 90).
        ambush_max_s: 1..=900 = "default_ambush_max_s",
        /// The ambush ends as "gone" only once the vehicle is seen beyond
        /// `ambush_release_m` (m; at least `ambush_watch_m`; absent or 0:
        /// `ambush_watch_m`, the S5a Part B behaviour). S5a Fix (review M1):
        /// a wider release than watch stops ambushes from starting and
        /// ending over and over at the edge of the watch.
        ambush_release_m: 0..=1500 = "default_ambush_release_m",
    }
}

fn default_ambush_watch_m() -> u32 {
    0
}
fn default_ambush_offset_m() -> u32 {
    25
}
fn default_ambush_max_s() -> u32 {
    90
}
fn default_ambush_release_m() -> u32 {
    0
}

fn default_pzf_hunt_m() -> u32 {
    100
}

fn default_pzf_clear_m() -> u32 {
    40
}

fn default_capture_m() -> u32 {
    150
}

impl AiAssaultTuning {
    fn check_more(&self, table: &str, out: &mut Vec<Issue>) {
        if self.ambush_release_m != 0 {
            check_order(
                out,
                table,
                ("ambush_watch_m", self.ambush_watch_m),
                ("ambush_release_m", self.ambush_release_m),
            );
        }
    }
}

tuning_table! {
    /// `[ai.infantry]`: the AI commander's infantry, mortar and satchel
    /// rules.
    pub struct AiInfantryTuning {
        /// Most squads bought per plan.
        buys_per_plan: 1..=8,
        /// Mortars never fire this close to own men (m).
        danger_close_m: 0..=500,
        /// A mortar's support spot is this far behind the point it
        /// supports (m).
        support_back_m: 0..=1000,
        /// Frag targets: spotted men within this of each other count as one
        /// group (m).
        frag_group_m: 1..=50,
        /// Satchels go at vehicles no faster than this (stopped, bogged,
        /// immobilised or crossing a trench at 1 m/s, with a margin)
        /// (mm/s) ...
        satchel_slow_mm_s: 0..=20000,
        /// ... landing this far outside the hull side (mm).
        satchel_side_mm: 0..=5000,
        /// Enemy trench probe: ground dug at least this deep on the
        /// approach (dm) ...
        trench_probe_dm: 1..=50,
        /// ... up to this far in front of the point (m) ...
        trench_probe_m: 1..=500,
        /// ... in steps of this (m).
        trench_probe_step_m: 1..=50,
        /// S7: a squad with frags or satchels this close to a known enemy
        /// open-topped vehicle throws into its open box (m; 0: off) ...
        open_top_throw_m: 0..=100 = "default_open_top_throw_m",
        /// ... when it is no faster than this (mm/s).
        open_top_slow_mm_s: 0..=20000 = "default_open_top_slow_mm_s",
    }
}

fn default_open_top_throw_m() -> u32 {
    25
}

fn default_open_top_slow_mm_s() -> u32 {
    300
}

impl AiInfantryTuning {
    fn check_more(&self, _: &str, _: &mut Vec<Issue>) {}
}

tuning_table! {
    /// `[ai.armour]`: the AI commander's vehicles and their escorts.
    pub struct AiArmourTuning {
        /// Most squads escorting one vehicle.
        escort_squads: 0..=8,
        /// Squads this close to a vehicle may join its escort (m).
        escort_join_m: 0..=2000,
        /// Escorts are released when their vehicle halts, but not in its
        /// first seconds (it waits for its path and its file) (s).
        escort_grace_s: 0..=600,
        /// An escorted assault has reached the enemy once an escort squad's
        /// leader is this close to the enemy trench or point (m).
        escort_reached_m: 1..=1000,
        /// Below this speed a vehicle counts as halted (mm/s).
        halted_mm_s: 0..=20000,
        /// Idle vehicles turn toward a threat further off their heading
        /// than this (deg) ...
        face_slack_deg: 0..=180,
        /// ... within this distance (m).
        face_range_m: 0..=2000,
        /// Enemy vehicles this close to a point are an armour threat to it
        /// (m).
        threat_m: 0..=2000,
        /// A vehicle retreats with its engine under this share of its HP
        /// (%).
        retreat_engine_pct: 0..=100,
        /// A vehicle backing up an own point stands this far behind it (m).
        backup_m: 0..=1000,
        /// After the vehicle unlock, while under the vehicle limit, infantry
        /// call-ins leave at least this much manpower unspent toward the
        /// next vehicle (S3b) (MP).
        tank_reserve_mp: 0..=100000,
        /// S3c rearm (S3b-42): a vehicle with every limited MG dry, HE at
        /// or below `combat.suppress.tank_he_reserve` or its penetrator at
        /// or below this (rounds), and no known enemy vehicle within
        /// `threat_m`, drives back to base ...
        rearm_ap_below: 0..=200,
        /// ... and stays until its main-gun rounds and MG stowage are this
        /// full (%) ...
        rearm_full_pct: 0..=100,
        /// ... or this long in base has passed (s).
        rearm_max_s: 0..=1800,
        /// S3c re-crew: a crew squad re-crews an abandoned own vehicle (not
        /// burning, not useless) with no known enemy soldier this close to
        /// it (m) and no known enemy vehicle within `threat_m` ...
        recrew_clear_m: 0..=2000,
        /// ... its own vehicle, or the nearest of the others this close to
        /// the crew (m).
        recrew_reach_m: 0..=2000,
        /// S3c denial: a vehicle with no live target fires at an enemy
        /// abandoned vehicle this close (m; only with
        /// `vehicles.recrew.capture` on; 0: off).
        deny_m: 0..=2000,
        /// S4 AI capture (S3c-33 (A)): a crew squad (`vehicle_crew`) whose
        /// tank is gone captures an enemy abandoned vehicle this close to
        /// it (not burning, not useless, no known enemy soldier within
        /// `recrew_clear_m`), when the rules allow captures (m; 0: off;
        /// absent in older rules: 150).
        capture_m: 0..=2000 = "default_capture_m",
        /// S7 overwatch (assault guns and tank destroyers): the anchor
        /// stands this far behind the most threatened own or contested
        /// point, on the line to the threat (m; 0: off, they act as tanks)
        /// ...
        overwatch_back_m: 0..=1000 = "default_overwatch_back_m",
        /// ... and the spot is the best of this many on a ring ...
        overwatch_candidates: 1..=32 = "default_overwatch_candidates",
        /// ... of this radius around it (m).
        overwatch_ring_m: 0..=500 = "default_overwatch_ring_m",
        /// After this many main-gun shots from a spot (or a penetration)
        /// it moves to the next best spot ...
        relocate_after_shots: 1..=50 = "default_relocate_after_shots",
        /// ... at least this far from the old one (m).
        relocate_m: 0..=500 = "default_relocate_m",
        /// A known enemy vehicle this close turns the overwatch toward it
        /// (m).
        td_hunt_m: 0..=2000 = "default_td_hunt_m",
        /// Overwatch spots this close to known enemy infantry are passed
        /// over (m) ...
        td_infantry_standoff_m: 0..=1000 = "default_td_infantry_standoff_m",
        /// ... and an open-topped vehicle with known enemy infantry this
        /// close reverses away (m).
        open_top_infantry_m: 0..=500 = "default_open_top_infantry_m",
        /// S13 H1 switch (docs/slices/S13-heavy-wave.md 5): a card with
        /// `ai_heavy` is bought by its own path once enemy armour was seen
        /// (0: off; S13a: 0).
        heavy_buy: 0..=1 = "d_s13_zero",
        /// H1: an enemy tank, TD or assault gun seen this recently starts
        /// a heavy plan (s) ...
        heavy_seen_s: 0..=3600 = "d_s13_heavy_seen_s",
        /// ... and the AI saves for the heavy at most this long (s).
        heavy_save_max_s: 0..=3600 = "d_s13_heavy_save_max_s",
        /// S13 H2 switch: a vehicle def with `ai_overwatch` joins the TD/AG
        /// overwatch habit (0: off; S13a: 0).
        heavy_overwatch: 0..=1 = "d_s13_zero",
        /// S13 H3 switch: the push limit extends the rearm trip (0: off;
        /// S13a: 0) ...
        push_limit: 0..=1 = "d_s13_zero",
        /// ... a tank this far from the nearest rearm source (m) ...
        push_without_supply_m: 0..=5000 = "d_s13_push_without_supply_m",
        /// ... holding at most this many main-gun rounds goes back.
        tank_reserve_rounds: 0..=200 = "d_s13_tank_reserve_rounds",
    }
}

fn d_s13_zero() -> u32 {
    0
}
fn d_s13_heavy_seen_s() -> u32 {
    300
}
fn d_s13_heavy_save_max_s() -> u32 {
    240
}
fn d_s13_push_without_supply_m() -> u32 {
    300
}
fn d_s13_tank_reserve_rounds() -> u32 {
    10
}

fn default_overwatch_back_m() -> u32 {
    80
}

fn default_overwatch_candidates() -> u32 {
    8
}

fn default_overwatch_ring_m() -> u32 {
    30
}

fn default_relocate_after_shots() -> u32 {
    2
}

fn default_relocate_m() -> u32 {
    40
}

fn default_td_hunt_m() -> u32 {
    300
}

fn default_td_infantry_standoff_m() -> u32 {
    120
}

fn default_open_top_infantry_m() -> u32 {
    60
}

impl AiArmourTuning {
    fn check_more(&self, _: &str, _: &mut Vec<Issue>) {}
}

tuning_table! {
    /// `[call_in]`: where a bought squad spawns in its base (S3b: a
    /// call-in spawns at once, no queue and no delivery time).
    pub struct CallInTuning {
        /// Six spawn spots sit on a ring at this share of the base radius
        /// (the seventh is the centre) (%).
        spawn_ring_pct: 0..=100,
        /// A spot with a man or vehicle this close is passed over for the
        /// least crowded one, so squads bought in quick succession do not
        /// land on each other (m).
        spawn_clear_m: 0..=100,
        /// A vehicle counts as this many men when spots are compared.
        spawn_vehicle_weight: 1..=100,
    }
}

impl CallInTuning {
    fn check_more(&self, _: &str, _: &mut Vec<Issue>) {}
}

tuning_table! {
    /// `[vehicles.driving]`: how a vehicle follows its path, yields and
    /// sits on the ground.
    pub struct DrivingTuning {
        /// Hull pitch and roll limit (deg).
        max_tilt_deg: 1..=60,
        /// Fastest change of pitch and roll (deg/s).
        tilt_rate_deg_s: 1..=360,
        /// A vehicle's avoidance circle is half the hull width plus this
        /// (mm).
        circle_pad_mm: 0..=3000,
        /// The driver moves on to the next waypoint within this (m).
        waypoint_reach_m: 1..=50,
        /// The last waypoint counts as reached within this (mm).
        arrive_mm: 100..=20000,
        /// S8b: a wheeled vehicle moves on to the next waypoint within
        /// this (m; a turning circle is wider than a hull).
        wheeled_waypoint_reach_m: 1..=50,
        /// S8b: a wheeled vehicle has arrived within this (mm).
        wheeled_arrive_mm: 100..=20000,
        /// S8b: throttle while a wheeled vehicle steers on a large heading
        /// error or backs out of its turning circle (%).
        wheeled_creep_pct: 1..=100,
        /// S8b: stuck re-paths in one place (within
        /// `wheeled_waypoint_reach_m`) before a wheeled vehicle gives the
        /// move up and stops there.
        wheeled_stuck_repaths: 1..=100,
        /// Over this heading error the driver stops pushing and turns (deg)
        /// ...
        pivot_err_deg: 1..=180,
        /// ... pivoting on the spot below this speed (mm/s).
        pivot_speed_mm_s: 0..=20000,
        /// Full steer at this heading error and beyond (deg).
        full_steer_deg: 1..=180,
        /// Full throttle up to this heading error, falling to none at
        /// `pivot_err_deg` (deg).
        full_throttle_deg: 0..=180,
        /// A move to a target behind the hull and this close reverses (m).
        reverse_order_m: 0..=500,
        /// Yield box length, percent of the hull length (%).
        yield_len_pct: 0..=1000,
        /// A vehicle slower than this counts as standing (yield rule)
        /// (mm/s).
        yield_still_mm_s: 0..=20000,
        /// A friendly soldier this far ahead slows a vehicle to walking pace
        /// (m) ...
        friendly_ahead_m: 0..=50,
        /// ... which is this (mm/s).
        walk_mm_s: 1..=20000,
        /// Stuck check: less progress than this (m) ...
        stuck_progress_m: 0..=50,
        /// ... in this long re-paths (s).
        stuck_s: 1..=600,
        /// Heading offset to the right while a vehicle comes head-on (deg).
        keep_right_deg: 0..=90,
        /// A vehicle making no progress backs off for 1 s after this long
        /// (s).
        back_off_s: 1..=600,
        /// At most one re-path per vehicle per this long (s).
        repath_gap_s: 0..=600,
        /// A face order is done within this heading error (deg).
        face_done_deg: 1..=90,
        /// An approaching vehicle re-aims when its target moved this far
        /// (m).
        approach_slack_m: 0..=500,
        /// A soldier pushed out of a hull footprint lands this far past its
        /// side, beyond his own radius (mm) ...
        push_gap_mm: 0..=3000,
        /// ... never onto ground dug this much deeper (mm).
        push_deeper_mm: 0..=5000,
        /// Spotting check points sit this far below the hull and turret box
        /// tops (mm).
        spot_drop_mm: 0..=2000,
    }
}

impl DrivingTuning {
    fn check_more(&self, table: &str, out: &mut Vec<Issue>) {
        check_order(
            out,
            table,
            ("full_throttle_deg", self.full_throttle_deg),
            ("pivot_err_deg", self.pivot_err_deg),
        );
    }
}

tuning_table! {
    /// `[vehicles.path]`: vehicle path costs (percent of a normal step)
    /// and blocked ground.
    pub struct VehiclePathTuning {
        /// Woods step cost (%).
        woods_pct: 1..=10000,
        /// Ground dug at least `dug_dm` (craters) (%).
        dug_pct: 1..=10000,
        /// (dm)
        dug_dm: 1..=255,
        /// Ground dug at least `deep_dm` (trenches, big craters) (%).
        deep_pct: 1..=10000,
        /// (dm)
        deep_dm: 1..=255,
        /// The avoided node in a stuck re-path (%).
        avoid_pct: 100..=100000,
        /// A node next to a blocked one (the clearance band) (%).
        margin_pct: 100..=100000,
        /// AT-ditch lines block nodes within this (m).
        ditch_reach_m: 0..=50,
        /// A wreck's footprint is padded by this for the block layer (m).
        wreck_pad_m: 0..=50,
    }
}

impl VehiclePathTuning {
    fn check_more(&self, table: &str, out: &mut Vec<Issue>) {
        check_order(
            out,
            table,
            ("dug_dm", self.dug_dm),
            ("deep_dm", self.deep_dm),
        );
    }
}

tuning_table! {
    /// `[vehicles.escort]`: the file of infantry following a vehicle.
    pub struct EscortTuning {
        /// Rows start this far behind the hull (m) ...
        behind_m: 0..=100,
        /// ... and are this far apart, as are the files (mm).
        gap_mm: 100..=20000,
        /// Men per file.
        file_men: 1..=64,
    }
}

impl EscortTuning {
    fn check_more(&self, _: &str, _: &mut Vec<Issue>) {}
}

tuning_table! {
    /// `[vehicles.gunnery]`: vehicle weapons and hits on vehicles (the
    /// gun and turret rates are in each vehicle def).
    pub struct GunneryTuning {
        /// The coax engages men this close to the turret's line (deg).
        coax_cone_deg: 0..=90,
        /// A spall ray ends after this (m).
        spall_reach_m: 1..=30,
        /// Tank HE at infantry: enemy men within this of a man count as
        /// his group when the gunner picks the best group (m).
        he_group_m: 1..=50,
        /// Lead estimation error against a moving target: the gunner's
        /// estimate of its velocity is off by up to this much, triangular
        /// (%); it shrinks with ranging like the spread.
        lead_error_pct: 0..=200,
        /// `Auto` loads a gun's special penetrator (APCR) only against a
        /// target the standard round penetrates with less than this
        /// chance, and only when the special round's chance is higher (%).
        special_below_pct: 0..=100,
        /// S7 casemate gunnery: a target bearing outside the gun's arc
        /// less this margin makes the driver turn the hull toward it
        /// (deg; default 3).
        casemate_margin_deg: 0..=30 = "default_casemate_margin_deg",
        /// S7 open-top exposure: a thrown item going off inside an open box
        /// sends its inner rays in a cone of this full angle up from the
        /// floor (deg; default 90).
        open_top_item_cone_deg: 1..=180 = "default_open_top_item_cone_deg",
        /// S7: a throw aimed inside an enemy open box's footprint aims this
        /// high above its floor, so the lob drops in (cm; default 30).
        open_top_item_aim_cm: 0..=200 = "default_open_top_item_aim_cm",
        /// S8a (S8a-4): a face-hardened plate's thickness counts as this
        /// share of itself against every round (%; default 125, verify:
        /// FH plate resists small calibre about 1.2-1.4x RHA).
        face_hardened_pct: 100..=200 = "default_face_hardened_pct",
        /// S8a: a small-arms round meeting a plate more obliquely than
        /// this glances off (deg from the normal; default 70).
        small_arms_ricochet_deg: 30..=89 = "default_small_arms_ricochet_deg",
        /// S13 (docs/slices/S13-heavy-wave.md 3.6): `Auto` loads the
        /// special round only when its chance is at least this (%; 0: any
        /// higher chance, as before S13).
        special_min_pct: 0..=100 = "d_s13_zero",
    }
}

fn default_small_arms_ricochet_deg() -> u32 {
    70
}

fn default_face_hardened_pct() -> u32 {
    125
}

fn default_casemate_margin_deg() -> u32 {
    3
}

fn default_open_top_item_cone_deg() -> u32 {
    90
}

fn default_open_top_item_aim_cm() -> u32 {
    30
}

impl GunneryTuning {
    fn check_more(&self, _: &str, _: &mut Vec<Issue>) {}
}

tuning_table! {
    /// `[vehicles.damage]` (S3b): what a hit on the ammo stowage does and
    /// when too few crew knock a vehicle out. The rack's damage threshold
    /// and detonation chance are the `[vehicles]` keys `rack_min_damage`
    /// and `rack_detonate_pct`.
    pub struct VehicleDamageTuning {
        /// A rack hit for at least `rack_min_damage` that does not
        /// detonate starts an ammo fire with this chance (%) ...
        rack_fire_pct: 0..=100,
        /// ... which knocks the vehicle out after this long; the crew cannot
        /// put it out (s).
        ammo_fire_s: 1..=600,
    }
}

impl VehicleDamageTuning {
    fn check_more(&self, _: &str, _: &mut Vec<Issue>) {}
}

tuning_table! {
    /// `[vehicles.bail]` (S3c): when the surviving crew of a vehicle with a
    /// `crew_squad` bail out, and how they get out (S3c-1 (A)).
    pub struct BailTuning {
        /// 1: the crew bail out as soon as an ammo fire starts.
        ammo_fire_bail: 0..=1,
        /// The crew bail out of an engine fire still burning after this
        /// long (s; 0: never for engine fires).
        engine_fire_bail_s: 0..=600,
        /// The crew bail out when fewer than this many are alive (0: never;
        /// replaces S3b's `crew_ko_below`).
        crew_below: 0..=5,
        /// 1: the crew bail out of a vehicle whose main gun is destroyed and
        /// that cannot move (track, engine or transmission out).
        useless_bail: 0..=1,
        /// A vehicle with its gun destroyed or immobilised that is
        /// penetrated again within this long of the previous penetration is
        /// abandoned (s; 0: never).
        under_fire_s: 0..=120,
        /// A crewman below this many HP is wounded ...
        wounded_hp: 0..=1000,
        /// ... and takes this share of his hatch's exit time (%).
        wounded_exit_pct: 100..=400,
        /// A man out of the hatch stands this far outside the hull box (cm).
        exit_clear_cm: 10..=300,
        /// He cannot fire for this long after getting out (ms; climbing
        /// down).
        dismount_ms: 0..=10000,
        /// He arrives with this much suppression (points).
        bail_suppression: 0..=200,
        /// Most abandoned vehicles on the map; beyond it the oldest becomes
        /// a wreck (scrapped).
        abandoned_cap: 1..=64,
        /// A crew squad in its own base with no order is dismissed after
        /// this long (s; no refund).
        crew_dismiss_s: 1..=600,
    }
}

impl BailTuning {
    fn check_more(&self, _: &str, _: &mut Vec<Issue>) {}
}

tuning_table! {
    /// `[vehicles.transport]` (S8a, vehicles.md 6.6): infantry carriers.
    /// Defaults are the S8a starting values.
    pub struct TransportTuning {
        /// Transports can be called in from this much match time on (s),
        /// instead of `[vehicles] unlock_s`; never later than it (S8a-5).
        unlock_s: 0..=3600 = "default_transport_unlock_s",
        /// Men who dismount at once (knock-out, fire) cannot fire for this
        /// long and arrive with `[vehicles.bail] bail_suppression` (ms).
        stun_ms: 0..=10000 = "default_transport_stun_ms",
        /// A man boards within this of the door (cm).
        board_reach_cm: 50..=1000 = "default_transport_board_reach_cm",
        /// Nobody boards or gets out while the vehicle is faster (cm/s).
        move_block_cm_s: 0..=1000 = "default_transport_move_block_cm_s",
    }
}

fn default_transport_unlock_s() -> u32 {
    120
}
fn default_transport_stun_ms() -> u32 {
    2000
}
fn default_transport_board_reach_cm() -> u32 {
    300
}
fn default_transport_move_block_cm_s() -> u32 {
    100
}

tuning_table! {
    /// `[vehicles.guns]` (S9, docs/slices/S9-at-guns.md 4): served guns.
    /// Defaults are the S9 starting values.
    pub struct GunsTuning {
        /// A dead gunner's or loader's place is taken after this long by
        /// the lowest-priority living man (s).
        slot_swap_s: 0..=60 = "default_guns_slot_swap_s",
        /// A man at a slot fires his own weapon only while the gun has no
        /// target and an enemy is this close (m; 0: never).
        self_defence_m: 0..=500 = "default_guns_self_defence_m",
        /// A gun holding fire breaks it for men only this close (m).
        ambush_infantry_m: 0..=1000 = "default_guns_ambush_infantry_m",
        /// Fewest men at the slots who emplace, pack or re-lay a gun.
        min_men: 1..=16 = "default_guns_min_men",
        /// A gun with no man of its owner at a slot and none within this
        /// (m) for `abandon_s` turns neutral.
        hold_m: 1..=200 = "default_guns_hold_m",
        /// Seconds before an unattended gun turns neutral (S9, 3.6).
        abandon_s: 1..=600 = "default_guns_abandon_s",
        /// A tow's hitch point must be within this of the gun's trail tip
        /// (m) to hitch it.
        hitch_reach_m: 1..=50 = "default_guns_hitch_reach_m",
    }
}

fn default_guns_slot_swap_s() -> u32 {
    3
}
fn default_guns_ambush_infantry_m() -> u32 {
    200
}
fn default_guns_self_defence_m() -> u32 {
    40
}
fn default_guns_min_men() -> u32 {
    2
}
fn default_guns_hold_m() -> u32 {
    15
}
fn default_guns_abandon_s() -> u32 {
    10
}
fn default_guns_hitch_reach_m() -> u32 {
    6
}

impl Default for GunsTuning {
    fn default() -> GunsTuning {
        GunsTuning {
            slot_swap_s: default_guns_slot_swap_s(),
            self_defence_m: default_guns_self_defence_m(),
            ambush_infantry_m: default_guns_ambush_infantry_m(),
            min_men: default_guns_min_men(),
            hold_m: default_guns_hold_m(),
            abandon_s: default_guns_abandon_s(),
            hitch_reach_m: default_guns_hitch_reach_m(),
        }
    }
}

impl GunsTuning {
    fn check_more(&self, _table: &str, _out: &mut Vec<Issue>) {}
}

impl Default for TransportTuning {
    fn default() -> TransportTuning {
        TransportTuning {
            unlock_s: default_transport_unlock_s(),
            stun_ms: default_transport_stun_ms(),
            board_reach_cm: default_transport_board_reach_cm(),
            move_block_cm_s: default_transport_move_block_cm_s(),
        }
    }
}

impl TransportTuning {
    fn check_more(&self, _: &str, _: &mut Vec<Issue>) {}
}

tuning_table! {
    /// `[vehicles.recrew]` (S3c): who can man an abandoned vehicle again,
    /// and how (S3c-2, S3c-3 (A)).
    pub struct RecrewTuning {
        /// The squad boards within this of the hull (m).
        reach_m: 1..=20,
        /// Time for the men at one hatch to get in, per man (s); the hatches
        /// work at once.
        board_s: 1..=60,
        /// Fewest men a squad needs to re-crew (driver and gunner).
        min_men: 1..=5,
        /// 1: any infantry squad but AT teams may board, untrained.
        any_infantry: 0..=1,
        /// 1: an enemy squad may capture an abandoned vehicle.
        capture: 0..=1,
        /// 1: only a tank crew squad may capture an enemy's vehicle
        /// (infantry may still re-crew their own side's).
        capture_needs_crew: 0..=1,
        /// An untrained loader (or gunner without a loader) reloads this
        /// slowly (% of the reload time).
        untrained_reload_pct: 100..=400,
        /// An untrained gunner traverses the turret at this share (%).
        untrained_traverse_pct: 10..=100,
        /// An untrained gunner's spread (% of the trained spread).
        untrained_spread_pct: 100..=400,
    }
}

impl RecrewTuning {
    fn check_more(&self, _: &str, _: &mut Vec<Issue>) {}
}

tuning_table! {
    /// `[movement.formation]`: how squads walk, keep together and arrive,
    /// and how soldiers move round vehicles and over dug ground.
    pub struct FormationTuning {
        /// A leader (or a detached man) moves on to the next waypoint
        /// within this (m).
        waypoint_reach_m: 1..=50,
        /// A member this far from his slot is behind and speeds up (m).
        catch_up_m: 0..=50,
        /// A member this far from his slot makes the leader slow down (m)
        /// ...
        stretched_m: 1..=200,
        /// ... to this share of his speed (%).
        stretched_speed_pct: 1..=100,
        /// Members on the move aim this far past their (moving) slot (mm).
        slot_lead_mm: 0..=10000,
        /// Arriving gives up waiting for stragglers after this long (s).
        arrive_timeout_s: 1..=600,
        /// A man this close to a vehicle circle's rim, heading into it,
        /// slides round it (mm).
        slide_gap_mm: 0..=10000,
        /// Grades up to this on dug ground do not slow a soldier (%) ...
        steep_free_pct: 0..=1000,
        /// ... and his slowest pace on steep dug ground is this (%).
        steep_min_pct: 1..=100,
    }
}

impl FormationTuning {
    fn check_more(&self, _: &str, _: &mut Vec<Issue>) {}
}

tuning_table! {
    /// `[cover]`: where arriving men look for cover (a free trench spot
    /// first, else a spot with terrain between them and the nearest seen
    /// enemy).
    pub struct CoverTuning {
        /// Men look for cover this far from their line slot (m).
        radius_m: 1..=30,
        /// Only enemies this close to the goal count for terrain cover (m).
        enemy_m: 0..=5000,
        /// Two men's cover spots keep at least this far apart (mm).
        gap_mm: 0..=10000,
        /// A trench spot needs dug ground this far around it too (mm).
        trench_margin_mm: 0..=3000,
        /// Enemy eye height for the cover rays (mm) ...
        eye_mm: 0..=5000,
        /// ... and the covered man's chest height (mm).
        chest_mm: 0..=5000,
    }
}

impl CoverTuning {
    fn check_more(&self, _: &str, _: &mut Vec<Issue>) {}
}

tuning_table! {
    /// `[items.throw]`: how thrown items fly and where smoke reaches.
    pub struct ThrowTuning {
        /// The item leaves the hand this far below the top of the firing
        /// capsule (mm).
        release_drop_mm: 0..=2000,
        /// Shortest flight (a satchel dropped at the feet still arcs) (ms).
        min_flight_ms: 1..=5000,
        /// Lob steepness: tangent of the launch angle over flat ground, in
        /// thousandths (1000 = 45°).
        lob_tan_milli: 100..=10000,
        /// A smoke cloud reaches this far below its centre (a sight line
        /// along a slope) (mm).
        smoke_below_mm: 0..=20000,
    }
}

impl ThrowTuning {
    fn check_more(&self, _: &str, _: &mut Vec<Issue>) {}
}

tuning_table! {
    /// `[dig.common]`: the spoil bank shape of every dig profile, and how
    /// close a digger works.
    pub struct DigCommonTuning {
        /// The parapet rises over this run from the lip (mm) ...
        parapet_rise_mm: 1..=5000,
        /// ... stays at full height over this run (mm) ...
        parapet_top_mm: 0..=5000,
        /// ... and falls back to the ground over this run (mm).
        parapet_fall_mm: 1..=5000,
        /// The parapet only raises ground at most this far below the base,
        /// so it never fills another trench or a crater (mm).
        parapet_undug_mm: 0..=1000,
        /// A digger works when this close to his spot (mm).
        reach_mm: 100..=10000,
        /// S3b: an HE crater centred on raised spoil (a parapet) blows the
        /// spoil away within its radius down to this share of the crater
        /// depth below the spoil's top, never below the base surface, and
        /// raises no rim there (%).
        spoil_crater_pct: 0..=1000,
    }
}

impl DigCommonTuning {
    fn check_more(&self, _: &str, _: &mut Vec<Issue>) {}
}

tuning_table! {
    /// `[supply]` (S5a): ammunition prices and base refill rates. Owner
    /// decision D1 (2026-11-01): no free ammunition anywhere, the base
    /// included. A rules def without this table keeps unlimited rounds and
    /// free refills (legacy content, S5a-22).
    pub struct SupplyTuning {
        /// Price of small-arms rounds (rifle, SMG, pistol, MG, vehicle MG
        /// belts) in manpower per tonne (200 = 0.2 per kg).
        small_arms_mp_per_t: 0..=100000,
        /// Price of explosives (grenades, satchels, AT rockets,
        /// Panzerfausts) (MP per t).
        explosives_mp_per_t: 0..=100000,
        /// Price of shells (mortar bombs, main-gun rounds) (MP per t).
        shell_mp_per_t: 0..=100000,
        /// D4: a full main-gun rack refill costs at most this share of the
        /// vehicle's card (%).
        rack_refill_max_card_pct: 1..=1000,
        /// Fallback lever: main-gun rounds cost this share of their shell
        /// price (%; the rack factor is the lower of this and the D4
        /// ceiling).
        rack_rearm_price_pct: 0..=1000,
        /// D3: the most rare rounds any vehicle def may hold (`rare_cap`).
        rare_cap_max: 0..=100,
        /// D3: one rare round per this many seconds in the own base.
        rare_rearm_base_s: 1..=600,
        /// Small-arms rounds per second per man refilled in the own base.
        base_small_arms_rounds_s: 1..=1000,
        /// One mortar bomb per this many seconds per man in the own base
        /// (S5a). 0: the base refills no bombs; they come only in crates
        /// (S6, owner Q4 and S5a-42).
        base_bomb_s: 0..=600,
        /// No bomb goes into a mortar squad in the own base until this long
        /// after its mortar last fired (s; 0 or absent: no wait). S5a Fix
        /// (review M2): brakes the base battery (artillery-on-map.md §4.10)
        /// until S6 crates.
        base_bomb_after_fire_s: 0..=1800 = "default_base_bomb_after_fire_s",
        /// S11a1 G7: 1 = an infantry gun rearms in the own base at the
        /// shells price (as an AT gun); 0 = at an HQ, dump or truck only.
        arty_base_rearm: 0..=1 = "d_fm_one",
        /// HUD "low" marker below this share of a squad's load (%).
        low_pct: 1..=100,
        /// S6: a crate this close to a mortar's gunner, a man or a tank's
        /// hull feeds it (m).
        crate_feed_m: 1..=50 = "d_crate_feed_m",
        /// S6: one crate issued in the own base per this many seconds per
        /// player (s).
        base_crate_issue_s: 0..=600 = "d_base_crate_issue_s",
        /// S6: live crates per player (carried or set down) ...
        crate_cap: 0..=100 = "d_crate_cap",
        /// ... of which shells crates.
        shell_crate_cap: 0..=100 = "d_shell_crate_cap",
        /// S6: built or digging HQs per player ...
        hq_max: 0..=4 = "d_hq_max",
        /// ... and ammunition dumps.
        dump_max: 0..=16 = "d_dump_max",
        /// S12 W8: aid posts per player, built or digging.
        aid_max: 0..=8 = "d_aid_max",
        /// S6: one bomb into a mortar squad inside an HQ or dump radius
        /// per this many seconds (s) ...
        structure_bomb_s: 1..=600 = "d_structure_bomb_s",
        /// ... and from a shells crate within `crate_feed_m` of its
        /// gunner (s).
        crate_bomb_s: 1..=600 = "d_crate_bomb_s",
        /// S6 (D3): one standard main-gun round forward (HQ, dump, shells
        /// crate) per this many seconds (s) ...
        fwd_rearm_s: 1..=600 = "d_fwd_rearm_s",
        /// ... and one rare round, up to the vehicle's `rare_cap` (s).
        rare_rearm_fwd_s: 1..=600 = "d_rare_rearm_fwd_s",
        /// S6: a source serves this many men per service ...
        serve_men: 1..=50 = "d_serve_men",
        /// ... one service per this many milliseconds (ms).
        serve_period_ms: 100..=10000 = "d_serve_period_ms",
        /// S6: a man is served once he has not fired this long (s).
        unfired_s: 0..=60 = "d_unfired_s",
        /// S6: hit points a bullet takes from a crate or structure box ...
        bullet_box_damage: 0..=1000 = "d_bullet_box_damage",
        /// ... and a main-gun round that does not burst (AP) (hp).
        shell_box_damage: 0..=10000 = "d_shell_box_damage",
        /// S6: a broken box holding shells or explosives cooks off one
        /// burst per this many kg ...
        cookoff_kg_per_burst: 1..=1000 = "d_cookoff_kg_per_burst",
        /// ... at most this many bursts ...
        cookoff_max: 0..=32 = "d_cookoff_max",
        /// ... spread over this many seconds (s).
        cookoff_s: 1..=60 = "d_cookoff_s",
        /// S6: an own man this close picks up a crate (m).
        pickup_m: 1..=20 = "d_pickup_m",
        /// S8b (7.3): MG rounds per second a vehicle's stowage takes from a
        /// forward source (HQ, dump, truck; 0: none, the S6 behaviour).
        fwd_mg_rounds_s: 0..=1000 = "d_fwd_mg_rounds_s",
        /// S8b (7.3): kg per second of small arms or explosives a stopped
        /// truck loads in base (shells share the crate queue).
        truck_load_kg_s: 1..=1000 = "d_truck_load_kg_s",
        /// S8b: a truck drops one crate per this many seconds (s).
        drop_s: 1..=60 = "d_drop_s",
        /// S8b: a truck this close to its drop point stops and drops (m).
        drop_reach_m: 1..=50 = "d_drop_reach_m",
        /// S8b (S8b-9): kg of shells and explosives in the cargo for a hit
        /// to detonate it; below this it only burns.
        cargo_detonate_min_kg: 0..=5000 = "d_cargo_detonate_min_kg",
    }
}

fn default_base_bomb_after_fire_s() -> u32 {
    0
}

// S6 defaults for a `[supply]` table without the S6 keys (the S5a
// fixtures): docs/slices/S6-hq-crates.md "Rules".
fn d_crate_feed_m() -> u32 {
    5
}
fn d_base_crate_issue_s() -> u32 {
    10
}
fn d_crate_cap() -> u32 {
    12
}
fn d_shell_crate_cap() -> u32 {
    8
}
fn d_hq_max() -> u32 {
    1
}
fn d_dump_max() -> u32 {
    3
}
fn d_aid_max() -> u32 {
    2
}
fn d_structure_bomb_s() -> u32 {
    2
}
fn d_crate_bomb_s() -> u32 {
    4
}
fn d_fwd_rearm_s() -> u32 {
    4
}
fn d_rare_rearm_fwd_s() -> u32 {
    30
}
fn d_serve_men() -> u32 {
    6
}
fn d_serve_period_ms() -> u32 {
    500
}
fn d_unfired_s() -> u32 {
    3
}
fn d_bullet_box_damage() -> u32 {
    4
}
fn d_shell_box_damage() -> u32 {
    400
}
fn d_cookoff_kg_per_burst() -> u32 {
    16
}
fn d_cookoff_max() -> u32 {
    8
}
fn d_cookoff_s() -> u32 {
    6
}
fn d_pickup_m() -> u32 {
    2
}
fn d_fwd_mg_rounds_s() -> u32 {
    0
}
fn d_truck_load_kg_s() -> u32 {
    8
}
fn d_drop_s() -> u32 {
    3
}
fn d_drop_reach_m() -> u32 {
    8
}
fn d_cargo_detonate_min_kg() -> u32 {
    40
}
fn d_tank_crate_far_m() -> u32 {
    300
}

tuning_table! {
    /// `[ai.supply]` (S6): the AI's HQ, crate runs and supply to tanks.
    pub struct AiSupplyTuning {
        /// The HQ goes this far behind the most valuable held point,
        /// toward the own base (m).
        hq_behind_m: 0..=1000,
        /// Candidate HQ spots tested per plan (reverse-slope rays) ...
        hq_probe_rays: 1..=32,
        /// A mortar or the HQ below this share of its bombs or stock gets
        /// a crate run (%).
        crate_run_below_pct: 0..=100,
        /// A tank below this share of its main-gun rounds that cannot
        /// reach a source gets a shells crate carried to it (%).
        tank_crate_below_pct: 0..=100,
        /// ... when it cannot drive, or (not on a rearm trip) when every
        /// own source (the base, a built HQ or dump) is farther than this
        /// straight line: the spec's "path budget" (m; review S6 F3).
        tank_crate_far_m: 0..=2000 = "d_tank_crate_far_m",
        /// At most this share of income goes on crates (%).
        shell_mp_max_pct: 0..=100,
        /// The AI buys its HQ squad after this many seconds of match (s).
        hq_after_s: 0..=3600,
        /// A mortar squad with no bombs and no crate run for this long
        /// walks to the HQ or base (s).
        dry_walk_s: 1..=3600,
        /// S6b: the AI keeps one bearer squad per this many own mortar
        /// squads, rounded up, at least one (0, the default: one bearer
        /// squad, as in S6).
        mortars_per_bearers: 0..=64 = "d_mortars_per_bearers",
    }
}

fn d_mortars_per_bearers() -> u32 {
    0
}

impl AiSupplyTuning {
    fn check_more(&self, _: &str, _: &mut Vec<Issue>) {}
}

tuning_table! {
    /// `[ai.transport]` (S8a, docs/slices/S8a-halftracks.md 7): how the AI
    /// uses its half-tracks. Defaults are the S8a starting values.
    pub struct AiTransportTuning {
        /// A squad rides only to an objective farther than this from the
        /// vehicle; a ride this long counts as transported (m).
        min_ride_m: 0..=2000 = "d_tr_min_ride_m",
        /// The vehicle waits at most this long for its squad to board, then
        /// goes to support alone (s).
        wait_s: 1..=600 = "d_tr_wait_s",
        /// The men get off this far short of the first point on the way
        /// within `dismount_sight_m` of a known enemy with a clear line (m).
        dismount_back_m: 0..=500 = "d_tr_dismount_back_m",
        /// ... (m).
        dismount_sight_m: 0..=2000 = "d_tr_dismount_sight_m",
        /// Sight-line rays per check (0: no line test, distance only).
        los_rays: 0..=32 = "d_tr_los_rays",
        /// A penetration this recent counts as "hit": everyone out (s).
        hit_s: 0..=60 = "d_tr_hit_s",
        /// A rider above this suppression gets everyone out (%).
        rider_suppression_pct: 1..=100 = "d_tr_rider_suppression_pct",
        /// Stopped this long on the way: the men get off where it stands
        /// (s).
        stuck_s: 1..=600 = "d_tr_stuck_s",
        /// After unloading, the vehicle supports from overwatch this long
        /// before it returns for the next squad (s).
        support_s: 0..=3600 = "d_tr_support_s",
        /// A move that ends farther than this from its goal is a failed
        /// move (m) ...
        arrive_m: 1..=200 = "d_tr_arrive_m",
        /// ... and after this many the job uses where the vehicle stands.
        max_fails: 1..=10 = "d_tr_max_fails",
        /// No job sends the same move again within this (s).
        repath_s: 1..=120 = "d_tr_repath_s",
        /// S8b: squads within this of the vehicle may be picked (m).
        pick_m: 1..=2000 = "d_tr_pick_m",
        /// S8b: the vehicle meets a squad farther than `meet_stand_m` this
        /// far ahead of it on its way (m) ...
        meet_ahead_m: 0..=500 = "d_tr_meet_ahead_m",
        /// ... and the men board when it stands within this (m).
        meet_stand_m: 1..=200 = "d_tr_meet_stand_m",
        /// S8b: after getting off a squad keeps going to its point this
        /// long unless enemies are near (s; 0: no lock).
        ride_lock_s: 0..=600 = "d_tr_ride_lock_s",
        /// S8b: a ride this long counts as "transported" (report; the S8a
        /// threshold, kept while `min_ride_m` picks longer rides) (m).
        counted_ride_m: 0..=2000 = "d_tr_counted_ride_m",
        /// S8b: the ride ends this far short of the point (m).
        drop_short_m: 0..=500 = "d_tr_drop_short_m",
        /// S8b: 1: the unload is ordered on arrival, while it still rolls;
        /// 0: once stopped.
        unload_rolling: 0..=1 = "d_tr_unload_rolling",
    }
}

fn d_tr_min_ride_m() -> u32 {
    150
}
fn d_tr_wait_s() -> u32 {
    30
}
fn d_tr_dismount_back_m() -> u32 {
    40
}
fn d_tr_dismount_sight_m() -> u32 {
    300
}
fn d_tr_los_rays() -> u32 {
    8
}
fn d_tr_hit_s() -> u32 {
    2
}
fn d_tr_rider_suppression_pct() -> u32 {
    50
}
fn d_tr_stuck_s() -> u32 {
    10
}
fn d_tr_support_s() -> u32 {
    60
}
fn d_tr_arrive_m() -> u32 {
    15
}
fn d_tr_max_fails() -> u32 {
    2
}
fn d_tr_repath_s() -> u32 {
    5
}
fn d_tr_pick_m() -> u32 {
    250
}
fn d_tr_meet_ahead_m() -> u32 {
    30
}
fn d_tr_meet_stand_m() -> u32 {
    20
}
fn d_tr_ride_lock_s() -> u32 {
    60
}
fn d_tr_counted_ride_m() -> u32 {
    120
}
fn d_tr_drop_short_m() -> u32 {
    60
}
fn d_tr_unload_rolling() -> u32 {
    1
}

tuning_table! {
    /// `[ai.recon]` (S8b, docs/slices/S8b-trucks-recon.md 9): how the AI
    /// uses its armoured cars.
    pub struct AiReconTuning {
        /// First car from this match time (s) ...
        after_s: 0..=3600,
        /// ... a second one from this (s).
        second_after_s: 0..=3600,
        /// All its cars cost at most this share of the income so far (%).
        income_max_pct: 0..=100,
        /// It observes from this far short of the point the infantry goes
        /// for (m).
        view_back_m: 0..=1000,
        /// Known AT teams, tanks and tank destroyers this close make it
        /// back off (m; 0: never) ...
        at_threat_m: 0..=2000,
        /// ... this far per order (m).
        backoff_m: 1..=500,
    }
}

impl AiReconTuning {
    fn check_more(&self, _: &str, _: &mut Vec<Issue>) {}
}

impl Default for AiReconTuning {
    /// S8b starting values (a rules def without `[ai.recon]`).
    fn default() -> AiReconTuning {
        AiReconTuning {
            after_s: 90,
            second_after_s: 480,
            income_max_pct: 14,
            view_back_m: 150,
            at_threat_m: 200,
            backoff_m: 60,
        }
    }
}

tuning_table! {
    /// `[ai.guns]` (S9, docs/slices/S9-at-guns.md 12): how the AI buys,
    /// sites and holds its AT guns.
    pub struct AiGunsTuning {
        /// It buys a gun from this match time (s), or once an enemy
        /// vehicle has been seen.
        after_s: 0..=3600,
        /// All its gun cards cost at most this share of the income so far
        /// (%): that share is set aside for them from the time it may buy.
        income_max_pct: 0..=100,
        /// Manpower that stays after a gun is bought.
        reserve_mp: 0..=10000,
        /// Guns alive or called in at once.
        max_guns: 0..=8,
        /// Sites lie on rings around the point from this far (m) ...
        site_min_m: 1..=1000,
        /// ... up to this far (m) ...
        site_max_m: 1..=1000,
        /// ... on this many rings ...
        site_rings: 1..=8,
        /// ... with this many bearings on each, all around.
        site_bearings: 1..=16,
        /// No site is more than this nearer to the enemy base than the
        /// point (m).
        site_forward_m: 0..=1000 = "d_gun_site_forward_m",
        /// No site within this of a point the enemy holds (m).
        site_enemy_point_m: 0..=1000 = "d_gun_site_enemy_point_m",
        /// Sites that see less than this share of the best site's corridor
        /// samples are dropped (%).
        site_view_min_pct: 0..=100 = "d_gun_site_view_min_pct",
        /// The last stretch of the enemy's route to the point that is
        /// sampled (m) ...
        corridor_m: 1..=2000 = "d_gun_corridor_m",
        /// ... every this far (m).
        corridor_step_m: 1..=200 = "d_gun_corridor_step_m",
        /// No site within this of the corridor (m): a gun on the route is
        /// overrun; it fires across it from the side.
        corridor_clear_m: 0..=1000 = "d_gun_corridor_clear_m",
        /// Seen enemy armoured vehicles whose routes are searched, nearest
        /// the point first.
        corridor_paths: 0..=8 = "d_gun_corridor_paths",
        /// Two guns site at least this far apart (m).
        site_apart_m: 0..=500,
        /// With no route: the straight approach sampled ahead of the point
        /// (m) ...
        approach_m: 1..=1000,
        /// ... at this many points. Also the fewest samples the routes of
        /// seen armour must give before the enemy base's route is added.
        los_rays: 1..=16,
        /// No site within this of known enemy men (m).
        enemy_standoff_m: 0..=500,
        /// A move that ends farther than this from its goal has failed (m).
        arrive_m: 1..=200,
        /// A site with cover counts as reached with the gun this close (m).
        arrive_cover_m: 1..=200,
        /// Failed moves before it unhitches where it stands.
        max_fails: 1..=10,
        /// No job sends the same order again within this (s).
        repath_s: 1..=120,
        /// The tow unhitches at the latest this long after setting off (s).
        go_max_s: 10..=1800,
        /// No unhitching with an enemy this close (m).
        contact_m: 0..=500,
        /// Holds fire until a target is this close (m).
        engage_still_m: 0..=2000,
        /// Re-lays the trail at most this often (s).
        relay_gap_s: 1..=600,
        /// An idle engineer squad this close digs its pit (m; 0: never).
        pit_engineer_m: 0..=1000,
        /// Idle infantry this close serves a gun with no crew (m; 0: never).
        recrew_m: 0..=1000,
        /// One such order per gun within this (s).
        recrew_gap_s: 1..=1800,
        /// A gun with no vehicle in reach this long moves to a held point
        /// with armour (s; 0: never).
        resite_idle_s: 0..=3600,
        /// Most moves per gun.
        resite_max: 0..=20,
    }
}

fn d_gun_site_forward_m() -> u32 {
    0
}
fn d_gun_site_enemy_point_m() -> u32 {
    250
}
fn d_gun_site_view_min_pct() -> u32 {
    10
}
fn d_gun_corridor_m() -> u32 {
    500
}
fn d_gun_corridor_step_m() -> u32 {
    25
}
fn d_gun_corridor_clear_m() -> u32 {
    150
}
fn d_gun_corridor_paths() -> u32 {
    2
}

impl AiGunsTuning {
    fn check_more(&self, at: &str, out: &mut Vec<Issue>) {
        check_order(
            out,
            at,
            ("site_min_m", self.site_min_m),
            ("site_max_m", self.site_max_m),
        );
    }
}

impl Default for AiGunsTuning {
    /// S9 starting values (a rules def without `[ai.guns]`).
    fn default() -> AiGunsTuning {
        AiGunsTuning {
            after_s: 120,
            income_max_pct: 30,
            reserve_mp: 0,
            max_guns: 2,
            site_min_m: 150,
            site_max_m: 250,
            site_rings: 5,
            site_bearings: 16,
            site_forward_m: d_gun_site_forward_m(),
            site_enemy_point_m: d_gun_site_enemy_point_m(),
            site_view_min_pct: d_gun_site_view_min_pct(),
            corridor_m: d_gun_corridor_m(),
            corridor_step_m: d_gun_corridor_step_m(),
            corridor_clear_m: d_gun_corridor_clear_m(),
            corridor_paths: d_gun_corridor_paths(),
            site_apart_m: 60,
            approach_m: 350,
            los_rays: 6,
            enemy_standoff_m: 80,
            arrive_m: 15,
            arrive_cover_m: 5,
            max_fails: 2,
            repath_s: 8,
            go_max_s: 150,
            contact_m: 60,
            engage_still_m: 400,
            relay_gap_s: 25,
            pit_engineer_m: 350,
            recrew_m: 100,
            recrew_gap_s: 60,
            resite_idle_s: 120,
            resite_max: 3,
        }
    }
}

tuning_table! {
    /// `[ai.fort]` (S10a-ai, docs/slices/S10a-ai.md 3.7): how the AI buys
    /// the engineer cards, carries materials, builds works and breaches
    /// wire. The four switches (1 on, 0 off) are the ablations of spec 9;
    /// `nest`, `dugout`, `roof`, `sandbags` split `works` (stage D).
    pub struct AiFortTuning {
        /// Switch: MG nests, dugouts, the HQ roof, sandbags at guns.
        works: 0..=1,
        /// Switch: barbed wire.
        wire: 0..=1,
        /// Switch: wire knowledge, breach jobs, the urgent breach buy.
        breach: 0..=1,
        /// Switch: the three S10a engineer cards are bought.
        cards: 0..=1,
        /// Per-work switches under `works` (stage D ablations; 0 stops
        /// only that kind): MG nests ...
        nest: 0..=1,
        /// ... dugouts (and so sheltering in one) ...
        dugout: 0..=1,
        /// ... the HQ roof ...
        roof: 0..=1,
        /// ... sandbag walls at guns.
        sandbags: 0..=1,
        /// Fort jobs at once.
        builders_max: 0..=16,
        /// Works per point.
        works_per_point: 0..=16,
        /// No building at a point with more threat than this.
        build_threat_max: 0..=100,
        /// A builder this close to a gun or the HQ may work there (m).
        builder_reach_m: 0..=2000,
        /// A builder carrying less than this loads in base (kg).
        load_min_kg: 0..=2000,
        /// Materials cost at most this share of the income earned (%).
        materials_max_pct: 0..=100,
        /// A truck run loads this much (kg).
        truck_material_kg: 0..=10000,
        /// A truck waits at a work at most this long (s).
        material_wait_s: 1..=3600,
        /// An MG nest stands this far past the trench end (m).
        nest_offset_m: 0..=200,
        /// Wire lies this far out from the trench toward the enemy (m) ...
        wire_out_m: 1..=500,
        /// ... this long (m).
        wire_len_m: 2..=200,
        /// A dugout stands this far behind the trench middle (m).
        dugout_back_m: 0..=200,
        /// A gun in Hold this long gets a sandbag wall (s).
        gun_hold_s: 0..=3600,
        /// The wall stands this far in front of the gun (m).
        gun_wall_m: 0..=100,
        /// A point is shelled when this share of its defenders is ducked
        /// or pinned (%) ...
        shelled_pct: 1..=100,
        /// ... with no known enemy soldier this close (m).
        shelled_clear_m: 0..=2000,
        /// Men shelter while the shelled mark is this fresh (s) ...
        shelter_s: 1..=600,
        /// ... and no known enemy is this close (m).
        shelter_clear_m: 0..=2000,
        /// Enemy wire is seen from this close (m) ...
        wire_seen_m: 1..=2000,
        /// ... with at most this many rays per plan.
        wire_rays: 0..=256,
        /// Known wire this close to an attacked point wants a breach (m).
        breach_near_m: 0..=2000,
        /// One breach path search per point within this (s).
        recheck_s: 1..=600,
        /// The breach squad stages this far short of the crossing (m).
        stage_m: 0..=1000,
        /// Smoke lands this share of the way from the crossing to the MG (%).
        smoke_frac_pct: 0..=100,
        /// The carrier goes this long after the suppression or smoke (s).
        suppress_lead_s: 0..=120,
        /// A Bangalore is laid when a known defender is this close to the
        /// crossing, else the wire is cut (m).
        cut_fire_m: 0..=2000,
        /// No charge laid or cut done within this: a retry (s).
        place_wait_s: 1..=600,
        /// Tries before a breach is given up.
        breach_tries: 1..=10,
        /// A breach job ends after this long (s).
        breach_max_s: 1..=3600,
        /// A gap is open with no standing section this close to the
        /// crossing (m).
        gap_clear_m: 0..=50,
        /// Urgent breach buys alive at once.
        breach_cards_max: 0..=8,
        /// B1-4: at most this many own points hold a builder for their
        /// wanted work (fix F15, the assign bonus); 2 (the old cap, set by
        /// `builders_max` 2) is today's behaviour. Jobs still start for
        /// engineers assigned without a hold. Inside `builders_max`.
        hold_max: 0..=16 = "d_fort_hold_max",
        /// B1-5: no fort hold and no new fort job (S10a works, wire and
        /// the S10b point fields, obstacles, bunker, barriers) before this
        /// many seconds of match time (s); 0 = off (today's behaviour). A
        /// running job is never cancelled.
        build_after_s: 0..=3600 = "d_fort_gate_off",
        /// B1-5: no fort hold and no new fort job while the side has fewer
        /// than this many foot squads with a man alive (every own squad
        /// with no vehicle that is not a crew: rifle, MG, AT, mortar,
        /// engineer, other); 0 = off. A running job is never cancelled.
        build_min_squads: 0..=64 = "d_fort_gate_off",
    }
}

fn d_fort_gate_off() -> u32 {
    0
}

fn d_fort_hold_max() -> u32 {
    2
}

impl AiFortTuning {
    fn check_more(&self, _at: &str, _out: &mut Vec<Issue>) {}
}

impl Default for AiFortTuning {
    /// S10a-ai starting values (a rules def without `[ai.fort]`).
    fn default() -> AiFortTuning {
        AiFortTuning {
            works: 1,
            wire: 1,
            breach: 1,
            cards: 1,
            nest: 1,
            dugout: 1,
            roof: 1,
            sandbags: 1,
            builders_max: 2,
            works_per_point: 3,
            build_threat_max: 4,
            builder_reach_m: 200,
            load_min_kg: 40,
            materials_max_pct: 3,
            truck_material_kg: 200,
            material_wait_s: 120,
            nest_offset_m: 6,
            wire_out_m: 35,
            wire_len_m: 60,
            dugout_back_m: 8,
            gun_hold_s: 60,
            gun_wall_m: 4,
            shelled_pct: 50,
            shelled_clear_m: 150,
            shelter_s: 20,
            shelter_clear_m: 100,
            wire_seen_m: 150,
            wire_rays: 16,
            breach_near_m: 80,
            recheck_s: 60,
            stage_m: 60,
            smoke_frac_pct: 50,
            suppress_lead_s: 6,
            cut_fire_m: 100,
            place_wait_s: 30,
            breach_tries: 3,
            breach_max_s: 180,
            gap_clear_m: 2,
            breach_cards_max: 1,
            hold_max: d_fort_hold_max(),
            build_after_s: d_fort_gate_off(),
            build_min_squads: d_fort_gate_off(),
        }
    }
}

tuning_table! {
    /// `[ai.obstacles]` (S10b-ai, docs/slices/S10b-ai.md 2): how the AI lays
    /// mines, builds obstacles and the bunker, sweeps lanes and demolishes
    /// pieces. The six switches (1 on, 0 off) are the ablations of spec 2.
    pub struct AiObstacleTuning {
        /// Switch: AP and AT minefields.
        mines: 0..=1,
        /// Switch (under `mines`): AT minefields.
        at_mines: 0..=1,
        /// Switch: log barriers and hedgehog rows.
        obstacles: 0..=1,
        /// Switch: the bunker.
        bunker: 0..=1,
        /// Switch: sweeping lanes before an attack.
        sweep: 0..=1,
        /// Switch: demolishing obstacles in a vehicle's way.
        demolish: 0..=1,
        /// S10b jobs at once (inside `[ai.fort] builders_max`).
        jobs_max: 1..=16,
        /// S10b materials cost at most this share of the income earned (%).
        materials_max_pct: 0..=100,
        /// An AP field lies this far beyond the point's wire line (m) ...
        ap_beyond_wire_m: 0..=200,
        /// ... or this far out from the trench with no wire (m).
        ap_out_m: 1..=500,
        /// An AT field lies this far out along the vehicle approach (m).
        at_out_m: 1..=1000,
        /// Minefields per point.
        fields_per_point: 0..=16,
        /// AT minefields per player.
        at_fields_max: 0..=64,
        /// A log barrier stands this far out in an own gun's arc (m).
        obst_gun_m: 1..=500,
        /// A hedgehog row stands this far in front of a nest or bunker (m).
        obst_work_m: 1..=500,
        /// Hedgehogs in a row ...
        hedgehogs_row: 1..=16,
        /// ... this far apart (m).
        hedgehog_step_m: 1..=50,
        /// No obstacle this close to an own vehicle route (m).
        own_route_m: 0..=200,
        /// Bunkers per player.
        bunkers_max: 0..=8,
        /// An enemy mine blast hurting own units marks a suspect this long (s).
        suspect_s: 1..=3600,
        /// A suspect or known mine this close to the attack line wants a sweep (m) ...
        sweep_near_m: 0..=500,
        /// ... or a known mine this close to the point (m).
        sweep_point_m: 0..=500,
        /// A free sweeper this close may be sent (m).
        sweep_reach_m: 0..=5000,
        /// Pieces one demolition job blows before it gives up.
        demolish_pieces_max: 1..=3,
        /// A known enemy piece this close to a stuck vehicle wants a demolition (m).
        demolish_near_m: 0..=500,
        /// Enemy obstacles are known this close to an own unit (m).
        obst_seen_m: 1..=2000,
        /// Every S10b job ends after this long (s; L1).
        job_max_s: 1..=36000,
        /// A refused goal is not ordered again within this (s; L4) ...
        refused_s: 0..=3600,
        /// ... within this distance (m).
        refused_m: 0..=500,
        /// One vehicle approach search per point within this (s).
        recheck_s: 1..=600,
        /// S10b-ai Build E switch: a wanted bunker with no fit squad at its
        /// point takes one free `build_heavy` (else `build_structure`)
        /// squad from anywhere.
        bunker_assign: 0..=1 = "d_obst_one",
        /// S10b-ai Build E switch: once per bunker job, the run for its
        /// heavy step may take a truck from tank service.
        bunker_truck_first: 0..=1 = "d_obst_one",
        /// S10b-ai Build E: the L1 deadline of a log barrier job (s; every
        /// other job keeps `job_max_s`).
        barrier_job_max_s: 1..=36000 = "d_obst_barrier_job_max_s",
        /// S10b-ai Build E switch: attackers walk a swept lane in single
        /// file on its centre line until past its end.
        lane_file: 0..=1 = "d_obst_one",
        /// S10b-ai Build E: rows of an AI AT field (the second one step
        /// behind the first, shifted half a step along it).
        at_rows: 1..=2 = "d_obst_at_rows",
        /// B1-0 switch (owner S10b-ai-E3): a demolition job whose squad
        /// has no satchel for its next piece passes to another free own
        /// `demolish` squad with one (nearest the vehicle first).
        demolish_handover: 0..=1 = "d_obst_one",
        /// B1-0 switch (owner S10b-ai-E1): a bunker job whose heavy step
        /// waits for material its squad does not carry gives up the S10b
        /// job slot (`jobs_max`), so one other S10b job may start.
        bunker_yields: 0..=1 = "d_obst_one",
        /// S10c-ai switch (F1-F9, docs/slices/S10c-ai.md 2): hasty works
        /// (AP field, wire, AT field) on the enemy approach lanes of the
        /// front points, with no trench, nest or wire first. Off: 0.
        front: 0..=1 = "d_obst_zero",
        /// S10c-ai switch (C2): a wanted front work with no fit squad at
        /// its point takes one free builder from anywhere. Off: 0.
        mine_assign: 0..=1 = "d_obst_zero",
        /// S10c-ai switch (C4): a blocking belt across the vehicle lane
        /// beyond a front point's AT field. Off: 0.
        belt: 0..=1 = "d_obst_zero",
        /// S10c-ai (F1): the own points nearest the enemy base that are
        /// front points.
        front_points: 0..=8 = "d_obst_front_points",
        /// S10c-ai (F3): the front wire lies this far out along the foot
        /// lane (m; the AP field `ap_beyond_wire_m` beyond it).
        front_out_m: 0..=500 = "d_obst_front_out_m",
        /// S10c-ai (F3): length of a front wire line (m).
        front_wire_len_m: 0..=500 = "d_obst_front_wire_len_m",
        /// S10c-ai (F5): a front job does not start, and is dropped, while
        /// a known enemy is this close to its site (m; the builder keeps
        /// `[ai] threat_radius`).
        front_site_threat_m: 0..=500 = "d_obst_front_site_threat_m",
        /// S10c-ai (F7): front AP fields per player.
        front_fields_max: 0..=16 = "d_obst_front_fields_max",
        /// S10c-ai (C8): front jobs (FrontAp, FrontWire, a front AT field,
        /// a belt) running at once.
        front_slots: 0..=16 = "d_obst_one",
        /// S10c-ai F1 (orchestrator 2026-11-16): 1 = the home point (the
        /// map point nearest the own base) may be a front point; 0 = only
        /// while it is the player's only own point. Off: 0.
        front_home: 0..=1 = "d_obst_zero",
        /// S10c-ai (C4): blocking pieces in a belt ...
        belt_pieces: 0..=20 = "d_obst_belt_pieces",
        /// ... this far beyond the AT field, on the enemy side (m).
        belt_gap_m: 0..=500 = "d_obst_belt_gap_m",
        /// S10c-ai (C5): the own route round a belt may be at most this
        /// share of the route without it (%; 0 = rule off).
        belt_detour_pct: 0..=1000 = "d_obst_belt_detour_pct",
        /// S10c-ai (C3): with fewer living builder squads than this the
        /// next buy is the faction's builder card (0 = off).
        builder_cards_min: 0..=8 = "d_obst_zero",
        /// S10c-ai C2 (fix pass 3, orchestrator 2026-11-16): `mine_assign`
        /// pulls a free builder only within this of the site (m; sweeps
        /// and demolitions keep `sweep_reach_m`).
        mine_assign_reach_m: 0..=2000 = "d_obst_mine_assign_reach_m",
    }
}

fn d_obst_zero() -> u32 {
    0
}

fn d_obst_front_points() -> u32 {
    2
}

fn d_obst_front_out_m() -> u32 {
    50
}

fn d_obst_front_wire_len_m() -> u32 {
    20
}

fn d_obst_front_site_threat_m() -> u32 {
    50
}

fn d_obst_front_fields_max() -> u32 {
    2
}

fn d_obst_belt_pieces() -> u32 {
    5
}

fn d_obst_belt_gap_m() -> u32 {
    10
}

fn d_obst_belt_detour_pct() -> u32 {
    130
}

fn d_obst_mine_assign_reach_m() -> u32 {
    450
}

fn d_obst_one() -> u32 {
    1
}

fn d_obst_barrier_job_max_s() -> u32 {
    1200
}

fn d_obst_at_rows() -> u32 {
    2
}

impl AiObstacleTuning {
    /// `at_mines` lives under `mines`; nothing else ties keys together.
    fn check_more(&self, _at: &str, _out: &mut Vec<Issue>) {}
}

impl Default for AiObstacleTuning {
    /// S10b-ai starting values (a rules def without `[ai.obstacles]`).
    fn default() -> AiObstacleTuning {
        AiObstacleTuning {
            mines: 1,
            at_mines: 1,
            obstacles: 1,
            bunker: 1,
            sweep: 1,
            demolish: 1,
            jobs_max: 1,
            materials_max_pct: 3,
            ap_beyond_wire_m: 5,
            ap_out_m: 40,
            at_out_m: 60,
            fields_per_point: 2,
            at_fields_max: 3,
            obst_gun_m: 40,
            obst_work_m: 20,
            hedgehogs_row: 3,
            hedgehog_step_m: 4,
            own_route_m: 10,
            bunkers_max: 1,
            suspect_s: 600,
            sweep_near_m: 20,
            sweep_point_m: 30,
            sweep_reach_m: 300,
            demolish_pieces_max: 3,
            demolish_near_m: 30,
            obst_seen_m: 150,
            job_max_s: 900,
            refused_s: 300,
            refused_m: 20,
            recheck_s: 60,
            bunker_assign: d_obst_one(),
            bunker_truck_first: d_obst_one(),
            barrier_job_max_s: d_obst_barrier_job_max_s(),
            lane_file: d_obst_one(),
            at_rows: d_obst_at_rows(),
            demolish_handover: d_obst_one(),
            bunker_yields: d_obst_one(),
            front: d_obst_zero(),
            mine_assign: d_obst_zero(),
            belt: d_obst_zero(),
            front_points: d_obst_front_points(),
            front_out_m: d_obst_front_out_m(),
            front_wire_len_m: d_obst_front_wire_len_m(),
            front_site_threat_m: d_obst_front_site_threat_m(),
            front_fields_max: d_obst_front_fields_max(),
            front_slots: d_obst_one(),
            front_home: d_obst_zero(),
            belt_pieces: d_obst_belt_pieces(),
            belt_gap_m: d_obst_belt_gap_m(),
            belt_detour_pct: d_obst_belt_detour_pct(),
            builder_cards_min: d_obst_zero(),
            mine_assign_reach_m: d_obst_mine_assign_reach_m(),
        }
    }
}

tuning_table! {
    /// `[ai.truck]` (S8b, docs/slices/S8b-trucks-recon.md 9): how the AI
    /// uses its supply trucks.
    pub struct AiTruckTuning {
        /// It buys one with this many fighting vehicles alive ...
        min_vehicles: 1..=64,
        /// ... once the HQ stands or this much match time has passed (s).
        hq_wait_s: 0..=3600,
        /// Truck cards cost at most this share of the income so far (%).
        income_max_pct: 0..=100,
        /// Kg of shells it loads (at most the def's `load_kg`).
        load_shells_kg: 0..=10000,
        /// It waits in base this long for a full load (s) ...
        load_wait_s: 1..=3600,
        /// ... and leaves with at least this share of it (%).
        min_load_pct: 0..=100,
        /// It waits this far behind the centre of the armour (m).
        follow_m: 0..=2000,
        /// Sight-line rays per spot tested (0: no line test).
        los_rays: 0..=32,
        /// Under this share of its load it goes back to the base (%).
        return_below_pct: 0..=100,
        /// It runs from any seen enemy this close (m; 0: never).
        flee_m: 0..=2000,
        /// A move that ends farther than this from its goal has failed (m).
        arrive_m: 1..=200,
        /// ... after this many it takes where it stands.
        max_fails: 1..=10,
        /// No job sends the same move again within this (s).
        repath_s: 1..=120,
        /// S10b-ai switch (F8, L5): a truck's give-up is remembered as a
        /// refused goal and not ordered again.
        refusal_memory: 0..=1 = "d_truck_refusal_memory",
    }
}

impl AiTruckTuning {
    fn check_more(&self, _: &str, _: &mut Vec<Issue>) {}
}

impl Default for AiTruckTuning {
    /// S8b starting values (a rules def without `[ai.truck]`).
    fn default() -> AiTruckTuning {
        AiTruckTuning {
            min_vehicles: 2,
            hq_wait_s: 300,
            income_max_pct: 9,
            load_shells_kg: 160,
            load_wait_s: 90,
            min_load_pct: 40,
            follow_m: 150,
            los_rays: 8,
            return_below_pct: 20,
            flee_m: 120,
            arrive_m: 15,
            max_fails: 2,
            repath_s: 8,
            refusal_memory: d_truck_refusal_memory(),
        }
    }
}

fn d_truck_refusal_memory() -> u32 {
    1
}

impl AiTransportTuning {
    fn check_more(&self, _: &str, _: &mut Vec<Issue>) {}
}

impl Default for AiTransportTuning {
    /// The S8a starting values (a rules def without `[ai.transport]`).
    fn default() -> AiTransportTuning {
        AiTransportTuning {
            min_ride_m: d_tr_min_ride_m(),
            wait_s: d_tr_wait_s(),
            dismount_back_m: d_tr_dismount_back_m(),
            dismount_sight_m: d_tr_dismount_sight_m(),
            los_rays: d_tr_los_rays(),
            hit_s: d_tr_hit_s(),
            rider_suppression_pct: d_tr_rider_suppression_pct(),
            stuck_s: d_tr_stuck_s(),
            support_s: d_tr_support_s(),
            arrive_m: d_tr_arrive_m(),
            max_fails: d_tr_max_fails(),
            repath_s: d_tr_repath_s(),
            pick_m: d_tr_pick_m(),
            meet_ahead_m: d_tr_meet_ahead_m(),
            meet_stand_m: d_tr_meet_stand_m(),
            ride_lock_s: d_tr_ride_lock_s(),
            counted_ride_m: d_tr_counted_ride_m(),
            drop_short_m: d_tr_drop_short_m(),
            unload_rolling: d_tr_unload_rolling(),
        }
    }
}

impl Default for AiSupplyTuning {
    /// The S6 starting values (a rules def without `[ai.supply]`).
    fn default() -> AiSupplyTuning {
        AiSupplyTuning {
            hq_behind_m: 120,
            hq_probe_rays: 8,
            crate_run_below_pct: 50,
            tank_crate_below_pct: 25,
            tank_crate_far_m: d_tank_crate_far_m(),
            shell_mp_max_pct: 25,
            hq_after_s: 60,
            dry_walk_s: 120,
            mortars_per_bearers: 0,
        }
    }
}

impl SupplyTuning {
    fn check_more(&self, _: &str, _: &mut Vec<Issue>) {}
}

tuning_table! {
    /// `[combat.share]` (S5a): sharing rounds inside a squad and feeding a
    /// crew weapon from a squadmate.
    pub struct ShareTuning {
        /// One sharing pass per squad this often (s).
        interval_s: 1..=60,
        /// Giver and taker this close (m).
        reach_m: 1..=100,
        /// A man below this share of his own load takes ... (%)
        below_pct: 0..=100,
        /// ... from the squadmate (same round) above this share, ammo
        /// bearers first (%) ...
        giver_above_pct: 0..=100,
        /// ... this share of the giver's load per pass (%).
        share_pct: 1..=100,
        /// A dry crew weapon reloads from a squadmate this close (assistant
        /// and bearers first) (m).
        crew_feed_m: 1..=50,
    }
}

impl ShareTuning {
    fn check_more(&self, _: &str, _: &mut Vec<Issue>) {}
}

impl Default for ShareTuning {
    /// The S5a starting values (a rules def without `[combat.share]`).
    fn default() -> ShareTuning {
        ShareTuning {
            interval_s: 5,
            reach_m: 10,
            below_pct: 25,
            giver_above_pct: 50,
            share_pct: 20,
            crew_feed_m: 5,
        }
    }
}

tuning_table! {
    /// `[combat.fire]` (S5a): fire discipline (infantry-structures.md
    /// §6.2, S5a-4, S5a-18).
    pub struct FireTuning {
        /// In `discipline` mode a man opens fire only at targets within
        /// this share of his weapon's range (%).
        hold_beyond_pct: 10..=100,
        /// A man in `hold` mode returns fire at enemies this close (m).
        hold_open_m: 0..=200,
        /// A squad holding fire springs to `discipline` once one of its men
        /// fired, or once one is suppressed to this many points (0: never
        /// on suppression alone; absent: 30, the S5a Part B rule at the
        /// ww2 `crouch_suppression`). S5a Fix (review H1).
        hold_spring_points: 0..=1000 = "default_hold_spring_points",
    }
}

fn default_hold_spring_points() -> u32 {
    30
}

impl FireTuning {
    fn check_more(&self, _: &str, _: &mut Vec<Issue>) {}
}

impl Default for FireTuning {
    /// A rules def without `[combat.fire]` (legacy content, S5a-29 (A)):
    /// discipline fires to full range, as before S5a, so older mods and the
    /// frozen fixtures keep their behaviour. `mods/ww2` sets 70.
    fn default() -> FireTuning {
        FireTuning {
            hold_beyond_pct: 100,
            hold_open_m: 30,
            hold_spring_points: default_hold_spring_points(),
        }
    }
}

tuning_table! {
    /// `[ai.ammo]` (S5a): the AI's ammunition reserve and refill trips.
    pub struct AiAmmoTuning {
        /// A squad below this share of its main feed with no known enemy
        /// within `threat_radius_m` goes to the base to refill (%) ...
        return_below_pct: 0..=100,
        /// ... below this it goes even under threat (%).
        dry_pct: 0..=100,
        /// At most this share of the squads at one point leave to refill
        /// in one plan (%; at least one).
        max_returning_pct: 1..=100,
        /// A refilling squad leaves the base at this share of its load
        /// (%) ...
        refill_full_pct: 1..=100,
        /// ... or after this long in the base (s).
        refill_max_s: 1..=900,
        /// A mortar barrages only above this share of its bombs, unless an
        /// own point is under threat (%).
        mortar_reserve_pct: 0..=100,
        /// Manpower kept back from unit buys while an own unit waits in the
        /// base for a paid refill (MP).
        float_mp: 0..=10000,
        /// A unit leaves the base after this long with its refill paused
        /// for lack of manpower (s).
        rearm_stall_s: 1..=600,
    }
}

impl AiAmmoTuning {
    fn check_more(&self, table: &str, out: &mut Vec<Issue>) {
        check_order(
            out,
            table,
            ("dry_pct", self.dry_pct),
            ("return_below_pct", self.return_below_pct),
        );
        check_order(
            out,
            table,
            ("return_below_pct", self.return_below_pct),
            ("refill_full_pct", self.refill_full_pct),
        );
    }
}

impl Default for AiAmmoTuning {
    /// The S5a starting values (a rules def without `[ai.ammo]`).
    fn default() -> AiAmmoTuning {
        AiAmmoTuning {
            return_below_pct: 30,
            dry_pct: 5,
            max_returning_pct: 34,
            refill_full_pct: 90,
            refill_max_s: 120,
            mortar_reserve_pct: 30,
            float_mp: 100,
            rearm_stall_s: 15,
        }
    }
}

tuning_table! {
    /// `[fire_mission]` (S5b): mortar fire missions, observers and the
    /// voice link, aim error and ranging-in, the player's knowledge map
    /// (artillery-on-map.md §4.1, §4.8; docs/modding/fire-missions.md). A
    /// rules def without this table runs every mission as the old barrage:
    /// no observer rule, no aim error, no knowledge rule (S5b-1 (A)).
    pub struct FireMissionTuning {
        /// A squad leader adjusts fire on ground points this far (m).
        observe_leader_m: 0..=5000,
        /// Men with the `observer` trait (FO teams) observe this far (m).
        observe_observer_m: 0..=10000,
        /// Voice link: an observer within this of the mortar's gunner can
        /// pass corrections (m).
        observer_link_m: 1..=1000,
        /// A sight line to a ground point ends this high above it (mm) ...
        point_lift_mm: 0..=10000,
        /// ... and to a burst this high above the burst point (mm).
        burst_seen_lift_mm: 0..=20000,
        /// Order to first round, observed or map fire (s) ...
        fire_direction_s: 0..=600,
        /// ... and direct (the crew sees the point) (s).
        direct_s: 0..=600,
        /// After an adjusting round bursts, the wait before the next (s).
        adjust_wait_s: 0..=120,
        /// Adjust until the aim error is at most this (m) ...
        ffe_error_m: 0..=500,
        /// ... or this many adjusting rounds were fired.
        adjust_rounds_max: 1..=20,
        /// First-round aim error, observed and direct (% of range) ...
        first_round_error_pct: 0..=100,
        /// ... at least this (m).
        first_round_error_min_m: 0..=500,
        /// Each round an observer sees land leaves this share of the aim
        /// error (%).
        ranging_step_pct: 0..=100,
        /// Map fire aim error (% of range) ...
        map_fire_error_pct: 0..=100,
        /// ... at least this (m).
        map_fire_error_min_m: 0..=1000,
        /// A new mission within this of a ranged target reuses its error (m).
        ranging_cell_m: 1..=1000,
        /// Ranging is forgotten after this (s) ...
        ranging_expiry_s: 1..=7200,
        /// ... or when the tube moves farther than this from where it
        /// ranged (m).
        ranging_keep_move_m: 0..=1000,
        /// Most remembered target areas per mortar squad.
        ranging_max_cells: 1..=16,
        /// Cell size of the player's knowledge map (m).
        intel_cell_m: 5..=500,
        /// A cell where an enemy was last seen stays known this long (s).
        intel_keep_s: 1..=7200,
        /// A cell where an enemy was seen in a trench stays known this long
        /// (s; 0: the whole match).
        intel_trench_keep_s: 0..=86400,
        /// Knowledge cells are updated this often (s).
        intel_period_s: 1..=60,
        /// Map fire is allowed within this of a known cell's centre, a seen
        /// enemy or a capture point's edge (m).
        intel_snap_m: 0..=500,
        /// Rounds in a mission ordered from the HUD tool.
        player_rounds: 1..=50,
        /// S11a1 G1 switch: a served gun whose main weapon has `[indirect]`
        /// takes fire missions (0: refused `not_mortar`, as before S11a1).
        gun_missions: 0..=1 = "d_fm_one",
        /// S11a1 G5 switch: an observer is also linked by radio (a living
        /// `radio` man of his squad near him and one of the firing unit
        /// near the tube; 0: the voice link only).
        radio_link: 0..=1 = "d_fm_one",
        /// S11a1 G2 switch: a gun's solution must clear the ground (0: no
        /// ground test of the arc).
        mask: 0..=1 = "d_fm_one",
        /// S11a1 G3: a gun mission's order to first round, observed or map
        /// fire (s; direct fire keeps `direct_s`).
        gun_fire_direction_s: 0..=3600 = "d_fm_gun_fire_direction_s",
        /// S11a1 G3: a gun mission's adjusting rounds at most.
        gun_adjust_rounds_max: 1..=50 = "d_fm_one",
        /// S11a1 G4: a gun mission's first-round aim error, observed and
        /// direct (% of range) ...
        gun_first_round_error_pct: 0..=50 = "d_fm_gun_first_round_error_pct",
        /// ... at least this (m).
        gun_first_round_error_min_m: 0..=2000 = "d_fm_gun_first_round_error_min_m",
        /// S11a1 G4: a gun's map fire aim error (% of range) ...
        gun_map_fire_error_pct: 0..=50 = "d_fm_gun_map_fire_error_pct",
        /// ... at least this (m).
        gun_map_fire_error_min_m: 0..=2000 = "d_fm_gun_map_fire_error_min_m",
        /// S11a1 G6: a seen enemy this close to the gun ends its mission
        /// (crew self-defence) (m).
        gun_break_m: 0..=2000 = "d_fm_gun_break_m",
        /// S11a2 F1 switch: bursts of mortar and gun fire-mission rounds
        /// hurt the firing player's own men (blast, suppression, fragments;
        /// 0: own men are spared, as before S11a2).
        friendly_fire: 0..=1 = "d_fm_zero",
        /// S11a2 K1 switch: infantry-gun mission rounds leave sound evidence
        /// with the enemy, who marks the gun (0: no evidence, no marker).
        counter_battery: 0..=1 = "d_fm_zero",
        /// S11a2 K2: rounds heard from one spot before it is marked.
        cb_rounds_to_mark: 1..=50 = "d_fm_cb_rounds_to_mark",
        /// S11a2 K3: a spot loses one piece of evidence this often; at none
        /// it is forgotten with its marker (s).
        cb_decay_s: 1..=3600 = "d_fm_cb_decay_s",
        /// S11a2 K2: the marker is made this long after the marking round (s).
        cb_delay_s: 1..=3600 = "d_fm_cb_delay_s",
        /// S11a2 K2: the first marker's error radius (% of the listener's
        /// distance to the gun) ...
        cb_error_pct: 1..=100 = "d_fm_cb_error_pct",
        /// ... each further round heard leaves this share of it (%) ...
        cb_error_step_pct: 1..=100 = "d_fm_cb_error_step_pct",
        /// ... down to this share of the listener's distance (%).
        cb_error_floor_pct: 1..=100 = "d_fm_cb_error_floor_pct",
        /// S11a2 K1: a round fired within this of a spot (or within its
        /// marker's error, if larger) counts for that spot (m).
        cb_reset_m: 0..=2000 = "d_fm_cb_reset_m",
        /// S11a2 K4 (a report): a counter-fire burst this close to an enemy
        /// infantry gun counts as near (m).
        cb_near_m: 0..=2000 = "d_fm_cb_near_m",
    }
}

fn d_fm_one() -> u32 {
    1
}
fn d_fm_gun_fire_direction_s() -> u32 {
    20
}
fn d_fm_gun_first_round_error_pct() -> u32 {
    3
}
fn d_fm_gun_first_round_error_min_m() -> u32 {
    25
}
fn d_fm_gun_map_fire_error_pct() -> u32 {
    6
}
fn d_fm_gun_map_fire_error_min_m() -> u32 {
    50
}
fn d_fm_gun_break_m() -> u32 {
    150
}
fn d_fm_zero() -> u32 {
    0
}
fn d_fm_cb_rounds_to_mark() -> u32 {
    3
}
fn d_fm_cb_decay_s() -> u32 {
    300
}
fn d_fm_cb_delay_s() -> u32 {
    15
}
fn d_fm_cb_error_pct() -> u32 {
    15
}
fn d_fm_cb_error_step_pct() -> u32 {
    80
}
fn d_fm_cb_error_floor_pct() -> u32 {
    5
}
fn d_fm_cb_reset_m() -> u32 {
    75
}
fn d_fm_cb_near_m() -> u32 {
    20
}

impl FireMissionTuning {
    fn check_more(&self, table: &str, out: &mut Vec<Issue>) {
        check_order(
            out,
            table,
            ("first_round_error_min_m", self.first_round_error_min_m),
            ("map_fire_error_min_m", self.map_fire_error_min_m),
        );
        check_order(
            out,
            table,
            (
                "gun_first_round_error_min_m",
                self.gun_first_round_error_min_m,
            ),
            ("gun_map_fire_error_min_m", self.gun_map_fire_error_min_m),
        );
        check_order(
            out,
            table,
            ("cb_error_floor_pct", self.cb_error_floor_pct),
            ("cb_error_pct", self.cb_error_pct),
        );
    }
}

tuning_table! {
    /// `[ai.mortar]` (S5b): the AI's fire missions, spotters and map fire
    /// (`crates/sim/src/ai/mortar.rs`, docs/modding/fire-missions.md "The
    /// AI").
    pub struct AiMortarTuning {
        /// Rounds per AI mission (was `ai.barrage_rounds`).
        mission_rounds: 1..=50,
        /// Most sight-line rays a mortar squad casts per plan looking for
        /// a spotter spot (16 spots on up to 3 targets; 0: no spotters).
        spot_samples: 0..=32,
        /// A spotter who has seen nothing this long after reaching his spot
        /// comes back (s; three times this if he never gets there); a
        /// recalled spotter blocks a new one this long.
        spot_max_s: 1..=600,
        /// A spotter with no new target stays this long before rejoining (s).
        spot_hold_s: 0..=600,
        /// Map fire only while the mortar holds more than this share of its
        /// bombs (%) ...
        map_fire_min_pct: 0..=100,
        /// ... and only at a target scoring at least this (still men·10 +
        /// moving men·5 + men in trenches·20; S5b Part B).
        map_fire_min_score: 0..=1000,
        /// S6b: map fire only where the sim's map start error at that range
        /// (`[fire_mission] map_fire_error_pct` / `_min_m`) is at most this
        /// (m; 1000, the default: no limit).
        map_fire_max_miss_m: 0..=1000 = "default_map_fire_max_miss_m",
    }
}

fn default_map_fire_max_miss_m() -> u32 {
    1000
}

impl AiMortarTuning {
    fn check_more(&self, _: &str, _: &mut Vec<Issue>) {}

    /// A rules def without `[ai.mortar]`: `ai.barrage_rounds` rounds per
    /// mission and the S5b starting values.
    pub fn legacy(rounds: u32) -> AiMortarTuning {
        AiMortarTuning {
            mission_rounds: rounds,
            spot_samples: 8,
            spot_max_s: 60,
            spot_hold_s: 30,
            map_fire_min_pct: 60,
            map_fire_min_score: 40,
            map_fire_max_miss_m: 1000,
        }
    }
}

tuning_table! {
    /// S10a `[fortify.tiers.*]`: a tier's work rates (% of the field rate).
    pub struct TierTuning {
        /// % build rate of a builder of this tier.
        build_pct: 0..=1000,
        /// % wire-cutting rate of a cutter of this tier.
        cut_pct: 0..=1000,
    }
}

impl TierTuning {
    fn check_more(&self, _: &str, _: &mut Vec<Issue>) {}
}

/// S10a `[fortify.tiers]`: field, assault and construction engineers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FortTiers {
    pub field: TierTuning,
    pub assault: TierTuning,
    pub construction: TierTuning,
}

/// S10a `[fortify]`: fortifications, wire, materials and breaching
/// (docs/slices/S10a-fortifications.md 3.2). A plain struct, not a
/// `tuning_table!`, because it holds `[fortify.tiers]`; its flat keys are
/// in [`FortTuning::KEYS`] (and [`TABLES`]) all the same.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FortTuning {
    /// % of a man's speed inside a standing wire band.
    pub wire_speed_pct: u32,
    /// A* cost of a wire node for infantry (%).
    pub wire_path_cost_pct: u32,
    /// % of `frag_damage` a fragment crossing a wire section deals to it.
    pub wire_frag_pct: u32,
    /// s per wire section at 100 %.
    pub cut_s: u32,
    /// m: `CutWire` takes the nearest standing section within this.
    pub cut_reach_m: u32,
    /// m: builders draw from an own construction crate or truck this close.
    pub material_reach_m: u32,
    /// Construction kg one man carries.
    pub carry_kg_per_man: u32,
    /// MP per t of construction material at base.
    pub construction_mp_per_t: u32,
    /// hp a non-bursting main-gun round takes from a piece.
    pub shot_piece_damage: u32,
    /// S10b: % of their rate at which `build_structure` squads build `build_heavy` blueprints (R21).
    #[serde(default = "fort_default_heavy_by_field_pct")]
    pub heavy_by_field_pct: u32,
    /// S10b: cm: a sweeper finds a mine this close (R8).
    #[serde(default = "fort_default_detect_cm")]
    pub detect_cm: u32,
    /// S10b: % chance per second a sweeper finds a mine within `detect_cm` (R8).
    #[serde(default = "fort_default_detect_pct_per_s")]
    pub detect_pct_per_s: u32,
    /// S10b: % of walking speed while sweeping (R9).
    #[serde(default = "fort_default_sweep_speed_pct")]
    pub sweep_speed_pct: u32,
    /// S10b: m: the longest `ClearMines` lane (R9).
    #[serde(default = "fort_default_lane_max_m")]
    pub lane_max_m: u32,
    /// S10b: cm: half the width of a swept lane (R9).
    #[serde(default = "fort_default_lane_half_cm")]
    pub lane_half_cm: u32,
    /// S10b: s to lift one found mine at 100 % (R11).
    #[serde(default = "fort_default_lift_s")]
    pub lift_s: u32,
    /// S10b: s to place a demolition charge (R19).
    #[serde(default = "fort_default_demolish_place_s")]
    pub demolish_place_s: u32,
    /// S10b: s from placing to the burst (R19).
    #[serde(default = "fort_default_demolish_fuse_s")]
    pub demolish_fuse_s: u32,
    /// S10b: % of the charge's damage the demolished piece takes (R20).
    #[serde(default = "fort_default_demolish_pct")]
    pub demolish_pct: u32,
    /// S10b: m: `Demolish` takes the nearest piece within this of `at` (R19).
    #[serde(default = "fort_default_demolish_reach_m")]
    pub demolish_reach_m: u32,
    /// S10b: m: no minefield, obstacle or bunker site this close to a vehicle hull (R18).
    #[serde(default = "fort_default_vehicle_clear_m")]
    pub vehicle_clear_m: u32,
    /// S10b-ai switch: paths price steps near enemy mines the player knows (R11).
    #[serde(default = "fort_default_mine_paths")]
    pub mine_paths: u32,
    /// S10b-ai: A* cost of a priced step (%; R11).
    #[serde(default = "fort_default_mine_path_cost_pct")]
    pub mine_path_cost_pct: u32,
    /// S10b-ai: cm: a foot step this close to a known enemy AP mine is priced (R11).
    #[serde(default = "fort_default_mine_band_foot_cm")]
    pub mine_band_foot_cm: u32,
    /// S10b-ai: cm: a vehicle step this close to a known enemy AT mine is priced (R11).
    #[serde(default = "fort_default_mine_band_vehicle_cm")]
    pub mine_band_vehicle_cm: u32,
    pub tiers: FortTiers,
}

fn fort_default_heavy_by_field_pct() -> u32 {
    50
}
fn fort_default_detect_cm() -> u32 {
    150
}
fn fort_default_detect_pct_per_s() -> u32 {
    50
}
fn fort_default_sweep_speed_pct() -> u32 {
    25
}
fn fort_default_lane_max_m() -> u32 {
    50
}
fn fort_default_lane_half_cm() -> u32 {
    200
}
fn fort_default_lift_s() -> u32 {
    20
}
fn fort_default_demolish_place_s() -> u32 {
    10
}
fn fort_default_demolish_fuse_s() -> u32 {
    10
}
fn fort_default_demolish_pct() -> u32 {
    300
}
fn fort_default_demolish_reach_m() -> u32 {
    25
}
fn fort_default_vehicle_clear_m() -> u32 {
    6
}
fn fort_default_mine_paths() -> u32 {
    1
}
fn fort_default_mine_path_cost_pct() -> u32 {
    2000
}
fn fort_default_mine_band_foot_cm() -> u32 {
    300
}
fn fort_default_mine_band_vehicle_cm() -> u32 {
    400
}

impl FortTuning {
    /// The flat keys with their ranges.
    pub const KEYS: &'static [TuningKey] = &[
        ("wire_speed_pct", 1, 100),
        ("wire_path_cost_pct", 100, 10_000),
        ("wire_frag_pct", 0, 100),
        ("cut_s", 1, 600),
        ("cut_reach_m", 1, 200),
        ("material_reach_m", 1, 100),
        ("carry_kg_per_man", 0, 100),
        ("construction_mp_per_t", 0, 100_000),
        ("shot_piece_damage", 0, 10_000),
        ("heavy_by_field_pct", 1, 100),
        ("detect_cm", 10, 1_000),
        ("detect_pct_per_s", 1, 100),
        ("sweep_speed_pct", 1, 100),
        ("lane_max_m", 1, 200),
        ("lane_half_cm", 50, 1_000),
        ("lift_s", 1, 600),
        ("demolish_place_s", 1, 600),
        ("demolish_fuse_s", 1, 120),
        ("demolish_pct", 1, 1_000),
        ("demolish_reach_m", 1, 200),
        ("vehicle_clear_m", 0, 100),
        ("mine_paths", 0, 1),
        ("mine_path_cost_pct", 100, 10_000),
        ("mine_band_foot_cm", 0, 1_000),
        ("mine_band_vehicle_cm", 0, 1_000),
    ];

    fn values(&self) -> [u32; 25] {
        [
            self.wire_speed_pct,
            self.wire_path_cost_pct,
            self.wire_frag_pct,
            self.cut_s,
            self.cut_reach_m,
            self.material_reach_m,
            self.carry_kg_per_man,
            self.construction_mp_per_t,
            self.shot_piece_damage,
            self.heavy_by_field_pct,
            self.detect_cm,
            self.detect_pct_per_s,
            self.sweep_speed_pct,
            self.lane_max_m,
            self.lane_half_cm,
            self.lift_s,
            self.demolish_place_s,
            self.demolish_fuse_s,
            self.demolish_pct,
            self.demolish_reach_m,
            self.vehicle_clear_m,
            self.mine_paths,
            self.mine_path_cost_pct,
            self.mine_band_foot_cm,
            self.mine_band_vehicle_cm,
        ]
    }

    /// Range checks (`table` is `fortify`).
    pub fn check(&self, table: &str, out: &mut Vec<Issue>) {
        for ((k, lo, hi), v) in Self::KEYS.iter().zip(self.values()) {
            check_range(out, table, k, v, *lo, *hi);
        }
        self.tiers.field.check(&format!("{table}.tiers.field"), out);
        self.tiers
            .assault
            .check(&format!("{table}.tiers.assault"), out);
        self.tiers
            .construction
            .check(&format!("{table}.tiers.construction"), out);
    }
}

tuning_table! {
    /// `[ai.arty]` (S11a1, docs/slices/S11a1.md 2, rules A1-A6, P1-P3):
    /// how the AI buys and fires its infantry gun, places its observer
    /// team and fires preparation for a breach. The three switches are 0
    /// until Build C (absent: these values).
    pub struct AiArtyTuning {
        /// A1 switch: buy an infantry gun.
        buy: 0..=1,
        /// A3 switch: buy and post an observer team.
        fo: 0..=1,
        /// P1-P3 switch: preparation fire for the breach job.
        prep: 0..=1,
        /// A1: no gun before this match time (s).
        after_s: 0..=3600,
        /// A1: infantry-gun cards cost at most this share of the income so
        /// far (%).
        income_max_pct: 0..=100,
        /// A1: manpower that stays after a gun is bought.
        reserve_mp: 0..=10000,
        /// A1: infantry guns alive or called in at once.
        max_guns: 0..=8,
        /// A3: observer teams alive at once ...
        fo_max: 0..=8,
        /// ... and the wait after one is lost before the next (s).
        fo_rebuy_s: 0..=3600,
        /// A4: rounds of one AI gun mission.
        mission_rounds: 1..=50,
        /// A4, A5: no new mission at or under this share of the gun's HE
        /// load (%).
        reserve_pct: 0..=100,
        /// A5: gun shells cost at most this share of the income so far (%).
        shell_mp_max_pct: 0..=100,
        /// A4, A6: own men this close to a target: no mission, or lift (m).
        danger_close_m: 0..=2000,
        /// A3: the first post candidate this far short of the point (m) ...
        post_m: 0..=2000,
        /// ... and at this sideways offset either side (m).
        post_side_m: 0..=2000,
        /// A3: sight rays a plan's post search may cast.
        post_rays: 0..=48,
        /// A3: a candidate with a known enemy this close is passed over (m).
        post_clear_m: 0..=2000,
        /// A3: after a failed search, the next one waits this long (s).
        post_retry_s: 0..=3600,
        /// P2: a mission within this of the breach focus counts as its
        /// preparation (m) ...
        prep_near_m: 0..=2000,
        /// ... once it has fired this many rounds ...
        prep_rounds_min: 1..=50,
        /// ... and the breach job waits for it at most this long (s).
        prep_wait_s: 0..=3600,
        /// S11a2 S1-S3 switch: one own-men predicate for the order filter
        /// and the lift of every own mission (0: the S11a1 behaviour).
        ff_safe: 0..=1 = "d_arty_zero",
        /// S11a2 R1-R3 switch: an AI gun answers an own counter-battery
        /// marker with map fire.
        counter_fire: 0..=1 = "d_arty_zero",
        /// S11a2 S1 (ii): an own squad walking to within `danger_close_m`
        /// of a target counts once its lead is this close to it (m).
        lift_m: 0..=2000 = "d_arty_lift_m",
        /// S11a2 R2: no counter-fire on a marker wider than this (m).
        cb_max_error_m: 0..=2000 = "d_arty_cb_max_error_m",
        /// S11b X3 switch: an AI gun does not order counter-fire twice in
        /// a row.
        cb_alternate: 0..=1 = "d_arty_zero",
        /// S11b H1 switch: gun buys count the manpower of call-ins still
        /// pending.
        buy_pending: 0..=1 = "d_arty_zero",
        /// S11b H1 switch: the AI buys a field howitzer.
        how: 0..=1 = "d_arty_zero",
        /// S11b H1: no howitzer before this match time (s) ...
        how_after_s: 0..=3600 = "d_arty_how_after_s",
        /// ... at most this many alive or called in ...
        how_max: 0..=8 = "d_arty_how_max",
        /// ... howitzer cards cost at most this share of the income so
        /// far (%), apart from the infantry gun's `income_max_pct` ...
        how_income_pct: 0..=100 = "d_arty_how_income_pct",
        /// ... and this much manpower stays after the buy.
        how_reserve_mp: 0..=100000 = "d_arty_how_reserve_mp",
        /// S11b L1 switch: cards tagged `light_mortar` join the mortar slot
        /// of the buy mix.
        light: 0..=1 = "d_arty_zero",
        /// B3 switch: an AI gun's direct (open-sight) fire also stops at
        /// the A5 shell budget: over it, the gun holds fire beyond
        /// `[vehicles.guns] self_defence_m` (struck, or a known enemy
        /// vehicle within `[ai.guns] engage_still_m`: no hold).
        direct_in_budget: 0..=1 = "d_arty_zero",
    }
}

fn d_arty_zero() -> u32 {
    0
}
fn d_arty_lift_m() -> u32 {
    100
}
fn d_arty_cb_max_error_m() -> u32 {
    60
}
fn d_arty_how_after_s() -> u32 {
    480
}
fn d_arty_how_max() -> u32 {
    1
}
fn d_arty_how_income_pct() -> u32 {
    15
}
fn d_arty_how_reserve_mp() -> u32 {
    300
}

impl AiArtyTuning {
    fn check_more(&self, _at: &str, _out: &mut Vec<Issue>) {}
}

impl Default for AiArtyTuning {
    /// S11a1 starting values (a rules def without `[ai.arty]`): every
    /// switch off.
    fn default() -> AiArtyTuning {
        AiArtyTuning {
            buy: 0,
            fo: 0,
            prep: 0,
            after_s: 240,
            income_max_pct: 15,
            reserve_mp: 0,
            max_guns: 1,
            fo_max: 1,
            fo_rebuy_s: 180,
            mission_rounds: 4,
            reserve_pct: 30,
            shell_mp_max_pct: 25,
            danger_close_m: 50,
            post_m: 300,
            post_side_m: 60,
            post_rays: 8,
            post_clear_m: 150,
            post_retry_s: 60,
            prep_near_m: 40,
            prep_rounds_min: 2,
            prep_wait_s: 60,
            ff_safe: 0,
            counter_fire: 0,
            lift_m: 100,
            cb_max_error_m: 60,
            cb_alternate: 0,
            buy_pending: 0,
            how: 0,
            how_after_s: 480,
            how_max: 1,
            how_income_pct: 15,
            how_reserve_mp: 300,
            light: 0,
            direct_in_budget: 0,
        }
    }
}

tuning_table! {
    /// `[wounds]` (S12, docs/slices/S12-wounds-medics.md 2, rules W1-W11):
    /// hit zones, wound states, bleeding, bandages, medics, aid posts and
    /// reinforcements. The switches `enabled` and `reinforce` are 0 in S12a
    /// (absent: these values).
    pub struct WoundsTuning {
        /// W1-W9 switch: wound states (0: a man at 0 HP dies, as before S12).
        enabled: 0..=1 = "d_w_zero",
        /// W1: the top share of a standing or crouched capsule is the head
        /// (%) ...
        head_top_pct: 1..=100 = "d_w_head_top_pct",
        /// ... and the bottom share the legs (%).
        legs_bottom_pct: 1..=100 = "d_w_legs_bottom_pct",
        /// W2: a head hit does this share of the weapon's damage (%) ...
        head_damage_pct: 1..=400 = "d_w_hundred",
        /// ... a torso hit this share ...
        torso_damage_pct: 1..=400 = "d_w_hundred",
        /// ... and a leg hit this share (%).
        legs_damage_pct: 1..=400 = "d_w_legs_damage_pct",
        /// W3: a hit that leaves at most this many HP wounds (HP).
        wounded_hp: 1..=1000 = "d_w_wounded_hp",
        /// W3: a hit that leaves this many HP or more below 0 kills
        /// outright (HP).
        overkill_kill: 0..=1000 = "d_w_overkill_kill",
        /// W3: a head hit of at least this damage kills outright (0: off).
        head_kill_dmg: 0..=1000 = "d_w_zero",
        /// W4: an unbandaged wounded man loses 1 HP this often (ms).
        bleed_ms_per_hp: 1..=60000 = "d_w_bleed_ms_per_hp",
        /// W5: a downed man bleeds out after a seeded time from (s) ...
        down_bleed_min_s: 1..=3600 = "d_w_down_bleed_min_s",
        /// ... to (s, inclusive).
        down_bleed_max_s: 1..=3600 = "d_w_down_bleed_max_s",
        /// W4: a wounded man's spread (% of his normal spread) ...
        wounded_spread_pct: 1..=400 = "d_w_wounded_spread_pct",
        /// ... and his reload and aim time (%).
        wounded_reload_pct: 1..=400 = "d_w_wounded_reload_pct",
        /// W4: a wounded man walks at this share of his speed (%) ...
        wounded_speed_pct: 1..=400 = "d_w_wounded_speed_pct",
        /// ... this share with a leg wound (%).
        legs_speed_pct: 1..=400 = "d_w_legs_speed_pct",
        /// W4: a field dressing takes this long (s).
        bandage_s: 1..=3600 = "d_w_bandage_s",
        /// W6: a medic stabilises a downed man in this long (s).
        stabilise_s: 1..=3600 = "d_w_stabilise_s",
        /// W7: one man drags a casualty at this share of his speed (%) ...
        drag_speed_pct: 1..=400 = "d_w_drag_speed_pct",
        /// ... two medics with a stretcher at this share (%).
        stretcher_speed_pct: 1..=400 = "d_w_stretcher_speed_pct",
        /// W8: an aid post treats a wounded man in this long (s) ...
        treat_s: 1..=3600 = "d_w_treat_s",
        /// ... and he returns with this share of his HP (%).
        return_hp_pct: 1..=100 = "d_w_return_hp_pct",
        /// W9: an evacuation takes this long (s) ...
        evac_s: 1..=3600 = "d_w_evac_s",
        /// ... and refunds this share of a paid man's price (%).
        evac_refund_pct: 1..=400 = "d_w_evac_refund_pct",
        /// W6: a medic with no move order tends own downed men this close
        /// (m).
        auto_tend_m: 0..=2000 = "d_w_auto_tend_m",
        /// W11 switch: `Reinforce` fills an infantry squad from base.
        reinforce: 0..=1 = "d_w_zero",
        /// W11: a replacement costs this share of the card's per-man
        /// price (%).
        reinforce_cost_pct: 1..=400 = "d_w_hundred",
    }
}

fn d_w_zero() -> u32 {
    0
}
fn d_w_hundred() -> u32 {
    100
}
fn d_w_head_top_pct() -> u32 {
    12
}
fn d_w_legs_bottom_pct() -> u32 {
    45
}
fn d_w_legs_damage_pct() -> u32 {
    60
}
fn d_w_wounded_hp() -> u32 {
    50
}
fn d_w_overkill_kill() -> u32 {
    60
}
fn d_w_bleed_ms_per_hp() -> u32 {
    4000
}
fn d_w_down_bleed_min_s() -> u32 {
    60
}
fn d_w_down_bleed_max_s() -> u32 {
    180
}
fn d_w_wounded_spread_pct() -> u32 {
    130
}
fn d_w_wounded_reload_pct() -> u32 {
    125
}
fn d_w_wounded_speed_pct() -> u32 {
    85
}
fn d_w_legs_speed_pct() -> u32 {
    60
}
fn d_w_bandage_s() -> u32 {
    10
}
fn d_w_stabilise_s() -> u32 {
    15
}
fn d_w_drag_speed_pct() -> u32 {
    30
}
fn d_w_stretcher_speed_pct() -> u32 {
    60
}
fn d_w_treat_s() -> u32 {
    60
}
fn d_w_return_hp_pct() -> u32 {
    75
}
fn d_w_evac_s() -> u32 {
    30
}
fn d_w_evac_refund_pct() -> u32 {
    40
}
fn d_w_auto_tend_m() -> u32 {
    25
}

impl WoundsTuning {
    fn check_more(&self, table: &str, out: &mut Vec<Issue>) {
        check_order(
            out,
            table,
            ("down_bleed_min_s", self.down_bleed_min_s),
            ("down_bleed_max_s", self.down_bleed_max_s),
        );
        // The head band and the leg band must not overlap.
        check_order(
            out,
            table,
            ("head_top_pct", self.head_top_pct),
            ("100 - legs_bottom_pct", 100 - self.legs_bottom_pct.min(100)),
        );
        // W9: an evacuation pays back no more than a replacement costs (no
        // manpower gain loop).
        check_order(
            out,
            table,
            ("evac_refund_pct", self.evac_refund_pct),
            ("reinforce_cost_pct", self.reinforce_cost_pct),
        );
    }
}

impl Default for WoundsTuning {
    /// The S12a values (a rules def without `[wounds]`): every switch off.
    fn default() -> WoundsTuning {
        WoundsTuning {
            enabled: 0,
            head_top_pct: 12,
            legs_bottom_pct: 45,
            head_damage_pct: 100,
            torso_damage_pct: 100,
            legs_damage_pct: 60,
            wounded_hp: 50,
            overkill_kill: 60,
            head_kill_dmg: 0,
            bleed_ms_per_hp: 4000,
            down_bleed_min_s: 60,
            down_bleed_max_s: 180,
            wounded_spread_pct: 130,
            wounded_reload_pct: 125,
            wounded_speed_pct: 85,
            legs_speed_pct: 60,
            bandage_s: 10,
            stabilise_s: 15,
            drag_speed_pct: 30,
            stretcher_speed_pct: 60,
            treat_s: 60,
            return_hp_pct: 75,
            evac_s: 30,
            evac_refund_pct: 40,
            auto_tend_m: 25,
            reinforce: 0,
            reinforce_cost_pct: 100,
        }
    }
}

tuning_table! {
    /// `[ai.medic]` (S12b, docs/slices/S12-wounds-medics.md 2, 5, rules
    /// A1-A5): how the AI buys and uses medic teams, aid posts and
    /// reinforcements. Every switch 0 in S12a (absent: these values).
    pub struct AiMedicTuning {
        /// A1 switch: buy medic teams.
        buy: 0..=1 = "d_w_zero",
        /// A3 switch: field medics tend and carry.
        field: 0..=1 = "d_w_zero",
        /// A2, A4 switch: build and staff an aid post.
        post: 0..=1 = "d_w_zero",
        /// A5 switch: reinforce squads.
        reinforce: 0..=1 = "d_w_zero",
        /// A1: medic teams at most ...
        max_teams: 0..=8 = "d_am_max_teams",
        /// ... bought only while this many infantry squads live.
        min_inf_squads: 0..=50 = "d_am_min_inf_squads",
        /// A3: a field team follows this far behind the lead squad (m) ...
        front_m: 0..=2000 = "d_am_front_m",
        /// ... and goes to a downed man only with no spotted enemy this
        /// close to him (m).
        enemy_clear_m: 0..=2000 = "d_am_enemy_clear_m",
        /// A5: reinforce a squad under this share of its card size (%).
        reinforce_below_pct: 1..=100 = "d_am_reinforce_below_pct",
    }
}

fn d_am_max_teams() -> u32 {
    2
}
fn d_am_min_inf_squads() -> u32 {
    4
}
fn d_am_front_m() -> u32 {
    60
}
fn d_am_enemy_clear_m() -> u32 {
    40
}
fn d_am_reinforce_below_pct() -> u32 {
    60
}

impl AiMedicTuning {
    fn check_more(&self, _at: &str, _out: &mut Vec<Issue>) {}
}

impl Default for AiMedicTuning {
    /// The S12a values (a rules def without `[ai.medic]`): switches off.
    fn default() -> AiMedicTuning {
        AiMedicTuning {
            buy: 0,
            field: 0,
            post: 0,
            reinforce: 0,
            max_teams: 2,
            min_inf_squads: 4,
            front_m: 60,
            enemy_clear_m: 40,
            reinforce_below_pct: 60,
        }
    }
}

tuning_table! {
    /// `[armour]` (S13, docs/slices/S13-heavy-wave.md 3, rule O1):
    /// obliquity by plate-to-calibre ratio for rounds with a
    /// `slope_class`. Absent: these values. With no classed round the
    /// rule changes nothing.
    pub struct ArmourTuning {
        /// O1 switch (0: every round on the cosine law, `slope_class`
        /// ignored).
        obliquity: 0..=1 = "d_ar_obliquity",
        /// The O1 weight ramps from 0 at this plate-to-calibre ratio (% of
        /// the calibre) ...
        td_lo_pct: 0..=300 = "d_ar_td_lo_pct",
        /// ... to its class's full weight at this one (%).
        td_hi_pct: 0..=300 = "d_ar_td_hi_pct",
        /// Full weight of class `apcbc` (AP, APC, APCBC: `c^1.5` at 100 %)
        /// (%) ...
        apcbc_w_pct: 0..=100 = "d_ar_hundred",
        /// ... and of class `apcr` (APCR, HVAP: `c^2` at 100 %) (%).
        apcr_w_pct: 0..=100 = "d_ar_hundred",
    }
}

fn d_ar_obliquity() -> u32 {
    1
}
fn d_ar_td_lo_pct() -> u32 {
    40
}
fn d_ar_td_hi_pct() -> u32 {
    90
}
fn d_ar_hundred() -> u32 {
    100
}

impl ArmourTuning {
    fn check_more(&self, table: &str, out: &mut Vec<Issue>) {
        // The ramp needs lo < hi (no division by zero in the weight).
        if self.td_lo_pct >= self.td_hi_pct {
            out.push(
                Issue::new(
                    format!("{table}.td_lo_pct"),
                    format!(
                        "must be below td_hi_pct ({} >= {})",
                        self.td_lo_pct, self.td_hi_pct
                    ),
                )
                .with_hint(format!(
                    "[{table}] td_lo_pct = {}",
                    self.td_hi_pct.saturating_sub(1)
                )),
            );
        }
    }
}

impl Default for ArmourTuning {
    /// The S13 values (a rules def without `[armour]`).
    fn default() -> ArmourTuning {
        ArmourTuning {
            obliquity: 1,
            td_lo_pct: 40,
            td_hi_pct: 90,
            apcbc_w_pct: 100,
            apcr_w_pct: 100,
        }
    }
}

tuning_table! {
    /// `[flame]` (S14, docs/slices/S14-assault-kit.md 2, rules F1-F9):
    /// flame particles, burning men, burning ground, step-out. The switch
    /// `enabled` is 0 in S14a (absent: these values). Every number is a
    /// balance lever: change it here, not in code.
    pub struct FlameTuning {
        /// F1-F9 switch (0: flame members resolve to today's men, no
        /// particle).
        enabled: 0..=1 = "d_fl_zero",
        /// F1: particles in flight per player (hard ceiling 256).
        particles_per_player: 1..=256 = "d_fl_particles_per_player",
        /// F2: a particle's life (ms).
        ttl_ms: 50..=10000 = "d_fl_ttl_ms",
        /// F4: a particle passing this close to a man's capsule axis is a
        /// near miss (cm).
        near_cm: 1..=1000 = "d_fl_near_cm",
        /// F3: a struck man burns this long, refreshed by each hit (s) ...
        burn_s: 1..=3600 = "d_fl_burn_s",
        /// ... losing this much HP a second while burning ...
        burn_hp_per_s: 0..=1000 = "d_fl_burn_hp_per_s",
        /// ... dealt every this often (ms; also the patch pulse).
        pulse_ms: 50..=10000 = "d_fl_pulse_ms",
        /// F3: suppression points a burning man takes each pulse.
        burn_suppression: 0..=100 = "d_fl_burn_suppression",
        /// F5: burning ground reaches this far around its point (cm) ...
        patch_radius_cm: 1..=1000 = "d_fl_patch_radius_cm",
        /// ... burns this long (s) ...
        patch_s: 1..=3600 = "d_fl_patch_s",
        /// ... hurts a man in it this much HP a second ...
        patch_hp_per_s: 0..=1000 = "d_fl_patch_hp_per_s",
        /// ... whose feet are within this height of it (cm).
        patch_z_cm: 1..=1000 = "d_fl_patch_z_cm",
        /// F5: a landing this close to a patch refreshes it (cm).
        patch_merge_cm: 1..=1000 = "d_fl_patch_merge_cm",
        /// F5: patches at most (hard ceiling 256); one over is dropped.
        patches_max: 1..=256 = "d_fl_patches_max",
        /// F6 switch: men leave burning ground ...
        step_out: 0..=1 = "d_fl_one",
        /// ... to this far beyond its edge (m).
        step_out_m: 1..=50 = "d_fl_step_out_m",
        /// F7: the own base refills a flamethrower's fuel one unit per
        /// this long (ms; plan 3.2 F7, design 6.5 "one unit per 2 s").
        base_refill_ms: 100..=60000 = "d_fl_base_refill_ms",
    }
}

fn d_fl_zero() -> u32 {
    0
}
fn d_fl_one() -> u32 {
    1
}
fn d_fl_particles_per_player() -> u32 {
    64
}
fn d_fl_ttl_ms() -> u32 {
    2500
}
fn d_fl_near_cm() -> u32 {
    300
}
fn d_fl_burn_s() -> u32 {
    4
}
fn d_fl_burn_hp_per_s() -> u32 {
    15
}
fn d_fl_pulse_ms() -> u32 {
    500
}
fn d_fl_burn_suppression() -> u32 {
    40
}
fn d_fl_patch_radius_cm() -> u32 {
    100
}
fn d_fl_patch_s() -> u32 {
    15
}
fn d_fl_patch_hp_per_s() -> u32 {
    20
}
fn d_fl_patch_z_cm() -> u32 {
    100
}
fn d_fl_patch_merge_cm() -> u32 {
    50
}
fn d_fl_patches_max() -> u32 {
    128
}
fn d_fl_step_out_m() -> u32 {
    2
}
fn d_fl_base_refill_ms() -> u32 {
    2000
}

impl FlameTuning {
    fn check_more(&self, _at: &str, _out: &mut Vec<Issue>) {}
}

impl Default for FlameTuning {
    /// The S14a values (a rules def without `[flame]`): switch off.
    fn default() -> FlameTuning {
        FlameTuning {
            enabled: 0,
            particles_per_player: 64,
            ttl_ms: 2500,
            near_cm: 300,
            burn_s: 4,
            burn_hp_per_s: 15,
            pulse_ms: 500,
            burn_suppression: 40,
            patch_radius_cm: 100,
            patch_s: 15,
            patch_hp_per_s: 20,
            patch_z_cm: 100,
            patch_merge_cm: 50,
            patches_max: 128,
            step_out: 1,
            step_out_m: 2,
            base_refill_ms: 2000,
        }
    }
}

tuning_table! {
    /// `[melee]` (S14, docs/slices/S14-assault-kit.md 2, 5, rules M1-M5):
    /// the default strike (entrenching spade) of a weapon with no `[melee]`
    /// block. The switch `enabled` is 0 in S14a (absent: these values).
    pub struct MeleeTuning {
        /// M1-M5 switch (0: no strike; `Strike` refused).
        enabled: 0..=1 = "d_fl_zero",
        /// M1: the strike segment's length (cm) ...
        reach_cm: 1..=1000 = "d_ml_reach_cm",
        /// ... its damage (HP) ...
        damage: 0..=1000 = "d_ml_damage",
        /// ... wind-up (ms) ...
        windup_ms: 1..=4000 = "d_ml_windup_ms",
        /// ... and recovery with no fire (ms).
        recover_ms: 1..=4000 = "d_ml_recover_ms",
        /// M2: a strike starts with an enemy capsule within reach plus this
        /// (cm).
        start_pad_cm: 0..=1000 = "d_ml_start_pad_cm",
    }
}

fn d_ml_reach_cm() -> u32 {
    60
}
fn d_ml_damage() -> u32 {
    60
}
fn d_ml_windup_ms() -> u32 {
    500
}
fn d_ml_recover_ms() -> u32 {
    1000
}
fn d_ml_start_pad_cm() -> u32 {
    30
}

impl MeleeTuning {
    fn check_more(&self, _at: &str, _out: &mut Vec<Issue>) {}
}

impl Default for MeleeTuning {
    /// The S14a values (a rules def without `[melee]`): switch off.
    fn default() -> MeleeTuning {
        MeleeTuning {
            enabled: 0,
            reach_cm: 60,
            damage: 60,
            windup_ms: 500,
            recover_ms: 1000,
            start_pad_cm: 30,
        }
    }
}

tuning_table! {
    /// `[clear]` (S14, docs/slices/S14-assault-kit.md 2, 6, rules C1-C5):
    /// the `ClearTrench` order. The switch `enabled` is 0 in S14a (absent:
    /// these values).
    pub struct ClearTuning {
        /// C1-C5 switch (0: `ClearTrench` refused).
        enabled: 0..=1 = "d_fl_zero",
        /// C3: a step goes at least this far along the trench (m) ...
        step_m: 1..=100 = "d_cl_step_m",
        /// ... at most this many steps (C5) ...
        steps_max: 1..=50 = "d_cl_steps_max",
        /// ... this many men throw at each step point ...
        throwers: 1..=8 = "d_cl_throwers",
        /// ... and the squad waits this long after the fuse (s).
        settle_s: 0..=3600 = "d_cl_settle_s",
        /// C3: the trench walk visits at most this many 4 m cells.
        walk_cells: 1..=1024 = "d_cl_walk_cells",
    }
}

fn d_cl_step_m() -> u32 {
    8
}
fn d_cl_steps_max() -> u32 {
    6
}
fn d_cl_throwers() -> u32 {
    2
}
fn d_cl_settle_s() -> u32 {
    2
}
fn d_cl_walk_cells() -> u32 {
    64
}

impl ClearTuning {
    fn check_more(&self, _at: &str, _out: &mut Vec<Issue>) {}
}

impl Default for ClearTuning {
    /// The S14a values (a rules def without `[clear]`): switch off.
    fn default() -> ClearTuning {
        ClearTuning {
            enabled: 0,
            step_m: 8,
            steps_max: 6,
            throwers: 2,
            settle_s: 2,
            walk_cells: 64,
        }
    }
}

tuning_table! {
    /// `[ai.flame]` (S14-ai, docs/slices/S14-assault-kit.md 2, 7, rules
    /// A1-A4): the AI's flame jobs, refuel, flame buys and trench clears.
    /// Every switch 0 in S14a (absent: these values).
    pub struct AiFlameTuning {
        /// A1 switch: flame jobs.
        jobs: 0..=1 = "d_fl_zero",
        /// A2 switch: refuel a dry flame man.
        refuel: 0..=1 = "d_fl_zero",
        /// A3 switch: buy the faction's flame card.
        buy: 0..=1 = "d_fl_zero",
        /// A4 switch: order `ClearTrench`.
        clear: 0..=1 = "d_fl_zero",
        /// A1: a target is a known enemy this close to the point (m) ...
        target_m: 1..=1000 = "d_af_target_m",
        /// ... the squad closes to this far short of it (m) ...
        stage_m: 1..=1000 = "d_af_stage_m",
        /// ... and the flame man to this far from it (m).
        stand_m: 1..=1000 = "d_af_stand_m",
        /// A1, A2: fuel kept in reserve (% of a full tank).
        reserve_pct: 0..=100 = "d_af_reserve_pct",
        /// A1 L1: a job ends after this long (s) ...
        job_max_s: 1..=3600 = "d_af_job_max_s",
        /// ... L2: or with no phase progress for this long (s).
        stall_s: 1..=3600 = "d_af_stall_s",
        /// A3: flame cards alive at most.
        cards_max: 0..=8 = "d_af_cards_max",
        /// A4: clear a trench with a known enemy this close to the point
        /// (m).
        clear_near_m: 1..=1000 = "d_af_clear_near_m",
        /// A1 L3, A4: a failed target or squad waits this long (s).
        recheck_s: 1..=3600 = "d_af_recheck_s",
    }
}

fn d_af_target_m() -> u32 {
    40
}
fn d_af_stage_m() -> u32 {
    40
}
fn d_af_stand_m() -> u32 {
    20
}
fn d_af_reserve_pct() -> u32 {
    10
}
fn d_af_job_max_s() -> u32 {
    180
}
fn d_af_stall_s() -> u32 {
    45
}
fn d_af_cards_max() -> u32 {
    2
}
fn d_af_clear_near_m() -> u32 {
    15
}
fn d_af_recheck_s() -> u32 {
    60
}

impl AiFlameTuning {
    fn check_more(&self, _at: &str, _out: &mut Vec<Issue>) {}
}

impl Default for AiFlameTuning {
    /// The S14a values (a rules def without `[ai.flame]`): switches off.
    fn default() -> AiFlameTuning {
        AiFlameTuning {
            jobs: 0,
            refuel: 0,
            buy: 0,
            clear: 0,
            target_m: 40,
            stage_m: 40,
            stand_m: 20,
            reserve_pct: 10,
            job_max_s: 180,
            stall_s: 45,
            cards_max: 2,
            clear_near_m: 15,
            recheck_s: 60,
        }
    }
}

/// Every tuning table: its dotted path in the rules TOML and its keys with
/// their ranges (docs and the per-key tests).
pub const TABLES: &[(&str, &[TuningKey])] = &[
    ("supply", SupplyTuning::KEYS),
    ("combat.share", ShareTuning::KEYS),
    ("combat.fire", FireTuning::KEYS),
    ("ai.ammo", AiAmmoTuning::KEYS),
    ("fire_mission", FireMissionTuning::KEYS),
    ("ai.mortar", AiMortarTuning::KEYS),
    ("ai.supply", AiSupplyTuning::KEYS),
    ("ai.transport", AiTransportTuning::KEYS),
    ("ai.recon", AiReconTuning::KEYS),
    ("ai.truck", AiTruckTuning::KEYS),
    ("ai.guns", AiGunsTuning::KEYS),
    ("ai.fort", AiFortTuning::KEYS),
    ("ai.obstacles", AiObstacleTuning::KEYS),
    ("ai.arty", AiArtyTuning::KEYS),
    ("wounds", WoundsTuning::KEYS),
    ("ai.medic", AiMedicTuning::KEYS),
    ("armour", ArmourTuning::KEYS),
    ("flame", FlameTuning::KEYS),
    ("melee", MeleeTuning::KEYS),
    ("clear", ClearTuning::KEYS),
    ("ai.flame", AiFlameTuning::KEYS),
    ("combat.body", BodyTuning::KEYS),
    ("combat.mortar", MortarTuning::KEYS),
    ("combat.blast", BlastTuning::KEYS),
    ("combat.suppression", SuppressionTuning::KEYS),
    ("combat.suppress", SuppressTuning::KEYS),
    ("ai.infantry", AiInfantryTuning::KEYS),
    ("ai.armour", AiArmourTuning::KEYS),
    ("ai.assault", AiAssaultTuning::KEYS),
    ("vehicles.driving", DrivingTuning::KEYS),
    ("vehicles.path", VehiclePathTuning::KEYS),
    ("vehicles.escort", EscortTuning::KEYS),
    ("vehicles.gunnery", GunneryTuning::KEYS),
    ("vehicles.damage", VehicleDamageTuning::KEYS),
    ("vehicles.bail", BailTuning::KEYS),
    ("vehicles.recrew", RecrewTuning::KEYS),
    ("vehicles.transport", TransportTuning::KEYS),
    ("vehicles.guns", GunsTuning::KEYS),
    ("movement.formation", FormationTuning::KEYS),
    ("cover", CoverTuning::KEYS),
    ("items.throw", ThrowTuning::KEYS),
    ("dig.common", DigCommonTuning::KEYS),
    ("call_in", CallInTuning::KEYS),
    ("fortify", FortTuning::KEYS),
    ("fortify.tiers.field", TierTuning::KEYS),
    ("fortify.tiers.assault", TierTuning::KEYS),
    ("fortify.tiers.construction", TierTuning::KEYS),
];

/// Every tuning table of a rules def or setup, for [`check_tuning`].
pub(crate) struct TuningRefs<'a> {
    pub body: &'a BodyTuning,
    pub mortar: &'a MortarTuning,
    pub blast: &'a BlastTuning,
    pub suppression: &'a SuppressionTuning,
    pub suppress: &'a SuppressTuning,
    pub ai_infantry: &'a AiInfantryTuning,
    pub ai_armour: &'a AiArmourTuning,
    pub ai_assault: &'a AiAssaultTuning,
    pub driving: &'a DrivingTuning,
    pub vehicle_path: &'a VehiclePathTuning,
    pub escort: &'a EscortTuning,
    pub gunnery: &'a GunneryTuning,
    pub vehicle_damage: &'a VehicleDamageTuning,
    pub bail: &'a BailTuning,
    pub recrew: &'a RecrewTuning,
    pub transport: &'a TransportTuning,
    pub guns: &'a GunsTuning,
    pub formation: &'a FormationTuning,
    pub cover: &'a CoverTuning,
    pub throw: &'a ThrowTuning,
    pub dig_common: &'a DigCommonTuning,
    pub call_in: &'a CallInTuning,
    pub supply: Option<&'a SupplyTuning>,
    pub share: &'a ShareTuning,
    pub fire: &'a FireTuning,
    pub ai_ammo: &'a AiAmmoTuning,
    pub fire_mission: Option<&'a FireMissionTuning>,
    pub fortify: Option<&'a FortTuning>,
    pub ai_mortar: &'a AiMortarTuning,
    pub ai_supply: &'a AiSupplyTuning,
    pub ai_transport: &'a AiTransportTuning,
    pub ai_recon: &'a AiReconTuning,
    pub ai_truck: &'a AiTruckTuning,
    pub ai_guns: &'a AiGunsTuning,
    pub ai_fort: &'a AiFortTuning,
    pub ai_obstacles: &'a AiObstacleTuning,
    pub ai_arty: &'a AiArtyTuning,
    pub wounds: &'a WoundsTuning,
    pub ai_medic: &'a AiMedicTuning,
    pub armour: &'a ArmourTuning,
    pub flame: &'a FlameTuning,
    pub melee: &'a MeleeTuning,
    pub clear: &'a ClearTuning,
    pub ai_flame: &'a AiFlameTuning,
}

/// Checks every tuning table; `prefix` goes before each table path (`""`
/// for a rules def, `"rules."` for a setup).
pub(crate) fn check_tuning(out: &mut Vec<Issue>, prefix: &str, t: TuningRefs) {
    let p = |table: &str| format!("{prefix}{table}");
    t.body.check(&p("combat.body"), out);
    t.mortar.check(&p("combat.mortar"), out);
    t.blast.check(&p("combat.blast"), out);
    t.suppression.check(&p("combat.suppression"), out);
    t.suppress.check(&p("combat.suppress"), out);
    t.ai_infantry.check(&p("ai.infantry"), out);
    t.ai_armour.check(&p("ai.armour"), out);
    t.ai_assault.check(&p("ai.assault"), out);
    if t.ai_assault.trench_line_m > t.suppress.line_max_m {
        out.push(
            Issue::new(
                format!("{prefix}ai.assault.trench_line_m"),
                format!(
                    "must not exceed combat.suppress.line_max_m ({} > {})",
                    t.ai_assault.trench_line_m, t.suppress.line_max_m
                ),
            )
            .with_hint(format!(
                "[ai.assault] trench_line_m = {}",
                t.suppress.line_max_m
            )),
        );
    }
    t.driving.check(&p("vehicles.driving"), out);
    t.vehicle_path.check(&p("vehicles.path"), out);
    t.escort.check(&p("vehicles.escort"), out);
    t.gunnery.check(&p("vehicles.gunnery"), out);
    t.vehicle_damage.check(&p("vehicles.damage"), out);
    t.bail.check(&p("vehicles.bail"), out);
    t.recrew.check(&p("vehicles.recrew"), out);
    t.transport.check(&p("vehicles.transport"), out);
    t.guns.check(&p("vehicles.guns"), out);
    t.formation.check(&p("movement.formation"), out);
    t.cover.check(&p("cover"), out);
    t.throw.check(&p("items.throw"), out);
    t.dig_common.check(&p("dig.common"), out);
    t.call_in.check(&p("call_in"), out);
    if let Some(sp) = t.supply {
        sp.check(&p("supply"), out);
    }
    t.share.check(&p("combat.share"), out);
    t.fire.check(&p("combat.fire"), out);
    t.ai_ammo.check(&p("ai.ammo"), out);
    if let Some(fm) = t.fire_mission {
        fm.check(&p("fire_mission"), out);
    }
    if let Some(f) = t.fortify {
        f.check(&p("fortify"), out);
    }
    t.ai_mortar.check(&p("ai.mortar"), out);
    t.ai_supply.check(&p("ai.supply"), out);
    t.ai_transport.check(&p("ai.transport"), out);
    t.ai_recon.check(&p("ai.recon"), out);
    t.ai_truck.check(&p("ai.truck"), out);
    t.ai_guns.check(&p("ai.guns"), out);
    t.ai_fort.check(&p("ai.fort"), out);
    t.ai_obstacles.check(&p("ai.obstacles"), out);
    t.ai_arty.check(&p("ai.arty"), out);
    t.wounds.check(&p("wounds"), out);
    t.ai_medic.check(&p("ai.medic"), out);
    t.armour.check(&p("armour"), out);
    t.flame.check(&p("flame"), out);
    t.melee.check(&p("melee"), out);
    t.clear.check(&p("clear"), out);
    t.ai_flame.check(&p("ai.flame"), out);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_issue_names_the_key_and_a_fix() {
        let mut out = Vec::new();
        let t = MortarTuning {
            muzzle_mm: 800,
            flight_milli: 0,
            min_flight_s: 4,
            barrage_retry_m: 20,
        };
        t.check("combat.mortar", &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].path, "combat.mortar.flight_milli");
        assert_eq!(
            out[0].hint.as_deref(),
            Some("[combat.mortar] flight_milli = 1")
        );
    }

    #[test]
    fn order_checks_catch_swapped_keys() {
        let mut out = Vec::new();
        let t = BlastTuning {
            lift_mm: 300,
            crater_depth_min_pct: 150,
            crater_depth_max_pct: 140,
            crater_lift_mm: 500,
            frag_fall_probe_cm: 150,
        };
        t.check("combat.blast", &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].path, "combat.blast.crater_depth_min_pct");
    }

    #[test]
    fn keys_list_every_field() {
        assert_eq!(BodyTuning::KEYS.len(), 11);
        assert_eq!(DrivingTuning::KEYS.len(), 28);
    }
}
