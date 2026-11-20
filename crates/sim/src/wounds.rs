//! S12 wounds (docs/slices/S12-wounds-medics.md 3, 4): the saved layout of
//! wound states, casualties and aid-post beds, their load checks, and the
//! wound rules W1-W5, W12-W14 (Build A). Every rule reads `[wounds]
//! enabled`; at 0 a soldier at 0 HP dies exactly as before S12 and no S12
//! RNG draw happens.

use serde::{Deserialize, Serialize};

use crate::ecs::{Entities, EntityKind, SimId};
use crate::fx::FxVec2;
use crate::game::{MatchState, SquadId, kill_class};

/// `Wound::state`: not hurt past `wounded_hp`.
pub const FIT: u8 = 0;
/// `Wound::state`: hurt, in his squad, bleeding until bandaged (W4).
pub const WOUNDED: u8 = 1;
/// `Wound::state`: down (`EntityKind::Downed`), out of his squad (W5).
pub const DOWNED: u8 = 2;

/// W1 hit zones (`Hit::zone`).
pub const ZONE_TORSO: u8 = 0;
pub const ZONE_HEAD: u8 = 1;
pub const ZONE_LEGS: u8 = 2;
/// Blast overpressure and other hits with no zone.
pub const ZONE_NONE: u8 = 3;

/// Most casualties a match holds at once (spec 3).
pub const MAX_CASUALTIES: usize = 512;
/// Most beds an aid post holds (the def's `beds` is at most this).
pub const MAX_BEDS: usize = 8;

/// A soldier's wound state (spec 3; in `Combat`, hashed with its chunk).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Wound {
    /// [`FIT`], [`WOUNDED`] or [`DOWNED`].
    pub state: u8,
    pub bandaged: bool,
    /// A leg wound (`legs_speed_pct`).
    pub legs: bool,
    /// Treated at an aid post once (W8, S12-7).
    pub treated: bool,
    /// Bought or reinforced: refunded when evacuated (W9).
    pub paid: bool,
    /// Bleed time carried to the next HP (ms, below `bleed_ms_per_hp`).
    pub bleed_acc_ms: u16,
    /// Ticks of bandaging left (0: not bandaging).
    pub bandaging: u16,
    /// The last hitter: player and `kill_class` (W4 credit).
    pub by: (u8, u16),
}

/// A downed man (spec 3; `MatchState::casualties`, by his id).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Casualty {
    /// The squad he fell out of (it may be gone).
    pub from_squad: SquadId,
    /// His squad's card (index into `MatchSetup::squad_types`).
    pub card: u16,
    /// He dies at this tick (0: stabilised).
    pub bleed_out_at: u64,
    /// A medic tending him and the tick he finishes (Build B).
    pub tend: Option<(SimId, u64)>,
    /// The men carrying him (Build B).
    pub carriers: [Option<SimId>; 2],
    /// Where they carry him (Build B).
    pub dest: Option<FxVec2>,
    /// Evacuated at this tick (0: none; Build B).
    pub evac_at: u64,
}

/// One bed of an aid post (Build B; `Structure::beds`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Bed {
    pub man: SimId,
    pub done_at: u64,
    pub evac: bool,
}

/// Where a man was hit and how (W1-W3; the widened `Fallen`).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub struct Hit {
    /// HP left after the hit, signed (`hp - dmg`).
    pub raw: i32,
    /// [`ZONE_TORSO`], [`ZONE_HEAD`], [`ZONE_LEGS`] or [`ZONE_NONE`].
    pub zone: u8,
    /// Blast overpressure (always a death, W3).
    pub blast: bool,
}

/// S12 per-player counters (saved, hashed; the `s12` JSON block). Pairs
/// are `[trench, open]`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct S12Counters {
    pub downed: u32,
    pub killed_outright: u32,
    pub bled_out: u32,
    /// Downed men killed by a later hit.
    pub downed_killed: u32,
    pub stabilised: u32,
    pub bandaged: u32,
    pub carried: u32,
    pub treated: u32,
    pub evacuated: u32,
    pub refund_mp: u32,
    pub reinforced_men: u32,
    pub reinforce_mp: u32,
    /// Enemy men this player downed, by `kill_class` (W13).
    pub downed_by_class: [u32; kill_class::COUNT],
    pub head_hits: [u32; 2],
    pub torso_hits: [u32; 2],
    pub leg_hits: [u32; 2],
}

