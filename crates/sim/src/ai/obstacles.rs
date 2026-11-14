//! S10b-ai: the AI's mines, obstacles, bunker, sweeps and demolitions
//! (`docs/slices/S10b-ai.md`; values in `[ai.obstacles]`).
//!
//! Build A holds the saved layout (spec 3), the refusal memory helpers (L3,
//! L4) and the fortify-test hook skeleton ([`AiHabit`], [`DriveArgs`],
//! [`HabitEnd`]; `World::drive_ai_habit`). No habit acts yet: every hook
//! arm ends `NotBuilt` and nothing in a match calls it.
//!
//! * **Refusals (L3).** A job that ends without success adds an
//!   [`AiRefusal`] (`AiState::refused`, sorted by kind, failed tick, x, y;
//!   at most [`MAX_REFUSED`]). Full: the entry with the oldest `failed`
//!   (ties: the first in sort order) is replaced, never a younger one.
//! * **Refused goals (L4).** No job starts and no command is sent for a goal
//!   within `refused_m` of a refusal of its kind younger than `refused_s`.
//!
//! Integer and fixed-point only, sorted vectors and `BTreeMap`s, no RNG.

use serde::{Deserialize, Serialize};

use super::{AiState, sq_m};
use crate::ecs::{PlayerId, SimId};
use crate::fx::{Fx, FxVec2};
use crate::game::{MatchState, SquadId};

/// Most sweep jobs one AI keeps (spec 3).
pub const MAX_SWEEPS: usize = 4;
/// Most demolition jobs one AI keeps (spec 3).
pub const MAX_DEMOLITIONS: usize = 4;
/// Most refused goals one AI remembers (L3).
pub const MAX_REFUSED: usize = 32;
/// Most (vehicle, own piece) pairs `AiState::blocked_pairs` keeps.
pub const MAX_BLOCKED_PAIRS: usize = 32;
/// A refusal is kept at least this long (s; spec 3: max(`refused_s`, 300)).
pub const REFUSAL_KEEP_MIN_S: u64 = 300;
/// Most pieces one demolition job lists (`demolish_pieces_max` is at most 3).
pub const MAX_DEMOLISH_PIECES: usize = 3;

/// What kind of goal a refusal belongs to (L3, L4).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum JobKind {
    /// A fort job (field, obstacle, bunker; `AiState::forts`).
    #[default]
    Fort = 0,
    /// A sweep (`AiState::sweeps`).
    Sweep = 1,
    /// A demolition (`AiState::demolitions`).
    Demolish = 2,
    /// A truck's goal (F8, L5).
    TruckGoal = 3,
}

/// Where a sweep job stands.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum SweepPhase {
    /// The sweeper walks to the lane's start.
    #[default]
    Go = 0,
    /// `ClearMines` is sent; the job watches `along` (L2).
    Sweep = 1,
    /// Finished (dropped at the next plan).
    Done = 2,
}

/// Where a demolition job stands.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum DemolishPhase {
    /// The squad walks to the next piece.
    #[default]
    Go = 0,
    /// `Demolish` is sent; the job waits for the charge (L2).
    Charge = 1,
    /// The piece is down: the vehicle re-paths.
    Repath = 2,
    /// Finished (dropped at the next plan).
    Done = 3,
}

/// One sweeper squad's lane (keyed by the squad; spec 3, R10).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AiSweep {
    /// The attacked point.
    pub point: u8,
    /// The lane, from `a` to `b` (at most `[fortify] lane_max_m`).
    pub a: FxVec2,
    pub b: FxVec2,
    /// Tick the job began (L1) and its phase began (L2).
    pub start: u64,
    pub since: u64,
    pub phase: SweepPhase,
    pub tries: u8,
}

/// One demolition squad's job (keyed by the squad; spec 3, R14).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AiDemolish {
    /// The own vehicle whose way is blocked.
    pub vehicle: SimId,
    /// The pieces to blow, in order (piece ids).
    pub pieces: [Option<u32>; MAX_DEMOLISH_PIECES],
    /// How many of them are done.
    pub done: u8,
    /// Tick the job began (L1) and its phase began (L2).
    pub start: u64,
    pub since: u64,
    pub phase: DemolishPhase,
}

/// A goal that failed (L3): no job or command for it within `refused_m`
/// while younger than `refused_s` (L4).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AiRefusal {
    pub kind: JobKind,
    pub at: FxVec2,
    /// The tick the goal failed.
    pub failed: u64,
}

impl AiRefusal {
    /// The sort key of `AiState::refused`.
    pub fn key(&self) -> (JobKind, u64, i32, i32) {
        (self.kind, self.failed, self.at.x.raw(), self.at.y.raw())
    }
}

/// An S10b-ai habit the fortify-test hook drives alone.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum AiHabit {
    /// Fort jobs of the S10b kinds (fields, obstacles, bunker).
    Fort,
    Sweep,
    Demolish,
    /// An S8b truck's job (the truck goal's liveness row, L5).
    Truck,
    /// Build E: `others` walk the point's swept lane in single file
    /// (`ai::sweep`); ends Done once all are past, Failed at once with no
    /// Done lane (or `lane_file` 0).
    LaneFile,
}

