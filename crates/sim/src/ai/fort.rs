//! S10a-ai fortification habits (`docs/slices/S10a-ai.md` 3.1-3.3;
//! `[ai.fort]`). One [`AiFortJob`] per builder squad (`AiState::forts`):
//!
//! * **Jobs.** A free `build_structure` / `place_wire` squad assigned to an
//!   own point with a finished AI trench (threat under `build_threat_max`)
//!   builds the point's MG nest, then its wire line, then (shelled, or an
//!   enemy mortar seen) a dugout; a builder within `builder_reach_m` of the
//!   own HQ roofs it once an enemy mortar was seen, and of a gun in Hold for
//!   `gun_hold_s` sandbags it. At most `builders_max` jobs, `works_per_point`
//!   works a point. Sites come from the saved trench line by integer maths
//!   (no RNG, no HashMap order).
//! * **Phases.** Load (full carry in base, under the materials budget) ->
//!   Go (`Fortify`) -> Order (the work appears, else refused) -> Watch
//!   (until the work ends) -> Done. A job's squad leaves `mine`: no other
//!   habit re-orders it (S10a-6). Below 60 % strength or with a known enemy
//!   within `threat_radius` the job is dropped (the mechanics remove the
//!   work once the squad gets another order).
//! * **Materials.** A builder in base carrying under `load_min_kg` loads its
//!   full carry; materials MP stay under `materials_max_pct` of the income
//!   earned. A work waiting for kg gets a truck run (`material_truck`), else
//!   its builder walks back to base, loads and returns (the line goes on
//!   from the first segment not yet standing). The S6 construction-crate
//!   run is not built (Build B dropped it; see the log).
//! * **Marks.** A held point is marked shelled when `shelled_pct` of its
//!   defenders' men are pinned or ducked with no known enemy within
//!   `shelled_clear_m`; `enemy_mortar_seen` once a known enemy soldier is in
//!   a `fire_mission` squad. `order_infantry` reads them for the shelter
//!   move, and puts one MG team into each built nest.
//! * **S10b-ai fields** (`docs/slices/S10b-ai.md` R1-R4, R8, L1-L4). With
//!   `[ai.obstacles] mines` a held trenched point also wants an AP field
//!   (after its wire, or with wire off) and, with `at_mines`, an AT field
//!   across the enemy vehicle approach (one vehicle path search per point
//!   per `recheck_s`). They run as fort jobs inside `jobs_max`, on their own
//!   materials budget; a job that fails (refused order, stall, deadline
//!   `job_max_s`) writes an `AiRefusal` and no job starts near a young one.

use frontline_content::MatchSetup;
use frontline_content::setup::{
    Capability, DigProfile, Fixed, FortShape, SquadTypeSetup, StockKind, SupplyKind,
};
use serde::{Deserialize, Serialize};

use super::{Known, KnownVehicle, Mine, MyTank, NO_POINT, PointEval, Rel, TaskKind, sq_m};
use crate::ai::{HabitEnd, JobKind};
use crate::command::CommandKind;
use crate::ecs::PlayerId;
use crate::fort::{FortState, WorkStep};
use crate::fx::{Fx, FxVec2};
use crate::game::{NavPhase, Order, SquadId};

/// Most fort jobs one AI keeps (spec 4).
pub const MAX_FORT_JOBS: usize = 16;
/// Most tries a fort job makes before it ends (an untrusted save cannot
/// hold more).
pub const MAX_FORT_TRIES: u8 = 16;

/// S10b-ai Build D: names of `AiCounters::bunker_why`. 0-11: the first
/// check that kept the bunker job from starting at a plan (no own trenched
/// point; the point not ready (not own, trench not done, threat); no site
/// or one stands; `builders_max`; `jobs_max`, the blueprint limit or a
/// young refusal; another job started at the point this plan; no free
/// squad assigned there; none of them a fit builder (60 %, caps); none
/// can bring the 250 kg step (carry, free truck, kg at hand); the S10b
/// budget room; a known enemy near; none of these (it would start).
/// 12-15: bunker jobs ended by the deadline, dropped (squad gone, weak,
/// point lost or threatened), failed, done.
pub const BUNKER_WHY: [&str; 16] = [
    "no_point",
    "point_not_ready",
    "no_site",
    "builders_max",
    "s10b_room",
    "other_first",
    "no_squad",
    "no_builder",
    "no_carry",
    "no_budget",
    "threat",
    "would_start",
    "end_deadline",
    "end_dropped",
    "end_failed",
    "end_done",
];
/// S10c-ai (review finding 12): names of `AiCounters::mine_want_why`, the
/// first check that kept an R1 (AP) or R2 (AT) field from starting at an
/// own point, per plan (AT fields on `recheck_s` plans only, as R2): the
/// trench not done or the point threatened (`build_threat_max`); the AP
/// field waits for the wire; a cap (`fields_per_point`, `at_fields_max`,
/// the blueprint limit); no site; a young refusal there (L4); no job slot
/// (`builders_max`, `jobs_max`); no fit free builder assigned there (60 %,
/// caps, materials); every fit builder or the site has a known enemy near.
pub const MINE_WANT_WHY: [&str; 8] = [
    "not_own_or_trench",
    "wire_first",
    "cap",
    "no_site",
    "refused",
    "slot",
    "no_builder",
    "threat",
];
/// S10c-ai (Build B): names of `AiCounters::front_why`, the first check
/// that kept a wanted front work from starting (as `BUNKER_WHY`).
pub const FRONT_WHY: [&str; 12] = [
    "no_front",
    "lane_none",
    "site_bad",
    "cap",
    "refused",
    "slot",
    "builders_max",
    "no_builder",
    "no_carry",
    "no_budget",
    "threat_squad",
    "threat_site",
];
/// Loads a job asks for before it goes with what it has.
const LOAD_TRIES: u8 = 2;
/// A nest, dugout or wall "exists" near its site within this (m).
const SITE_NEAR_M: i32 = 6;
/// A point work's facing point lies this far toward the enemy (m).
const FACING_M: i32 = 10;
/// The sandbag wall at a gun is this long (m; two 2 m segments).
const WALL_LEN_M: i32 = 4;
/// S10b-ai R5: the log barrier before a gun is this long at most (m).
const BARRIER_LEN_M: i32 = 20;
/// S10b-ai R5: a built bunker anchors its point's hedgehog row within this
/// of the point (m).
const BUNKER_NEAR_M: i32 = 40;
/// S10b-ai `own_blocked`: an own blocking piece this near a failed own
/// vehicle (m; spec 3 item 3).
const OWN_BLOCKED_M: i32 = 8;
/// S10b-ai (spec 5): another work this near an S10b job's site takes it (m).
const SITE_TAKEN_M: i32 = 2;
/// S10c-ai F5: a front builder whose leader is this near its site (m) is
/// "at the site": its own threat radius is then `front_site_threat_m`.
const FRONT_AT_SITE_M: i32 = 20;
/// S10c-ai F6: a mine (wire section) of a front line counts as laid when an
/// own one of the blueprint is this near its site (m).
const FRONT_LAID_M: i32 = 1;
/// The wire's fallback distance when the first line is refused (m).
const WIRE_RETRY_M: i32 = 25;
/// A truck run gives up this long after its start beyond
/// `material_wait_s` (the drive there, s).
const RUN_DRIVE_S: u64 = 180;
/// Fix F4/F17: a refused work is remembered this long (s): its job stays
/// in `AiState::forts` parked ([`AiFortJob::parked`]), its squad free,
/// and the point does not want that work meanwhile (a job counts as the
/// work existing). A constant: a tuning key would change the saved setup.
const REFUSED_S: u64 = 300;
/// Fix F5 (spec 3.3 "2 per player"): most dugouts one AI keeps.
const DUGOUTS_MAX: u32 = 2;
/// Fix F13: a watched work whose step (a line: its segment) has not moved
/// for `WATCH_STALL_X` times the step's time at the squad's strength plus
/// `WATCH_SLACK_S` ends its job (pinned men or men off their spots add no
/// work).
const WATCH_STALL_X: u64 = 3;
const WATCH_SLACK_S: u64 = 60;
/// Fix F13: a builder whose leader walks farther than this from the site
/// is still on its way (no stall).
const WATCH_NEAR_M: i32 = 12;
/// Most men one dugout shelters, and the step between their spots (mm).
pub(super) const SHELTER_SPOTS: usize = 8;
const SHELTER_STEP_MM: i32 = 250;

/// Where a fort job stands (spec 3.3: Load -> Go -> Order -> Watch -> Done).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum FortPhase {
    /// The builder loads materials in its base.
    #[default]
    Load = 0,
    /// It walks to the site.
    Go = 1,
    /// `Fortify` is sent.
    Order = 2,
    /// The work stands or is being built; the job watches it.
    Watch = 3,
    /// Finished (dropped at the next plan).
    Done = 4,
}

/// S10c-ai (spec 3, review finding 4): most `AiState::front_lanes`.
pub const MAX_FRONT_LANES: usize = 8;

/// S10c-ai (F2, F3): the two site lines of one front point, fixed when the
/// point's lane is first found, so every plan sees the front wants (Build B
/// fills them; empty in Build A). Each line is (start, end).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct FrontLane {
    /// Capture point index.
    pub point: u8,
    /// Tick the lane was found (at most the current tick).
    pub since: u64,
    /// The FrontAp line and the FrontWire line.
    pub ap: [FxVec2; 2],
    pub wire: [FxVec2; 2],
    /// F6: a FrontAp / FrontWire job of this lane ended laid / built.
    pub ap_done: bool,
    pub wire_done: bool,
}

/// One builder squad's fort job (spec 4; keyed by the squad).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AiFortJob {
    /// Index into `MatchSetup::forts`.
    pub fort: u16,
    /// Capture point index, or `ai::NO_POINT` (a gun's wall, the HQ roof).
    pub point: u8,
    /// The `Fortify` line (at most `fort::MAX_FORT_POINTS` points).
    pub line: Vec<FxVec2>,
    pub phase: FortPhase,
    /// S10b-ai (L1): tick the job began (at most `since`).
    pub start: u64,
    /// Tick the phase began (Watch: the last plan its work was not
    /// waiting for materials).
    pub since: u64,
    /// The work id once the order was accepted.
    pub work: Option<u32>,
    /// Load and Order: loads asked and refusals. Watch: the work's step
    /// last seen (`WorkStep as u8`, the progress mark, fix F13). Done with
    /// `MAX_FORT_TRIES` and no work: parked ([`Self::parked`]).
    pub tries: u8,
}

impl AiFortJob {
    /// Fix F4/F17: a refused work remembered until `since` +
    /// `REFUSED_S`; the squad is free meanwhile.
    pub fn parked(&self) -> bool {
        self.phase == FortPhase::Done && self.work.is_none() && self.tries == MAX_FORT_TRIES
    }
}

/// The AI's use of a blueprint.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
// The S10b-ai kinds have no caller until Build B (tests use them).
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) enum WorkKind {
    Nest,
    Wire,
    Dugout,
    Roof,
    Wall,
    /// S10b-ai (spec 3): an AP minefield (a line blueprint laying a foot
    /// mine).
    ApField,
    /// An AT minefield (a line blueprint laying a vehicle mine).
    AtField,
    /// S10b-ai Build E (`at_rows` 2, S10b-ai-B2a): the AT field's second
    /// row, its own job on the AT minefield blueprint, once the first is
    /// laid ([`super::Planner::at_row2_site`]).
    AtRow2,
    /// A Czech hedgehog (a point blueprint blocking vehicles).
    Hedgehog,
    /// A log barrier (a line blueprint blocking vehicles).
    LogBarrier,
    /// The bunker (a point blueprint on a `bunker_pit`).
    Bunker,
    /// S10c-ai F3: the hasty AP field across a front point's foot lane
    /// (the AP minefield blueprint, its line from `FrontLane::ap`).
    FrontAp,
    /// S10c-ai F3: the short wire line across a front point's foot lane
    /// (the wire blueprint, its line from `FrontLane::wire`).
    FrontWire,
    /// S10c-ai C4: the belt, a row of blocking pieces across a front
    /// point's vehicle lane beyond its AT field (the hedgehog blueprint
    /// here; a log barrier line when no `build_heavy` squad can take it).
    Belt,
}

/// S10a-ai (spec 3.1): the three S10a cards (construction or assault
/// engineers), skipped by `[ai.fort] cards = 0`.
pub(crate) fn is_s10a_card(ty: &SquadTypeSetup) -> bool {
    ty.has(Capability::BuildHeavy) || ty.has(Capability::Breach)
}

/// Whether capability bits `caps` build something (`build_structure` or
/// `place_wire`).
pub(super) fn builds(caps: u32) -> bool {
    caps & (Capability::BuildStructure.bit() | Capability::PlaceWire.bit()) != 0
}

/// The blueprint the AI uses for `k` (the first that fits; by shape, pit,
/// mine trigger and `blocks_vehicles`, never by id).
pub(crate) fn fort_of(s: &MatchSetup, k: WorkKind) -> Option<u16> {
    // S10b-ai: the mine trigger of a minefield blueprint (0 foot, 1 vehicle).
    let trigger = |d: &frontline_content::setup::FortSetup| {
        d.mine_item
            .and_then(|i| s.items.get(i as usize))
            .and_then(|it| it.mine.as_ref())
            .map(|m| m.trigger)
    };
    s.forts
        .iter()
        .position(|d| match k {
            // S10b-ai (spec 3): the new kinds take exactly their S10b
            // blueprints.
            WorkKind::ApField | WorkKind::FrontAp => {
                d.shape == FortShape::Line && trigger(d) == Some(0)
            }
            WorkKind::AtField | WorkKind::AtRow2 => {
                d.shape == FortShape::Line && trigger(d) == Some(1)
            }
            WorkKind::Hedgehog | WorkKind::Belt => d.shape == FortShape::Point && d.blocks_vehicles,
            WorkKind::LogBarrier => d.shape == FortShape::Line && d.blocks_vehicles,
            WorkKind::Bunker => {
                d.mine_item.is_none()
                    && !d.blocks_vehicles
                    && d.shape == FortShape::Point
                    && d.pit == Some(DigProfile::BunkerPit)
            }
            // S10b R29: minefields and obstacles are never an S10a habit's
            // blueprint (`ap_minefield` sorts before `sandbag_wall`).
            _ => {
                d.mine_item.is_none()
                    && !d.blocks_vehicles
                    && match k {
                        WorkKind::Nest => {
                            d.shape == FortShape::Point && d.pit == Some(DigProfile::MgNestPit)
                        }
                        WorkKind::Dugout => {
                            d.shape == FortShape::Point && d.pit == Some(DigProfile::DugoutPit)
                        }
                        WorkKind::Roof => d.shape == FortShape::OverHq,
                        WorkKind::Wire | WorkKind::FrontWire => {
                            d.shape == FortShape::Line && d.wire.is_some()
                        }
                        WorkKind::Wall => d.shape == FortShape::Line && d.wire.is_none(),
                        _ => false,
                    }
            }
        })
        .map(|i| i as u16)
}

/// Whether `[ai.fort]` lets the AI build `k` (`works` and the kind's own
/// switch, stage D; wire: `wire`).
pub(crate) fn switched_on(s: &MatchSetup, k: WorkKind) -> bool {
    let f = &s.rules.ai.fort;
    let o = &s.rules.ai.obstacles;
    s.rules.fortify.is_some()
        && match k {
            WorkKind::Wire => f.wire != 0,
            WorkKind::Nest => f.works != 0 && f.nest != 0,
            WorkKind::Dugout => f.works != 0 && f.dugout != 0,
            WorkKind::Roof => f.works != 0 && f.roof != 0,
            WorkKind::Wall => f.works != 0 && f.sandbags != 0,
            // S10b-ai `[ai.obstacles]` switches (spec 2).
            WorkKind::ApField => o.mines != 0,
            WorkKind::AtField => o.mines != 0 && o.at_mines != 0,
            WorkKind::AtRow2 => o.mines != 0 && o.at_mines != 0 && o.at_rows >= 2,
            WorkKind::Hedgehog | WorkKind::LogBarrier => o.obstacles != 0,
            WorkKind::Bunker => o.bunker != 0,
            // S10c-ai (spec 2): the front kinds under `front` and the
            // switch of what they lay.
            WorkKind::FrontAp => o.front != 0 && o.mines != 0,
            WorkKind::FrontWire => o.front != 0 && f.wire != 0,
            // C4: `belt` with the obstacle habit.
            WorkKind::Belt => o.front != 0 && o.belt != 0 && o.obstacles != 0,
        }
}

/// S10c-ai: whether the front rules (F1-F9) are on (`[ai.obstacles]
/// front`).
pub(super) fn front_on(s: &MatchSetup) -> bool {
    s.rules.fortify.is_some() && s.rules.ai.obstacles.front != 0
}

/// Whether any fort habit is on (`works` or `wire`, or an S10b-ai laying
/// switch).
pub(super) fn habits_on(s: &MatchSetup) -> bool {
    s10a_on(s) || s10b_on(s)
}

/// Whether an S10a fort habit is on (`works` or `wire`): the base loads
/// and the truck runs are theirs.
pub(super) fn s10a_on(s: &MatchSetup) -> bool {
    let f = &s.rules.ai.fort;
    s.rules.fortify.is_some() && (f.works != 0 || f.wire != 0)
}

/// S10b-ai: whether an `[ai.obstacles]` laying habit is on (`mines`,
/// `obstacles`, `bunker`).
pub(super) fn s10b_on(s: &MatchSetup) -> bool {
    let o = &s.rules.ai.obstacles;
    s.rules.fortify.is_some() && (o.mines != 0 || o.obstacles != 0 || o.bunker != 0)
}

/// S10b-ai: whether blueprint `d` is an S10b work (minefield, obstacle,
/// bunker): its jobs keep L1-L4, `jobs_max` and the S10b budget (R8).
pub(crate) fn s10b_fort(d: &frontline_content::setup::FortSetup) -> bool {
    d.mine_item.is_some() || d.blocks_vehicles || d.pit == Some(DigProfile::BunkerPit)
}

/// S10b-ai R7 (S10b R21): whether a squad with capability bits `caps`
/// builds blueprint `d` as a field builder only (`build_structure` on the
/// `build_heavy` bunker, at `heavy_by_field_pct`; the AI's other heavy
/// works keep `build_heavy`). Those builders come after `build_heavy` ones.
fn heavy_by_field(d: &frontline_content::setup::FortSetup, caps: u32) -> bool {
    d.pit == Some(DigProfile::BunkerPit)
        && crate::game::MatchState::heavy_def(d)
        && caps & (1u32 << d.capability) == 0
        && caps & Capability::BuildStructure.bit() != 0
}

/// How long a refusal is kept (ticks; spec 3: max(`refused_s`, 300 s)).
pub(super) fn refusal_keep_ticks(s: &MatchSetup) -> u64 {
    let hz = s.tick_hz.max(1) as u64;
    (s.rules.ai.obstacles.refused_s as u64).max(super::REFUSAL_KEEP_MIN_S) * hz
}

/// The fixed window of `refused_reorders` (s; spec 3).
pub(super) const REORDER_WINDOW_S: u64 = 300;
/// The fixed age of `jobs_overdue` (s; spec 3).
pub(super) const OVERDUE_S: u64 = 900;
/// S10b-ai Build E: the fixed age of `jobs_overdue` for a log barrier job
/// (s; the default `barrier_job_max_s`).
pub(super) const OVERDUE_BARRIER_S: u64 = 1200;

/// S10b-ai L1: the deadline (s) of a fort job on blueprint `fort`: a log
/// barrier's `barrier_job_max_s` (Build E, S10b-ai-C1), else `job_max_s`.
pub(crate) fn job_max_s(s: &MatchSetup, fort: u16) -> u64 {
    let o = &s.rules.ai.obstacles;
    if fort_of(s, WorkKind::LogBarrier) == Some(fort) {
        o.barrier_job_max_s as u64
    } else {
        o.job_max_s as u64
    }
}

/// S10c-ai F1: the `[ai.obstacles] front_points` points `me` owns nearest
/// the enemy base (straight distance, ties by index), nearest first. With
/// one own point that point is the front. Orchestrator 2026-11-16, as
/// changed for Build C (`front_home` 0, with `front` on): `me`'s home
/// point (the map point nearest its own base, ties by index) is a front
/// point only while the enemy owns more than half of the map's points
/// (`me` is pushed back); before that the front points are `me`'s other
/// own points only (none while the home point is its only one), so the
/// opening builds the home works as on main. `front_home` 1: plain F1.
/// With `front` off the home rule is not applied (the Build A
/// `front_plans` counters stay as they were). Reads state only.
pub(crate) fn front_points_of(g: &crate::game::MatchState, me: PlayerId) -> Vec<usize> {
    let s = &g.setup;
    let o = &s.rules.ai.obstacles;
    let n = o.front_points as usize;
    let Some(enemy) = (0..s.players.len() as u8).map(PlayerId).find(|&o| o != me) else {
        return Vec::new();
    };
    let eb = g.base_of(enemy);
    let mut own: Vec<(u64, usize)> = g
        .points
        .iter()
        .zip(&s.map.points)
        .enumerate()
        .filter(|(_, (c, _))| c.owner == Some(me))
        .map(|(i, (_, ps))| ((FxVec2::from(ps.pos) - eb).length_sq_raw(), i))
        .collect();
    let theirs = g.points.iter().filter(|c| c.owner == Some(enemy)).count();
    if o.front != 0 && o.front_home == 0 && theirs * 2 <= g.points.len() {
        let ob = g.base_of(me);
        let home = s
            .map
            .points
            .iter()
            .enumerate()
            .min_by_key(|(i, ps)| ((FxVec2::from(ps.pos) - ob).length_sq_raw(), *i))
            .map(|(i, _)| i);
        own.retain(|&(_, i)| Some(i) != home);
    }
    own.sort_unstable();
    own.truncate(n);
    own.into_iter().map(|(_, i)| i).collect()
}

fn near(a: FxVec2, b: FxVec2, m: Fx) -> bool {
    (a - b).length_sq_raw() <= sq_m(m)
}

/// The middle of a work's or job's line (a point site: its first point).
fn mid_of(shape: FortShape, line: &[FxVec2]) -> FxVec2 {
    match (shape, line) {
        (FortShape::Line, [a, .., b]) => (*a + *b).div_int(2),
        (_, [a, ..]) => *a,
        _ => FxVec2::ZERO,
    }
}