/// Load checks of one entity's wound (spec 3, `load_rejects_bad_s12_state`).
pub(crate) fn wound_ok(kind: EntityKind, w: &Wound, players: usize, bandage_ticks: u64) -> bool {
    let state_ok = match kind {
        EntityKind::Soldier => w.state <= WOUNDED,
        EntityKind::Downed => w.state == DOWNED,
        _ => *w == Wound::default(),
    };
    let fit_clean = w.state != FIT || (!w.bandaged && !w.legs && w.bandaging == 0);
    state_ok
        && fit_clean
        && (w.bandaging as u64) <= bandage_ticks
        && !(w.bandaged && w.bandaging > 0)
        && (w.by == (0, 0)
            || ((w.by.0 as usize) < players && (w.by.1 as usize) < kill_class::COUNT))
}

impl MatchState {
    /// Load checks of the casualties and beds (spec 3). Every Downed entity
    /// has exactly one casualty and every casualty names a Downed entity.
    pub(crate) fn validate_s12(&self, ents: &Entities, tick: u64) -> Result<(), &'static str> {
        let s = &self.setup;
        if self.casualties.len() > MAX_CASUALTIES {
            return Err("more casualties than the cap");
        }
        let downed = ents
            .kind()
            .iter()
            .filter(|k| **k == EntityKind::Downed)
            .count();
        if downed != self.casualties.len() {
            return Err("downed men and casualties disagree");
        }
        let max_bleed = crate::bounds::MAX_TICK
            + (s.rules.wounds.down_bleed_max_s as u64 + 1) * s.tick_hz as u64;
        for (&id, c) in &self.casualties {
            if ents.kind().get(id.index()) != Some(&EntityKind::Downed) {
                return Err("a casualty is not a downed man");
            }
            if ents.squad()[id.index()].is_some() {
                return Err("a downed man is in a squad");
            }
            if c.card as usize >= s.squad_types.len() {
                return Err("a casualty's card is unknown");
            }
            if c.bleed_out_at != 0 && (c.bleed_out_at <= tick || c.bleed_out_at > max_bleed) {
                return Err("a casualty's bleed-out tick is out of range");
            }
            let owner = ents.owner()[id.index()];
            let own_man = |m: SimId| {
                ents.kind().get(m.index()) == Some(&EntityKind::Soldier)
                    && ents.owner()[m.index()] == owner
            };
            if let Some((m, at)) = c.tend
                && (!own_man(m) || at <= tick || at > crate::bounds::MAX_TICK)
            {
                return Err("a casualty's medic is bad");
            }
            if c.carriers.iter().flatten().any(|m| !own_man(*m))
                || (c.carriers[0].is_some() && c.carriers[0] == c.carriers[1])
            {
                return Err("a casualty's carriers are bad");
            }
            let size = s.map.size_m;
            if let Some(d) = c.dest {
                let on = |v: crate::fx::Fx| {
                    v >= crate::fx::Fx::ZERO && v <= crate::fx::Fx::from_int(size)
                };
                if !on(d.x) || !on(d.y) {
                    return Err("a casualty's destination is off the map");
                }
            }
            if c.evac_at != 0 && (c.evac_at <= tick || c.evac_at > crate::bounds::MAX_TICK) {
                return Err("a casualty's evacuation tick is out of range");
            }
        }
        for st in self.supply6.structures.values() {
            if st.beds.len() > MAX_BEDS {
                return Err("an aid post has too many beds");
            }
            let aid = s
                .stocks
                .get(st.def as usize)
                .is_some_and(|d| d.kind == frontline_content::setup::StockKind::Aid);
            if !st.beds.is_empty() && !aid {
                return Err("beds on a structure that is not an aid post");
            }
            for b in &st.beds {
                let k = ents.kind().get(b.man.index());
                if !matches!(k, Some(EntityKind::Soldier | EntityKind::Downed))
                    || ents.owner()[b.man.index()] != st.owner
                    || b.done_at > crate::bounds::MAX_TICK
                {
                    return Err("a bed's man is bad");
                }
            }
        }
        Ok(())
    }
}
