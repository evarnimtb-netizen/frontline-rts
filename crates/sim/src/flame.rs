//! S14 assault kit (docs/slices/S14-assault-kit.md 3): the saved layout of
//! flame particles, burning ground, burning and striking men, trench
//! clears and the S14 counters, with their load checks (Build A's layout
//! commit), the flame rules F1-F9, Clear trench C1-C5 and the melee strike
//! M1-M5. Every rule reads its switch
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
/// Load bound of a particle's height (m) and of each velocity component
/// (m/s) (S14 Fix 16).
pub const FLAME_BOUND_M: i32 = 1000;
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
/// tick a burn may end at; `size`: the map's side (a strike's point is
/// on the map, S14 Fix 16).
pub(crate) fn close_ok(
    kind: EntityKind,
    c: &Close,
    players: usize,
    vehicles: usize,
    burn_max: u64,
    size: Fx,
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
        STRIKE_WINDUP | STRIKE_RECOVER => {
            let on = |v: Fx| v >= Fx::ZERO && v <= size;
            c.strike > 0 && kind == EntityKind::Soldier && on(c.strike_at.x) && on(c.strike_at.y)
        }
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
            let pl = &mut self.players[p];
            pl.s14.flame_fired += 1;
            let f = &mut pl.ammo.fired[crate::supply::fired_class::FLAME];
            *f = f.saturating_add(1);
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
            // S14 Fix 16: a Flame-class weapon, a height and speed in
            // bounds.
            let lim_m = Fx::from_int(FLAME_BOUND_M);
            if p.player as usize >= np
                || p.weapon as usize >= s.weapons.len()
                || s.weapons[p.weapon as usize].class != frontline_content::WeaponClass::Flame
                || p.shooter.index() >= ents.kind().len()
                || !on_map(p.pos)
                || p.pos.z.abs() > lim_m
                || p.vel.x.abs() > lim_m
                || p.vel.y.abs() > lim_m
                || p.vel.z.abs() > lim_m
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

/// S14 Fix 1: the height a flame man's `Suppress` shot aims at, at point
/// `p` of his line. A point within `[flame] slit_near_cm` of the
/// footprint of an enemy roof raised over a pit (a fort with a `pit`
/// whose piece sits `z_from = "base"` above the undug surface: the
/// bunker) is aimed at the middle of the gap under that roof (the slit,
/// from the fort def: base + offset z / 2); any other point at the ground
/// there (a trench's floor where dug) plus `[flame] aim_cm`.
pub(crate) fn flame_aim_z(game: &MatchState, terrain: &Heightfield, owner: u8, p: FxVec2) -> Fx {
    let f = &game.setup.rules.flame;
    let near = Fx::from_int(f.slit_near_cm as i32).div_int(100);
    for q in game.fort.pieces.values() {
        if q.owner.0 == owner {
            continue;
        }
        let Some(def) = game.setup.forts.get(q.fort as usize) else {
            continue;
        };
        let Some(pd) = def.pieces.get(q.part as usize) else {
            continue;
        };
        let up: Fx = pd.offset[2].into();
        if def.pit.is_none() || !pd.z_from_base || up <= Fx::ZERO {
            continue;
        }
        let (sn, cs) = q.yaw.sin_cos();
        let face = FxVec2::new(cs, sn);
        let d = p - q.pos;
        let (hx, hy): (Fx, Fx) = (pd.half[0].into(), pd.half[1].into());
        if d.dot(face).abs() <= hx + near && d.dot(face.perp()).abs() <= hy + near {
            return q.z - up + up.div_int(2);
        }
    }
    terrain.height_at(p) + Fx::from_int(f.aim_cm as i32).div_int(100)
}

/// S14 Fix 1: [`crate::combat::suppress_aim`] for a flame man: the aim
/// height from the target ([`flame_aim_z`]); the probe `clear_cm` short
/// of it as high as the aim or the undug surface there plus `aim_cm`.
pub(crate) fn flame_suppress_aim(
    game: &MatchState,
    terrain: &Heightfield,
    owner: u8,
    line: &[FxVec2; 2],
    t: Fx,
    from: FxVec3,
    range: Fx,
) -> Option<(FxVec3, FxVec3)> {
    let rules = &game.setup.rules;
    let p = line[0] + (line[1] - line[0]) * t;
    let aim = FxVec3::new(p.x, p.y, flame_aim_z(game, terrain, owner, p));
    if (aim.xy() - from.xy()).length_sq_raw() > crate::combat::range_sq(range) {
        return None;
    }
    let back = (from.xy() - p).normalize_or_zero()
        * Fx::from_ratio(rules.combat.suppress.clear_cm as i64, 100);
    let q = terrain.clamp(p + back);
    let lift = Fx::from_ratio(rules.flame.aim_cm as i64, 100);
    let ground = terrain.height_at(q).max(terrain.base_height_at(q));
    let probe = FxVec3::new(q.x, q.y, aim.z.max(ground + lift));
    Some((aim, probe))
}

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
                            // F2 end list: a landing on a static box's top
                            // burns at the point (F5); its side, vehicles,
                            // pieces' sides and crates' sides: no effect
                            // (S14-6 (A)).
                            Blow::Static(k, _) => {
                                if let Some(b) = ctx.statics.get(k as usize) {
                                    let mut q = before;
                                    let next = crate::projectile::advance(&mut q);
                                    let d = next - before.pos;
                                    if let Some((t, crate::armour::Face::Top, _)) =
                                        crate::armour::segment_box(&b.frame, before.pos, d)
                                    {
                                        landed.push((before.pos + d * t, p.player));
                                    }
                                }
                            }
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
        // F5: landings make patches or refresh one within `patch_merge_cm`
        // (none with the switch `patches` 0, S14 Fix 7).
        if f.patches == 0 {
            landed.clear();
        }
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
        // Checker 10: HP per pulse with no truncation loss. A pulse deals
        // the milli-HP due by its end less those due by its start, counted
        // from the pulse index `(tick + id) / pulse` (stateless, so nothing
        // new is saved): 15 HP/s at 500 ms deals 7, 8, 7, 8 ...
        let per_pulse = |hp_per_s: u32, n: u64| -> u32 {
            let milli = hp_per_s as u64 * f.pulse_ms as u64;
            ((milli * (n + 1)) / 1000 - (milli * n) / 1000) as u32
        };
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
            let patch_dmg = per_pulse(f.patch_hp_per_s, (tick + q.id as u64) / pulse);
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
                // Burns of enemy men only (own burns have no counter of
                // their own: none is saved).
                if ents.owner[id.index()].0 != q.by.0 {
                    self.players[q.player as usize].s14.patch_hits += 1;
                }
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
                        dmg: per_pulse(f.burn_hp_per_s, (tick + i as u64) / pulse),
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

    /// F6 (checker 6): whether a man at `from` can walk to `to`: the
    /// grade between them is within the infantry path rule's
    /// `[movement] max_slope_pct` (the step test of `crate::path`), and no
    /// static box (a bunker's wall or roof, a crate, an obstacle piece)
    /// stands at `to` from the ground to 0.5 m over it.
    pub(crate) fn walkable(&self, terrain: &Heightfield, from: FxVec3, to: FxVec3) -> bool {
        let run = (to.xy() - from.xy()).length();
        let rise = (to.z - terrain.height_at(from.xy())).abs();
        if run > Fx::ZERO
            && rise.mul_int(100).div(run)
                > Fx::from_int(self.setup.rules.movement.max_slope_pct as i32)
        {
            return false;
        }
        let probe = |dz: Fx| FxVec3::new(to.x, to.y, to.z + dz);
        !self.static_boxes().iter().any(|b| {
            b.reach >= Fx::ZERO
                && (b.frame.center.xy() - to.xy()).length() <= b.reach
                && [Fx::from_milli(100), Fx::from_milli(500)]
                    .into_iter()
                    .any(|dz| crate::armour::box_dist_sq_raw(&b.frame, probe(dz)) == 0)
        })
    }

    /// S14 F6 (Status stage, ascending id): with `[flame] step_out` 1, a
    /// soldier whose feet are in a patch and who has no move order of his
    /// own (in formation, or detached and idle) takes a detached move to
    /// the first of 8 points `step_out_m` beyond that patch's edge, from
    /// the direction away from its centre in ascending 45° steps, that is
    /// on the map, in no patch and walkable ([`Self::walkable`]). Riders
    /// and gun crews stay. He rejoins
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
            // S14 Fix 8: a man in formation steps out only while his squad
            // holds or arrives (a walking squad's men leave by themselves).
            if !u.detached
                && !ctx.ents.squad[i]
                    .and_then(|s| self.squads.get(&s))
                    .is_some_and(|q| matches!(q.nav.phase, NavPhase::Holding | NavPhase::Arriving))
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
                if on
                    && self.patch_at(FxVec3::new(to.x, to.y, z)).is_none()
                    && self.walkable(ctx.terrain, pos, FxVec3::new(to.x, to.y, z))
                {
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

// -------------------------------------------------------------- C1-C5 --

use crate::ecs::PlayerId;
use crate::game::{Ctx, NavPhase, Order};
use frontline_content::ItemKind;

/// C1: `at` must lie within this of a dug cell (m).
const CLEAR_REACH_M: i32 = 8;
/// C2: the squad closes to this far short of the entry cell (m).
const CLEAR_SHORT_M: i32 = 6;
/// C2, C4: a squad whose leader is this close to its goal has arrived (m).
const CLEAR_ARRIVE_M: i32 = 3;
/// C3: no own patch within this of the step point before advancing (m).
const CLEAR_FIRE_M: i32 = 2;

/// The centre of 4 m grid cell `c`.
fn cell_pos(c: (u16, u16)) -> FxVec2 {
    FxVec2::from_int(
        c.0 as i32 * crate::terrain::SPACING_M,
        c.1 as i32 * crate::terrain::SPACING_M,
    )
}

/// Whether grid cell (`gx`, `gy`) is dug to trench depth.
fn dug(t: &Heightfield, gx: i64, gy: i64) -> bool {
    let d = t.dim() as i64;
    (0..d).contains(&gx)
        && (0..d).contains(&gy)
        && t.dug_dm(gx as u32, gy as u32) >= crate::terrain::TRENCH_NAV_DM
}

/// C1: the dug cell nearest `at` within `CLEAR_REACH_M` (ties: the lower
/// grid index), if any.
pub(crate) fn trench_cell_near(t: &Heightfield, at: FxVec2) -> Option<(u16, u16)> {
    let s = crate::terrain::SPACING_M;
    let (cx, cy) = (
        (at.x.floor_int() + s / 2).div_euclid(s) as i64,
        (at.y.floor_int() + s / 2).div_euclid(s) as i64,
    );
    let r = (CLEAR_REACH_M / s + 1) as i64;
    let reach = FxVec2::new(Fx::from_int(CLEAR_REACH_M), Fx::ZERO).length_sq_raw();
    let mut best: Option<(u64, i64, (u16, u16))> = None;
    for gy in cy - r..=cy + r {
        for gx in cx - r..=cx + r {
            if !dug(t, gx, gy) {
                continue;
            }
            let c = (gx as u16, gy as u16);
            let d = (cell_pos(c) - at).length_sq_raw();
            let key = (d, gy * t.dim() as i64 + gx, c);
            if d <= reach && best.is_none_or(|b| (key.0, key.1) < (b.0, b.1)) {
                best = Some(key);
            }
        }
    }
    best.map(|b| b.2)
}

/// C3: the next step point from cell `cur`: a breadth-first walk over dug
/// cells (4-neighbour, at most `walk_cells` cells, not past `step_m` of
/// walk), the frontier cell (walk at least `step_m`) farthest from the
/// entry `from` (ties: the lower grid index); with no frontier the reached
/// cell farthest from the entry, if it is farther than `cur`.
pub(crate) fn next_step(
    t: &Heightfield,
    cur: (u16, u16),
    from: (u16, u16),
    step_m: u32,
    walk_cells: u32,
) -> Option<(u16, u16)> {
    let s = crate::terrain::SPACING_M as u32;
    let need = step_m.div_ceil(s).max(1);
    let dim = t.dim() as i64;
    let idx = |c: (u16, u16)| c.1 as i64 * dim + c.0 as i64;
    let far = |c: (u16, u16)| (cell_pos(c) - cell_pos(from)).length_sq_raw();
    let mut seen = std::collections::BTreeSet::from([idx(cur)]);
    let mut queue = std::collections::VecDeque::from([(cur, 0u32)]);
    let mut reached: Vec<((u16, u16), u32)> = Vec::new();
    while let Some((c, d)) = queue.pop_front() {
        reached.push((c, d));
        if d >= need || reached.len() as u32 >= walk_cells {
            continue;
        }
        for (dx, dy) in [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)] {
            let (nx, ny) = (c.0 as i64 + dx, c.1 as i64 + dy);
            if !dug(t, nx, ny) || !seen.insert(ny * dim + nx) {
                continue;
            }
            queue.push_back(((nx as u16, ny as u16), d + 1));
        }
    }
    let best = |it: &mut dyn Iterator<Item = (u16, u16)>| {
        it.max_by(|a, b| far(*a).cmp(&far(*b)).then(idx(*b).cmp(&idx(*a))))
    };
    if let Some(c) = best(&mut reached.iter().filter(|r| r.1 >= need).map(|r| r.0)) {
        return Some(c);
    }
    best(&mut reached.iter().map(|r| r.0)).filter(|&c| far(c) > far(cur))
}

impl MatchState {
    /// C1 `ClearTrench { squad, at }`: refused (`NotAllowed`) with `[clear]
    /// enabled` 0, for a squad not the player's, a vehicle or gun squad, a
    /// squad with no frag among its men, or `at` farther than 8 m from a
    /// dug cell (`BadSpot`). Accepted: the squad AttackMoves to 6 m short of
    /// the entry cell on its side, crouched (C2); a clear it had is
    /// replaced.
    pub(crate) fn order_clear(
        &mut self,
        ctx: &mut Ctx,
        p: PlayerId,
        squad: SquadId,
        at: FxVec2,
    ) -> bool {
        use crate::fort::FortRefusal;
        if self.setup.rules.clear.enabled == 0
            || !self.owned(p, squad)
            || self.is_vehicle_squad(squad)
            || self.is_gun_squad(ctx.ents, squad)
        {
            return self.refuse_fort(p, Some(squad), FortRefusal::NotAllowed);
        }
        let frag = ItemKind::Frag.index();
        let men = self.squads[&squad].members.clone();
        if !men.iter().any(|m| ctx.ents.kit[m.index()].items[frag] > 0) {
            return self.refuse_fort(p, Some(squad), FortRefusal::NotAllowed);
        }
        let Some(entry) = trench_cell_near(ctx.terrain, at) else {
            return self.refuse_fort(p, Some(squad), FortRefusal::BadSpot);
        };
        let e = cell_pos(entry);
        let lead = ctx.ents.transform[self.squads[&squad].lead(ctx.ents).index()]
            .pos
            .xy();
        let mut dir = (lead - e).normalize_or_zero();
        if dir == FxVec2::ZERO {
            dir = FxVec2::new(Fx::ONE, Fx::ZERO);
        }
        let short = Fx::from_int(CLEAR_SHORT_M);
        let goal = e + FxVec2::new(dir.x.mul(short), dir.y.mul(short));
        if !self.order_move(ctx, p, squad, Order::AttackMove { target: goal }) {
            return false;
        }
        self.set_stance(ctx, p, &men, crate::unit::StanceOrder::Crouch);
        self.clears.insert(
            squad,
            Clear {
                at: ctx.terrain.clamp(at),
                cell: entry,
                from: entry,
                step: 0,
                phase: 0,
                phase_at: ctx.tick,
            },
        );
        self.players[p.0 as usize].s14.clears += 1;
        true
    }

    /// C1: any later order to a clearing squad (or to one of its men) ends
    /// its clear (not counted as done); S14 Fix 15: the C2 crouch ends with
    /// it.
    pub(crate) fn clear_ended_by(&mut self, ctx: &mut Ctx, kind: &crate::command::CommandKind) {
        let ents = &*ctx.ents;
        use crate::command::CommandKind as K;
        let units: &[SimId] = match kind {
            K::ClearTrench { .. } => return,
            K::MoveUnits { units, .. }
            | K::AttackMoveUnits { units, .. }
            | K::RetreatUnits { units }
            | K::StopUnits { units }
            | K::DigUnits { units, .. }
            | K::Throw { units, .. }
            | K::ThrowEach { units, .. }
            | K::ReverseUnits { units, .. }
            | K::FaceUnits { units, .. }
            | K::AttackUnit { units, .. }
            | K::SuppressUnits { units, .. }
            | K::Tend { units, .. }
            | K::Carry { units, .. }
            | K::PlaceBangalore { units, .. }
            | K::Strike { units } => units,
            _ => &[],
        };
        let mut ended: Vec<SquadId> = units
            .iter()
            .filter_map(|m| ents.squad.get(m.index()).copied().flatten())
            .collect();
        if let Some(sq) = crate::game::command_squad(kind) {
            ended.push(sq);
        }
        if let K::Embark { squads, .. } = kind {
            ended.extend(squads.iter().copied());
        }
        for sq in ended {
            if self.clears.remove(&sq).is_some()
                && let Some(q) = self.squads.get(&sq)
            {
                let (p, men) = (q.owner, q.members.clone());
                self.set_stance(ctx, p, &men, crate::unit::StanceOrder::Auto);
            }
        }
    }

    /// C2-C5 (Status stage, ascending squad id): each clearing squad
    /// approaches (0), throws at and waits on its step point (1; the first
    /// step point is the entry cell, S14 Fix 11), advances into it (2),
    /// step by step until the job ends (C5; S14 Fix 6: also when its cell
    /// or entry is no longer dug).
    pub(crate) fn clear_tick(&mut self, ctx: &mut Ctx) {
        if self.clears.is_empty() {
            return;
        }
        let hz = self.setup.tick_hz as u64;
        let rules = self.setup.rules.clear.clone();
        let flame_on = self.setup.rules.flame.enabled != 0;
        let patch_wait = self.setup.rules.flame.patch_s as u64 * hz;
        let ids: Vec<SquadId> = self.clears.keys().copied().collect();
        for sq in ids {
            let Some(q) = self.squads.get(&sq) else {
                self.clears.remove(&sq);
                continue;
            };
            let p = q.owner;
            let men = q.members.clone();
            let ty = &self.setup.squad_types[q.type_idx as usize];
            let lead = ctx.ents.transform[q.lead(ctx.ents).index()].pos.xy();
            let holding = q.nav.phase == NavPhase::Holding;
            let mut cl = self.clears[&sq];
            let frag = ItemKind::Frag.index();
            let frags = men.iter().any(|m| ctx.ents.kit[m.index()].items[frag] > 0);
            let fuel = self.flame_men(ctx.ents, &men).next().is_some();
            // C5: under half strength, or nothing left to clear with; S14
            // Fix 6: or its cell or entry no longer dug (a crater filled
            // it): a clear never holds a cell a save would refuse.
            let undug = |c: (u16, u16)| !dug(ctx.terrain, c.0 as i64, c.1 as i64);
            let end = (men.len() as u32) * 2 < ty.men as u32
                || (!frags && !fuel)
                || undug(cl.cell)
                || undug(cl.from);
            let near = |to: FxVec2, m: i32| {
                (lead - to).length_sq_raw()
                    <= FxVec2::new(Fx::from_int(m), Fx::ZERO).length_sq_raw()
            };
            let mut done = end;
            if !done {
                match cl.phase {
                    0 | 2 => {
                        let to = cell_pos(cl.cell);
                        let reach = if cl.phase == 0 {
                            CLEAR_SHORT_M + CLEAR_ARRIVE_M
                        } else {
                            CLEAR_ARRIVE_M
                        };
                        if holding || near(to, reach) {
                            // S14 Fix 11: the first step is the entry cell
                            // itself (the enemy the order named stands
                            // there), then the walk along the trench.
                            let next = if cl.phase == 0 {
                                Some(cl.cell)
                            } else {
                                next_step(
                                    ctx.terrain,
                                    cl.cell,
                                    cl.from,
                                    rules.step_m,
                                    rules.walk_cells,
                                )
                            };
                            if cl.phase == 2 && cl.step as u32 >= rules.steps_max {
                                done = true;
                            } else if let Some(next) = next {
                                cl.cell = next;
                                cl.step += 1;
                                cl.phase = 1;
                                cl.phase_at = ctx.tick;
                                self.players[p.0 as usize].s14.clear_steps += 1;
                                self.clear_throw(ctx, &men, cell_pos(next), rules.throwers);
                            } else {
                                done = true;
                            }
                        }
                    }
                    _ => {
                        let pt = cell_pos(cl.cell);
                        // S14 Fix 5: the flame man fires one burst (C3),
                        // then holds (stateless: from `phase_at`).
                        if flame_on {
                            self.clear_burst_end(ctx, &men, pt, cl.phase_at);
                        }
                        let wait = self.clear_fuse(ty) + rules.settle_s as u64 * hz;
                        let due = ctx.tick >= cl.phase_at + wait;
                        let r = Fx::from_int(CLEAR_FIRE_M);
                        let burning = flame_on
                            && ctx.tick < cl.phase_at + wait + patch_wait
                            && self.patches.iter().any(|x| {
                                x.player == p.0
                                    && (x.pos.xy() - pt).length_sq_raw()
                                        <= FxVec2::new(r, Fx::ZERO).length_sq_raw()
                            });
                        if due && !burning {
                            cl.phase = 2;
                            cl.phase_at = ctx.tick;
                            if !self.order_move(ctx, p, sq, Order::AttackMove { target: pt }) {
                                done = true;
                            }
                        }
                    }
                }
            }
            if done {
                self.clears.remove(&sq);
                self.players[p.0 as usize].s14.clears_done += 1;
                // The squad holds the trench where it stands; S14 Fix 15:
                // the C2 crouch ends with the clear.
                self.order_stop(ctx, p, sq);
                self.set_stance(ctx, p, &men, crate::unit::StanceOrder::Auto);
            } else {
                self.clears.insert(sq, cl);
            }
        }
    }

    /// The squad's flame men with fuel (a Flame-class weapon in hand, fuel in
    /// it or carried).
    fn flame_men<'a>(
        &'a self,
        ents: &'a Entities,
        men: &'a [SimId],
    ) -> impl Iterator<Item = SimId> + 'a {
        men.iter().copied().filter(move |m| {
            let i = m.index();
            let k = &ents.kit[i];
            let w = &self.setup.weapons[k.weapon as usize];
            w.class == frontline_content::WeaponClass::Flame
                && !k.on_sidearm
                && (ents.combat[i].mag > 0
                    || ents.squad[i]
                        .and_then(|s| self.squads.get(&s))
                        .map(|q| &self.setup.squad_types[q.type_idx as usize])
                        .and_then(|ty| ty.feed_slot(w))
                        .is_some_and(|f| k.rounds[f] > 0))
        })
    }

    /// S14 Fix 5 (C3 "fires one burst"): from `burst` ticks after the step
    /// began (his weapon's `burst_rounds` shots at its interval, at least
    /// one), a flame man of `men` on the detached `Suppress` of the step
    /// point `pt` holds fire (idle, detached: he rejoins at the advance).
    fn clear_burst_end(&self, ctx: &mut Ctx, men: &[SimId], pt: FxVec2, from: u64) {
        for &m in men {
            let i = m.index();
            let u = &ctx.ents.unit[i];
            if !u.detached || u.order != (crate::unit::UnitOrder::Suppress { line: [pt, pt] }) {
                continue;
            }
            let w = &self.setup.weapons[ctx.ents.kit[i].weapon as usize];
            let shots = w.burst_rounds.max(1) as u64;
            let burst = (shots * w.interval_milli as u64).div_ceil(1000).max(1);
            if ctx.tick >= from + burst {
                ctx.ents.unit[i].order = crate::unit::UnitOrder::Idle;
                ctx.hash.mark_entity_chunk(m.chunk());
            }
        }
    }

    /// C3: the longest frag wind-up plus fuse of the squad's card (ticks).
    fn clear_fuse(&self, ty: &frontline_content::setup::SquadTypeSetup) -> u64 {
        ty.kit(&self.setup.items, ItemKind::Frag)
            .map_or(0, |(_, it)| it.windup_ticks as u64 + it.fuse_ticks as u64)
    }

    /// C3: up to `throwers` men who can throw a frag at `pt`, nearest first
    /// (ties: lower id), throw one; a flame man with fuel fires at it (a
    /// detached `Suppress` of the point; he rejoins at the advance).
    fn clear_throw(&mut self, ctx: &mut Ctx, men: &[SimId], pt: FxVec2, throwers: u32) {
        let mut can: Vec<(u64, SimId)> = men
            .iter()
            .filter_map(|&m| {
                self.can_throw(ctx.ents, m, ItemKind::Frag, pt)
                    .map(|d| (d, m))
            })
            .collect();
        can.sort_unstable();
        can.truncate(throwers as usize);
        can.sort_unstable_by_key(|&(_, m)| m);
        for (_, m) in can {
            self.start_throw(ctx, m, ItemKind::Frag, pt);
        }
        if self.setup.rules.flame.enabled == 0 {
            return;
        }
        let flamers: Vec<SimId> = self.flame_men(ctx.ents, men).collect();
        for m in flamers {
            let at = ctx.ents.transform[m.index()].pos;
            let facing = (pt - at.xy()).normalize_or_zero();
            self.detach(
                ctx,
                m,
                crate::unit::UnitOrder::Suppress { line: [pt, pt] },
                facing,
            );
            let nav = &mut ctx.ents.unit[m.index()].nav;
            nav.phase = NavPhase::Holding;
            nav.goal = at.xy();
        }
    }
}

