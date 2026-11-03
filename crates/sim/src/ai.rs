//! AI commander (S1, §9 layer 1): a sim-side player controller.
//!
//! Each AI player re-plans every `replan_ticks` (its difficulty), serially in
//! player order in the Intent stage, reading only sim state and what its
//! player can see (`Combat::seen_by`). A plan emits [`CommandKind`]s into
//! the player's pending queue, due `reaction_ticks` later (at least one
//! tick); `World::step` merges due AI commands with the human commands of
//! that tick, sorts them by `(player, seq)` and applies them through
//! `MatchState::apply`, exactly like a human's. AI commands carry their own
//! per-player sequence numbers.
//!
//! A plan:
//!
//! 1. **Retreat.** Squads below `retreat_strength_pct` with a spotted enemy
//!    within `threat_radius` fall back to base.
//! 2. **Buy.** The role furthest below the target mix (`mix_pct`, counting
//!    squads alive and about to be bought) is bought when the manpower
//!    left after pending buys and the pop cap allow; at most
//!    `ai.infantry.buys_per_plan` per plan. S3b (no call-in queue): a
//!    role whose type is still on its re-call cooldown when the command
//!    would apply is passed over for the next role in mix order that is
//!    ready; after the vehicle unlock, under the vehicle limit, non-vehicle
//!    buys leave `ai.armour.tank_reserve_mp` unspent (not an urgent AT
//!    team, below).
//! 3. **Points.** Each point gets a priority from its value, owner, distance
//!    from base and threat (spotted enemies near it), and a demand (squads
//!    wanted). Infantry squads are assigned greedily by priority minus
//!    travel distance plus role and continuity bonuses, first up to each
//!    point's demand, then leftovers to their best point. Own points are
//!    defended (Move), others captured or attacked (AttackMove).
//! 4. **Dig.** An engineer squad sent to an own point whose trench is not
//!    finished digs it: `dig_length` long, `dig_offset` in front of the
//!    point toward the nearest spotted enemy (else the nearest enemy point,
//!    else the enemy base). Defenders of a point with a finished trench
//!    stand in it.
//! 5. **Mortars.** A mortar team fires `barrage_rounds` at the best spotted
//!    enemy group in range (bigger groups and men in trenches score
//!    higher; never within `ai.infantry.danger_close_m` of own men); otherwise it
//!    moves to a support spot `ai.infantry.support_back_m` behind the most important
//!    point.
//! 6. **Throws** (S1b, `crate::items`), at most one per squad per
//!    `throw_cooldown`, for squads not retreating, first that applies:
//!    * *satchel:* engineers throw at a spotted enemy in a trench within
//!      satchel range of a carrier;
//!    * *frag:* at a spotted enemy in a trench, or with at least
//!      `frag_group_men` spotted men within `ai.infantry.frag_group_m` of him
//!      (bigger groups, then trenches, then the nearest first), within
//!      frag range of a man;
//!    * *smoke:* a moving or attack-moving squad with a third of its men
//!      pinned throws smoke halfway toward the nearest spotted enemy (at
//!      most to the smoke's range); S3c: a squad on a satchel hunt, halfway
//!      toward its vehicle.
//!
//!    `Throw` names every man who could throw it; the sim picks the
//!    thrower. Throwers end up detached: a squad that is walking again
//!    while all its detached men are done throwing is regrouped.
//!
//! 7. **Vehicles** (S3, `docs/slices/S3-armour.md` "AI"):
//!    * *buy:* the mix gains the vehicle share (`mix_vehicle_pct`) after
//!      the unlock time and under the vehicle limit, and the AT-team share
//!      (`mix_at_pct`) after the unlock once an enemy vehicle has been seen
//!      (the saved `enemy_vehicle_seen`); when the enemy has more vehicles
//!      in sight than the AI has AT teams and vehicles, an AT team is
//!      bought first;
//!    * *upkeep:* a vehicle with its gun destroyed, two crew dead or its
//!      engine under `ai.armour.retreat_engine_pct` % retreats to base when it can
//!      move (and waits there for the base repair); an immobilised vehicle
//!      with no seen enemy within `repair_clear` repairs;
//!    * *assault:* each other vehicle takes the non-own point with the most
//!      infantry assigned to it (else it backs up the most threatened own
//!      point). It attack-moves to [`AiState::at_seen`]'s answer: if enemy
//!      AT was seen within `threat_radius` of the point in the last
//!      `at_clear` seconds, `tank_standoff` short of the point; else
//!      `assault_close` short of the enemy trench in front of it (probed
//!      along the approach) or of the point. Up to `ai.armour.escort_squads` rifle
//!      or engineer squads assigned to that point within `ai.armour.escort_join_m`
//!      escort it (`FollowUnits`); once the vehicle halts for any reason
//!      (after `ai.armour.escort_grace_s`), the escorts still attached detach and
//!      attack-move the point;
//!    * *facing:* an idle vehicle turns its hull (`FaceUnits`) toward the
//!      nearest seen enemy vehicle or AT soldier within `ai.armour.face_range_m`;
//!    * *AT teams* defend own points, preferring points with enemy vehicles
//!      within `ai.armour.threat_m`, in the trench if there is one; they fire
//!      by their weapon's own rules;
//!    * *satchels:* engineers throw at a seen enemy vehicle no faster than
//!      `ai.infantry.satchel_slow_mm_s` (stopped, bogged, crossing) within satchel range, at its position when the fuse runs out,
//!      `ai.infantry.satchel_side_mm` outside the hull side nearer the thrower.
//!
//!    [`AiCounters`] count what the AI did for the skirmish report.
//!
//! 8. **Assault, hunts** (S3b, `docs/slices/S3b-balance.md` B3–B5,
//!    `ai::assault`; distances and times in `[ai.assault]`):
//!    * *fire-and-move:* at a non-own point with at least two of the
//!      squads assigned to it, one (the squad already doing it, else an MG
//!      team, else a rifle squad) is the base of fire: within
//!      `support_mg_m` / `support_rifle_m` of the line and with a line of
//!      fire to its middle it gets `SuppressUnits` on the line (across the
//!      approach at the probed enemy trench, else at the middle of the
//!      enemies seen near the point, `trench_line_m` long); otherwise it
//!      walks to the first of up to 8 spots with a line of fire. The other
//!      squads attack as usual, except that one more than `bound_m` ahead
//!      of the rearmost halts (bounds). Once a manoeuvre squad's leader is
//!      within `lift_fire_m` of the line the base lifts (normal orders).
//!    * *tanks:* a vehicle halted at its assault or standoff goal
//!      suppresses that line within `support_tank_m` while its escort goes
//!      in, and lifts (Stop) once own men are at the line;
//!    * *AT hunts:* an AT team with rockets goes for the nearest known
//!      enemy vehicle within `at_hunt_m` of it or its point, unless a known
//!      enemy soldier is within `at_hunt_clear_m` of its route's middle: to
//!      the nearest of five spots (sides, rear quarters, rear) off the
//!      turret's front, `at_hunt_inset_m` inside its launcher's
//!      `engage_still`; in range off the turret's front it holds;
//!    * *satchel hunts:* engineers carrying a satchel go for the nearest
//!      known enemy vehicle within `satchel_hunt_m` that has been slow
//!      (`ai.infantry.satchel_slow_mm_s`) for `satchel_slow_s` (the saved
//!      `AiState::slow_since`), to a spot `satchel_stand_m` outside its
//!      nearer hull side; the satchel throw rule then fires.
//!
//!    Squads on a hunt, a base of fire and halted bounders get no other
//!    infantry order that plan; hunting squads do not join escorts.
//!
//! 9. **Armour habits** (S3c, `docs/slices/S3c-armour.md` Part B;
//!    `ai::armour`, `ai::assault`; `[ai.armour]`, `[ai.assault]`):
//!    * *rearm:* a vehicle with every limited MG dry, HE at or below
//!      `combat.suppress.tank_he_reserve` or its penetrator at or below
//!      `rearm_ap_below`, and no known enemy vehicle within `threat_m`,
//!      retreats to base (task `Rearm`) and waits there until
//!      `rearm_full_pct` full or `rearm_max_s` in base;
//!    * *sticky satchel hunts:* a hunt (saved in [`AiState::hunts`]) also
//!      starts on a stuck vehicle and keeps its vehicle while it was seen
//!      within `satchel_stick_s` (going to its last seen place), was not
//!      fast for more than `satchel_chase_s`, is within `satchel_hunt_m` +
//!      `satchel_leash_m`, for at most `satchel_hunt_max_s`, while the
//!      squad has a satchel and is not retreating. Hunters crouch, crawl
//!      within `satchel_crawl_m` in the hull MG's arc, and do not throw
//!      their satchel at infantry;
//!    * *rockets:* AT teams also hunt stuck vehicles within
//!      `at_hunt_stuck_m`, and without a hunt follow an own vehicle's
//!      assault within `at_follow_m`, `at_follow_back_m` behind it;
//!    * *second base of fire:* an assault with at least three squads whose
//!      base's fire weight (rounds/min × suppression over its men) is under
//!      `second_base_below_pct` % of the enemy's strongest MG team weapon
//!      gets a second base (an MG team, else a rifle squad) at least
//!      `second_base_gap_m` from the first;
//!    * *crews:* a crew squad re-crews its abandoned vehicle (else the
//!      nearest abandoned own one within `recrew_reach_m`) when it is not
//!      burning or useless, no known enemy soldier is within
//!      `recrew_clear_m` and no enemy vehicle within `threat_m`, and the
//!      re-crew check passes; it waits while men are still getting out;
//!      otherwise it walks back to base. The AI does not capture;
//!    * *denial:* a holding vehicle with no live target and no live enemy
//!      vehicle within `face_range_m` fires at an enemy abandoned vehicle
//!      within `deny_m` in its line of sight (with `capture` on).
//!
//! An order is only sent when the squad's task changed or its order no
//! longer matches the task, and never while a command for that squad is
//! still pending, so squads are not re-pathed every plan.
//!
//! Difficulty: `income_pct` (in `crate::rules`), `reaction_ticks`,
//! `replan_ticks`, and `decision_quality_pct` q: scores get uniform noise of
//! ±2·(100 − q), a needed retreat is ordered with probability 50 + q/2 %,
//! and a buy picks a random role with probability (100 − q)/2 %.
//!
//! Everything is integer or Q16.16, iterates squads, points and soldiers in
//! ascending order, and draws randomness from `rng(seed, tick, player,
//! AI_PLAN)`.

use std::collections::BTreeMap;

use frontline_content::setup::Capability;
use frontline_content::{ComponentKind, DigProfile, MatchSetup};
use serde::{Deserialize, Serialize};

