//! S14 assault kit (docs/slices/S14-assault-kit.md 3): the saved layout of
//! flame particles, burning ground, burning and striking men, trench
//! clears and the S14 counters, with their load checks (Build A's layout
//! commit), and the flame rules F1-F9. Every rule reads its switch
//! (`[flame] enabled`, `[melee] enabled`, `[clear] enabled`); at 0 no S14
//! code path runs and no RNG draw changes.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ecs::{Entities, EntityKind, SimId};
use crate::fx::{Fx, FxVec2, FxVec3};
use crate::game::{MatchState, SquadId, kill_class};

/// Hard ceiling of flame particles in flight per player (`[flame]
/// particles_per_player` is at most this).
pub const MAX_FLAME_PER_PLAYER: usize = 256;
/// Hard ceiling of burning-ground patches (`[flame] patches_max` is at
/// most this).
pub const MAX_PATCHES: usize = 256;
/// `Close::strike_phase`: no strike.
pub const STRIKE_NONE: u8 = 0;
/// `Close::strike_phase`: winding up (M3).
pub const STRIKE_WINDUP: u8 = 1;
/// `Close::strike_phase`: recovering after the strike (M3).
pub const STRIKE_RECOVER: u8 = 2;
/// Highest `Clear::phase` (C2 approach, C3 throw and wait, C4 advance).
pub const CLEAR_PHASE_MAX: u8 = 2;

/// F2: one flame particle in flight (`MatchState::flame`, ascending id).
/// Never in the projectile pool.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Flame {
    /// Never reused.
    pub id: u32,
    pub pos: FxVec3,
    /// m/s.
    pub vel: FxVec3,
    /// Ticks left (1..; a particle at 0 is removed).
    pub ttl: u16,
    pub player: u8,
    pub shooter: SimId,
    /// Index into `MatchSetup::weapons` (a Flame-class weapon).
    pub weapon: u16,
}

/// F5: a patch of burning ground (`MatchState::patches`, ascending id).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Patch {
    /// Never reused.
    pub id: u32,
    pub pos: FxVec3,
    /// The tick it goes out (always in the future in a save).
    pub ends_at: u64,
    pub player: u8,
    /// Credit for its burns: (player, kill class with the weapon's type
    /// bits, as `Wound::by`).
    pub by: (u8, u16),
}

/// S14 close-combat state of a man (`Combat::close`; default for every
/// other entity): F3 burning and M1-M3 strikes. Hashed with its chunk.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Close {
    /// F3: he burns until this tick (0: not burning).
    pub burn_until: u64,
    /// F3: who set him burning (player, kill class).
    pub burn_by: (u8, u16),
    /// M3: ticks left in the current strike phase.
    pub strike: u8,
    /// M3: `STRIKE_NONE`, `STRIKE_WINDUP` or `STRIKE_RECOVER`.
    pub strike_phase: u8,
    /// M1: where the strike goes (the target's position at the wind-up).
    pub strike_at: FxVec2,
}

/// C1-C5: a squad clearing a trench (`MatchState::clears`).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Clear {
    /// The ordered point.
    pub at: FxVec2,
    /// The current step's dug cell (4 m grid).
    pub cell: (u16, u16),
    /// The entry cell (C3 walks away from it).
    pub from: (u16, u16),
    /// Steps done (at most `[clear] steps_max`).
    pub step: u8,
    /// 0 approach, 1 throw and wait, 2 advance.
    pub phase: u8,
    /// The tick the phase began.
    pub phase_at: u64,
}

/// S14 per-player counters (saved and hashed, read only by reports).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct S14Counters {
    /// F1: particles fired, and requests dropped over the per-player cap.
    pub flame_fired: u32,
    pub flame_capped: u32,
    /// F3: particles that struck a man, of them in a dug cell, in a pit.
    pub flame_hits: u32,
    pub flame_in_trench_hits: u32,
    pub flame_in_bunker_hits: u32,
    /// F3, F5: enemy men downed and killed by fire.
    pub burn_downs: u32,
    pub burn_kills: u32,
    /// F5: patches made (dropped ones at the cap are not), and burns
    /// dealt by patches.
    pub patches_made: u32,
    pub patch_hits: u32,
    /// F6: men who stepped out of burning ground.
    pub stepped_out: u32,
    /// M1-M4: strikes made, hits, kills.
    pub strikes: u32,
    pub strike_hits: u32,
    pub strike_kills: u32,
    /// C1-C5: clear orders taken, steps done, clears ended.
    pub clears: u32,
    pub clear_steps: u32,
    pub clears_done: u32,
    /// F5: patches dropped at `patches_max`.
    pub patches_dropped: u32,
}