// ------------------------------------------------------------- M1-M5 --

use crate::combat::{DamageCtx, Fallen, FireCtx};
use crate::ecs::{Combat, Stance, Transform, Weapon};
use crate::items::Kit;
use frontline_content::WeaponClass;
use frontline_content::setup::MatchSetup;

/// M1: a strike's numbers with one weapon in the man's hands: the
/// weapon's `[melee]` block, else the rules' `[melee]` (the spade).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) struct Melee {
    /// The segment's length (m).
    pub reach: Fx,
    pub damage: u32,
    /// Wind-up and recovery (ticks, 1..=255: the ranges keep 4000 ms
    /// inside a u8 at every allowed rate, `strike_ticks_fit_a_u8`).
    pub windup: u8,
    pub recover: u8,
}

/// M1: the strike of weapon `wi` (see [`Melee`]).
pub(crate) fn melee_of(setup: &MatchSetup, wi: u16) -> Melee {
    let r = &setup.rules.melee;
    let (reach_cm, damage, windup_ms, recover_ms) =
        match setup.weapons.get(wi as usize).and_then(|w| w.melee) {
            Some(m) => (m.reach_cm, m.damage, m.windup_ms, m.recover_ms),
            None => (r.reach_cm, r.damage, r.windup_ms, r.recover_ms),
        };
    let hz = setup.tick_hz as u64;
    let ticks = |ms: u32| (ms as u64 * hz).div_ceil(1000).clamp(1, u8::MAX as u64) as u8;
    Melee {
        reach: Fx::from_ratio(reach_cm.min(1000) as i64, 100),
        damage,
        windup: ticks(windup_ms),
        recover: ticks(recover_ms),
    }
}

