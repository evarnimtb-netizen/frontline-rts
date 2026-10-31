//! Player commands: the only input to the sim (§5.1).
//!
//! [`crate::World::step`] sorts the commands for a tick by `(player, seq)`
//! before applying them (`Ord` is derived with those fields first, and the
//! payload breaks any remaining ties, so the order is total). Invalid commands
//! (dead unit, wrong owner) are ignored, identically on every client.

use serde::{Deserialize, Serialize};

use crate::ecs::{PlayerId, SimId};
use crate::fx::FxVec2;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub struct Command {
    pub player: PlayerId,
    /// Per-player sequence number.
    pub seq: u32,
    pub kind: CommandKind,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum CommandKind {
    /// Walk to `target` (clamped to the map), then resume wandering.
    Move { unit: SimId, target: FxVec2 },
    /// Spawn a soldier owned by the issuing player at `pos`.
    Spawn { pos: FxVec2 },
    /// Remove one of the issuing player's units.
    Despawn { unit: SimId },
}
