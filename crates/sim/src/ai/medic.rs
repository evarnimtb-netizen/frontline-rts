//! S12b AI (docs/slices/S12-wounds-medics.md 5, A1-A5 and liveness;
//! `[ai.medic]`): medic teams, the aid post, carrying, staffing and
//! reinforcements. Called in a plan after the supply habit, before the
//! buys. Plans draw no RNG and read squads, men and casualties by
//! ascending id. No saved field of its own (the plan's layout rule): a
//! team's job is read each plan from the sim state the medic rules keep
//! (`Casualty::tend`, `carriers`, the men's detached orders and their
//! `UnitNav::phase_ticks`), so a save between plans changes nothing.
//! Every rule needs `[wounds] enabled`; each has its own switch.
//!
//! * **A1 buy** (`buy`): the faction's medic card (tag `medic`, `ai_weight`
//!   over 0) when `min_inf_squads` infantry squads live and 3 or more own
//!   downed men lie on the field (each went down within the last
//!   `down_bleed_max_s`, the stateless reading of "3 downs in 120 s"); a
//!   second once a team staffs a post; at most `max_teams` (pending buys
//!   count); out of the infantry budget (manpower less the held pots).
//! * **A2 post** (`post`): one aid post, built by the lowest free team: in
//!   the own HQ's radius on the side toward the own base, else at the own
//!   base's edge toward the assault target (the non-own point nearest the
//!   base).
//! * **A3 field** (`field`): a free team (no job, not staffing) sends a man
//!   (`Tend`) to the nearest own unstabilised, uncarried, unclaimed downed
//!   man within [`FIELD_REACH_M`] of its leader when no spotted enemy is
//!   within `enemy_clear_m` of him (else it holds: `medic_holds`); else it
//!   carries (`Carry`, both men) the nearest stabilised one there to the
//!   post (else the base) when no other own unstabilised man lies within
//!   [`CARRY_CLEAR_M`] of him; else it follows `front_m` behind the
//!   assault group's lead squad (the own Capture or Attack squad nearest
//!   its target), toward the own base.
//! * **A4 staffing** (`post`): while own men wait in the post's radius (in
//!   a bed, a wounded man halted there untreated, a stabilised downed man)
//!   or walk or are carried there, the team already there (else the
//!   nearest free team) stands in it; with nobody waiting no team stays.
//!   A retreating squad (`Retreat`) sends its untreated wounded men
//!   (`MoveUnits`) to the post when the post is nearer to them than the
//!   base; their squad takes no new orders while they walk there or wait.
//! * **A5 reinforce** (`reinforce`, and `[wounds] reinforce`): an infantry
//!   squad under `reinforce_below_pct` of its card size (members and own
//!   living casualties) whose leader is in the own base or that is
//!   retreating gets `Reinforce`, when its card is off cooldown, the
//!   budget pays at least one man and pop room is left; before the buys
//!   (the price is held from them, its card counts as bought). A squad
//!   in the own base, halted or retreating, with a reinforcement standing
//!   detached and halted (Fit, on no other job) is regrouped.
//! * **Liveness** (L1-L5 as S10b-ai): a team man is on a job while he
//!   tends (or walks to, or stands by) an own unstabilised, uncarried
//!   downed man nobody else tends, or carries one. Otherwise, or after
//!   [`STALL_S`] in one walk or wait phase (a carry's walk: 5 times that),
//!   the team is regrouped (its casualty died, was evacuated or taken, or
//!   the job stalled). Patients walking to or waiting at a post are
//!   regrouped after [`STALL_S`] likewise, and when no post is left.

use frontline_content::setup::StockKind;

use super::{AiKind, Known, Mine, Planner, TaskKind, ai_kind, ring_slot, sq_m};
use crate::command::CommandKind;
use crate::ecs::{EntityKind, SimId};
use crate::fx::{Fx, FxVec2};
use crate::game::{NavPhase, Order, SquadId};
use crate::unit::UnitOrder;
use crate::wounds::{FIT, WOUNDED};