/// What the hook drives the habit with.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct DriveArgs {
    /// The squad doing the job (builder, sweeper, demolisher).
    pub squad: Option<SquadId>,
    /// The point it serves or attacks.
    pub point: usize,
    /// Other squads (attackers, the blocked vehicle's squad).
    pub others: Vec<SquadId>,
    /// A goal the caller fixes (a lane end, a piece's spot).
    pub at: Option<FxVec2>,
}

/// How a driven habit's job ended.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum HabitEnd {
    /// Done (laid, built, swept, a gap opened).
    Done,
    /// Ended without success (deadline, stall, refusal; L3).
    Failed,
    /// The habit is not built yet (Build A's skeleton).
    NotBuilt,
}

impl AiState {
    /// L3: remembers that a goal of `kind` at `at` failed at `tick`
    /// (sorted insert; full: the oldest `failed` is replaced).
    pub fn refuse(&mut self, kind: JobKind, at: FxVec2, tick: u64) {
        let r = AiRefusal {
            kind,
            at,
            failed: tick,
        };
        if self.refused.len() >= MAX_REFUSED {
            // The oldest failure (ties: the first in sort order); never a
            // younger one while an older exists.
            let Some(old) = (0..self.refused.len()).min_by_key(|&i| self.refused[i].failed) else {
                return;
            };
            self.refused.remove(old);
        }
        let k = r.key();
        let at = self.refused.partition_point(|x| x.key() <= k);
        self.refused.insert(at, r);
    }

    /// L4: whether a goal of `kind` at `at` lies within `near_m` m of a
    /// refusal of its kind younger than `refused_ticks` at `tick`.
    pub fn refused_near(
        &self,
        kind: JobKind,
        at: FxVec2,
        tick: u64,
        refused_ticks: u64,
        near_m: u32,
    ) -> bool {
        let r2 = sq_m(Fx::from_int(near_m.min(i32::MAX as u32) as i32));
        self.refused.iter().any(|r| {
            r.kind == kind
                && tick.saturating_sub(r.failed) < refused_ticks
                && (r.at - at).length_sq_raw() <= r2
        })
    }

    /// Drops refusals older than `keep_ticks` at `tick` (the caller passes
    /// max(`refused_s`, [`REFUSAL_KEEP_MIN_S`]) in ticks). Truck goals stay:
    /// they end when a truck loads again (L5, `ai::truck`).
    pub fn drop_stale_refusals(&mut self, tick: u64, keep_ticks: u64) {
        self.refused
            .retain(|r| r.kind == JobKind::TruckGoal || tick.saturating_sub(r.failed) < keep_ticks);
    }

    /// S10b-ai load checks (spec 6): sweeps, demolitions, refusals. As in
    /// `validate_fort`, a key squad gone since the last plan (wiped out) is
    /// allowed, and so is a demolition vehicle that left its squad (wrecked,
    /// bailed out): the next plan drops the job (`sweep_continue`: gone;
    /// `run_demolish`: no `job_vehicle` -> Quit).
    pub(super) fn validate_s10b(
        &self,
        g: &MatchState,
        player: PlayerId,
        tick: u64,
        ents_len: usize,
    ) -> bool {
        use frontline_content::setup::Capability;
        let np = g.points.len();
        let size = Fx::from_int(g.setup.map.size_m);
        let on_map = |p: &FxVec2| p.x >= Fx::ZERO && p.y >= Fx::ZERO && p.x <= size && p.y <= size;
        // A squad made so far: gone, or own and of a type with `cap`.
        let own_with = |id: &SquadId, cap: Capability| {
            id.0 < g.next_squad
                && g.squads.get(id).is_none_or(|q| {
                    q.owner == player
                        && g.setup
                            .squad_types
                            .get(q.type_idx as usize)
                            .is_some_and(|t| t.has(cap))
                })
        };
        // A vehicle: an entity no infantry squad holds (in no squad once
        // wrecked or abandoned; maybe re-crewed by anyone since).
        let vehicle = |v: SimId| {
            v.index() < ents_len
                && g.squads.values().all(|q| {
                    !q.members.contains(&v)
                        || g.setup
                            .squad_types
                            .get(q.type_idx as usize)
                            .is_some_and(|t| t.vehicle.is_some())
                })
        };
        let lane_max = g
            .setup
            .rules
            .fortify
            .as_ref()
            .map(|f| Fx::from_int(f.lane_max_m.min(i32::MAX as u32) as i32));
        self.sweeps.len() <= MAX_SWEEPS
            && self.sweeps.iter().all(|(id, j)| {
                own_with(id, Capability::ClearMines)
                    && (j.point as usize) < np
                    && on_map(&j.a)
                    && on_map(&j.b)
                    && lane_max.is_some_and(|m| (j.b - j.a).length() <= m)
                    && j.start <= j.since
                    && j.since <= tick
            })
            && self.demolitions.len() <= MAX_DEMOLITIONS
            && self.demolitions.iter().all(|(id, j)| {
                own_with(id, Capability::Demolish)
                    && vehicle(j.vehicle)
                    && j.pieces.iter().flatten().all(|&p| p < g.fort.next_piece)
                    && j.done as usize <= MAX_DEMOLISH_PIECES
                    && j.start <= j.since
                    && j.since <= tick
            })
            && self.refused.len() <= MAX_REFUSED
            && self.refused.windows(2).all(|w| w[0].key() <= w[1].key())
            && self.refused.iter().all(|r| r.failed <= tick)
            && self.suspects_seen as usize <= crate::fort::MAX_BLASTS
            && self.blocked_pairs.len() <= MAX_BLOCKED_PAIRS
            && self.blocked_pairs.windows(2).all(|w| w[0] < w[1])
            && self
                .blocked_pairs
                .iter()
                .all(|&(_, p)| p < g.fort.next_piece)
    }
}

