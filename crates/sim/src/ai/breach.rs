//! S10a-ai wire knowledge and breaching (`docs/slices/S10a-ai.md` 3.4).
//! Build A holds only the saved state (the layout commit): one
//! [`AiBreach`] per breach squad (`AiState::breaches`) and the known enemy
//! wire sections. The behaviour comes in Build C.

use serde::{Deserialize, Serialize};

use crate::fx::FxVec2;

/// Most breach jobs one AI keeps (spec 4).
pub const MAX_BREACHES: usize = 8;
/// Most enemy wire sections one AI remembers (spec 4).
pub const MAX_WIRE_KNOWN: usize = 256;
/// Most tries a breach job may record (`[ai.fort] breach_tries` is at most
/// 10).
pub const MAX_BREACH_TRIES: u8 = 10;

/// Where a breach job stands (spec 3.4).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum BreachPhase {
    /// The breach squad holds `stage_m` short of the crossing.
    #[default]
    Stage = 0,
    /// The base of fire aims at the defenders nearest the crossing.
    Suppress = 1,
    /// Smoke between the crossing and the MG.
    Smoke = 2,
    /// `PlaceBangalore` or `CutWire` is sent.
    Go = 3,
    /// Nothing laid or cut in time: suppress, smoke and send again.
    Retry = 4,
    /// The gap is open: attackers path through it.
    Through = 5,
}

/// One breach squad's job (spec 4; keyed by the squad).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AiBreach {
    /// The attacked point.
    pub point: u8,
    /// Where the path crosses the known wire.
    pub crossing: FxVec2,
    /// The crossed section's id.
    pub section: u32,
    pub phase: BreachPhase,
    /// Tick the job (or the phase) began.
    pub since: u64,
    pub tries: u8,
    /// When the base of fire was told to fire, and the smoke thrown.
    pub suppress_at: Option<u64>,
    pub smoke_at: Option<u64>,
}
