//! S12 Build B (docs/slices/S12-wounds-medics.md 4, W6-W11): medics,
//! stabilising, buddy bandages, carrying, aid posts, evacuation and
//! replacements. Every rule reads `[wounds] enabled` (W11 also `reinforce`);
//! with it 0 nothing here runs, no command is accepted and no RNG is drawn.
//! No saved field of its own: the state lives in Build A's layout
//! (`Casualty::tend`, `carriers`, `dest`, `evac_at`; `Structure::beds`;
//! `Wound`), and a man's part in a job is read from his detached order:
//!
//! * a **tender** (medic or buddy) is a detached man of the casualty's (or
//!   wounded man's) player standing within [`REACH_M`] with no move order
//!   (`UnitOrder::Idle`): `Tend` and the W6 auto-tend send him there;
//! * a **carrier** is named in `Casualty::carriers`; he walks to the man
//!   (`UnitOrder::Move` to the casualty's spot), waits there, then carries
//!   him (`UnitOrder::Move` to `Casualty::dest`).

use frontline_content::setup::member_trait;

use crate::ecs::{EntityKind, PlayerId, SimId};
use crate::fx::{Fx, FxVec2};
use crate::game::NavPhase;
use crate::game::{Ctx, MatchState};
use crate::unit::UnitOrder;
use crate::wounds::WOUNDED;

/// W6, W7: a tender or carrier works within this distance (m).
pub const REACH_M: i32 = 2;

/// W6 buddy bandage: a buddy works within this distance (m; the man he
/// dresses may shift a little as the buddy steps in).
pub const BUDDY_REACH_M: i32 = 3;

fn reach_sq() -> Fx {
    Fx::from_int(REACH_M * REACH_M)
}

/// Whether velocity `v` is a standing man's (W6 "not moving": under
/// 0.5 m/s, so the jostle of men standing close does not count).
fn still(v: FxVec2) -> bool {
    v.length_sq() <= Fx::from_milli(250)
}

impl MatchState {
    /// Whether entity `i` is a soldier whose slot has the `medic` trait.
    pub(crate) fn is_medic(&self, ents: &crate::ecs::Entities, i: usize) -> bool {
        if ents.kind[i] != EntityKind::Soldier {
            return false;
        }
        let Some(sq) = ents.squad[i].and_then(|s| self.squads.get(&s)) else {
            return false;
        };
        self.setup.squad_types[sq.type_idx as usize]
            .members
            .get(ents.kit[i].slot as usize)
            .is_some_and(|m| m.traits & member_trait::MEDIC != 0)
    }

    /// W7: every man carrying (or walking to carry) a casualty, with that
    /// casualty, ascending by man.
    pub(crate) fn carriers(&self) -> Vec<(SimId, SimId)> {
        let mut out: Vec<(SimId, SimId)> = self
            .casualties
            .iter()
            .flat_map(|(&id, c)| c.carriers.iter().flatten().map(move |&m| (m, id)))
            .collect();
        out.sort_unstable();
        out
    }

    /// W7: the pace (%) of carrier `m` while he carries (`None`: he does
    /// not carry now): two medics stretcher, anyone else drags.
    pub(crate) fn carry_pace(&self, ents: &crate::ecs::Entities, m: SimId) -> Option<u32> {
        let w = &self.setup.rules.wounds;
        if w.enabled == 0 || self.casualties.is_empty() {
            return None;
        }
        let (_, c) = self
            .casualties
            .iter()
            .find(|(_, c)| c.carriers.contains(&Some(m)))?;
        let dest = c.dest?;
        if ents.unit[m.index()].order != (UnitOrder::Move { target: dest }) {
            return None;
        }
        let stretcher = c
            .carriers
            .iter()
            .all(|x| x.is_some_and(|x| self.is_medic(ents, x.index())));
        Some(if stretcher {
            w.stretcher_speed_pct
        } else {
            w.drag_speed_pct
        })
    }

