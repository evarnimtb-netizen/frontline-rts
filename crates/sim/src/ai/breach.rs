//! S10a-ai wire knowledge and breaching (`docs/slices/S10a-ai.md` 3.4;
//! values in `[ai.fort]`, switch `breach`).
//!
//! * **Knowing wire.** An enemy section becomes known when an own soldier
//!   within `wire_seen_m` of its middle has a clear terrain line from 1.5 m
//!   over his ground to 0.5 m over the section's middle: at most
//!   `wire_rays` rays per plan, sections in ascending id from the saved
//!   `AiState::wire_cursor` (wrapping). `AiState::wire_known` (ascending,
//!   at most [`MAX_WIRE_KNOWN`]) forgets sections that no longer stand.
//! * **Want.** For each non-own point (ascending) with squads assigned to
//!   attack it, no breach job yet and a known section within
//!   `breach_near_m`: one `path::find_foot_path` (the sim's own costs,
//!   wire included) from the nearest attacker's leader to the point, at
//!   most one per point per `recheck_s` (`AiState::bypass`, which also
//!   dates a job's start). A path that crosses no known section goes
//!   round (`wire_bypassed`); else a breach job ([`AiBreach`], keyed by
//!   the breach squad) at the first crossing: of the squads attacking the
//!   point, one carrying a Bangalore (`breach`) first, else one with
//!   `clear_wire`, nearest, at least half strong. None: the urgent buy
//!   (`ai.rs` `buy`) gets the faction's assault card.
//! * **Phases.** *Stage*: the squad crouches and moves `stage_m` short of
//!   the crossing on the side away from the point. *Suppress*: the point's
//!   base of fire (`ai::assault`) aims at the known defender nearest the
//!   crossing, MG-team men first ([`super::Planner::breach_focus`]);
//!   waited for at most twice `suppress_lead_s`. *Smoke*: a man with smoke
//!   in the breach squad, else in a squad attacking the point, throws it
//!   `smoke_frac_pct` of the way from the crossing to that defender
//!   (capped at the throw range). *Go*: `suppress_lead_s` after the fire
//!   or the smoke, `PlaceBangalore` at the crossing if carried and a known
//!   defender is within `cut_fire_m` of it, else `CutWire`. *Retry*: no
//!   charge laid and no cut under way within `place_wait_s`: suppress,
//!   smoke and send again, `breach_tries` in all. *Through*: no standing
//!   enemy section within `gap_clear_m` of the crossing: the job ends
//!   opened and the squads attack as usual (A* finds the gap). A job also
//!   ends when its squad is gone or under half strength, the point is no
//!   longer attacked, or after `breach_max_s`.
//!
//! Integer and fixed-point only, ascending ids, no RNG: the plan is the
//! same at every thread count and across save and load.

use frontline_content::ItemKind;
use frontline_content::MatchSetup;
use frontline_content::setup::Capability;
use serde::{Deserialize, Serialize};

use super::{AiKind, AiState, Known, Mine, PointEval, Rel, TaskKind, sq_m};
use crate::command::CommandKind;
use crate::ecs::{Entities, PlayerId, SimId};
use crate::fort::{closest_on, crossing, near_segment};
use crate::fx::{Fx, FxVec2};
use crate::game::{MatchState, Order, SquadId};
use crate::terrain::Heightfield;

/// Most breach jobs one AI keeps (spec 4).
pub const MAX_BREACHES: usize = 8;
/// Most enemy wire sections one AI remembers (spec 4).
pub const MAX_WIRE_KNOWN: usize = 256;
/// Most tries a breach job may record (`[ai.fort] breach_tries` is at most
/// 10).
pub const MAX_BREACH_TRIES: u8 = 10;
/// Eye and target heights of the wire sight line (mm).
const EYE_MM: i32 = 1500;
const WIRE_MARK_MM: i32 = 500;
/// The breach squad is at its stage spot this close (m).
const STAGE_ARRIVE_M: i32 = 10;
/// A laid charge counts for the crossing this close (m).
const CHARGE_NEAR_M: i32 = 6;