impl super::Planner<'_> {
    // ------------------------------------------------------------ sites --

    /// The trench line of own point `i` and the direction to the enemy
    /// (from the point to the trench middle, else the line's normal).
    fn fort_frame(&self, i: usize) -> Option<([FxVec2; 2], FxVec2, FxVec2)> {
        let t = self.ai.trenches.iter().find(|t| t.point as usize == i)?;
        let [a, b] = t.line;
        let mid = (a + b).div_int(2);
        let along = (b - a).normalize_or_zero();
        let pos: FxVec2 = self.g.setup.map.points.get(i)?.pos.into();
        let mut enemy = (mid - pos).normalize_or_zero();
        if enemy == FxVec2::ZERO {
            enemy = along.perp();
        }
        (along != FxVec2::ZERO).then_some((t.line, along, enemy))
    }

    /// The site line of work `k` at own point `i` (spec 3.3 table).
    /// `out_m` overrides the wire's distance (the 25 m retry).
    fn point_site(&self, i: usize, k: WorkKind, out_m: Option<i32>) -> Option<Vec<FxVec2>> {
        self.point_site_at(i, k, out_m, true)
    }

    /// The nearest other capture point not held by this AI (ties by
    /// index): where its squads leave point `i` for next.
    fn route_out(&self, i: usize) -> Option<FxVec2> {
        let g = self.g;
        let pts = &g.setup.map.points;
        let pos: FxVec2 = pts.get(i)?.pos.into();
        (0..pts.len())
            .filter(|&j| j != i && g.points.get(j).is_some_and(|c| c.owner != Some(self.me)))
            .map(|j| -> FxVec2 { pts[j].pos.into() })
            .min_by_key(|&p| ((p - pos).length_sq_raw(), p.x.raw(), p.y.raw()))
    }

    /// Stage D (owner "60 m lines"): how far a wire line centred at `c`
    /// (unit `along`, half length `h`) at point `i` slides along itself
    /// so it does not lie across the straight route from the point to
    /// [`Self::route_out`]: when that route crosses the line more than
    /// `SITE_NEAR_M` from its middle, the line slides so its near end
    /// stops `SITE_NEAR_M` short of the route (at most `h`). A route
    /// through the middle is the approach the wire faces: the line stays
    /// (one unbroken line, S10a-14 (A); own men go round its end).
    fn wire_shift(&self, i: usize, c: FxVec2, along: FxVec2, h: Fx) -> Fx {
        let Some(to) = self.route_out(i) else {
            return Fx::ZERO;
        };
        let Some(p) = self.g.setup.map.points.get(i) else {
            return Fx::ZERO;
        };
        let pos: FxVec2 = p.pos.into();
        let d = to - pos;
        let Some(t) = crate::fort::crossing(pos, d, c - along * h, c + along * h) else {
            return Fx::ZERO;
        };
        let x = pos + d * t;
        let at = (x - c).dot(along);
        let clear = Fx::from_int(SITE_NEAR_M);
        if at.abs() < clear {
            Fx::ZERO
        } else if at > Fx::ZERO {
            -(h + clear - at).min(h)
        } else {
            (h + clear + at).min(h)
        }
    }

    /// [`Self::point_site`]; `shifted` false: the wire line before
    /// [`Self::wire_shift`] (the exists check reads both).
    fn point_site_at(
        &self,
        i: usize,
        k: WorkKind,
        out_m: Option<i32>,
        shifted: bool,
    ) -> Option<Vec<FxVec2>> {
        let f = &self.g.setup.rules.ai.fort;
        // S10c-ai: the front kinds take their saved lane lines and the AT
        // field its vehicle-path site: none needs the trench frame (F3).
        match k {
            // F6: from the first site not yet laid (built) to the end.
            WorkKind::FrontAp | WorkKind::FrontWire => {
                let l = self.front_lane(i)?;
                let full = if k == WorkKind::FrontAp { l.ap } else { l.wire };
                return self.front_rest(k, full);
            }
            WorkKind::AtField => return self.at_site(i),
            WorkKind::AtRow2 => return self.at_row2_site(i),
            // C4: the belt row's next free hedgehog site, facing the enemy.
            WorkKind::Belt => {
                let (_, _, enemy) = self.belt_frame(i)?;
                let p = self.belt_next(i)?;
                return Some(vec![
                    p,
                    self.terrain.clamp(p + enemy * Fx::from_int(FACING_M)),
                ]);
            }
            _ => {}
        }
        let ([a, b], along, enemy) = self.fort_frame(i)?;
        let mid = (a + b).div_int(2);
        let tr = self.terrain;
        let facing = |p: FxVec2| tr.clamp(p + enemy * Fx::from_int(FACING_M));
        match k {
            WorkKind::Nest => {
                let (end, other) = if i.is_multiple_of(2) { (b, a) } else { (a, b) };
                let out = (end - other).normalize_or_zero();
                let p = tr.clamp(end + out * Fx::from_int(f.nest_offset_m as i32));
                Some(vec![p, facing(p)])
            }
            WorkKind::Dugout => {
                let p = tr.clamp(mid - enemy * Fx::from_int(f.dugout_back_m as i32));
                Some(vec![p, facing(p)])
            }
            WorkKind::Wire => {
                let out = out_m.unwrap_or(f.wire_out_m as i32);
                let mut c = mid + enemy * Fx::from_int(out);
                let h = self.wire_len().div_int(2);
                if shifted {
                    c = c + along * self.wire_shift(i, c, along, h);
                }
                let half = along * h;
                let line = vec![tr.clamp(c - half), tr.clamp(c + half)];
                ((line[1] - line[0]).length() >= Fx::from_int(2)).then_some(line)
            }
            // S10b-ai R1: parallel to the trench, beyond the wire line
            // (`ap_beyond_wire_m`), else `ap_out_m` out; the blueprint's
            // length.
            WorkKind::ApField => {
                let s = &self.g.setup;
                let o = &s.rules.ai.obstacles;
                let fd = fort_of(s, WorkKind::ApField)?;
                let out = if self.ap_after_wire() {
                    f.wire_out_m as i32 + o.ap_beyond_wire_m as i32
                } else {
                    o.ap_out_m as i32
                };
                let c = mid + enemy * Fx::from_int(out);
                let half = along * Fx::from(s.forts[fd as usize].max_length).div_int(2);
                let line = vec![tr.clamp(c - half), tr.clamp(c + half)];
                ((line[1] - line[0]).length() >= Fx::from_int(2)).then_some(line)
            }
            // R5: the row's next free site.
            WorkKind::Hedgehog => {
                let p = self.hedgehog_next(i)?;
                Some(vec![p, facing(p)])
            }
            // R7: the trench end opposite the nest (the nest's offset
            // mirrored by parity), facing the enemy.
            WorkKind::Bunker => {
                let (end, other) = if i.is_multiple_of(2) { (a, b) } else { (b, a) };
                let out = (end - other).normalize_or_zero();
                let p = tr.clamp(end + out * Fx::from_int(f.nest_offset_m as i32));
                Some(vec![p, facing(p)])
            }
            // The log barrier's site is the gun's (`start_barriers`).
            _ => None,
        }
    }

    /// S10b-ai R1: whether the AP field waits for the point's wire (the
    /// AI lays wire: `wire` on and a wire blueprint).
    fn ap_after_wire(&self) -> bool {
        let s = &self.g.setup;
        switched_on(s, WorkKind::Wire) && fort_of(s, WorkKind::Wire).is_some()
    }

    /// S10b-ai R2: the AT field site of point `i` (memoised for the plan).
    fn at_site(&self, i: usize) -> Option<Vec<FxVec2>> {
        if let Some((_, l)) = self.at_sites.borrow().iter().find(|(j, _)| *j == i) {
            return l.clone();
        }
        let l = self.at_site_search(i);
        self.at_sites.borrow_mut().push((i, l.clone()));
        l
    }

    /// R2: one vehicle path (the enemy's least slope-able armour, else any
    /// of its vehicles) from the enemy base to point `i`; the path point
    /// `at_out_m` back from the point, a blueprint-length line
    /// perpendicular to the path there, centred, clamped. None found (no
    /// path, a path shorter than `at_out_m`): no AT site.
    fn at_site_search(&self, i: usize) -> Option<Vec<FxVec2>> {
        let g = self.g;
        let s = &g.setup;
        let fd = fort_of(s, WorkKind::AtField)?;
        let pos: FxVec2 = s.map.points.get(i)?.pos.into();
        let enemy = (0..s.players.len() as u8)
            .map(PlayerId)
            .find(|&o| o != self.me)?;
        let ty = self.route_vehicle(enemy)?;
        let mut scratch = crate::path::PathScratch::default();
        let way = self.gun_route(g.base_of(enemy), pos, ty, &mut scratch)?;
        let out = Fx::from_int(s.rules.ai.obstacles.at_out_m as i32);
        let half_len = Fx::from(s.forts[fd as usize].max_length).div_int(2);
        let tr = self.terrain;
        let mut walked = Fx::ZERO;
        for pair in way.windows(2).rev() {
            let (a, b) = (pair[0], pair[1]);
            let seg = (b - a).length();
            if seg == Fx::ZERO {
                continue;
            }
            if walked + seg >= out {
                let d = (b - a).normalize_or_zero();
                // A vehicle driving the route has its hull's middle on it:
                // the row moves along itself by the track half-gauge so a
                // mine lies under a track (fix: centred, the tracks straddle
                // the middle mine and miss the next ones on the 4 m grid).
                let p = b - d * (out - walked) + d.perp() * half_gauge(ty);
                let half = d.perp() * half_len;
                let line = vec![tr.clamp(p - half), tr.clamp(p + half)];
                return ((line[1] - line[0]).length() >= Fx::from_int(2)).then_some(line);
            }
            walked += seg;
        }
        None
    }

    // ------------------------------------------------------------ front --

    /// S10c-ai F2: the saved lane of point `i` (none with `front` off).
    fn front_lane(&self, i: usize) -> Option<&FrontLane> {
        if !front_on(&self.g.setup) {
            return None;
        }
        self.ai.front_lanes.iter().find(|l| l.point as usize == i)
    }

    /// S10c-ai F6: the part of saved front line `full` (kind `k`) still to
    /// lay: from the start of its first segment whose mine site (the
    /// segment's end, as `stand_segment` lays it) has no own mine of the
    /// blueprint within `FRONT_LAID_M`, or (wire) whose middle has no own
    /// wire section of the blueprint within it, to the line's end (so
    /// `front_kind` still knows the job). The whole line when nothing is
    /// laid; None when every site is laid (or no blueprint).
    fn front_rest(&self, k: WorkKind, full: [FxVec2; 2]) -> Option<Vec<FxVec2>> {
        let s = &self.g.setup;
        let f = fort_of(s, k)?;
        let d = &s.forts[f as usize];
        let me = self.me;
        let fs = &self.g.fort;
        let tr = self.terrain;
        let r = Fx::from_int(FRONT_LAID_M);
        let seg = Fx::from(d.segment).max(Fx::from_milli(100));
        let len = (full[1] - full[0]).length();
        let n = crate::fort::segments(&full, d.segment);
        let at = |x: Fx| tr.clamp(crate::squad::point_along(&full, x.min(len)));
        let first = (0..n).find(|&j| {
            let (a, b) = (seg.mul_int(j as i32), seg.mul_int(j as i32 + 1));
            if d.mine_item.is_some() {
                let p = at(b);
                !fs.mines
                    .values()
                    .any(|m| m.owner == me && m.fort == f && near(m.pos, p, r))
            } else {
                let p = (at(a) + at(b)).div_int(2);
                !fs.wire
                    .values()
                    .any(|w| w.owner == me && w.fort == f && near((w.a + w.b).div_int(2), p, r))
            }
        })?;
        if first == 0 {
            return Some(full.to_vec());
        }
        Some(vec![at(seg.mul_int(first as i32)), full[1]])
    }

    /// S10c-ai F1 with `front` on: this player's front points (nearest the
    /// enemy base first); none with it off.
    fn front_on_points(&self) -> Vec<usize> {
        if front_on(&self.g.setup) {
            self.front_points()
        } else {
            Vec::new()
        }
    }

    /// S10c-ai F4: the order point works are visited in: the front points
    /// first (nearest the enemy base first), then the others ascending.
    /// With `front` off: 0..n (as on main).
    fn point_order(&self, n: usize) -> Vec<usize> {
        let mut v: Vec<usize> = self
            .front_on_points()
            .into_iter()
            .filter(|&i| i < n)
            .collect();
        for i in 0..n {
            if !v.contains(&i) {
                v.push(i);
            }
        }
        v
    }

    /// S10c-ai: the front kind of fort job `j`: FrontAp or FrontWire (its
    /// blueprint, and its line ending at the lane line's end: a running
    /// line keeps its end), or the AT field (either row) at a lane's point.
    fn front_kind(&self, j: &AiFortJob) -> Option<WorkKind> {
        let s = &self.g.setup;
        let lane = self.ai.front_lanes.iter().find(|l| l.point == j.point)?;
        let end = *j.line.last()?;
        let r = Fx::from_int(2);
        if fort_of(s, WorkKind::FrontAp) == Some(j.fort) && near(end, lane.ap[1], r) {
            Some(WorkKind::FrontAp)
        } else if fort_of(s, WorkKind::FrontWire) == Some(j.fort) && near(end, lane.wire[1], r) {
            Some(WorkKind::FrontWire)
        } else if fort_of(s, WorkKind::AtField) == Some(j.fort) {
            Some(WorkKind::AtField)
        } else if self.belt_job(j) {
            Some(WorkKind::Belt)
        } else {
            None
        }
    }

    /// S10c-ai: whether a live (not parked, not done) front job of kind `k`
    /// works at point `i`.
    fn front_live(&self, i: usize, k: WorkKind) -> bool {
        self.ai.forts.values().any(|j| {
            j.point as usize == i
                && !j.parked()
                && j.phase != FortPhase::Done
                && self.front_kind(j) == Some(k)
        })
    }

    /// S10c-ai F7: saved lanes that hold a front field (`ap_done`) or a
    /// live FrontAp job (counted from `front_lanes`, not `fields_of`).
    fn front_fields(&self) -> u32 {
        self.ai
            .front_lanes
            .iter()
            .filter(|l| l.ap_done || self.front_live(l.point as usize, WorkKind::FrontAp))
            .count() as u32
    }

    /// S10c-ai C8: whether fewer than `front_slots` front jobs (FrontAp,
    /// FrontWire, an AT row at a lane's point; not parked, not done) run,
    /// so a new one may start and one builder slot stays for the home
    /// works.
    fn front_slot_free(&self) -> bool {
        let n = self
            .ai
            .forts
            .values()
            .filter(|j| !j.parked() && j.phase != FortPhase::Done && self.front_kind(j).is_some())
            .count();
        n < self.g.setup.rules.ai.obstacles.front_slots as usize
    }

    /// S10c-ai C2 (`mine_assign` 1): the free squad (not in `taken` or
    /// `out`) from anywhere that can take front work `fort` on `line`
    /// (F5's threat, F10's carry rule), nearest the site first, then id;
    /// independent of the point assignment (as `bunker_assign`).
    #[allow(clippy::too_many_arguments)]
    fn mine_assign_pick<'m>(
        &self,
        mine: &'m [Mine],
        fort: u16,
        line: &[FxVec2],
        known: &[Known],
        kv: &[KnownVehicle],
        taken: &[SquadId],
        out: &[SquadId],
    ) -> Option<&'m Mine> {
        let s = &self.g.setup;
        if s.rules.ai.obstacles.mine_assign == 0 {
            return None;
        }
        let d = &s.forts[fort as usize];
        let site = mid_of(d.shape, line);
        mine.iter()
            .filter(|m| {
                !taken.contains(&m.id)
                    && !out.contains(&m.id)
                    && self.can_take_at(m, fort, site, known, kv, true)
                    && self.front_carry_ok(m.id, fort, line)
            })
            .min_by_key(|m| {
                (
                    self.ai.forts.contains_key(&m.id),
                    heavy_by_field(d, m.caps),
                    (m.lead - site).length_sq_raw(),
                    m.id,
                )
            })
    }

    /// S10c-ai F2: keeps `AiState::front_lanes` (every plan, `front` on):
    /// an entry whose point is no longer an own front point and holds no
    /// live front job goes; on a `recheck_s` plan each front point without
    /// one gets its lane ([`Self::front_lane_search`]), ascending by point.
    fn front_lanes_update(&mut self) {
        if !front_on(&self.g.setup) {
            return;
        }
        let front = self.front_points();
        let keep: Vec<bool> = self
            .ai
            .front_lanes
            .iter()
            .map(|l| {
                front.contains(&(l.point as usize))
                    || self.ai.forts.values().any(|j| {
                        j.point == l.point
                            && !j.parked()
                            && j.phase != FortPhase::Done
                            && self.front_kind(j).is_some()
                    })
            })
            .collect();
        let mut k = keep.into_iter();
        self.ai.front_lanes.retain(|_| k.next().unwrap_or(false));
        // F6: a line whose every site is laid (built) with no live job of
        // its kind is done (a job dropped after its last mine).
        let done: Vec<(bool, bool)> = self
            .ai
            .front_lanes
            .iter()
            .map(|l| {
                let i = l.point as usize;
                let full = |k: WorkKind, line| {
                    fort_of(&self.g.setup, k).is_some()
                        && !self.front_live(i, k)
                        && self.front_rest(k, line).is_none()
                };
                (
                    full(WorkKind::FrontAp, l.ap),
                    full(WorkKind::FrontWire, l.wire),
                )
            })
            .collect();
        for (l, (ap, wire)) in self.ai.front_lanes.iter_mut().zip(done) {
            l.ap_done |= ap;
            l.wire_done |= wire;
        }
        if !self.recheck_plan() {
            return;
        }
        for i in front {
            if self.ai.front_lanes.iter().any(|l| l.point as usize == i) {
                continue;
            }
            if self.ai.front_lanes.len() >= MAX_FRONT_LANES {
                break;
            }
            let Some(lane) = self.front_lane_search(i) else {
                continue;
            };
            let at = self
                .ai
                .front_lanes
                .iter()
                .position(|l| l.point > lane.point)
                .unwrap_or(self.ai.front_lanes.len());
            self.ai.front_lanes.insert(at, lane);
        }
    }

    /// S10c-ai F2, F3: point `i`'s lane: one foot path (as `breach_path`,
    /// wire cost off) from the enemy-owned map point nearest `i` (ties by
    /// position; none: the enemy base) to the point, and its two site
    /// lines ([`Self::lane_line`]): the wire `front_out_m` back from the
    /// point (`front_wire_len_m` long), the AP field `ap_beyond_wire_m`
    /// beyond it (the blueprint's length). None: no path, or a lane
    /// shorter than a line's distance.
    fn front_lane_search(&self, i: usize) -> Option<FrontLane> {
        let g = self.g;
        let s = &g.setup;
        let o = &s.rules.ai.obstacles;
        let pts = &s.map.points;
        let pos: FxVec2 = pts.get(i)?.pos.into();
        let enemy = (0..s.players.len() as u8)
            .map(PlayerId)
            .find(|&p| p != self.me)?;
        let from = (0..pts.len())
            .filter(|&j| j != i && g.points.get(j).is_some_and(|c| c.owner == Some(enemy)))
            .map(|j| -> FxVec2 { pts[j].pos.into() })
            .min_by_key(|&p| ((p - pos).length_sq_raw(), p.x.raw(), p.y.raw()))
            .unwrap_or_else(|| g.base_of(enemy));
        let mv = &s.rules.movement;
        let cost = crate::path::PathCost {
            slope_cost_pct: mv.slope_cost_pct,
            max_slope_pct: mv.max_slope_pct,
            trench_cost_pct: mv.trench_cost_pct,
            ditch_cost_pct: mv.ditch_cost_pct,
        };
        let ditches = (!g.at_ditches.is_empty()).then_some(&g.vderived.block);
        let mut scratch = crate::path::PathScratch::default();
        let r = crate::path::find_foot_path(
            self.terrain,
            from,
            pos,
            &cost,
            ditches,
            None,
            None,
            mv.path_max_expansions,
            &mut scratch,
        );
        if !r.reached {
            return None;
        }
        let mut way = Vec::with_capacity(r.points.len() + 1);
        way.push(from);
        way.extend(r.points);
        let out_w = o.front_out_m as i32;
        let out_a = out_w + o.ap_beyond_wire_m as i32;
        let wire_len = Fx::from_int(o.front_wire_len_m as i32);
        let ap_len = fort_of(s, WorkKind::FrontAp)
            .map_or(wire_len, |f| Fx::from(s.forts[f as usize].max_length));
        Some(FrontLane {
            point: i as u8,
            since: self.tick,
            ap: self.lane_line(&way, out_a, ap_len)?,
            wire: self.lane_line(&way, out_w, wire_len)?,
            ap_done: false,
            wire_done: false,
        })
    }

    /// S10c-ai F3: the line `len` long across lane `way` at the lane point
    /// `out` m back from its end (the `at_site_search` walk), perpendicular
    /// to the lane's leg there, centred, clamped, at least 2 m; no
    /// `wire_shift`. None: a lane shorter than `out`.
    fn lane_line(&self, way: &[FxVec2], out: i32, len: Fx) -> Option<[FxVec2; 2]> {
        let out = Fx::from_int(out);
        let half_len = len.div_int(2);
        let tr = self.terrain;
        let mut walked = Fx::ZERO;
        for pair in way.windows(2).rev() {
            let (a, b) = (pair[0], pair[1]);
            let seg = (b - a).length();
            if seg == Fx::ZERO {
                continue;
            }
            if walked + seg >= out {
                let d = (b - a).normalize_or_zero();
                let p = b - d * (out - walked);
                let half = d.perp() * half_len;
                let line = [tr.clamp(p - half), tr.clamp(p + half)];
                return ((line[1] - line[0]).length() >= Fx::from_int(2)).then_some(line);
            }
            walked += seg;
        }
        None
    }

    /// S10c-ai F3, F7: the front works point `i` still wants, in build
    /// order: FrontAp (its lane's `ap_done` false, no live FrontAp job,
    /// fewer than `front_fields_max` lanes with a field, the blueprint
    /// cap), FrontWire (the same with `wire_done`), then the R2 AT field
    /// (its trench condition lifted; `recheck_s` plans only, `at_fields_max`
    /// and the blueprint cap) and its second row. Only at a front point
    /// with a saved lane and `front` on.
    fn front_wants(&self, i: usize) -> Vec<WorkKind> {
        let s = &self.g.setup;
        let o = &s.rules.ai.obstacles;
        let mut out = Vec::new();
        if !self.front_on_points().contains(&i) {
            return out;
        }
        let Some(lane) = self.front_lane(i) else {
            return out;
        };
        let (ap_done, wire_done) = (lane.ap_done, lane.wire_done);
        for k in [WorkKind::FrontAp, WorkKind::FrontWire] {
            let done = if k == WorkKind::FrontAp {
                ap_done
            } else {
                wire_done
            };
            if !switched_on(s, k) || done || self.front_live(i, k) {
                continue;
            }
            let (Some(f), Some(line)) = (fort_of(s, k), self.point_site(i, k, None)) else {
                continue;
            };
            if k == WorkKind::FrontAp && self.front_fields() >= o.front_fields_max {
                continue;
            }
            if self.under_limit(f, &line) {
                out.push(k);
            }
        }
        if switched_on(s, WorkKind::AtField)
            && self.recheck_plan()
            && let Some(f) = fort_of(s, WorkKind::AtField)
            && self.field_has(i, WorkKind::AtField, f) == Some(false)
            && self.fields_of(f) < o.at_fields_max
            && self
                .at_site(i)
                .is_some_and(|line| self.under_limit(f, &line))
        {
            out.push(WorkKind::AtField);
        }
        if self.at_row2_wanted(i) {
            out.push(WorkKind::AtRow2);
        }
        // C4: the belt once the AT row 1 is laid.
        if self.belt_wanted(i) {
            out.push(WorkKind::Belt);
        }
        out
    }

    /// S10c-ai F4: what point `i` wants, in order: its front wants
    /// ([`Self::front_wants`]) first, then (a finished trench only) the
    /// S10a and S10b wants of [`Self::point_wants`] not already listed.
    /// With `front` off: `point_wants` at a trenched point, else nothing.
    fn wants_at(&self, i: usize, trench_done: bool) -> Vec<WorkKind> {
        let mut out = self.front_wants(i);
        if trench_done {
            for k in self.point_wants(i) {
                if !out.contains(&k) {
                    out.push(k);
                }
            }
        }
        out
    }

    /// S10c-ai F10: whether builder `sq` may take a front job on `line` of
    /// blueprint `f`: it carries the whole work, or stands in the own base
    /// (it loads there), or a truck brings it (`truck_for`).
    fn front_carry_ok(&self, sq: SquadId, f: u16, line: &[FxVec2]) -> bool {
        let d = &self.g.setup.forts[f as usize];
        // A point work (a belt hedgehog, C4) is one step.
        let steps = match d.shape {
            FortShape::Line => crate::fort::segments(line, d.segment),
            _ => 1,
        };
        let need = d.materials_g.saturating_mul(steps);
        self.carried_g(sq) >= need
            || self.g.squads.get(&sq).is_some_and(|q| self.in_own_base(q))
            || self.truck_for(f)
    }

    /// Whether `k` is a front kind at point `i` (FrontAp, FrontWire, or an
    /// AT row at a front point with a saved lane).
    fn front_work(&self, i: usize, k: WorkKind) -> bool {
        match k {
            WorkKind::FrontAp | WorkKind::FrontWire | WorkKind::Belt => true,
            WorkKind::AtField | WorkKind::AtRow2 => {
                self.front_lane(i).is_some() && self.front_on_points().contains(&i)
            }
            _ => false,
        }
    }

    // ------------------------------------------------------------- belt --

    /// S10c-ai C4: whether belts are on (`front`, `belt`, `obstacles`).
    fn belt_on(&self) -> bool {
        let s = &self.g.setup;
        front_on(s) && switched_on(s, WorkKind::Belt)
    }

    /// C4: the frame of point `i`'s belt, read from its laid AT field (own
    /// AT mines within the field radius of [`Self::at_row1_laid`]), not
    /// from the enemy's vehicle path, which bends round the belt once it
    /// stands: (centre, along, enemy). `along` runs between the field's
    /// two farthest-apart mines (positions sorted, the first pair kept on
    /// ties); `enemy` is its normal away from the point; the centre lies on
    /// the middle of that pair (the field is centred on the lane, within
    /// the track half-gauge), `belt_gap_m` beyond the field's enemy-side
    /// edge. None: belts off, or fewer than two such mines.
    fn belt_frame(&self, i: usize) -> Option<(FxVec2, FxVec2, FxVec2)> {
        if !self.belt_on() {
            return None;
        }
        let g = self.g;
        let s = &g.setup;
        let f = fort_of(s, WorkKind::AtField)?;
        let pos: FxVec2 = s.map.points.get(i)?.pos.into();
        let d = &s.forts[f as usize];
        let r = Fx::from(d.max_length).div_int(2)
            + Fx::from_int(SITE_NEAR_M)
            + Fx::from_int(s.rules.ai.obstacles.at_out_m as i32);
        let me = self.me;
        let mut ms: Vec<FxVec2> = g
            .fort
            .mines
            .values()
            .filter(|m| m.owner == me && m.fort == f && near(m.pos, pos, r))
            .map(|m| m.pos)
            .collect();
        ms.sort_unstable_by_key(|p| (p.x.raw(), p.y.raw()));
        let mut best: Option<(u64, FxVec2, FxVec2)> = None;
        for (k, &a) in ms.iter().enumerate() {
            for &b in &ms[k + 1..] {
                let l = (b - a).length_sq_raw();
                if best.is_none_or(|(m, _, _)| l > m) {
                    best = Some((l, a, b));
                }
            }
        }
        let (_, a, b) = best?;
        let along = (b - a).normalize_or_zero();
        if along == FxVec2::ZERO {
            return None;
        }
        let mid = (a + b).div_int(2);
        let mut enemy = along.perp();
        if enemy.dot(mid - pos) < Fx::ZERO {
            enemy = -enemy;
        }
        let edge = ms
            .iter()
            .map(|&m| (m - mid).dot(enemy))
            .max()
            .unwrap_or(Fx::ZERO)
            .max(Fx::ZERO);
        let gap = Fx::from_int(s.rules.ai.obstacles.belt_gap_m as i32);
        Some((mid + enemy * (edge + gap), along, enemy))
    }

    /// C4: the `belt_pieces` hedgehog sites of point `i`'s belt,
    /// `hedgehog_step_m` apart along the field's line, centred on the
    /// frame's centre; `side` 1 (C5's other side) moved along itself by
    /// its own length (`belt_pieces` steps), so the sides share no site.
    fn belt_row(&self, i: usize, side: usize) -> Option<Vec<FxVec2>> {
        let o = &self.g.setup.rules.ai.obstacles;
        let n = o.belt_pieces as i32;
        if n == 0 {
            return None;
        }
        let (c, along, _) = self.belt_frame(i)?;
        let step = Fx::from_int(o.hedgehog_step_m as i32);
        let shift = step.mul_int(n * side.min(1) as i32);
        Some(
            (0..n)
                .map(|k| {
                    let at = step.mul_int(2 * k - (n - 1)).div_int(2) + shift;
                    self.terrain.clamp(c + along * at)
                })
                .collect(),
        )
    }

    /// C4 (S10c-4 (A)): the belt as one log barrier line: `belt_pieces` x
    /// `hedgehog_step_m` long (at most the blueprint's `max_length`),
    /// centred where [`Self::belt_row`]'s side `side` is.
    fn belt_log(&self, i: usize, side: usize) -> Option<Vec<FxVec2>> {
        let s = &self.g.setup;
        let o = &s.rules.ai.obstacles;
        let lf = fort_of(s, WorkKind::LogBarrier)?;
        let (c, along, _) = self.belt_frame(i)?;
        let step = Fx::from_int(o.hedgehog_step_m as i32);
        let n = o.belt_pieces as i32;
        let len = step
            .mul_int(n)
            .min(Fx::from(s.forts[lf as usize].max_length));
        let c = c + along * step.mul_int(n * side.min(1) as i32);
        let half = along * len.div_int(2);
        let line = vec![self.terrain.clamp(c - half), self.terrain.clamp(c + half)];
        ((line[1] - line[0]).length() >= Fx::from_int(2)).then_some(line)
    }

    /// C4 "one belt per lane", C5 "the side taken": the side of point
    /// `i`'s belt that holds an own hedgehog (on a row site) or log barrier
    /// (near its line's middle) work, piece or job, side 0 first, and which
    /// of the two it is. None: no belt yet.
    fn belt_side(&self, i: usize) -> Option<(usize, WorkKind)> {
        let s = &self.g.setup;
        let hh = fort_of(s, WorkKind::Hedgehog);
        let lf = fort_of(s, WorkKind::LogBarrier);
        let r = self.hedgehog_r();
        (0..2).find_map(|side| {
            if let Some(f) = hh
                && self
                    .belt_row(i, side)
                    .is_some_and(|row| row.iter().any(|&p| self.fort_near(f, p, r)))
            {
                return Some((side, WorkKind::Hedgehog));
            }
            let f = lf?;
            let line = self.belt_log(i, side)?;
            let half = (line[1] - line[0]).length().div_int(2);
            self.fort_near(f, mid_of(FortShape::Line, &line), half)
                .then_some((side, WorkKind::LogBarrier))
        })
    }

    /// C4: the first free hedgehog site of point `i`'s belt row (its taken
    /// side, else side 0); None when a log barrier holds the belt or the
    /// row is full.
    fn belt_next(&self, i: usize) -> Option<FxVec2> {
        let f = fort_of(&self.g.setup, WorkKind::Hedgehog)?;
        let side = match self.belt_side(i) {
            Some((_, WorkKind::LogBarrier)) => return None,
            Some((side, _)) => side,
            None => 0,
        };
        let r = self.hedgehog_r();
        self.belt_row(i, side)?
            .into_iter()
            .find(|&p| !self.fort_near(f, p, r))
    }

    /// C4: whether point `i` wants (the next piece of) its belt: belts on,
    /// its AT row 1 laid (no AT job of the point running), no belt job
    /// live there, and a free hedgehog site left (no log barrier belt).
    fn belt_wanted(&self, i: usize) -> bool {
        let s = &self.g.setup;
        self.belt_on()
            && fort_of(s, WorkKind::AtField).is_some_and(|f| self.at_row1_laid(i, f))
            && !self.front_live(i, WorkKind::Belt)
            && self.belt_next(i).is_some()
    }

    /// C4: whether fort job `j` builds a belt: belts on, a hedgehog or log
    /// barrier job of a point with a saved lane whose site lies within one
    /// `hedgehog_step_m` of either side's row.
    fn belt_job(&self, j: &AiFortJob) -> bool {
        let s = &self.g.setup;
        if !self.belt_on() || j.point == NO_POINT {
            return false;
        }
        let Some(d) = s.forts.get(j.fort as usize) else {
            return false;
        };
        if fort_of(s, WorkKind::Hedgehog) != Some(j.fort)
            && fort_of(s, WorkKind::LogBarrier) != Some(j.fort)
        {
            return false;
        }
        let site = mid_of(d.shape, &j.line);
        let r = Fx::from_int(s.rules.ai.obstacles.hedgehog_step_m as i32).max(Fx::ONE);
        (0..2).any(|side| {
            self.belt_row(j.point as usize, side)
                .is_some_and(|row| crate::fort::near_segment(site, row[0], row[row.len() - 1], r))
        })
    }

    /// C5: the own route of point `i` ([`Self::own_route`]'s legs) on block
    /// layer `block`, its length; None when the leg to the point, or the
    /// leg on to [`Self::route_out`] (when there is one), is not found.
    fn route_len_on(&self, i: usize, block: &crate::vehicle::BlockLayer) -> Option<Fx> {
        let g = self.g;
        let pos: FxVec2 = g.setup.map.points.get(i)?.pos.into();
        let ty = self.route_vehicle(self.me)?;
        let mut scratch = crate::path::PathScratch::default();
        let mut way = self.gun_route_on(g.base_of(self.me), pos, ty, block, &mut scratch)?;
        if let Some(to) = self.route_out(i) {
            let more = self.gun_route_on(pos, to, ty, block, &mut scratch)?;
            way.extend(more.into_iter().skip(1));
        }
        Some(
            way.windows(2)
                .fold(Fx::ZERO, |acc, w| acc + (w[1] - w[0]).length()),
        )
    }

    /// C5 (replaces R6's 10 m for belts): footprint 0 when the own route
    /// of point `i` with its nodes blocked (sampled every metre along it)
    /// exists and is at most `belt_detour_pct` of the route without it;
    /// else footprint 1 the same; else an L3 refusal at footprint 0's
    /// middle, `belt_route_rejects` += 1 (None). `belt_detour_pct` 0: the
    /// rule is off (footprint 0). No own route at all: nothing to keep
    /// clear (as R6).
    fn belt_route_side(&mut self, i: usize, feet: [Option<Vec<FxVec2>>; 2]) -> Option<usize> {
        let s = &self.g.setup;
        let first = feet[0].clone()?;
        let pct = s.rules.ai.obstacles.belt_detour_pct;
        if pct == 0 {
            return Some(0);
        }
        let Some(ty) = self.route_vehicle(self.me) else {
            return Some(0);
        };
        let layer = self.g.vderived.block_of(ty);
        let Some(base) = self.route_len_on(i, layer) else {
            return Some(0);
        };
        for (k, f) in feet.iter().enumerate() {
            let Some(f) = f else { continue };
            let (a, b) = (f[0], f[f.len() - 1]);
            let n = (b - a).length().floor_int().max(1);
            let pts: Vec<FxVec2> = (0..=n)
                .map(|t| a + ((b - a) * Fx::from_int(t)).div_int(n))
                .collect();
            let blocked = layer.with_pieces_at(self.terrain, &pts);
            if let Some(len) = self.route_len_on(i, &blocked)
                && len.raw() as i128 * 100 <= base.raw() as i128 * pct as i128
            {
                return Some(k);
            }
        }
        let at = (first[0] + first[first.len() - 1]).div_int(2);
        self.ai.refuse(JobKind::Fort, at, self.tick);
        self.ai.counters.belt_route_rejects += 1;
        self.ai.counters.route_refusals += 1;
        self.ai.counters.s10b_refused += 1;
        None
    }

    /// The vehicle type whose routes player `p`'s armour drives: its
    /// roster's least slope-able armour, else any of its vehicles.
    fn route_vehicle(&self, p: PlayerId) -> Option<&frontline_content::vehicle::VehicleSetup> {
        let s = &self.g.setup;
        let roster = &s.factions[s.players.get(p.0 as usize)?.faction as usize].roster;
        let vehicles: Vec<&frontline_content::vehicle::VehicleSetup> = roster
            .iter()
            .filter_map(|&t| s.vehicles.get(s.squad_types[t as usize].vehicle? as usize))
            .collect();
        vehicles
            .iter()
            .filter(|v| super::guns::is_armour(v.role))
            .min_by_key(|v| v.mobility.max_slope_pct)
            .or_else(|| vehicles.iter().min_by_key(|v| v.mobility.max_slope_pct))
            .copied()
    }

    /// S10b-ai R6: the own vehicle route of point `i` (memoised for the
    /// plan): own base -> the point -> [`Self::route_out`] (two searches).
    /// None: no own vehicle type or no route (nothing to keep clear).
    fn own_route(&self, i: usize) -> Option<Vec<FxVec2>> {
        if let Some((_, r)) = self.own_routes.borrow().iter().find(|(j, _)| *j == i) {
            return r.clone();
        }
        let r = self.own_route_search(i);
        self.own_routes.borrow_mut().push((i, r.clone()));
        r
    }

    fn own_route_search(&self, i: usize) -> Option<Vec<FxVec2>> {
        let g = self.g;
        let pos: FxVec2 = g.setup.map.points.get(i)?.pos.into();
        let ty = self.route_vehicle(self.me)?;
        let mut scratch = crate::path::PathScratch::default();
        let mut way = self.gun_route(g.base_of(self.me), pos, ty, &mut scratch)?;
        if let Some(more) = self
            .route_out(i)
            .and_then(|to| self.gun_route(pos, to, ty, &mut scratch))
        {
            way.extend(more.into_iter().skip(1));
        }
        Some(way)
    }

    /// R6: whether footprint `foot` (a line, or one point) plus
    /// `wreck_pad_m` lies within `own_route_m` of point `i`'s own route.
    /// `own_route_m` 0: the rule is off (the spec's control).
    fn on_own_route(&self, i: usize, foot: &[FxVec2]) -> bool {
        let s = &self.g.setup;
        let m = s.rules.ai.obstacles.own_route_m;
        if m == 0 || foot.is_empty() {
            return false;
        }
        let Some(way) = self.own_route(i) else {
            return false;
        };
        let r = Fx::from_int(m as i32 + s.rules.vehicles.path.wreck_pad_m as i32);
        let ns = crate::fort::near_segment;
        let (c, d) = (foot[0], foot[foot.len() - 1]);
        way.windows(2).any(|w| {
            let (a, b) = (w[0], w[1]);
            ns(c, a, b, r)
                || ns(d, a, b, r)
                || ns(a, c, d, r)
                || ns(b, c, d, r)
                || crate::fort::crossing(a, b - a, c, d).is_some()
        })
    }

    /// R6 at site choice: footprint 0, else the mirrored footprint 1 once;
    /// else an L3 refusal at footprint 0's middle, `own_route_rejects` += 1
    /// (None).
    fn own_route_side(&mut self, i: usize, feet: [Option<Vec<FxVec2>>; 2]) -> Option<usize> {
        let first = feet[0].clone()?;
        for (k, f) in feet.iter().enumerate() {
            if let Some(f) = f
                && !self.on_own_route(i, f)
            {
                return Some(k);
            }
        }
        let at = (first[0] + first[first.len() - 1]).div_int(2);
        self.ai.refuse(JobKind::Fort, at, self.tick);
        self.ai.counters.own_route_rejects += 1;
        self.ai.counters.route_refusals += 1;
        self.ai.counters.s10b_refused += 1;
        None
    }

    /// The map point nearest `at` (ties by index).
    fn point_near(&self, at: FxVec2) -> Option<usize> {
        let pts = &self.g.setup.map.points;
        (0..pts.len()).min_by_key(|&j| {
            let p: FxVec2 = pts[j].pos.into();
            ((p - at).length_sq_raw(), j)
        })
    }

    /// S10b-ai R5: the log barrier before a gun at `site` facing `facing`:
    /// the blueprint length (at most `BARRIER_LEN_M`) across the arc
    /// `obst_gun_m` out; `side` 0 centred, -1/+1 moved along itself by its
    /// length (R6's mirrored site).
    fn barrier_line(
        &self,
        fort: u16,
        site: FxVec2,
        facing: FxVec2,
        side: i32,
    ) -> Option<Vec<FxVec2>> {
        let s = &self.g.setup;
        let fwd = (facing - site).normalize_or_zero();
        if fwd == FxVec2::ZERO {
            return None;
        }
        let len = Fx::from(s.forts[fort as usize].max_length).min(Fx::from_int(BARRIER_LEN_M));
        let c = site
            + fwd * Fx::from_int(s.rules.ai.obstacles.obst_gun_m as i32)
            + fwd.perp() * len.mul_int(side);
        let half = fwd.perp() * len.div_int(2);
        let line = vec![self.terrain.clamp(c - half), self.terrain.clamp(c + half)];
        ((line[1] - line[0]).length() >= Fx::from_int(2)).then_some(line)
    }

    /// R5: what point `i`'s hedgehog row stands in front of: its built
    /// nest, else an own built bunker near the point.
    fn hedgehog_anchor(&self, i: usize) -> Option<FxVec2> {
        let s = &self.g.setup;
        if let (Some(f), Some(site)) = (
            fort_of(s, WorkKind::Nest),
            self.point_site(i, WorkKind::Nest, None),
        ) && self.built_near(f, site[0])
        {
            return Some(site[0]);
        }
        self.bunker_built(i)
    }

    /// The centre of an own built bunker within `BUNKER_NEAR_M` of point
    /// `i` (its roof piece; the nearest, ties by position).
    fn bunker_built(&self, i: usize) -> Option<FxVec2> {
        let s = &self.g.setup;
        let b = fort_of(s, WorkKind::Bunker)?;
        let pos: FxVec2 = s.map.points.get(i)?.pos.into();
        let me = self.me;
        self.g
            .fort
            .pieces
            .values()
            .filter(|q| {
                q.owner == me
                    && q.fort == b
                    && q.part == 0
                    && near(q.pos, pos, Fx::from_int(BUNKER_NEAR_M))
            })
            .map(|q| q.pos)
            .min_by_key(|&p| ((p - pos).length_sq_raw(), p.x.raw(), p.y.raw()))
    }

    /// R7 manning (for `order_infantry`): the built own bunker at own point
    /// `i`; None with `bunker` off.
    pub(super) fn bunker_at(&self, i: usize) -> Option<FxVec2> {
        let s = &self.g.setup;
        if !switched_on(s, WorkKind::Bunker) {
            return None;
        }
        self.bunker_built(i)
    }

    /// R7 manning: the built own bunker at own point `i` and the squad of
    /// `free` (the point's free squads) that defends from it: the MG team
    /// (lowest id), else the squad nearest the bunker that is no MG or AT
    /// crew. None with `bunker` off or no bunker.
    pub(super) fn bunker_crew<'m>(
        &self,
        i: usize,
        free: impl Iterator<Item = &'m Mine> + Clone,
    ) -> Option<(FxVec2, SquadId)> {
        let at = self.bunker_at(i)?;
        free.clone()
            .filter(|m| m.role == super::AiKind::Mg)
            .min_by_key(|m| m.id)
            .or_else(|| {
                free.filter(|m| !matches!(m.role, super::AiKind::Mg | super::AiKind::AntiTank))
                    .min_by_key(|m| ((m.lead - at).length_sq_raw(), m.id))
            })
            .map(|m| (at, m.id))
    }

    /// R7: the own point with an AI trench nearest an enemy base (ties by
    /// index): the one point that gets the bunker.
    fn bunker_point(&self) -> Option<usize> {
        let g = self.g;
        let s = &g.setup;
        let bases: Vec<FxVec2> = (0..s.players.len() as u8)
            .map(PlayerId)
            .filter(|&o| o != self.me)
            .map(|o| g.base_of(o))
            .collect();
        (0..s.map.points.len())
            .filter(|&j| {
                g.points.get(j).is_some_and(|c| c.owner == Some(self.me))
                    && self.ai.trenches.iter().any(|t| t.point as usize == j)
            })
            .filter_map(|j| {
                let pos: FxVec2 = s.map.points[j].pos.into();
                let d = bases.iter().map(|&b| (b - pos).length_sq_raw()).min()?;
                Some((d, j))
            })
            .min()
            .map(|(_, j)| j)
    }

    /// R7: the bunker point `i` still wants: it is the bunker point, the
    /// player has fewer than `bunkers_max` (built, building or a job).
    fn bunker_wants(&self, i: usize) -> Vec<WorkKind> {
        let s = &self.g.setup;
        let k = WorkKind::Bunker;
        let ok = switched_on(s, k)
            && fort_of(s, k).is_some_and(|b| self.fort_use(b) < s.rules.ai.obstacles.bunkers_max)
            && self.bunker_point() == Some(i)
            && self.point_has(i, k) == Some(false);
        if ok { vec![k] } else { Vec::new() }
    }

    /// R5: the `hedgehogs_row` sites of point `i`'s row, `hedgehog_step_m`
    /// apart, starting `obst_work_m` in front of the anchor and running
    /// along the trench away from its middle (`side` 1: mirrored, toward it,
    /// from one step beyond side 0's first site: the sides share no site).
    fn hedgehog_row(&self, i: usize, side: usize) -> Option<Vec<FxVec2>> {
        let o = &self.g.setup.rules.ai.obstacles;
        let ([a, b], along, enemy) = self.fort_frame(i)?;
        let anchor = self.hedgehog_anchor(i)?;
        let mid = (a + b).div_int(2);
        let out = (anchor - mid).dot(along) >= Fx::ZERO;
        let dir = if out == (side == 0) {
            along
        } else {
            FxVec2::ZERO - along
        };
        let first = anchor + enemy * Fx::from_int(o.obst_work_m as i32);
        let step = Fx::from_int(o.hedgehog_step_m as i32);
        // Side 1 starts one step beyond side 0's first site: the two rows
        // share no site, so `hedgehog_side` reads the side R6 chose.
        let k0 = side.min(1) as i32;
        Some(
            (0..o.hedgehogs_row as i32)
                .map(|k| self.terrain.clamp(first + dir * step.mul_int(k + k0)))
                .collect(),
        )
    }

    /// How near an own hedgehog (work, piece or job) takes a row site.
    fn hedgehog_r(&self) -> Fx {
        Fx::from_int((self.g.setup.rules.ai.obstacles.hedgehog_step_m as i32 / 2).max(1))
    }

    /// R5 "sites fixed at the first start": the side of point `i`'s row
    /// that has a hedgehog already (side 0 first); None: no row yet.
    fn hedgehog_side(&self, i: usize) -> Option<usize> {
        let f = fort_of(&self.g.setup, WorkKind::Hedgehog)?;
        let r = self.hedgehog_r();
        (0..2).find(|&side| {
            self.hedgehog_row(i, side)
                .is_some_and(|row| row.iter().any(|&p| self.fort_near(f, p, r)))
        })
    }

    /// R5: the first free site of point `i`'s row (its fixed side, else
    /// side 0), None when the row is full or has no anchor.
    fn hedgehog_next(&self, i: usize) -> Option<FxVec2> {
        let f = fort_of(&self.g.setup, WorkKind::Hedgehog)?;
        let r = self.hedgehog_r();
        let row = self.hedgehog_row(i, self.hedgehog_side(i).unwrap_or(0))?;
        row.into_iter().find(|&p| !self.fort_near(f, p, r))
    }

    /// R5: the obstacles point `i` still wants (its hedgehog row).
    fn obstacle_wants(&self, i: usize) -> Vec<WorkKind> {
        let s = &self.g.setup;
        let k = WorkKind::Hedgehog;
        let ok = switched_on(s, k)
            && s.rules.ai.obstacles.hedgehogs_row > 0
            && fort_of(s, k).is_some()
            && self.hedgehog_next(i).is_some();
        if ok { vec![k] } else { Vec::new() }
    }

    /// The AI's wire line length: `wire_len_m`, at most the blueprint's
    /// `max_length` (a longer line is always refused, fix F4).
    fn wire_len(&self) -> Fx {
        let s = &self.g.setup;
        let want = Fx::from_int(s.rules.ai.fort.wire_len_m as i32);
        fort_of(s, WorkKind::Wire)
            .map_or(want, |w| want.min(Fx::from(s.forts[w as usize].max_length)))
    }

    /// Fix F4 (derived): blueprint `f`'s per-player use (the mechanics'
    /// count) plus this AI's jobs not yet a work, in segments for a line.
    fn fort_use(&self, f: u16) -> u32 {
        let d = &self.g.setup.forts[f as usize];
        let jobs: u32 = self
            .ai
            .forts
            .values()
            .filter(|j| j.fort == f && j.work.is_none() && !j.parked())
            .map(|j| match d.shape {
                FortShape::Line => crate::fort::segments(&j.line, d.segment),
                _ => 1,
            })
            .sum();
        self.g.fort_used(self.me, f).saturating_add(jobs)
    }

    /// Fix F4: whether a work of blueprint `f` on `line` fits under the
    /// blueprint's `limit_per_player` (else the mechanics refuse it).
    fn under_limit(&self, f: u16, line: &[FxVec2]) -> bool {
        let d = &self.g.setup.forts[f as usize];
        let need = match d.shape {
            FortShape::Line => crate::fort::segments(line, d.segment),
            _ => 1,
        };
        self.fort_use(f).saturating_add(need) <= d.limit_per_player
    }

    /// Whether an own work, piece, wire section or fort job of blueprint
    /// `f` lies within `r` of `at`.
    fn fort_near(&self, f: u16, at: FxVec2, r: Fx) -> bool {
        let g = self.g;
        let me = self.me;
        let fs: &FortState = &g.fort;
        let shape = g.setup.forts[f as usize].shape;
        fs.works
            .values()
            .any(|w| w.owner == me && w.fort == f && near(mid_of(shape, &w.line), at, r))
            || fs
                .pieces
                .values()
                .any(|q| q.owner == me && q.fort == f && q.part == 0 && near(q.pos, at, r))
            || fs
                .wire
                .values()
                .any(|s| s.owner == me && s.fort == f && near((s.a + s.b).div_int(2), at, r))
            || fs
                .mines
                .values()
                .any(|m| m.owner == me && m.fort == f && near(m.pos, at, r))
            || self
                .ai
                .forts
                .values()
                .any(|j| j.fort == f && near(mid_of(shape, &j.line), at, r))
    }

    /// Whether a built own piece of blueprint `f` stands near `at`.
    fn built_near(&self, f: u16, at: FxVec2) -> bool {
        let me = self.me;
        let r = Fx::from_int(SITE_NEAR_M);
        self.g
            .fort
            .pieces
            .values()
            .any(|q| q.owner == me && q.fort == f && q.part == 0 && near(q.pos, at, r))
    }

    /// Whether work `k` at own point `i` exists (built, building or a job).
    fn point_has(&self, i: usize, k: WorkKind) -> Option<bool> {
        let f = fort_of(&self.g.setup, k)?;
        if matches!(k, WorkKind::ApField | WorkKind::AtField) {
            return self.field_has(i, k, f);
        }
        let site = self.point_site(i, k, None)?;
        let shape = self.g.setup.forts[f as usize].shape;
        let r = match k {
            WorkKind::Wire => self.wire_len().div_int(2) + Fx::from_int(SITE_NEAR_M),
            _ => Fx::from_int(SITE_NEAR_M),
        };
        // The wire's 25 m retry counts too, and both unshifted (a shift
        // is at most half the line: any shifted line's middle lies within
        // `r` of the unshifted one; the route out may change).
        let alts: Vec<Vec<FxVec2>> = if k == WorkKind::Wire {
            [
                self.point_site(i, k, Some(WIRE_RETRY_M)),
                self.point_site_at(i, k, None, false),
                self.point_site_at(i, k, Some(WIRE_RETRY_M), false),
            ]
            .into_iter()
            .flatten()
            .collect()
        } else {
            Vec::new()
        };
        Some(
            self.fort_near(f, mid_of(shape, &site), r)
                || alts.iter().any(|l| self.fort_near(f, mid_of(shape, l), r)),
        )
    }

    /// S10b-ai: whether point `i` has its field `k` (blueprint `f`): an own
    /// mine, work or job of it within half the blueprint length (+
    /// `SITE_NEAR_M`) of the AP site's middle; AT (no path search): within
    /// `at_out_m` + that of the point.
    fn field_has(&self, i: usize, k: WorkKind, f: u16) -> Option<bool> {
        let s = &self.g.setup;
        let r = Fx::from(s.forts[f as usize].max_length).div_int(2) + Fx::from_int(SITE_NEAR_M);
        let (at, r) = match k {
            WorkKind::AtField => (
                s.map.points.get(i)?.pos.into(),
                r + Fx::from_int(s.rules.ai.obstacles.at_out_m as i32),
            ),
            _ => {
                let site = self.point_site(i, k, None)?;
                ((site[0] + site[site.len() - 1]).div_int(2), r)
            }
        };
        Some(self.fort_near(f, at, r))
    }

    /// S10b-ai R4: own fields of blueprint `f` (mines, works and jobs
    /// grouped greedily in map order: one field per group within the
    /// blueprint length).
    fn fields_of(&self, f: u16) -> u32 {
        let g = self.g;
        let me = self.me;
        let d = &g.setup.forts[f as usize];
        let r = Fx::from(d.max_length);
        let at: Vec<FxVec2> = g
            .fort
            .mines
            .values()
            .filter(|m| m.owner == me && m.fort == f)
            .map(|m| m.pos)
            .chain(
                g.fort
                    .works
                    .values()
                    .filter(|w| w.owner == me && w.fort == f)
                    .map(|w| mid_of(d.shape, &w.line)),
            )
            .chain(
                self.ai
                    .forts
                    .values()
                    .filter(|j| j.fort == f && !j.parked() && j.work.is_none())
                    .map(|j| mid_of(d.shape, &j.line)),
            )
            .collect();
        let mut heads: Vec<FxVec2> = Vec::new();
        for p in at {
            if !heads.iter().any(|h| near(*h, p, r)) {
                heads.push(p);
            }
        }
        heads.len() as u32
    }

    /// S10b-ai Build E (`at_rows` 2, S10b-ai-B2a): the second row of point
    /// `i`'s AT field: the first row's line moved one step (`segment`)
    /// toward the point and half a step along itself, so its mines lie
    /// between the first row's lanes.
    pub(super) fn at_row2_site(&self, i: usize) -> Option<Vec<FxVec2>> {
        let s = &self.g.setup;
        let f = fort_of(s, WorkKind::AtField)?;
        let row = self.at_site(i)?;
        let (a, b) = (row[0], row[row.len() - 1]);
        let along = (b - a).normalize_or_zero();
        let pos: FxVec2 = s.map.points.get(i)?.pos.into();
        let mut back = along.perp();
        if back.dot(pos - (a + b).div_int(2)) < Fx::ZERO {
            back = -back;
        }
        let step = Fx::from(s.forts[f as usize].segment);
        let shift = back * step + along * step.div_int(2);
        let tr = self.terrain;
        let line = vec![tr.clamp(a + shift), tr.clamp(b + shift)];
        ((line[1] - line[0]).length() >= Fx::from_int(2)).then_some(line)
    }

    /// Whether point `i`'s first AT row is laid: an own AT mine near the
    /// point (as [`Self::field_has`]), no own AT work there and no running
    /// AT job of the point.
    fn at_row1_laid(&self, i: usize, f: u16) -> bool {
        let g = self.g;
        let s = &g.setup;
        let me = self.me;
        let Some(pt) = s.map.points.get(i) else {
            return false;
        };
        let at: FxVec2 = pt.pos.into();
        let d = &s.forts[f as usize];
        let r = Fx::from(d.max_length).div_int(2)
            + Fx::from_int(SITE_NEAR_M)
            + Fx::from_int(s.rules.ai.obstacles.at_out_m as i32);
        g.fort
            .mines
            .values()
            .any(|m| m.owner == me && m.fort == f && near(m.pos, at, r))
            && !g
                .fort
                .works
                .values()
                .any(|w| w.owner == me && w.fort == f && near(mid_of(d.shape, &w.line), at, r))
            && !self
                .ai
                .forts
                .values()
                .any(|j| j.fort == f && j.point as usize == i && j.phase != FortPhase::Done)
    }

    /// Whether `row` (a second-row line) has its row: an own AT mine
    /// within half a step of it (the first row's lie a step away), or an
    /// own AT work or job whose middle lies within 3/4 step of its middle.
    fn at_row2_has(&self, f: u16, row: &[FxVec2]) -> bool {
        let g = self.g;
        let me = self.me;
        let d = &g.setup.forts[f as usize];
        let step = Fx::from(d.segment);
        let (a, b) = (row[0], row[row.len() - 1]);
        let mid = (a + b).div_int(2);
        let r = step.mul_int(3).div_int(4);
        g.fort.mines.values().any(|m| {
            m.owner == me && m.fort == f && crate::fort::near_segment(m.pos, a, b, step.div_int(2))
        }) || g
            .fort
            .works
            .values()
            .any(|w| w.owner == me && w.fort == f && near(mid_of(d.shape, &w.line), mid, r))
            || self
                .ai
                .forts
                .values()
                .any(|j| j.fort == f && !j.parked() && near(mid_of(d.shape, &j.line), mid, r))
    }

    /// Review finding 20: point `i`'s own AT mines within the field
    /// radius (as [`Self::at_row1_laid`]), plus the segments of its own AT
    /// works there and of its live AT jobs.
    fn at_point_count(&self, i: usize, f: u16) -> u32 {
        let g = self.g;
        let s = &g.setup;
        let me = self.me;
        let Some(pt) = s.map.points.get(i) else {
            return 0;
        };
        let at: FxVec2 = pt.pos.into();
        let d = &s.forts[f as usize];
        let r = Fx::from(d.max_length).div_int(2)
            + Fx::from_int(SITE_NEAR_M)
            + Fx::from_int(s.rules.ai.obstacles.at_out_m as i32);
        let mines = g
            .fort
            .mines
            .values()
            .filter(|m| m.owner == me && m.fort == f && near(m.pos, at, r))
            .count() as u32;
        let works: u32 = g
            .fort
            .works
            .values()
            .filter(|w| w.owner == me && w.fort == f && near(mid_of(d.shape, &w.line), at, r))
            .map(|w| crate::fort::segments(&w.line, d.segment))
            .sum();
        let jobs: u32 = self
            .ai
            .forts
            .values()
            .filter(|j| {
                j.fort == f && j.point as usize == i && !j.parked() && j.phase != FortPhase::Done
            })
            .map(|j| crate::fort::segments(&j.line, d.segment))
            .sum();
        mines.saturating_add(works).saturating_add(jobs)
    }

    /// R4 for the second row: wanted (on the recheck plan) once the first
    /// row is laid, the second is not, and the blueprint cap and the S10b
    /// budget hold the whole row (else the field keeps one row). Point-wide
    /// (review finding 20): only while the point's own AT mines, works and
    /// jobs are fewer than 1.5 rows, so a moved route lays no third row.
    fn at_row2_wanted(&self, i: usize) -> bool {
        let s = &self.g.setup;
        if !switched_on(s, WorkKind::AtRow2) || !self.recheck_plan() {
            return false;
        }
        let Some(f) = fort_of(s, WorkKind::AtRow2) else {
            return false;
        };
        if !self.at_row1_laid(i, f) {
            return false;
        }
        let Some(row) = self.at_row2_site(i) else {
            return false;
        };
        let d = &s.forts[f as usize];
        let one = crate::fort::segments(&row, d.segment);
        if self.at_point_count(i, f).saturating_mul(2) >= one.saturating_mul(3) {
            return false;
        }
        let need_g = one.saturating_mul(d.materials_g);
        !self.at_row2_has(f, &row) && self.under_limit(f, &row) && self.room_for(f) >= need_g
    }

    /// S10b-ai: the plan of each `recheck_s` window (stateless: plans come
    /// every `replan_ticks`, so one falls in each window).
    fn recheck_plan(&self) -> bool {
        let s = &self.g.setup;
        let w = s.rules.ai.obstacles.recheck_s as u64 * s.tick_hz.max(1) as u64;
        w == 0 || self.tick % w.max(1) < self.plan_ticks().min(w)
    }

    /// S10b-ai R1, R2, R4: the fields point `i` still wants (AP, then AT),
    /// within `fields_per_point`, `at_fields_max` and the blueprint caps.
    /// The AT field is offered on the recheck plan only (its site costs a
    /// path search).
    fn field_wants(&self, i: usize) -> Vec<WorkKind> {
        let s = &self.g.setup;
        let o = &s.rules.ai.obstacles;
        let mut out = Vec::new();
        if !switched_on(s, WorkKind::ApField) {
            return out;
        }
        let mut have = 0u32;
        for k in [WorkKind::ApField, WorkKind::AtField] {
            if !switched_on(s, k) {
                continue;
            }
            let Some(f) = fort_of(s, k) else { continue };
            let Some(h) = self.field_has(i, k, f) else {
                continue;
            };
            if h {
                have += 1;
                continue;
            }
            let d = &s.forts[f as usize];
            let segs = crate::fort::segments(
                &[FxVec2::ZERO, FxVec2::new(Fx::from(d.max_length), Fx::ZERO)],
                d.segment,
            );
            let fits = self.fort_use(f).saturating_add(segs) <= d.limit_per_player;
            let ok = match k {
                WorkKind::ApField => {
                    !self.ap_after_wire() || self.point_has(i, WorkKind::Wire) == Some(true)
                }
                _ => self.fields_of(f) < o.at_fields_max && self.recheck_plan(),
            };
            if fits && ok {
                out.push(k);
            }
        }
        out.truncate(o.fields_per_point.saturating_sub(have) as usize);
        // Build E: the second AT row belongs to the same field (no
        // `fields_per_point` count).
        if self.at_row2_wanted(i) {
            out.push(WorkKind::AtRow2);
        }
        out
    }

    /// The works point `i` still wants, in build order (nest, wire,
    /// dugout), within `works_per_point`; then (S10b-ai) its fields.
    fn point_wants(&self, i: usize) -> Vec<WorkKind> {
        let s = &self.g.setup;
        let f = &s.rules.ai.fort;
        let mut have = 0u32;
        let mut want = Vec::new();
        let nest = self.point_has(i, WorkKind::Nest);
        let shelled = self.ai.shelled.contains_key(&(i as u8)) || self.ai.enemy_mortar_seen;
        for k in [WorkKind::Nest, WorkKind::Wire, WorkKind::Dugout] {
            let Some(h) = self.point_has(i, k) else {
                continue;
            };
            if h {
                have += 1;
                continue;
            }
            // Fix F4: a work over the blueprint's per-player limit would be
            // refused; fix F5: at most `DUGOUTS_MAX` dugouts.
            let fits = fort_of(s, k)
                .zip(self.point_site(i, k, None))
                .is_some_and(|(fd, l)| self.under_limit(fd, &l))
                && (k != WorkKind::Dugout
                    || fort_of(s, k).is_some_and(|fd| self.fort_use(fd) < DUGOUTS_MAX));
            let wanted = fits
                && switched_on(s, k)
                && match k {
                    // Wire once the nest is done or started, or the mod
                    // has no nest, or (stage D) `nest` alone is off. With
                    // `works` 0 the wire still waits for a nest, so never
                    // comes: the works_off ablation as measured (no works
                    // and no point wire).
                    WorkKind::Wire => {
                        nest == Some(true)
                            || fort_of(s, WorkKind::Nest).is_none()
                            || (f.works != 0 && f.nest == 0)
                    }
                    WorkKind::Dugout => shelled,
                    _ => true,
                };
            if wanted {
                want.push(k);
            }
        }
        let room = f.works_per_point.saturating_sub(have) as usize;
        want.truncate(room);
        if s10b_on(s) {
            want.extend(self.bunker_wants(i));
            want.extend(self.field_wants(i));
            want.extend(self.obstacle_wants(i));
        }
        want
    }

    /// Marks own points that still want works. Nothing with `works` and
    /// `wire` off.
    pub(super) fn fort_points(&mut self, points: &mut [PointEval]) {
        if !habits_on(&self.g.setup) {
            return;
        }
        // S10c-ai F2: the saved lanes first, so this plan's wants see them.
        self.front_lanes_update();
        let max = self.g.setup.rules.ai.fort.build_threat_max;
        for (i, p) in points.iter_mut().enumerate() {
            // S10c-ai F4: a front point wants its front works without a
            // trench (`wants_at`); every other want keeps `trench_done`.
            // F5: the point's `build_threat_max` gate is not a front work's
            // (its threat is `front_threat`, at the start).
            let wants = self.wants_at(i, p.trench_done);
            p.wants_works = p.rel == Rel::Own
                && if p.threat < max {
                    !wants.is_empty()
                } else {
                    wants.iter().any(|&k| self.front_work(i, k))
                };
        }
    }

    /// Fort jobs under way (parked refusals left out).
    fn active_jobs(&self) -> usize {
        self.ai.forts.values().filter(|j| !j.parked()).count()
    }

    /// Whether another job may start (`builders_max`, and room in the
    /// saved map for a squad not yet in it).
    fn job_room(&self, started: usize) -> bool {
        let cap = (self.g.setup.rules.ai.fort.builders_max as usize).min(MAX_FORT_JOBS);
        self.active_jobs() + started < cap
    }

    /// B1-5: own foot squads with a man alive (every own squad with no
    /// vehicle that is not a crew: rifle, MG, AT, mortar, engineer,
    /// other; squads on a job included).
    pub(super) fn foot_squads(&self) -> u32 {
        let g = self.g;
        g.squads
            .values()
            .filter(|q| {
                q.owner == self.me
                    && !q.members.is_empty()
                    && g.setup.squad_types[q.type_idx as usize].vehicle.is_none()
                    && super::ai_kind(&g.setup, q.type_idx) != super::AiKind::Crew
            })
            .count() as u32
    }

    /// B1-5: whether new fort holds and jobs may start: match time at
    /// least `build_after_s` and at least `build_min_squads` foot squads
    /// ([`Self::foot_squads`]); a key at 0 is off. Running jobs go on.
    pub(super) fn build_gate_open(&self) -> bool {
        let f = &self.g.setup.rules.ai.fort;
        let hz = self.g.setup.tick_hz.max(1) as u64;
        (f.build_after_s == 0 || self.tick >= f.build_after_s as u64 * hz)
            && (f.build_min_squads == 0 || self.foot_squads() >= f.build_min_squads)
    }

    /// Fix F12: a known enemy soldier or vehicle within `threat_radius` of
    /// any of `at` (the builder and its site).
    fn fort_threat(&self, known: &[Known], kv: &[KnownVehicle], at: &[FxVec2]) -> bool {
        let r = sq_m(self.g.setup.rules.ai.threat_radius.into());
        at.iter().any(|&p| {
            known.iter().any(|e| (e.pos - p).length_sq_raw() <= r)
                || kv.iter().any(|v| (v.pos - p).length_sq_raw() <= r)
        })
    }

    /// S10c-ai F5: the threat rule of a front job, the same at its start
    /// (`can_take_at`) and for the drop (`fort_continue`): which part
    /// holds, if any. `Some(true)`: a known enemy soldier or vehicle within
    /// `front_site_threat_m` of `site`; `Some(false)`: one within the
    /// builder's radius of `lead`, `[ai] threat_radius` while the leader is
    /// more than `FRONT_AT_SITE_M` from the site (walking there), else
    /// `front_site_threat_m`. None: no threat.
    fn front_threat(
        &self,
        known: &[Known],
        kv: &[KnownVehicle],
        lead: FxVec2,
        site: FxVec2,
    ) -> Option<bool> {
        let s = &self.g.setup;
        let site_m = Fx::from_int(s.rules.ai.obstacles.front_site_threat_m as i32);
        let hit = |p: FxVec2, m: Fx| {
            let r = sq_m(m);
            known.iter().any(|e| (e.pos - p).length_sq_raw() <= r)
                || kv.iter().any(|v| (v.pos - p).length_sq_raw() <= r)
        };
        if hit(site, site_m) {
            return Some(true);
        }
        let lead_m = if near(lead, site, Fx::from_int(FRONT_AT_SITE_M)) {
            site_m
        } else {
            s.rules.ai.threat_radius.into()
        };
        hit(lead, lead_m).then_some(false)
    }

    /// `carry_kg_per_man` x living men of squad `sq` (g).
    fn full_carry_g(&self, sq: SquadId) -> u32 {
        let g = self.g;
        let (Some(ft), Some(q)) = (g.setup.rules.fortify.as_ref(), g.squads.get(&sq)) else {
            return 0;
        };
        let alive = q.members.iter().filter(|m| self.ents.is_alive(**m)).count() as u32;
        ft.carry_kg_per_man
            .saturating_mul(1000)
            .saturating_mul(alive)
    }

    /// Fix F14: whether builder `sq` can bring one step of blueprint `f`
    /// (a point work's kg, a line's segment): its full carry holds it, or
    /// a truck is free for a run; and the kg it carries plus the budget
    /// cover it.
    fn can_supply(&self, sq: SquadId, f: u16, site: FxVec2) -> bool {
        let d = &self.g.setup.forts[f as usize];
        let need = d.materials_g;
        (self.full_carry_g(sq) >= need || self.truck_for(f) || self.at_hand(sq, d, site))
            && self.carried_g(sq).saturating_add(self.room_for(f)) >= need
    }

    /// S10b-ai R7: an S10b step over the builder's full carry (the 250 kg
    /// bunker) needs no truck run when its carry plus the own construction
    /// crates and vehicle kg within `material_reach_m` of `site` (what the
    /// work draws from) cover it.
    fn at_hand(&self, sq: SquadId, d: &frontline_content::setup::FortSetup, site: FxVec2) -> bool {
        if !s10b_fort(d) {
            return false;
        }
        let g = self.g;
        let me = self.me;
        let need = d.materials_g as u64;
        let mut got = self.carried_g(sq) as u64;
        if got >= need {
            return true;
        }
        let Some(ft) = g.setup.rules.fortify.as_ref() else {
            return false;
        };
        let r = Fx::from_int(ft.material_reach_m as i32);
        got += g
            .supply6
            .crates
            .values()
            .filter(|c| {
                c.owner == me
                    && c.carrier.is_none()
                    && g.setup.stocks[c.def as usize].supply == Some(SupplyKind::Construction)
                    && near(c.pos, site, r)
            })
            .map(|c| c.g as u64)
            .sum::<u64>();
        got += g
            .vehicles
            .iter()
            .filter(|(v, vs)| {
                vs.material_g > 0
                    && vs.wreck_tick.is_none()
                    && self.ents.owner[v.index()] == me
                    && near(self.ents.transform[v.index()].pos.xy(), site, r)
            })
            .map(|(_, vs)| vs.material_g as u64)
            .sum::<u64>();
        got >= need
    }

    /// The budget room (g) for blueprint `f`: S10b works their own (R8).
    fn room_for(&self, f: u16) -> u32 {
        if s10b_fort(&self.g.setup.forts[f as usize]) {
            self.s10b_room_g()
        } else {
            self.materials_room_g()
        }
    }

    /// Whether free squad `m` could take a job for blueprint `f` with its
    /// site at `site` now (fort_start's pick and the assign hold).
    fn can_take(
        &self,
        m: &Mine,
        f: u16,
        site: FxVec2,
        known: &[Known],
        kv: &[KnownVehicle],
    ) -> bool {
        self.can_take_at(m, f, site, known, kv, false)
    }

    /// [`Self::can_take`]; `front`: a front job, whose threat is F5's
    /// [`Self::front_threat`] instead of `fort_threat` (80 m / 80 m).
    fn can_take_at(
        &self,
        m: &Mine,
        f: u16,
        site: FxVec2,
        known: &[Known],
        kv: &[KnownVehicle],
        front: bool,
    ) -> bool {
        let d = &self.g.setup.forts[f as usize];
        m.strength_pct >= 60
            && builds(m.caps)
            && (m.caps & (1u32 << d.capability) != 0 || heavy_by_field(d, m.caps))
            && self.can_supply(m.id, f, site)
            && if front {
                self.front_threat(known, kv, m.lead, site).is_none()
            } else {
                !self.fort_threat(known, kv, &[m.lead, site])
            }
    }

    /// Fix F15: the builder each wanting own point holds (the assign
    /// bonus): one per point, points ascending, while `builders_max` has
    /// room and fewer than `hold_max` (B1-4) points hold one, a free
    /// engineer (no `breach`) that could start the first wanted work it
    /// can, nearest first. Nothing with the habits off. `hold_max` caps
    /// only the holds: `fort_start` still gives a job to a fit engineer
    /// assigned to the point without one.
    pub(super) fn fort_holds(
        &mut self,
        mine: &[Mine],
        known: &[Known],
        kv: &[KnownVehicle],
        points: &mut [PointEval],
    ) {
        if !habits_on(&self.g.setup) || !self.build_gate_open() {
            return;
        }
        let s = &self.g.setup;
        let mut held: Vec<SquadId> = Vec::new();
        // S10c-ai F4: front points first.
        for i in self.point_order(points.len()) {
            let pt = &points[i];
            if !pt.wants_works {
                continue;
            }
            if held.len() >= s.rules.ai.fort.hold_max as usize || !self.job_room(held.len()) {
                break;
            }
            let pos = pt.pos;
            let hot = pt.threat >= s.rules.ai.fort.build_threat_max;
            let pick = self.wants_at(i, pt.trench_done).into_iter().find_map(|k| {
                let front = self.front_work(i, k);
                if (hot && !front) || (front && !self.front_slot_free()) {
                    return None;
                }
                let (f, line) = (fort_of(s, k)?, self.point_site(i, k, None)?);
                let site = mid_of(s.forts[f as usize].shape, &line);
                mine.iter()
                    .filter(|m| {
                        m.role == super::AiKind::Engineer
                            && !m.can(Capability::Breach)
                            && !held.contains(&m.id)
                            && self.can_take_at(m, f, site, known, kv, front)
                            && (!front || self.front_carry_ok(m.id, f, &line))
                    })
                    .min_by_key(|m| {
                        let d = &s.forts[f as usize];
                        (
                            heavy_by_field(d, m.caps),
                            (m.lead - pos).length_sq_raw(),
                            m.id,
                        )
                    })
                    .map(|m| m.id)
            });
            if let Some(id) = pick {
                held.push(id);
                points[i].hold = Some(id);
            }
        }
    }

    // ------------------------------------------------------------ marks --

    /// The shelled marks and `enemy_mortar_seen` (spec 3.3; `works` on).
    pub(super) fn fort_marks(&mut self, known: &[Known]) {
        let s = &self.g.setup;
        if s.rules.fortify.is_none() || s.rules.ai.fort.works == 0 {
            return;
        }
        let g = self.g;
        if !self.ai.enemy_mortar_seen
            && known.iter().any(|k| {
                g.squads.get(&SquadId(k.squad)).is_some_and(|q| {
                    s.squad_types[q.type_idx as usize].has(Capability::FireMission)
                })
            })
        {
            self.ai.enemy_mortar_seen = true;
        }
        let f = &s.rules.ai.fort;
        let crouch = s.rules.combat.crouch_suppression;
        let clear = sq_m(Fx::from_int(f.shelled_clear_m as i32));
        for (i, c) in g.points.iter().enumerate() {
            if c.owner != Some(self.me) {
                continue;
            }
            let pos: FxVec2 = s.map.points[i].pos.into();
            if known.iter().any(|k| (k.pos - pos).length_sq_raw() <= clear) {
                continue;
            }
            let (mut men, mut down) = (0u32, 0u32);
            for (id, t) in &self.ai.tasks {
                if t.kind != TaskKind::Defend || t.point as usize != i {
                    continue;
                }
                let Some(q) = g.squads.get(id) else { continue };
                for &m in &q.members {
                    if !self.ents.is_alive(m) {
                        continue;
                    }
                    let cb = &self.ents.combat[m.index()];
                    men += 1;
                    down += (cb.pinned || cb.suppression / 1000 >= crouch) as u32;
                }
            }
            if men > 0 && down * 100 >= men * f.shelled_pct {
                self.ai.shelled.insert(i as u8, self.tick);
            }
        }
    }

    /// The built own nest (MG team) and the dugout to shelter in (one
    /// squad) at own point `i`, if any (for `order_infantry`).
    pub(super) fn nest_at(&self, i: usize) -> Option<FxVec2> {
        let s = &self.g.setup;
        if s.rules.fortify.is_none() || s.rules.ai.fort.works == 0 {
            return None;
        }
        let f = fort_of(s, WorkKind::Nest)?;
        let site = self.point_site(i, WorkKind::Nest, None)?[0];
        self.built_near(f, site).then_some(site)
    }

    /// The dugout spot of own point `i` (built or not; `works` on): an
    /// order there is a shelter move.
    pub(super) fn dugout_spot(&self, i: usize) -> Option<FxVec2> {
        let s = &self.g.setup;
        if s.rules.fortify.is_none() || s.rules.ai.fort.works == 0 {
            return None;
        }
        Some(self.point_site(i, WorkKind::Dugout, None)?[0])
    }

    /// The dugout spot of own point `i` when a dugout stands there (fix F6).
    pub(super) fn dugout_built(&self, i: usize) -> Option<FxVec2> {
        let d = fort_of(&self.g.setup, WorkKind::Dugout)?;
        self.dugout_spot(i).filter(|&at| self.built_near(d, at))
    }

    /// The dugout at own point `i` when its men should shelter now: built,
    /// the point marked shelled within `shelter_s`, no known enemy within
    /// `shelter_clear_m`.
    pub(super) fn shelter_at(&self, i: usize, known: &[Known]) -> Option<FxVec2> {
        let s = &self.g.setup;
        let f = &s.rules.ai.fort;
        if !switched_on(s, WorkKind::Dugout) {
            return None;
        }
        let hz = s.tick_hz.max(1) as u64;
        let at = *self.ai.shelled.get(&(i as u8))?;
        if self.tick > at + f.shelter_s as u64 * hz {
            return None;
        }
        let pos: FxVec2 = s.map.points.get(i)?.pos.into();
        let r = sq_m(Fx::from_int(f.shelter_clear_m as i32));
        if known.iter().any(|k| (k.pos - pos).length_sq_raw() <= r) {
            return None;
        }
        let d = fort_of(s, WorkKind::Dugout)?;
        let site = self.point_site(i, WorkKind::Dugout, None)?[0];
        self.built_near(d, site).then_some(site)
    }

    /// Spots under the built own dugout roof at `site` (shelter, spec
    /// 3.3): trench spots (the cover rule's, so a man who arrives keeps
    /// his) inside the roof box, at least `cover.gap_mm` apart, found on a
    /// 0.25 m grid along then across the roof, at most [`SHELTER_SPOTS`].
    /// Empty without a roof there.
    pub(super) fn shelter_spots(&self, site: FxVec2) -> Vec<FxVec2> {
        let s = &self.g.setup;
        let me = self.me;
        let Some(d) = fort_of(s, WorkKind::Dugout) else {
            return Vec::new();
        };
        let Some(half) = s.forts[d as usize].pieces.first().map(|p| p.half) else {
            return Vec::new();
        };
        let r = Fx::from_int(SITE_NEAR_M);
        let roof = self
            .g
            .fort
            .pieces
            .values()
            .filter(|q| q.owner == me && q.fort == d && q.part == 0 && near(q.pos, site, r))
            .min_by_key(|q| ((q.pos - site).length_sq_raw(), q.pos.x.raw(), q.pos.y.raw()));
        let Some(roof) = roof else { return Vec::new() };
        let (sn, cs) = roof.yaw.sin_cos();
        let along = FxVec2::new(cs, sn);
        let across = along.perp();
        let step = Fx::from_milli(SHELTER_STEP_MM);
        let n = |h: Fixed| (Fx::from(h).raw() / step.raw()).clamp(0, 16);
        let (na, nb) = (n(half[0]), n(half[1]));
        let gap = Fx::from_milli(s.rules.cover.gap_mm as i32);
        let mut out: Vec<FxVec2> = Vec::new();
        for a in -na..=na {
            for b in -nb..=nb {
                let p = roof.pos + along * step.mul_int(a) + across * step.mul_int(b);
                if self.terrain.contains(p)
                    && crate::squad::trench_spot(self.terrain, None, p, &s.rules)
                    && out.iter().all(|&q| !near(q, p, gap))
                {
                    out.push(p);
                }
            }
        }
        out.truncate(SHELTER_SPOTS);
        out
    }

    /// Whether squad `id` has men sent under its point's dugout roof
    /// ([`Self::shelter_men`]): the throwers' regroup leaves it alone.
    pub(super) fn sheltering(&self, id: SquadId) -> bool {
        let Some(t) = self.ai.tasks.get(&id) else {
            return false;
        };
        if t.kind != TaskKind::Defend || t.point == NO_POINT {
            return false;
        }
        let (Some(d), Some(q)) = (self.dugout_spot(t.point as usize), self.g.squads.get(&id))
        else {
            return false;
        };
        q.members.iter().any(|m| {
            self.ents
                .unit
                .get(m.index())
                .is_some_and(|u| u.detached && near(u.nav.goal, d, Fx::from_int(3)))
        })
    }

    /// Sends squad `id`'s men (member order, at most the spots) one by one
    /// to the spots under the dugout roof at `site`; a man already there or
    /// walking there gets nothing. False without a roof (the caller then
    /// sends the squad order). The rest of the squad keeps its order.
    pub(super) fn shelter_men(&mut self, id: SquadId, site: FxVec2) -> bool {
        let spots = self.shelter_spots(site);
        if spots.is_empty() {
            return false;
        }
        let Some(q) = self.g.squads.get(&id) else {
            return false;
        };
        let ents = self.ents;
        let mut men: Vec<_> = q
            .members
            .iter()
            .copied()
            .filter(|m| ents.kind.get(m.index()) == Some(&crate::ecs::EntityKind::Soldier))
            .collect();
        // By id: the member order changes (a new leader), the spots do not.
        men.sort_unstable();
        let close = Fx::from_milli(500);
        let mut sent = false;
        for (&m, &spot) in men.iter().zip(&spots) {
            let i = m.index();
            let u = &ents.unit[i];
            let there = near(ents.transform[i].pos.xy(), spot, close);
            let going = u.detached && near(u.nav.goal, spot, Fx::from_milli(100));
            if there || going {
                continue;
            }
            self.ai.emit(
                self.due,
                CommandKind::MoveUnits {
                    units: vec![m],
                    target: spot,
                },
            );
            sent = true;
        }
        self.ai.counters.shelter_moves += sent as u32;
        true
    }

    // -------------------------------------------------------- materials --

    /// Construction grams this AI may still buy: under `materials_max_pct`
    /// of the income earned (start + income per whole minute), less loads
    /// pending, and out of the manpower after the gun pot.
    pub(super) fn materials_room_g(&self) -> u32 {
        let g = self.g;
        let s = &g.setup;
        let Some(ft) = s.rules.fortify.as_ref() else {
            return 0;
        };
        let per_t = ft.construction_mp_per_t as u64;
        let hz = s.tick_hz.max(1) as u64;
        let minutes = self.tick / (60 * hz);
        let income = s.rules.start_manpower as u64 + s.rules.income_per_min as u64 * minutes;
        let budget = income * s.rules.ai.fort.materials_max_pct as u64 / 100 * crate::supply::MICRO;
        let pending: u64 = self
            .ai
            .pending
            .iter()
            .filter_map(|c| match c.kind {
                CommandKind::LoadMaterials { kg, .. } => Some(kg as u64 * 1000 * per_t),
                _ => None,
            })
            .sum();
        // S10b-ai R8: S10b works draw on their own budget (0 with its
        // switches off, so S10a's is as before).
        // Saturating: a loaded `mines_laid` is not bounded (finding 3).
        let s10b = self.s10b_used_g().saturating_mul(per_t);
        let spent =
            (g.fort.accounts[self.me.0 as usize].materials_micro + pending).saturating_sub(s10b);
        let pl = &g.players[self.me.0 as usize];
        let have = (pl.manpower as i64 - self.gun_pot()).max(0) as u64 * crate::supply::MICRO
            + pl.ammo.credit as u64;
        let room = budget
            .saturating_sub(spent)
            .min(have.saturating_sub(pending));
        if per_t == 0 {
            return u32::MAX;
        }
        (room / per_t).min(u32::MAX as u64) as u32
    }

    /// S10b-ai R8: grams this AI put into S10b works: own standing mines
    /// at their blueprint's segment grams, mines gone since at the
    /// cheapest S10b mine's (a lower bound), grams in own S10b works, and
    /// what S10b job squads carry or have a load pending for. 0 with the
    /// switches off (no S10b job, no S10b mine of an AI).
    pub(super) fn s10b_used_g(&self) -> u64 {
        let g = self.g;
        let s = &g.setup;
        if !s10b_on(s) {
            return 0;
        }
        let me = self.me;
        let fs = &g.fort;
        let grams = |f: u16| s.forts.get(f as usize).map_or(0, |d| d.materials_g as u64);
        let (mut standing, mut used) = (0u64, 0u64);
        for m in fs.mines.values().filter(|m| m.owner == me) {
            standing += 1;
            used += grams(m.fort);
        }
        let cheapest = s
            .forts
            .iter()
            .filter(|d| d.mine_item.is_some())
            .map(|d| d.materials_g as u64)
            .min()
            .unwrap_or(0);
        let laid = fs
            .accounts
            .get(me.0 as usize)
            .map_or(0, |a| a.mines_laid as u64);
        // Saturating: a loaded `mines_laid` is not bounded.
        used = used.saturating_add(laid.saturating_sub(standing).saturating_mul(cheapest));
        used = used.saturating_add(
            fs.works
                .values()
                .filter(|w| w.owner == me && s.forts.get(w.fort as usize).is_some_and(s10b_fort))
                .map(|w| w.got_g as u64)
                .sum::<u64>(),
        );
        for (sq, j) in &self.ai.forts {
            if j.parked() || !s.forts.get(j.fort as usize).is_some_and(s10b_fort) {
                continue;
            }
            used = used.saturating_add(self.carried_g(*sq) as u64);
            used = used.saturating_add(
                self.ai
                    .pending
                    .iter()
                    .filter_map(|c| match c.kind {
                        CommandKind::LoadMaterials { squad, kg } if squad == *sq => {
                            Some(kg as u64 * 1000)
                        }
                        _ => None,
                    })
                    .sum::<u64>(),
            );
        }
        used
    }

    /// S10b-ai R8: grams S10b works may still load: under
    /// `[ai.obstacles] materials_max_pct` of the income earned, less
    /// [`Self::s10b_used_g`], and out of the manpower after the gun pot.
    pub(super) fn s10b_room_g(&self) -> u32 {
        let g = self.g;
        let s = &g.setup;
        let Some(ft) = s.rules.fortify.as_ref() else {
            return 0;
        };
        let per_t = ft.construction_mp_per_t as u64;
        let hz = s.tick_hz.max(1) as u64;
        let minutes = self.tick / (60 * hz);
        let income = s.rules.start_manpower as u64 + s.rules.income_per_min as u64 * minutes;
        let budget =
            income * s.rules.ai.obstacles.materials_max_pct as u64 / 100 * crate::supply::MICRO;
        let spent = self.s10b_used_g().saturating_mul(per_t);
        let pending: u64 = self
            .ai
            .pending
            .iter()
            .filter_map(|c| match c.kind {
                CommandKind::LoadMaterials { kg, .. } => Some(kg as u64 * 1000 * per_t),
                _ => None,
            })
            .sum();
        let pl = &g.players[self.me.0 as usize];
        let have = (pl.manpower as i64 - self.gun_pot()).max(0) as u64 * crate::supply::MICRO
            + pl.ammo.credit as u64;
        let room = budget
            .saturating_sub(spent)
            .min(have.saturating_sub(pending));
        if per_t == 0 {
            return u32::MAX;
        }
        (room / per_t).min(u32::MAX as u64) as u32
    }

    fn load_pending(&self, sq: SquadId) -> bool {
        self.ai.pending.iter().any(|c| {
            matches!(c.kind, CommandKind::LoadMaterials { squad, .. } if squad == sq)
                || matches!(c.kind, CommandKind::Fortify { squad, .. } if squad == sq)
        })
    }

    /// Room on builder squad `sq` (g): `carry_kg_per_man` per living man
    /// less what it carries.
    fn carry_room_g(&self, sq: SquadId) -> u32 {
        let g = self.g;
        let Some(ft) = g.setup.rules.fortify.as_ref() else {
            return 0;
        };
        let Some(q) = g.squads.get(&sq) else { return 0 };
        let alive = q.members.iter().filter(|m| self.ents.is_alive(**m)).count() as u32;
        (ft.carry_kg_per_man * 1000 * alive).saturating_sub(self.carried_g(sq))
    }

    fn carried_g(&self, sq: SquadId) -> u32 {
        self.g.fort.carried_g.get(&sq).copied().unwrap_or(0)
    }

    /// `LoadMaterials` for builder `sq`'s full carry within the budget.
    /// Whether it was sent.
    /// S10b-ai R8: `room` is the budget's (`room_for` the job's blueprint).
    fn load_builder(&mut self, sq: SquadId, room: u32) -> bool {
        let kg = (self.carry_room_g(sq).min(room) / 1000).min(u16::MAX as u32);
        if kg == 0 || self.load_pending(sq) {
            return false;
        }
        self.ai.emit(
            self.due,
            CommandKind::LoadMaterials {
                squad: sq,
                kg: kg as u16,
            },
        );
        true
    }

    /// Builders standing in the own base carrying under `load_min_kg` load
    /// their full carry (spec 3.2).
    fn base_loads(&mut self, mine: &[Mine]) {
        let min_g = self.g.setup.rules.ai.fort.load_min_kg * 1000;
        for m in mine {
            if !builds(m.caps) || self.carried_g(m.id) >= min_g {
                continue;
            }
            if self.in_own_base(&self.g.squads[&m.id]) {
                let room = self.materials_room_g();
                self.load_builder(m.id, room);
            }
        }
    }

    /// Whether an own truck could start a material run (one in its Load
    /// phase, no run under way).
    fn truck_free(&self) -> bool {
        self.ai.material_run.is_none()
            && self
                .ai
                .trucks
                .values()
                .any(|j| j.phase == super::truck::TruckPhase::Load)
    }

    /// Whether a truck can bring a step of blueprint `f`: one is free
    /// ([`Self::truck_free`]), or (Build E, `bunker_truck_first`) `f` is
    /// the bunker and an own truck job lives that [`Self::truck_divert`]
    /// may take from tank service.
    fn truck_for(&self, f: u16) -> bool {
        self.truck_free()
            || (self.divert_on()
                && fort_of(&self.g.setup, WorkKind::Bunker) == Some(f)
                && self.ai.material_run.is_none()
                && !self.ai.trucks.is_empty())
    }

    /// Build E: whether `bunker_truck_first` is in force (S10b and the
    /// bunker switched on).
    fn divert_on(&self) -> bool {
        let s = &self.g.setup;
        s.rules.ai.obstacles.bunker_truck_first != 0
            && s10b_on(s)
            && switched_on(s, WorkKind::Bunker)
    }

    /// S10b-ai Build E (`bunker_truck_first`, S10b-ai-D1): a truck in tank
    /// service (Follow or Serve) is sent back to base for the bunker's
    /// material run while the bunker work waits for kg its builder cannot
    /// carry, no truck is in or on its way to base (Load, Return) and the
    /// run would start (the bunker's work is `waiting_work`'s, `run_kg`
    /// holds: truck room and S10b budget). Once per job: not after anyone but the
    /// builder brought kg to the step (its drawn kg over the builder's
    /// full carry). In base it takes the run as a loading truck does, then
    /// goes back to loading and its service (`end_run`).
    fn truck_divert(&mut self, t: &MyTank, job: &mut super::AiTruck) -> bool {
        use super::truck::TruckPhase;
        if !self.divert_on()
            || self.ai.material_run.is_some()
            || !matches!(job.phase, TruckPhase::Follow | TruckPhase::Serve)
            || self.ai.trucks.iter().any(|(q, j)| {
                *q != t.squad && matches!(j.phase, TruckPhase::Load | TruckPhase::Return)
            })
        {
            return false;
        }
        let g = self.g;
        let s = &g.setup;
        let Some(b) = fort_of(s, WorkKind::Bunker) else {
            return false;
        };
        let me = self.me;
        // The bunker's work must be the one a loading truck would serve
        // (`waiting_work`) with its builder's kg alone drawn (once per job),
        // and the run must start there (`run_kg`, the Load branch's test):
        // a truck sent back that starts no run would be sent again at every
        // plan (review finding 24).
        let Some(wid) = self.waiting_work() else {
            return false;
        };
        let bunker_waits = g
            .fort
            .works
            .get(&wid)
            .is_some_and(|w| w.fort == b && w.got_g <= self.full_carry_g(w.builder));
        if !bunker_waits || self.run_kg(t, wid).is_none() {
            return false;
        }
        if job.phase == TruckPhase::Serve {
            self.end_serve(job, false);
        }
        job.phase = TruckPhase::Return;
        job.since = self.tick;
        job.fails = 0;
        job.moved_to = t.pos;
        let base = g.base_of(me);
        self.truck_drive(t, job, base);
        true
    }

    /// S10a-ai truck run (spec 3.2), called by `run_truck` for each own
    /// truck. True when the truck is on (or just took) a material run and
    /// its other habits wait.
    pub(super) fn material_truck(
        &mut self,
        t: &MyTank,
        job: &mut super::AiTruck,
        in_base: bool,
        idle: bool,
    ) -> bool {
        let g = self.g;
        let s = &g.setup;
        if !s10a_on(s) {
            return false;
        }
        let Some(ft) = s.rules.fortify.as_ref() else {
            return false;
        };
        let f = &s.rules.ai.fort;
        let hz = s.tick_hz.max(1) as u64;
        let tick = self.tick;
        let vs = &g.vehicles[&t.id];
        if let Some((sq, wid)) = self.ai.material_run {
            if sq != t.squad {
                return false;
            }
            let me = self.me;
            let work = g.fort.works.get(&wid).filter(|w| w.owner == me);
            let late = tick > job.since + (f.material_wait_s as u64 + RUN_DRIVE_S) * hz;
            let unloaded =
                in_base && vs.material_g == 0 && tick > job.since + 2 * self.plan_ticks();
            // Fix F18: a line's run lasts until the line is done or gone
            // or the truck is empty (each segment draws from it); a point
            // work's ends once it builds.
            let line = |w: &crate::fort::Work| s.forts[w.fort as usize].shape == FortShape::Line;
            let end = match work {
                None => true,
                Some(w) => {
                    (w.step == WorkStep::Build && !line(w))
                        || late
                        || unloaded
                        || (!in_base && vs.material_g == 0)
                }
            };
            if end {
                self.ai.material_run = None;
                end_run(job, in_base, tick, t.pos);
                return true;
            }
            let Some(w) = work else { return true };
            if vs.material_g == 0 {
                return true;
            }
            let (site, _) = g.site_of(w);
            let reach = Fx::from_int(ft.material_reach_m as i32 - 3).max(Fx::ONE);
            if !near(t.pos, site, reach) {
                // Fix F18: it parks on the own base's side of the site, so
                // a line's run follows the line behind it, never along it
                // (a truck crushes wire).
                let mut back = (g.base_of(self.me) - site).normalize_or_zero();
                if back == FxVec2::ZERO {
                    back = (t.pos - site).normalize_or_zero();
                }
                let goal = self
                    .terrain
                    .clamp(site + back * (reach - Fx::from_int(2)).max(Fx::ONE));
                if idle || !near(job.moved_to, goal, Fx::from_int(5)) {
                    self.truck_drive(t, job, goal);
                }
            } else if !idle {
                self.ai.emit(self.due, CommandKind::Stop { squad: t.squad });
            }
            return true;
        }
        if self.truck_divert(t, job) {
            return true;
        }
        if job.phase != super::truck::TruckPhase::Load || !in_base || !idle {
            return false;
        }
        let Some(wid) = self.waiting_work() else {
            return false;
        };
        let Some(kg) = self.run_kg(t, wid) else {
            return false;
        };
        if kg > 0 {
            self.ai.emit(
                self.due,
                CommandKind::LoadMaterials {
                    squad: t.squad,
                    kg: kg.min(u16::MAX as u32) as u16,
                },
            );
        }
        self.ai.material_run = Some((t.squad, wid));
        job.since = tick;
        self.ai.counters.material_runs[0] += 1;
        true
    }

    /// The kg truck `t` loads for a run to work `wid` now, or `None` when
    /// the run cannot start: the truck's room, the budget of the work's
    /// kind (S10b-ai R8) and kg still aboard (fix F18) must bring the
    /// work's step (at most `truck_material_kg`).
    fn run_kg(&self, t: &MyTank, wid: u32) -> Option<u32> {
        let g = self.g;
        let s = &g.setup;
        let f = &s.rules.ai.fort;
        let vs = g.vehicles.get(&t.id)?;
        let v = &s.vehicles[vs.type_idx as usize];
        let cap = v.supply.as_ref().map_or(0, |x| x.cap_g as u64);
        let held = vs.cargo_g.iter().map(|&x| x as u64).sum::<u64>() + vs.material_g as u64;
        let room = cap.saturating_sub(held).min(u32::MAX as u64) as u32;
        let have_kg = vs.material_g / 1000;
        let wf = g.fort.works.get(&wid)?.fort;
        let kg = (f.truck_material_kg.saturating_sub(have_kg) * 1000)
            .min(room)
            .min(self.room_for(wf))
            / 1000;
        let need_kg = s.forts[wf as usize].materials_g.div_ceil(1000);
        (have_kg + kg > 0 && have_kg + kg >= need_kg.min(f.truck_material_kg)).then_some(kg)
    }

    /// The first own work (ascending id) of an AI fort job waiting for
    /// kg its builder does not carry.
    fn waiting_work(&self) -> Option<u32> {
        let me = self.me;
        self.ai.forts.iter().find_map(|(sq, j)| {
            let id = j.work?;
            let w = self.g.fort.works.get(&id)?;
            (w.owner == me
                && w.builder == *sq
                && w.step == WorkStep::Wait
                && self.carried_g(*sq) == 0)
                .then_some(id)
        })
    }

    pub(super) fn plan_ticks(&self) -> u64 {
        let s = &self.g.setup;
        let d = s
            .rules
            .difficulty
            .get(s.players[self.me.0 as usize].difficulty);
        d.replan_ticks.max(1) as u64
    }

    // ------------------------------------------------------------- jobs --

    /// Runs the fort jobs (before refills and assignment): a job's squad
    /// leaves `mine`; jobs whose squad is gone, weak or threatened (a known
    /// enemy soldier or vehicle near it or its site, fix F12), or whose
    /// point is lost or under `build_threat_max` (fix F13), end. Parked
    /// refusals stay until `REFUSED_S` with their squads free. Also the
    /// material run's deadline (fix F2) and the base loads. Nothing with
    /// `works` and `wire` off.
    pub(super) fn fort_continue(
        &mut self,
        mine: &mut Vec<Mine>,
        known: &[Known],
        kv: &[KnownVehicle],
        busy: &[SquadId],
    ) {
        let g = self.g;
        if !habits_on(&g.setup) {
            return;
        }
        let me = self.me;
        let hz = g.setup.tick_hz.max(1) as u64;
        let tick = self.tick;
        if self
            .ai
            .material_run
            .is_some_and(|(sq, _)| g.squads.get(&sq).is_none_or(|q| q.owner != me))
        {
            self.ai.material_run = None;
        }
        self.run_deadline();
        let s10b_on = s10b_on(&g.setup);
        if s10b_on {
            self.ai
                .drop_stale_refusals(tick, refusal_keep_ticks(&g.setup));
            self.count_own_blocked();
        }
        let ids: Vec<SquadId> = self.ai.forts.keys().copied().collect();
        for id in ids {
            let job = &self.ai.forts[&id];
            let s10b = s10b_on && g.setup.forts.get(job.fort as usize).is_some_and(s10b_fort);
            if job.parked() {
                let owned = g.squads.get(&id).is_some_and(|q| q.owner == me);
                if !owned || tick >= job.since.saturating_add(REFUSED_S * hz) {
                    self.ai.forts.remove(&id);
                }
                continue;
            }
            if busy.contains(&id) && g.squads.get(&id).is_some_and(|q| q.owner == me) {
                continue;
            }
            // S10b-ai L1: the deadline, whatever the phase (a failure).
            if s10b && tick >= job.start.saturating_add(job_max_s(&g.setup, job.fort) * hz) {
                let job = job.clone();
                self.ai.forts.remove(&id);
                self.bunker_end(&job, 12);
                self.s10b_end(&job, true);
                continue;
            }
            // S10c-ai F5, F6, F8: a front job (FrontAp, FrontWire, an AT
            // row at a front point) ends without a refusal when its squad
            // is gone or weak (`front_lost`), its point is lost, or F5's
            // threat holds (`front_drops`); what it laid stays and the rest
            // of its line is wanted again.
            let front = self.front_kind(job).is_some();
            let Some(k) = mine.iter().position(|m| m.id == id) else {
                if let Some(job) = self.ai.forts.remove(&id) {
                    if front {
                        self.ai.counters.front_lost += 1;
                    }
                    if s10b {
                        self.bunker_end(&job, 13);
                        self.s10b_end(&job, false);
                    }
                }
                continue;
            };
            let m = &mine[k];
            let site = mid_of(g.setup.forts[job.fort as usize].shape, &job.line);
            let owner_lost = job.point != NO_POINT
                && g.points
                    .get(job.point as usize)
                    .is_none_or(|c| c.owner != Some(me));
            // F5: the point's `build_threat_max` gate is not a front job's.
            let hot = !front
                && job.point != NO_POINT
                && g.setup.map.points.get(job.point as usize).is_some_and(|p| {
                    self.threat_at(known, p.pos.into()) >= g.setup.rules.ai.fort.build_threat_max
                });
            let threat = if front {
                self.front_threat(known, kv, m.lead, site).is_some()
            } else {
                self.fort_threat(known, kv, &[m.lead, site])
            };
            let weak = m.strength_pct < 60;
            if weak || owner_lost || hot || threat {
                if front {
                    if weak {
                        self.ai.counters.front_lost += 1;
                    } else if threat && !owner_lost {
                        self.ai.counters.front_drops += 1;
                    }
                }
                if let Some(job) = self.ai.forts.remove(&id)
                    && s10b
                {
                    self.bunker_end(&job, 13);
                    self.s10b_end(&job, false);
                }
                continue;
            }
            let mut job = job.clone();
            let was = job.phase;
            self.run_fort_job(id, &mut job);
            // S10c-ai F6: a front job that ends laid or built marks its lane.
            if job.phase == FortPhase::Done && was == FortPhase::Watch {
                self.front_mark_done(&job);
            }
            if job.phase == FortPhase::Done && s10b {
                // L3: a refused order (parked), or a watched work that
                // stalled or went without being laid, is a failure; a load
                // out of budget or carry is not the goal's.
                let failed = job.parked() || (was == FortPhase::Watch && !self.s10b_laid(&job));
                self.bunker_end(&job, if failed { 14 } else { 15 });
                self.s10b_end(&job, failed);
            }
            if job.phase == FortPhase::Done {
                // S10b-ai: a refused S10b job is not parked; its L3
                // refusal keeps the spot (L4).
                if job.parked() && !s10b {
                    self.ai.forts.insert(id, job);
                } else {
                    self.ai.forts.remove(&id);
                }
                // Free again this plan: it gets an infantry task.
                continue;
            }
            self.ai.forts.insert(id, job);
            mine.remove(k);
        }
        // S10a's base loads (not with only the S10b habits on).
        if s10a_on(&g.setup) {
            self.base_loads(mine);
        }
    }

    /// Fix F2: a material run ends when its work is gone or it is late,
    /// whether or not its truck reached `material_truck` this plan (a
    /// truck that is immobile, repairing or fleeing skips it).
    fn run_deadline(&mut self) {
        let Some((sq, wid)) = self.ai.material_run else {
            return;
        };
        let g = self.g;
        let me = self.me;
        let hz = g.setup.tick_hz.max(1) as u64;
        let tick = self.tick;
        let wait = g.setup.rules.ai.fort.material_wait_s as u64;
        let gone = g.fort.works.get(&wid).is_none_or(|w| w.owner != me);
        let late = self
            .ai
            .trucks
            .get(&sq)
            .is_none_or(|j| tick > j.since + (wait + RUN_DRIVE_S) * hz);
        if !(gone || late) {
            return;
        }
        self.ai.material_run = None;
        let at = g.squads.get(&sq).map(|q| {
            let v = q.members[0];
            (self.in_own_base(q), self.ents.transform[v.index()].pos.xy())
        });
        if let (Some(job), Some((in_base, pos))) = (self.ai.trucks.get_mut(&sq), at) {
            end_run(job, in_base, tick, pos);
        }
    }

    /// Starts new fort jobs for free builders (after assignment and the
    /// hunts): point works, the HQ roof, walls at guns. Returns the squads
    /// it took.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn fort_start(
        &mut self,
        mine: &[Mine],
        assigned: &[(usize, usize)],
        points: &[PointEval],
        known: &[Known],
        kv: &[KnownVehicle],
        taken: &[SquadId],
    ) -> Vec<SquadId> {
        let g = self.g;
        let s = &g.setup;
        let mut out = Vec::new();
        if !habits_on(s) {
            return out;
        }
        // B1-5: before `build_after_s` or under `build_min_squads` no
        // new job starts (running ones go on in `fort_continue`).
        if self.build_gate_open() {
            self.point_starts(mine, assigned, points, known, kv, taken, &mut out, false);
            self.bunker_why(mine, assigned, points, known, kv, taken, &out);
            self.mine_want_why(mine, assigned, points, known, kv, taken, &out);
            self.front_why(mine, assigned, points, known, kv, taken, &out);
            if switched_on(s, WorkKind::Roof) && self.job_room(0) {
                self.start_roof(mine, known, kv, taken, &mut out);
            }
            if switched_on(s, WorkKind::Wall) && self.job_room(0) {
                self.start_walls(mine, known, kv, taken, &mut out);
            }
            if switched_on(s, WorkKind::LogBarrier) && self.job_room(0) {
                self.start_barriers(mine, known, kv, taken, &mut out);
            }
        }
        // T97: plan-squads on a fort job, and (fix F15) builders the
        // assign bonus held at a point that got no job, over all
        // plan-squads given infantry tasks (those on a job included).
        // S10a's counters: not counted with only the S10b habits on.
        if !s10a_on(s) {
            return out;
        }
        let jobs = self.active_jobs() as u32;
        let held = points
            .iter()
            .enumerate()
            .filter(|&(i, p)| {
                p.hold.is_some_and(|h| {
                    !out.contains(&h) && assigned.iter().any(|&(j, pi)| pi == i && mine[j].id == h)
                })
            })
            .count() as u32;
        self.ai.counters.fort_build_plans += jobs + held;
        self.ai.counters.infantry_plans +=
            (mine.len() as u32 + jobs).saturating_sub(out.len() as u32);
        out
    }

    /// Point works for the squads assigned to own trenched points (points
    /// ascending, one job per point per plan); `s10b_only`: the S10b-ai
    /// kinds alone (the fortify-test hook).
    #[allow(clippy::too_many_arguments)]
    fn point_starts(
        &mut self,
        mine: &[Mine],
        assigned: &[(usize, usize)],
        points: &[PointEval],
        known: &[Known],
        kv: &[KnownVehicle],
        taken: &[SquadId],
        out: &mut Vec<SquadId>,
        s10b_only: bool,
    ) {
        let g = self.g;
        let s = &g.setup;
        let f = &s.rules.ai.fort;
        // Fix F3: a started job is in `forts` already (`job_room` counts
        // it); `out` only keeps a squad from a second job.
        // Point works, points ascending (S10c-ai F4: front points first;
        // their front works need no trench, `wants_at`).
        for i in self.point_order(points.len()) {
            let p = &points[i];
            if !self.job_room(0) {
                break;
            }
            if p.rel != Rel::Own {
                continue;
            }
            // S10c-ai F5: the point's `build_threat_max` gate keeps only
            // the non-front works (a front work has `front_threat`).
            let hot = p.threat >= f.build_threat_max;
            for k in self.wants_at(i, p.trench_done) {
                let front = self.front_work(i, k);
                if hot && !front {
                    continue;
                }
                // S10c-ai C8: no new front job while `front_slots` run.
                if front && !self.front_slot_free() {
                    continue;
                }
                let (Some(mut fort), Some(mut line)) = (fort_of(s, k), self.point_site(i, k, None))
                else {
                    continue;
                };
                let mut site = mid_of(s.forts[fort as usize].shape, &line);
                let s10b = s10b_fort(&s.forts[fort as usize]);
                if s10b_only && !s10b {
                    continue;
                }
                // S10b-ai R8 (`jobs_max`), R4 (blueprint cap), L4 (no
                // start near a young refusal).
                if s10b {
                    if !self.s10b_room()
                        || !self.under_limit(fort, &line)
                        || self.refused_here(site)
                    {
                        continue;
                    }
                    if matches!(
                        k,
                        WorkKind::ApField
                            | WorkKind::AtField
                            | WorkKind::AtRow2
                            | WorkKind::FrontAp
                    ) && self.recheck_plan()
                    {
                        self.ai.counters.lay_sites += 1;
                    }
                }
                // R6 at the row's first start (its sites are then fixed):
                // side 0, else the mirrored row, else refused.
                if k == WorkKind::Hedgehog
                    && self.hedgehog_side(i).is_none()
                    && self.can_build_any(mine, assigned, i, fort, site, known, kv, taken, out)
                {
                    let row = |side| self.hedgehog_row(i, side).filter(|r| !r.is_empty());
                    let feet = [row(0), row(1)];
                    match self.own_route_side(i, feet) {
                        Some(1) => {
                            let Some(p) = self.hedgehog_row(i, 1).and_then(|r| r.first().copied())
                            else {
                                continue;
                            };
                            let fwd = line[1] - line[0];
                            line = vec![p, self.terrain.clamp(p + fwd)];
                        }
                        Some(_) => {}
                        None => continue,
                    }
                }
                // S10c-ai C4, C5 at the belt's first start (its sites are
                // then fixed): hedgehogs when a free `build_heavy` squad can
                // take the first, else one log barrier line (S10c-4 (A));
                // side 0, else the other side, else refused.
                if k == WorkKind::Belt && self.belt_side(i).is_none() {
                    let takes = |f: u16, l: &[FxVec2], at: FxVec2| {
                        mine.iter().any(|m| {
                            !taken.contains(&m.id)
                                && !out.contains(&m.id)
                                && self.can_take_at(m, f, at, known, kv, true)
                                && self.front_carry_ok(m.id, f, l)
                        })
                    };
                    let heavy = takes(fort, &line, site);
                    let log = if heavy {
                        None
                    } else {
                        let Some(lf) = fort_of(s, WorkKind::LogBarrier) else {
                            continue;
                        };
                        let Some(l) = self.belt_log(i, 0) else {
                            continue;
                        };
                        let at = mid_of(FortShape::Line, &l);
                        if !self.under_limit(lf, &l) || self.refused_here(at) || !takes(lf, &l, at)
                        {
                            continue;
                        }
                        Some(lf)
                    };
                    let foot = |side| {
                        if log.is_some() {
                            self.belt_log(i, side)
                        } else {
                            self.belt_row(i, side)
                        }
                    };
                    let feet = [foot(0), foot(1)];
                    let Some(side) = self.belt_route_side(i, feet) else {
                        continue;
                    };
                    if let Some(lf) = log {
                        let Some(l) = self.belt_log(i, side) else {
                            continue;
                        };
                        fort = lf;
                        line = l;
                    } else {
                        let Some(p) = self.belt_row(i, side).and_then(|r| r.first().copied())
                        else {
                            continue;
                        };
                        let fwd = line[1] - line[0];
                        line = vec![p, self.terrain.clamp(p + fwd)];
                    }
                    if !self.under_limit(fort, &line) {
                        continue;
                    }
                    site = mid_of(s.forts[fort as usize].shape, &line);
                }
                let pick = assigned
                    .iter()
                    .filter(|&&(_, pi)| pi == i)
                    .map(|&(j, _)| &mine[j])
                    .filter(|m| {
                        !taken.contains(&m.id)
                            && !out.contains(&m.id)
                            && self.can_take_at(m, fort, site, known, kv, front)
                            && (!front || self.front_carry_ok(m.id, fort, &line))
                    })
                    .min_by_key(|m| {
                        let d = &s.forts[fort as usize];
                        (
                            self.ai.forts.contains_key(&m.id),
                            heavy_by_field(d, m.caps),
                            m.id,
                        )
                    });
                // S10b-ai Build E (`bunker_assign`, S10b-ai-D1): a wanted
                // bunker with no fit squad at its point takes one free
                // squad from anywhere (`build_heavy` first, then nearest).
                let pick = pick.or_else(|| {
                    if k != WorkKind::Bunker || s.rules.ai.obstacles.bunker_assign == 0 {
                        return None;
                    }
                    mine.iter()
                        .filter(|m| {
                            !taken.contains(&m.id)
                                && !out.contains(&m.id)
                                && self.can_take(m, fort, site, known, kv)
                        })
                        .min_by_key(|m| {
                            let d = &s.forts[fort as usize];
                            (
                                self.ai.forts.contains_key(&m.id),
                                heavy_by_field(d, m.caps),
                                (m.lead - site).length_sq_raw(),
                                m.id,
                            )
                        })
                });
                // S10c-ai C2 (`mine_assign`): a front work with no fit
                // squad at its point takes one free builder from anywhere.
                let pick = pick.or_else(|| {
                    if !front {
                        return None;
                    }
                    self.mine_assign_pick(mine, fort, &line, known, kv, taken, out)
                });
                let Some(m) = pick else { continue };
                let id = m.id;
                let site = mid_of(s.forts[fort as usize].shape, &line);
                if !self.can_take_at(m, fort, site, known, kv, front) {
                    continue;
                }
                // S10c-ai F6: a FrontAp / FrontWire line that holds part of
                // its work already (it starts past the saved line's start).
                let resumed = matches!(k, WorkKind::FrontAp | WorkKind::FrontWire)
                    && self.front_lane(i).is_some_and(|l| {
                        let full = if k == WorkKind::FrontAp { l.ap } else { l.wire };
                        line.first() != Some(&full[0])
                    });
                if self.new_fort_job(id, fort, i as u8, line) {
                    out.push(id);
                    // S10c-ai: a front job started (FrontAp, FrontWire, an
                    // AT row at a front point).
                    if front {
                        self.ai.counters.front_jobs += 1;
                    }
                    if resumed {
                        self.ai.counters.front_resumes += 1;
                    }
                    // C4: one count per belt job (a hedgehog or the line).
                    if k == WorkKind::Belt {
                        self.ai.counters.belt_jobs += 1;
                    }
                }
                // One job per point per plan: the next sees this one.
                break;
            }
        }
    }

    /// Whether a squad assigned to point `i` could take a job of `fort` at
    /// `site` (R6's searches and refusal wait for a builder).
    #[allow(clippy::too_many_arguments)]
    fn can_build_any(
        &self,
        mine: &[Mine],
        assigned: &[(usize, usize)],
        i: usize,
        fort: u16,
        site: FxVec2,
        known: &[Known],
        kv: &[KnownVehicle],
        taken: &[SquadId],
        out: &[SquadId],
    ) -> bool {
        assigned
            .iter()
            .filter(|&&(_, pi)| pi == i)
            .map(|&(j, _)| &mine[j])
            .any(|m| {
                !taken.contains(&m.id)
                    && !out.contains(&m.id)
                    && self.can_take(m, fort, site, known, kv)
            })
    }

    /// S10b-ai R5, R6: a log barrier across the arc of each own gun in
    /// Hold for `gun_hold_s` (one per gun), built by a builder within
    /// `builder_reach_m` of the gun.
    fn start_barriers(
        &mut self,
        mine: &[Mine],
        known: &[Known],
        kv: &[KnownVehicle],
        taken: &[SquadId],
        out: &mut Vec<SquadId>,
    ) {
        let g = self.g;
        let s = &g.setup;
        let Some(fort) = fort_of(s, WorkKind::LogBarrier) else {
            return;
        };
        let f = &s.rules.ai.fort;
        let hz = s.tick_hz.max(1) as u64;
        let reach = Fx::from_int(f.builder_reach_m as i32);
        let len = Fx::from(s.forts[fort as usize].max_length).min(Fx::from_int(BARRIER_LEN_M));
        let guns: Vec<(FxVec2, FxVec2)> = self
            .ai
            .guns
            .values()
            .filter(|j| {
                j.phase == super::GunPhase::Hold && self.tick >= j.since + f.gun_hold_s as u64 * hz
            })
            .map(|j| (j.site, j.facing))
            .collect();
        for (site, facing) in guns {
            if !self.s10b_room() {
                break;
            }
            let Some(first) = self.barrier_line(fort, site, facing, 0) else {
                continue;
            };
            let mid = (first[0] + first[1]).div_int(2);
            // One per gun: a barrier, its work or job before it already.
            if self.fort_near(fort, mid, len.mul_int(2))
                || !self.under_limit(fort, &first)
                || self.refused_here(mid)
            {
                continue;
            }
            let free = |m: &&Mine| {
                !taken.contains(&m.id)
                    && !out.contains(&m.id)
                    && near(m.lead, site, reach)
                    && self.can_take(m, fort, mid, known, kv)
            };
            if !mine.iter().any(|m| free(&m)) {
                continue;
            }
            let Some(i) = self.point_near(site) else {
                continue;
            };
            // R6: the mirrored site is the side farther from the route.
            let alt = self.own_route(i).and_then(|way| {
                let gap = |l: &Vec<FxVec2>| {
                    let c = (l[0] + l[1]).div_int(2);
                    way.iter()
                        .map(|&p| (p - c).length_sq_raw())
                        .min()
                        .unwrap_or(0)
                };
                let a = self.barrier_line(fort, site, facing, 1);
                let b = self.barrier_line(fort, site, facing, -1);
                match (a, b) {
                    (Some(a), Some(b)) => Some(if gap(&b) > gap(&a) { b } else { a }),
                    (a, b) => a.or(b),
                }
            });
            let line = match self.own_route_side(i, [Some(first), alt.clone()]) {
                Some(0) => self.barrier_line(fort, site, facing, 0),
                Some(_) => alt,
                None => None,
            };
            let Some(line) = line.filter(|l| self.under_limit(fort, l)) else {
                continue;
            };
            let at = (line[0] + line[1]).div_int(2);
            let pick = mine
                .iter()
                .filter(|m| {
                    !taken.contains(&m.id)
                        && !out.contains(&m.id)
                        && near(m.lead, site, reach)
                        && self.can_take(m, fort, at, known, kv)
                })
                .min_by_key(|m| (self.ai.forts.contains_key(&m.id), m.id));
            if let Some(m) = pick {
                let id = m.id;
                if self.new_fort_job(id, fort, NO_POINT, line) {
                    out.push(id);
                }
            }
        }
    }

    /// S10b-ai `own_blocked` (T115): own vehicles whose move failed (the
    /// lead stands within 4 m of its served path's end, which is not its
    /// goal: a reached path ends exactly on it; or it holds with no path
    /// more than 4 m from its goal) with an own `blocks_vehicles` piece within
    /// `OWN_BLOCKED_M`, and no vehicle route from where it stands to its goal
    /// (a halt for any other reason is no block); each (vehicle, piece) pair counts once, when it
    /// first appears (`AiState::blocked_pairs`, sorted, saved; S10c-ai C6:
    /// kept while both live, so it is not counted again when it reappears).
    /// Nothing with `obstacles` off.
    fn count_own_blocked(&mut self) {
        let g = self.g;
        let s = &g.setup;
        if s.rules.fortify.is_none() || s.rules.ai.obstacles.obstacles == 0 {
            return;
        }
        let me = self.me;
        let r = Fx::from_int(OWN_BLOCKED_M);
        let four = Fx::from_int(4);
        let mut pairs: Vec<(SquadId, u32)> = Vec::new();
        for (&id, q) in &g.squads {
            let nav = &q.nav;
            if q.owner != me
                || q.members.is_empty()
                || s.squad_types[q.type_idx as usize].vehicle.is_none()
                || nav.phase == NavPhase::PathPending
            {
                continue;
            }
            let at = self.ents.transform[q.lead(self.ents).index()].pos.xy();
            // S10b-ai E: a move that ended short (Holding with no path, its
            // goal more than 4 m off) counts too.
            let failed = match nav.path.last() {
                Some(&end) => !near(end, nav.goal, Fx::from_milli(500)) && near(end, at, four),
                None => {
                    matches!(nav.phase, NavPhase::Holding | NavPhase::Arriving)
                        && !near(at, nav.goal, four)
                }
            };
            if !failed {
                continue;
            }
            let start = pairs.len();
            for (&pid, p) in &g.fort.pieces {
                if p.owner == me
                    && s.forts
                        .get(p.fort as usize)
                        .is_some_and(|d| d.blocks_vehicles)
                    && near(p.pos, at, r)
                {
                    pairs.push((id, pid));
                }
            }
            // A real block only: the vehicle's own route from where it
            // stands to its goal must not exist. A halt beside an own piece
            // for another reason (a Stop, a target acquired, a wheeled
            // give-up elsewhere) leaves the goal reachable: not counted.
            // Searched only for the rare candidates near an own piece.
            if pairs.len() > start {
                let ty = s.squad_types[q.type_idx as usize]
                    .vehicle
                    .and_then(|v| s.vehicles.get(v as usize));
                let reachable = ty.is_some_and(|ty| {
                    let mut scratch = crate::path::PathScratch::default();
                    self.gun_route(at, nav.goal, ty, &mut scratch).is_some()
                });
                if reachable {
                    pairs.truncate(start);
                }
            }
        }
        pairs.sort_unstable();
        pairs.dedup();
        pairs.truncate(super::MAX_BLOCKED_PAIRS);
        let old = std::mem::take(&mut self.ai.blocked_pairs);
        let new = pairs
            .iter()
            .filter(|p| old.binary_search(p).is_err())
            .count();
        self.ai.counters.own_blocked += new as u32;
        // S10c-ai C6 (B1-2): a counted pair stays while its vehicle squad
        // and its piece both live, so a pair that clears and reappears
        // (the vehicle moved off and back) is not new.
        pairs.extend(old.into_iter().filter(|(sq, pid)| {
            g.squads.get(sq).is_some_and(|q| !q.members.is_empty())
                && g.fort.pieces.contains_key(pid)
        }));
        pairs.sort_unstable();
        pairs.dedup();
        pairs.truncate(super::MAX_BLOCKED_PAIRS);
        self.ai.blocked_pairs = pairs;
    }

    /// S10b-ai fortify-test hook (`AiHabit::Fort`): one plan of the S10b
    /// fort habit for `args.squad` alone: its job goes on (`fort_continue`)
    /// or, free, it may start an S10b job at `args.point`. `Some(Done)` /
    /// `Some(Failed)` on the plan its job ended (by its refusal).
    pub(super) fn drive_fort(&mut self, args: &super::DriveArgs) -> Option<HabitEnd> {
        let g = self.g;
        let me = self.me;
        let sq = args.squad?;
        let Some(q) = g
            .squads
            .get(&sq)
            .filter(|q| q.owner == me && !q.members.is_empty())
        else {
            return Some(HabitEnd::Failed);
        };
        let ty = &g.setup.squad_types[q.type_idx as usize];
        let alive = |ai: &super::AiState| {
            ai.forts
                .get(&sq)
                .is_some_and(|j| j.phase != FortPhase::Done)
        };
        let was = alive(self.ai);
        let refused = self.ai.counters.s10b_refused;
        let known = self.known_enemies();
        let kv = self.known_vehicles();
        let busy: Vec<SquadId> = self
            .ai
            .pending
            .iter()
            .filter_map(|c| super::squad_of(&c.kind))
            .collect();
        let mut mine = vec![Mine {
            id: sq,
            role: super::ai_kind(&g.setup, q.type_idx),
            lead: self.ents.transform[q.lead(self.ents).index()].pos.xy(),
            strength_pct: q.members.len() as u32 * 100 / ty.men.max(1) as u32,
            caps: ty.caps,
        }];
        self.fort_continue(&mut mine, &known, &kv, &busy);
        let ended = was && !alive(self.ai);
        if !mine.is_empty() && !alive(self.ai) && !busy.contains(&sq) {
            let mut points = self.evaluate_points(&known, &kv);
            for (i, p) in points.iter_mut().enumerate() {
                if i != args.point {
                    p.rel = Rel::Enemy;
                }
            }
            let assigned = vec![(0usize, args.point)];
            let mut out = Vec::new();
            self.point_starts(&mine, &assigned, &points, &known, &kv, &[], &mut out, true);
            if !alive(self.ai) && switched_on(&g.setup, WorkKind::LogBarrier) {
                self.start_barriers(&mine, &known, &kv, &[], &mut out);
            }
        }
        self.drive_crew(args);
        let end = if self.ai.counters.s10b_refused > refused {
            HabitEnd::Failed
        } else {
            HabitEnd::Done
        };
        ended.then_some(end)
    }

    /// The fortify-test hook's R7 manning: of `args.others` (own squads
    /// at the point) the one `bunker_crew` picks is sent to the bunker
    /// centre (as `order_infantry` sends it).
    fn drive_crew(&mut self, args: &super::DriveArgs) {
        let g = self.g;
        let me = self.me;
        let free: Vec<Mine> = args
            .others
            .iter()
            .filter_map(|&id| {
                let q = g
                    .squads
                    .get(&id)
                    .filter(|q| q.owner == me && !q.members.is_empty())?;
                let ty = &g.setup.squad_types[q.type_idx as usize];
                Some(Mine {
                    id,
                    role: super::ai_kind(&g.setup, q.type_idx),
                    lead: self.ents.transform[q.lead(self.ents).index()].pos.xy(),
                    strength_pct: q.members.len() as u32 * 100 / ty.men.max(1) as u32,
                    caps: ty.caps,
                })
            })
            .collect();
        let Some((at, id)) = self.bunker_crew(args.point, free.iter()) else {
            return;
        };
        let target = self.terrain.clamp(at);
        let going = matches!(g.squads[&id].order, Order::Move { target: t } if t == target);
        let sent = self
            .ai
            .pending
            .iter()
            .any(|c| super::squad_of(&c.kind) == Some(id));
        if !going && !sent {
            self.ai
                .emit(self.due, CommandKind::Move { squad: id, target });
        }
    }

    /// S10b-ai R8: whether another S10b job may start (`jobs_max`).
    /// S10b-ai Build D: counts a bunker job's end (`BUNKER_WHY` index
    /// `why`); other jobs are not counted.
    fn bunker_end(&mut self, job: &AiFortJob, why: usize) {
        if fort_of(&self.g.setup, WorkKind::Bunker) == Some(job.fort) {
            self.ai.counters.bunker_why[why] += 1;
        }
    }

    /// S10b-ai Build D: after a plan's point starts, with the bunker
    /// switched on, under `bunkers_max` and no bunker job, counts the
    /// first check that kept one from starting (`BUNKER_WHY` 0-11). Reads
    /// state only (no RNG); nothing with the switch off.
    #[allow(clippy::too_many_arguments)]
    fn bunker_why(
        &mut self,
        mine: &[Mine],
        assigned: &[(usize, usize)],
        points: &[PointEval],
        known: &[Known],
        kv: &[KnownVehicle],
        taken: &[SquadId],
        out: &[SquadId],
    ) {
        let s = &self.g.setup;
        let k = WorkKind::Bunker;
        if !s10b_on(s) || !switched_on(s, k) {
            return;
        }
        let Some(b) = fort_of(s, k) else { return };
        if self.fort_use(b) >= s.rules.ai.obstacles.bunkers_max {
            return;
        }
        let why = self.bunker_block(b, mine, assigned, points, known, kv, taken, out);
        self.ai.counters.bunker_why[why] += 1;
    }

    #[allow(clippy::too_many_arguments)]
    fn bunker_block(
        &self,
        b: u16,
        mine: &[Mine],
        assigned: &[(usize, usize)],
        points: &[PointEval],
        known: &[Known],
        kv: &[KnownVehicle],
        taken: &[SquadId],
        out: &[SquadId],
    ) -> usize {
        let s = &self.g.setup;
        let k = WorkKind::Bunker;
        let Some(i) = self.bunker_point() else {
            return 0;
        };
        let Some(p) = points.get(i) else { return 1 };
        if p.rel != Rel::Own || !p.trench_done || p.threat >= s.rules.ai.fort.build_threat_max {
            return 1;
        }
        let Some(line) = self.point_site(i, k, None) else {
            return 2;
        };
        if self.point_has(i, k) != Some(false) {
            return 2;
        }
        if !self.job_room(0) {
            return 3;
        }
        let site = mid_of(s.forts[b as usize].shape, &line);
        if !self.s10b_room() || !self.under_limit(b, &line) || self.refused_here(site) {
            return 4;
        }
        if self
            .ai
            .forts
            .values()
            .any(|j| j.point as usize == i && j.start == self.tick && !j.parked())
        {
            return 5;
        }
        let d = &s.forts[b as usize];
        // Build E: with `bunker_assign` every free squad is a candidate.
        let any = s.rules.ai.obstacles.bunker_assign != 0;
        let free: Vec<&Mine> = mine
            .iter()
            .enumerate()
            .filter(|&(j, _)| any || assigned.iter().any(|&(a, pi)| a == j && pi == i))
            .map(|(_, m)| m)
            .filter(|m| !taken.contains(&m.id) && !out.contains(&m.id))
            .collect();
        if free.is_empty() {
            return 6;
        }
        let fit: Vec<&&Mine> = free
            .iter()
            .filter(|m| {
                m.strength_pct >= 60
                    && builds(m.caps)
                    && (m.caps & (1u32 << d.capability) != 0 || heavy_by_field(d, m.caps))
            })
            .collect();
        if fit.is_empty() {
            return 7;
        }
        let need = d.materials_g;
        let carry: Vec<&&&Mine> = fit
            .iter()
            .filter(|m| {
                self.full_carry_g(m.id) >= need || self.truck_for(b) || self.at_hand(m.id, d, site)
            })
            .collect();
        if carry.is_empty() {
            return 8;
        }
        if !carry
            .iter()
            .any(|m| self.carried_g(m.id).saturating_add(self.room_for(b)) >= need)
        {
            return 9;
        }
        if carry
            .iter()
            .all(|m| self.fort_threat(known, kv, &[m.lead, site]))
        {
            return 10;
        }
        11
    }

    /// S10c-ai F1: this player's front points (`front_points_of`).
    pub(super) fn front_points(&self) -> Vec<usize> {
        front_points_of(self.g, self.me)
    }

    /// S10c-ai: `front_plans` and `front_calm_plans` (no known enemy
    /// within `[ai] threat_radius` of any front point). Every plan, every
    /// switch value: it reads state only (the gate of spec 8).
    pub(super) fn front_count(&mut self, known: &[Known], kv: &[KnownVehicle]) {
        let front = self.front_points();
        if front.is_empty() {
            return;
        }
        let at: Vec<FxVec2> = front
            .iter()
            .map(|&i| self.g.setup.map.points[i].pos.into())
            .collect();
        let calm = !self.fort_threat(known, kv, &at);
        let c = &mut self.ai.counters;
        c.front_plans += 1;
        if calm {
            c.front_calm_plans += 1;
        }
    }

    /// S10c-ai (spec 3, Build B item 4): `front_why` per plan with `front`
    /// on: `no_front` once with no front point; else, for each front
    /// point, the first check that kept its first outstanding front work
    /// from starting ([`Self::front_block`]). Called after `point_starts`;
    /// reads state only.
    #[allow(clippy::too_many_arguments)]
    fn front_why(
        &mut self,
        mine: &[Mine],
        assigned: &[(usize, usize)],
        points: &[PointEval],
        known: &[Known],
        kv: &[KnownVehicle],
        taken: &[SquadId],
        out: &[SquadId],
    ) {
        if !front_on(&self.g.setup) {
            return;
        }
        let front = self.front_points();
        let mut whys = Vec::new();
        if front.is_empty() {
            whys.push(0);
        }
        for i in front {
            if let Some(w) = self.front_block(i, mine, assigned, points, known, kv, taken, out) {
                whys.push(w);
            }
        }
        for w in whys {
            self.ai.counters.front_why[w] += 1;
        }
    }

    /// The `FRONT_WHY` index for front point `i`, or None when nothing is
    /// outstanding there (laid, built, switched off, a front job runs) or
    /// nothing keeps the first outstanding work (FrontAp, FrontWire, then
    /// the AT field on a `recheck_s` plan) from starting.
    #[allow(clippy::too_many_arguments)]
    fn front_block(
        &self,
        i: usize,
        mine: &[Mine],
        assigned: &[(usize, usize)],
        points: &[PointEval],
        known: &[Known],
        kv: &[KnownVehicle],
        taken: &[SquadId],
        out: &[SquadId],
    ) -> Option<usize> {
        let s = &self.g.setup;
        let o = &s.rules.ai.obstacles;
        let Some(lane) = self.front_lane(i) else {
            return Some(1);
        };
        if self.ai.forts.values().any(|j| {
            j.point as usize == i
                && !j.parked()
                && j.phase != FortPhase::Done
                && self.front_kind(j).is_some()
        }) {
            return None;
        }
        let at_f = fort_of(s, WorkKind::AtField);
        let k = if switched_on(s, WorkKind::FrontAp) && !lane.ap_done {
            WorkKind::FrontAp
        } else if switched_on(s, WorkKind::FrontWire) && !lane.wire_done {
            WorkKind::FrontWire
        } else if switched_on(s, WorkKind::AtField)
            && self.recheck_plan()
            && at_f.is_some_and(|f| self.field_has(i, WorkKind::AtField, f) == Some(false))
        {
            WorkKind::AtField
        } else {
            return None;
        };
        let f = fort_of(s, k)?;
        let Some(line) = self.point_site(i, k, None) else {
            return Some(2);
        };
        let cap = match k {
            WorkKind::FrontAp => self.front_fields() >= o.front_fields_max,
            WorkKind::AtField => self.fields_of(f) >= o.at_fields_max,
            _ => false,
        };
        if cap || !self.under_limit(f, &line) {
            return Some(3);
        }
        let d = &s.forts[f as usize];
        let site = mid_of(d.shape, &line);
        let s10b = s10b_fort(d);
        if s10b && self.refused_here(site) {
            return Some(4);
        }
        // C8: `front_slots` front jobs run (a slot of their own).
        if (s10b && !self.s10b_room()) || !self.front_slot_free() {
            return Some(5);
        }
        if !self.job_room(0) {
            return Some(6);
        }
        points.get(i)?;
        // C2: with `mine_assign` on, a fit builder from anywhere (so
        // `no_builder` means none at all).
        let anywhere = o.mine_assign != 0;
        let fit: Vec<&Mine> = assigned
            .iter()
            .filter(|&&(_, pi)| anywhere || pi == i)
            .map(|&(j, _)| &mine[j])
            .filter(|m| {
                !taken.contains(&m.id)
                    && !out.contains(&m.id)
                    && m.strength_pct >= 60
                    && builds(m.caps)
                    && (m.caps & (1u32 << d.capability) != 0 || heavy_by_field(d, m.caps))
            })
            .collect();
        if fit.is_empty() {
            return Some(7);
        }
        let carry: Vec<&&Mine> = fit
            .iter()
            .filter(|m| {
                self.front_carry_ok(m.id, f, &line)
                    && (self.full_carry_g(m.id) >= d.materials_g
                        || self.truck_for(f)
                        || self.at_hand(m.id, d, site))
            })
            .collect();
        if carry.is_empty() {
            return Some(8);
        }
        if !carry
            .iter()
            .any(|m| self.carried_g(m.id).saturating_add(self.room_for(f)) >= d.materials_g)
        {
            return Some(9);
        }
        // F5: the threat at the site comes before the builders' own (the
        // point's `build_threat_max` gate is not a front work's).
        if carry
            .iter()
            .all(|m| self.front_threat(known, kv, m.lead, site) == Some(true))
        {
            return Some(11);
        }
        if carry
            .iter()
            .all(|m| self.front_threat(known, kv, m.lead, site).is_some())
        {
            return Some(10);
        }
        None
    }

    /// S10c-ai (review finding 12): `mine_want_why` for each own point
    /// whose R1 (AP) or R2 (AT) field is switched on and not laid nor
    /// being laid (`MINE_WANT_WHY`). Called after `point_starts`, beside
    /// `bunker_why`; reads state only (the R2 site search fills only this
    /// plan's `at_sites` memo).
    #[allow(clippy::too_many_arguments)]
    fn mine_want_why(
        &mut self,
        mine: &[Mine],
        assigned: &[(usize, usize)],
        points: &[PointEval],
        known: &[Known],
        kv: &[KnownVehicle],
        taken: &[SquadId],
        out: &[SquadId],
    ) {
        let s = &self.g.setup;
        if !s10b_on(s) || !switched_on(s, WorkKind::ApField) {
            return;
        }
        let mut whys = Vec::new();
        for (i, p) in points.iter().enumerate() {
            if p.rel != Rel::Own {
                continue;
            }
            for k in [WorkKind::ApField, WorkKind::AtField] {
                if let Some(why) = self.mine_block(i, p, k, mine, assigned, known, kv, taken, out) {
                    whys.push(why);
                }
            }
        }
        for why in whys {
            self.ai.counters.mine_want_why[why] += 1;
        }
    }

    /// The `MINE_WANT_WHY` index for field kind `k` at own point `i`, or
    /// None when it is not counted (switched off, laid, a job lays it, an
    /// AT field off a `recheck_s` plan, or nothing keeps it from starting).
    #[allow(clippy::too_many_arguments)]
    fn mine_block(
        &self,
        i: usize,
        p: &PointEval,
        k: WorkKind,
        mine: &[Mine],
        assigned: &[(usize, usize)],
        known: &[Known],
        kv: &[KnownVehicle],
        taken: &[SquadId],
        out: &[SquadId],
    ) -> Option<usize> {
        let s = &self.g.setup;
        if !switched_on(s, k) || (k == WorkKind::AtField && !self.recheck_plan()) {
            return None;
        }
        let f = fort_of(s, k)?;
        if !p.trench_done || p.threat >= s.rules.ai.fort.build_threat_max {
            return Some(0);
        }
        match self.field_has(i, k, f) {
            Some(true) => return None,
            None => return Some(3),
            Some(false) => {}
        }
        if self
            .ai
            .forts
            .values()
            .any(|j| j.fort == f && j.point as usize == i && j.phase != FortPhase::Done)
        {
            return None;
        }
        if k == WorkKind::ApField
            && self.ap_after_wire()
            && self.point_has(i, WorkKind::Wire) != Some(true)
        {
            return Some(1);
        }
        if !self.field_wants(i).contains(&k) {
            return Some(2);
        }
        let Some(line) = self.point_site(i, k, None) else {
            return Some(3);
        };
        if !self.job_room(0) || !self.s10b_room() {
            return Some(5);
        }
        if !self.under_limit(f, &line) {
            return Some(2);
        }
        let site = mid_of(s.forts[f as usize].shape, &line);
        if self.refused_here(site) {
            return Some(4);
        }
        let d = &s.forts[f as usize];
        let fit: Vec<&Mine> = assigned
            .iter()
            .filter(|&&(_, pi)| pi == i)
            .map(|&(j, _)| &mine[j])
            .filter(|m| {
                !taken.contains(&m.id)
                    && !out.contains(&m.id)
                    && m.strength_pct >= 60
                    && builds(m.caps)
                    && (m.caps & (1u32 << d.capability) != 0 || heavy_by_field(d, m.caps))
                    && self.can_supply(m.id, f, site)
            })
            .collect();
        if fit.is_empty() {
            return Some(6);
        }
        if fit
            .iter()
            .all(|m| self.fort_threat(known, kv, &[m.lead, site]))
        {
            return Some(7);
        }
        None
    }

    fn s10b_room(&self) -> bool {
        let s = &self.g.setup;
        let n = self
            .ai
            .forts
            .iter()
            .filter(|&(&sq, j)| {
                j.phase != FortPhase::Done
                    && s.forts.get(j.fort as usize).is_some_and(s10b_fort)
                    && !self.bunker_yields(sq, j)
            })
            .count();
        n < s.rules.ai.obstacles.jobs_max as usize
    }

    /// B1-0 (owner S10b-ai-E1, `[ai.obstacles] bunker_yields`): a bunker
    /// job in Watch whose heavy step (over its builder's full carry) waits
    /// for material its own squad does not carry gives up the S10b job
    /// slot, so one other S10b job may start (`builders_max` and the S10b
    /// budget still apply). Reads state only; false with the switch off.
    fn bunker_yields(&self, sq: SquadId, j: &AiFortJob) -> bool {
        let s = &self.g.setup;
        if s.rules.ai.obstacles.bunker_yields == 0
            || j.phase != FortPhase::Watch
            || fort_of(s, WorkKind::Bunker) != Some(j.fort)
        {
            return false;
        }
        let Some(d) = s.forts.get(j.fort as usize) else {
            return false;
        };
        let me = self.me;
        j.work
            .and_then(|wid| self.g.fort.works.get(&wid))
            .is_some_and(|w| {
                w.owner == me
                    && w.builder == sq
                    && w.step == WorkStep::Wait
                    && self.carried_g(sq) == 0
                    && self.full_carry_g(sq) < d.materials_g
            })
    }

    /// S10b-ai L4: a fort goal at `at` within `refused_m` of a refusal
    /// younger than `refused_s`.
    fn refused_here(&self, at: FxVec2) -> bool {
        let s = &self.g.setup;
        let o = &s.rules.ai.obstacles;
        let hz = s.tick_hz.max(1) as u64;
        self.ai.refused_near(
            JobKind::Fort,
            at,
            self.tick,
            o.refused_s as u64 * hz,
            o.refused_m,
        )
    }

    /// S10b-ai: an S10b job of squad `id` ends (L1-L3): a failure writes
    /// its refusal at the job's site middle; `jobs_overdue` counts a job
    /// alive over 900 s (a log barrier job: [`OVERDUE_BARRIER_S`]; one
    /// plan of slack).
    fn s10b_end(&mut self, job: &AiFortJob, failed: bool) {
        let s = &self.g.setup;
        let hz = s.tick_hz.max(1) as u64;
        let overdue = if fort_of(s, WorkKind::LogBarrier) == Some(job.fort) {
            OVERDUE_BARRIER_S
        } else {
            OVERDUE_S
        };
        if self.tick.saturating_sub(job.start) > overdue * hz + self.plan_ticks() {
            self.ai.counters.jobs_overdue += 1;
        }
        if failed {
            let at = mid_of(s.forts[job.fort as usize].shape, &job.line);
            self.ai.refuse(JobKind::Fort, at, self.tick);
            self.ai.counters.fort_job_fails += 1;
            self.ai.counters.s10b_refused += 1;
        }
    }

    /// S10c-ai F6: a FrontAp / FrontWire job that ended with its work gone
    /// and its blueprint's mine, piece or wire section within
    /// `SITE_NEAR_M` of its line's end sets its lane's `ap_done` /
    /// `wire_done`.
    fn front_mark_done(&mut self, job: &AiFortJob) {
        let Some(k) = self.front_kind(job) else {
            return;
        };
        if !matches!(k, WorkKind::FrontAp | WorkKind::FrontWire) {
            return;
        }
        let me = self.me;
        let r = Fx::from_int(SITE_NEAR_M);
        let wire = job.line.last().is_some_and(|&end| {
            self.g.fort.wire.values().any(|w| {
                w.owner == me && w.fort == job.fort && near((w.a + w.b).div_int(2), end, r)
            })
        });
        let gone = job.work.is_none_or(|w| !self.g.fort.works.contains_key(&w));
        if !(self.s10b_laid(job) || (gone && wire)) {
            return;
        }
        if let Some(l) = self
            .ai
            .front_lanes
            .iter_mut()
            .find(|l| l.point == job.point)
        {
            if k == WorkKind::FrontAp {
                l.ap_done = true;
            } else {
                l.wire_done = true;
            }
        }
    }

    /// S10b-ai: whether a watched S10b work whose work is gone was laid
    /// (built): an own mine or piece of its blueprint within
    /// `SITE_NEAR_M` of its line's end.
    fn s10b_laid(&self, job: &AiFortJob) -> bool {
        // A point work's line is (site, facing point): its site.
        let point = self.g.setup.forts[job.fort as usize].shape != FortShape::Line;
        let end = if point {
            job.line.first()
        } else {
            job.line.last()
        };
        let Some(&end) = end else {
            return false;
        };
        let me = self.me;
        let fs = &self.g.fort;
        let r = Fx::from_int(SITE_NEAR_M);
        job.work.is_none_or(|w| !fs.works.contains_key(&w))
            && (fs
                .mines
                .values()
                .any(|m| m.owner == me && m.fort == job.fort && near(m.pos, end, r))
                || fs
                    .pieces
                    .values()
                    .any(|q| q.owner == me && q.fort == job.fort && near(q.pos, end, r)))
    }

    fn start_roof(
        &mut self,
        mine: &[Mine],
        known: &[Known],
        kv: &[KnownVehicle],
        taken: &[SquadId],
        out: &mut Vec<SquadId>,
    ) {
        let g = self.g;
        let s = &g.setup;
        if !self.ai.enemy_mortar_seen {
            return;
        }
        let Some(fort) = fort_of(s, WorkKind::Roof) else {
            return;
        };
        let me = self.me;
        let hq = g.supply6.structures.iter().find(|(id, st)| {
            st.owner == me
                && st.built
                && s.stocks[st.def as usize].kind == StockKind::Hq
                && !g.fort.pieces.values().any(|q| q.hq == Some(**id))
                && !g.fort.works.values().any(|w| w.hq == Some(**id))
        });
        let Some((_, st)) = hq else { return };
        if self.ai.forts.values().any(|j| j.fort == fort) {
            return;
        }
        let at = st.pos;
        let reach = Fx::from_int(s.rules.ai.fort.builder_reach_m as i32);
        let pick = mine
            .iter()
            .filter(|m| {
                !taken.contains(&m.id)
                    && !out.contains(&m.id)
                    && near(m.lead, at, reach)
                    && self.can_take(m, fort, at, known, kv)
            })
            .min_by_key(|m| (self.ai.forts.contains_key(&m.id), m.id));
        if let Some(m) = pick {
            let id = m.id;
            let line = vec![at, self.terrain.clamp(at + FxVec2::new(Fx::ONE, Fx::ZERO))];
            if self.new_fort_job(id, fort, NO_POINT, line) {
                out.push(id);
            }
        }
    }

    fn start_walls(
        &mut self,
        mine: &[Mine],
        known: &[Known],
        kv: &[KnownVehicle],
        taken: &[SquadId],
        out: &mut Vec<SquadId>,
    ) {
        let g = self.g;
        let s = &g.setup;
        let Some(fort) = fort_of(s, WorkKind::Wall) else {
            return;
        };
        let f = &s.rules.ai.fort;
        let hz = s.tick_hz.max(1) as u64;
        let reach = Fx::from_int(f.builder_reach_m as i32);
        let guns: Vec<(FxVec2, FxVec2)> = self
            .ai
            .guns
            .values()
            .filter(|j| {
                j.phase == super::GunPhase::Hold && self.tick >= j.since + f.gun_hold_s as u64 * hz
            })
            .map(|j| (j.site, j.facing))
            .collect();
        for (site, facing) in guns {
            if !self.job_room(0) {
                break;
            }
            let fwd = (facing - site).normalize_or_zero();
            if fwd == FxVec2::ZERO {
                continue;
            }
            let c = site + fwd * Fx::from_int(f.gun_wall_m as i32);
            let half = fwd.perp() * Fx::from_int(WALL_LEN_M).div_int(2);
            let line = vec![self.terrain.clamp(c - half), self.terrain.clamp(c + half)];
            let mid = (line[0] + line[1]).div_int(2);
            if self.fort_near(fort, mid, Fx::from_int(SITE_NEAR_M))
                || !self.under_limit(fort, &line)
            {
                continue;
            }
            let pick = mine
                .iter()
                .filter(|m| {
                    !taken.contains(&m.id)
                        && !out.contains(&m.id)
                        && near(m.lead, site, reach)
                        && self.can_take(m, fort, mid, known, kv)
                })
                .min_by_key(|m| (self.ai.forts.contains_key(&m.id), m.id));
            if let Some(m) = pick {
                let id = m.id;
                if self.new_fort_job(id, fort, NO_POINT, line) {
                    out.push(id);
                }
            }
        }
    }

    /// Starts a job (false when the saved map has no room: a squad not in
    /// it with `MAX_FORT_JOBS` jobs and parked refusals).
    fn new_fort_job(&mut self, id: SquadId, fort: u16, point: u8, line: Vec<FxVec2>) -> bool {
        if !self.ai.forts.contains_key(&id) && self.ai.forts.len() >= MAX_FORT_JOBS {
            return false;
        }
        // S10b-ai: S10b starts, and the L4 guard (fixed 300 s window).
        let s = &self.g.setup;
        if s.forts.get(fort as usize).is_some_and(s10b_fort) {
            let hz = s.tick_hz.max(1) as u64;
            let at = mid_of(s.forts[fort as usize].shape, &line);
            let near_m = s.rules.ai.obstacles.refused_m;
            if self
                .ai
                .refused_near(JobKind::Fort, at, self.tick, REORDER_WINDOW_S * hz, near_m)
            {
                self.ai.counters.refused_reorders += 1;
            }
            self.ai.counters.s10b_jobs += 1;
        }
        let mut job = AiFortJob {
            fort,
            point,
            line,
            phase: FortPhase::Load,
            start: self.tick,
            since: self.tick,
            work: None,
            tries: 0,
        };
        self.ai.counters.fort_jobs += 1;
        self.run_fort_job(id, &mut job);
        self.ai.forts.insert(id, job);
        true
    }

    /// One plan of a fort job (module docs).
    fn run_fort_job(&mut self, id: SquadId, job: &mut AiFortJob) {
        let g = self.g;
        let s = &g.setup;
        let hz = s.tick_hz.max(1) as u64;
        let tick = self.tick;
        let d = &s.forts[job.fort as usize];
        let sq = &g.squads[&id];
        let set = |job: &mut AiFortJob, p: FortPhase| {
            job.phase = p;
            job.since = tick;
        };
        if job.phase == FortPhase::Load {
            let need = match d.shape {
                FortShape::Line => d.materials_g * crate::fort::segments(&job.line, d.segment),
                _ => d.materials_g,
            };
            let carried = self.carried_g(id);
            let full = self.carry_room_g(id) == 0;
            // Fix F14: a step the squad cannot carry (and no truck can
            // bring) ends the job instead of a load or a walk.
            if self.full_carry_g(id) < d.materials_g
                && !self.truck_for(job.fort)
                && !self.at_hand(id, d, mid_of(d.shape, &job.line))
            {
                set(job, FortPhase::Done);
                return;
            }
            if carried >= need || (carried > 0 && full) {
                set(job, FortPhase::Go);
            } else if self.in_own_base(sq) {
                if self.load_pending(id) {
                    return;
                }
                if job.tries < LOAD_TRIES && self.load_builder(id, self.room_for(job.fort)) {
                    job.tries += 1;
                    return;
                }
                // Out of budget or tries: go with what it has, if anything.
                if carried == 0 {
                    set(job, FortPhase::Done);
                    return;
                }
                set(job, FortPhase::Go);
            } else if carried > 0 || self.truck_for(job.fort) {
                set(job, FortPhase::Go);
            } else if self.front_kind(job).is_some() {
                // S10c-ai F10: no front job sends its squad walking home.
                set(job, FortPhase::Done);
                return;
            } else {
                // Walk back to base for materials.
                let base = g.base_of(self.me);
                let going = matches!(sq.order, Order::Move { target } if near(target, base, Fx::from_int(20)));
                if !going {
                    self.ai.emit(
                        self.due,
                        CommandKind::Move {
                            squad: id,
                            target: base,
                        },
                    );
                    self.ai.counters.material_runs[2] += 1;
                }
                return;
            }
        }
        match job.phase {
            FortPhase::Go => {
                self.ai.emit(
                    self.due,
                    CommandKind::Fortify {
                        squad: id,
                        fort: job.fort,
                        line: job.line.clone(),
                    },
                );
                set(job, FortPhase::Order);
            }
            FortPhase::Order => {
                if self.load_pending(id) {
                    return;
                }
                let me = self.me;
                let work = g
                    .fort
                    .works
                    .iter()
                    .find(|(_, w)| w.owner == me && w.builder == id && w.fort == job.fort);
                if let Some((&wid, w)) = work {
                    job.work = Some(wid);
                    job.tries = w.step as u8;
                    set(job, FortPhase::Watch);
                    return;
                }
                // Refused (counted where it was refused): the wire tries
                // 25 m out once, everything else ends.
                job.tries = job.tries.saturating_add(1).min(MAX_FORT_TRIES);
                // S10c-ai F3: a front wire keeps its lane line (no retry).
                let wire = fort_of(s, WorkKind::Wire) == Some(job.fort)
                    && self.front_kind(job) != Some(WorkKind::FrontWire);
                let alt = (wire && job.point != NO_POINT && job.tries <= LOAD_TRIES + 1)
                    .then(|| {
                        self.point_site(job.point as usize, WorkKind::Wire, Some(WIRE_RETRY_M))
                    })
                    .flatten()
                    .filter(|l| *l != job.line);
                match alt {
                    Some(l) => {
                        job.line = l;
                        job.tries = MAX_FORT_TRIES.min(LOAD_TRIES + 2);
                        set(job, FortPhase::Go);
                    }
                    None => {
                        // Fix F4/F17: parked, remembered for `REFUSED_S`.
                        job.work = None;
                        job.tries = MAX_FORT_TRIES;
                        set(job, FortPhase::Done);
                    }
                }
            }
            FortPhase::Watch => {
                let me = self.me;
                let Some(w) = job
                    .work
                    .and_then(|wid| g.fort.works.get(&wid))
                    .filter(|w| w.owner == me && w.builder == id)
                else {
                    set(job, FortPhase::Done);
                    return;
                };
                // S10b-ai (spec 5): another work landed on the site: the
                // job ends (`ai_s10b_site_taken_ends_job`).
                if s10b_fort(d) {
                    let at = mid_of(d.shape, &job.line);
                    let r = Fx::from_int(SITE_TAKEN_M);
                    let taken = g.fort.works.iter().any(|(&k, o)| {
                        Some(k) != job.work
                            && near(mid_of(s.forts[o.fort as usize].shape, &o.line), at, r)
                    }) || g
                        .fort
                        .pieces
                        .values()
                        .any(|q| q.fort != job.fort && near(q.pos, at, r));
                    if taken {
                        set(job, FortPhase::Done);
                        return;
                    }
                }
                // A line goes on from its first segment not yet standing.
                let mut moved = job.tries != w.step as u8;
                if d.shape == FortShape::Line
                    && let [_, .., end] = w.line[..]
                {
                    let done = Fx::from(d.segment).mul_int(w.seg as i32);
                    let line = vec![crate::squad::point_along(&w.line, done), end];
                    moved |= line != job.line;
                    job.line = line;
                }
                // Fix F13: `since` is the tick the work's step (or segment)
                // last moved, or the squad last walked toward the site
                // (from base: minutes); `tries` the step then.
                let lead = self.ents.transform[sq.lead(self.ents).index()].pos.xy();
                let (at, _) = g.site_of(w);
                let (mut men, mut pinned) = (0u32, 0u32);
                for &m in sq.members.iter().filter(|m| self.ents.is_alive(**m)) {
                    men += 1;
                    pinned += self.ents.combat[m.index()].pinned as u32;
                }
                moved |= !near(lead, at, Fx::from_int(WATCH_NEAR_M))
                    && matches!(sq.nav.phase, NavPhase::Moving | NavPhase::PathPending)
                    && pinned * 2 < men;
                if moved {
                    job.tries = w.step as u8;
                    job.since = tick;
                }
                if w.step != WorkStep::Wait || self.carried_g(id) > 0 {
                    if tick > job.since.saturating_add(self.stall_ticks(id, d, w.step)) {
                        // No progress: the job ends, the squad is free
                        // (its next order removes the work) and the point
                        // is checked again by the next plans.
                        set(job, FortPhase::Done);
                    }
                    return;
                }
                let wid = job.work.unwrap_or(u32::MAX);
                let run_here = self.ai.material_run.is_some_and(|(_, r)| r == wid);
                let wait = f_wait(s) * hz;
                // S10b-ai Build D (the bunker never built in a match): an
                // S10b step over the builder's full carry (the 250 kg
                // bunker) only comes by truck: while an own truck job lives
                // it waits for runs (a 200 kg run leaves it short; the
                // truck's return trip is minutes) up to L1's `job_max_s`;
                // with none, `material_wait_s` from the last run. It used
                // to end when no truck was free at the step's first plan.
                let heavy = s10b_fort(d) && self.full_carry_g(id) < d.materials_g;
                if heavy && run_here {
                    job.since = tick;
                }
                if run_here
                    || (heavy && !self.ai.trucks.is_empty())
                    || ((heavy || self.truck_free()) && tick < job.since + wait)
                {
                    return;
                }
                // Fix F14: a step it cannot carry ends the job (no walk);
                // S10c-ai F10: so does every front job.
                if self.full_carry_g(id) < d.materials_g || self.front_kind(job).is_some() {
                    set(job, FortPhase::Done);
                    return;
                }
                // Walk back: the Move ends the work (its kg are lost).
                job.work = None;
                job.tries = 0;
                set(job, FortPhase::Load);
                let base = g.base_of(self.me);
                self.ai.emit(
                    self.due,
                    CommandKind::Move {
                        squad: id,
                        target: base,
                    },
                );
                self.ai.counters.material_runs[2] += 1;
            }
            FortPhase::Load | FortPhase::Done => {}
        }
    }

    /// Fix F13: ticks a watched work's `step` may stand still:
    /// `WATCH_STALL_X` x the step's time at squad `id`'s living men, plus
    /// `WATCH_SLACK_S` (the pit: its dig work at 100 points a man-second;
    /// the build: `work_ticks` x `work_men` over the men).
    fn stall_ticks(
        &self,
        id: SquadId,
        d: &frontline_content::setup::FortSetup,
        step: WorkStep,
    ) -> u64 {
        let s = &self.g.setup;
        let hz = s.tick_hz.max(1) as u64;
        let men = self.g.squads.get(&id).map_or(1, |q| {
            q.members.iter().filter(|m| self.ents.is_alive(**m)).count() as u64
        });
        let men = men.max(1);
        let nominal = match (step, d.pit) {
            (WorkStep::Dig, Some(pit)) => {
                let dig = s.rules.dig_profile(pit);
                let pts = (dig.work_per_m as u64 * Fx::from(dig.pit_line).raw().max(0) as u64)
                    >> Fx::FRAC_BITS;
                pts * hz / (100 * men)
            }
            _ => d.work_ticks as u64 * d.work_men as u64 / men,
        };
        nominal
            .saturating_mul(WATCH_STALL_X)
            .saturating_add(WATCH_SLACK_S * hz)
    }
}