    /// Whether a detached man arriving at `goal` is on a medic job there
    /// (he then stands on the spot, not in nearby cover): a casualty or an
    /// unbandaged wounded man of his player lies within [`REACH_M`], or he
    /// carries.
    pub(crate) fn on_medic_job(
        &self,
        kind: &[EntityKind],
        owners: &[PlayerId],
        tf: &[crate::ecs::Transform],
        combat: &[crate::ecs::Combat],
        m: SimId,
        goal: FxVec2,
    ) -> bool {
        if self.setup.rules.wounds.enabled == 0 {
            return false;
        }
        let owner = owners[m.index()];
        let near = |q: FxVec2| (q - goal).length_sq() <= reach_sq();
        self.casualties.iter().any(|(&id, c)| {
            c.carriers.contains(&Some(m))
                || (owners[id.index()] == owner && near(tf[id.index()].pos.xy()))
        }) || (0..kind.len()).any(|j| {
            j != m.index()
                && kind[j] == EntityKind::Soldier
                && owners[j] == owner
                && combat[j].wound.state == WOUNDED
                && !combat[j].wound.bandaged
                && near(tf[j].pos.xy())
        })
    }

    /// Sends own soldier `m` (detached) to stand at `to`.
    pub(crate) fn send_man(&mut self, ctx: &mut Ctx, m: SimId, to: FxVec2) {
        let pos = ctx.ents.transform[m.index()].pos.xy();
        let mut facing = (to - pos).normalize_or_zero();
        if facing == FxVec2::ZERO {
            facing = FxVec2::new(Fx::ONE, Fx::ZERO);
        }
        self.detach(ctx, m, UnitOrder::Move { target: to }, facing);
        self.request_unit_path(ctx, m, to);
    }

    /// Stops detached man `m` where he stands (as `StopUnits`).
    fn halt_man(ctx: &mut Ctx, m: SimId) {
        let pos = ctx.ents.transform[m.index()].pos.xy();
        let u = &mut ctx.ents.unit[m.index()];
        u.detached = true;
        u.order = UnitOrder::Idle;
        u.nav.phase = NavPhase::Holding;
        u.nav.goal = pos;
        u.nav.path.clear();
        u.nav.next = 0;
        ctx.hash.mark_entity_chunk(m.chunk());
    }

    /// A tender's job is done: he rejoins his squad.
    pub(crate) fn release(ctx: &mut Ctx, m: SimId) {
        let i = m.index();
        if ctx.ents.kind[i] == EntityKind::Soldier && ctx.ents.unit[i].detached {
            ctx.ents.unit[i].rejoin();
            ctx.hash.mark_entity_chunk(m.chunk());
        }
    }

    /// Whether man `j` (of `owner`) is a buddy for a man at `at`:
    /// detached with no move order (halted there), within
    /// [`BUDDY_REACH_M`], not pinned.
    fn tender_at(ents: &crate::ecs::Entities, j: usize, owner: PlayerId, at: FxVec2) -> bool {
        ents.kind[j] == EntityKind::Soldier
            && ents.owner[j] == owner
            && ents.unit[j].detached
            && ents.unit[j].order == UnitOrder::Idle
            && ents.unit[j].riding.is_none()
            && !ents.combat[j].pinned
            && (ents.transform[j].pos.xy() - at).length_sq()
                <= Fx::from_int(BUDDY_REACH_M * BUDDY_REACH_M)
    }

    /// W6: whether medic `j` (of `owner`) stands ready by a man at `at`:
    /// within reach, standing, with no move order (detached and halted, or
    /// in his squad's held formation). Pinned or not: the caller decides.
    fn medic_ready(
        &self,
        ents: &crate::ecs::Entities,
        j: usize,
        owner: PlayerId,
        at: FxVec2,
    ) -> bool {
        let u = &ents.unit[j];
        ents.kind[j] == EntityKind::Soldier
            && ents.owner[j] == owner
            && u.riding.is_none()
            && u.serving.is_none()
            && still(ents.velocity[j].lin)
            && (ents.transform[j].pos.xy() - at).length_sq() <= reach_sq()
            && if u.detached {
                u.order == UnitOrder::Idle
            } else {
                ents.squad[j]
                    .and_then(|s| self.squads.get(&s))
                    .is_some_and(|sq| sq.nav.phase == NavPhase::Holding)
            }
            && self.is_medic(ents, j)
    }

