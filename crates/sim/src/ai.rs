//! AI commander (S1, §9 layer 1): a sim-side player controller.
//!
//! Each AI player re-plans every `replan_ticks` (its difficulty), serially in
//! player order in the Intent stage, reading only sim state and what its
//! player can see (`Combat::seen_by`). A plan emits [`CommandKind`]s into
//! the player's pending queue, due `reaction_ticks` later (at least one
//! tick); `World::step` merges due AI commands with the human commands of
//! that tick, sorts them by `(player, seq)` and applies them through
//! `MatchState::apply`, exactly like a human's. AI commands carry their own
//! per-player sequence numbers.
//!
//! A plan:
//!
//! 1. **Retreat.** Squads below `retreat_strength_pct` with a spotted enemy
//!    within `threat_radius` fall back to base.
//! 2. **Buy.** The role furthest below the target mix (`mix_pct`, counting
//!    squads alive, called in and about to be bought) is bought when the
//!    manpower left after pending buys and the pop cap allow; at most
//!    [`MAX_BUYS_PER_PLAN`] per plan.
//! 3. **Points.** Each point gets a priority from its value, owner, distance
//!    from base and threat (spotted enemies near it), and a demand (squads
//!    wanted). Infantry squads are assigned greedily by priority minus
//!    travel distance plus role and continuity bonuses, first up to each
//!    point's demand, then leftovers to their best point. Own points are
//!    defended (Move), others captured or attacked (AttackMove).
//! 4. **Dig.** An engineer squad sent to an own point whose trench is not
//!    finished digs it: `dig_length` long, `dig_offset` in front of the
//!    point toward the nearest spotted enemy (else the nearest enemy point,
//!    else the enemy base). Defenders of a point with a finished trench
//!    stand in it.
//! 5. **Mortars.** A mortar team fires `barrage_rounds` at the best spotted
//!    enemy group in range (bigger groups and men in trenches score
//!    higher; never within [`DANGER_CLOSE_M`] of own men); otherwise it
//!    moves to a support spot [`SUPPORT_BACK_M`] behind the most important
//!    point.
//! 6. **Throws** (S1b, `crate::items`), at most one per squad per
//!    `throw_cooldown`, for squads not retreating, first that applies:
//!    * *satchel:* engineers throw at a spotted enemy in a trench within
//!      satchel range of a carrier;
//!    * *frag:* at a spotted enemy in a trench, or with at least
//!      `frag_group_men` spotted men within [`FRAG_GROUP_M`] of him
//!      (bigger groups, then trenches, then the nearest first), within
//!      frag range of a man;
//!    * *smoke:* a moving or attack-moving squad with a third of its men
//!      pinned throws smoke halfway toward the nearest spotted enemy (at
//!      most to the smoke's range).
//!
//!    `Throw` names every man who could throw it; the sim picks the
//!    thrower. Throwers end up detached: a squad that is walking again
//!    while all its detached men are done throwing is regrouped.
//!
//! An order is only sent when the squad's task changed or its order no
//! longer matches the task, and never while a command for that squad is
//! still pending, so squads are not re-pathed every plan.
//!
//! Difficulty: `income_pct` (in `crate::rules`), `reaction_ticks`,
//! `replan_ticks`, and `decision_quality_pct` q: scores get uniform noise of
//! ±2·(100 − q), a needed retreat is ordered with probability 50 + q/2 %,
//! and a buy picks a random role with probability (100 − q)/2 %.
//!
//! Everything is integer or Q16.16, iterates squads, points and soldiers in
//! ascending order, and draws randomness from `rng(seed, tick, player,
//! AI_PLAN)`.

use std::collections::BTreeMap;

use frontline_content::SquadRole;
use serde::{Deserialize, Serialize};

use crate::command::CommandKind;
use crate::ecs::{Entities, PlayerId, SimId};
use crate::fx::{Fx, FxVec2};
use crate::game::{MAX_CALL_IN_QUEUE, MAX_DIG_POINTS, MatchState, NavPhase, Order, SquadId};
use crate::rng::{Purpose, RngStream};
use crate::terrain::Heightfield;
use frontline_content::ItemKind;

/// Most AI commands waiting for their reaction delay.
pub const MAX_PENDING: usize = 256;
/// Most squads bought per plan.
pub const MAX_BUYS_PER_PLAN: usize = 2;
/// Most trenches the AI remembers (one per point is planned).
pub const MAX_TRENCHES: usize = 64;
/// Mortars never fire this close to own men (m).
pub const DANGER_CLOSE_M: i32 = 35;
/// Mortar support spot: this far behind the point it supports (m).
pub const SUPPORT_BACK_M: i32 = 200;
/// No point index (support and retreat tasks).
pub const NO_POINT: u8 = u8::MAX;
/// Frag targets: spotted men within this distance count as one group (m).
pub const FRAG_GROUP_M: i32 = 5;