use crate::command::CommandKind;
use crate::ecs::{Entities, EntityKind, PlayerId, SimId};
use crate::fx::{Fx, FxVec2};
use crate::game::{MAX_DIG_POINTS, MatchState, NavPhase, Order, SquadId};
use crate::rng::{Purpose, RngStream};
use crate::terrain::Heightfield;
use frontline_content::ItemKind;

mod ammo;
mod armour;
mod assault;
pub use ammo::{AMBUSH_END_NAMES, AMBUSH_ENDS};
pub use assault::{
    END_FAST, END_GONE, END_LEASH, END_RETREAT, END_SPENT, END_TIMEOUT, END_UNSEEN, HUNT_END_NAMES,
    HUNT_ENDS, MAX_HUNTS,
};

/// Most AI commands waiting for their reaction delay.
pub const MAX_PENDING: usize = 256;
/// Most trenches the AI remembers (one per point is planned).
pub const MAX_TRENCHES: usize = 64;
/// No point index (support and retreat tasks).
pub const NO_POINT: u8 = u8::MAX;
/// Most escorted assaults the AI tracks at once (one per own vehicle).
pub const MAX_ESCORTS: usize = 64;
// S3b: the AI's gameplay distances, speeds and timings are rules data
// (`[ai.infantry]` and `[ai.armour]`, `frontline_content::tuning`).

/// A command waiting for the AI's reaction delay.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AiCommand {
    /// Applied at the start of this tick.
    pub due: u64,
    pub seq: u32,
    pub kind: CommandKind,
}

/// What the AI wants a squad to do.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[repr(u8)]
pub enum TaskKind {
    Capture = 0,
    Attack = 1,
    Defend = 2,
    Dig = 3,
    Barrage = 4,
    Support = 5,
    Retreat = 6,
    /// S3 vehicles: hold `tank_standoff` short of an enemy point (AT seen)
    /// ...
    Standoff = 7,
    /// ... or advance to `assault_close` short of its trench.
    Assault = 8,
    /// S3b: a base of fire suppressing (or walking to its spot).
    Suppress = 9,
    /// S3b: an AT team or engineers going for an enemy vehicle.
    Hunt = 10,
    /// S3c: a vehicle driving back to base to rearm, then waiting there.
    Rearm = 11,
    /// S3c: a crew squad re-crewing an abandoned own vehicle.
    Recrew = 12,
    /// S3c: an AT team following an own vehicle's assault.
    Follow = 13,
    /// S5a: a squad walking back to the base to refill its rounds, then
    /// waiting there (`ai::ammo`).
    Refill = 14,
    /// S5a: a Panzerfaust squad waiting in ambush beside an enemy
    /// vehicle's path, holding fire (`ai::ammo`).
    Ambush = 15,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Task {
    pub kind: TaskKind,
    /// Capture point index, or [`NO_POINT`].
    pub point: u8,
    pub target: FxVec2,
}

/// A trench the AI ordered at a point (kept so the line stays put).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AiTrench {
    pub point: u8,
    pub line: [FxVec2; 2],
}

/// One AI player's commander state. Saved, loaded and hashed with the match.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AiState {
    /// Next tick that runs a plan.
    pub next_plan: u64,
    pub next_seq: u32,
    /// In due order.
    pub pending: Vec<AiCommand>,
    pub tasks: BTreeMap<SquadId, Task>,
    pub trenches: Vec<AiTrench>,
    /// Commands emitted, and how many of them `apply` rejected.
    pub issued: u32,
    pub rejected: u32,
    /// Tick of each squad's last throw order, while it cools down.
    pub last_throw: BTreeMap<SquadId, u64>,
    /// S3: an enemy vehicle has been seen (AT teams join the mix).
    pub enemy_vehicle_seen: bool,
    /// S3: tick of the last sighting of an enemy AT soldier within
    /// `threat_radius` of each point (by point index; entries older than
    /// `at_clear` are dropped).
    pub at_seen: BTreeMap<u8, u64>,
    /// S3: escorted assaults, by the vehicle's squad.
    pub escorts: BTreeMap<SquadId, AiEscort>,
    /// S3: what the AI did (skirmish report).
    pub counters: AiCounters,
    /// S3b: since when each seen enemy vehicle has been slow (satchel
    /// hunts; at most `assault::MAX_SLOW`).
    pub slow_since: BTreeMap<SimId, u64>,
    /// S3c sticky satchel hunts, by the hunting engineer squad (at most
    /// `assault::MAX_HUNTS`).
    pub hunts: BTreeMap<SquadId, AiHunt>,
    /// S5a: squads on a refill trip and the tick each reached the base
    /// (0: still on the way; at most [`MAX_PENDING`] entries).
    pub refill: BTreeMap<SquadId, u64>,
    /// S5a: Panzerfaust ambushes, by squad (at most [`MAX_AMBUSHES`]).
    pub ambush: BTreeMap<SquadId, AiAmbush>,
}

/// S5a: most Panzerfaust ambushes one AI keeps.
pub const MAX_AMBUSHES: usize = 64;

/// S5a: a Panzerfaust squad's ambush on one enemy vehicle
/// (`ai::ammo`).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AiAmbush {
    pub vehicle: SimId,
    /// Tick it started.
    pub since: u64,
    /// The spot it waits at.
    pub spot: FxVec2,
    /// Tubes its men carried at the last plan (shots fired are counted from
    /// the drop).
    pub tubes: u16,
    /// Given up (time out): the squad is free again (the S4 hunt may take
    /// it) until the vehicle is gone, so it does not start again.
    pub done: bool,
}

/// An engineer squad's satchel hunt on one enemy vehicle (S3c B2).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AiHunt {
    pub vehicle: SimId,
    /// Tick the hunt started.
    pub since: u64,
    /// Tick and place it was last seen.
    pub last_seen: u64,
    pub last_pos: FxVec2,
    /// Since when it has been faster than `ai.infantry.satchel_slow_mm_s`
    /// (`None`: slow now).
    pub fast_since: Option<u64>,
    /// Out of sight at the last plan.
    pub unseen: bool,
}

/// A vehicle's escorted assault on a point (S3).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AiEscort {
    pub point: u8,
    /// Squads still following the vehicle, ascending.
    pub squads: Vec<SquadId>,
    /// Every squad that started in the escort, ascending.
    pub started: Vec<SquadId>,
    /// Tick the escort started.
    pub since: u64,
    /// An escort squad's leader came within `ai.armour.escort_reached_m` of the
    /// enemy trench (counted once).
    pub reached: bool,
}

/// S3 AI counters (skirmish report), since the match started.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct AiCounters {
    pub vehicles_bought: u32,
    pub at_bought: u32,
    pub escorts_started: u32,
    /// Escorted assaults that reached `ai.armour.escort_reached_m` of the enemy
    /// trench.
    pub escorts_reached: u32,
    pub escorts_released: u32,
    pub satchels_at_vehicles: u32,
    pub repairs: u32,
    pub retreats: u32,
    pub faces: u32,
    /// S3b: `SuppressUnits` orders (bases of fire and vehicles).
    pub suppress_orders: u32,
    /// S3b: manoeuvre squads halted for a bound.
    pub bounds: u32,
    /// S3b: AT-team and satchel hunts started.
    pub at_hunts: u32,
    pub satchel_hunts: u32,
    /// S3c: rearm trips started (B1).
    pub rearms: u32,
    /// S3c sticky hunts (B2): hunts kept through a loss of sight, satchels
    /// thrown by hunting squads, and hunts ended by each reason (index:
    /// `assault::END_*`).
    pub hunts_kept_unseen: u32,
    pub hunt_throws: u32,
    pub hunt_ends: [u32; assault::HUNT_ENDS],
    /// S3c: assaults given a second base of fire (B4).
    pub second_bases: u32,
    /// S3c (B3): AT hunts on stuck vehicles beyond `at_hunt_m`, and AT
    /// teams sent after an own vehicle's assault.
    pub at_stuck_hunts: u32,
    pub at_follows: u32,
    /// S3c (B5): `Recrew` orders, crews sent back to base, `AttackUnit`
    /// orders at enemy abandoned vehicles.
    pub recrews: u32,
    pub crew_retreats: u32,
    pub denials: u32,
    /// S4: call-ins by squad type (index into `MatchSetup::squad_types`;
    /// grows to the type count on the first buy).
    pub bought: Vec<u32>,
    /// S4: hunts started by squads with single-shot AT items, and capture
    /// orders at enemy abandoned vehicles.
    pub pzf_hunts: u32,
    pub capture_orders: u32,
    /// S4: plans whose buy waited only for the pop cap.
    pub pop_cap_holds: u32,
    /// S5a: refill trips started (all, and those started dry below
    /// `ai.ammo.dry_pct`), and trips ended by a paused refill.
    pub refill_trips: u32,
    pub refill_dry_trips: u32,
    pub refill_stalls: u32,
    /// S5a: plan-squads away on a refill trip (summed per plan) and plans
    /// (mean squads away = `refill_away / plans`).
    pub refill_away: u32,
    pub plans: u32,
    /// S5a: mortar barrages held back by the bomb reserve; vehicle rearm
    /// waits ended by a paused refill; buys that kept `float_mp` back.
    pub mortar_reserve_holds: u32,
    pub rearm_stalls: u32,
    pub float_holds: u32,
    /// S5a Panzerfaust ambushes: started, tubes fired from one, and ends by
    /// reason (index: `ammo::AMBUSH_*`).
    pub ambushes: u32,
    pub ambush_shots: u32,
    pub ambush_ends: [u32; ammo::AMBUSH_ENDS],
}

impl AiState {
    pub(crate) fn new(first_plan: u64) -> AiState {
        AiState {
            next_plan: first_plan,
            next_seq: 0,
            pending: Vec::new(),
            tasks: BTreeMap::new(),
            trenches: Vec::new(),
            issued: 0,
            rejected: 0,
            last_throw: BTreeMap::new(),
            enemy_vehicle_seen: false,
            at_seen: BTreeMap::new(),
            escorts: BTreeMap::new(),
            counters: AiCounters::default(),
            slow_since: BTreeMap::new(),
            hunts: BTreeMap::new(),
            refill: BTreeMap::new(),
            ambush: BTreeMap::new(),
        }
    }

    fn emit(&mut self, due: u64, kind: CommandKind) {
        if self.pending.len() >= MAX_PENDING {
            return;
        }
        self.pending.push(AiCommand {
            due,
            seq: self.next_seq,
            kind,
        });
        self.next_seq = self.next_seq.wrapping_add(1);
        self.issued += 1;
    }