/// M2: the nearest enemy of `owner` among `near` (live soldiers on foot)
/// whose body axis lies within `within` of `at` horizontally and whose
/// feet are inside the strike's height window `z` (S14 Fix 3,
/// [`strike_window`]); ties by id. Returns him and his position.
#[allow(clippy::too_many_arguments)]
fn nearest_enemy(
    near: impl FnOnce(&mut dyn FnMut(SimId)),
    kind: &[EntityKind],
    owners: &[PlayerId],
    transform: &[Transform],
    me: SimId,
    owner: u8,
    at: FxVec2,
    within: Fx,
    z: (Fx, Fx),
) -> Option<(SimId, FxVec2)> {
    let lim = within.raw().max(0) as u64;
    let lim_sq = lim * lim;
    let mut best: Option<(u64, SimId)> = None;
    near(&mut |id: SimId| {
        let j = id.index();
        if id == me || kind.get(j) != Some(&EntityKind::Soldier) || owners[j].0 == owner {
            return;
        }
        let fz = transform[j].pos.z;
        if fz < z.0 || fz > z.1 {
            return;
        }
        let d = (transform[j].pos.xy() - at).length_sq_raw();
        if d <= lim_sq && best.is_none_or(|b| (d, id) < b) {
            best = Some((d, id));
        }
    });
    best.map(|(_, id)| (id, transform[id.index()].pos.xy()))
}