    /// W6 buddy bandage: the lowest-id own tender of wounded man `i` (not
    /// himself, not carrying), from the detached men `idle`.
    pub(crate) fn buddy_of(
        &self,
        ents: &crate::ecs::Entities,
        idle: &[SimId],
        i: usize,
        carrying: &[(SimId, SimId)],
    ) -> Option<SimId> {
        let at = ents.transform[i].pos.xy();
        idle.iter().copied().find(|&b| {
            b.index() != i
                && carrying.binary_search_by_key(&b, |x| x.0).is_err()
                && Self::tender_at(ents, b.index(), ents.owner[i], at)
        })
    }

    /// Detached soldiers with no move order (the possible tenders),
    /// ascending.
    pub(crate) fn idle_detached(ents: &crate::ecs::Entities) -> Vec<SimId> {
        (0..ents.len())
            .filter(|&j| {
                ents.kind[j] == EntityKind::Soldier
                    && ents.unit[j].detached
                    && ents.unit[j].order == UnitOrder::Idle
            })
            .map(|j| SimId(j as u32))
            .collect()
    }

    /// The W6 and W7 rules of one tick (after bleeding, before the aid
    /// posts): tends progress, pause or end; medics with no move order go
    /// to the nearest own downed man; carriers fetch, carry and set down.
    pub(crate) fn medics_tick(&mut self, ctx: &mut Ctx) {
        if self.setup.rules.wounds.enabled == 0 {
            return;
        }
        self.carry_tick(ctx);
        self.stabilise_tick(ctx);
        self.auto_tend(ctx);
    }

    /// W6: a medic tender within reach stabilises a casualty (lower id
    /// first) in `stabilise_s`; pinned he pauses; gone, walked off or
    /// re-ordered the tend ends and the man keeps bleeding.
    fn stabilise_tick(&mut self, ctx: &mut Ctx) {
        if self.casualties.is_empty() {
            return;
        }
        let tick = ctx.tick;
        let hz = self.setup.tick_hz.max(1) as u64;
        let ticks = self.setup.rules.wounds.stabilise_s as u64 * hz;
        let carrying = self.carriers();
        let medics: Vec<SimId> = (0..ctx.ents.len())
            .filter(|&j| self.is_medic(ctx.ents, j))
            .map(|j| SimId(j as u32))
            .collect();
        let ids: Vec<SimId> = self.casualties.keys().copied().collect();
        let mut tending: Vec<SimId> = self
            .casualties
            .values()
            .filter_map(|c| c.tend.map(|t| t.0))
            .collect();
        tending.sort_unstable();
        for id in ids {
            let c = &self.casualties[&id];
            if c.bleed_out_at == 0 || c.carriers.iter().any(|x| x.is_some()) {
                // Stabilised, or carried: no tend.
                if let Some((m, _)) = c.tend {
                    self.casualties.get_mut(&id).expect("casualty").tend = None;
                    tending.retain(|&x| x != m);
                }
                continue;
            }
            let i = id.index();
            let owner = ctx.ents.owner[i];
            let at = ctx.ents.transform[i].pos.xy();
            if let Some((m, done)) = c.tend {
                let j = m.index();
                let near = self.medic_ready(ctx.ents, j, owner, at);
                let c = self.casualties.get_mut(&id).expect("casualty");
                if !near {
                    c.tend = None;
                    tending.retain(|&x| x != m);
                } else if ctx.ents.combat[j].pinned {
                    // Pinned: pauses.
                    c.tend = Some((m, done + 1));
                } else if done <= tick {
                    c.tend = None;
                    c.bleed_out_at = 0;
                    tending.retain(|&x| x != m);
                    self.bump_s12(owner.0, |s| &mut s.stabilised);
                    Self::release(ctx, m);
                }
                ctx.hash.mark_entity_chunk(id.chunk());
                continue;
            }
            // Lowest-id ready medic, not pinned, who tends nobody else.
            let pick = medics.iter().copied().find(|&m| {
                tending.binary_search(&m).is_err()
                    && carrying.binary_search_by_key(&m, |x| x.0).is_err()
                    && !ctx.ents.combat[m.index()].pinned
                    && self.medic_ready(ctx.ents, m.index(), owner, at)
            });
            if let Some(m) = pick {
                self.casualties.get_mut(&id).expect("casualty").tend = Some((m, tick + ticks));
                let k = tending.binary_search(&m).unwrap_or_else(|k| k);
                tending.insert(k, m);
                ctx.hash.mark_entity_chunk(id.chunk());
            }
        }
    }

