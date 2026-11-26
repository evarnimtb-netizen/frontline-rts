//! S15 USSR (docs/slices/S15-ussr.md 3, 5): the saved layout of riders on
//! tanks, plates, captured bombs and bayonets, and the S15 counters, with
//! their load checks (Build A's layout commit). Every rule reads its switch
//! (`[riders] enabled`, `[body_armour] enabled`, `[supply] foreign_ammo`,
//! `[melee] bayonets`, `[ai.riders] jobs`); at 0 no S15 code path runs and
//! no RNG draw changes.

use serde::{Deserialize, Serialize};

use crate::ecs::SimId;
use crate::game::SquadId;

/// `S15Counters::rider_exits` and `RiderOff::why`, by index (plan 3).
pub mod rider_exit {
    /// A `Disembark` (player or AI).
    pub const ORDERED: u8 = 0;
    /// R3a: a rider past the suppression threshold.
    pub const SUPPRESSED: u8 = 1;
    /// R3b: a shell, rocket or mine struck the vehicle.
    pub const HIT: u8 = 2;
    /// R3c: its main gun fired.
    pub const GUN_FIRED: u8 = 3;
    /// R3d: knock-out, abandon or fire (the S8a rule).
    pub const KNOCKED_OUT: u8 = 4;
    pub const COUNT: usize = 5;
    /// Report names, by index.
    pub const NAMES: [&str; COUNT] = [
        "ordered",
        "suppressed",
        "vehicle_hit",
        "gun_fired",
        "knocked_out",
    ];
}

/// S15-ai: `AiCounters::rider_job_ends` names, by index (plan 3, 7 A1).
pub const RIDER_END_NAMES: [&str; 5] = ["dropped", "forced_off", "tank_lost", "deadline", "stall"];

/// `RiderJob::phase`: the squad walks to the tank and boards (the tank
/// waits).
pub const RIDER_BOARD: u8 = 0;
/// `RiderJob::phase`: riding to the point.
pub const RIDER_RIDE: u8 = 1;
/// `RiderJob::phase`: off (`Disembark` short of the point, or R3), joining
/// the point's attack.
pub const RIDER_DROP: u8 = 2;
/// Highest `RiderJob::phase`.
pub const RIDER_PHASE_MAX: u8 = RIDER_DROP;
/// Hard ceiling of rider jobs per AI (`[ai.riders] squads_max` is at most
/// this).
pub const MAX_RIDER_JOBS: usize = 8;
/// Hard ceiling of remembered failed pairings per AI (L3).
pub const MAX_RIDER_REFUSED: usize = 32;

/// S15 per-player counters (saved and hashed, read only by reports).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct S15Counters {
    /// P1-P4: hits on a plated man inside the plate's zone and arc, and of
    /// them those the plate stopped (all, fragments, strikes). Counted on
    /// the plated man's player.
    pub plate_hits: u32,
    pub plate_stops: u32,
    pub plate_stops_frag: u32,
    pub plate_stops_melee: u32,
    /// R2: men who boarded a vehicle's rider slots.
    pub rider_boards: u32,
    /// R3: riders off, by `rider_exit` reason.
    pub rider_exits: [u32; rider_exit::COUNT],
    /// R4: riders hit while on the deck (killed or downed there).
    pub rider_casualties: u32,
    /// X1-X2: foreign rounds drawn from a held enemy crate, and fired.
    pub foreign_drawn: u32,
    pub foreign_fired: u32,
    /// B1: strikes made with a bayonet block, and hits.
    pub bayonet_strikes: u32,
    pub bayonet_hits: u32,
}

/// R3: riders getting off a vehicle by themselves
/// (`VehicleState::rider_off`): all slots at once over the sides when the
/// timer runs out, then stunned.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct RiderOff {
    /// Ticks left (at least 1 while set).
    pub ticks: u16,
    /// `rider_exit` reason (SUPPRESSED, HIT or GUN_FIRED).
    pub why: u8,
}

/// S15-ai A1: a squad riding a tank to a point (`AiRiders::jobs`,
/// ascending tank id).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct RiderJob {
    /// The vehicle (an entity with rider slots).
    pub tank: SimId,
    pub squad: SquadId,
    /// The attack point (index into the match's points).
    pub point: u8,
    /// `RIDER_BOARD`, `RIDER_RIDE` or `RIDER_DROP`.
    pub phase: u8,
    /// The tick the phase began.
    pub phase_at: u64,
}

/// S15-ai L3: a failed pairing, not retried for `[ai.riders] recheck_s`.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct RiderRefused {
    pub tank: SimId,
    pub squad: SquadId,
    /// The tick it failed.
    pub at: u64,
}

/// S15-ai rider state of one AI (`AiState::riders`).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct AiRiders {
    /// A1 jobs, strictly ascending by tank (at most [`MAX_RIDER_JOBS`]).
    pub jobs: Vec<RiderJob>,
    /// L3 failed pairings in failing order (at most
    /// [`MAX_RIDER_REFUSED`]).
    pub refused: Vec<RiderRefused>,
}