/// M2: how far (m) a strike may start: reach, `start_pad_cm` (0 in the
/// mod since S14 Fix 3: the strike reaches only reach plus the radius)
/// and the target's capsule radius.
fn start_reach(setup: &MatchSetup, m: &Melee) -> Fx {
    m.reach
        + Fx::from_ratio(setup.rules.melee.start_pad_cm.min(1000) as i64, 100)
        + Fx::from_milli(setup.rules.combat.body.radius_mm as i32)
}

/// S14 Fix 3: the point of segment `a`-`b` nearest `p`.
fn axis_point(a: FxVec3, b: FxVec3, p: FxVec3) -> FxVec3 {
    let d = b - a;
    let len = d.dot(d);
    if len <= Fx::ZERO {
        return a;
    }
    let t = (p - a).dot(d).div(len).clamp(Fx::ZERO, Fx::ONE);
    a + d * t
}

/// S14 Fix 3: the feet heights a strike from a man with feet at `z`
/// reaches: from `[melee] down_cm` below to `up_cm` above (any stance:
/// crouched, prone, a man in a trench struck from its lip or the lip
/// struck from the trench).
fn strike_window(setup: &MatchSetup, z: Fx, _c: &Combat) -> (Fx, Fx) {
    let r = &setup.rules.melee;
    (
        z - Fx::from_ratio(r.down_cm.min(1000) as i64, 100),
        z + Fx::from_ratio(r.up_cm.min(1000) as i64, 100),
    )
}

