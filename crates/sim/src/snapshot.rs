//! Render snapshot for the presenter (§6 "Snapshots").
//!
//! This is the only place where sim values become `f32`. Nothing here is read
//! back by the sim. Coordinates are sim axes (x east, y north, z up, metres);
//! the Godot bridge converts to its y-up frame.
//!
//! Match worlds add per-soldier owner, squad and combat state (stance, hit
//! points, suppression, who sees him), corpses, the rounds fired and HE
//! bursts of the tick, per-squad and per-player summaries (manpower and
//! income, tickets and drain, pop, kills, losses, time holding points,
//! defeat), the capture points and the match outcome; bench worlds leave
//! those empty except the owner. S1b adds per soldier the stance order and
//! the detached flag, and for each detached man his own order and spot
//! ([`UnitSnapshot`], for order markers). Thrown items (S1b) add per
//! soldier his item counts and whether he is winding up a throw, per squad
//! the item totals (ability buttons), the items in flight or lying fused
//! ([`ThrownSnapshot`]), the smoke clouds ([`SmokeSnapshot`]) and a
//! [`BlastKind`] per explosion.
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
use crate::ecs::PlayerId;
use crate::ecs::{Entities, EntityKind, SimId, Stance};
use crate::game::{MatchState, NavPhase, Order, OrderKind, Outcome};
use crate::items::{BlastKind, cloud_now};
use crate::projectile::ProjectilePool;
use crate::rules::{Defeat, capture_full};
use crate::systems::FireRequest;
use crate::terrain::Heightfield;
use crate::unit::{StanceOrder, UnitOrder};
use frontline_content::{ITEM_KINDS, ItemKind};

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
    /// Members that are detached (S1b; counted in `members` too).
    pub detached: u8,
    pub order: OrderKind,
    pub phase: NavPhase,
    /// Selection anchor: the formation leader's position `[x, y, z]`.
    pub anchor: [f32; 3],
    /// Where the squad is headed (its position when holding).
    pub goal: [f32; 2],
    /// Dig orders: trench progress in percent (0 otherwise).
    pub dig_pct: u8,
    /// Items the men carry, by [`ItemKind::index`] (frag, smoke, satchel).
    pub items: [u16; ITEM_KINDS],
}

/// A thrown item in the air, or landed and waiting for its fuse.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ThrownSnapshot {
    pub id: u32,
    pub kind: ItemKind,
    /// Index into `MatchSetup::items`.
    pub item: u16,
    pub owner: u8,
    pub pos: [f32; 3],
    pub landed: bool,
}

/// A smoke cloud: a vertical cylinder standing at `pos`, as tall as it is
/// wide.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SmokeSnapshot {
    pub id: u32,
    pub pos: [f32; 3],
    pub radius: f32,
    /// 1 while it blocks every sight line, falling to 0 as it thins out.
    pub density: f32,
}

/// A detached soldier's own order (S1b), for order markers.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UnitSnapshot {
    pub id: SimId,
    pub owner: u8,
    pub squad: u32,
    /// `Idle`, `Move`, `AttackMove`, `Retreat` or `Dig`.
    pub order: OrderKind,
    pub phase: NavPhase,
    /// What the order aims at: the move target, the own base for a
    /// retreat, the middle of the trench line for a dig, his spot when idle.
    pub target: [f32; 2],
    /// His own spot: where he walks to and stands (his slot on the line,
    /// after cover-seeking once there).
    pub goal: [f32; 2],
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
    /// Players that see the body (`Combat::seen_by`; spotting keeps checking
    /// corpses), so fog of war hides bodies like men.
    pub seen_by: u16,
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
    /// Index into `MatchSetup::weapons` for a shell, into
    /// `MatchSetup::items` for a frag or satchel.
    pub weapon: u16,
    pub kind: BlastKind,
}

