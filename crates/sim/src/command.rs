//! Player commands: the only input to the sim (§5.1).
//!
//! [`crate::World::step`] sorts the commands for a tick by `(player, seq)`
//! before applying them (`Ord` is derived with those fields first, and the
//! payload breaks any remaining ties, so the order is total). Invalid commands
//! (dead unit, wrong owner) are ignored, identically on every client.

use frontline_content::ItemKind;
use serde::{Deserialize, Serialize};

use crate::ecs::{PlayerId, SimId};
use crate::fx::FxVec2;
use crate::game::SquadId;
use crate::unit::StanceOrder;

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
    // ---- Soldier-level orders (S1b, `crate::unit`). `units` holds 1 to
    // `unit::MAX_UNITS` ids; it is sorted and deduplicated when applied,
    // and dead, foreign and non-soldier ids are dropped (none left: the
    // command is ignored). A squad whose every living man is named gets the
    // matching squad order instead; the other men are detached. S3: in
    // `MoveUnits`, `AttackMoveUnits`, `StopUnits` and `RetreatUnits` an own
    // vehicle's id gives its squad of one the squad order.
    /// The men walk to `target` (clamped to the map) and spread in a small
    /// line around it.
    MoveUnits { units: Vec<SimId>, target: FxVec2 },
    /// Like `MoveUnits`, engaging enemies on the way.
    AttackMoveUnits { units: Vec<SimId>, target: FxVec2 },
    /// The men stop where they stand.
    StopUnits { units: Vec<SimId> },
    /// Only these men fall back to the own base.
    RetreatUnits { units: Vec<SimId> },
    /// These men dig a trench along `line` (as `Dig`), split among them.
    /// Men of squads that cannot dig are dropped. Naming exactly one whole
    /// squad gives that squad a `Dig` order.
    DigUnits {
        units: Vec<SimId>,
        line: Vec<FxVec2>,
    },
    /// Stance order for the men. Not an order to go anywhere: it neither
    /// detaches nor regroups anyone.
    SetStance {
        units: Vec<SimId>,
        stance: StanceOrder,
    },
    /// The squad's detached men rejoin it (its order is kept).
    Regroup { squad: SquadId },
    // ---- Thrown items (S1b abilities, `crate::items`). `units` as above;
    // there is no squad-level throw (a whole squad sends its men).
    /// The eligible man nearest `target` (ties: lowest id) throws one
    /// `item` there: he carries it, is not pinned or already throwing, and
    /// `target` is within his item's range. Nobody eligible: ignored. The
    /// thrower is detached and stays where he stands.
    Throw {
        units: Vec<SimId>,
        item: ItemKind,
        target: FxVec2,
    },
    /// Every eligible man throws one `item` at `target`, each with his own
    /// scatter.
    ThrowEach {
        units: Vec<SimId>,
        item: ItemKind,
        target: FxVec2,
    },
}