/// M2-M3 for soldier `i` in `aim_and_fire` (per chunk: only his own
/// `Combat::close` is written). A running phase counts down: at the end
/// of the wind-up his id goes to `strikes` (resolved in the Damage stage)
/// and the recovery starts; at the end of the recovery the strike is
/// over. With `[melee] enabled` 1 a man who may strike (M2: not pinned,
/// ducked, prone, serving a gun, throwing or carrying a crate, his weapon
/// `wi` unable to fire within the wind-up) and has an enemy in reach
/// starts one at the nearest. Returns true when he neither aims nor fires
/// this tick (a phase runs or starts).
#[allow(clippy::too_many_arguments)]
pub(crate) fn strike_tick(
    ctx: &FireCtx,
    i: usize,
    c: &mut Combat,
    ws: &Weapon,
    kit: &Kit,
    ty: &frontline_content::setup::SquadTypeSetup,
    wi: u16,
    fed: bool,
    strikes: &mut Vec<SimId>,
) -> bool {
    let setup = ctx.game.setup();
    let id = SimId(i as u32);
    match c.close.strike_phase {
        STRIKE_WINDUP => {
            c.close.strike = c.close.strike.saturating_sub(1);
            if c.close.strike == 0 {
                strikes.push(id);
                c.close.strike_phase = STRIKE_RECOVER;
                c.close.strike = melee_of(setup, wi).recover;
            }
            return true;
        }
        STRIKE_RECOVER => {
            c.close.strike = c.close.strike.saturating_sub(1);
            if c.close.strike == 0 {
                c.close = Close {
                    burn_until: c.close.burn_until,
                    burn_by: c.close.burn_by,
                    ..Close::default()
                };
            }
            return true;
        }
        _ => {}
    }
    if setup.rules.melee.enabled == 0
        || c.pinned
        || c.ducked
        || c.stance == Stance::Prone
        || ctx.unit[i].serving.is_some()
        || kit.windup.is_some()
        || ctx.game.supply6().carries(id)
    {
        return false;
    }
    let w = &setup.weapons[wi as usize];
    let m = melee_of(setup, wi);
    // M2: his weapon cannot fire within the wind-up: a launcher, a mortar
    // or a flamethrower that is dry (or off); a counted weapon that is dry
    // (reloading or out: every reload outlasts a wind-up); any other whose
    // cooldown (reload, bolt, burst pause) is longer than the wind-up. The
    // first aim at a new target (half a second, his training's share) is
    // not "not ready": a loaded SMG at 1 m shoots.
    let aim = ctx.squad[i]
        .and_then(|s| ctx.game.squad(s))
        .map_or(0, |sq| {
            crate::combat::pct_of(
                setup.tick_hz / 2,
                crate::combat::training_of(setup, sq).aim_time_pct,
            )
        });
    let wait = (m.windup as u32).max(aim);
    // S14 Fix 3: a dry man who can reload from his pouches or draw his
    // sidearm does that (the dry fire path) instead of striking.
    if c.mag == 0
        && (fed || w.class == WeaponClass::Flame)
        && crate::supply::dry_load(setup, ty, kit).is_some()
    {
        return false;
    }
    let ready = match w.class {
        WeaponClass::Launcher | WeaponClass::Mortar => false,
        WeaponClass::Flame => {
            setup.rules.flame.enabled != 0 && c.mag > 0 && ws.cooldown as u32 <= wait
        }
        _ if fed && c.mag == 0 => false,
        _ => ws.cooldown as u32 <= wait,
    };
    if ready {
        return false;
    }
    let pos = ctx.transform[i].pos.xy();
    let within = start_reach(setup, &m);
    let z = strike_window(setup, ctx.transform[i].pos.z, c);
    let Some((_, at)) = nearest_enemy(
        |f| ctx.spatial.for_each_near(pos, within, f),
        ctx.kind,
        ctx.owner,
        ctx.transform,
        id,
        ctx.owner[i].0,
        pos,
        within,
        z,
    ) else {
        return false;
    };
    c.close.strike_phase = STRIKE_WINDUP;
    c.close.strike = m.windup;
    c.close.strike_at = at;
    true
}