/// Load check of one entity's `Close` (spec 3.1). `burn_max`: the latest
/// tick a burn may end at.
pub(crate) fn close_ok(
    kind: EntityKind,
    c: &Close,
    players: usize,
    vehicles: usize,
    burn_max: u64,
) -> bool {
    let man = matches!(kind, EntityKind::Soldier | EntityKind::Downed);
    if !man {
        return *c == Close::default();
    }
    let by_ok = |by: (u8, u16)| {
        (by.0 as usize) < players
            && (kill_class::base(by.1) as usize) < kill_class::COUNT
            && kill_class::vehicle(by.1).is_none_or(|t| t < vehicles)
    };
    let burn_ok = if c.burn_until == 0 {
        c.burn_by == (0, 0)
    } else {
        c.burn_until <= burn_max && by_ok(c.burn_by)
    };
    let strike_ok = match c.strike_phase {
        STRIKE_NONE => c.strike == 0 && c.strike_at == FxVec2::default(),
        STRIKE_WINDUP | STRIKE_RECOVER => c.strike > 0 && kind == EntityKind::Soldier,
        _ => false,
    };
    burn_ok && strike_ok
}

/// The rule bounds the S14 load checks read.
pub(crate) struct S14Limits {
    /// Particles in flight per player.
    pub particles: usize,
    /// A particle's longest life (ticks).
    pub ttl_ticks: u64,
    pub patches: usize,
    /// A patch's longest life (ticks).
    pub patch_ticks: u64,
    /// Most clear steps.
    pub steps: u8,
    /// A burn's longest life (ticks).
    pub burn_ticks: u64,
}

impl MatchState {
    /// S14 F1-F2: the flame particles in flight, ascending id.
    pub fn flame(&self) -> &[Flame] {
        &self.flame
    }

    /// S14 F5: the burning-ground patches, ascending id.
    pub fn patches(&self) -> &[Patch] {
        &self.patches
    }

    /// S14 C1-C5: the squads clearing a trench.
    pub fn clears(&self) -> &Clears {
        &self.clears
    }

    /// S14 F1: flame requests (ascending request order) become particles
    /// in `MatchState::flame`, never pool rounds; one over the player's
    /// cap is dropped and counted in `flame_capped` (not `shots_dropped`).
    pub(crate) fn spawn_flame(&mut self, reqs: &[crate::systems::FireRequest]) {
        if !reqs.iter().any(|r| r.flame) {
            return;
        }
        let lim = self.s14_limits();
        let mut live = vec![0usize; self.players.len()];
        for f in &self.flame {
            live[f.player as usize] += 1;
        }
        for r in reqs.iter().filter(|r| r.flame) {
            let p = r.player as usize;
            if live[p] >= lim.particles {
                self.players[p].s14.flame_capped += 1;
                continue;
            }
            live[p] += 1;
            let id = self.next_flame;
            self.next_flame += 1;
            self.flame.push(Flame {
                id,
                pos: r.pos,
                vel: r.vel,
                ttl: lim.ttl_ticks as u16,
                player: r.player,
                shooter: r.shooter,
                weapon: r.weapon,
            });
            self.players[p].s14.flame_fired += 1;
        }
    }

    /// The S14 bounds of this match's rules.
    pub(crate) fn s14_limits(&self) -> S14Limits {
        let hz = self.setup.tick_hz as u64;
        let r = &self.setup.rules;
        let f = &r.flame;
        S14Limits {
            particles: (f.particles_per_player as usize).min(MAX_FLAME_PER_PLAYER),
            ttl_ticks: (f.ttl_ms as u64 * hz)
                .div_ceil(1000)
                .clamp(1, u16::MAX as u64),
            patches: (f.patches_max as usize).min(MAX_PATCHES),
            patch_ticks: f.patch_s as u64 * hz,
            steps: r.clear.steps_max.min(u8::MAX as u32) as u8,
            burn_ticks: f.burn_s as u64 * hz,
        }
    }