/// A job (a walk, a wait, a tend) with no new phase for this long is
/// dropped (s; plan 5 liveness).
pub(crate) const STALL_S: u64 = 120;
/// A3: a stabilised man is carried only with no other own unstabilised
/// downed man this close to him (m).
pub(crate) const CARRY_CLEAR_M: i32 = 30;
/// A3: a field team looks for downed men this far from its leader (m).
pub(crate) const FIELD_REACH_M: i32 = 100;
/// A3: a following team is re-ordered when its leader is farther than
/// this from its spot (m).
const FOLLOW_SLACK_M: i32 = 20;

/// What a team man does now (derived each plan).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Job {
    /// In formation (or riding): no job.
    Free,
    /// On a job (tends, walks to tend, stands by, carries).
    Busy,
    /// Detached with no job left, or stalled: the team is regrouped.
    Stale,
}

/// A medic team as the plan sees it.
struct Team {
    id: SquadId,
    lead: FxVec2,
    men: Vec<SimId>,
    /// Some man on a job (or building the post).
    busy: bool,
    /// Standing in (or walking to) the own built post's radius.
    at_post: bool,
}

impl Planner<'_> {
    /// S12b A1-A5 (module docs). `mine` loses the medic teams and the
    /// squads whose wounded walk to or wait at the post.
    pub(super) fn medics(&mut self, mine: &mut Vec<Mine>, known: &[Known], busy: &[SquadId]) {
        let g = self.g;
        let s = &g.setup;
        if s.rules.wounds.enabled == 0 {
            return;
        }
        let me = self.me;
        let cfg = s.rules.ai.medic.clone();
        let team_ids: Vec<SquadId> = g
            .squads
            .iter()
            .filter(|(_, q)| q.owner == me && s.type_has_tag(q.type_idx, "medic"))
            .map(|(&id, _)| id)
            .collect();
        mine.retain(|m| !team_ids.contains(&m.id));
        let post = self.own_post();
        let mut regroup: Vec<SquadId> = Vec::new();
        if cfg.post != 0 {
            self.patients(mine, post.as_ref(), busy, &mut regroup);
        }
        // Teams: liveness first.
        let mut teams: Vec<Team> = Vec::new();
        // Liveness belongs to the jobs: with A3 and A4 off no team has one.
        let jobs = cfg.field != 0 || cfg.post != 0;
        for &id in &team_ids {
            if busy.contains(&id) || !jobs {
                continue;
            }
            let sq = &g.squads[&id];
            let mut stale = false;
            let mut on_job = matches!(sq.order, Order::Dig { .. });
            for &m in &sq.members {
                match self.man_job(m) {
                    Job::Free => {}
                    Job::Busy => on_job = true,
                    Job::Stale => stale = true,
                }
            }
            if stale {
                if !self.regroup_pending(id) {
                    regroup.push(id);
                }
                continue;
            }
            let lead = self.ents.transform[sq.lead(self.ents).index()].pos.xy();
            let at_post = post.as_ref().is_some_and(|(_, p, r)| {
                p.built
                    && match sq.order {
                        Order::Move { target } => (target - p.pos).length_sq_raw() <= sq_m(*r),
                        _ => sq.members.iter().all(|m| {
                            (self.ents.transform[m.index()].pos.xy() - p.pos).length_sq_raw()
                                <= sq_m(*r)
                        }),
                    }
            });
            teams.push(Team {
                id,
                lead,
                men: sq.members.clone(),
                busy: on_job,
                at_post,
            });
        }
        for id in regroup.iter().copied() {
            self.ai.emit(self.due, CommandKind::Regroup { squad: id });
        }
        if cfg.post != 0 {
            self.build_post(&teams, post.is_some());
            self.staff_post(&mut teams, post.as_ref());
        } else {
            teams.iter_mut().for_each(|t| t.at_post = false);
        }
        if cfg.buy != 0 {
            self.buy_medics(team_ids.len(), &teams);
        }
        if cfg.field != 0 {
            self.field(&teams, post.as_ref(), known);
        }
        if cfg.reinforce != 0 && s.rules.wounds.reinforce != 0 {
            self.reinforce(busy);
        }
    }

    /// The own aid post (lowest id), with its radius (m).
    fn own_post(&self) -> Option<(u32, crate::structures::Structure, Fx)> {
        let s = &self.g.setup;
        self.g
            .supply6
            .structures
            .iter()
            .find(|(_, st)| st.owner == self.me && s.stocks[st.def as usize].kind == StockKind::Aid)
            .map(|(&id, st)| (id, st.clone(), s.stocks[st.def as usize].radius.into()))
    }

    fn regroup_pending(&self, id: SquadId) -> bool {
        self.ai
            .pending
            .iter()
            .any(|c| matches!(c.kind, CommandKind::Regroup { squad } if squad == id))
    }

    fn stalled(&self, m: SimId, limit_s: u64) -> bool {
        let hz = self.g.setup.tick_hz.max(1) as u64;
        self.ents.unit[m.index()].nav.phase_ticks as u64 >= limit_s * hz
    }

    /// Whether own downed man `id` needs a tend: unstabilised, uncarried.
    fn needs_tend(&self, id: SimId) -> bool {
        self.g.casualties.get(&id).is_some_and(|c| {
            c.bleed_out_at != 0 && c.carriers[0].is_none() && self.ents.owner[id.index()] == self.me
        })
    }

    /// A team man's job (module docs, "Liveness").
    fn man_job(&self, m: SimId) -> Job {
        let i = m.index();
        let u = &self.ents.unit[i];
        if self.ents.kind[i] != EntityKind::Soldier
            || !u.detached
            || u.riding.is_some()
            || u.serving.is_some()
        {
            return Job::Free;
        }
        let reach = sq_m(Fx::from_int(crate::medics::REACH_M));
        let g = self.g;
        // Carrying (or fetching, or waiting for the other carrier).
        if g.casualties.values().any(|c| c.carriers.contains(&Some(m))) {
            let moving = u.nav.phase == NavPhase::Moving;
            let limit = if moving { 5 * STALL_S } else { STALL_S };
            return if self.stalled(m, limit) {
                Job::Stale
            } else {
                Job::Busy
            };
        }
        let spot = match u.order {
            UnitOrder::Move { target } => target,
            UnitOrder::Idle => self.ents.transform[i].pos.xy(),
            _ => return Job::Stale,
        };
        let job = g.casualties.iter().find(|&(&id, c)| {
            self.needs_tend(id)
                && c.tend.is_none_or(|t| t.0 == m)
                && (self.ents.transform[id.index()].pos.xy() - spot).length_sq_raw() <= reach
        });
        match job {
            Some(_) if !self.stalled(m, STALL_S) => Job::Busy,
            _ => Job::Stale,
        }
    }

    // ------------------------------------------------------------ A1 --

    /// A1 (module docs).
    fn buy_medics(&mut self, alive: usize, teams: &[Team]) {
        let g = self.g;
        let s = &g.setup;
        let me = self.me;
        let cfg = &s.rules.ai.medic;
        let faction = &s.factions[s.players[me.0 as usize].faction as usize];
        let Some(t) = faction
            .roster
            .iter()
            .copied()
            .find(|&t| s.type_has_tag(t, "medic") && s.squad_types[t as usize].ai_weight > 0)
        else {
            return;
        };
        let pending = self
            .ai
            .pending
            .iter()
            .filter(|c| c.kind == CommandKind::CallIn { squad_type: t })
            .count();
        let have = alive + pending;
        if have >= cfg.max_teams as usize || pending > 0 {
            return;
        }
        let infantry = g
            .squads
            .values()
            .filter(|q| {
                q.owner == me
                    && matches!(
                        ai_kind(s, q.type_idx),
                        AiKind::Rifle | AiKind::Mg | AiKind::Engineer
                    )
            })
            .count();
        if infantry < cfg.min_inf_squads as usize {
            return;
        }
        let want = if have == 0 {
            let downs = g
                .casualties
                .keys()
                .filter(|id| self.ents.owner[id.index()] == me)
                .count();
            downs >= 3
        } else {
            teams.iter().any(|t| t.at_post)
        };
        if !want || g.call_in_check(self.ents, me, t, self.due).is_err() {
            return;
        }
        let budget = g.players[me.0 as usize].manpower as i64 - self.held_pots() - self.medic_pot();
        if budget < s.squad_types[t as usize].cost as i64 {
            return;
        }
        self.ai
            .emit(self.due, CommandKind::CallIn { squad_type: t });
        self.ai.counters.medic_buys += 1;
    }

    /// A5: manpower held for pending `Reinforce` orders (one man's price
    /// each: the least the sim takes), kept from the buys.
    pub(super) fn medic_pot(&self) -> i64 {
        let s = &self.g.setup;
        self.ai
            .pending
            .iter()
            .filter_map(|c| match c.kind {
                CommandKind::Reinforce { squad } => self.g.squads.get(&squad),
                _ => None,
            })
            .map(|q| {
                let ty = &s.squad_types[q.type_idx as usize];
                (ty.cost as u64 / ty.men.max(1) as u64 * s.rules.wounds.reinforce_cost_pct as u64
                    / 100) as i64
            })
            .sum()
    }

    // ------------------------------------------------------------ A2 --

    /// A2 (module docs): the lowest free team builds the post.
    fn build_post(&mut self, teams: &[Team], have: bool) {
        let g = self.g;
        let s = &g.setup;
        let me = self.me;
        if have
            || self
                .ai
                .pending
                .iter()
                .any(|c| matches!(c.kind, CommandKind::Build { .. }))
        {
            return;
        }
        let faction = s.players[me.0 as usize].faction;
        let Some(def) = s.stock_of(faction, StockKind::Aid, None) else {
            return;
        };
        let Some(team) = teams.iter().find(|t| !t.busy) else {
            return;
        };
        let cost = s.stocks[def as usize].cost as i64;
        if (g.players[me.0 as usize].manpower as i64) - self.held_pots() - self.medic_pot() < cost {
            return;
        }
        let (at, facing) = self.post_site();
        self.ai.emit(
            self.due,
            CommandKind::Build {
                squad: team.id,
                def,
                pos: at,
                facing,
            },
        );
        self.ai.counters.post_builds += 1;
    }

    /// A2: the post's spot and facing: in the own built HQ's radius (half
    /// way out) toward the own base, else on the own base's edge toward
    /// the non-own point nearest the base.
    pub(super) fn post_site(&self) -> (FxVec2, FxVec2) {
        let g = self.g;
        let s = &g.setup;
        let base = g.base_of(self.me);
        let hq = g.supply6.structures.values().find(|st| {
            st.owner == self.me && st.built && s.stocks[st.def as usize].kind == StockKind::Hq
        });
        if let Some(h) = hq {
            let r: Fx = s.stocks[h.def as usize].radius.into();
            let to_base = base - h.pos;
            let dir = to_base.normalize_or_zero();
            let out = r.div_int(2).min(to_base.length());
            let at = self.terrain.clamp(h.pos + dir * out);
            return (at, dir.perp());
        }
        let pts = &s.map.points;
        let target = (0..pts.len())
            .filter(|&k| g.points[k].owner != Some(self.me))
            .min_by_key(|&k| {
                let at: FxVec2 = pts[k].pos.into();
                ((at - base).length_sq_raw(), k)
            })
            .map_or(base, |k| pts[k].pos.into());
        let dir = (target - base).normalize_or_zero();
        let r: Fx = s.map.bases[s.players[self.me.0 as usize].base as usize]
            .radius
            .into();
        (self.terrain.clamp(base + dir * r), dir.perp())
    }

    // ------------------------------------------------------------ A4 --

    /// A4 (module docs): whether anyone waits at the post, and the team
    /// that staffs it. Teams at a post nobody needs are freed.
    fn staff_post(
        &mut self,
        teams: &mut [Team],
        post: Option<&(u32, crate::structures::Structure, Fx)>,
    ) {
        let Some((_, p, r)) = post.filter(|x| x.1.built) else {
            for t in teams.iter_mut() {
                t.at_post = false;
            }
            return;
        };
        let need = self.post_needed(p, *r);
        if !need {
            for t in teams.iter_mut() {
                t.at_post = false;
            }
            return;
        }
        if teams.iter().any(|t| t.at_post) {
            return;
        }
        let pick = teams
            .iter_mut()
            .filter(|t| !t.busy)
            .min_by_key(|t| ((t.lead - p.pos).length_sq_raw(), t.id));
        if let Some(t) = pick {
            let to = self.terrain.clamp(ring_slot(p.pos, 1));
            self.ai.emit(
                self.due,
                CommandKind::Move {
                    squad: t.id,
                    target: to,
                },
            );
            t.at_post = true;
        }
    }

    /// A4: own men wait at post `p` (radius `r`): a bed taken, a wounded
    /// man halted in it untreated, a stabilised downed man in it, a wounded
    /// man walking to it, a man carried to it.
    fn post_needed(&self, p: &crate::structures::Structure, r: Fx) -> bool {
        let g = self.g;
        let r2 = sq_m(r);
        let inside = |q: FxVec2| (q - p.pos).length_sq_raw() <= r2;
        if !p.beds.is_empty() {
            return true;
        }
        let downed = g.casualties.iter().any(|(&id, c)| {
            self.ents.owner[id.index()] == self.me
                && ((c.bleed_out_at == 0 && inside(self.ents.transform[id.index()].pos.xy()))
                    || c.dest.is_some_and(inside))
        });
        downed || self.ents.soldiers().any(|m| self.patient_at(m, p, r))
    }

    /// A4: own soldier `m` is a patient of post `p`: wounded, untreated,
    /// detached, halted in its radius or walking to it.
    fn patient_at(&self, m: SimId, p: &crate::structures::Structure, r: Fx) -> bool {
        let i = m.index();
        let w = self.ents.combat[i].wound;
        let u = &self.ents.unit[i];
        if self.ents.owner[i] != self.me
            || w.state != WOUNDED
            || w.treated
            || !u.detached
            || u.riding.is_some()
            || u.serving.is_some()
        {
            return false;
        }
        let spot = match u.order {
            UnitOrder::Move { target } => target,
            UnitOrder::Idle => self.ents.transform[i].pos.xy(),
            _ => return false,
        };
        (spot - p.pos).length_sq_raw() <= sq_m(r)
    }

    /// A4: retreating squads send their untreated wounded to the post;
    /// squads with patients stay out of `mine`; stalled or stranded
    /// patients are regrouped (into `regroup`).
    fn patients(
        &mut self,
        mine: &mut Vec<Mine>,
        post: Option<&(u32, crate::structures::Structure, Fx)>,
        busy: &[SquadId],
        regroup: &mut Vec<SquadId>,
    ) {
        let g = self.g;
        let me = self.me;
        let base = g.base_of(me);
        let built = post.filter(|x| x.1.built);
        let in_bed = |m: SimId| {
            g.supply6
                .structures
                .values()
                .any(|st| st.beds.iter().any(|b| b.man == m))
        };
        let mut waiting: Vec<SquadId> = Vec::new();
        for (&id, sq) in &g.squads {
            if sq.owner != me || busy.contains(&id) {
                continue;
            }
            let mut send: Vec<SimId> = Vec::new();
            let mut stale = false;
            let mut patient = false;
            for &m in &sq.members {
                let i = m.index();
                let w = self.ents.combat[i].wound;
                let u = &self.ents.unit[i];
                if w.state != WOUNDED || w.treated || u.riding.is_some() || u.serving.is_some() {
                    continue;
                }
                let pos = self.ents.transform[i].pos.xy();
                if !u.detached {
                    if sq.order == Order::Retreat
                        && let Some((_, p, _)) = built
                        && (p.pos - pos).length_sq_raw() < (base - pos).length_sq_raw()
                    {
                        send.push(m);
                    }
                    continue;
                }
                if in_bed(m) {
                    patient = true;
                    continue;
                }
                let at_post = built.is_some_and(|(_, p, r)| self.patient_at(m, p, *r));
                if at_post {
                    patient = true;
                    if self.stalled(m, STALL_S) {
                        stale = true;
                    }
                } else if u.order == UnitOrder::Idle && self.stalled(m, STALL_S) {
                    // Walked off for a post that is gone (or never was):
                    // back to his squad.
                    stale = true;
                }
            }
            if stale {
                if !self.regroup_pending(id) && !regroup.contains(&id) {
                    regroup.push(id);
                }
                continue;
            }
            if patient {
                waiting.push(id);
            }
            if let (false, Some((_, p, _))) = (send.is_empty(), built) {
                self.ai.emit(
                    self.due,
                    CommandKind::MoveUnits {
                        units: send,
                        target: p.pos,
                    },
                );
            }
        }
        mine.retain(|m| !waiting.contains(&m.id) && !regroup.contains(&m.id));
    }

    // ------------------------------------------------------------ A3 --

    /// A3 (module docs): tend, carry, else follow.
    fn field(
        &mut self,
        teams: &[Team],
        post: Option<&(u32, crate::structures::Structure, Fx)>,
        known: &[Known],
    ) {
        let g = self.g;
        let s = &g.setup;
        let me = self.me;
        let cfg = &s.rules.ai.medic;
        let reach2 = sq_m(Fx::from_int(FIELD_REACH_M));
        let clear2 = sq_m(Fx::from_int(cfg.enemy_clear_m.min(2000) as i32));
        let carry2 = sq_m(Fx::from_int(CARRY_CLEAR_M));
        let tend2 = sq_m(Fx::from_int(crate::medics::REACH_M));
        let pos = |id: SimId| self.ents.transform[id.index()].pos.xy();
        // Downed men another own medic tends or walks to.
        let mut claimed: Vec<SimId> = g
            .casualties
            .iter()
            .filter(|&(&id, c)| {
                c.tend.is_some()
                    || self.ents.soldiers().any(|m| {
                        let u = &self.ents.unit[m.index()];
                        let spot = match u.order {
                            UnitOrder::Move { target } => target,
                            _ => pos(m),
                        };
                        self.ents.owner[m.index()] == me
                            && u.detached
                            && (spot - pos(id)).length_sq_raw() <= tend2
                            && g.is_medic(self.ents, m.index())
                    })
            })
            .map(|(&id, _)| id)
            .collect();
        let base = g.base_of(me);
        let post_in = |q: FxVec2| {
            post.is_some_and(|(_, p, r)| p.built && (q - p.pos).length_sq_raw() <= sq_m(*r))
        };
        let lead = self.assault_lead();
        for t in teams.iter().filter(|t| !t.busy && !t.at_post) {
            let free: Vec<SimId> = t
                .men
                .iter()
                .copied()
                .filter(|m| !self.ents.unit[m.index()].detached)
                .collect();
            if free.is_empty() {
                continue;
            }
            // Tend: the nearest unclaimed bleeding man in reach.
            let mut held = false;
            let mut tend: Option<(u64, SimId)> = None;
            for &id in g.casualties.keys() {
                if !self.needs_tend(id) || claimed.contains(&id) {
                    continue;
                }
                let d = (pos(id) - t.lead).length_sq_raw();
                if d > reach2 {
                    continue;
                }
                if known
                    .iter()
                    .any(|k| (k.pos - pos(id)).length_sq_raw() <= clear2)
                {
                    held = true;
                    continue;
                }
                if tend.is_none_or(|b| d < b.0) {
                    tend = Some((d, id));
                }
            }
            if let Some((_, id)) = tend {
                self.ai.emit(
                    self.due,
                    CommandKind::Tend {
                        units: free,
                        target: id,
                    },
                );
                self.ai.counters.medic_tends += 1;
                claimed.push(id);
                continue;
            }
            if held {
                self.ai.counters.medic_holds += 1;
            }
            // Carry: the nearest stabilised man in reach, away from the
            // post and the base, with no bleeding man near him.
            let fit: Vec<SimId> = free
                .iter()
                .copied()
                .filter(|m| self.ents.combat[m.index()].wound.state == FIT)
                .collect();
            let mut carry: Option<(u64, SimId)> = None;
            if !fit.is_empty() {
                for (&id, c) in &g.casualties {
                    let at = pos(id);
                    if self.ents.owner[id.index()] != me
                        || c.bleed_out_at != 0
                        || c.carriers[0].is_some()
                        || claimed.contains(&id)
                        || post_in(at)
                        || crate::supply::in_own_base(s, me, at)
                    {
                        continue;
                    }
                    let d = (at - t.lead).length_sq_raw();
                    if d > reach2 || carry.is_some_and(|b| b.0 <= d) {
                        continue;
                    }
                    let bleeding_near = g.casualties.keys().any(|&o| {
                        o != id && self.needs_tend(o) && (pos(o) - at).length_sq_raw() <= carry2
                    });
                    if !bleeding_near {
                        carry = Some((d, id));
                    }
                }
            }
            if let Some((_, id)) = carry {
                let to = match post {
                    Some((_, p, _)) if p.built => p.pos,
                    _ => base,
                };
                self.ai.emit(
                    self.due,
                    CommandKind::Carry {
                        units: fit,
                        target: id,
                        to,
                    },
                );
                self.ai.counters.medic_carries += 1;
                claimed.push(id);
                continue;
            }
            // Follow `front_m` behind the assault's lead squad.
            let Some(front) = lead else { continue };
            let back = (base - front).normalize_or_zero();
            let to = self
                .terrain
                .clamp(front + back * Fx::from_int(cfg.front_m.min(2000) as i32));
            let slack = sq_m(Fx::from_int(FOLLOW_SLACK_M));
            let going = matches!(g.squads[&t.id].order,
                Order::Move { target } if (target - to).length_sq_raw() <= slack);
            if !going && (t.lead - to).length_sq_raw() > slack {
                self.ai.emit(
                    self.due,
                    CommandKind::Move {
                        squad: t.id,
                        target: to,
                    },
                );
            }
        }
    }

    /// A3: the leader's place of the assault group's lead squad: of the own
    /// squads with a Capture or Attack task, the one whose leader is
    /// nearest its task's target (ties: lower id).
    fn assault_lead(&self) -> Option<FxVec2> {
        let g = self.g;
        self.ai
            .tasks
            .iter()
            .filter(|(id, t)| {
                matches!(t.kind, TaskKind::Capture | TaskKind::Attack)
                    && g.squads.get(id).is_some_and(|q| q.owner == self.me)
            })
            .map(|(id, t)| {
                let q = &g.squads[id];
                let at = self.ents.transform[q.lead(self.ents).index()].pos.xy();
                ((at - t.target).length_sq_raw(), *id, at)
            })
            .min_by_key(|x| (x.0, x.1))
            .map(|x| x.2)
    }

    // ------------------------------------------------------------ A5 --

    /// A5 (module docs).
    fn reinforce(&mut self, busy: &[SquadId]) {
        let g = self.g;
        let s = &g.setup;
        let me = self.me;
        let below = s.rules.ai.medic.reinforce_below_pct as usize;
        let mut budget =
            g.players[me.0 as usize].manpower as i64 - self.held_pots() - self.medic_pot();
        let mut room = s.rules.pop_cap as i64 - g.population(self.ents, me) as i64;
        let mut cards: Vec<u16> = self
            .ai
            .pending
            .iter()
            .filter_map(|c| match c.kind {
                CommandKind::CallIn { squad_type } => Some(squad_type),
                CommandKind::Reinforce { squad } => g.squads.get(&squad).map(|q| q.type_idx),
                _ => None,
            })
            .collect();
        let crew = |t: u16| s.type_has_tag(t, "crew") || s.type_has_tag(t, "gun_crew");
        let mut fold: Vec<SquadId> = Vec::new();
        for (&id, sq) in &g.squads {
            let t = sq.type_idx;
            let ty = &s.squad_types[t as usize];
            if sq.owner != me
                || busy.contains(&id)
                || ty.vehicle.is_some()
                || ty.cost == 0
                || crew(t)
                || !matches!(
                    ai_kind(s, t),
                    AiKind::Rifle
                        | AiKind::Mg
                        | AiKind::Engineer
                        | AiKind::AntiTank
                        | AiKind::Mortar
                )
            {
                continue;
            }
            let lead = self.ents.transform[sq.lead(self.ents).index()].pos.xy();
            let home = crate::supply::in_own_base(s, me, lead);
            // Reinforcements standing halted in the base: fold them in.
            if home
                && matches!(sq.order, Order::Idle | Order::Retreat)
                && sq.members.iter().any(|&m| {
                    let i = m.index();
                    let u = &self.ents.unit[i];
                    u.detached
                        && u.order == UnitOrder::Idle
                        && self.ents.combat[i].wound.state == FIT
                        && self.ents.kit[i].windup.is_none()
                        && !g.other_job(self.ents, i, true)
                })
                && !self.regroup_pending(id)
            {
                fold.push(id);
            }
            let down = g.casualties.values().filter(|c| c.from_squad == id).count();
            let have = sq.members.len() + down;
            let men = ty.men.max(1) as usize;
            if have * 100 >= below * men || have >= men {
                continue;
            }
            if !(home || sq.order == Order::Retreat) {
                continue;
            }
            let group = ty.cooldown_group;
            if g.cooldown_left(me, t, self.due) > 0
                || cards.iter().any(|&b| {
                    b == t || (group.is_some() && s.squad_types[b as usize].cooldown_group == group)
                })
            {
                continue;
            }
            let price = (ty.cost as u64 / men as u64 * s.rules.wounds.reinforce_cost_pct as u64
                / 100) as i64;
            if budget < price || room < 1 {
                continue;
            }
            self.ai.emit(self.due, CommandKind::Reinforce { squad: id });
            self.ai.counters.reinforce_orders += 1;
            budget -= price;
            room -= 1;
            cards.push(t);
        }
        for id in fold {
            self.ai.emit(self.due, CommandKind::Regroup { squad: id });
        }
    }
}

impl crate::game::MatchState {
    /// S12 Build C (report): own downed men lying now, per player (the
    /// skirmish report's `pop_downed_peak`).
    pub fn downed_by_player(&self, ents: &crate::ecs::Entities) -> Vec<u32> {
        let mut n = vec![0u32; self.players.len()];
        for id in self.casualties.keys() {
            if let Some(c) = n.get_mut(ents.owner()[id.index()].0 as usize) {
                *c += 1;
            }
        }
        n
    }
}