    /// Checks a loaded AI state (untrusted saves) of `player` at `tick`.
    pub(crate) fn validate(
        &self,
        g: &MatchState,
        player: PlayerId,
        tick: u64,
    ) -> Result<(), &'static str> {
        let np = g.points.len();
        let bad_squad = |id: &SquadId| id.0 >= g.next_squad;
        // An escort is keyed by a vehicle squad of this AI; one that no
        // longer exists is dropped at the next plan.
        let bad_escort = |id: &SquadId| {
            bad_squad(id)
                || g.squads.get(id).is_some_and(|sq| {
                    sq.owner != player
                        || sq.members.len() != 1
                        || g.setup
                            .squad_types
                            .get(sq.type_idx as usize)
                            .is_none_or(|t| t.vehicle.is_none())
                })
        };
        if self.pending.len() > MAX_PENDING
            || self.trenches.len() > MAX_TRENCHES
            // S4 review L3: one buy counter per squad type at most.
            || self.counters.bought.len() > g.setup.squad_types.len()
            || self.rejected > self.issued
            || self.tasks.keys().any(bad_squad)
            || self.last_throw.keys().any(bad_squad)
            || self.last_throw.values().any(|&t| t > tick)
            || self
                .tasks
                .values()
                .any(|t| t.point != NO_POINT && t.point as usize >= np)
            || self.trenches.iter().any(|t| t.point as usize >= np)
            || self.at_seen.len() > np
            || self
                .at_seen
                .iter()
                .any(|(&p, &t)| p as usize >= np || t > tick)
            || self.escorts.len() > MAX_ESCORTS
            || self.escorts.iter().any(|(id, e)| {
                bad_escort(id)
                    || e.point as usize >= np
                    || e.since > tick
                    || e.squads.len() > g.setup.rules.ai.armour.escort_squads as usize
                    || e.started.len() > g.setup.rules.ai.armour.escort_squads as usize
                    || e.squads.iter().chain(&e.started).any(bad_squad)
                    || e.squads.windows(2).any(|w| w[0] >= w[1])
                    || e.started.windows(2).any(|w| w[0] >= w[1])
            })
            || !self.validate_assault(tick)
            || self.hunts.keys().any(bad_squad)
            // S5a.
            || self.refill.len() > MAX_PENDING
            || self.refill.keys().any(bad_squad)
            || self.refill.values().any(|&t| t > tick)
            || self.ambush.len() > MAX_AMBUSHES
            || self.ambush.keys().any(bad_squad)
            || self.ambush.values().any(|a| a.since > tick)
            || self.pending.windows(2).any(|w| w[0].due > w[1].due)
            || self.pending.iter().any(|c| match &c.kind {
                CommandKind::Dig { line, .. } => line.len() > MAX_DIG_POINTS,
                // `buy` indexes the squad types by pending call-ins.
                CommandKind::CallIn { squad_type } => {
                    *squad_type as usize >= g.setup.squad_types.len()
                }
                CommandKind::Throw { units, .. }
                | CommandKind::ThrowEach { units, .. }
                | CommandKind::FaceUnits { units, .. }
                | CommandKind::RepairUnits { units }
                | CommandKind::FollowUnits { units, .. } => units.len() > crate::unit::MAX_UNITS,
                CommandKind::SuppressUnits { units, line } => {
                    units.len() > crate::unit::MAX_UNITS || !(1..=2).contains(&line.len())
                }
                CommandKind::MoveSoldier { .. }
                | CommandKind::Spawn { .. }
                | CommandKind::Despawn { .. } => true,
                _ => false,
            })
        {
            return Err("AI state out of range");
        }
        Ok(())
    }
}

/// The squad a command addresses, if any.
fn squad_of(kind: &CommandKind) -> Option<SquadId> {
    match kind {
        CommandKind::Move { squad, .. }
        | CommandKind::AttackMove { squad, .. }
        | CommandKind::Stop { squad }
        | CommandKind::Retreat { squad }
        | CommandKind::Dig { squad, .. }
        | CommandKind::Barrage { squad, .. } => Some(*squad),
        _ => None,
    }
}

/// The S3 vehicle orders whose `units` name the squads they address (an
/// escort's men, a vehicle).
fn units_of(kind: &CommandKind) -> Option<&[SimId]> {
    match kind {
        CommandKind::FollowUnits { units, .. }
        | CommandKind::FaceUnits { units, .. }
        | CommandKind::RepairUnits { units }
        | CommandKind::SuppressUnits { units, .. } => Some(units),
        _ => None,
    }
}

/// S4: what the AI treats a squad as, from its card's tags (the first of
/// crew, vehicle, at, mortar, engineer, mg, line it carries; `Other` for
/// none). The behaviours keyed on it (point scores, digging, assaults,
/// hunts) are the S1-S3c role behaviours; capabilities gate the orders.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum AiKind {
    Rifle,
    Mg,
    Engineer,
    Mortar,
    AntiTank,
    Vehicle,
    Crew,
    Other,
}

/// The [`AiKind`] of squad type `t`.
pub(crate) fn ai_kind(s: &MatchSetup, t: u16) -> AiKind {
    match s.type_word(t) {
        "crew" => AiKind::Crew,
        "vehicle" => AiKind::Vehicle,
        "at" => AiKind::AntiTank,
        "mortar" => AiKind::Mortar,
        "engineer" => AiKind::Engineer,
        "mg" => AiKind::Mg,
        "rifle" => AiKind::Rifle,
        _ => AiKind::Other,
    }
}

/// S4: the buy-mix slot (index into `AiSetup::mix`) of squad type `t`:
/// the first mix entry whose tag the card carries; `None` for a card the
/// AI never buys (no mix tag, or a crew).
fn mix_slot(s: &MatchSetup, t: u16) -> Option<usize> {
    let ty = &s.squad_types[t as usize];
    if ty.has_tag(s.tag("crew")) {
        return None;
    }
    s.rules.ai.mix.iter().position(|m| ty.tags.contains(&m.tag))
}

/// Squared distance bound for `m` metres, comparable with `length_sq_raw`.
fn sq_m(m: Fx) -> u64 {
    FxVec2::new(m, Fx::ZERO).length_sq_raw()
}

fn dist_m(a: FxVec2, b: FxVec2) -> i64 {
    (a - b).length().floor_int() as i64
}

/// A spotted enemy soldier.
struct Known {
    id: SimId,
    pos: FxVec2,
    squad: u32,
    in_trench: bool,
    /// Of an AT team (S3).
    at: bool,
}

/// A spotted enemy vehicle (S3).
struct KnownVehicle {
    id: SimId,
    pos: FxVec2,
    yaw: Fx,
    speed: Fx,
    vel: FxVec2,
    type_idx: u16,
}

/// One of the AI's own vehicles free for a task at plan time (S3).
struct MyTank {
    squad: SquadId,
    id: SimId,
    pos: FxVec2,
    yaw: Fx,
    /// Cannot drive now (bogged, immobilised, under repair, ...).
    immobile: bool,
}

/// One of the AI's own squads at plan time.
struct Mine {
    id: SquadId,
    role: AiKind,
    lead: FxVec2,
    strength_pct: u32,
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum Rel {
    Own,
    Neutral,
    Enemy,
}

struct PointEval {
    pos: FxVec2,
    rel: Rel,
    threat: u32,
    /// Seen enemy vehicles within `ai.armour.threat_m` (S3).
    armour: u32,
    priority: i64,
    demand: u32,
    assigned: u32,
    /// Index into `AiState::trenches`, and whether it is finished.
    trench: Option<usize>,
    trench_done: bool,
}

impl MatchState {
    /// Moves due AI commands (all AI players, in player order) into `out`.
    pub(crate) fn take_ai_commands(&mut self, tick: u64, out: &mut Vec<crate::Command>) {
        for (p, ai) in self.ai.iter_mut().enumerate() {
            let Some(ai) = ai else { continue };
            let n = ai.pending.iter().take_while(|c| c.due <= tick).count();
            out.extend(ai.pending.drain(..n).map(|c| crate::Command {
                player: PlayerId(p as u8),
                seq: c.seq,
                kind: c.kind,
            }));
        }
    }

    /// Counts a rejected AI command for `player`.
    pub(crate) fn ai_rejected(&mut self, player: PlayerId) {
        if let Some(Some(ai)) = self.ai.get_mut(player.0 as usize) {
            ai.rejected += 1;
        }
    }

    /// Runs the plans that are due this tick (module docs).
    pub(crate) fn run_ai(&mut self, ents: &Entities, terrain: &Heightfield, tick: u64) {
        if self.outcome != crate::game::Outcome::Running {
            return;
        }
        for p in 0..self.ai.len() {
            let Some(mut ai) = self.ai[p].take() else {
                continue;
            };
            if self.players[p].defeat.is_none() && tick >= ai.next_plan {
                let ps = &self.setup.players[p];
                let d = self.setup.rules.difficulty.get(ps.difficulty);
                ai.next_plan = tick + d.replan_ticks.max(1) as u64;
                let due = tick + d.reaction_ticks.max(1) as u64;
                Planner {
                    g: self,
                    ai: &mut ai,
                    me: PlayerId(p as u8),
                    ents,
                    terrain,
                    due,
                    tick,
                    quality: d.decision_quality_pct.min(100) as i64,
                    rng: RngStream::new(self.setup.seed, tick, SimId(p as u32), Purpose::AI_PLAN),
                }
                .plan();
            }
            self.ai[p] = Some(ai);
        }
    }
}

struct Planner<'a> {
    g: &'a MatchState,
    ai: &'a mut AiState,
    me: PlayerId,
    ents: &'a Entities,
    terrain: &'a Heightfield,
    due: u64,
    tick: u64,
    quality: i64,
    rng: RngStream,
}