impl super::Planner<'_> {
    /// The fortify-test hook (spec 3): one plan of habit `h` alone. Fort:
    /// Build B (`ai::fort::drive_fort`); Sweep and Demolish: Build D
    /// (`ai::sweep`, `ai::demolish`).
    fn drive_habit(&mut self, h: AiHabit, args: &DriveArgs) -> Option<HabitEnd> {
        match h {
            AiHabit::Fort => self.drive_fort(args),
            AiHabit::Sweep => self.drive_sweep(args),
            AiHabit::Demolish => self.drive_demolish(args),
            AiHabit::Truck => self.drive_truck(args),
            AiHabit::LaneFile => self.drive_lane_file(args),
        }
    }
}

impl super::Planner<'_> {
    /// R13: the enemy obstacles this AI knows ([`MatchState::known_obstacles`]).
    pub(super) fn known_obstacles(&self) -> Vec<u32> {
        self.g.known_obstacles(self.ents, self.me)
    }
}

impl MatchState {
    /// S10b-ai (R13): the ids (ascending) of enemy `blocks_vehicles`
    /// pieces within `obst_seen_m` of an own soldier or vehicle entity (by
    /// distance alone: no rays; the vehicle path layer is already global).
    pub(crate) fn known_obstacles(&self, ents: &crate::ecs::Entities, me: PlayerId) -> Vec<u32> {
        use crate::ecs::EntityKind;
        let f = self.fort();
        let enemy: Vec<(u32, FxVec2)> = f
            .pieces
            .iter()
            .filter(|(_, p)| {
                p.owner != me
                    && self
                        .setup
                        .forts
                        .get(p.fort as usize)
                        .is_some_and(|d| d.blocks_vehicles)
            })
            .map(|(&id, p)| (id, p.pos))
            .collect();
        if enemy.is_empty() {
            return Vec::new();
        }
        let r = Fx::from_int(self.setup.rules.ai.obstacles.obst_seen_m as i32);
        let r2 = sq_m(r);
        let own: Vec<FxVec2> = ents
            .kind()
            .iter()
            .enumerate()
            .filter(|&(i, k)| {
                matches!(k, EntityKind::Soldier | EntityKind::Vehicle) && ents.owner()[i] == me
            })
            .map(|(i, _)| ents.transform()[i].pos.xy())
            .collect();
        enemy
            .into_iter()
            .filter(|(_, at)| own.iter().any(|u| (*u - *at).length_sq_raw() <= r2))
            .map(|(id, _)| id)
            .collect()
    }

    /// S10b-ai fortify-test hook (`World::drive_ai_habit`): one plan of
    /// habit `h` of player `me` on the caller's (unsaved) state, if due
    /// (`ai.next_plan`). `Some(end)` once the job has ended.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn drive_habit_plan(
        &self,
        ai: &mut AiState,
        ents: &crate::ecs::Entities,
        terrain: &crate::terrain::Heightfield,
        tick: u64,
        me: PlayerId,
        h: AiHabit,
        args: &DriveArgs,
    ) -> Option<HabitEnd> {
        if tick < ai.next_plan {
            return None;
        }
        let ps = self.setup.players.get(me.0 as usize)?;
        let d = self.setup.rules.difficulty.get(ps.difficulty);
        ai.next_plan = tick + d.replan_ticks.max(1) as u64;
        let due = tick + d.reaction_ticks.max(1) as u64;
        super::Planner {
            g: self,
            ai,
            me,
            ents,
            terrain,
            due,
            tick,
            quality: d.decision_quality_pct.min(100) as i64,
            rng: crate::rng::RngStream::new(
                self.setup.seed,
                tick,
                SimId(me.0 as u32),
                crate::rng::Purpose::AI_PLAN,
            ),
            mortar_rays: super::mortar::PLAN_RAYS,
            at_sites: Default::default(),
            own_routes: Default::default(),
        }
        .drive_habit(h, args)
    }
}