    /// Load checks of the S14 state (spec 3.1, `load_rejects_bad_s14_state`):
    /// particles, patches, clears, counters. `Close` is checked per entity
    /// with the other combat fields.
    pub(crate) fn validate_s14(&self, ents: &Entities, tick: u64) -> Result<(), &'static str> {
        let s = &self.setup;
        let np = s.players.len();
        let size = Fx::from_int(s.map.size_m);
        let on_map =
            |q: FxVec3| (Fx::ZERO..=size).contains(&q.x) && (Fx::ZERO..=size).contains(&q.y);
        let lim = self.s14_limits();
        let cap = lim.particles;
        let mut per = vec![0usize; np];
        let ttl_max = lim.ttl_ticks;
        let mut last = None;
        for p in &self.flame {
            if last.is_some_and(|l| p.id <= l) || p.id >= self.next_flame {
                return Err("flame particle ids are not ascending");
            }
            last = Some(p.id);
            if p.player as usize >= np
                || p.weapon as usize >= s.weapons.len()
                || p.shooter.index() >= ents.kind().len()
                || !on_map(p.pos)
            {
                return Err("a flame particle is bad");
            }
            if p.ttl == 0 || p.ttl as u64 > ttl_max {
                return Err("a flame particle's ttl is out of range");
            }
            per[p.player as usize] += 1;
            if per[p.player as usize] > cap {
                return Err("more flame particles than the cap");
            }
        }
        if self.patches.len() > lim.patches {
            return Err("more patches than the cap");
        }
        let nv = s.vehicles.len();
        let patch_max = crate::bounds::MAX_TICK + lim.patch_ticks + s.tick_hz as u64;
        let mut last = None;
        for p in &self.patches {
            if last.is_some_and(|l| p.id <= l) || p.id >= self.next_patch {
                return Err("patch ids are not ascending");
            }
            last = Some(p.id);
            if p.ends_at <= tick || p.ends_at > patch_max {
                return Err("a patch's end is out of range");
            }
            if p.player as usize >= np
                || !on_map(p.pos)
                || (p.by.0 as usize) >= np
                || (kill_class::base(p.by.1) as usize) >= kill_class::COUNT
                || kill_class::vehicle(p.by.1).is_some_and(|t| t >= nv)
            {
                return Err("a patch is bad");
            }
        }
        for (sq, cl) in &self.clears {
            let Some(q) = self.squads.get(sq) else {
                return Err("a clear names no squad");
            };
            if s.squad_types[q.type_idx as usize].vehicle.is_some()
                || cl.step > lim.steps
                || cl.phase > CLEAR_PHASE_MAX
                || cl.phase_at > tick
            {
                return Err("a clear is bad");
            }
            let on = |v: Fx| v >= Fx::ZERO && v <= size;
            if !on(cl.at.x) || !on(cl.at.y) {
                return Err("a clear's point is off the map");
            }
        }
        for pl in &self.players {
            if !crate::bounds::counters_ok(&pl.s14) {
                return Err("a player counter is out of range");
            }
        }
        Ok(())
    }

    /// The clear cells must be dug trench (spec 3.1; needs the terrain,
    /// so the world calls it after [`MatchState::validate`]).
    pub(crate) fn validate_s14_terrain(
        &self,
        t: &crate::terrain::Heightfield,
    ) -> Result<(), &'static str> {
        for cl in self.clears.values() {
            for (gx, gy) in [cl.cell, cl.from] {
                if gx as u32 >= t.dim()
                    || gy as u32 >= t.dim()
                    || t.dug_dm(gx as u32, gy as u32) < crate::terrain::TRENCH_NAV_DM
                {
                    return Err("a clear cell is not dug");
                }
            }
        }
        Ok(())
    }
}

/// S14 saved match state, kept together so `MatchState` gains one field
/// per list (spec 3.1).
pub type Clears = BTreeMap<SquadId, Clear>;

// ------------------------------------------------------------- F2-F5 --