impl Planner<'_> {
    fn plan(&mut self) {
        let g = self.g;
        let me = self.me;
        self.ai
            .tasks
            .retain(|id, _| g.squads.get(id).is_some_and(|s| s.owner == me));
        let mut busy: Vec<SquadId> = Vec::new();
        for c in &self.ai.pending {
            if let Some(s) = squad_of(&c.kind) {
                busy.push(s);
            } else if let Some(units) = units_of(&c.kind) {
                busy.extend(units.iter().filter_map(|u| self.ents.squad[u.index()]));
            }
        }

        let known = self.known_enemies();
        let kv = self.known_vehicles();
        self.note_sightings(&known, &kv);
        self.note_slow(&kv);
        let mut mine = Vec::new();
        let mut tanks = Vec::new();
        let mut crews = Vec::new();
        for (&id, sq) in &g.squads {
            if sq.owner != me || busy.contains(&id) {
                continue;
            }
            let ty = &g.setup.squad_types[sq.type_idx as usize];
            // S3c: crews re-crew or walk back to base (step 9).
            if ai_kind(&g.setup, sq.type_idx) == AiKind::Crew {
                crews.push(id);
                continue;
            }
            if ty.vehicle.is_some() {
                let v = sq.members[0];
                let t = &self.ents.transform[v.index()];
                let vs = &g.vehicles[&v];
                tanks.push(MyTank {
                    squad: id,
                    id: v,
                    pos: t.pos.xy(),
                    yaw: t.yaw,
                    immobile: vs.immobile(&g.setup.vehicles[vs.type_idx as usize]),
                });
                continue;
            }
            mine.push(Mine {
                id,
                role: ai_kind(&g.setup, sq.type_idx),
                lead: self.ents.transform[sq.lead(self.ents).index()].pos.xy(),
                strength_pct: sq.members.len() as u32 * 100 / ty.men.max(1) as u32,
            });
        }

        self.retreats(&mut mine, &known);
        // S5a: refill trips, staggered per point.
        self.refills(&mut mine, &known);
        self.vehicle_upkeep(&mut tanks, &known, &kv);
        self.crews(&crews, &known, &kv);
        self.buy(&kv);
        let mut points = self.evaluate_points(&known, &kv);
        let assigned = self.assign(&mine, &mut points);
        // S5a: Panzerfaust ambushes first; the hunts skip their squads.
        let ambushing = self.ambushes(&mine, &assigned, &points, &kv);
        let mut hunting = self.hunts(&mine, &assigned, &points, &known, &kv);
        hunting.extend(ambushing);
        hunting.sort_unstable();
        hunting.dedup();
        let escorting = self.vehicle_tasks(
            &tanks, &mine, &assigned, &points, &known, &kv, &busy, &hunting,
        );
        let mut claimed: Vec<SquadId> = hunting.iter().chain(&escorting).copied().collect();
        claimed.extend(self.fire_and_move(&mine, &assigned, &points, &known, &claimed));
        self.order_infantry(&mine, &assigned, &mut points, &known, &escorting, &claimed);
        self.mortars(&mine, &points, &known);
        self.throws(&mine, &known, &kv);
    }

    /// S3c: whether the squad's leader stands in the own base.
    fn in_own_base(&self, sq: &crate::game::Squad) -> bool {
        let s = &self.g.setup;
        let base = &s.map.bases[s.players[self.me.0 as usize].base as usize];
        let r = sq_m(base.radius.into());
        let lead = self.ents.transform[sq.lead(self.ents).index()].pos.xy();
        (lead - FxVec2::from(base.center)).length_sq_raw() <= r
    }

    /// Uniform integer in `[0, n)`.
    fn below(&mut self, n: u32) -> i64 {
        crate::rng::below(self.rng.next_u64(), n) as i64
    }

    /// Uniform noise of ±2·(100 − q).
    fn noise(&mut self) -> i64 {
        let a = 2 * (100 - self.quality);
        if a == 0 {
            return 0;
        }
        self.rng.range_i32(-a as i32, a as i32) as i64
    }

    fn known_enemies(&self) -> Vec<Known> {
        let bit = 1u16 << self.me.0;
        self.ents
            .soldiers()
            .filter(|id| {
                let i = id.index();
                self.ents.owner[i] != self.me && self.ents.combat[i].seen_by & bit != 0
            })
            .map(|id| {
                let i = id.index();
                let squad = self.ents.squad[i];
                Known {
                    id,
                    pos: self.ents.transform[i].pos.xy(),
                    squad: squad.map_or(u32::MAX, |s| s.0),
                    in_trench: self.ents.combat[i].in_trench,
                    at: squad
                        .and_then(|s| self.g.squads.get(&s))
                        .is_some_and(|s| s.launcher.is_some()),
                }
            })
            .collect()
    }

    fn threat_at(&self, known: &[Known], at: FxVec2) -> u32 {
        let r = sq_m(self.g.setup.rules.ai.threat_radius.into());
        known
            .iter()
            .filter(|k| (k.pos - at).length_sq_raw() <= r)
            .count() as u32
    }

    fn task(&mut self, id: SquadId, kind: TaskKind, point: u8, target: FxVec2) {
        self.ai.tasks.insert(
            id,
            Task {
                kind,
                point,
                target,
            },
        );
    }

    // ---------------------------------------------------------- retreat --

    fn retreats(&mut self, mine: &mut Vec<Mine>, known: &[Known]) {
        let pct = self.g.setup.rules.ai.retreat_strength_pct;
        let mut keep = Vec::with_capacity(mine.len());
        for m in mine.drain(..) {
            let sq = &self.g.squads[&m.id];
            if sq.order == Order::Retreat {
                continue;
            }
            if self.ai.tasks.get(&m.id).map(|t| t.kind) == Some(TaskKind::Retreat) {
                // Arrived at base: available again.
                self.ai.tasks.remove(&m.id);
            }
            if m.strength_pct < pct && self.threat_at(known, m.lead) > 0 {
                let roll = self.below(100);
                if roll < 50 + self.quality / 2 {
                    self.ai.emit(self.due, CommandKind::Retreat { squad: m.id });
                    let base = self.g.base_of(self.me);
                    self.task(m.id, TaskKind::Retreat, NO_POINT, base);
                    continue;
                }
            }
            keep.push(m);
        }
        *mine = keep;
    }

    // -------------------------------------------------------------- buy --

    fn buy(&mut self, kv: &[KnownVehicle]) {
        let g = self.g;
        let s = &g.setup;
        let p = self.me.0 as usize;
        let faction = &s.factions[s.players[p].faction as usize];
        let ai = &s.rules.ai;
        // S4 tag mix: one slot per `AiSetup::mix` entry; a card counts
        // toward the first entry whose tag it carries.
        let slots = ai.mix.len();
        let at_slot = s
            .tag("at")
            .and_then(|t| ai.mix.iter().position(|m| m.tag == t));
        let veh_slot = s
            .tag("vehicle")
            .and_then(|t| ai.mix.iter().position(|m| m.tag == t));
        let gated = |r: usize| Some(r) == at_slot || Some(r) == veh_slot;
        let mut cards: Vec<Vec<u16>> = vec![Vec::new(); slots];
        for &t in &faction.roster {
            if let Some(r) = mix_slot(s, t) {
                cards[r].push(t);
            }
        }
        // The slot's highest-weight card (ties: the first in the roster):
        // the one the AI saves up for.
        let best = |r: usize| -> Option<u16> {
            cards[r]
                .iter()
                .copied()
                .fold(None, |b: Option<u16>, t| match b {
                    Some(b)
                        if s.squad_types[b as usize].ai_weight
                            >= s.squad_types[t as usize].ai_weight =>
                    {
                        Some(b)
                    }
                    _ => Some(t),
                })
        };
        let mut counts = vec![0i64; slots];
        let mut alive_by_type = vec![0u32; s.squad_types.len()];
        for sq in g.squads.values().filter(|sq| sq.owner == self.me) {
            alive_by_type[sq.type_idx as usize] += 1;
            if let Some(r) = mix_slot(s, sq.type_idx) {
                counts[r] += 1;
            }
        }
        // S4: own squads still carrying a single-shot AT item count as half
        // an AT team toward the urgent-AT rule.
        let k = ItemKind::SingleShotAt.index();
        let pzf_squads = g
            .squads
            .values()
            .filter(|sq| {
                sq.owner == self.me
                    && sq.pzf.is_some()
                    && sq
                        .members
                        .iter()
                        .any(|m| self.ents.kit()[m.index()].items[k] > 0)
            })
            .count() as i64;
        let pl = &g.players[p];
        let mut budget = pl.manpower as i64;
        // S5a: keep `float_mp` back while an own unit waits for a paid
        // refill in the base.
        if self.waiting_refill() {
            budget -= s.rules.ai.ammo.float_mp as i64;
            self.ai.counters.float_holds += 1;
        }
        let mut pop = g.population(self.ents, self.me) as i64;
        let mut vehicles = g.vehicles_counted(self.ents, self.me);
        // S3b: types bought by call-ins still pending are on cooldown from
        // their buy (with their cooldown group).
        let mut bought: Vec<u16> = Vec::new();
        for c in &self.ai.pending {
            if let CommandKind::CallIn { squad_type } = c.kind {
                let ty = &s.squad_types[squad_type as usize];
                if let Some(r) = mix_slot(s, squad_type) {
                    counts[r] += 1;
                }
                alive_by_type[squad_type as usize] += 1;
                budget -= ty.cost as i64;
                pop += ty.pop(&s.vehicles) as i64;
                vehicles += ty.vehicle.is_some() as u32;
                bought.push(squad_type);
            }
        }
        // A type can be bought when its cooldown is over by the time the
        // command is applied and no pending buy restarts it; S4: and its
        // card limits (`max_alive`, limit group) leave room.
        let due = self.due;
        let me = self.me;
        let ready = |t: u16, bought: &[u16], alive: &[u32]| {
            let ty = &s.squad_types[t as usize];
            let group = ty.cooldown_group;
            let under_max = ty.max_alive == 0 || alive[t as usize] < ty.max_alive as u32;
            let under_group = ty.limit_group.is_none_or(|lg| {
                let n: u32 = s
                    .squad_types
                    .iter()
                    .enumerate()
                    .filter(|(_, o)| o.limit_group == Some(lg))
                    .map(|(k, _)| alive[k])
                    .sum();
                n < s.rules.limit_groups[lg as usize].limit
            });
            under_max
                && under_group
                && due >= ty.initial_cooldown_ticks as u64
                && g.cooldown_left(me, t, due) == 0
                && !bought.iter().any(|&b| {
                    b == t || (group.is_some() && s.squad_types[b as usize].cooldown_group == group)
                })
        };
        // S3: vehicles after the unlock and under the limit; AT teams after
        // the unlock once an enemy vehicle has been seen. Before the unlock
        // the mix is the S1 one.
        let vr = &s.rules.vehicles;
        let unlocked = self.tick >= vr.unlock_ticks as u64;
        let mut mix: Vec<u32> = ai
            .mix
            .iter()
            .enumerate()
            .map(|(r, m)| if gated(r) { 0 } else { m.pct })
            .collect();
        if let Some(r) = at_slot
            && unlocked
            && self.ai.enemy_vehicle_seen
        {
            mix[r] = ai.mix[r].pct;
        }
        if let Some(r) = veh_slot
            && unlocked
            && vehicles < vr.limit
        {
            mix[r] = ai.mix[r].pct;
        }
        let count_of = |counts: &[i64], r: Option<usize>| r.map_or(0, |r| counts[r]);
        for _ in 0..self.g.setup.rules.ai.infantry.buys_per_plan {
            let total: i64 = counts.iter().sum();
            let active = |r: usize| !cards[r].is_empty() && (!gated(r) || mix[r] > 0);
            let weight: i64 = (0..slots)
                .filter(|&r| active(r))
                .map(|r| mix[r] as i64)
                .sum();
            // Slots by how far they are below the mix, furthest first.
            let mut ranked: Vec<usize> = (0..slots).filter(|&r| active(r) && mix[r] > 0).collect();
            ranked.sort_by_key(|&r| {
                (
                    -(mix[r] as i64 * (total + 1) - weight * counts[r]),
                    r as i64,
                )
            });
            let mut role = ranked.first().copied();
            let roll = self.below(200);
            // Saving up for a vehicle: no stray buy spends the savings.
            let saving = role.is_some() && role == veh_slot && {
                let r = role.unwrap_or(0);
                best(r).is_some_and(|t| {
                    budget < s.squad_types[t as usize].cost as i64
                        && ready(t, &bought, &alive_by_type)
                })
            };
            if roll < 100 - self.quality && !saving {
                let options: Vec<usize> = (0..slots).filter(|&r| active(r)).collect();
                if !options.is_empty() {
                    role = Some(options[self.below(options.len() as u32) as usize]);
                }
            }
            // More enemy vehicles in sight than own AT teams and vehicles:
            // an AT team first, reserve or not.
            let mut urgent_at = false;
            if let Some(a) = at_slot
                && mix[a] > 0
                && !cards[a].is_empty()
                && 2 * kv.len() as i64
                    > 2 * (count_of(&counts, at_slot) + count_of(&counts, veh_slot)) + pzf_squads
            {
                role = Some(a);
                urgent_at = true;
            }
            // S3b: the chosen slot if a card of it is off cooldown, else the
            // next in mix order that is.
            let Some(first) = role else {
                return;
            };
            let Some(r) = std::iter::once(first)
                .chain(ranked.iter().copied().filter(|&r| r != first))
                .find(|&r| cards[r].iter().any(|&t| ready(t, &bought, &alive_by_type)))
            else {
                return;
            };
            // S4: among the slot's ready cards, the one furthest below its
            // `ai_weight` share of this match's buys (fewest buys per weight;
            // ties: roster order), so full and understrength cards come in
            // the ratio of their weights. Out of manpower or pop for it:
            // save up (no cheaper card instead).
            let ready_cards: Vec<u16> = cards[r]
                .iter()
                .copied()
                .filter(|&t| ready(t, &bought, &alive_by_type))
                .collect();
            // (`AiCounters::bought` counts at emit: pending buys included.)
            let buys = |t: u16| {
                self.ai
                    .counters
                    .bought
                    .get(t as usize)
                    .copied()
                    .unwrap_or(0) as u64
            };
            let Some(&t) = ready_cards.iter().min_by(|&&a, &&b| {
                let wa = s.squad_types[a as usize].ai_weight.max(1) as u64;
                let wb = s.squad_types[b as usize].ai_weight.max(1) as u64;
                ((buys(a) + 1) * wb).cmp(&((buys(b) + 1) * wa))
            }) else {
                return;
            };
            {
                let ty = &s.squad_types[t as usize];
                if budget < ty.cost as i64 {
                    return;
                }
                if pop + ty.pop(&s.vehicles) as i64 > s.rules.pop_cap as i64 {
                    // S4 report: held back by the pop cap.
                    self.ai.counters.pop_cap_holds += 1;
                    return;
                }
            }
            let ty = &s.squad_types[t as usize];
            let men = ty.pop(&s.vehicles) as i64;
            // After the unlock, under the vehicle limit: infantry leaves a
            // small reserve toward the next vehicle.
            let reserve = if veh_slot.is_some_and(|v| mix[v] > 0 && !cards[v].is_empty()) {
                ai.armour.tank_reserve_mp as i64
            } else {
                0
            };
            if Some(r) != veh_slot
                && !(urgent_at && Some(r) == at_slot)
                && budget - (ty.cost as i64) < reserve
            {
                return;
            }
            self.ai
                .emit(self.due, CommandKind::CallIn { squad_type: t });
            budget -= ty.cost as i64;
            pop += men;
            bought.push(t);
            alive_by_type[t as usize] += 1;
            counts[r] += 1;
            let c = &mut self.ai.counters;
            if c.bought.len() < s.squad_types.len() {
                c.bought.resize(s.squad_types.len(), 0);
            }
            c.bought[t as usize] += 1;
            if Some(r) == veh_slot {
                c.vehicles_bought += 1;
                vehicles += 1;
                if vehicles >= vr.limit {
                    mix[r] = 0;
                }
            } else if Some(r) == at_slot {
                c.at_bought += 1;
            }
        }
    }

    // ----------------------------------------------------------- points --

    fn evaluate_points(&mut self, known: &[Known], kv: &[KnownVehicle]) -> Vec<PointEval> {
        let g = self.g;
        let base = g.base_of(self.me);
        let depth_dm = Fx::from(g.setup.rules.dig.depth).mul_int(10).floor_int();
        let mut out = Vec::with_capacity(g.points.len());
        for (i, (c, ps)) in g.points.iter().zip(&g.setup.map.points).enumerate() {
            let pos: FxVec2 = ps.pos.into();
            let rel = match c.owner {
                Some(o) if o == self.me => Rel::Own,
                None => Rel::Neutral,
                Some(_) => Rel::Enemy,
            };
            let threat = self.threat_at(known, pos);
            let armour_r = sq_m(Fx::from_int(self.g.setup.rules.ai.armour.threat_m as i32));
            let armour = kv
                .iter()
                .filter(|v| (v.pos - pos).length_sq_raw() <= armour_r)
                .count() as u32;
            let under_capture = c.capturer.is_some_and(|q| q != self.me);
            let mut priority = ps.value as i64 * 100 - dist_m(pos, base) / 8;
            priority += match rel {
                Rel::Neutral => 150,
                Rel::Enemy => 100,
                Rel::Own if threat > 0 || under_capture => 80 + threat as i64 * 10,
                Rel::Own => 20,
            };
            let extra = (threat / 4).min(2);
            let demand = match rel {
                Rel::Own if threat == 0 && !under_capture => 1,
                Rel::Own | Rel::Neutral => 1 + extra,
                Rel::Enemy => 2 + extra,
            };
            let trench = self.ai.trenches.iter().position(|t| t.point as usize == i);
            let trench_done = trench.is_some_and(|k| {
                let [a, b] = self.ai.trenches[k].line;
                let (gx, gy) = self.terrain.nearest_grid((a + b).div_int(2));
                self.terrain.dug_dm(gx, gy) as i32 * 10 >= depth_dm * 9
            });
            out.push(PointEval {
                pos,
                rel,
                threat,
                armour,
                priority,
                demand,
                assigned: 0,
                trench,
                trench_done,
            });
        }
        out
    }

    /// Greedy squad → point assignment. Returns `(squad index in mine,
    /// point index)` in assignment order.
    fn assign(&mut self, mine: &[Mine], points: &mut [PointEval]) -> Vec<(usize, usize)> {
        let mut pairs = Vec::new();
        let g = self.g;
        for (j, m) in mine.iter().enumerate() {
            // S4 review L2: every barrage-capable squad is a mortar team to
            // the planner (`mortars`), never also a point holder.
            let barrage = g.squads.get(&m.id).is_some_and(|sq| {
                g.setup.squad_types[sq.type_idx as usize].has(Capability::Barrage)
            });
            if m.role == AiKind::Mortar || barrage {
                continue;
            }
            let cur = self.ai.tasks.get(&m.id).map(|t| t.point);
            for (i, pt) in points.iter().enumerate() {
                let mut score = pt.priority - dist_m(m.lead, pt.pos) / 4;
                if cur == Some(i as u8) {
                    score += 60;
                }
                let weak = m.strength_pct < 60;
                score += match (m.role, pt.rel) {
                    (AiKind::AntiTank, Rel::Own) if pt.armour > 0 => 150,
                    (AiKind::AntiTank, Rel::Own) => 40,
                    (AiKind::AntiTank, _) => -60,
                    (AiKind::Engineer, Rel::Own) if !pt.trench_done => 80,
                    (AiKind::Mg, Rel::Own) => 30,
                    (AiKind::Mg, Rel::Enemy) => -20,
                    (_, Rel::Own) if weak => 40,
                    _ if weak => -40,
                    _ => 0,
                };
                score += self.noise();
                pairs.push((score, j, i));
            }
        }
        // Best score first; ties by squad then point index.
        pairs.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
        let mut done = vec![false; mine.len()];
        let mut out = Vec::new();
        for &(_, j, i) in &pairs {
            if !done[j] && points[i].assigned < points[i].demand {
                done[j] = true;
                points[i].assigned += 1;
                out.push((j, i));
            }
        }
        for &(_, j, i) in &pairs {
            if !done[j] {
                done[j] = true;
                points[i].assigned += 1;
                out.push((j, i));
            }
        }
        out
    }

    fn order_infantry(
        &mut self,
        mine: &[Mine],
        assigned: &[(usize, usize)],
        points: &mut [PointEval],
        known: &[Known],
        escorting: &[SquadId],
        claimed: &[SquadId],
    ) {
        let g = self.g;
        let mut slot = vec![0u32; points.len()];
        for &(j, i) in assigned {
            let m = &mine[j];
            if claimed.contains(&m.id) && !escorting.contains(&m.id) {
                // A hunt, a base of fire or a bound (S3b, `ai::assault`).
                continue;
            }
            if escorting.contains(&m.id) {
                // Walking with a vehicle (S3); it is released later.
                let pos = points[i].pos;
                self.task(m.id, TaskKind::Attack, i as u8, pos);
                continue;
            }
            let pt = &points[i];
            let k = slot[i];
            slot[i] += 1;
            let sq = &g.squads[&m.id];
            let wants_dig = m.role == AiKind::Engineer
                && pt.rel == Rel::Own
                && !pt.trench_done
                && pt.threat < 4;
            if wants_dig {
                let line = match pt.trench {
                    Some(t) => Some(self.ai.trenches[t].line),
                    None => self.trench_line(i, points, known),
                };
                let Some(line) = line else { continue };
                if pt.trench.is_none() && self.ai.trenches.len() < MAX_TRENCHES {
                    self.ai.trenches.push(AiTrench {
                        point: i as u8,
                        line,
                    });
                    points[i].trench = Some(self.ai.trenches.len() - 1);
                }
                let mid = (line[0] + line[1]).div_int(2);
                let digging = matches!(&sq.order, Order::Dig { line: l, .. } if l[..] == line[..]);
                if !digging {
                    self.ai.emit(
                        self.due,
                        CommandKind::Dig {
                            squad: m.id,
                            line: line.to_vec(),
                            profile: DigProfile::Trench,
                        },
                    );
                }
                self.task(m.id, TaskKind::Dig, i as u8, mid);
                continue;
            }
            let (kind, target) = match pt.rel {
                Rel::Own => {
                    let target = match pt.trench {
                        Some(t) if pt.trench_done => {
                            // Stand along the trench, squads spread over it.
                            let [a, b] = self.ai.trenches[t].line;
                            let mid = (a + b).div_int(2);
                            let along = (b - a).div_int(4);
                            match k % 3 {
                                0 => mid,
                                1 => mid + along,
                                _ => mid - along,
                            }
                        }
                        _ => ring_slot(pt.pos, k),
                    };
                    (TaskKind::Defend, target)
                }
                Rel::Neutral => (TaskKind::Capture, ring_slot(pt.pos, k)),
                Rel::Enemy => (TaskKind::Attack, ring_slot(pt.pos, k)),
            };
            let target = self.terrain.clamp(target);
            let same = self
                .ai
                .tasks
                .get(&m.id)
                .is_some_and(|t| t.kind == kind && t.point == i as u8);
            let near =
                |a: FxVec2, b: FxVec2, r: i32| (a - b).length_sq_raw() <= sq_m(Fx::from_int(r));
            let on_track = match &sq.order {
                Order::Idle => near(m.lead, target, 20),
                Order::Move { target: t } | Order::AttackMove { target: t } => near(*t, target, 10),
                _ => false,
            };
            if !(same && on_track) {
                let cmd = if kind == TaskKind::Defend {
                    CommandKind::Move {
                        squad: m.id,
                        target,
                    }
                } else {
                    CommandKind::AttackMove {
                        squad: m.id,
                        target,
                    }
                };
                self.ai.emit(self.due, cmd);
            }
            self.task(m.id, kind, i as u8, target);
        }
    }

    /// A trench line in front of point `i`, across the direction to the
    /// nearest enemy (module docs). `None` if it would not fit the map.
    fn trench_line(&self, i: usize, points: &[PointEval], known: &[Known]) -> Option<[FxVec2; 2]> {
        let g = self.g;
        let cfg = &g.setup.rules.ai;
        let at = points[i].pos;
        let toward = known
            .iter()
            .map(|k| k.pos)
            .min_by_key(|p| ((*p - at).length_sq_raw(), p.x, p.y))
            .or_else(|| {
                points
                    .iter()
                    .filter(|p| p.rel == Rel::Enemy)
                    .map(|p| p.pos)
                    .min_by_key(|p| (*p - at).length_sq_raw())
            })
            .or_else(|| {
                (0..g.players.len())
                    .filter(|&q| q != self.me.0 as usize)
                    .map(|q| g.base_of(PlayerId(q as u8)))
                    .min_by_key(|p| (*p - at).length_sq_raw())
            })?;
        let dir = (toward - at).normalize_or_zero();
        if dir == FxVec2::ZERO {
            return None;
        }
        let center = at + dir * Fx::from(cfg.dig_offset);
        let half = dir.perp() * Fx::from(cfg.dig_length).div_int(2);
        let line = [
            self.terrain.clamp(center - half),
            self.terrain.clamp(center + half),
        ];
        let len = (line[1] - line[0]).length();
        (len >= Fx::from_int(2) && len <= Fx::from(g.setup.rules.dig.max_length)).then_some(line)
    }

    // ---------------------------------------------------------- mortars --

    fn mortars(&mut self, mine: &[Mine], points: &[PointEval], known: &[Known]) {
        let g = self.g;
        let s = &g.setup;
        // Enemy groups by squad: (squad, sum x, sum y, men, men in trenches).
        let mut groups: BTreeMap<u32, (i64, i64, i64, i64)> = BTreeMap::new();
        for k in known {
            let e = groups.entry(k.squad).or_default();
            e.0 += k.pos.x.raw() as i64;
            e.1 += k.pos.y.raw() as i64;
            e.2 += 1;
            e.3 += k.in_trench as i64;
        }
        let own: Vec<FxVec2> = self
            .ents
            .soldiers()
            .filter(|id| self.ents.owner[id.index()] == self.me)
            .map(|id| self.ents.transform[id.index()].pos.xy())
            .collect();
        let danger = sq_m(Fx::from_int(
            self.g.setup.rules.ai.infantry.danger_close_m as i32,
        ));
        let front = points
            .iter()
            .filter(|p| p.rel != Rel::Own || p.threat > 0)
            .max_by_key(|p| p.priority)
            .or_else(|| points.iter().max_by_key(|p| p.priority))
            .map(|p| p.pos);
        // S4: squads that can barrage (a member carries a mortar).
        for m in mine
            .iter()
            .filter(|m| s.squad_types[g.squads[&m.id].type_idx as usize].has(Capability::Barrage))
        {
            let sq = &g.squads[&m.id];
            if matches!(sq.order, Order::Barrage { .. }) {
                continue;
            }
            let w = &s.weapons[s.squad_types[sq.type_idx as usize].weapon as usize];
            let min = Fx::from(w.range_min) + Fx::from_int(20);
            let max = Fx::from(w.range_max) - Fx::from_int(10);
            let mut best: Option<(i64, FxVec2)> = None;
            for &(sx, sy, n, trench) in groups.values() {
                let c = FxVec2::new(Fx::from_raw((sx / n) as i32), Fx::from_raw((sy / n) as i32));
                let d = (c - m.lead).length();
                if d < min || d > max || own.iter().any(|o| (*o - c).length_sq_raw() <= danger) {
                    continue;
                }
                let score = n * 10 + trench * 20 + self.noise();
                if score >= 20 && best.is_none_or(|(b, _)| score > b) {
                    best = Some((score, c));
                }
            }
            // S5a mortar reserve: barrage only above the reserve unless an
            // own point is under threat, never more rounds than it holds.
            let (held, pct) = crate::supply::mortar_bombs(s, self.ents, sq);
            let defend = points.iter().any(|p| p.rel == Rel::Own && p.threat > 0);
            if best.is_some()
                && (held == 0
                    || (pct.is_some_and(|p| p <= s.rules.ai.ammo.mortar_reserve_pct) && !defend))
            {
                self.ai.counters.mortar_reserve_holds += 1;
                best = None;
            }
            if let Some((_, target)) = best {
                let rounds = match pct {
                    Some(_) => s
                        .rules
                        .ai
                        .barrage_rounds
                        .min(held.min(u16::MAX as u32) as u16),
                    None => s.rules.ai.barrage_rounds,
                };
                self.ai.emit(
                    self.due,
                    CommandKind::Barrage {
                        squad: m.id,
                        target,
                        rounds,
                    },
                );
                self.task(m.id, TaskKind::Barrage, NO_POINT, target);
                continue;
            }
            let Some(front) = front else { continue };
            let back = (g.base_of(self.me) - front).normalize_or_zero();
            let spot = self.terrain.clamp(
                front + back * Fx::from_int(self.g.setup.rules.ai.infantry.support_back_m as i32),
            );
            let near = |a: FxVec2, r: i32| (a - spot).length_sq_raw() <= sq_m(Fx::from_int(r));
            let on_track = match &sq.order {
                Order::Idle => near(m.lead, 40),
                Order::Move { target } => near(*target, 40),
                _ => false,
            };
            if !on_track {
                self.ai.emit(
                    self.due,
                    CommandKind::Move {
                        squad: m.id,
                        target: spot,
                    },
                );
            }
            self.task(m.id, TaskKind::Support, NO_POINT, spot);
        }
    }
}

