//! S11a2 counter-battery by sound (docs/slices/S11a2.md 3, rules K1-K4;
//! design artillery-on-map.md 4.3). Every fire-mission round of an infantry
//! gun is heard by the other players; each keeps a short list of spots
//! (where the rounds came from). A spot with enough evidence becomes a
//! marker after a delay, with an error that shrinks with every further
//! round and a piece of evidence lost every `cb_decay_s`. A marker is known
//! ground for map fire (K4) and reveals nothing else.
//!
//! Switch `[fire_mission] counter_battery` (0: no spot is ever made).

use serde::{Deserialize, Serialize};

use crate::ecs::PlayerId;
use crate::fx::{Fx, FxVec2};
use crate::game::MatchState;

/// Most spots one player keeps (`PlayerState::cb`).
pub const MAX_CB_SPOTS: usize = 8;

/// One enemy firing spot a player has heard (saved, hashed).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct CbSpot {
    /// The player whose gun fired (never the holder).
    pub of: u8,
    /// Where the first counted round was fired from.
    pub pos: FxVec2,
    /// The marker (meaningful once `marked`).
    pub at: FxVec2,
    /// The marker's error radius (mm; 0 until marked).
    pub err_mm: u32,
    /// The listener distance the first error was made from (mm).
    pub heard_mm: u32,
    /// Rounds counted, less the decay; the spot goes at 0.
    pub evidence: u16,
    /// Rounds counted, never decayed (the AI's "new evidence", R2).
    pub shots: u32,
    /// The next tick one piece of evidence decays (a whole second).
    pub decay_at: u64,
    /// The tick the marker is made (a whole second; 0: none pending).
    pub mark_at: u64,
    /// A marker exists.
    pub marked: bool,
}

/// Load check of `holder`'s spots at `tick` (spec 3; one rule per clause).
pub(crate) fn validate_spots(
    g: &MatchState,
    holder: PlayerId,
    spots: &[CbSpot],
    tick: u64,
) -> Result<(), &'static str> {
    if spots.is_empty() {
        return Ok(());
    }
    let s = &g.setup;
    let Some(fm) = s.rules.fire_mission.as_ref() else {
        return Err("counter-battery spots without fire-mission rules");
    };
    if spots.len() > MAX_CB_SPOTS {
        return Err("too many counter-battery spots");
    }
    let size = Fx::from_int(s.map.size_m);
    let on_map = |p: FxVec2| p.x >= Fx::ZERO && p.y >= Fx::ZERO && p.x <= size && p.y <= size;
    let size_mm = (s.map.size_m.max(0) as u64 * 1000).min(u32::MAX as u64) as u32;
    let hz = s.tick_hz.max(1) as u64;
    let decay_max = tick + (fm.cb_decay_s as u64 + 1) * hz;
    let mark_max = tick + (fm.cb_delay_s as u64 + 1) * hz;
    for c in spots {
        let ok = (c.of as usize) < s.players.len()
            && c.of != holder.0
            && on_map(c.pos)
            && on_map(c.at)
            && c.err_mm <= size_mm
            && c.heard_mm <= size_mm
            && c.evidence >= 1
            && tick < c.decay_at + hz
            && c.decay_at <= decay_max
            && (c.mark_at == 0 || (tick < c.mark_at + hz && c.mark_at <= mark_max && !c.marked))
            && (!c.marked || c.shots >= 1);
        if !ok {
            return Err("bad counter-battery spot");
        }
    }
    Ok(())
}