    /// W6 auto-tend (once a second per medic, at `(tick + id) % hz`): a
    /// medic with no move order (in his squad's held formation, or
    /// detached and standing), under `crouch_suppression`, not carrying or
    /// tending, walks to the nearest own unstabilised, uncarried downed
    /// man within `auto_tend_m` that no other medic has (tends, or walks
    /// to).
    fn auto_tend(&mut self, ctx: &mut Ctx) {
        if self.casualties.is_empty() || self.setup.rules.wounds.auto_tend_m == 0 {
            return;
        }
        let tick = ctx.tick;
        let hz = self.setup.tick_hz.max(1) as u64;
        let crouch = self.setup.rules.combat.crouch_suppression;
        let r = Fx::from_int(self.setup.rules.wounds.auto_tend_m.min(2000) as i32);
        let r_sq = r.mul(r);
        let n = ctx.ents.len();
        let busy: Vec<SimId> = {
            let mut v: Vec<SimId> = self
                .casualties
                .values()
                .flat_map(|c| {
                    c.tend
                        .map(|t| t.0)
                        .into_iter()
                        .chain(c.carriers.iter().flatten().copied())
                })
                .collect();
            v.sort_unstable();
            v
        };
        for i in 0..n {
            if !(tick + i as u64).is_multiple_of(hz) || ctx.ents.kind[i] != EntityKind::Soldier {
                continue;
            }
            let m = SimId(i as u32);
            let u = &ctx.ents.unit[i];
            if u.riding.is_some() || u.serving.is_some() || busy.binary_search(&m).is_ok() {
                continue;
            }
            if ctx.ents.combat[i].suppression / 1000 >= crouch || !self.is_medic(ctx.ents, i) {
                continue;
            }
            let free = if u.detached {
                u.order == UnitOrder::Idle
            } else {
                ctx.ents.squad[i]
                    .and_then(|s| self.squads.get(&s))
                    .is_some_and(|sq| sq.nav.phase == NavPhase::Holding)
            };
            if !free {
                continue;
            }
            let owner = ctx.ents.owner[i];
            let pos = ctx.ents.transform[i].pos.xy();
            let mut best: Option<(Fx, SimId, FxVec2)> = None;
            for (&id, c) in &self.casualties {
                if c.bleed_out_at == 0
                    || c.tend.is_some()
                    || c.carriers.iter().any(|x| x.is_some())
                    || ctx.ents.owner[id.index()] != owner
                {
                    continue;
                }
                let at = ctx.ents.transform[id.index()].pos.xy();
                let d = (at - pos).length_sq();
                if d > r_sq || best.is_some_and(|b| b.0 <= d) {
                    continue;
                }
                // Another medic already walks to him or stands by him.
                let claimed = (0..n).any(|j| {
                    j != i
                        && ctx.ents.kind[j] == EntityKind::Soldier
                        && ctx.ents.owner[j] == owner
                        && ctx.ents.unit[j].detached
                        && (ctx.ents.unit[j].order == (UnitOrder::Move { target: at })
                            || (ctx.ents.unit[j].order == UnitOrder::Idle
                                && (ctx.ents.transform[j].pos.xy() - at).length_sq() <= reach_sq()))
                        && self.is_medic(ctx.ents, j)
                });
                if !claimed {
                    best = Some((d, id, at));
                }
            }
            if let Some((d, _, at)) = best {
                // Already beside him: the tend starts on its own.
                if d <= reach_sq() && u.detached {
                    continue;
                }
                self.send_man(ctx, m, at);
            }
        }
    }