impl Planner<'_> {
    // --------------------------------------------------------- vehicles --

    fn known_vehicles(&self) -> Vec<KnownVehicle> {
        let bit = 1u16 << self.me.0;
        self.g
            .vehicles
            .iter()
            .filter(|(id, _)| {
                let i = id.index();
                self.ents.kind[i] == EntityKind::Vehicle
                    && self.ents.owner[i] != self.me
                    && self.ents.combat[i].seen_by & bit != 0
            })
            .map(|(&id, vs)| {
                let t = &self.ents.transform[id.index()];
                KnownVehicle {
                    id,
                    pos: t.pos.xy(),
                    yaw: t.yaw,
                    speed: vs.speed,
                    vel: vs.avg_vel,
                    type_idx: vs.type_idx,
                }
            })
            .collect()
    }

    /// Remembers enemy vehicles and AT teams seen (module docs).
    fn note_sightings(&mut self, known: &[Known], kv: &[KnownVehicle]) {
        if !kv.is_empty() {
            self.ai.enemy_vehicle_seen = true;
        }
        let ai = &self.g.setup.rules.ai;
        let clear = ai.at_clear_ticks as u64;
        let tick = self.tick;
        self.ai
            .at_seen
            .retain(|_, t| tick < t.saturating_add(clear));
        let r = sq_m(ai.threat_radius.into());
        for (i, ps) in self.g.setup.map.points.iter().enumerate() {
            let pos: FxVec2 = ps.pos.into();
            if known
                .iter()
                .any(|k| k.at && (k.pos - pos).length_sq_raw() <= r)
            {
                self.ai.at_seen.insert(i as u8, tick);
            }
        }
    }

    /// Whether a seen enemy (soldier or vehicle) is within `r` of `at`.
    fn enemy_near(&self, known: &[Known], kv: &[KnownVehicle], at: FxVec2, r: Fx) -> bool {
        let r = sq_m(r);
        known.iter().any(|k| (k.pos - at).length_sq_raw() <= r)
            || kv.iter().any(|v| (v.pos - at).length_sq_raw() <= r)
    }

    /// Sends the squads of escort `e` still following vehicle `v` at its
    /// point (attack-move), as on a halt.
    fn release(&mut self, e: &AiEscort, v: SimId, busy: &[SquadId]) {
        let g = self.g;
        let Some(ps) = g.setup.map.points.get(e.point as usize) else {
            return;
        };
        let goal: FxVec2 = ps.pos.into();
        for (k, &q) in e.squads.iter().enumerate() {
            let follows = g
                .squads
                .get(&q)
                .is_some_and(|sq| sq.order == (Order::Follow { vehicle: v }));
            if !follows || busy.contains(&q) {
                continue;
            }
            let target = self.terrain.clamp(ring_slot(goal, k as u32));
            self.ai
                .emit(self.due, CommandKind::AttackMove { squad: q, target });
            self.task(q, TaskKind::Attack, e.point, target);
        }
    }

    /// Retreat and field repair (module docs). Vehicles given an order
    /// here, retreating or repairing leave `tanks`.
    fn vehicle_upkeep(&mut self, tanks: &mut Vec<MyTank>, known: &[Known], kv: &[KnownVehicle]) {
        let g = self.g;
        let s = &g.setup;
        let base = &s.map.bases[s.players[self.me.0 as usize].base as usize];
        let base_at: FxVec2 = base.center.into();
        let base_r = sq_m(base.radius.into());
        let mut keep = Vec::with_capacity(tanks.len());
        for t in tanks.drain(..) {
            let sq = &g.squads[&t.squad];
            let vs = &g.vehicles[&t.id];
            let v = &s.vehicles[vs.type_idx as usize];
            if sq.order == Order::Retreat {
                if let Some(e) = self.ai.escorts.remove(&t.squad) {
                    self.release(&e, t.id, &[]);
                }
                continue;
            }
            let engine_max = v
                .components
                .iter()
                .find(|c| c.kind == ComponentKind::Engine)
                .map_or(0, |c| c.hp);
            let dead = vs.seats.iter().filter(|st| st.hp == 0).count();
            let beaten = vs.component(ComponentKind::Gun) == 0
                || dead >= 2
                || (vs.component(ComponentKind::Engine) as u32) * 100
                    < engine_max * self.g.setup.rules.ai.armour.retreat_engine_pct;
            if beaten && !vs.immobile(v) {
                if let Some(e) = self.ai.escorts.remove(&t.squad) {
                    self.release(&e, t.id, &[]);
                }
                if (t.pos - base_at).length_sq_raw() > base_r {
                    self.ai
                        .emit(self.due, CommandKind::Retreat { squad: t.squad });
                    self.ai.counters.retreats += 1;
                }
                // Out, or waiting in base for the base repair.
                self.task(t.squad, TaskKind::Retreat, NO_POINT, base_at);
                continue;
            }
            if vs.repairing.is_some() {
                continue;
            }
            // S3c (B1): rearm trips.
            if self.rearm(&t, kv) {
                continue;
            }
            if vs.immobilised(v)
                && crate::damage::can_field_repair(vs, v)
                && !self.enemy_near(known, kv, t.pos, s.rules.ai.repair_clear.into())
            {
                self.ai
                    .emit(self.due, CommandKind::RepairUnits { units: vec![t.id] });
                self.ai.counters.repairs += 1;
                continue;
            }
            keep.push(t);
        }
        *tanks = keep;
    }

    /// The point a vehicle goes for: the non-own point with the most
    /// infantry assigned to it (then priority), else `None`.
    fn vehicle_point(&self, points: &[PointEval], assigned: &[(usize, usize)]) -> Option<usize> {
        (0..points.len())
            .filter(|&i| points[i].rel != Rel::Own)
            .max_by_key(|&i| {
                let n = assigned.iter().filter(|a| a.1 == i).count();
                (n, points[i].priority, -(i as i64))
            })
    }

    /// The first ground dug at least `ai.infantry.trench_probe_dm` on the approach
    /// from `from` to point `at`, up to `ai.infantry.trench_probe_m` in front of it.
    fn enemy_trench(&self, from: FxVec2, at: FxVec2) -> Option<FxVec2> {
        let dir = (at - from).normalize_or_zero();
        if dir == FxVec2::ZERO {
            return None;
        }
        let probe = &self.g.setup.rules.ai.infantry;
        let mut d = probe.trench_probe_m as i32;
        while d >= -(probe.trench_probe_step_m as i32) {
            let p = self.terrain.clamp(at - dir * Fx::from_int(d));
            let (gx, gy) = self.terrain.nearest_grid(p);
            if self.terrain.dug_dm(gx, gy) as u32 >= probe.trench_probe_dm {
                return Some(p);
            }
            d -= probe.trench_probe_step_m as i32;
        }
        None
    }

    /// Where a vehicle at `from` goes against point `i`, and how.
    fn assault_goal(&self, from: FxVec2, i: usize, points: &[PointEval]) -> (TaskKind, FxVec2) {
        let ai = &self.g.setup.rules.ai;
        let at = points[i].pos;
        let dir = (at - from).normalize_or_zero();
        if self.ai.at_seen.contains_key(&(i as u8)) {
            let goal = at - dir * Fx::from(ai.tank_standoff);
            return (TaskKind::Standoff, self.terrain.clamp(goal));
        }
        let front = self.enemy_trench(from, at).unwrap_or(at);
        let goal = front - dir * Fx::from(ai.assault_close);
        (TaskKind::Assault, self.terrain.clamp(goal))
    }

    /// Escorts, assaults, back-up and facing for the free vehicles
    /// (module docs). Returns the squads escorting a vehicle now.
    #[allow(clippy::too_many_arguments)]
    fn vehicle_tasks(
        &mut self,
        tanks: &[MyTank],
        mine: &[Mine],
        assigned: &[(usize, usize)],
        points: &[PointEval],
        known: &[Known],
        kv: &[KnownVehicle],
        busy: &[SquadId],
        hunting: &[SquadId],
    ) -> Vec<SquadId> {
        let g = self.g;
        let hz = g.setup.tick_hz.max(1) as u64;
        let target_point = self.vehicle_point(points, assigned);
        // Escort upkeep: drop gone vehicles and squads, release on a halt,
        // count arrivals; a released escort ends once the infantry goes
        // for another point.
        let ids: Vec<SquadId> = self.ai.escorts.keys().copied().collect();
        for tid in ids {
            let mut e = self.ai.escorts.remove(&tid).expect("listed");
            let Some(tsq) = g.squads.get(&tid) else {
                continue;
            };
            let Some(&v) = tsq.members.first() else {
                continue;
            };
            let Some(vs) = g.vehicles.get(&v) else {
                continue;
            };
            let point = e.point as usize;
            if point >= points.len() || points[point].rel == Rel::Own {
                self.release(&e, v, busy);
                continue;
            }
            let follows = |id: &SquadId| {
                g.squads
                    .get(id)
                    .is_some_and(|q| q.order == (Order::Follow { vehicle: v }))
            };
            e.squads.retain(follows);
            if e.squads.is_empty() && target_point.is_some_and(|t| t != point) {
                continue;
            }
            let vdef = &g.setup.vehicles[vs.type_idx as usize];
            let halted = tsq.order == Order::Idle
                || vs.immobile(vdef)
                || (vs.speed.abs()
                    < Fx::from_milli(self.g.setup.rules.ai.armour.halted_mm_s as i32)
                    && tsq.nav.phase != NavPhase::PathPending);
            if !e.squads.is_empty()
                && halted
                && self.tick >= e.since + self.g.setup.rules.ai.armour.escort_grace_s as u64 * hz
            {
                self.release(&e, v, busy);
                e.squads.clear();
                self.ai.counters.escorts_released += 1;
            }
            if !e.reached {
                let vpos = self.ents.transform[v.index()].pos.xy();
                let front = self
                    .enemy_trench(vpos, points[point].pos)
                    .unwrap_or(points[point].pos);
                let r = sq_m(Fx::from_int(
                    self.g.setup.rules.ai.armour.escort_reached_m as i32,
                ));
                let close = e.started.iter().any(|q| {
                    g.squads.get(q).is_some_and(|sq| {
                        !sq.members.is_empty()
                            && (self.ents.transform[sq.lead(self.ents).index()].pos.xy() - front)
                                .length_sq_raw()
                                <= r
                    })
                });
                if close {
                    e.reached = true;
                    self.ai.counters.escorts_reached += 1;
                }
            }
            self.ai.escorts.insert(tid, e);
        }

        let mut taken: Vec<SquadId> = self
            .ai
            .escorts
            .values()
            .flat_map(|e| e.squads.iter().copied())
            .collect();
        for t in tanks {
            let sq = &g.squads[&t.squad];
            let vs = &g.vehicles[&t.id];
            let mut ordered = false;
            let point = match self.ai.escorts.get(&t.squad) {
                Some(e) => Some(e.point as usize),
                None => target_point,
            };
            if !t.immobile {
                let (kind, goal, pi) = match point {
                    Some(i) => {
                        let (kind, fresh) = self.assault_goal(t.pos, i, points);
                        // Keep the goal while the plan for the point holds,
                        // so the vehicle is not re-pathed every plan.
                        let goal = match self.ai.tasks.get(&t.squad) {
                            Some(old) if old.kind == kind && old.point == i as u8 => old.target,
                            _ => fresh,
                        };
                        (kind, goal, i as u8)
                    }
                    None => {
                        // Back up the most threatened own point.
                        let Some(i) = (0..points.len())
                            .filter(|&i| points[i].rel == Rel::Own)
                            .max_by_key(|&i| {
                                (points[i].armour, points[i].threat, points[i].priority)
                            })
                        else {
                            continue;
                        };
                        let at = points[i].pos;
                        let back = (g.base_of(self.me) - at).normalize_or_zero();
                        let goal = self.terrain.clamp(
                            at + back * Fx::from_int(self.g.setup.rules.ai.armour.backup_m as i32),
                        );
                        (TaskKind::Defend, goal, i as u8)
                    }
                };
                let near = |a: FxVec2, r: i32| (a - goal).length_sq_raw() <= sq_m(Fx::from_int(r));
                let on_track = match &sq.order {
                    Order::Idle => near(t.pos, 20),
                    Order::Move { target } | Order::AttackMove { target } => near(*target, 15),
                    _ => false,
                };
                if !on_track {
                    self.ai.emit(
                        self.due,
                        CommandKind::AttackMove {
                            squad: t.squad,
                            target: goal,
                        },
                    );
                    ordered = true;
                }
                self.task(t.squad, kind, pi, goal);
                // A new escort from the infantry sent to that point.
                if kind != TaskKind::Defend
                    && !self.ai.escorts.contains_key(&t.squad)
                    && self.ai.escorts.len() < MAX_ESCORTS
                {
                    let join = sq_m(Fx::from_int(
                        self.g.setup.rules.ai.armour.escort_join_m as i32,
                    ));
                    let mut cands: Vec<(u64, SquadId)> = assigned
                        .iter()
                        .filter(|a| a.1 == pi as usize)
                        .map(|a| &mine[a.0])
                        .filter(|m| {
                            matches!(m.role, AiKind::Rifle | AiKind::Engineer)
                                && m.strength_pct >= 60
                                && !taken.contains(&m.id)
                                && !hunting.contains(&m.id)
                                && (m.lead - t.pos).length_sq_raw() <= join
                        })
                        .map(|m| ((m.lead - t.pos).length_sq_raw(), m.id))
                        .collect();
                    cands.sort_unstable();
                    cands.truncate(self.g.setup.rules.ai.armour.escort_squads as usize);
                    if !cands.is_empty() {
                        let mut squads: Vec<SquadId> = cands.iter().map(|c| c.1).collect();
                        squads.sort_unstable();
                        for &q in &squads {
                            let mut units = g.squads[&q].members.clone();
                            units.truncate(crate::unit::MAX_UNITS);
                            self.ai.emit(
                                self.due,
                                CommandKind::FollowUnits {
                                    units,
                                    vehicle: t.id,
                                },
                            );
                            taken.push(q);
                        }
                        self.ai.escorts.insert(
                            t.squad,
                            AiEscort {
                                point: pi,
                                squads: squads.clone(),
                                started: squads,
                                since: self.tick,
                                reached: false,
                            },
                        );
                        self.ai.counters.escorts_started += 1;
                    }
                }
            }
            // S3b: halted at its goal, it suppresses the line in front of
            // the point (`ai::assault`), and is then not turned.
            if !ordered
                && sq.order == Order::Idle
                && matches!(
                    kind_of(&self.ai.tasks, t.squad),
                    Some(TaskKind::Assault | TaskKind::Standoff)
                )
                && let Some(i) = point
                && self.tank_suppress(t, i, points, known, mine)
            {
                continue;
            }
            // S3c (B5): denial fire at an enemy abandoned vehicle.
            if !ordered && self.deny(t, kv) {
                continue;
            }
            // An idle vehicle turns to the nearest threat it knows of.
            if ordered || t.immobile || sq.order != Order::Idle || vs.suppress.is_some() {
                continue;
            }
            let range = sq_m(Fx::from_int(
                self.g.setup.rules.ai.armour.face_range_m as i32,
            ));
            let threat = kv
                .iter()
                .map(|k| (k.pos, k.id))
                .chain(known.iter().filter(|k| k.at).map(|k| (k.pos, k.id)))
                .filter(|(p, _)| (*p - t.pos).length_sq_raw() <= range)
                .min_by_key(|(p, id)| ((*p - t.pos).length_sq_raw(), *id));
            let Some((at, _)) = threat else { continue };
            let d = at - t.pos;
            let off = (Fx::atan2(d.y, d.x) - t.yaw).wrap_angle().abs();
            let slack = Fx::from_int(self.g.setup.rules.ai.armour.face_slack_deg as i32)
                .mul(Fx::PI)
                .div_int(180);
            let same = vs
                .face
                .is_some_and(|f| (f - at).length_sq_raw() <= sq_m(Fx::from_int(20)));
            if off > slack && !same {
                self.ai.emit(
                    self.due,
                    CommandKind::FaceUnits {
                        units: vec![t.id],
                        toward: at,
                    },
                );
                self.ai.counters.faces += 1;
            }
        }
        taken.sort_unstable();
        taken.dedup();
        taken
    }

    /// Engineers: a satchel beside a slow enemy vehicle (module docs).
    fn satchel_vehicle(&self, sq: &crate::game::Squad, kv: &[KnownVehicle]) -> Option<ThrowPick> {
        let g = self.g;
        let ty = &g.setup.squad_types[sq.type_idx as usize];
        if ai_kind(&g.setup, sq.type_idx) != AiKind::Engineer || kv.is_empty() {
            return None;
        }
        let (_, it) = ty.kit(&g.setup.items, ItemKind::Satchel)?;
        let fuse = Fx::from_int(it.fuse_ticks as i32).div_int(g.setup.tick_hz.max(1) as i32);
        let lead = self.ents.transform[sq.lead(self.ents).index()].pos.xy();
        let reach = sq_m(Fx::from(it.range_max) + Fx::from_int(10));
        kv.iter().find_map(|v| {
            if v.speed.abs()
                > Fx::from_milli(self.g.setup.rules.ai.infantry.satchel_slow_mm_s as i32)
                || (v.pos - lead).length_sq_raw() > reach
            {
                return None;
            }
            let geo = &g.vderived.geo[v.type_idx as usize];
            let at = v.pos + v.vel * fuse;
            let left = crate::vehicle::yaw_dir(v.yaw).perp();
            let side = if (lead - at).dot(left) >= Fx::ZERO {
                Fx::ONE
            } else {
                -Fx::ONE
            };
            let off = geo.hull_half.y
                + Fx::from_milli(self.g.setup.rules.ai.infantry.satchel_side_mm as i32);
            let target = self.terrain.clamp(at + left * (side * off));
            let men = self.throwers(sq, ItemKind::Satchel, target);
            (!men.is_empty()).then_some((ItemKind::Satchel, men, target))
        })
    }
}

