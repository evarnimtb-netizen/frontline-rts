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

use frontline_content::MatchSetup;
use frontline_content::setup::{
    Capability, DigProfile, Fixed, FortShape, SquadTypeSetup, StockKind,
};
use serde::{Deserialize, Serialize};

use super::{Known, KnownVehicle, Mine, MyTank, NO_POINT, PointEval, Rel, TaskKind, sq_m};
use crate::command::CommandKind;
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
pub(crate) enum WorkKind {
    Nest,
    Wire,
    Dugout,
    Roof,
    Wall,
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

/// The blueprint the AI uses for `k` (the first that fits; by shape and
/// pit, never by id).
pub(crate) fn fort_of(s: &MatchSetup, k: WorkKind) -> Option<u16> {
    s.forts
        .iter()
        // S10b R29: minefields and obstacles are never an AI habit's
        // blueprint (`ap_minefield` sorts before `sandbag_wall`).
        .position(|d| {
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
                }
        })
        .map(|i| i as u16)
}

/// Whether `[ai.fort]` lets the AI build `k` (`works` and the kind's own
/// switch, stage D; wire: `wire`).
pub(crate) fn switched_on(s: &MatchSetup, k: WorkKind) -> bool {
    let f = &s.rules.ai.fort;
    s.rules.fortify.is_some()
        && match k {
            WorkKind::Wire => f.wire != 0,
            WorkKind::Nest => f.works != 0 && f.nest != 0,
            WorkKind::Dugout => f.works != 0 && f.dugout != 0,
            WorkKind::Roof => f.works != 0 && f.roof != 0,
            WorkKind::Wall => f.works != 0 && f.sandbags != 0,
        }
}

/// Whether any fort habit is on (`works` or `wire`).
pub(super) fn habits_on(s: &MatchSetup) -> bool {
    let f = &s.rules.ai.fort;
    s.rules.fortify.is_some() && (f.works != 0 || f.wire != 0)
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
            WorkKind::Roof | WorkKind::Wall => None,
        }
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

    /// The works point `i` still wants, in build order (nest, wire,
    /// dugout), within `works_per_point`.
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
    fn can_supply(&self, sq: SquadId, f: u16) -> bool {
        let need = self.g.setup.forts[f as usize].materials_g;
        (self.full_carry_g(sq) >= need || self.truck_free())
            && self.carried_g(sq).saturating_add(self.materials_room_g()) >= need
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
            && m.caps & (1u32 << d.capability) != 0
            && self.can_supply(m.id, f)
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
                    .min_by_key(|m| ((m.lead - pos).length_sq_raw(), m.id))
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
        let spent = g.fort.accounts[self.me.0 as usize].materials_micro + pending;
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
    fn load_builder(&mut self, sq: SquadId) -> bool {
        let kg = (self.carry_room_g(sq).min(self.materials_room_g()) / 1000).min(u16::MAX as u32);
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
                self.load_builder(m.id);
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
        if !habits_on(s) {
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
        let ids: Vec<SquadId> = self.ai.forts.keys().copied().collect();
        for id in ids {
            let job = &self.ai.forts[&id];
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
            let Some(k) = mine.iter().position(|m| m.id == id) else {
                self.ai.forts.remove(&id);
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
                self.ai.forts.remove(&id);
                continue;
            }
            let mut job = job.clone();
            self.run_fort_job(id, &mut job);
            if job.phase == FortPhase::Done {
                if job.parked() {
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
        self.base_loads(mine);
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
                let (Some(fort), Some(line)) = (fort_of(s, k), self.point_site(i, k, None)) else {
                    continue;
                };
                let site = mid_of(s.forts[fort as usize].shape, &line);
                let pick = assigned
                    .iter()
                    .filter(|&&(_, pi)| pi == i)
                    .map(|&(j, _)| &mine[j])
                    .filter(|m| {
                        !taken.contains(&m.id)
                            && !out.contains(&m.id)
                            && self.can_take(m, fort, site, known, kv)
                    })
                    .min_by_key(|m| (self.ai.forts.contains_key(&m.id), m.id));
                let Some(m) = pick else { continue };
                let id = m.id;
                if self.new_fort_job(id, fort, i as u8, line) {
                    out.push(id);
                }
                // One job per point per plan: the next sees this one.
                break;
            }
        }
        if switched_on(s, WorkKind::Roof) && self.job_room(0) {
            self.start_roof(mine, known, kv, taken, &mut out);
        }
        if switched_on(s, WorkKind::Wall) && self.job_room(0) {
            self.start_walls(mine, known, kv, taken, &mut out);
        }
        // T97: plan-squads on a fort job, and (fix F15) builders the
        // assign bonus held at a point that got no job, over all
        // plan-squads given infantry tasks (those on a job included).
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
        let mut job = AiFortJob {
            fort,
            point,
            line,
            phase: FortPhase::Load,
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
            if self.full_carry_g(id) < d.materials_g && !self.truck_free() {
                set(job, FortPhase::Done);
                return;
            }
            if carried >= need || (carried > 0 && full) {
                set(job, FortPhase::Go);
            } else if self.in_own_base(sq) {
                if self.load_pending(id) {
                    return;
                }
                if job.tries < LOAD_TRIES && self.load_builder(id) {
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
