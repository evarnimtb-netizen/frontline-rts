//! Render snapshot for the presenter (§6 "Snapshots").
//!
//! This is the only place where sim values become `f32`. Nothing here is read
//! back by the sim. Coordinates are sim axes (x east, y north, z up, metres);
//! the Godot bridge converts to its y-up frame.
//!
//! Match worlds add per-soldier owner, squad and combat state (stance, hit
//! points, suppression, who sees him), corpses, the rounds fired and HE
//! bursts of the tick, and per-squad and per-player summaries; bench worlds
//! leave those empty except the owner.
//!
//! Fog of war: bit `p` of `soldier_visible` says player `p` sees that
//! soldier (own soldiers always). A presenter for local player `p` draws an
//! enemy only when his bit is set.
//!
//! Terrain: `dirty_tiles` lists the detail tiles that changed in this tick.
//! The presenter reads their heights with [`tile_heights_f32`] (or
//! `World::terrain().detail_tile(x, y)`) on the sim thread, when it copies
//! the snapshot out, and rebuilds only those chunks; a listed tile that is
//! gone was demoted back to the base surface. A presenter that skips
//! snapshots can catch up by comparing each tile's `changed_tick`.

use crate::combat::Explosion;
use crate::ecs::{Entities, EntityKind, SimId, Stance};
use crate::game::{MatchState, NavPhase, Order, OrderKind, Outcome};
use crate::projectile::ProjectilePool;
use crate::systems::FireRequest;
use crate::terrain::Heightfield;

/// `soldier_squad` value for a soldier without a squad.
pub const NO_SQUAD: u32 = u32::MAX;

/// One squad, for selection, cards and order markers.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SquadSnapshot {
    pub id: u32,
    /// Index into `MatchSetup::squad_types`.
    pub type_idx: u16,
    pub owner: u8,
    pub members: u8,
    pub order: OrderKind,
    pub phase: NavPhase,
    /// Selection anchor: the leader's position `[x, y, z]`.
    pub anchor: [f32; 3],
    /// Where the squad is headed (its position when holding).
    pub goal: [f32; 2],
    /// Dig orders: trench progress in percent (0 otherwise).
    pub dig_pct: u8,
}

/// A dead soldier, kept for 30 s (rules `combat.corpse_s`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CorpseSnapshot {
    pub id: SimId,
    /// `[x, y, z, yaw]`.
    pub pos: [f32; 4],
    pub owner: u8,
    /// Ticks until the body is removed.
    pub ticks_left: u16,
}

/// A round fired this tick (muzzle flash, tracer, sound).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShotSnapshot {
    pub shooter: SimId,
    /// Index into `MatchSetup::weapons`.
    pub weapon: u16,
    /// Muzzle position.
    pub from: [f32; 3],
    /// Initial velocity (m/s).
    pub vel: [f32; 3],
}

/// An HE burst this tick (explosion, light flash, dust).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExplosionSnapshot {
    pub pos: [f32; 3],
    pub weapon: u16,
}

/// One player's match status.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlayerSnapshot {
    pub manpower: u32,
    pub tickets: u32,
    pub surrendered: bool,
    /// Alive soldiers plus soldiers on order.
    pub pop: u32,
    pub kills: u32,
    pub losses: u32,
    /// Queued call-ins: `(squad type, ticks until arrival)`.
    pub call_ins: Vec<(u16, u32)>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RenderSnapshot {
    /// Tick this snapshot shows (ticks completed).
    pub tick: u64,
    /// Per live soldier, in ascending id order: `[x, y, z, yaw]`.
    pub soldiers: Vec<[f32; 4]>,
    /// Ids matching `soldiers`, for interpolating between snapshots.
    pub soldier_ids: Vec<SimId>,
    /// Owner (player slot) per entry of `soldiers`.
    pub soldier_owner: Vec<u8>,
    /// Squad id per entry of `soldiers`, or [`NO_SQUAD`] (match mode only).
    pub soldier_squad: Vec<u32>,
    /// Per entry of `soldiers` (match mode only): stance, hit points,
    /// suppression (0..=100), pinned, and the players that see him
    /// (bit per player slot, own player always set).
    pub soldier_stance: Vec<Stance>,
    pub soldier_hp: Vec<u16>,
    pub soldier_suppression: Vec<u8>,
    pub soldier_pinned: Vec<bool>,
    pub soldier_visible: Vec<u16>,
    /// Dead soldiers, ascending id (match mode only).
    pub corpses: Vec<CorpseSnapshot>,
    /// Rounds fired this tick, in fire order (match mode only).
    pub shots: Vec<ShotSnapshot>,
    /// HE bursts this tick, in order (match mode only).
    pub explosions: Vec<ExplosionSnapshot>,
    /// Per live projectile, in slot order: `[x, y, z]`.
    pub projectiles: Vec<[f32; 3]>,
    /// Squads in ascending id order (match mode only).
    pub squads: Vec<SquadSnapshot>,
    /// Per player slot (match mode only).
    pub players: Vec<PlayerSnapshot>,
    /// Winner's player slot once the match is over.
    pub winner: Option<u8>,
    /// Terrain detail tiles `(x, y)` changed in this tick, ascending
    /// `(y, x)`. Tile `(x, y)` covers `[x, x+1) × [y, y+1) · TILE_M` metres.
    pub dirty_tiles: Vec<(u16, u16)>,
}