/// A material run's end for its truck job: back to Load in base, else
/// Return.
fn end_run(job: &mut super::AiTruck, in_base: bool, tick: u64, pos: FxVec2) {
    job.phase = if in_base {
        super::truck::TruckPhase::Load
    } else {
        super::truck::TruckPhase::Return
    };
    job.since = tick;
    job.fails = 0;
    job.moved_to = pos;
}

fn f_wait(s: &MatchSetup) -> u64 {
    s.rules.ai.fort.material_wait_s as u64
}

/// S10b-ai (R2): a vehicle's track (else wheel) half-gauge: the largest
/// sideways anchor offset of its track or wheel components (0 without).
fn half_gauge(v: &frontline_content::vehicle::VehicleSetup) -> Fx {
    use frontline_content::vehicle::ComponentKind as K;
    v.components
        .iter()
        .filter(|c| {
            matches!(
                c.kind,
                K::TrackLeft | K::TrackRight | K::WheelsFront | K::WheelsRear
            )
        })
        .flat_map(|c| c.anchors.iter())
        .map(|a| crate::vehicle::v3(a.pos).y.abs())
        .max()
        .unwrap_or(Fx::ZERO)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s10a_cards_are_the_build_heavy_and_breach_cards() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2");
        let defs = frontline_content::ModDefs::load(&root).unwrap();
        let req = frontline_content::MatchRequest::skirmish(
            1,
            "ww2:germany",
            "ww2:usa",
            frontline_content::Difficulty::Normal,
        );
        let s = defs.resolve(&req).unwrap();
        let mut ids: Vec<&str> = s
            .squad_types
            .iter()
            .filter(|t| is_s10a_card(t))
            .map(|t| t.id.as_str())
            .collect();
        ids.sort_unstable();
        assert_eq!(
            ids,
            [
                "ww2:ger_baupioniergruppe",
                "ww2:ger_sturmpioniergruppe",
                "ww2:usa_assault_section"
            ]
        );
        for k in [
            WorkKind::Nest,
            WorkKind::Wire,
            WorkKind::Dugout,
            WorkKind::Roof,
            WorkKind::Wall,
        ] {
            assert!(fort_of(&s, k).is_some(), "{k:?}");
        }
    }
}
