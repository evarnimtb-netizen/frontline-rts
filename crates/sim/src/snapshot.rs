//! Render snapshot for the presenter (§6 "Snapshots").
//!
//! This is the only place where sim values become `f32`. Nothing here is read
//! back by the sim. Coordinates are sim axes (x east, y north, z up, metres);
//! the Godot bridge converts to its y-up frame.
//!
//! Match worlds add per-soldier owner and squad, and per-squad and
//! per-player summaries; bench worlds leave those empty except the owner.

use crate::ecs::{Entities, EntityKind, SimId};
use crate::game::{MatchState, NavPhase, OrderKind, Outcome};
use crate::projectile::ProjectilePool;

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
}

/// One player's match status.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlayerSnapshot {
    pub manpower: u32,
    pub tickets: u32,
    pub surrendered: bool,
    /// Alive soldiers plus soldiers on order.
    pub pop: u32,
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
    /// Per live projectile, in slot order: `[x, y, z]`.
    pub projectiles: Vec<[f32; 3]>,
    /// Squads in ascending id order (match mode only).
    pub squads: Vec<SquadSnapshot>,
    /// Per player slot (match mode only).
    pub players: Vec<PlayerSnapshot>,
    /// Winner's player slot once the match is over.
    pub winner: Option<u8>,
}

impl RenderSnapshot {
    /// Refills `self` in place (reuses allocations).
    pub(crate) fn fill(
        &mut self,
        tick: u64,
        ents: &Entities,
        pool: &ProjectilePool,
        game: Option<&MatchState>,
    ) {
        self.tick = tick;
        self.soldiers.clear();
        self.soldier_ids.clear();
        self.soldier_owner.clear();
        self.soldier_squad.clear();
        for (i, kind) in ents.kind().iter().enumerate() {
            if *kind == EntityKind::Soldier {
                let t = &ents.transform()[i];
                let [x, y, z] = t.pos.to_f32();
                self.soldiers.push([x, y, z, t.yaw.to_f32()]);
                self.soldier_ids.push(SimId(i as u32));
                self.soldier_owner.push(ents.owner()[i].0);
                if game.is_some() {
                    self.soldier_squad
                        .push(ents.squad()[i].map_or(NO_SQUAD, |s| s.0));
                }
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
}