/// A command waiting for the AI's reaction delay.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AiCommand {
    /// Applied at the start of this tick.
    pub due: u64,
    pub seq: u32,
    pub kind: CommandKind,
}

/// What the AI wants a squad to do.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[repr(u8)]
pub enum TaskKind {
    Capture = 0,
    Attack = 1,
    Defend = 2,
    Dig = 3,
    Barrage = 4,
    Support = 5,
    Retreat = 6,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Task {
    pub kind: TaskKind,
    /// Capture point index, or [`NO_POINT`].
    pub point: u8,
    pub target: FxVec2,
}

/// A trench the AI ordered at a point (kept so the line stays put).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AiTrench {
    pub point: u8,
    pub line: [FxVec2; 2],
}

/// One AI player's commander state. Saved, loaded and hashed with the match.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AiState {
    /// Next tick that runs a plan.
    pub next_plan: u64,
    pub next_seq: u32,
    /// In due order.
    pub pending: Vec<AiCommand>,
    pub tasks: BTreeMap<SquadId, Task>,
    pub trenches: Vec<AiTrench>,
    /// Commands emitted, and how many of them `apply` rejected.
    pub issued: u32,
    pub rejected: u32,
    /// Tick of each squad's last throw order, while it cools down.
    pub last_throw: BTreeMap<SquadId, u64>,
}

impl AiState {
    pub(crate) fn new(first_plan: u64) -> AiState {
        AiState {
            next_plan: first_plan,
            next_seq: 0,
            pending: Vec::new(),
            tasks: BTreeMap::new(),
            trenches: Vec::new(),
            issued: 0,
            rejected: 0,
            last_throw: BTreeMap::new(),
        }
    }

    fn emit(&mut self, due: u64, kind: CommandKind) {
        if self.pending.len() >= MAX_PENDING {
            return;
        }
        self.pending.push(AiCommand {
            due,
            seq: self.next_seq,
            kind,
        });
        self.next_seq = self.next_seq.wrapping_add(1);
        self.issued += 1;
    }

    /// Checks a loaded AI state (untrusted saves) at `tick`.
    pub(crate) fn validate(&self, g: &MatchState, tick: u64) -> Result<(), &'static str> {
        let np = g.points.len();
        let bad_squad = |id: &SquadId| id.0 >= g.next_squad;
        if self.pending.len() > MAX_PENDING
            || self.trenches.len() > MAX_TRENCHES
            || self.rejected > self.issued
            || self.tasks.keys().any(bad_squad)
            || self.last_throw.keys().any(bad_squad)
            || self.last_throw.values().any(|&t| t > tick)
            || self
                .tasks
                .values()
                .any(|t| t.point != NO_POINT && t.point as usize >= np)
            || self.trenches.iter().any(|t| t.point as usize >= np)
            || self.pending.windows(2).any(|w| w[0].due > w[1].due)
            || self.pending.iter().any(|c| match &c.kind {
                CommandKind::Dig { line, .. } => line.len() > MAX_DIG_POINTS,
                // `buy` indexes the squad types by pending call-ins.
                CommandKind::CallIn { squad_type } => {
                    *squad_type as usize >= g.setup.squad_types.len()
                }
                CommandKind::Throw { units, .. } | CommandKind::ThrowEach { units, .. } => {
                    units.len() > crate::unit::MAX_UNITS
                }
                CommandKind::MoveSoldier { .. }
                | CommandKind::Spawn { .. }
                | CommandKind::Despawn { .. } => true,
                _ => false,
            })
        {
            return Err("AI state out of range");
        }
        Ok(())
    }
}

/// The squad a command addresses, if any.
fn squad_of(kind: &CommandKind) -> Option<SquadId> {
    match kind {
        CommandKind::Move { squad, .. }
        | CommandKind::AttackMove { squad, .. }
        | CommandKind::Stop { squad }
        | CommandKind::Retreat { squad }
        | CommandKind::Dig { squad, .. }
        | CommandKind::Barrage { squad, .. } => Some(*squad),
        _ => None,
    }
}

fn role_index(r: SquadRole) -> usize {
    match r {
        SquadRole::Rifle => 0,
        SquadRole::Mg => 1,
        SquadRole::Engineer => 2,
        SquadRole::Mortar => 3,
    }
}

/// Squared distance bound for `m` metres, comparable with `length_sq_raw`.
fn sq_m(m: Fx) -> u64 {
    FxVec2::new(m, Fx::ZERO).length_sq_raw()
}