    /// W7: carriers walk to their man, wait for each other, carry him to
    /// `dest` (he lies where the first carrier is), and set him down at
    /// `dest` (counted `carried`). A carrier dead, down, wounded (hit) or
    /// re-ordered sets him down where he is; the other carrier stops.
    fn carry_tick(&mut self, ctx: &mut Ctx) {
        let jobs: Vec<SimId> = self
            .casualties
            .iter()
            .filter(|(_, c)| c.carriers[0].is_some())
            .map(|(&id, _)| id)
            .collect();
        for id in jobs {
            let c = self.casualties[&id].clone();
            let i = id.index();
            let owner = ctx.ents.owner[i];
            let at = ctx.ents.transform[i].pos.xy();
            let Some(dest) = c.dest else {
                self.set_down(ctx, id, &[]);
                continue;
            };
            let men: Vec<SimId> = c.carriers.iter().flatten().copied().collect();
            // Each carrier: 0 fetching, 1 waiting at him, 2 carrying,
            // 3 arrived with him; None: he dropped out.
            let phase = |ctx: &Ctx, m: SimId| -> Option<u8> {
                let j = m.index();
                if ctx.ents.kind[j] != EntityKind::Soldier
                    || ctx.ents.owner[j] != owner
                    || ctx.ents.combat[j].wound.state != crate::wounds::FIT
                    || !ctx.ents.unit[j].detached
                    || ctx.ents.unit[j].riding.is_some()
                {
                    return None;
                }
                let o = &ctx.ents.unit[j].order;
                let p = ctx.ents.transform[j].pos.xy();
                if *o == (UnitOrder::Move { target: dest }) {
                    Some(2)
                } else if *o == (UnitOrder::Move { target: at }) {
                    Some(0)
                } else if *o == UnitOrder::Idle && (p - at).length_sq() <= reach_sq() {
                    Some(if (p - dest).length_sq() <= reach_sq() {
                        3
                    } else {
                        1
                    })
                } else {
                    None
                }
            };
            let ph: Vec<Option<u8>> = men.iter().map(|&m| phase(ctx, m)).collect();
            if ph.iter().any(|p| p.is_none()) {
                let stay: Vec<SimId> = men
                    .iter()
                    .zip(&ph)
                    .filter(|(_, p)| p.is_some())
                    .map(|(&m, _)| m)
                    .collect();
                self.set_down(ctx, id, &stay);
                continue;
            }
            if ph.iter().all(|&p| p == Some(1)) {
                // Everyone is at him: lift and go.
                for &m in &men {
                    self.send_man(ctx, m, dest);
                }
                continue;
            }
            if ph.contains(&Some(2)) || ph.contains(&Some(3)) {
                // He lies at the first carrier.
                let lead = men[0];
                let p = ctx.ents.transform[lead.index()].pos;
                if ctx.ents.transform[i].pos != p {
                    ctx.ents.transform[i].pos = p;
                    ctx.hash.mark_entity_chunk(id.chunk());
                }
                if ph.iter().all(|&p| p == Some(3)) {
                    self.bump_s12(owner.0, |s| &mut s.carried);
                    let cm = self.casualties.get_mut(&id).expect("casualty");
                    cm.carriers = [None, None];
                    cm.dest = None;
                    for &m in &men {
                        Self::release(ctx, m);
                    }
                    ctx.hash.mark_entity_chunk(id.chunk());
                }
                // Else the first there waits for the other.
            }
        }
    }

    /// W7: casualty `id` is set down where he lies; carriers in `stop`
    /// halt where they stand.
    fn set_down(&mut self, ctx: &mut Ctx, id: SimId, stop: &[SimId]) {
        if let Some(c) = self.casualties.get_mut(&id) {
            c.carriers = [None, None];
            c.dest = None;
        }
        for &m in stop {
            Self::halt_man(ctx, m);
        }
        ctx.hash.mark_entity_chunk(id.chunk());
    }

    // ---------------------------------------------------------- commands --