impl MatchState {
    /// M2 `Strike { units }` (direct control): each of the player's men
    /// with no strike running and an enemy in reach starts one at once at
    /// the nearest (his weapon's readiness is not asked). Refused with
    /// `[melee] enabled` 0 or when no man starts one.
    pub(crate) fn order_strike(&mut self, ctx: &mut Ctx, p: PlayerId, units: &[SimId]) -> bool {
        use crate::fort::FortRefusal;
        if self.setup.rules.melee.enabled == 0 {
            return self.refuse_fort(p, None, FortRefusal::NotAllowed);
        }
        let Some(men) = self.unit_set(ctx, p, units) else {
            return self.refuse_fort(p, None, FortRefusal::NotAllowed);
        };
        let mut any = false;
        for m in men {
            let i = m.index();
            let Some(ty) = ctx.ents.squad[i]
                .and_then(|s| self.squads.get(&s))
                .map(|sq| &self.setup.squad_types[sq.type_idx as usize])
            else {
                continue;
            };
            let c = &ctx.ents.combat[i];
            if c.close.strike_phase != STRIKE_NONE || c.hp == 0 {
                continue;
            }
            let wi = crate::supply::active_weapon(ty, &ctx.ents.kit[i]);
            let mel = melee_of(&self.setup, wi);
            let within = start_reach(&self.setup, &mel);
            let ents = &*ctx.ents;
            let pos = ents.transform[i].pos.xy();
            let z = strike_window(&self.setup, ents.transform[i].pos.z, c);
            let near = |f: &mut dyn FnMut(SimId)| {
                for s in ents.soldiers() {
                    if ents.unit[s.index()].riding.is_none() {
                        f(s);
                    }
                }
            };
            let Some((_, at)) = nearest_enemy(
                near,
                &ents.kind,
                &ents.owner,
                &ents.transform,
                m,
                p.0,
                pos,
                within,
                z,
            ) else {
                continue;
            };
            let c = &mut ctx.ents.combat[i].close;
            c.strike_phase = STRIKE_WINDUP;
            c.strike = mel.windup;
            c.strike_at = at;
            ctx.hash.mark_entity_chunk(m.chunk());
            any = true;
        }
        if !any {
            return self.refuse_fort(p, None, FortRefusal::NotAllowed);
        }
        true
    }