fn dist_m(a: FxVec2, b: FxVec2) -> i64 {
    (a - b).length().floor_int() as i64
}

/// A spotted enemy soldier.
struct Known {
    id: SimId,
    pos: FxVec2,
    squad: u32,
    in_trench: bool,
}

/// One of the AI's own squads at plan time.
struct Mine {
    id: SquadId,
    role: SquadRole,
    lead: FxVec2,
    strength_pct: u32,
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum Rel {
    Own,
    Neutral,
    Enemy,
}

struct PointEval {
    pos: FxVec2,
    rel: Rel,
    threat: u32,
    priority: i64,
    demand: u32,
    assigned: u32,
    /// Index into `AiState::trenches`, and whether it is finished.
    trench: Option<usize>,
    trench_done: bool,
}

impl MatchState {
    /// Moves due AI commands (all AI players, in player order) into `out`.
    pub(crate) fn take_ai_commands(&mut self, tick: u64, out: &mut Vec<crate::Command>) {
        for (p, ai) in self.ai.iter_mut().enumerate() {
            let Some(ai) = ai else { continue };
            let n = ai.pending.iter().take_while(|c| c.due <= tick).count();
            out.extend(ai.pending.drain(..n).map(|c| crate::Command {
                player: PlayerId(p as u8),
                seq: c.seq,
                kind: c.kind,
            }));
        }
    }

    /// Counts a rejected AI command for `player`.
    pub(crate) fn ai_rejected(&mut self, player: PlayerId) {
        if let Some(Some(ai)) = self.ai.get_mut(player.0 as usize) {
            ai.rejected += 1;
        }
    }

    /// Runs the plans that are due this tick (module docs).
    pub(crate) fn run_ai(&mut self, ents: &Entities, terrain: &Heightfield, tick: u64) {
        if self.outcome != crate::game::Outcome::Running {
            return;
        }
        for p in 0..self.ai.len() {
            let Some(mut ai) = self.ai[p].take() else {
                continue;
            };
            if self.players[p].defeat.is_none() && tick >= ai.next_plan {
                let ps = &self.setup.players[p];
                let d = self.setup.rules.difficulty.get(ps.difficulty);
                ai.next_plan = tick + d.replan_ticks.max(1) as u64;
                let due = tick + d.reaction_ticks.max(1) as u64;
                Planner {
                    g: self,
                    ai: &mut ai,
                    me: PlayerId(p as u8),
                    ents,
                    terrain,
                    due,
                    tick,
                    quality: d.decision_quality_pct.min(100) as i64,
                    rng: RngStream::new(self.setup.seed, tick, SimId(p as u32), Purpose::AI_PLAN),
                }
                .plan();
            }
            self.ai[p] = Some(ai);
        }
    }
}

struct Planner<'a> {
    g: &'a MatchState,
    ai: &'a mut AiState,
    me: PlayerId,
    ents: &'a Entities,
    terrain: &'a Heightfield,
    due: u64,
    tick: u64,
    quality: i64,
    rng: RngStream,
}