use crate::combat::{BallisticEvent, Blow, FlightCtx};
use crate::hash::HashCache;
use crate::projectile::{NO_AMMO, Projectile};
use crate::spatial::SpatialHash;
use crate::terrain::Heightfield;

/// A burn pulse event (F3, F5): no weapon, no pool slot.
fn burn_event(player: u8, blow: Blow) -> BallisticEvent {
    BallisticEvent {
        weapon: 0,
        player,
        ammo: NO_AMMO,
        slot: u32::MAX,
        shooter: SimId(u32::MAX),
        blow,
        mission: 0,
    }
}

impl MatchState {
    /// Whether the S14 flame stage has anything to do: the switch on, or
    /// a particle or patch left (a staged save).
    pub(crate) fn s14_live(&self) -> bool {
        self.setup.rules.flame.enabled != 0 || !self.flame.is_empty() || !self.patches.is_empty()
    }

    /// S14 F2-F5, serial, in the Ballistics stage after `fly_thrown`:
    /// particles fly (ascending id) through the projectile integrator
    /// `combat::fly` (terrain, enemy static boxes, vehicles, enemy capsules
    /// and downed men; near misses at `[flame] near_cm`); hits and near
    /// misses join `blows` for the Damage stage (the weapon's damage and
    /// suppression, class FLAME); a struck man burns; a landing (on the
    /// ground, or at a struck man's feet) makes or refreshes a patch; then
    /// the burn and patch pulses add `Blow::Burn` events, and burns and
    /// patches that ran out end.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn flame_stage(
        &mut self,
        ents: &mut Entities,
        hash: &mut HashCache,
        spatial: &SpatialHash,
        downed: &SpatialHash,
        terrain: &Heightfield,
        substeps: u32,
        tick: u64,
        blows: &mut Vec<BallisticEvent>,
    ) {
        let hz = self.setup.tick_hz as u64;
        let f = self.setup.rules.flame.clone();
        let lim = self.s14_limits();
        // F2: flight.
        let mut parts = std::mem::take(&mut self.flame);
        let mut landed: Vec<(FxVec3, u8)> = Vec::new();
        let mut struck: Vec<(SimId, u8)> = Vec::new();
        {
            let s = &self.setup;
            let ctx = FlightCtx {
                ents: &*ents,
                spatial,
                terrain,
                weapons: &s.weapons,
                near_miss: Fx::from_int(f.near_cm as i32).div_int(100),
                body: &s.rules.combat.body,
                vehicles: (!self.vehicles.is_empty()).then_some(&self.vderived),
                armour: self.armour(),
                ammo: &s.ammo,
                statics: self.static_boxes(),
                downed: (!self.casualties.is_empty()).then_some(downed),
                wounds: (s.rules.wounds.enabled != 0).then_some(&s.rules.wounds),
            };
            let (mut ended, mut ev, mut tests) = (Vec::new(), Vec::new(), 0u64);
            let dt = Fx::ONE.div_int(60);
            parts.retain_mut(|p| {
                let mut slot = [Some(Projectile {
                    pos: p.pos,
                    vel: p.vel,
                    owner: p.shooter,
                    drag: Fx::ZERO,
                    ttl: u16::MAX,
                    weapon: p.weapon,
                    player: p.player,
                    ammo: NO_AMMO,
                    origin: p.pos,
                    mission: 0,
                })];
                for _ in 0..substeps {
                    let before = slot[0].expect("in flight");
                    ended.clear();
                    ev.clear();
                    crate::combat::fly(0, &mut slot, &ctx, 1, &mut ended, &mut ev, &mut tests);
                    for e in ev.drain(..) {
                        match e.blow {
                            Blow::Hit(id, _) => {
                                struck.push((id, p.player));
                                blows.push(BallisticEvent {
                                    slot: u32::MAX,
                                    ..e
                                });
                            }
                            Blow::NearMiss(_) => blows.push(BallisticEvent {
                                slot: u32::MAX,
                                ..e
                            }),
                            // S14-6 (A): vehicles, pieces, crates: no effect.
                            _ => {}
                        }
                    }
                    if slot[0].is_none() {
                        // A ground landing burns at the middle of the last
                        // substep, on the surface.
                        if ended
                            .first()
                            .is_some_and(|e| e.1 == crate::projectile::ProjectileEnd::GroundHit)
                        {
                            let mid = before.pos + (before.vel * dt).div_int(2);
                            let z = terrain.height_at(mid.xy());
                            landed.push((FxVec3::new(mid.x, mid.y, z), p.player));
                        }
                        return false;
                    }
                }
                let q = slot[0].expect("in flight");
                (p.pos, p.vel) = (q.pos, q.vel);
                p.ttl -= 1;
                p.ttl > 0
            });
        }
        self.flame = parts;
        // F3: a struck man burns (refreshed by each hit), and the stream
        // lands at his feet.
        let burn_ticks = lim.burn_ticks;
        for &(id, player) in &struck {
            let i = id.index();
            let c = &mut ents.combat[i];
            c.close.burn_until = tick + burn_ticks;
            c.close.burn_by = (player, kill_class::FLAME);
            hash.mark_entity_chunk(id.chunk());
            // Under cover: a static box (a roof) over his head within its
            // footprint's reach (the bunker's men, RE done-when 1).
            let at = ents.transform[i].pos;
            let head = at.z + c.height;
            let roofed = self.static_boxes().iter().any(|b| {
                b.frame.center.z - b.frame.half.z >= head - Fx::from_milli(500)
                    && (b.frame.center.xy() - at.xy()).length() <= b.reach
            });
            let s14 = &mut self.players[player as usize].s14;
            s14.flame_hits += 1;
            if c.in_trench {
                s14.flame_in_trench_hits += 1;
            }
            if roofed {
                s14.flame_in_bunker_hits += 1;
            }
            landed.push((at, player));
        }
        // F5: landings make patches or refresh one within `patch_merge_cm`.
        let merge = Fx::from_int(f.patch_merge_cm as i32).div_int(100);
        let ends = tick + lim.patch_ticks;
        for (pt, player) in landed {
            if let Some(k) = self
                .patches
                .iter()
                .position(|q| (q.pos.xy() - pt.xy()).length() <= merge)
            {
                self.patches[k].ends_at = ends;
                continue;
            }
            if self.patches.len() >= lim.patches {
                self.players[player as usize].s14.patches_dropped += 1;
                continue;
            }
            let id = self.next_patch;
            self.next_patch += 1;
            self.patches.push(Patch {
                id,
                pos: pt,
                ends_at: ends,
                player,
                by: (player, kill_class::FLAME),
            });
            self.players[player as usize].s14.patches_made += 1;
        }
        self.patches.retain(|q| q.ends_at > tick);
        // F3, F5 pulses, `(tick + id) % pulse` in id order.
        let pulse = (f.pulse_ms as u64 * hz).div_ceil(1000).max(1);
        let burn_dmg = f.burn_hp_per_s * f.pulse_ms / 1000;
        let patch_dmg = f.patch_hp_per_s * f.pulse_ms / 1000;
        let radius = Fx::from_int(f.patch_radius_cm as i32).div_int(100);
        let zmax = Fx::from_int(f.patch_z_cm as i32).div_int(100);
        let mut men: Vec<SimId> = Vec::new();
        for k in 0..self.patches.len() {
            let q = self.patches[k];
            if !(tick + q.id as u64).is_multiple_of(pulse) {
                continue;
            }
            men.clear();
            let mut take = |id: SimId| {
                let t = &ents.transform[id.index()];
                if (t.pos.xy() - q.pos.xy()).length() <= radius && (t.pos.z - q.pos.z).abs() <= zmax
                {
                    men.push(id);
                }
            };
            spatial.for_each_near(q.pos.xy(), radius, &mut take);
            if !self.casualties.is_empty() {
                downed.for_each_near(q.pos.xy(), radius, &mut take);
            }
            men.sort_unstable();
            men.dedup();
            for &id in &men {
                blows.push(burn_event(
                    q.by.0,
                    Blow::Burn {
                        id,
                        dmg: patch_dmg,
                        by: q.by,
                        supp: 0,
                    },
                ));
                let c = &mut ents.combat[id.index()].close;
                if c.burn_until == 0 {
                    c.burn_by = q.by;
                }
                c.burn_until = c.burn_until.max(tick + burn_ticks);
                hash.mark_entity_chunk(id.chunk());
                self.players[q.player as usize].s14.patch_hits += 1;
            }
        }
        for i in 0..ents.kind.len() {
            let c = &mut ents.combat[i].close;
            if c.burn_until == 0 {
                continue;
            }
            let id = SimId(i as u32);
            if c.burn_until <= tick
                || !matches!(ents.kind[i], EntityKind::Soldier | EntityKind::Downed)
            {
                c.burn_until = 0;
                c.burn_by = (0, 0);
                hash.mark_entity_chunk(id.chunk());
                continue;
            }
            if (tick + i as u64).is_multiple_of(pulse) {
                blows.push(burn_event(
                    c.burn_by.0,
                    Blow::Burn {
                        id,
                        dmg: burn_dmg,
                        by: c.burn_by,
                        supp: f.burn_suppression,
                    },
                ));
            }
        }
    }
}