    /// `Tend { units, target }`: the nearest of `units` (ties: lower id)
    /// goes to `target`. A downed man of the player (not stabilised, not
    /// carried) takes a medic (W6 stabilise); an unbandaged wounded man of
    /// the player takes anyone else (W6 buddy bandage). Refused with
    /// `[wounds] enabled` 0.
    pub(crate) fn order_tend(
        &mut self,
        ctx: &mut Ctx,
        p: PlayerId,
        units: &[SimId],
        target: SimId,
    ) -> bool {
        if self.setup.rules.wounds.enabled == 0 {
            return false;
        }
        let Some(men) = self.unit_set(ctx, p, units) else {
            return false;
        };
        let t = target.index();
        if t >= ctx.ents.len() || ctx.ents.owner[t] != p {
            return false;
        }
        let carrying = self.carriers();
        let downed = match ctx.ents.kind[t] {
            EntityKind::Downed => match self.casualties.get(&target) {
                Some(c) if c.bleed_out_at != 0 && c.carriers[0].is_none() => true,
                _ => return false,
            },
            EntityKind::Soldier => {
                let w = ctx.ents.combat[t].wound;
                if w.state != WOUNDED || w.bandaged {
                    return false;
                }
                false
            }
            _ => return false,
        };
        let at = ctx.ents.transform[t].pos.xy();
        let pick = men
            .iter()
            .copied()
            .filter(|&m| {
                m != target
                    && carrying.binary_search_by_key(&m, |x| x.0).is_err()
                    && (!downed || self.is_medic(ctx.ents, m.index()))
            })
            .min_by_key(|&m| ((ctx.ents.transform[m.index()].pos.xy() - at).length_sq(), m));
        let Some(m) = pick else {
            return false;
        };
        // A buddy stands a metre short of a wounded man (not on him).
        let to = if downed {
            at
        } else {
            let from = ctx.ents.transform[m.index()].pos.xy();
            ctx.terrain.clamp(at + (from - at).normalize_or_zero())
        };
        self.send_man(ctx, m, to);
        true
    }

    /// `Carry { units, target, to }`: W7. Two medics of `units` carry
    /// downed man `target` on a stretcher; else the two nearest (one if
    /// only one) drag him; to `to` (clamped). Refused for a man already
    /// carried (a second pair), with men already carrying, or with
    /// `[wounds] enabled` 0.
    pub(crate) fn order_carry(
        &mut self,
        ctx: &mut Ctx,
        p: PlayerId,
        units: &[SimId],
        target: SimId,
        to: FxVec2,
    ) -> bool {
        if self.setup.rules.wounds.enabled == 0 {
            return false;
        }
        let Some(men) = self.unit_set(ctx, p, units) else {
            return false;
        };
        let t = target.index();
        if t >= ctx.ents.len() || ctx.ents.kind[t] != EntityKind::Downed || ctx.ents.owner[t] != p {
            return false;
        }
        match self.casualties.get(&target) {
            Some(c) if c.carriers[0].is_none() => {}
            _ => return false,
        }
        let carrying = self.carriers();
        let at = ctx.ents.transform[t].pos.xy();
        let by_dist = |set: Vec<SimId>| {
            let mut v: Vec<(Fx, SimId)> = set
                .into_iter()
                .map(|m| ((ctx.ents.transform[m.index()].pos.xy() - at).length_sq(), m))
                .collect();
            v.sort_unstable();
            v.into_iter().map(|x| x.1).collect::<Vec<_>>()
        };
        let free: Vec<SimId> = men
            .into_iter()
            .filter(|&m| {
                carrying.binary_search_by_key(&m, |x| x.0).is_err()
                    && ctx.ents.combat[m.index()].wound.state == crate::wounds::FIT
            })
            .collect();
        let medics: Vec<SimId> = free
            .iter()
            .copied()
            .filter(|&m| self.is_medic(ctx.ents, m.index()))
            .collect();
        let pick = if medics.len() >= 2 {
            by_dist(medics)
        } else {
            by_dist(free)
        };
        if pick.is_empty() {
            return false;
        }
        let to = ctx.terrain.clamp(to);
        // A tend in progress ends: he is lifted.
        let c = self.casualties.get_mut(&target).expect("casualty");
        c.tend = None;
        c.carriers = [pick.first().copied(), pick.get(1).copied()];
        c.dest = Some(to);
        ctx.hash.mark_entity_chunk(target.chunk());
        for m in pick.into_iter().take(2) {
            self.send_man(ctx, m, at);
        }
        true
    }
}
