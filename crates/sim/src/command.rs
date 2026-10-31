//! Player commands: the only input to the sim (§5.1).
//!
//! [`crate::World::step`] sorts the commands for a tick by `(player, seq)`
//! before applying them (`Ord` is derived with those fields first, and the
//! payload breaks any remaining ties, so the order is total). Invalid commands
//! (dead unit, wrong owner) are ignored, identically on every client.

use serde::{Deserialize, Serialize};

use crate::ecs::{PlayerId, SimId};
use crate::fx::FxVec2;
use crate::game::SquadId;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub struct Command {
    pub player: PlayerId,
    /// Per-player sequence number.
    pub seq: u32,
    pub kind: CommandKind,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum CommandKind {
    /// Bench: walk to `target` (clamped to the map), then resume wandering.
    MoveSoldier { unit: SimId, target: FxVec2 },
    /// Bench: spawn a soldier owned by the issuing player at `pos`.
    Spawn { pos: FxVec2 },
    /// Bench: remove one of the issuing player's units.
    Despawn { unit: SimId },
    /// Move the squad to `target` (clamped to the map); it spreads in a line
    /// across the move direction on arrival.
    Move { squad: SquadId, target: FxVec2 },
    /// Like `Move`, engaging enemies on the way (engaging: combat stage).
    AttackMove { squad: SquadId, target: FxVec2 },
    /// Stop where the men stand.
    Stop { squad: SquadId },
    /// Back to the own base, faster.
    Retreat { squad: SquadId },
    /// Dig a trench along `line` (2..=16 points, at most the rules'
    /// `dig.max_length`; squads that can dig only). Stage 1 walks the squad
    /// there and spreads it along the line; digging comes later.
    Dig { squad: SquadId, line: Vec<FxVec2> },
    /// Mortar squads: fire `rounds` (1..=50) at `target`. Stage 1 stores the
    /// order and turns the squad to face the target; firing comes later.
    Barrage {
        squad: SquadId,
        target: FxVec2,
        rounds: u16,
    },
    /// Buy a squad (index into `MatchSetup::squad_types`, from the player's
    /// faction roster). It arrives at the base after its call-in delay.
    CallIn { squad_type: u16 },
    /// Give up; the last player standing wins.
    Surrender,
}
