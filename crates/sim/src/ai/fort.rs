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
use frontline_content::setup::{Capability, DigProfile, FortShape, SquadTypeSetup, StockKind};
use serde::{Deserialize, Serialize};

use super::{Known, Mine, MyTank, NO_POINT, PointEval, Rel, TaskKind, sq_m};
use crate::command::CommandKind;
use crate::fort::{FortState, WorkStep};
use crate::fx::{Fx, FxVec2};
use crate::game::{Order, SquadId};

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
    pub tries: u8,
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
        .position(|d| match k {
            WorkKind::Nest => d.shape == FortShape::Point && d.pit == Some(DigProfile::MgNestPit),
            WorkKind::Dugout => d.shape == FortShape::Point && d.pit == Some(DigProfile::DugoutPit),
            WorkKind::Roof => d.shape == FortShape::OverHq,
            WorkKind::Wire => d.shape == FortShape::Line && d.wire.is_some(),
            WorkKind::Wall => d.shape == FortShape::Line && d.wire.is_none(),
        })
        .map(|i| i as u16)
}

/// Whether `[ai.fort]` lets the AI build `k`.
fn switched_on(s: &MatchSetup, k: WorkKind) -> bool {
    let f = &s.rules.ai.fort;
    s.rules.fortify.is_some()
        && match k {
            WorkKind::Wire => f.wire != 0,
            _ => f.works != 0,
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
                let c = mid + enemy * Fx::from_int(out);
                let half = along * Fx::from_int(f.wire_len_m as i32).div_int(2);
                let line = vec![tr.clamp(c - half), tr.clamp(c + half)];
                ((line[1] - line[0]).length() >= Fx::from_int(2)).then_some(line)
            }
            WorkKind::Roof | WorkKind::Wall => None,
        }
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
            WorkKind::Wire => {
                Fx::from_int(self.g.setup.rules.ai.fort.wire_len_m as i32).div_int(2)
                    + Fx::from_int(SITE_NEAR_M)
            }
            _ => Fx::from_int(SITE_NEAR_M),
        };
        // The wire's 25 m retry counts too.
        let alt = (k == WorkKind::Wire)
            .then(|| self.point_site(i, k, Some(WIRE_RETRY_M)))
            .flatten();
        Some(
            self.fort_near(f, mid_of(shape, &site), r)
                || alt.is_some_and(|l| self.fort_near(f, mid_of(shape, &l), r)),
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
            let wanted = switched_on(s, k)
                && match k {
                    // Wire once the nest is done or started.
                    WorkKind::Wire => nest == Some(true) || fort_of(s, WorkKind::Nest).is_none(),
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

    /// Marks own points that still want works (the assign bonus keeps
    /// builders there). Nothing with `works` and `wire` off.
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

    /// The dugout at own point `i` when its men should shelter now: built,
    /// the point marked shelled within `shelter_s`, no known enemy within
    /// `shelter_clear_m`.
    pub(super) fn shelter_at(&self, i: usize, known: &[Known]) -> Option<FxVec2> {
        let s = &self.g.setup;
        let f = &s.rules.ai.fort;
        if s.rules.fortify.is_none() || f.works == 0 {
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
            let end = match work {
                None => true,
                Some(w) => w.step == WorkStep::Build || late || unloaded,
            };
            if end {
                self.ai.material_run = None;
                job.phase = if in_base {
                    super::truck::TruckPhase::Load
                } else {
                    super::truck::TruckPhase::Return
                };
                job.since = tick;
                job.fails = 0;
                job.moved_to = t.pos;
                return true;
            }
            let Some(w) = work else { return true };
            if vs.material_g == 0 {
                return true;
            }
            let (site, _) = g.site_of(w);
            let reach = Fx::from_int(ft.material_reach_m as i32 - 3).max(Fx::ONE);
            if !near(t.pos, site, reach) {
                let back = (t.pos - site).normalize_or_zero();
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
        let kg = (f.truck_material_kg * 1000)
            .min(room)
            .min(self.materials_room_g())
            / 1000;
        let need_kg = s.forts[g.fort.works[&wid].fort as usize]
            .materials_g
            .div_ceil(1000);
        if kg == 0 || kg < need_kg.min(f.truck_material_kg) {
            return false;
        }
        self.ai.emit(
            self.due,
            CommandKind::LoadMaterials {
                squad: t.squad,
                kg: kg.min(u16::MAX as u32) as u16,
            },
        );
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
    /// leaves `mine`; jobs whose squad is gone, weak or threatened end.
    /// Also the base loads. Nothing with `works` and `wire` off.
    pub(super) fn fort_continue(
        &mut self,
        mine: &mut Vec<Mine>,
        known: &[Known],
        busy: &[SquadId],
    ) {
        let g = self.g;
        if !habits_on(&g.setup) {
            return;
        }
        let me = self.me;
        if self
            .ai
            .material_run
            .is_some_and(|(sq, _)| g.squads.get(&sq).is_none_or(|q| q.owner != me))
        {
            self.ai.material_run = None;
        }
        let ids: Vec<SquadId> = self.ai.forts.keys().copied().collect();
        for id in ids {
            if busy.contains(&id) && g.squads.get(&id).is_some_and(|q| q.owner == me) {
                continue;
            }
            let Some(k) = mine.iter().position(|m| m.id == id) else {
                self.ai.forts.remove(&id);
                continue;
            };
            let m = &mine[k];
            let r = sq_m(g.setup.rules.ai.threat_radius.into());
            let threat = known.iter().any(|e| (e.pos - m.lead).length_sq_raw() <= r);
            if m.strength_pct < 60 || threat {
                self.ai.forts.remove(&id);
                continue;
            }
            let mut job = self.ai.forts[&id].clone();
            self.run_fort_job(id, &mut job);
            if job.phase == FortPhase::Done {
                self.ai.forts.remove(&id);
                // Free again this plan: it gets an infantry task.
                continue;
            }
            self.ai.forts.insert(id, job);
            mine.remove(k);
        }
        self.base_loads(mine);
    }

    /// Starts new fort jobs for free builders (after assignment and the
    /// hunts): point works, the HQ roof, walls at guns. Returns the squads
    /// it took.
    pub(super) fn fort_start(
        &mut self,
        mine: &[Mine],
        assigned: &[(usize, usize)],
        points: &[PointEval],
        known: &[Known],
        taken: &[SquadId],
    ) -> Vec<SquadId> {
        let g = self.g;
        let s = &g.setup;
        let mut out = Vec::new();
        if !habits_on(s) {
            return out;
        }
        let f = &s.rules.ai.fort;
        let r = sq_m(s.rules.ai.threat_radius.into());
        let free = |m: &Mine, out: &[SquadId]| {
            m.strength_pct >= 60
                && builds(m.caps)
                && !taken.contains(&m.id)
                && !out.contains(&m.id)
                && !known.iter().any(|e| (e.pos - m.lead).length_sq_raw() <= r)
        };
        let able = |m: &Mine, fort: u16| {
            let d = &s.forts[fort as usize];
            m.caps & (1u32 << d.capability) != 0
        };
        let room = |this: &Self, out: &[SquadId]| {
            this.ai.forts.len() + out.len() < (f.builders_max as usize).min(MAX_FORT_JOBS)
        };
        // Point works, points ascending.
        for (i, p) in points.iter().enumerate() {
            if !room(self, &out) {
                break;
            }
            if p.rel != Rel::Own || !p.trench_done || p.threat >= f.build_threat_max {
                continue;
            }
            for k in self.point_wants(i) {
                if !room(self, &out) {
                    break;
                }
                let (Some(fort), Some(line)) = (fort_of(s, k), self.point_site(i, k, None)) else {
                    continue;
                };
                let pick = assigned
                    .iter()
                    .filter(|&&(_, pi)| pi == i)
                    .map(|&(j, _)| &mine[j])
                    .filter(|m| free(m, &out) && able(m, fort))
                    .min_by_key(|m| m.id);
                let Some(m) = pick else { continue };
                let id = m.id;
                self.new_fort_job(id, fort, i as u8, line);
                out.push(id);
                // One job per point per plan: the next sees this one.
                break;
            }
        }
        if f.works != 0 {
            let has_room = room(self, &out);
            self.start_roof(mine, &free, &able, &mut out, has_room);
            if room(self, &out) {
                self.start_walls(mine, &free, &able, &mut out, &room);
            }
        }
        // T97: plan-squads on a fort job over all plan-squads given
        // infantry tasks (those on a job included).
        let jobs = self.ai.forts.len() as u32;
        self.ai.counters.fort_build_plans += jobs;
        self.ai.counters.infantry_plans += mine.len() as u32 + jobs - out.len() as u32;
        out
    }

    fn start_roof(
        &mut self,
        mine: &[Mine],
        free: &dyn Fn(&Mine, &[SquadId]) -> bool,
        able: &dyn Fn(&Mine, u16) -> bool,
        out: &mut Vec<SquadId>,
        room: bool,
    ) {
        let g = self.g;
        let s = &g.setup;
        if !room || !self.ai.enemy_mortar_seen {
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
            .filter(|m| free(m, out) && able(m, fort) && near(m.lead, at, reach))
            .min_by_key(|m| m.id);
        if let Some(m) = pick {
            let id = m.id;
            let line = vec![at, self.terrain.clamp(at + FxVec2::new(Fx::ONE, Fx::ZERO))];
            self.new_fort_job(id, fort, NO_POINT, line);
            out.push(id);
        }
    }

    fn start_walls(
        &mut self,
        mine: &[Mine],
        free: &dyn Fn(&Mine, &[SquadId]) -> bool,
        able: &dyn Fn(&Mine, u16) -> bool,
        out: &mut Vec<SquadId>,
        room: &dyn Fn(&Self, &[SquadId]) -> bool,
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
            if !room(self, out) {
                break;
            }
            let fwd = (facing - site).normalize_or_zero();
            if fwd == FxVec2::ZERO {
                continue;
            }
            let c = site + fwd * Fx::from_int(f.gun_wall_m as i32);
            let half = fwd.perp() * Fx::from_int(WALL_LEN_M).div_int(2);
            let line = vec![self.terrain.clamp(c - half), self.terrain.clamp(c + half)];
            if self.fort_near(
                fort,
                (line[0] + line[1]).div_int(2),
                Fx::from_int(SITE_NEAR_M),
            ) {
                continue;
            }
            let pick = mine
                .iter()
                .filter(|m| free(m, out) && able(m, fort) && near(m.lead, site, reach))
                .min_by_key(|m| m.id);
            if let Some(m) = pick {
                let id = m.id;
                self.new_fort_job(id, fort, NO_POINT, line);
                out.push(id);
            }
        }
    }

    fn new_fort_job(&mut self, id: SquadId, fort: u16, point: u8, line: Vec<FxVec2>) {
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
                if let Some((&wid, _)) = work {
                    job.work = Some(wid);
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
                    None => set(job, FortPhase::Done),
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
                if d.shape == FortShape::Line
                    && let [_, .., end] = w.line[..]
                {
                    let done = Fx::from(d.segment).mul_int(w.seg as i32);
                    job.line = vec![crate::squad::point_along(&w.line, done), end];
                }
                if w.step != WorkStep::Wait || self.carried_g(id) > 0 {
                    job.since = tick;
                    return;
                }
                let wid = job.work.unwrap_or(u32::MAX);
                let run_here = self.ai.material_run.is_some_and(|(_, r)| r == wid);
                let wait = f_wait(s) * hz;
                if run_here || (self.truck_free() && tick < job.since + wait) {
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
