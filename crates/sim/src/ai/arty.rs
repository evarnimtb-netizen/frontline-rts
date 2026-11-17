//! S11a1 artillery AI state (docs/slices/S11a1.md 3, rules A1-A6, P1-P3):
//! the infantry-gun and observer-team records the AI keeps. Build A holds
//! the saved types, their caps and their load check only; the planner that
//! fills them is Build C's (`[ai.arty]`, every switch 0 until then).

use serde::{Deserialize, Serialize};

use crate::ecs::PlayerId;
use crate::fx::{Fx, FxVec2};
use crate::game::{MatchState, SquadId};

/// Most infantry-gun records one AI keeps (`AiState::arty`).
pub const MAX_ARTY: usize = 8;
/// Most observer-team records one AI keeps (`AiState::fo`).
pub const MAX_FO: usize = 8;

/// Where an AI infantry gun stands in its job.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum ArtyPhase {
    /// Ordered to emplace where it arrived (A2).
    #[default]
    Emplace,
    /// Emplaced: takes missions (A4).
    Ready,
}

/// One AI infantry gun, keyed by the gun's squad (saved, hashed).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct AiArty {
    pub phase: ArtyPhase,
    /// Tick the phase began.
    pub since: u64,
    /// Tick of the last order sent to the gun.
    pub last_order: u64,
    /// Missions ordered.
    pub missions: u16,
    /// The running mission is preparation fire (P1).
    pub prep: bool,
}

/// One AI observer team, keyed by its squad (saved, hashed).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct AiFo {
    /// The point the post looks at.
    pub point: u8,
    /// The chosen post, once found.
    pub post: Option<FxVec2>,
    /// The next candidate the post search tests (A3).
    pub cursor: u8,
    /// Tick the record began or the search restarted.
    pub since: u64,
    /// The team reached its post.
    pub arrived: bool,
    /// No new search before this tick (`post_retry_s`).
    pub retry_at: u64,
}

/// Candidates the A3 post search can test (5 distances x 3 sides).
pub const FO_CANDIDATES: u8 = 15;

/// Load check of the S11a1 AI records of `player` at `tick` (spec 3): keys
/// are squads that exist or existed (`< next_squad`; a squad that still
/// exists must be the player's: records of dead squads are dropped at the
/// next plan, so a save between may hold one), sizes within the caps, the
/// post on the map, the point in range, times not after `tick`.
pub(super) fn validate_arty(
    g: &MatchState,
    player: PlayerId,
    tick: u64,
    arty: &std::collections::BTreeMap<SquadId, AiArty>,
    fo: &std::collections::BTreeMap<SquadId, AiFo>,
    fo_lost_at: u64,
) -> bool {
    let size = Fx::from_int(g.setup.map.size_m);
    let on_map = |p: FxVec2| p.x >= Fx::ZERO && p.y >= Fx::ZERO && p.x <= size && p.y <= size;
    let ok_squad =
        |id: &SquadId| id.0 < g.next_squad && g.squads.get(id).is_none_or(|sq| sq.owner == player);
    arty.len() <= MAX_ARTY
        && fo.len() <= MAX_FO
        && fo_lost_at <= tick
        && arty
            .iter()
            .all(|(id, a)| ok_squad(id) && a.since <= tick && a.last_order <= tick)
        && fo.iter().all(|(id, f)| {
            ok_squad(id)
                && (f.point as usize) < g.points.len()
                && f.post.is_none_or(on_map)
                && f.cursor <= FO_CANDIDATES
                && f.since <= tick
                && f.retry_at <= tick + 3600 * g.setup.tick_hz.max(1) as u64
        })
}
