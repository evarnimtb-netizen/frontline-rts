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
                $field:ident : $lo:literal ..= $hi:literal,
            )*
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
        #[serde(deny_unknown_fields)]
        pub struct $name {
            $( $(#[doc = $doc])* pub $field: u32, )*
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
    }
}

impl SuppressionTuning {
    fn check_more(&self, _: &str, _: &mut Vec<Issue>) {}
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
        /// External parts (tracks, barrel) within this of a
        /// non-penetrating impact take the round's `nonpen_damage` (mm).
        external_reach_mm: 0..=10000,
        /// A spall ray ends after this (m).
        spall_reach_m: 1..=30,
    }
}

impl GunneryTuning {
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
    }
}

impl DigCommonTuning {
    fn check_more(&self, _: &str, _: &mut Vec<Issue>) {}
}

/// Every tuning table: its dotted path in the rules TOML and its keys with
/// their ranges (docs and the per-key tests).
pub const TABLES: &[(&str, &[TuningKey])] = &[
    ("combat.body", BodyTuning::KEYS),
    ("combat.mortar", MortarTuning::KEYS),
    ("combat.blast", BlastTuning::KEYS),
    ("combat.suppression", SuppressionTuning::KEYS),
    ("ai.infantry", AiInfantryTuning::KEYS),
    ("ai.armour", AiArmourTuning::KEYS),
    ("vehicles.driving", DrivingTuning::KEYS),
    ("vehicles.path", VehiclePathTuning::KEYS),
    ("vehicles.escort", EscortTuning::KEYS),
    ("vehicles.gunnery", GunneryTuning::KEYS),
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
    pub ai_infantry: &'a AiInfantryTuning,
    pub ai_armour: &'a AiArmourTuning,
    pub driving: &'a DrivingTuning,
    pub vehicle_path: &'a VehiclePathTuning,
    pub escort: &'a EscortTuning,
    pub gunnery: &'a GunneryTuning,
    pub formation: &'a FormationTuning,
    pub cover: &'a CoverTuning,
    pub throw: &'a ThrowTuning,
    pub dig_common: &'a DigCommonTuning,
    pub call_in: &'a CallInTuning,
}

/// Checks every tuning table; `prefix` goes before each table path (`""`
/// for a rules def, `"rules."` for a setup).
pub(crate) fn check_tuning(out: &mut Vec<Issue>, prefix: &str, t: TuningRefs) {
    let p = |table: &str| format!("{prefix}{table}");
    t.body.check(&p("combat.body"), out);
    t.mortar.check(&p("combat.mortar"), out);
    t.blast.check(&p("combat.blast"), out);
    t.suppression.check(&p("combat.suppression"), out);
    t.ai_infantry.check(&p("ai.infantry"), out);
    t.ai_armour.check(&p("ai.armour"), out);
    t.driving.check(&p("vehicles.driving"), out);
    t.vehicle_path.check(&p("vehicles.path"), out);
    t.escort.check(&p("vehicles.escort"), out);
    t.gunnery.check(&p("vehicles.gunnery"), out);
    t.formation.check(&p("movement.formation"), out);
    t.cover.check(&p("cover"), out);
    t.throw.check(&p("items.throw"), out);
    t.dig_common.check(&p("dig.common"), out);
    t.call_in.check(&p("call_in"), out);
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
