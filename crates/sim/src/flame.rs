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
pub(crate) fn close_ok(kind: EntityKind, c: &Close, players: usize, vehicles: usize, burn_max: u64) -> bool {
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

    /// The S14 bounds of this match's rules.
    pub(crate) fn s14_limits(&self) -> S14Limits {
        let hz = self.setup.tick_hz as u64;
        S14Limits {
            particles: MAX_FLAME_PER_PLAYER,
            ttl_ticks: u16::MAX as u64,
            patches: MAX_PATCHES,
            patch_ticks: 3600 * hz,
            steps: u8::MAX,
            burn_ticks: 3600 * hz,
        }
    }

    /// Load checks of the S14 state (spec 3.1, `load_rejects_bad_s14_state`):
    /// particles, patches, clears, counters. `Close` is checked per entity
    /// with the other combat fields.
    pub(crate) fn validate_s14(&self, ents: &Entities, tick: u64) -> Result<(), &'static str> {
        let s = &self.setup;
        let np = s.players.len();
        let size = Fx::from_int(s.map.size_m);
        let on_map = |q: FxVec3| (Fx::ZERO..=size).contains(&q.x) && (Fx::ZERO..=size).contains(&q.y);
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
    pub(crate) fn validate_s14_terrain(&self, t: &crate::terrain::Heightfield) -> Result<(), &'static str> {
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