/// Where a breach job stands (spec 3.4).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum BreachPhase {
    /// The breach squad holds `stage_m` short of the crossing.
    #[default]
    Stage = 0,
    /// The base of fire aims at the defenders nearest the crossing.
    Suppress = 1,
    /// Smoke between the crossing and the MG.
    Smoke = 2,
    /// `PlaceBangalore` or `CutWire` is sent.
    Go = 3,
    /// Nothing laid or cut in time: suppress, smoke and send again.
    Retry = 4,
    /// The gap is open: attackers path through it.
    Through = 5,
}

/// One breach squad's job (spec 4; keyed by the squad).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AiBreach {
    /// The attacked point.
    pub point: u8,
    /// Where the path crosses the known wire.
    pub crossing: FxVec2,
    /// The crossed section's id.
    pub section: u32,
    pub phase: BreachPhase,
    /// Tick the phase began (the job's start is `AiState::bypass` of its
    /// point: no path search runs for a point with a job).
    pub since: u64,
    pub tries: u8,
    /// When the base of fire was seen firing, and the smoke thrown.
    pub suppress_at: Option<u64>,
    pub smoke_at: Option<u64>,
}

/// How a breach job ended (the fortify-test hook reads it).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum BreachEnd {
    Opened,
    GivenUp,
}

/// Whether the breach habit is on (`[ai.fort] breach`, fortifications in
/// the rules).
pub(super) fn breach_on(s: &MatchSetup) -> bool {
    s.rules.fortify.is_some() && s.rules.ai.fort.breach != 0
}

/// A squad type that can open wire: `breach` with a Bangalore in its kit,
/// or `clear_wire`.
fn opens_wire(s: &MatchSetup, type_idx: u16) -> bool {
    let ty = &s.squad_types[type_idx as usize];
    ty.has(Capability::ClearWire)
        || (ty.has(Capability::Breach) && ty.kit(&s.items, ItemKind::Bangalore).is_some())
}