// ------------------------------------------------------------------ F6 --

impl MatchState {
    /// The patch whose burning ground holds a man's feet at `p`, if any.
    fn patch_at(&self, p: FxVec3) -> Option<&Patch> {
        let f = &self.setup.rules.flame;
        let radius = Fx::from_int(f.patch_radius_cm as i32).div_int(100);
        let zmax = Fx::from_int(f.patch_z_cm as i32).div_int(100);
        self.patches
            .iter()
            .find(|q| (p.xy() - q.pos.xy()).length() <= radius && (p.z - q.pos.z).abs() <= zmax)
    }

    /// S14 F6 (Status stage, ascending id): with `[flame] step_out` 1, a
    /// soldier whose feet are in a patch and who has no move order of his
    /// own (in formation, or detached and idle) takes a detached move to
    /// the first of 8 points `step_out_m` beyond that patch's edge, from
    /// the direction away from its centre in ascending 45° steps, that is
    /// on the map and in no patch. Riders and gun crews stay. He rejoins
    /// at his squad's next order (the S12 detached rule).
    pub(crate) fn step_out_tick(&mut self, ctx: &mut crate::game::Ctx) {
        let f = &self.setup.rules.flame;
        if f.enabled == 0 || f.step_out == 0 || self.patches.is_empty() {
            return;
        }
        let out =
            Fx::from_int(f.patch_radius_cm as i32).div_int(100) + Fx::from_int(f.step_out_m as i32);
        let size = Fx::from_int(self.setup.map.size_m);
        let (c45, s45) = (Fx::from_milli(707), Fx::from_milli(707));
        let mut moves: Vec<(SimId, FxVec2)> = Vec::new();
        for i in 0..ctx.ents.kind.len() {
            if ctx.ents.kind[i] != EntityKind::Soldier || ctx.ents.squad[i].is_none() {
                continue;
            }
            let u = &ctx.ents.unit[i];
            if u.riding.is_some()
                || u.serving.is_some()
                || (u.detached && u.order != crate::unit::UnitOrder::Idle)
            {
                continue;
            }
            let pos = ctx.ents.transform[i].pos;
            let Some(q) = self.patch_at(pos) else {
                continue;
            };
            let mut dir = (pos.xy() - q.pos.xy()).normalize_or_zero();
            if dir == FxVec2::ZERO {
                dir = FxVec2::new(Fx::ONE, Fx::ZERO);
            }
            let centre = q.pos;
            for _ in 0..8 {
                let to = centre.xy() + FxVec2::new(dir.x.mul(out), dir.y.mul(out));
                let on = to.x >= Fx::ZERO && to.y >= Fx::ZERO && to.x <= size && to.y <= size;
                let z = ctx.terrain.height_at(to);
                if on && self.patch_at(FxVec3::new(to.x, to.y, z)).is_none() {
                    moves.push((SimId(i as u32), to));
                    break;
                }
                dir = FxVec2::new(
                    dir.x.mul(c45) - dir.y.mul(s45),
                    dir.x.mul(s45) + dir.y.mul(c45),
                );
            }
        }
        for (m, to) in moves {
            let owner = ctx.ents.owner[m.index()].0 as usize;
            self.players[owner].s14.stepped_out += 1;
            self.send_man(ctx, m, to);
        }
    }
}
