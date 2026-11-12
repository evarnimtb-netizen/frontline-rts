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
    /// A Czech hedgehog (a point blueprint blocking vehicles).
    Hedgehog,
    /// A log barrier (a line blueprint blocking vehicles).
    LogBarrier,
    /// The bunker (a point blueprint on a `bunker_pit`).
    Bunker,
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
            WorkKind::ApField => d.shape == FortShape::Line && trigger(d) == Some(0),
            WorkKind::AtField => d.shape == FortShape::Line && trigger(d) == Some(1),
            WorkKind::Hedgehog => d.shape == FortShape::Point && d.blocks_vehicles,
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
                        WorkKind::Wire => d.shape == FortShape::Line && d.wire.is_some(),
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
            WorkKind::Hedgehog | WorkKind::LogBarrier => o.obstacles != 0,
            WorkKind::Bunker => o.bunker != 0,
        }
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
const REORDER_WINDOW_S: u64 = 300;
/// The fixed age of `jobs_overdue` (s; spec 3).
const OVERDUE_S: u64 = 900;

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
            // R2: across the enemy vehicle approach.
            WorkKind::AtField => self.at_site(i),
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
    /// along the trench away from its middle (`side` 1: mirrored, toward it).
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
        Some(
            (0..o.hedgehogs_row as i32)
                .map(|k| self.terrain.clamp(first + dir * step.mul_int(k)))
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
        let max = self.g.setup.rules.ai.fort.build_threat_max;
        for (i, p) in points.iter_mut().enumerate() {
            let ok = p.rel == Rel::Own && p.trench_done && p.threat < max;
            p.wants_works = ok && !self.point_wants(i).is_empty();
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

    /// Fix F12: a known enemy soldier or vehicle within `threat_radius` of
    /// any of `at` (the builder and its site).
    fn fort_threat(&self, known: &[Known], kv: &[KnownVehicle], at: &[FxVec2]) -> bool {
        let r = sq_m(self.g.setup.rules.ai.threat_radius.into());
        at.iter().any(|&p| {
            known.iter().any(|e| (e.pos - p).length_sq_raw() <= r)
                || kv.iter().any(|v| (v.pos - p).length_sq_raw() <= r)
        })
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
        (self.full_carry_g(sq) >= need || self.truck_free() || self.at_hand(sq, d, site))
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
        let d = &self.g.setup.forts[f as usize];
        m.strength_pct >= 60
            && builds(m.caps)
            && (m.caps & (1u32 << d.capability) != 0 || heavy_by_field(d, m.caps))
            && self.can_supply(m.id, f, site)
            && !self.fort_threat(known, kv, &[m.lead, site])
    }

    /// Fix F15: the builder each wanting own point holds (the assign
    /// bonus): one per point, points ascending, while `builders_max` has
    /// room, a free engineer (no `breach`) that could start the first
    /// wanted work it can, nearest first. Nothing with the habits off.
    pub(super) fn fort_holds(
        &mut self,
        mine: &[Mine],
        known: &[Known],
        kv: &[KnownVehicle],
        points: &mut [PointEval],
    ) {
        if !habits_on(&self.g.setup) {
            return;
        }
        let s = &self.g.setup;
        let mut held: Vec<SquadId> = Vec::new();
        for (i, pt) in points.iter_mut().enumerate() {
            if !pt.wants_works {
                continue;
            }
            if !self.job_room(held.len()) {
                break;
            }
            let pos = pt.pos;
            let pick = self.point_wants(i).into_iter().find_map(|k| {
                let (f, line) = (fort_of(s, k)?, self.point_site(i, k, None)?);
                let site = mid_of(s.forts[f as usize].shape, &line);
                mine.iter()
                    .filter(|m| {
                        m.role == super::AiKind::Engineer
                            && !m.can(Capability::Breach)
                            && !held.contains(&m.id)
                            && self.can_take(m, f, site, known, kv)
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
                pt.hold = Some(id);
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
        let s10b = self.s10b_used_g() * per_t;
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
        used += laid.saturating_sub(standing) * cheapest;
        used += fs
            .works
            .values()
            .filter(|w| w.owner == me && s.forts.get(w.fort as usize).is_some_and(s10b_fort))
            .map(|w| w.got_g as u64)
            .sum::<u64>();
        for (sq, j) in &self.ai.forts {
            if j.parked() || !s.forts.get(j.fort as usize).is_some_and(s10b_fort) {
                continue;
            }
            used += self.carried_g(*sq) as u64;
            used += self
                .ai
                .pending
                .iter()
                .filter_map(|c| match c.kind {
                    CommandKind::LoadMaterials { squad, kg } if squad == *sq => {
                        Some(kg as u64 * 1000)
                    }
                    _ => None,
                })
                .sum::<u64>();
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
        let spent = self.s10b_used_g() * per_t;
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
        if job.phase != super::truck::TruckPhase::Load || !in_base || !idle {
            return false;
        }
        let Some(wid) = self.waiting_work() else {
            return false;
        };
        let v = &s.vehicles[vs.type_idx as usize];
        let cap = v.supply.as_ref().map_or(0, |x| x.cap_g as u64);
        let held = vs.cargo_g.iter().map(|&x| x as u64).sum::<u64>() + vs.material_g as u64;
        let room = cap.saturating_sub(held).min(u32::MAX as u64) as u32;
        // Fix F18: kg still aboard from an earlier run count.
        let have_kg = vs.material_g / 1000;
        let kg = (f.truck_material_kg.saturating_sub(have_kg) * 1000)
            .min(room)
            .min(self.materials_room_g())
            / 1000;
        let need_kg = s.forts[g.fort.works[&wid].fort as usize]
            .materials_g
            .div_ceil(1000);
        if have_kg + kg == 0 || have_kg + kg < need_kg.min(f.truck_material_kg) {
            return false;
        }
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

    fn plan_ticks(&self) -> u64 {
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
        let job_max = g.setup.rules.ai.obstacles.job_max_s as u64 * hz;
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
            if s10b && tick >= job.start.saturating_add(job_max) {
                let job = job.clone();
                self.ai.forts.remove(&id);
                self.s10b_end(&job, true);
                continue;
            }
            let Some(k) = mine.iter().position(|m| m.id == id) else {
                if let Some(job) = self.ai.forts.remove(&id)
                    && s10b
                {
                    self.s10b_end(&job, false);
                }
                continue;
            };
            let m = &mine[k];
            let site = mid_of(g.setup.forts[job.fort as usize].shape, &job.line);
            let lost = job.point != NO_POINT
                && (g
                    .points
                    .get(job.point as usize)
                    .is_none_or(|c| c.owner != Some(me))
                    || g.setup.map.points.get(job.point as usize).is_some_and(|p| {
                        self.threat_at(known, p.pos.into())
                            >= g.setup.rules.ai.fort.build_threat_max
                    }));
            if m.strength_pct < 60 || lost || self.fort_threat(known, kv, &[m.lead, site]) {
                if let Some(job) = self.ai.forts.remove(&id)
                    && s10b
                {
                    self.s10b_end(&job, false);
                }
                continue;
            }
            let mut job = job.clone();
            let was = job.phase;
            self.run_fort_job(id, &mut job);
            if job.phase == FortPhase::Done && s10b {
                // L3: a refused order (parked), or a watched work that
                // stalled or went without being laid, is a failure; a load
                // out of budget or carry is not the goal's.
                let failed = job.parked() || (was == FortPhase::Watch && !self.s10b_laid(&job));
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
        self.point_starts(mine, assigned, points, known, kv, taken, &mut out, false);
        if switched_on(s, WorkKind::Roof) && self.job_room(0) {
            self.start_roof(mine, known, kv, taken, &mut out);
        }
        if switched_on(s, WorkKind::Wall) && self.job_room(0) {
            self.start_walls(mine, known, kv, taken, &mut out);
        }
        if switched_on(s, WorkKind::LogBarrier) && self.job_room(0) {
            self.start_barriers(mine, known, kv, taken, &mut out);
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
        // Point works, points ascending.
        for (i, p) in points.iter().enumerate() {
            if !self.job_room(0) {
                break;
            }
            if p.rel != Rel::Own || !p.trench_done || p.threat >= f.build_threat_max {
                continue;
            }
            for k in self.point_wants(i) {
                let (Some(fort), Some(mut line)) = (fort_of(s, k), self.point_site(i, k, None))
                else {
                    continue;
                };
                let site = mid_of(s.forts[fort as usize].shape, &line);
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
                    if matches!(k, WorkKind::ApField | WorkKind::AtField) && self.recheck_plan() {
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
                let pick = assigned
                    .iter()
                    .filter(|&&(_, pi)| pi == i)
                    .map(|&(j, _)| &mine[j])
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
                            m.id,
                        )
                    });
                let Some(m) = pick else { continue };
                let id = m.id;
                let site = mid_of(s.forts[fort as usize].shape, &line);
                if !self.can_take(m, fort, site, known, kv) {
                    continue;
                }
                if self.new_fort_job(id, fort, i as u8, line) {
                    out.push(id);
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
    /// goal: a reached path ends exactly on it) with an own `blocks_vehicles` piece within
    /// `OWN_BLOCKED_M`; each (vehicle, piece) pair counts once, when it
    /// first appears (`AiState::blocked_pairs`, sorted, not saved).
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
            let Some(&end) = nav.path.last() else {
                continue;
            };
            if near(end, nav.goal, Fx::from_milli(500)) || !near(end, at, four) {
                continue;
            }
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
        }
        pairs.sort_unstable();
        pairs.dedup();
        let old = std::mem::take(&mut self.ai.blocked_pairs);
        let new = pairs
            .iter()
            .filter(|p| old.binary_search(p).is_err())
            .count();
        self.ai.counters.own_blocked += new as u32;
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
    fn s10b_room(&self) -> bool {
        let s = &self.g.setup;
        let n = self
            .ai
            .forts
            .values()
            .filter(|j| {
                j.phase != FortPhase::Done && s.forts.get(j.fort as usize).is_some_and(s10b_fort)
            })
            .count();
        n < s.rules.ai.obstacles.jobs_max as usize
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
    /// alive over 900 s (one plan of slack).
    fn s10b_end(&mut self, job: &AiFortJob, failed: bool) {
        let s = &self.g.setup;
        let hz = s.tick_hz.max(1) as u64;
        if self.tick.saturating_sub(job.start) > OVERDUE_S * hz + self.plan_ticks() {
            self.ai.counters.jobs_overdue += 1;
        }
        if failed {
            let at = mid_of(s.forts[job.fort as usize].shape, &job.line);
            self.ai.refuse(JobKind::Fort, at, self.tick);
            self.ai.counters.s10b_refused += 1;
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
                && !self.truck_free()
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
            } else if carried > 0 || self.truck_free() {
                set(job, FortPhase::Go);
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
                let wire = fort_of(s, WorkKind::Wire) == Some(job.fort);
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
                if run_here || (self.truck_free() && tick < job.since + wait) {
                    return;
                }
                // Fix F14: a step it cannot carry ends the job (no walk).
                if self.full_carry_g(id) < d.materials_g {
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