impl Planner<'_> {
    fn plan(&mut self) {
        let g = self.g;
        let me = self.me;
        self.ai
            .tasks
            .retain(|id, _| g.squads.get(id).is_some_and(|s| s.owner == me));
        let busy: Vec<SquadId> = self
            .ai
            .pending
            .iter()
            .filter_map(|c| squad_of(&c.kind))
            .collect();

        let known = self.known_enemies();
        let mut mine = Vec::new();
        for (&id, sq) in &g.squads {
            if sq.owner != me || busy.contains(&id) {
                continue;
            }
            let ty = &g.setup.squad_types[sq.type_idx as usize];
            mine.push(Mine {
                id,
                role: ty.role,
                lead: self.ents.transform[sq.lead(self.ents).index()].pos.xy(),
                strength_pct: sq.members.len() as u32 * 100 / ty.men.max(1) as u32,
            });
        }

        self.retreats(&mut mine, &known);
        self.buy();
        let mut points = self.evaluate_points(&known);
        let assigned = self.assign(&mine, &mut points);
        self.order_infantry(&mine, &assigned, &mut points, &known);
        self.mortars(&mine, &points, &known);
        self.throws(&mine, &known);
    }

    /// Uniform integer in `[0, n)`.
    fn below(&mut self, n: u32) -> i64 {
        crate::rng::below(self.rng.next_u64(), n) as i64
    }

    /// Uniform noise of ±2·(100 − q).
    fn noise(&mut self) -> i64 {
        let a = 2 * (100 - self.quality);
        if a == 0 {
            return 0;
        }
        self.rng.range_i32(-a as i32, a as i32) as i64
    }

    fn known_enemies(&self) -> Vec<Known> {
        let bit = 1u16 << self.me.0;
        self.ents
            .soldiers()
            .filter(|id| {
                let i = id.index();
                self.ents.owner[i] != self.me && self.ents.combat[i].seen_by & bit != 0
            })
            .map(|id| {
                let i = id.index();
                Known {
                    id,
                    pos: self.ents.transform[i].pos.xy(),
                    squad: self.ents.squad[i].map_or(u32::MAX, |s| s.0),
                    in_trench: self.ents.combat[i].in_trench,
                }
            })
            .collect()
    }

    fn threat_at(&self, known: &[Known], at: FxVec2) -> u32 {
        let r = sq_m(self.g.setup.rules.ai.threat_radius.into());
        known
            .iter()
            .filter(|k| (k.pos - at).length_sq_raw() <= r)
            .count() as u32
    }

    fn task(&mut self, id: SquadId, kind: TaskKind, point: u8, target: FxVec2) {
        self.ai.tasks.insert(
            id,
            Task {
                kind,
                point,
                target,
            },
        );
    }

    // ---------------------------------------------------------- retreat --

    fn retreats(&mut self, mine: &mut Vec<Mine>, known: &[Known]) {
        let pct = self.g.setup.rules.ai.retreat_strength_pct;
        let mut keep = Vec::with_capacity(mine.len());
        for m in mine.drain(..) {
            let sq = &self.g.squads[&m.id];
            if sq.order == Order::Retreat {
                continue;
            }
            if self.ai.tasks.get(&m.id).map(|t| t.kind) == Some(TaskKind::Retreat) {
                // Arrived at base: available again.
                self.ai.tasks.remove(&m.id);
            }
            if m.strength_pct < pct && self.threat_at(known, m.lead) > 0 {
                let roll = self.below(100);
                if roll < 50 + self.quality / 2 {
                    self.ai.emit(self.due, CommandKind::Retreat { squad: m.id });
                    let base = self.g.base_of(self.me);
                    self.task(m.id, TaskKind::Retreat, NO_POINT, base);
                    continue;
                }
            }
            keep.push(m);
        }
        *mine = keep;
    }

    // -------------------------------------------------------------- buy --

    fn buy(&mut self) {
        let g = self.g;
        let s = &g.setup;
        let p = self.me.0 as usize;
        let faction = &s.factions[s.players[p].faction as usize];
        let mut by_role: [Option<u16>; 4] = [None; 4];
        for &t in &faction.roster {
            let r = role_index(s.squad_types[t as usize].role);
            by_role[r].get_or_insert(t);
        }
        let mut counts = [0i64; 4];
        for sq in g.squads.values().filter(|sq| sq.owner == self.me) {
            counts[role_index(s.squad_types[sq.type_idx as usize].role)] += 1;
        }
        let pl = &g.players[p];
        let mut queue = pl.call_ins.len();
        for c in &pl.call_ins {
            counts[role_index(s.squad_types[c.squad_type as usize].role)] += 1;
        }
        let mut budget = pl.manpower as i64;
        let mut pop = g.population(self.ents, self.me) as i64;
        for c in &self.ai.pending {
            if let CommandKind::CallIn { squad_type } = c.kind {
                let ty = &s.squad_types[squad_type as usize];
                counts[role_index(ty.role)] += 1;
                budget -= ty.cost as i64;
                pop += ty.men as i64;
                queue += 1;
            }
        }
        let mix = s.rules.ai.mix_pct;
        for _ in 0..MAX_BUYS_PER_PLAN {
            let total: i64 = counts.iter().sum();
            let mut role = (0..4)
                .filter(|&r| by_role[r].is_some() && mix[r] > 0)
                .max_by_key(|&r| (mix[r] as i64 * (total + 1) - 100 * counts[r], -(r as i64)));
            let roll = self.below(200);
            if roll < 100 - self.quality {
                let options: Vec<usize> = (0..4).filter(|&r| by_role[r].is_some()).collect();
                if !options.is_empty() {
                    role = Some(options[self.below(options.len() as u32) as usize]);
                }
            }
            let Some(t) = role.and_then(|r| by_role[r]) else {
                return;
            };
            let ty = &s.squad_types[t as usize];
            if budget < ty.cost as i64
                || pop + ty.men as i64 > s.rules.pop_cap as i64
                || queue >= MAX_CALL_IN_QUEUE
            {
                // Save up for it rather than buying something else.
                return;
            }
            self.ai
                .emit(self.due, CommandKind::CallIn { squad_type: t });
            budget -= ty.cost as i64;
            pop += ty.men as i64;
            queue += 1;
            counts[role_index(ty.role)] += 1;
        }
    }

    // ----------------------------------------------------------- points --

    fn evaluate_points(&mut self, known: &[Known]) -> Vec<PointEval> {
        let g = self.g;
        let base = g.base_of(self.me);
        let depth_dm = Fx::from(g.setup.rules.dig.depth).mul_int(10).floor_int();
        let mut out = Vec::with_capacity(g.points.len());
        for (i, (c, ps)) in g.points.iter().zip(&g.setup.map.points).enumerate() {
            let pos: FxVec2 = ps.pos.into();
            let rel = match c.owner {
                Some(o) if o == self.me => Rel::Own,
                None => Rel::Neutral,
                Some(_) => Rel::Enemy,
            };
            let threat = self.threat_at(known, pos);
            let under_capture = c.capturer.is_some_and(|q| q != self.me);
            let mut priority = ps.value as i64 * 100 - dist_m(pos, base) / 8;
            priority += match rel {
                Rel::Neutral => 150,
                Rel::Enemy => 100,
                Rel::Own if threat > 0 || under_capture => 80 + threat as i64 * 10,
                Rel::Own => 20,
            };
            let extra = (threat / 4).min(2);
            let demand = match rel {
                Rel::Own if threat == 0 && !under_capture => 1,
                Rel::Own | Rel::Neutral => 1 + extra,
                Rel::Enemy => 2 + extra,
            };
            let trench = self.ai.trenches.iter().position(|t| t.point as usize == i);
            let trench_done = trench.is_some_and(|k| {
                let [a, b] = self.ai.trenches[k].line;
                let (gx, gy) = self.terrain.nearest_grid((a + b).div_int(2));
                self.terrain.dug_dm(gx, gy) as i32 * 10 >= depth_dm * 9
            });
            out.push(PointEval {
                pos,
                rel,
                threat,
                priority,
                demand,
                assigned: 0,
                trench,
                trench_done,
            });
        }
        out
    }

    /// Greedy squad → point assignment. Returns `(squad index in mine,
    /// point index)` in assignment order.
    fn assign(&mut self, mine: &[Mine], points: &mut [PointEval]) -> Vec<(usize, usize)> {
        let mut pairs = Vec::new();
        for (j, m) in mine.iter().enumerate() {
            if m.role == SquadRole::Mortar {
                continue;
            }
            let cur = self.ai.tasks.get(&m.id).map(|t| t.point);
            for (i, pt) in points.iter().enumerate() {
                let mut score = pt.priority - dist_m(m.lead, pt.pos) / 4;
                if cur == Some(i as u8) {
                    score += 60;
                }
                let weak = m.strength_pct < 60;
                score += match (m.role, pt.rel) {
                    (SquadRole::Engineer, Rel::Own) if !pt.trench_done => 80,
                    (SquadRole::Mg, Rel::Own) => 30,
                    (SquadRole::Mg, Rel::Enemy) => -20,
                    (_, Rel::Own) if weak => 40,
                    _ if weak => -40,
                    _ => 0,
                };
                score += self.noise();
                pairs.push((score, j, i));
            }
        }
        // Best score first; ties by squad then point index.
        pairs.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
        let mut done = vec![false; mine.len()];
        let mut out = Vec::new();
        for &(_, j, i) in &pairs {
            if !done[j] && points[i].assigned < points[i].demand {
                done[j] = true;
                points[i].assigned += 1;
                out.push((j, i));
            }
        }
        for &(_, j, i) in &pairs {
            if !done[j] {
                done[j] = true;
                points[i].assigned += 1;
                out.push((j, i));
            }
        }
        out
    }

    fn order_infantry(
        &mut self,
        mine: &[Mine],
        assigned: &[(usize, usize)],
        points: &mut [PointEval],
        known: &[Known],
    ) {
        let g = self.g;
        let mut slot = vec![0u32; points.len()];
        for &(j, i) in assigned {
            let m = &mine[j];
            let pt = &points[i];
            let k = slot[i];
            slot[i] += 1;
            let sq = &g.squads[&m.id];
            let wants_dig = m.role == SquadRole::Engineer
                && pt.rel == Rel::Own
                && !pt.trench_done
                && pt.threat < 4;
            if wants_dig {
                let line = match pt.trench {
                    Some(t) => Some(self.ai.trenches[t].line),
                    None => self.trench_line(i, points, known),
                };
                let Some(line) = line else { continue };
                if pt.trench.is_none() && self.ai.trenches.len() < MAX_TRENCHES {
                    self.ai.trenches.push(AiTrench {
                        point: i as u8,
                        line,
                    });
                    points[i].trench = Some(self.ai.trenches.len() - 1);
                }
                let mid = (line[0] + line[1]).div_int(2);
                let digging = matches!(&sq.order, Order::Dig { line: l, .. } if l[..] == line[..]);
                if !digging {
                    self.ai.emit(
                        self.due,
                        CommandKind::Dig {
                            squad: m.id,
                            line: line.to_vec(),
                        },
                    );
                }
                self.task(m.id, TaskKind::Dig, i as u8, mid);
                continue;
            }
            let (kind, target) = match pt.rel {
                Rel::Own => {
                    let target = match pt.trench {
                        Some(t) if pt.trench_done => {
                            // Stand along the trench, squads spread over it.
                            let [a, b] = self.ai.trenches[t].line;
                            let mid = (a + b).div_int(2);
                            let along = (b - a).div_int(4);
                            match k % 3 {
                                0 => mid,
                                1 => mid + along,
                                _ => mid - along,
                            }
                        }
                        _ => ring_slot(pt.pos, k),
                    };
                    (TaskKind::Defend, target)
                }
                Rel::Neutral => (TaskKind::Capture, ring_slot(pt.pos, k)),
                Rel::Enemy => (TaskKind::Attack, ring_slot(pt.pos, k)),
            };
            let target = self.terrain.clamp(target);
            let same = self
                .ai
                .tasks
                .get(&m.id)
                .is_some_and(|t| t.kind == kind && t.point == i as u8);
            let near =
                |a: FxVec2, b: FxVec2, r: i32| (a - b).length_sq_raw() <= sq_m(Fx::from_int(r));
            let on_track = match &sq.order {
                Order::Idle => near(m.lead, target, 20),
                Order::Move { target: t } | Order::AttackMove { target: t } => near(*t, target, 10),
                _ => false,
            };
            if !(same && on_track) {
                let cmd = if kind == TaskKind::Defend {
                    CommandKind::Move {
                        squad: m.id,
                        target,
                    }
                } else {
                    CommandKind::AttackMove {
                        squad: m.id,
                        target,
                    }
                };
                self.ai.emit(self.due, cmd);
            }
            self.task(m.id, kind, i as u8, target);
        }
    }

    /// A trench line in front of point `i`, across the direction to the
    /// nearest enemy (module docs). `None` if it would not fit the map.
    fn trench_line(&self, i: usize, points: &[PointEval], known: &[Known]) -> Option<[FxVec2; 2]> {
        let g = self.g;
        let cfg = &g.setup.rules.ai;
        let at = points[i].pos;
        let toward = known
            .iter()
            .map(|k| k.pos)
            .min_by_key(|p| ((*p - at).length_sq_raw(), p.x, p.y))
            .or_else(|| {
                points
                    .iter()
                    .filter(|p| p.rel == Rel::Enemy)
                    .map(|p| p.pos)
                    .min_by_key(|p| (*p - at).length_sq_raw())
            })
            .or_else(|| {
                (0..g.players.len())
                    .filter(|&q| q != self.me.0 as usize)
                    .map(|q| g.base_of(PlayerId(q as u8)))
                    .min_by_key(|p| (*p - at).length_sq_raw())
            })?;
        let dir = (toward - at).normalize_or_zero();
        if dir == FxVec2::ZERO {
            return None;
        }
        let center = at + dir * Fx::from(cfg.dig_offset);
        let half = dir.perp() * Fx::from(cfg.dig_length).div_int(2);
        let line = [
            self.terrain.clamp(center - half),
            self.terrain.clamp(center + half),
        ];
        let len = (line[1] - line[0]).length();
        (len >= Fx::from_int(2) && len <= Fx::from(g.setup.rules.dig.max_length)).then_some(line)
    }

    // ---------------------------------------------------------- mortars --

    fn mortars(&mut self, mine: &[Mine], points: &[PointEval], known: &[Known]) {
        let g = self.g;
        let s = &g.setup;
        // Enemy groups by squad: (squad, sum x, sum y, men, men in trenches).
        let mut groups: BTreeMap<u32, (i64, i64, i64, i64)> = BTreeMap::new();
        for k in known {
            let e = groups.entry(k.squad).or_default();
            e.0 += k.pos.x.raw() as i64;
            e.1 += k.pos.y.raw() as i64;
            e.2 += 1;
            e.3 += k.in_trench as i64;
        }
        let own: Vec<FxVec2> = self
            .ents
            .soldiers()
            .filter(|id| self.ents.owner[id.index()] == self.me)
            .map(|id| self.ents.transform[id.index()].pos.xy())
            .collect();
        let danger = sq_m(Fx::from_int(DANGER_CLOSE_M));
        let front = points
            .iter()
            .filter(|p| p.rel != Rel::Own || p.threat > 0)
            .max_by_key(|p| p.priority)
            .or_else(|| points.iter().max_by_key(|p| p.priority))
            .map(|p| p.pos);
        for m in mine.iter().filter(|m| m.role == SquadRole::Mortar) {
            let sq = &g.squads[&m.id];
            if matches!(sq.order, Order::Barrage { .. }) {
                continue;
            }
            let w = &s.weapons[s.squad_types[sq.type_idx as usize].weapon as usize];
            let min = Fx::from(w.range_min) + Fx::from_int(20);
            let max = Fx::from(w.range_max) - Fx::from_int(10);
            let mut best: Option<(i64, FxVec2)> = None;
            for &(sx, sy, n, trench) in groups.values() {
                let c = FxVec2::new(Fx::from_raw((sx / n) as i32), Fx::from_raw((sy / n) as i32));
                let d = (c - m.lead).length();
                if d < min || d > max || own.iter().any(|o| (*o - c).length_sq_raw() <= danger) {
                    continue;
                }
                let score = n * 10 + trench * 20 + self.noise();
                if score >= 20 && best.is_none_or(|(b, _)| score > b) {
                    best = Some((score, c));
                }
            }
            if let Some((_, target)) = best {
                self.ai.emit(
                    self.due,
                    CommandKind::Barrage {
                        squad: m.id,
                        target,
                        rounds: s.rules.ai.barrage_rounds,
                    },
                );
                self.task(m.id, TaskKind::Barrage, NO_POINT, target);
                continue;
            }
            let Some(front) = front else { continue };
            let back = (g.base_of(self.me) - front).normalize_or_zero();
            let spot = self
                .terrain
                .clamp(front + back * Fx::from_int(SUPPORT_BACK_M));
            let near = |a: FxVec2, r: i32| (a - spot).length_sq_raw() <= sq_m(Fx::from_int(r));
            let on_track = match &sq.order {
                Order::Idle => near(m.lead, 40),
                Order::Move { target } => near(*target, 40),
                _ => false,
            };
            if !on_track {
                self.ai.emit(
                    self.due,
                    CommandKind::Move {
                        squad: m.id,
                        target: spot,
                    },
                );
            }
            self.task(m.id, TaskKind::Support, NO_POINT, spot);
        }
    }
}

