//! S18 H1: hand-over of a surrendered player's troops to his ally
//! (`docs/slices/S18-online-teams.md` 3, owner answer S18-1 (A)).
//!
//! When a player surrenders (his own `Surrender`, or the relay's for a
//! player who quit or timed out) and `[teams] handover` is on, everything
//! he owns that is still in play changes owner to his lowest-slot standing
//! ally, in one tick, in id order: living soldiers, vehicles, guns and
//! abandoned vehicles, downed men, squads, HQs, dumps and aid posts,
//! crates, works, pieces, wire, charges, mines and unit dig jobs. His
//! manpower is added to the ally's; his cards and cooldowns are dropped
//! (he is out). With no standing ally nothing moves (the team is out,
//! S17 T7). Corpses and wrecks keep their last owner (nobody plays them),
//! and so do items, rounds and bursts already in flight, cook-offs and
//! mine blasts (events that end on their own), the captured points
//! (S17 T4: they stay a player's and count for his team) and the per-player
//! reports and knowledge.
//!
//! The ally records the squads he got (`PlayerState::handed`), so with
//! `[teams] handover_pop` 0 (S18-6) they never count against his pop cap;
//! the giver records whom he gave to (`PlayerState::handed_to`), which is
//! what lets a load accept the ally owning a structure of the giver's
//! faction. Owner tests are the only thing that changes: a 1v1 never hands
//! over (no ally), so its state and hash are unchanged.

use crate::ai::AiState;
use crate::ecs::{EntityKind, PlayerId};
use crate::game::{Ctx, MatchState};

/// The entity kinds that change owner (living things in play).
fn handed_kind(k: EntityKind) -> bool {
    matches!(
        k,
        EntityKind::Soldier | EntityKind::Vehicle | EntityKind::Abandoned | EntityKind::Downed
    )
}

/// Whether `owner` may hold a def of `faction`: his own faction, or the
/// faction of a player whose hand-over chain (`handed_to`) leads to him.
pub(crate) fn plays_faction(
    factions: &[u16],
    handed_to: &[Option<PlayerId>],
    owner: PlayerId,
    faction: u16,
) -> bool {
    if factions.get(owner.0 as usize) == Some(&faction) {
        return true;
    }
    (0..factions.len()).any(|q| {
        if factions[q] != faction {
            return false;
        }
        // Follow q's chain; at most one step per player (a chain never
        // loops in a valid state, and a hostile save gives up).
        let mut at = q;
        for _ in 0..factions.len() {
            match handed_to.get(at).copied().flatten() {
                Some(t) if t == owner => return true,
                Some(t) => at = t.0 as usize,
                None => return false,
            }
        }
        false
    })
}

impl MatchState {
    /// S18 H1: whom `p`'s troops go to: his lowest-slot standing ally.
    pub fn handover_target(&self, p: PlayerId) -> Option<PlayerId> {
        (0..self.players.len() as u8)
            .filter(|&q| q != p.0 && self.teams.allied(p.0, q))
            .find(|&q| self.players[q as usize].defeat.is_none())
            .map(PlayerId)
    }

    /// Hands everything `from` owns in play to `to` (module docs). Called
    /// once, as `from` surrenders.
    pub(crate) fn hand_over(&mut self, ctx: &mut Ctx, from: PlayerId, to: PlayerId) {
        let ents = &mut *ctx.ents;
        for i in 0..ents.len() {
            if ents.owner[i] == from && handed_kind(ents.kind[i]) {
                ents.owner[i] = to;
                ctx.hash
                    .mark_entity_chunk(crate::ecs::SimId(i as u32).chunk());
            }
        }
        let mut got = Vec::new();
        for (&id, sq) in self.squads.iter_mut() {
            if sq.owner == from {
                sq.owner = to;
                got.push(id);
            }
        }
        for s in self.supply6.structures.values_mut() {
            if s.owner == from {
                s.owner = to;
            }
        }
        for c in self.supply6.crates.values_mut() {
            if c.owner == from {
                c.owner = to;
            }
        }
        let f = &mut self.fort;
        for w in f.works.values_mut().filter(|w| w.owner == from) {
            w.owner = to;
        }
        for p in f.pieces.values_mut().filter(|p| p.owner == from) {
            p.owner = to;
        }
        for w in f.wire.values_mut().filter(|w| w.owner == from) {
            w.owner = to;
        }
        for c in f.charges.iter_mut().filter(|c| c.owner == from) {
            c.owner = to;
        }
        for m in f.mines.values_mut().filter(|m| m.owner == from) {
            m.owner = to;
            // The new owner knows his mines (a mine's owner always does).
            if to.0 < 8 {
                m.known |= 1 << to.0;
            }
        }
        for d in self.unit_digs.values_mut().filter(|d| d.owner == from) {
            d.owner = to;
        }
        // The mine layers count every known bit (S17 F2); the next refresh
        // sees the changed masks.
        let mp = std::mem::take(&mut self.players[from.0 as usize].manpower);
        let pt = &mut self.players[to.0 as usize];
        pt.manpower = pt.manpower.saturating_add(mp);
        pt.handed.extend(got);
        pt.handed.sort_unstable();
        pt.handed.dedup();
        self.players[from.0 as usize].handed_to = Some(to);
        // An AI that surrendered (only a test does that) keeps no plans for
        // squads it no longer owns.
        if let Some(ai) = self.ai.get_mut(from.0 as usize).and_then(|a| a.as_mut()) {
            *ai = AiState::new(ctx.tick);
        }
    }

    /// S18-6: whether squad `sq` was handed to `player` and, with
    /// `[teams] handover_pop` 0, so does not count against his pop cap.
    pub(crate) fn pop_free(&self, player: PlayerId, sq: Option<crate::game::SquadId>) -> bool {
        sq.is_some_and(|sq| {
            self.players
                .get(player.0 as usize)
                .is_some_and(|p| p.handed.binary_search(&sq).is_ok())
        })
    }
}