/// A throw the planner wants: item, the men who could throw it, target.
type ThrowPick = (ItemKind, Vec<SimId>, FxVec2);

impl Planner<'_> {
    // ----------------------------------------------------------- throws --

    /// Satchel, frag and smoke throws, then regrouping (module docs).
    fn throws(&mut self, mine: &[Mine], known: &[Known], kv: &[KnownVehicle]) {
        let g = self.g;
        let cool = g.setup.rules.ai.throw_cooldown_ticks as u64;
        let tick = self.tick;
        self.ai
            .last_throw
            .retain(|id, t| g.squads.contains_key(id) && tick < t.saturating_add(cool));
        for m in mine {
            let sq = &g.squads[&m.id];
            if sq.order == Order::Retreat || self.ai.last_throw.contains_key(&m.id) {
                continue;
            }
            let at_vehicle = self.satchel_vehicle(sq, kv);
            let hunting = self.ai.hunts.contains_key(&m.id);
            if at_vehicle.is_some() {
                self.ai.counters.satchels_at_vehicles += 1;
                self.ai.counters.hunt_throws += u32::from(hunting);
            }
            // S3c (B2): a hunting squad keeps its satchel for the vehicle.
            let pick = at_vehicle
                .or_else(|| (!hunting).then(|| self.satchel(sq, known)).flatten())
                .or_else(|| self.frag(sq, known))
                .or_else(|| self.smoke(m.id, sq, known));
            let Some((item, units, target)) = pick else {
                continue;
            };
            self.ai.emit(
                self.due,
                CommandKind::Throw {
                    units,
                    item,
                    target,
                },
            );
            self.ai.last_throw.insert(m.id, tick);
        }
        // Throwers stay detached where they stood; take them along again
        // once their squad walks on.
        for m in mine {
            let sq = &g.squads[&m.id];
            let unit = |id: &SimId| &self.ents.unit[id.index()];
            let kit = |id: &SimId| &self.ents.kit[id.index()];
            let detached = sq.members.iter().filter(|id| unit(id).detached);
            let pending = self
                .ai
                .pending
                .iter()
                .any(|c| matches!(c.kind, CommandKind::Regroup { squad } if squad == m.id));
            if !pending
                && matches!(sq.nav.phase, NavPhase::Moving | NavPhase::PathPending)
                && detached.clone().next().is_some()
                && detached.clone().all(|id| kit(id).windup.is_none())
            {
                self.ai.emit(self.due, CommandKind::Regroup { squad: m.id });
            }
        }
    }

    /// Men of `sq` who may throw a `kind` item at `target` now.
    fn throwers(&self, sq: &crate::game::Squad, kind: ItemKind, target: FxVec2) -> Vec<SimId> {
        let mut out: Vec<SimId> = sq
            .members
            .iter()
            .copied()
            .filter(|&id| self.g.can_throw(self.ents, id, kind, target).is_some())
            .collect();
        out.sort_unstable();
        out
    }

    /// Engineers: a satchel at a spotted enemy in a trench in range.
    fn satchel(&self, sq: &crate::game::Squad, known: &[Known]) -> Option<ThrowPick> {
        if ai_kind(&self.g.setup, sq.type_idx) != AiKind::Engineer {
            return None;
        }
        known.iter().filter(|k| k.in_trench).find_map(|k| {
            let men = self.throwers(sq, ItemKind::Satchel, k.pos);
            (!men.is_empty()).then_some((ItemKind::Satchel, men, k.pos))
        })
    }

    /// A frag at the best spotted target in range (module docs).
    fn frag(&self, sq: &crate::game::Squad, known: &[Known]) -> Option<ThrowPick> {
        let g = self.g;
        let ty = &g.setup.squad_types[sq.type_idx as usize];
        let (_, it) = ty.kit(&g.setup.items, ItemKind::Frag)?;
        let lead = self.ents.transform[sq.lead(self.ents).index()].pos.xy();
        // Only enemies a squad of this size could reach are worth a look.
        let reach = sq_m(Fx::from(it.range_max) + Fx::from_int(40));
        let near: Vec<&Known> = known
            .iter()
            .filter(|k| (k.pos - lead).length_sq_raw() <= reach)
            .collect();
        let group = sq_m(Fx::from_int(
            self.g.setup.rules.ai.infantry.frag_group_m as i32,
        ));
        let need = g.setup.rules.ai.frag_group_men as i64;
        // Bigger groups, then trenches, then the nearest, then by id.
        let mut best: Option<((i64, bool, i64, SimId), ThrowPick)> = None;
        for k in &near {
            let men = near
                .iter()
                .filter(|o| (o.pos - k.pos).length_sq_raw() <= group)
                .count() as i64;
            if !k.in_trench && men < need {
                continue;
            }
            let key = (
                men,
                k.in_trench,
                -dist_m(k.pos, lead),
                SimId(u32::MAX - k.id.0),
            );
            if best.as_ref().is_some_and(|(b, _)| *b >= key) {
                continue;
            }
            let throwers = self.throwers(sq, ItemKind::Frag, k.pos);
            if !throwers.is_empty() {
                best = Some((key, (ItemKind::Frag, throwers, k.pos)));
            }
        }
        best.map(|(_, pick)| pick)
    }

    /// Smoke for a squad pinned on the move (module docs); a hunting squad
    /// (S3c) throws it halfway toward its vehicle.
    fn smoke(&self, sq_id: SquadId, sq: &crate::game::Squad, known: &[Known]) -> Option<ThrowPick> {
        if !matches!(sq.order, Order::Move { .. } | Order::AttackMove { .. }) {
            return None;
        }
        let pinned = sq
            .members
            .iter()
            .filter(|id| self.ents.combat[id.index()].pinned)
            .count();
        if pinned == 0 || pinned * 3 < sq.members.len() {
            return None;
        }
        let g = self.g;
        let ty = &g.setup.squad_types[sq.type_idx as usize];
        let (_, it) = ty.kit(&g.setup.items, ItemKind::Smoke)?;
        let max = Fx::from(it.range_max) - Fx::ONE;
        // The carriers: men with smoke who are not pinned.
        sq.members.iter().find_map(|&id| {
            let k = &self.ents.kit[id.index()];
            if k.items[ItemKind::Smoke.index()] == 0 || self.ents.combat[id.index()].pinned {
                return None;
            }
            let from = self.ents.transform[id.index()].pos.xy();
            // S3c (B2): a hunting squad screens itself from its vehicle.
            let toward = match self.ai.hunts.get(&sq_id) {
                Some(h) => h.last_pos,
                None => {
                    known
                        .iter()
                        .min_by_key(|k| ((k.pos - from).length_sq_raw(), k.id))?
                        .pos
                }
            };
            let d = (toward - from).length();
            let dir = (toward - from).normalize_or_zero();
            let at = self.terrain.clamp(from + dir * d.div_int(2).min(max));
            let men = self.throwers(sq, ItemKind::Smoke, at);
            (!men.is_empty()).then_some((ItemKind::Smoke, men, at))
        })
    }
}

/// The kind of squad `id`'s task, if any.
fn kind_of(tasks: &BTreeMap<SquadId, Task>, id: SquadId) -> Option<TaskKind> {
    tasks.get(&id).map(|t| t.kind)
}

/// Spot `k` around a point: the point itself, then a ring of 8 m.
fn ring_slot(at: FxVec2, k: u32) -> FxVec2 {
    if k == 0 {
        return at;
    }
    // k · 2.1 rad keeps successive squads well apart on the ring.
    let (s, c) = Fx::from_ratio(k as i64 * 2100, 1000).wrap_angle().sin_cos();
    at + FxVec2::new(c, s) * Fx::from_int(8)
}