/// One player's match status.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlayerSnapshot {
    pub manpower: u32,
    /// Manpower per minute right now (base, points held, AI difficulty).
    pub income_per_min: u32,
    pub tickets: u32,
    /// Tickets lost per minute right now.
    pub drain_per_min: u32,
    pub surrendered: bool,
    /// Set once the player is out.
    pub defeat: Option<Defeat>,
    /// Alive soldiers plus soldiers on order.
    pub pop: u32,
    pub pop_cap: u32,
    pub kills: u32,
    pub losses: u32,
    /// Points held now, and point-seconds held so far (end screen).
    pub points_held: u8,
    pub hold_s: u32,
    /// Queued call-ins: `(squad type, ticks until arrival)`.
    pub call_ins: Vec<(u16, u32)>,
}

/// One capture point, in map order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PointSnapshot {
    pub pos: [f32; 2],
    pub radius: f32,
    pub value: u32,
    pub owner: Option<u8>,
    /// Who is capturing it, and how far (0..=100).
    pub capturer: Option<u8>,
    pub progress_pct: u8,
    pub contested: bool,
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
    /// Per entry of `soldiers` (match mode only, S1b): the stance order
    /// (`SetStance`) and whether he is detached from his squad's formation.
    pub soldier_stance_order: Vec<StanceOrder>,
    pub soldier_detached: Vec<bool>,
    /// Per entry of `soldiers` (match mode only, S1b): items carried by
    /// [`ItemKind::index`], and whether he is winding up a throw.
    pub soldier_items: Vec<[u8; ITEM_KINDS]>,
    pub soldier_throwing: Vec<bool>,
    /// Detached soldiers, ascending id (match mode only).
    pub units: Vec<UnitSnapshot>,
    /// Dead soldiers, ascending id (match mode only).
    pub corpses: Vec<CorpseSnapshot>,
    /// Rounds fired this tick, in fire order (match mode only).
    pub shots: Vec<ShotSnapshot>,
    /// HE bursts this tick, in order (match mode only).
    pub explosions: Vec<ExplosionSnapshot>,
    /// Thrown items, ascending id (match mode only).
    pub thrown: Vec<ThrownSnapshot>,
    /// Smoke clouds, ascending id (match mode only).
    pub smoke: Vec<SmokeSnapshot>,
    /// Per live projectile, in slot order: `[x, y, z]`.
    pub projectiles: Vec<[f32; 3]>,
    /// Squads in ascending id order (match mode only).
    pub squads: Vec<SquadSnapshot>,
    /// Per player slot (match mode only).
    pub players: Vec<PlayerSnapshot>,
    /// Capture points in map order (match mode only).
    pub points: Vec<PointSnapshot>,
    /// Running, won or drawn.
    pub outcome: Outcome,
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
        self.soldier_stance_order.clear();
        self.soldier_detached.clear();
        self.soldier_items.clear();
        self.soldier_throwing.clear();
        self.units.clear();
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
                        let u = &ents.unit()[i];
                        self.soldier_stance_order.push(u.stance);
                        self.soldier_detached.push(u.detached);
                        let k = &ents.kit()[i];
                        self.soldier_items.push(k.items);
                        self.soldier_throwing.push(k.windup.is_some());
                        if u.detached
                            && let Some(g) = game
                        {
                            let target = match &u.order {
                                UnitOrder::Idle => u.nav.goal,
                                UnitOrder::Move { target } | UnitOrder::AttackMove { target } => {
                                    *target
                                }
                                UnitOrder::Retreat => g.base_of(PlayerId(owner)),
                                UnitOrder::Dig { job } => {
                                    g.unit_digs().get(job).map_or(u.nav.goal, |d| {
                                        let line = &d.line;
                                        let len = crate::squad::polyline_length(line);
                                        crate::squad::point_along(line, len.div_int(2))
                                    })
                                }
                            };
                            self.units.push(UnitSnapshot {
                                id: SimId(i as u32),
                                owner,
                                squad: ents.squad()[i].map_or(NO_SQUAD, |s| s.0),
                                order: u.order.kind(),
                                phase: u.nav.phase,
                                target: target.to_f32(),
                                goal: u.nav.goal.to_f32(),
                            });
                        }
                    }
                }
                EntityKind::Corpse => self.corpses.push(CorpseSnapshot {
                    id: SimId(i as u32),
                    pos: [x, y, z, t.yaw.to_f32()],
                    owner,
                    ticks_left: c.corpse,
                    seen_by: c.seen_by,
                }),
                EntityKind::Vacant => {}
            }
        }
        self.projectiles.clear();
        self.projectiles
            .extend(pool.slots().iter().flatten().map(|p| p.pos.to_f32()));

        self.squads.clear();
        self.players.clear();
        self.points.clear();
        self.thrown.clear();
        self.smoke.clear();
        self.outcome = Outcome::Running;
        self.winner = None;
        let Some(g) = game else {
            return;
        };
        for (id, sq) in g.squads() {
            if sq.members.is_empty() {
                continue;
            }
            let lead = sq.lead(ents);
            let mut items = [0u16; ITEM_KINDS];
            for m in &sq.members {
                for (n, c) in items.iter_mut().zip(ents.kit()[m.index()].items) {
                    *n += c as u16;
                }
            }
            self.squads.push(SquadSnapshot {
                id: id.0,
                type_idx: sq.type_idx,
                owner: sq.owner.0,
                members: sq.members.len() as u8,
                detached: sq
                    .members
                    .iter()
                    .filter(|m| ents.unit()[m.index()].detached)
                    .count() as u8,
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
                items,
            });
        }
        let items = &g.setup().items;
        for t in g.thrown() {
            self.thrown.push(ThrownSnapshot {
                id: t.id,
                kind: items[t.item as usize].kind,
                item: t.item,
                owner: t.owner.0,
                pos: t.pos.to_f32(),
                landed: t.landed,
            });
        }
        for c in g.smoke() {
            let Some(s) = items[c.item as usize].smoke.as_ref() else {
                continue;
            };
            let (r, density) = cloud_now(s, c.age);
            self.smoke.push(SmokeSnapshot {
                id: c.id,
                pos: c.pos.to_f32(),
                radius: r.to_f32(),
                density: crate::fx::Fx::from_ratio(density as i64, 1000).to_f32(),
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
            let id = PlayerId(p as u8);
            self.players.push(PlayerSnapshot {
                manpower: pl.manpower,
                income_per_min: g.income_per_min(id),
                tickets: pl.tickets,
                drain_per_min: g.drain_per_min(id),
                surrendered: pl.surrendered,
                defeat: pl.defeat,
                pop: alive + queued,
                pop_cap: g.setup().rules.pop_cap,
                kills: pl.kills,
                losses: pl.losses,
                points_held: g.points().iter().filter(|c| c.owner == Some(id)).count() as u8,
                hold_s: (pl.point_ticks / g.setup().tick_hz as u64) as u32,
                call_ins: pl
                    .call_ins
                    .iter()
                    .map(|c| (c.squad_type, c.ready_tick.saturating_sub(tick) as u32))
                    .collect(),
            });
        }
        let r = &g.setup().rules;
        let full = capture_full(r.capture_ticks).max(1);
        for (c, ps) in g.points().iter().zip(&g.setup().map.points) {
            self.points.push(PointSnapshot {
                pos: crate::fx::FxVec2::from(ps.pos).to_f32(),
                radius: crate::fx::Fx::from(r.capture_radius).to_f32(),
                value: ps.value,
                owner: c.owner.map(|o| o.0),
                capturer: c.capturer.map(|o| o.0),
                progress_pct: (c.progress as u64 * 100 / full as u64) as u8,
                contested: c.contested,
            });
        }
        self.outcome = g.outcome();
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
                kind: e.kind,
            }));
    }
}