    /// M1, M4 (Damage stage, after this tick's blows, ascending striker
    /// id): each striker still standing (one downed or killed earlier in
    /// this stage does not strike) strikes, S14 Fix 3, at the body of the
    /// enemy soldier nearest `strike_at` as he stands at resolution (ties
    /// by id) whose feet lie in the `[melee] down_cm` / `up_cm` window
    /// and whose capsule axis is within `reach` plus the capsule radius
    /// horizontally (any stance; downed men, vehicles and pieces neither
    /// struck nor in the way). He takes the strike's damage through `hurt`
    /// in the W1 zone at the axis point nearest the striker's chest
    /// (`fire_height` less 40 cm), class MELEE, and `[combat]
    /// hit_suppression`. No near miss; no such enemy: a miss.
    pub(crate) fn resolve_strikes(
        &mut self,
        ctx: &mut DamageCtx,
        strikes: &[SimId],
        dead: &mut Vec<Fallen>,
    ) {
        let body = self.setup.rules.combat.body.clone();
        let hit_supp = self.setup.rules.combat.hit_suppression;
        for &s in strikes {
            let i = s.index();
            let ents = &*ctx.ents;
            if ents.kind.get(i) != Some(&EntityKind::Soldier) || ents.combat[i].hp == 0 {
                continue;
            }
            let Some(ty) = ents.squad[i]
                .and_then(|q| self.squads.get(&q))
                .map(|sq| &self.setup.squad_types[sq.type_idx as usize])
            else {
                continue;
            };
            let wi = crate::supply::active_weapon(ty, &ents.kit[i]);
            let m = melee_of(&self.setup, wi);
            let owner = ents.owner[i].0;
            let c = &ents.combat[i];
            let pos = ents.transform[i].pos;
            let chest = (c.fire_height - Fx::from_milli(400)).max(Fx::from_milli(100));
            let from = FxVec3::new(pos.x, pos.y, pos.z + chest);
            let s14 = &mut self.players[owner as usize].s14;
            s14.strikes = s14.strikes.saturating_add(1);
            // S14 Fix 3: the strike goes at the body of the enemy nearest
            // `strike_at` as he stands now (any stance or height inside
            // the window), whose capsule axis is within reach plus its
            // radius horizontally.
            let win = strike_window(&self.setup, pos.z, c);
            let reach = m.reach + Fx::from_milli(body.radius_mm as i32);
            let aim = c.close.strike_at;
            let mut hit: Option<(u64, SimId, FxVec3)> = None;
            ctx.spatial
                .for_each_near(pos.xy(), reach + Fx::from_int(2), |id| {
                    let j = id.index();
                    if j == i
                        || ents.owner[j].0 == owner
                        || ents.kind[j] != EntityKind::Soldier
                        || ents.combat[j].hp == 0
                    {
                        return;
                    }
                    let t = &ents.transform[j];
                    if t.pos.z < win.0 || t.pos.z > win.1 {
                        return;
                    }
                    let (a, b, _) = crate::combat::capsule(t, &ents.combat[j], &body);
                    // The axis point nearest his chest, then horizontally.
                    let q = axis_point(a, b, from);
                    let flat = FxVec2::new(q.x - from.x, q.y - from.y);
                    if flat.length() > reach {
                        return;
                    }
                    let d = (t.pos.xy() - aim).length_sq_raw();
                    if hit.is_none_or(|h| (d, id) < (h.0, h.1)) {
                        hit = Some((d, id, q));
                    }
                });
            let Some((_, id, q)) = hit else { continue };
            let j = id.index();
            let z = q.z;
            let zone = ctx.wounds.as_ref().map_or(crate::wounds::ZONE_TORSO, |w| {
                crate::wounds::zone_at(&ctx.ents.transform[j], &ctx.ents.combat[j], z, w)
            });
            crate::combat::add_suppression(&mut ctx.ents.combat[j], hit_supp * 1000);
            let fell = crate::combat::hurt(
                ctx.ents,
                ctx.hash,
                dead,
                id,
                m.damage,
                (owner, kill_class::MELEE),
                (zone, false),
                ctx.wounds.as_ref(),
            );
            let s14 = &mut self.players[owner as usize].s14;
            s14.strike_hits = s14.strike_hits.saturating_add(1);
            if fell {
                s14.strike_kills = s14.strike_kills.saturating_add(1);
            }
        }
    }
}
