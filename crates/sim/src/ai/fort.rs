//! S10a-ai fortification habits (`docs/slices/S10a-ai.md` 3.1-3.3;
//! `[ai.fort]`). Build A holds only the saved state (the layout commit):
//! one [`AiFortJob`] per builder squad (`AiState::forts`), the shelled
//! marks and the material run. The behaviour comes in Build B.

use serde::{Deserialize, Serialize};

use crate::fx::FxVec2;

/// Most fort jobs one AI keeps (spec 4).
pub const MAX_FORT_JOBS: usize = 16;
/// Most tries a fort job makes before it ends (an untrusted save cannot
/// hold more).
pub const MAX_FORT_TRIES: u8 = 16;

/// Where a fort job stands (spec 3.3: Load -> Go -> Order -> Watch -> Done).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum FortPhase {
    /// The builder loads materials in its base.
    #[default]
    Load = 0,
    /// It walks to the site.
    Go = 1,
    /// `Fortify` is sent.
    Order = 2,
    /// The work stands or is being built; the job watches it.
    Watch = 3,
    /// Finished (dropped at the next plan).
    Done = 4,
}

/// One builder squad's fort job (spec 4; keyed by the squad).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AiFortJob {
    /// Index into `MatchSetup::forts`.
    pub fort: u16,
    /// Capture point index, or `ai::NO_POINT` (a gun's wall, the HQ roof).
    pub point: u8,
    /// The `Fortify` line (at most `fort::MAX_FORT_POINTS` points).
    pub line: Vec<FxVec2>,
    pub phase: FortPhase,
    /// Tick the phase began.
    pub since: u64,
    /// The work id once the order was accepted.
    pub work: Option<u32>,
    pub tries: u8,
}