use crate::ecs::{Entities, EntityKind, PlayerId};
use crate::game::MatchState;

impl MatchState {
    /// S15 load checks (plan 3 N3): counters in range; a rider timer only
    /// where riders are (checked with the vehicle state); foreign rounds
    /// only on a live man whose weapon has a foreign feed, within its cap.
    pub(crate) fn validate_s15(&self, ents: &Entities) -> Result<(), &'static str> {
        for pl in &self.players {
            if !crate::bounds::counters_ok(&pl.s15) {
                return Err("a player counter is out of range");
            }
        }
        for (i, k) in ents.kit.iter().enumerate() {
            if k.foreign == 0 {
                continue;
            }
            if ents.kind[i] != EntityKind::Soldier || k.foreign > self.foreign_cap(k.weapon) {
                return Err("foreign rounds without a foreign feed");
            }
        }
        Ok(())
    }

    /// X1: the most foreign rounds a man with weapon `w` may carry (0:
    /// none; no weapon has a foreign feed before Build A's X rules).
    pub(crate) fn foreign_cap(&self, _w: u16) -> u16 {
        0
    }
}

impl AiRiders {
    /// Load checks of AI `player`'s rider state (untrusted saves): bounds,
    /// ascending tanks, own vehicles with rider slots, own squads, phases
    /// in range, ticks not in the future.
    pub(crate) fn valid(
        &self,
        g: &MatchState,
        player: PlayerId,
        tick: u64,
        bad_squad: impl Fn(&SquadId) -> bool,
    ) -> bool {
        let slots = |id: SimId| {
            g.vehicles
                .get(&id)
                .is_some_and(|vs| !vs.riders.is_empty() && g.rider_slots_of(id))
        };
        let own = |sq: &SquadId| g.squads.get(sq).is_none_or(|q| q.owner == player);
        self.jobs.len() <= MAX_RIDER_JOBS
            && self.jobs.windows(2).all(|w| w[0].tank < w[1].tank)
            && self.jobs.iter().all(|j| {
                slots(j.tank)
                    && !bad_squad(&j.squad)
                    && own(&j.squad)
                    && (j.point as usize) < g.points.len()
                    && j.phase <= RIDER_PHASE_MAX
                    && j.phase_at <= tick
            })
            && self.refused.len() <= MAX_RIDER_REFUSED
            && self.refused.windows(2).all(|w| w[0].at <= w[1].at)
            && self
                .refused
                .iter()
                .all(|r| !bad_squad(&r.squad) && r.at <= tick && r.tank.0 < u32::MAX)
    }
}

impl MatchState {
    /// R1: whether vehicle `id`'s passenger slots are rider slots (outside
    /// the armour; tanks, assault guns, tank destroyers).
    pub fn rider_slots_of(&self, id: SimId) -> bool {
        self.transport_of(id).is_some_and(|t| t.exposed)
    }
}

/// R3b, R3c: cause `why` (`rider_exit::HIT` or `GUN_FIRED`) on vehicle
/// state `vs` starts its riders' exit timer when `[riders] enabled` and
/// the cause's switch are 1, the vehicle has rider slots and men on them,
/// and no timer runs yet. Reads the setup only (callers hold `vs` mutably).
pub(crate) fn rider_cause(
    s: &frontline_content::MatchSetup,
    vs: &mut crate::vehicle::VehicleState,
    why: u8,
) {
    let r = &s.rules.riders;
    let on = match why {
        rider_exit::SUPPRESSED => r.off_on_suppression,
        rider_exit::HIT => r.off_on_hit,
        rider_exit::GUN_FIRED => r.off_on_fire,
        _ => 0,
    };
    if r.enabled == 0 || on == 0 || vs.rider_off.is_some() || vs.wreck_tick.is_some() {
        return;
    }
    let exposed = s
        .vehicles
        .get(vs.type_idx as usize)
        .and_then(|v| v.transport.as_ref())
        .is_some_and(|t| t.exposed);
    if !exposed || vs.riders.iter().all(|m| m.is_none()) {
        return;
    }
    let ticks = (r.exit_ms as u64 * s.tick_hz as u64).div_ceil(1000);
    vs.rider_off = Some(RiderOff {
        ticks: ticks.clamp(1, u16::MAX as u64) as u16,
        why,
    });
}

/// R3a: whether a rider of `vs` is past `[riders] suppression_pct` of
/// `crouch_suppression` (`suppression` in thousandths of a point).
pub(crate) fn riders_suppressed(
    s: &frontline_content::MatchSetup,
    ents: &Entities,
    vs: &crate::vehicle::VehicleState,
) -> bool {
    let at = s.rules.riders.suppression_pct as u64 * s.rules.combat.crouch_suppression as u64 * 10;
    vs.riders
        .iter()
        .flatten()
        .any(|m| ents.combat[m.index()].suppression as u64 >= at)
}