impl super::Planner<'_> {
    // ------------------------------------------------------------ sight --

    /// Learns enemy wire by sight and forgets sections gone (module docs).
    pub(super) fn wire_sight(&mut self) {
        let g = self.g;
        let s = &g.setup;
        if !breach_on(s) {
            return;
        }
        let wire = &g.fort.wire;
        self.ai.wire_known.retain(|id| wire.contains_key(id));
        if wire.is_empty() {
            return;
        }
        let f = &s.rules.ai.fort;
        let me = self.me;
        let ents = self.ents;
        let men: Vec<FxVec2> = ents
            .soldiers()
            .filter(|id| {
                let i = id.index();
                ents.owner[i] == me && ents.unit[i].riding.is_none()
            })
            .map(|id| ents.transform[id.index()].pos.xy())
            .collect();
        if men.is_empty() {
            return;
        }
        let unknown: Vec<u32> = wire
            .iter()
            .filter(|(id, w)| w.owner != me && self.ai.wire_known.binary_search(id).is_err())
            .map(|(&id, _)| id)
            .collect();
        let start = unknown.partition_point(|&id| id <= self.ai.wire_cursor);
        let r = sq_m(Fx::from_int(f.wire_seen_m as i32));
        let mut rays = 0;
        for &id in unknown[start..].iter().chain(&unknown[..start]) {
            if rays >= f.wire_rays || self.ai.wire_known.len() >= MAX_WIRE_KNOWN {
                break;
            }
            let w = &wire[&id];
            let mid = w.a + (w.b - w.a).div_int(2);
            let Some(from) = men
                .iter()
                .copied()
                .filter(|p| (*p - mid).length_sq_raw() <= r)
                .min_by_key(|p| ((*p - mid).length_sq_raw(), p.x.raw(), p.y.raw()))
            else {
                continue;
            };
            rays += 1;
            self.ai.wire_cursor = id;
            let t = self.terrain;
            let eye = from.extend(t.height_at(from) + Fx::from_milli(EYE_MM));
            let mark = mid.extend(t.height_at(mid) + Fx::from_milli(WIRE_MARK_MM));
            if t.line_of_sight(eye, mark)
                && let Err(k) = self.ai.wire_known.binary_search(&id)
            {
                self.ai.wire_known.insert(k, id);
            }
        }
    }

    /// Known enemy sections within `r` of `at` (ascending).
    fn known_near(&self, at: FxVec2, r: Fx) -> Vec<u32> {
        self.ai
            .wire_known
            .iter()
            .copied()
            .filter(|id| {
                self.g
                    .fort
                    .wire
                    .get(id)
                    .is_some_and(|w| near_segment(at, w.a, w.b, r))
            })
            .collect()
    }

    // ------------------------------------------------------------- want --

    /// Whether a breach is wanted with no squad to make it (the urgent
    /// buy, spec 3.1): a non-own point attacked by own squads (their
    /// tasks) has known wire within `breach_near_m`, no breach job runs,
    /// and no own squad can open wire.
    pub(super) fn breach_wanted(&self) -> bool {
        let g = self.g;
        let s = &g.setup;
        if !breach_on(s) || self.ai.wire_known.is_empty() || !self.ai.breaches.is_empty() {
            return false;
        }
        let me = self.me;
        if g.squads
            .values()
            .any(|q| q.owner == me && !q.members.is_empty() && opens_wire(s, q.type_idx))
        {
            return false;
        }
        let near = Fx::from_int(s.rules.ai.fort.breach_near_m as i32);
        (0..g.points.len()).any(|i| {
            g.points[i].owner != Some(me)
                && self.ai.tasks.values().any(|t| {
                    t.point == i as u8 && matches!(t.kind, TaskKind::Attack | TaskKind::Capture)
                })
                && !self.known_near(s.map.points[i].pos.into(), near).is_empty()
        })
    }

    /// The first known section the path from `from` (its smoothed
    /// waypoints `pts`) crosses, and where.
    fn first_crossing(&self, from: FxVec2, pts: &[FxVec2]) -> Option<(u32, FxVec2)> {
        let wire = &self.g.fort.wire;
        let mut prev = from;
        for &p in pts {
            let d = p - prev;
            let hit = self
                .ai
                .wire_known
                .iter()
                .filter_map(|id| {
                    let w = wire.get(id)?;
                    crossing(prev, d, w.a, w.b).map(|f| (f, *id))
                })
                .min();
            if let Some((f, id)) = hit {
                return Some((id, prev + FxVec2::new(d.x.mul(f), d.y.mul(f))));
            }
            prev = p;
        }
        None
    }

    /// Whether the path from `from` crosses a standing enemy section the
    /// AI does not know yet.
    fn crosses_unknown(&self, from: FxVec2, pts: &[FxVec2]) -> bool {
        let me = self.me;
        let mut prev = from;
        for &p in pts {
            let d = p - prev;
            if self.g.fort.wire.iter().any(|(id, w)| {
                w.owner != me
                    && self.ai.wire_known.binary_search(id).is_err()
                    && crossing(prev, d, w.a, w.b).is_some()
            }) {
                return true;
            }
            prev = p;
        }
        false
    }

    /// One foot path as the sim serves it (wire costs included).
    fn breach_path(&self, from: FxVec2, to: FxVec2) -> crate::path::PathResult {
        let g = self.g;
        let terrain = self.terrain;
        let mv = &g.setup.rules.movement;
        let cost = crate::path::PathCost {
            slope_cost_pct: mv.slope_cost_pct,
            max_slope_pct: mv.max_slope_pct,
            trench_cost_pct: mv.trench_cost_pct,
            ditch_cost_pct: mv.ditch_cost_pct,
        };
        let ditches = (!g.at_ditches.is_empty()).then_some(&g.vderived.block);
        let (fort, forts) = (&g.fort, &g.setup.forts);
        let nodes = fort.path_nodes(forts, terrain.dim());
        let crosses = |a: FxVec2, b: FxVec2| fort.step_in_wire(forts, a, b);
        let wire = crate::path::WireCost {
            nodes: &nodes,
            cost_pct: g
                .setup
                .rules
                .fortify
                .as_ref()
                .map_or(100, |f| f.wire_path_cost_pct),
            crosses: &crosses,
        };
        let mut scratch = crate::path::PathScratch::default();
        crate::path::find_foot_path(
            terrain,
            from,
            to,
            &cost,
            ditches,
            Some(wire),
            mv.path_max_expansions,
            &mut scratch,
        )
    }

    /// New breach jobs (module docs, "Want"), after the assignment and the
    /// other claims. Returns the squads it took (claimed as hunts are).
    pub(super) fn breach_start(
        &mut self,
        mine: &[Mine],
        assigned: &[(usize, usize)],
        points: &[PointEval],
        claimed: &[SquadId],
    ) -> Vec<SquadId> {
        let g = self.g;
        let s = &g.setup;
        let mut took = Vec::new();
        if !breach_on(s) || self.ai.wire_known.is_empty() {
            return took;
        }
        let f = &s.rules.ai.fort;
        let hz = s.tick_hz.max(1) as u64;
        let near = Fx::from_int(f.breach_near_m as i32);
        for (i, pt) in points.iter().enumerate() {
            if self.ai.breaches.len() >= MAX_BREACHES {
                break;
            }
            if pt.rel == Rel::Own || self.ai.breaches.values().any(|b| b.point == i as u8) {
                continue;
            }
            let here: Vec<&Mine> = assigned
                .iter()
                .filter(|a| a.1 == i)
                .map(|a| &mine[a.0])
                .collect();
            if here.is_empty() || self.known_near(pt.pos, near).is_empty() {
                continue;
            }
            if self
                .ai
                .bypass
                .get(&(i as u8))
                .is_some_and(|&t| self.tick < t + f.recheck_s as u64 * hz)
            {
                continue;
            }
            let lead = here
                .iter()
                .min_by_key(|m| ((m.lead - pt.pos).length_sq_raw(), m.id))
                .map(|m| m.lead)
                .expect("not empty");
            let path = self.breach_path(lead, pt.pos);
            let found = self.first_crossing(lead, &path.points);
            if found.is_none() && self.crosses_unknown(lead, &path.points) {
                // Through wire not seen yet: no decision (the next plan
                // may know it).
                continue;
            }
            self.ai.bypass.insert(i as u8, self.tick);
            let Some((section, at)) = found else {
                if path.reached {
                    self.ai.counters.wire_bypassed += 1;
                }
                continue;
            };
            let carries = |m: &Mine| {
                m.can(Capability::Breach)
                    && g.squads[&m.id]
                        .members
                        .iter()
                        .any(|x| self.ents.kit[x.index()].items[ItemKind::Bangalore.index()] > 0)
            };
            let pick = here
                .iter()
                .copied()
                .filter(|m| {
                    !claimed.contains(&m.id)
                        && !took.contains(&m.id)
                        && m.strength_pct >= 50
                        && (carries(m) || m.can(Capability::ClearWire))
                })
                .min_by_key(|m| (!carries(m), (m.lead - at).length_sq_raw(), m.id));
            let Some(m) = pick else { continue };
            self.ai.counters.breaches_started += 1;
            let mut job = AiBreach {
                point: i as u8,
                crossing: self.terrain.clamp(at),
                section,
                phase: BreachPhase::Stage,
                since: self.tick,
                tries: 0,
                suppress_at: None,
                smoke_at: None,
            };
            let units: Vec<SimId> = g.squads[&m.id]
                .members
                .iter()
                .copied()
                .take(crate::unit::MAX_UNITS)
                .collect();
            self.ai.emit(
                self.due,
                CommandKind::SetStance {
                    units,
                    stance: crate::unit::StanceOrder::Crouch,
                },
            );
            if self.run_breach(m, &mut job, false).is_none() {
                self.ai.breaches.insert(m.id, job);
            }
            took.push(m.id);
        }
        took
    }

    // ----------------------------------------------------------- phases --

    /// Runs the breach jobs (before the assignment): their squads leave
    /// `mine`, ended jobs free theirs.
    pub(super) fn breach_continue(&mut self, mine: &mut Vec<Mine>, busy: &[SquadId]) {
        let g = self.g;
        if self.ai.breaches.is_empty() {
            return;
        }
        let me = self.me;
        let ids: Vec<SquadId> = self.ai.breaches.keys().copied().collect();
        for id in ids {
            if busy.contains(&id) && g.squads.get(&id).is_some_and(|q| q.owner == me) {
                continue;
            }
            let Some(k) = mine.iter().position(|m| m.id == id) else {
                self.ai.breaches.remove(&id);
                self.ai.counters.breaches_given_up += 1;
                continue;
            };
            let mut job = self.ai.breaches[&id].clone();
            let m = mine[k].clone();
            if self.run_breach(&m, &mut job, true).is_some() {
                self.ai.breaches.remove(&id);
                continue;
            }
            self.ai.breaches.insert(id, job);
            mine.remove(k);
        }
    }

    /// Tick the job of point `p` started (its last path search).
    fn breach_started(&self, p: u8) -> u64 {
        self.ai.bypass.get(&p).copied().unwrap_or(self.tick)
    }

    /// Whether an own squad (the breach squad's task included: it was
    /// sent to attack) attacks point `p`, or bases fire for it.
    fn point_attacked(&self, p: u8) -> bool {
        self.ai.tasks.values().any(|t| {
            t.point == p
                && matches!(
                    t.kind,
                    TaskKind::Attack | TaskKind::Capture | TaskKind::Suppress
                )
        })
    }

    /// One plan of a breach job: `Some(end)` when it ended (counted).
    /// `check` runs the end rules that read the last plan's tasks.
    fn run_breach(&mut self, m: &Mine, job: &mut AiBreach, check: bool) -> Option<BreachEnd> {
        let g = self.g;
        let s = &g.setup;
        let f = &s.rules.ai.fort;
        let hz = s.tick_hz.max(1) as u64;
        let tick = self.tick;
        let me = self.me;
        // Through: no standing enemy section near the crossing.
        let clear = Fx::from_int(f.gap_clear_m as i32);
        let open = !g
            .fort
            .wire
            .values()
            .any(|w| w.owner != me && near_segment(job.crossing, w.a, w.b, clear));
        if open {
            job.phase = BreachPhase::Through;
            self.ai.counters.breaches_opened += 1;
            return Some(BreachEnd::Opened);
        }
        let point_pos: FxVec2 = s.map.points[job.point as usize].pos.into();
        let late = tick > self.breach_started(job.point) + f.breach_max_s as u64 * hz;
        let lost = m.strength_pct < 50;
        let taken = g.points[job.point as usize].owner == Some(me);
        let dropped = check && !self.point_attacked(job.point);
        if late || lost || taken || dropped {
            self.ai.counters.breaches_given_up += 1;
            return Some(BreachEnd::GivenUp);
        }
        let sq = &g.squads[&m.id];
        let known = self.known_enemies();
        let focus = self.breach_focus_of(job, &known);
        match job.phase {
            BreachPhase::Stage => {
                let back = (job.crossing - point_pos).normalize_or_zero();
                let spot = self
                    .terrain
                    .clamp(job.crossing + back * Fx::from_int(f.stage_m as i32));
                let there = (m.lead - spot).length_sq_raw() <= sq_m(Fx::from_int(STAGE_ARRIVE_M));
                // It walks up once the base of fire fires (or after twice
                // `suppress_lead_s` without one).
                if job.suppress_at.is_none() && self.base_firing(job.point, m.id) {
                    job.suppress_at = Some(tick);
                    self.ai.counters.breach_suppress += 1;
                }
                let covered = job.suppress_at.is_some()
                    || tick >= job.since + 2 * f.suppress_lead_s as u64 * hz;
                if there || (sq.order == Order::Idle && f.stage_m == 0) {
                    job.phase = BreachPhase::Suppress;
                    job.since = tick;
                } else if covered && !matches!(sq.order, Order::Move { target } if target == spot) {
                    self.ai.emit(
                        self.due,
                        CommandKind::Move {
                            squad: m.id,
                            target: spot,
                        },
                    );
                }
            }
            BreachPhase::Suppress => {
                if job.suppress_at.is_none() && self.base_firing(job.point, m.id) {
                    job.suppress_at = Some(tick);
                    self.ai.counters.breach_suppress += 1;
                }
                let waited = tick >= job.since + 2 * f.suppress_lead_s as u64 * hz;
                if job.suppress_at.is_some() || waited {
                    job.phase = BreachPhase::Smoke;
                    job.since = tick;
                    // Toward the defender aimed at, else the point.
                    let at = focus.unwrap_or(point_pos);
                    if self.breach_smoke(m.id, job, at) {
                        job.smoke_at = Some(tick);
                        self.ai.counters.breach_smokes += 1;
                    }
                }
            }
            BreachPhase::Smoke => {
                if tick >= job.since + f.suppress_lead_s as u64 * hz {
                    self.breach_go(m, job, &known);
                    job.phase = BreachPhase::Go;
                    job.since = tick;
                }
            }
            BreachPhase::Go => {
                let near = sq_m(Fx::from_int(CHARGE_NEAR_M));
                let laid = g
                    .fort
                    .charges
                    .iter()
                    .any(|c| c.owner == me && (c.to - job.crossing).length_sq_raw() <= near);
                // A squad on its cut order (walking to the wire or
                // cutting) is under way: the walk alone can take longer
                // than `place_wait_s`; a stalled cut ends by `breach_max_s`.
                let cutting = matches!(sq.order, Order::CutWire { .. });
                // A carrier who fell or was called off (no windup left, no
                // cut order) is a failed try at once.
                let placing = sq.members.iter().any(|x| {
                    self.ents.kit[x.index()]
                        .windup
                        .is_some_and(|w| w.kind == ItemKind::Bangalore)
                });
                let walking = matches!(sq.order, Order::CutWire { .. });
                let failed = tick >= job.since + 2 * self.reaction() && !placing && !walking;
                if !laid && !cutting && (failed || tick >= job.since + f.place_wait_s as u64 * hz) {
                    job.phase = BreachPhase::Retry;
                    job.since = tick;
                }
            }
            BreachPhase::Retry => {
                job.tries = job.tries.saturating_add(1);
                if job.tries as u32 >= f.breach_tries {
                    self.ai.counters.breaches_given_up += 1;
                    return Some(BreachEnd::GivenUp);
                }
                job.phase = BreachPhase::Suppress;
                job.since = tick;
                job.suppress_at = None;
                job.smoke_at = None;
            }
            BreachPhase::Through => {}
        }
        None
    }

    /// The reaction delay (ticks): a command sent this plan applies then.
    fn reaction(&self) -> u64 {
        self.due.saturating_sub(self.tick).max(1)
    }

    /// Whether an own squad bases fire for point `p` now (task Suppress at
    /// `p`, order `Suppress`).
    fn base_firing(&self, p: u8, except: SquadId) -> bool {
        self.ai.tasks.iter().any(|(&id, t)| {
            id != except
                && t.point == p
                && t.kind == TaskKind::Suppress
                && self
                    .g
                    .squads
                    .get(&id)
                    .is_some_and(|q| matches!(q.order, Order::Suppress { .. }))
        })
    }

    /// The defender a breach at point `p` aims at (base of fire, smoke):
    /// the known MG-team man nearest the crossing within
    /// `ai.threat_radius` of the point, else the nearest known enemy
    /// soldier there. `None` without a job at `p`.
    pub(super) fn breach_focus(&self, p: usize, known: &[Known]) -> Option<FxVec2> {
        let job = self
            .ai
            .breaches
            .values()
            .filter(|b| b.point as usize == p)
            .min_by_key(|b| (b.crossing.x.raw(), b.crossing.y.raw()))?;
        self.breach_focus_of(job, known)
    }

    fn breach_focus_of(&self, job: &AiBreach, known: &[Known]) -> Option<FxVec2> {
        let g = self.g;
        let s = &g.setup;
        let pos: FxVec2 = s.map.points.get(job.point as usize)?.pos.into();
        let r = sq_m(s.rules.ai.threat_radius.into());
        let mg = |k: &Known| {
            g.squads
                .get(&SquadId(k.squad))
                .is_some_and(|q| super::ai_kind(s, q.type_idx) == AiKind::Mg)
        };
        known
            .iter()
            .filter(|k| (k.pos - pos).length_sq_raw() <= r)
            .min_by_key(|k| (!mg(k), (k.pos - job.crossing).length_sq_raw(), k.id))
            .map(|k| k.pos)
    }

    /// Smoke for breach `job` toward `toward`: a man of the breach squad,
    /// else of a squad attacking the point (ascending ids). Whether one
    /// was ordered.
    fn breach_smoke(&mut self, id: SquadId, job: &AiBreach, toward: FxVec2) -> bool {
        let g = self.g;
        let s = &g.setup;
        let frac = s.rules.ai.fort.smoke_frac_pct.min(100) as i32;
        let want = job.crossing + (toward - job.crossing).mul_int(frac).div_int(100);
        let mut squads = vec![id];
        squads.extend(self.ai.tasks.iter().filter_map(|(&q, t)| {
            (q != id
                && t.point == job.point
                && matches!(
                    t.kind,
                    TaskKind::Attack | TaskKind::Capture | TaskKind::Suppress
                ))
            .then_some(q)
        }));
        for q in squads {
            let Some(sq) = g.squads.get(&q) else { continue };
            let ty = &s.squad_types[sq.type_idx as usize];
            let Some((_, it)) = ty.kit(&s.items, ItemKind::Smoke) else {
                continue;
            };
            let max = Fx::from(it.range_max) - Fx::ONE;
            for &man in &sq.members {
                let i = man.index();
                if self.ents.kit[i].items[ItemKind::Smoke.index()] == 0
                    || self.ents.combat[i].pinned
                {
                    continue;
                }
                let from = self.ents.transform[i].pos.xy();
                let d = want - from;
                let at = if d.length() > max {
                    from + d.normalize_or_zero() * max
                } else {
                    want
                };
                let at = self.terrain.clamp(at);
                if self
                    .g
                    .can_throw(self.ents, man, ItemKind::Smoke, at)
                    .is_some()
                {
                    self.ai.emit(
                        self.due,
                        CommandKind::Throw {
                            units: vec![man],
                            item: ItemKind::Smoke,
                            target: at,
                        },
                    );
                    self.ai.last_throw.insert(q, self.tick);
                    return true;
                }
            }
        }
        false
    }

    /// The carrier goes: a Bangalore at the crossing when carried and a
    /// known defender is within `cut_fire_m` of it (or the squad cannot
    /// cut), else `CutWire`.
    fn breach_go(&mut self, m: &Mine, job: &AiBreach, known: &[Known]) {
        let g = self.g;
        let s = &g.setup;
        let sq = &g.squads[&m.id];
        let carried = m.can(Capability::Breach)
            && sq
                .members
                .iter()
                .any(|x| self.ents.kit[x.index()].items[ItemKind::Bangalore.index()] > 0);
        let r = sq_m(Fx::from_int(s.rules.ai.fort.cut_fire_m as i32));
        // Defended: a known defender near the crossing, a base of fire
        // fired for this try (the smoke may hide the defenders now), or the
        // squad is under fire (it lost men or a man is pinned: the fire it
        // takes need not be seen).
        let defended = known
            .iter()
            .any(|k| (k.pos - job.crossing).length_sq_raw() <= r)
            || job.suppress_at.is_some()
            || m.strength_pct < 100
            || sq
                .members
                .iter()
                .any(|x| self.ents.combat[x.index()].pinned);
        let cuts = m.can(Capability::ClearWire);
        if carried && (defended || !cuts) {
            let units: Vec<SimId> = sq
                .members
                .iter()
                .copied()
                .take(crate::unit::MAX_UNITS)
                .collect();
            self.ai.emit(
                self.due,
                CommandKind::PlaceBangalore {
                    units,
                    target: job.crossing,
                },
            );
        } else if cuts {
            // The section nearest the crossing (the sim picks it).
            let at = g
                .fort
                .wire
                .get(&job.section)
                .map_or(job.crossing, |w| closest_on(job.crossing, w.a, w.b));
            self.ai.emit(
                self.due,
                CommandKind::CutWire {
                    squad: m.id,
                    at: self.terrain.clamp(at),
                },
            );
        }
    }

    // ------------------------------------------------------------ drive --

    /// The fortify-test hook (spec 6): one plan of the breach habit alone
    /// for `breach` attacking point `p` with `support` as its bases of
    /// fire. Returns how the job ended, or `None` while it runs (or has
    /// not started: the wire is not known yet, or the path goes round).
    fn drive_breach(
        &mut self,
        breach: SquadId,
        support: &[SquadId],
        p: usize,
    ) -> Option<Option<BreachEnd>> {
        let g = self.g;
        let me = self.me;
        let mut ids = vec![breach];
        ids.extend_from_slice(support);
        let mut mine: Vec<Mine> = ids
            .iter()
            .filter_map(|&id| {
                let sq = g.squads.get(&id).filter(|q| q.owner == me)?;
                let ty = &g.setup.squad_types[sq.type_idx as usize];
                Some(Mine {
                    id,
                    role: super::ai_kind(&g.setup, sq.type_idx),
                    lead: self.ents.transform[sq.lead(self.ents).index()].pos.xy(),
                    strength_pct: sq.members.len() as u32 * 100 / ty.men.max(1) as u32,
                    caps: ty.caps,
                })
            })
            .collect();
        let busy: Vec<SquadId> = self
            .ai
            .pending
            .iter()
            .filter_map(|c| super::squad_of(&c.kind))
            .collect();
        let known = self.known_enemies();
        self.wire_sight();
        let had = self.ai.breaches.contains_key(&breach);
        if !had && !g.fort.wire.values().any(|w| w.owner != me) {
            // No enemy wire: nothing to breach.
            return Some(None);
        }
        let opened = self.ai.counters.breaches_opened;
        self.breach_continue(&mut mine, &busy);
        if had && !self.ai.breaches.contains_key(&breach) {
            let end = if self.ai.counters.breaches_opened > opened {
                BreachEnd::Opened
            } else {
                BreachEnd::GivenUp
            };
            return Some(Some(end));
        }
        // Every squad attacks point `p` (tasks, so the end rules see it).
        let pos: FxVec2 = g.setup.map.points[p].pos.into();
        for m in &mine {
            if m.id != breach && !self.ai.tasks.contains_key(&m.id) {
                self.task(m.id, TaskKind::Attack, p as u8, pos);
            }
        }
        if !had {
            let mut points = self.evaluate_points(&known, &[]);
            if points[p].rel == Rel::Own {
                return Some(None);
            }
            let assigned: Vec<(usize, usize)> = (0..mine.len()).map(|j| (j, p)).collect();
            points.iter_mut().for_each(|x| x.assigned = 0);
            let bypassed = self.ai.counters.wire_bypassed;
            let took = self.breach_start(&mine, &assigned, &points, &[]);
            if self.ai.counters.wire_bypassed > bypassed {
                // The path goes round the known wire: nothing to breach.
                return Some(None);
            }
            if took.is_empty() {
                return None;
            }
        }
        // The support squads are the bases of fire, aimed as the job wants.
        let from = mine.iter().find(|m| m.id != breach).map_or(pos, |m| m.lead);
        let line = match self.breach_focus(p, &known) {
            Some(at) => {
                let len = g.setup.rules.ai.assault.trench_line_m as i32;
                let half = (at - from).normalize_or_zero().perp() * Fx::from_int(len).div_int(2);
                Some([self.terrain.clamp(at - half), self.terrain.clamp(at + half)])
            }
            // No defender known yet: the assault's own line (the probed
            // trench), as `ai::assault` does.
            None => {
                let points = self.evaluate_points(&known, &[]);
                self.assault_line(from, p, &points, &known)
            }
        };
        if let Some(line) = line {
            let mut first: Option<FxVec2> = None;
            for m in mine
                .iter()
                .filter(|m| m.id != breach && !busy.contains(&m.id))
            {
                let at = self.base_of_fire(m, p, &line, first);
                first.get_or_insert(at);
            }
        }
        None
    }
}

impl MatchState {
    /// S10a-ai fortify-test hook (spec 6; `World::drive_breach`): one plan
    /// of the breach habit of player `me` with the given (fresh, unsaved)
    /// state, if it is due (`ai.next_plan`). `Some(end)` once the job has
    /// ended (`Some(None)`: nothing to breach).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn drive_breach_plan(
        &self,
        ai: &mut AiState,
        ents: &Entities,
        terrain: &Heightfield,
        tick: u64,
        me: PlayerId,
        breach: SquadId,
        support: &[SquadId],
        p: usize,
    ) -> Option<Option<BreachEnd>> {
        if tick < ai.next_plan || p >= self.points.len() {
            return None;
        }
        let ps = &self.setup.players[me.0 as usize];
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
        }
        .drive_breach(breach, support, p)
    }
}
