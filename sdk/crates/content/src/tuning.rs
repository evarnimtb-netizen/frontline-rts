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
    }
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
    }
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
    }
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
        /// (S5a only: from S6 bombs come in crates).
        base_bomb_s: 1..=600,
        /// No bomb goes into a mortar squad in the own base until this long
        /// after its mortar last fired (s; 0 or absent: no wait). S5a Fix
        /// (review M2): brakes the base battery (artillery-on-map.md §4.10)
        /// until S6 crates.
        base_bomb_after_fire_s: 0..=1800 = "default_base_bomb_after_fire_s",
        /// HUD "low" marker below this share of a squad's load (%).
        low_pct: 1..=100,
    }
}

fn default_base_bomb_after_fire_s() -> u32 {
    0
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

/// Every tuning table: its dotted path in the rules TOML and its keys with
/// their ranges (docs and the per-key tests).
pub const TABLES: &[(&str, &[TuningKey])] = &[
    ("supply", SupplyTuning::KEYS),
    ("combat.share", ShareTuning::KEYS),
    ("combat.fire", FireTuning::KEYS),
    ("ai.ammo", AiAmmoTuning::KEYS),
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
    ("movement.formation", FormationTuning::KEYS),
    ("cover", CoverTuning::KEYS),
    ("items.throw", ThrowTuning::KEYS),
    ("dig.common", DigCommonTuning::KEYS),
    ("call_in", CallInTuning::KEYS),
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
    pub formation: &'a FormationTuning,
    pub cover: &'a CoverTuning,
    pub throw: &'a ThrowTuning,
    pub dig_common: &'a DigCommonTuning,
    pub call_in: &'a CallInTuning,
    pub supply: Option<&'a SupplyTuning>,
    pub share: &'a ShareTuning,
    pub fire: &'a FireTuning,
    pub ai_ammo: &'a AiAmmoTuning,
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
        assert_eq!(DrivingTuning::KEYS.len(), 24);
    }
}