/// Copies detail tile `(x, y)`'s heights into `out` as `f32` metres (64 × 64,
/// row-major, see `DetailTile::heights`). Returns `false` (and leaves `out`
/// empty) if the tile does not exist, i.e. that chunk is the base surface.
pub fn tile_heights_f32(terrain: &Heightfield, x: u16, y: u16, out: &mut Vec<f32>) -> bool {
    out.clear();
    match terrain.detail_tile(x, y) {
        Some(t) => {
            out.extend(t.heights().iter().map(|h| h.to_f32()));
            true
        }
        None => false,
    }
}

impl RenderSnapshot {
    /// Refills `self` in place (reuses allocations).
    pub(crate) fn fill(
        &mut self,
        tick: u64,
        ents: &Entities,
        pool: &ProjectilePool,
        game: Option<&MatchState>,
        dirty_tiles: &[(u16, u16)],
    ) {
        self.tick = tick;
        self.dirty_tiles.clear();
        self.dirty_tiles.extend_from_slice(dirty_tiles);
        self.soldiers.clear();
        self.soldier_ids.clear();
        self.soldier_owner.clear();
        self.soldier_squad.clear();
        self.soldier_stance.clear();
        self.soldier_hp.clear();
        self.soldier_suppression.clear();
        self.soldier_pinned.clear();
        self.soldier_visible.clear();
        self.corpses.clear();
        for (i, kind) in ents.kind().iter().enumerate() {
            let t = &ents.transform()[i];
            let [x, y, z] = t.pos.to_f32();
            let owner = ents.owner()[i].0;
            let c = &ents.combat()[i];
            match kind {
                EntityKind::Soldier => {
                    self.soldiers.push([x, y, z, t.yaw.to_f32()]);
                    self.soldier_ids.push(SimId(i as u32));
                    self.soldier_owner.push(owner);
                    if game.is_some() {
                        self.soldier_squad
                            .push(ents.squad()[i].map_or(NO_SQUAD, |s| s.0));
                        self.soldier_stance.push(c.stance);
                        self.soldier_hp.push(c.hp);
                        self.soldier_suppression
                            .push((c.suppression / 1000).min(100) as u8);
                        self.soldier_pinned.push(c.pinned);
                        self.soldier_visible.push(c.seen_by | (1 << owner));
                    }
                }
                EntityKind::Corpse => self.corpses.push(CorpseSnapshot {
                    id: SimId(i as u32),
                    pos: [x, y, z, t.yaw.to_f32()],
                    owner,
                    ticks_left: c.corpse,
                }),
                EntityKind::Vacant => {}
            }
        }
        self.projectiles.clear();
        self.projectiles
            .extend(pool.slots().iter().flatten().map(|p| p.pos.to_f32()));

        self.squads.clear();
        self.players.clear();
        self.winner = None;
        let Some(g) = game else {
            return;
        };
        for (id, sq) in g.squads() {
            let Some(&lead) = sq.members.first() else {
                continue;
            };
            self.squads.push(SquadSnapshot {
                id: id.0,
                type_idx: sq.type_idx,
                owner: sq.owner.0,
                members: sq.members.len() as u8,
                order: sq.order.kind(),
                phase: sq.nav.phase,
                anchor: ents.transform()[lead.index()].pos.to_f32(),
                goal: sq.nav.goal.to_f32(),
                dig_pct: match &sq.order {
                    Order::Dig { job, .. } => {
                        job.progress_pct(&g.setup().rules.dig, g.setup().tick_hz)
                    }
                    _ => 0,
                },
            });
        }
        for (p, pl) in g.players().iter().enumerate() {
            let types = &g.setup().squad_types;
            let alive = ents
                .soldiers()
                .filter(|id| ents.owner()[id.index()].0 as usize == p)
                .count() as u32;
            let queued: u32 = pl
                .call_ins
                .iter()
                .map(|c| types[c.squad_type as usize].men as u32)
                .sum();
            self.players.push(PlayerSnapshot {
                manpower: pl.manpower,
                tickets: pl.tickets,
                surrendered: pl.surrendered,
                pop: alive + queued,
                kills: pl.kills,
                losses: pl.losses,
                call_ins: pl
                    .call_ins
                    .iter()
                    .map(|c| (c.squad_type, c.ready_tick.saturating_sub(tick) as u32))
                    .collect(),
            });
        }
        if let Outcome::Won { winner } = g.outcome() {
            self.winner = Some(winner.0);
        }
    }

    /// Refills this tick's shots and bursts.
    pub(crate) fn fill_events(&mut self, shots: &[FireRequest], explosions: &[Explosion]) {
        self.shots.clear();
        self.shots.extend(shots.iter().map(|r| ShotSnapshot {
            shooter: r.shooter,
            weapon: r.weapon,
            from: r.pos.to_f32(),
            vel: r.vel.to_f32(),
        }));
        self.explosions.clear();
        self.explosions
            .extend(explosions.iter().map(|e| ExplosionSnapshot {
                pos: e.pos.to_f32(),
                weapon: e.weapon,
            }));
    }
}