/// A throw the planner wants: item, the men who could throw it, target.
type ThrowPick = (ItemKind, Vec<SimId>, FxVec2);

impl Planner<'_> {
    // ----------------------------------------------------------- throws --

    /// Satchel, frag and smoke throws, then regrouping (module docs).
    fn throws(&mut self, mine: &[Mine], known: &[Known]) {
        let g = self.g;
        let cool = g.setup.rules.ai.throw_cooldown_ticks as u64;
        let tick = self.tick;
        self.ai
            .last_throw
            .retain(|id, t| g.squads.contains_key(id) && tick < t.saturating_add(cool));
        for m in mine {
            let sq = &g.squads[&m.id];
            if sq.order == Order::Retreat || self.ai.last_throw.contains_key(&m.id) {
                continue;
            }
            let pick = self
                .satchel(sq, known)
                .or_else(|| self.frag(sq, known))
                .or_else(|| self.smoke(sq, known));
            let Some((item, units, target)) = pick else {
                continue;
            };
            self.ai.emit(
                self.due,
                CommandKind::Throw {
                    units,
                    item,
                    target,
                },
            );
            self.ai.last_throw.insert(m.id, tick);
        }
        // Throwers stay detached where they stood; take them along again
        // once their squad walks on.
        for m in mine {
            let sq = &g.squads[&m.id];
            let unit = |id: &SimId| &self.ents.unit[id.index()];
            let kit = |id: &SimId| &self.ents.kit[id.index()];
            let detached = sq.members.iter().filter(|id| unit(id).detached);
            let pending = self
                .ai
                .pending
                .iter()
                .any(|c| matches!(c.kind, CommandKind::Regroup { squad } if squad == m.id));
            if !pending
                && matches!(sq.nav.phase, NavPhase::Moving | NavPhase::PathPending)
                && detached.clone().next().is_some()
                && detached.clone().all(|id| kit(id).windup.is_none())
            {
                self.ai.emit(self.due, CommandKind::Regroup { squad: m.id });
            }
        }
    }

    /// Men of `sq` who may throw a `kind` item at `target` now.
    fn throwers(&self, sq: &crate::game::Squad, kind: ItemKind, target: FxVec2) -> Vec<SimId> {
        let mut out: Vec<SimId> = sq
            .members
            .iter()
            .copied()
            .filter(|&id| self.g.can_throw(self.ents, id, kind, target).is_some())
            .collect();
        out.sort_unstable();
        out
    }

    /// Engineers: a satchel at a spotted enemy in a trench in range.
    fn satchel(&self, sq: &crate::game::Squad, known: &[Known]) -> Option<ThrowPick> {
        let ty = &self.g.setup.squad_types[sq.type_idx as usize];
        if ty.role != SquadRole::Engineer {
            return None;
        }
        known.iter().filter(|k| k.in_trench).find_map(|k| {
            let men = self.throwers(sq, ItemKind::Satchel, k.pos);
            (!men.is_empty()).then_some((ItemKind::Satchel, men, k.pos))
        })
    }

    /// A frag at the best spotted target in range (module docs).
    fn frag(&self, sq: &crate::game::Squad, known: &[Known]) -> Option<ThrowPick> {
        let g = self.g;
        let ty = &g.setup.squad_types[sq.type_idx as usize];
        let (_, it) = ty.kit(&g.setup.items, ItemKind::Frag)?;
        let lead = self.ents.transform[sq.lead(self.ents).index()].pos.xy();
        // Only enemies a squad of this size could reach are worth a look.
        let reach = sq_m(Fx::from(it.range_max) + Fx::from_int(40));
        let near: Vec<&Known> = known
            .iter()
            .filter(|k| (k.pos - lead).length_sq_raw() <= reach)
            .collect();
        let group = sq_m(Fx::from_int(FRAG_GROUP_M));
        let need = g.setup.rules.ai.frag_group_men as i64;
        // Bigger groups, then trenches, then the nearest, then by id.
        let mut best: Option<((i64, bool, i64, SimId), ThrowPick)> = None;
        for k in &near {
            let men = near
                .iter()
                .filter(|o| (o.pos - k.pos).length_sq_raw() <= group)
                .count() as i64;
            if !k.in_trench && men < need {
                continue;
            }
            let key = (
                men,
                k.in_trench,
                -dist_m(k.pos, lead),
                SimId(u32::MAX - k.id.0),
            );
            if best.as_ref().is_some_and(|(b, _)| *b >= key) {
                continue;
            }
            let throwers = self.throwers(sq, ItemKind::Frag, k.pos);
            if !throwers.is_empty() {
                best = Some((key, (ItemKind::Frag, throwers, k.pos)));
            }
        }
        best.map(|(_, pick)| pick)
    }

    /// Smoke for a squad pinned on the move (module docs).
    fn smoke(&self, sq: &crate::game::Squad, known: &[Known]) -> Option<ThrowPick> {
        if !matches!(sq.order, Order::Move { .. } | Order::AttackMove { .. }) {
            return None;
        }
        let pinned = sq
            .members
            .iter()
            .filter(|id| self.ents.combat[id.index()].pinned)
            .count();
        if pinned == 0 || pinned * 3 < sq.members.len() {
            return None;
        }
        let g = self.g;
        let ty = &g.setup.squad_types[sq.type_idx as usize];
        let (_, it) = ty.kit(&g.setup.items, ItemKind::Smoke)?;
        let max = Fx::from(it.range_max) - Fx::ONE;
        // The carriers: men with smoke who are not pinned.
        sq.members.iter().find_map(|&id| {
            let k = &self.ents.kit[id.index()];
            if k.items[ItemKind::Smoke.index()] == 0 || self.ents.combat[id.index()].pinned {
                return None;
            }
            let from = self.ents.transform[id.index()].pos.xy();
            let enemy = known
                .iter()
                .min_by_key(|k| ((k.pos - from).length_sq_raw(), k.id))?;
            let d = (enemy.pos - from).length();
            let dir = (enemy.pos - from).normalize_or_zero();
            let at = self.terrain.clamp(from + dir * d.div_int(2).min(max));
            let men = self.throwers(sq, ItemKind::Smoke, at);
            (!men.is_empty()).then_some((ItemKind::Smoke, men, at))
        })
    }
}

/// Spot `k` around a point: the point itself, then a ring of 8 m.
fn ring_slot(at: FxVec2, k: u32) -> FxVec2 {
    if k == 0 {
        return at;
    }
    // k · 2.1 rad keeps successive squads well apart on the ring.
    let (s, c) = Fx::from_ratio(k as i64 * 2100, 1000).wrap_angle().sin_cos();
    at + FxVec2::new(c, s) * Fx::from_int(8)
}
