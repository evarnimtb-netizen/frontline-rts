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
//! 5. **Mortars.** A mortar squad fires a mission of `barrage_rounds` (S5b:
//!    `[ai.mortar] mission_rounds`) at the best spotted
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
mod breach;
pub(crate) mod fort;
mod guns;
mod mortar;
pub(crate) mod obstacles;
mod overwatch;
mod supply;
mod transport;
mod truck;
pub use ammo::{AMBUSH_END_NAMES, AMBUSH_ENDS};
pub use assault::{
    END_FAST, END_GONE, END_LEASH, END_RETREAT, END_SPENT, END_TIMEOUT, END_UNSEEN, HUNT_END_NAMES,
    HUNT_ENDS, MAX_HUNTS,
};
pub use mortar::{AiSpotter, MAX_SPOTTERS, MODE_CHECKS, PLAN_RAYS};
pub use transport::{
    AiRideLock, AiRideMeas, AiTransport, MAX_RIDE_MEAS, MAX_TRANSPORTS, TransportPhase,
};

pub use breach::{
    AiBreach, BreachEnd, BreachPhase, MAX_BREACH_TRIES, MAX_BREACHES, MAX_WIRE_KNOWN,
};
pub use fort::{AiFortJob, BUNKER_WHY, FortPhase, MAX_FORT_JOBS, MAX_FORT_TRIES};
pub use guns::{AiGun, GunPhase, MAX_GUNS, MAX_RECREW_TRIES};
pub use obstacles::{
    AiDemolish, AiHabit, AiRefusal, AiSweep, DemolishPhase, DriveArgs, HabitEnd, JobKind,
    MAX_DEMOLITIONS, MAX_REFUSED, MAX_SWEEPS, REFUSAL_KEEP_MIN_S, SweepPhase,
};
pub use truck::{AiTruck, GAVE_UP_GOALS, MAX_TRUCKS, TruckPhase};

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
    /// S5b: was `Barrage` (same number).
    FireMission = 4,
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
    /// S5b: mortar squads' spotters, by squad (at most [`MAX_SPOTTERS`];
    /// `ai::mortar`).
    pub spotters: BTreeMap<SquadId, AiSpotter>,
    /// S7: assault guns' and tank destroyers' overwatch spots, by the
    /// vehicle's squad (at most [`MAX_OVERWATCH`]; `ai::overwatch`).
    pub overwatch: BTreeMap<SquadId, AiOverwatch>,
    /// S8a: half-track jobs, by the vehicle's squad (at most
    /// [`transport::MAX_TRANSPORTS`]; `ai::transport`).
    pub transports: BTreeMap<SquadId, AiTransport>,
    /// S8a Fix (review M2): open ride measurements, by the riding squad
    /// (at most [`transport::MAX_RIDE_MEAS`]; report only).
    pub ride_meas: BTreeMap<SquadId, AiRideMeas>,
    /// S8b: squads that got off keep going to their point (at most
    /// [`transport::MAX_RIDE_MEAS`]; `ai::transport`).
    pub ride_locks: BTreeMap<SquadId, AiRideLock>,
    /// S8b: truck jobs, by the vehicle's squad (at most
    /// [`truck::MAX_TRUCKS`]; `ai::truck`).
    pub trucks: BTreeMap<SquadId, AiTruck>,
    /// S9: gun jobs, by the gun's squad (at most [`guns::MAX_GUNS`]).
    pub guns: BTreeMap<SquadId, AiGun>,
    /// S9: guns infantry was sent to serve, and when (at most
    /// [`guns::MAX_RECREW_TRIES`]).
    pub gun_recrews: BTreeMap<SimId, u64>,
    /// S10a-ai (`ai::fort`): fort jobs, by the builder squad (at most
    /// [`MAX_FORT_JOBS`]).
    pub forts: BTreeMap<SquadId, AiFortJob>,
    /// S10a-ai (`ai::breach`): breach jobs, by the breach squad (at most
    /// [`MAX_BREACHES`]).
    pub breaches: BTreeMap<SquadId, AiBreach>,
    /// S10a-ai: enemy wire sections known (ascending ids, at most
    /// [`MAX_WIRE_KNOWN`]) and the next section the sight check reads.
    pub wire_known: Vec<u32>,
    pub wire_cursor: u32,
    /// S10a-ai: tick each held point was last marked shelled (by point).
    pub shelled: BTreeMap<u8, u64>,
    /// S10a-ai: a known enemy soldier was in a `fire_mission` squad.
    pub enemy_mortar_seen: bool,
    /// S10a-ai: tick of each point's last breach path search (by point).
    pub bypass: BTreeMap<u8, u64>,
    /// S10a-ai: the material run under way (one per player): the carrying
    /// squad (truck or bearers) and the work it serves.
    pub material_run: Option<(SquadId, u32)>,
    /// S10b-ai (`ai::obstacles`): sweep jobs, by the sweeper squad (at most
    /// [`MAX_SWEEPS`]).
    pub sweeps: BTreeMap<SquadId, AiSweep>,
    /// S10b-ai: demolition jobs, by the demolishing squad (at most
    /// [`MAX_DEMOLITIONS`]).
    pub demolitions: BTreeMap<SquadId, AiDemolish>,
    /// S10b-ai (L3): goals that failed, sorted by (kind, failed, x, y), at
    /// most [`MAX_REFUSED`].
    pub refused: Vec<AiRefusal>,
    /// S10b-ai (R9): the next `FortState::blasts` entry the suspect check
    /// reads (clamped to the list's length on read).
    pub suspects_seen: u32,
    /// S10b-ai `own_blocked`: the (vehicle, own blocking piece) pairs
    /// stuck at the last plan, ascending; a pair counts when it first
    /// appears (derived, not saved: spec Build C item 3).
    #[serde(skip)]
    pub blocked_pairs: Vec<(SquadId, u32)>,
}

/// S7: most overwatch entries one AI keeps.
pub const MAX_OVERWATCH: usize = 32;

/// S7: an assault gun's or tank destroyer's overwatch (`ai::overwatch`).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AiOverwatch {
    /// The anchor it was chosen around, the spot, and the threat it faces.
    pub anchor: FxVec2,
    pub spot: FxVec2,
    pub axis: FxVec2,
    /// Main-gun rounds aboard when it reached the spot (shots fired from
    /// it are counted against this).
    pub rounds: u16,
    /// Tick the spot was chosen; whether the vehicle has reached it.
    pub since: u64,
    pub held: bool,
    /// S8b, recon: the tick of its last back-off order.
    pub backed_at: Option<u64>,
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
    /// S5b fire missions (`ai::mortar`): missions ordered by the mode the
    /// AI expected (`fire_mission::mode`), spotters sent, missions observed
    /// by a spotter, spotters called back after `spot_max_s` with nothing
    /// seen and after `spot_hold_s` with nothing new, map fire held by
    /// `map_fire_min_pct` or the reserve, and targets passed over because
    /// the knowledge rule would refuse them (blind fire, never ordered).
    pub fm_missions: [u32; mortar::FM_MODES],
    pub fm_spotters: u32,
    pub fm_spotter_missions: u32,
    pub fm_spotter_timeouts: u32,
    pub fm_spotter_recalls: u32,
    pub fm_map_holds: u32,
    pub fm_blind_holds: u32,
    /// S5b: map fire held while the mortar walks to its support spot.
    pub fm_walk_holds: u32,
    /// S5b: mortar squads that waited a plan because the plan's ray budget
    /// (`ai::mortar::PLAN_RAYS`) was spent.
    pub fm_ray_holds: u32,
    /// S6b (`ai::mortar`): map fire held because its start error exceeds
    /// `ai.mortar.map_fire_max_miss_m`, and fire missions withdrawn
    /// because the squad was dry when they came due.
    pub fm_miss_holds: u32,
    pub fm_dry_drops: u32,
    /// S6 (`ai::supply`): HQ and bearer squads bought, HQ build orders,
    /// sight rays cast siting them and enemy eyes on the spots chosen,
    /// crates asked for at base, crate runs (and those to stranded tanks),
    /// dry mortars sent to the HQ.
    pub s6_buys: u32,
    pub s6_hq_orders: u32,
    pub s6_hq_rays: u32,
    pub s6_hq_seen: u32,
    pub s6_crates_asked: u32,
    pub s6_crate_runs: u32,
    pub tank_crate_runs: u32,
    pub s6_dry_walks: u32,
    /// S6 review F2: stranded shells crates (no own mortar gunner or tank
    /// within `crate_feed_m`, outside the base and every radius) that
    /// bearers went to pick up again.
    pub s6_crate_pickups: u32,
    /// S7: overwatch spots reached and moves to a new spot after firing or
    /// a penetration; throws ordered into enemy open-topped vehicles; open-
    /// topped vehicles reversed away from enemy infantry.
    pub overwatch_holds: u32,
    pub relocations: u32,
    pub open_top_throws: u32,
    pub open_top_backoffs: u32,
    /// S8a (`ai::transport`): squads told to board, rides started, unloads
    /// ordered (and of those under fire), failed transport moves.
    pub transport_loads: u32,
    pub transport_rides: u32,
    pub transport_unloads: u32,
    pub transport_unloads_fire: u32,
    pub transport_fails: u32,
    /// S8a Fix (review M2): transported squads that reached the point they
    /// rode for; ticks from the `Embark` order to arrival, and the ticks a
    /// straight walk from the order position would have taken; squads lost
    /// (gone or timed out) after boarding.
    pub obj_rides: u32,
    pub obj_ride_ticks: u64,
    pub obj_walk_ticks: u64,
    pub obj_lost: u32,
    /// S8b: ticks from the order to the last man aboard and from getting
    /// off to the point (the same rides as `obj_rides`); why unloads were
    /// ordered; plans a ride lock held a squad.
    pub obj_board_ticks: u64,
    pub obj_foot_ticks: u64,
    pub unload_exposed: u32,
    pub unload_arrived: u32,
    pub unload_stuck: u32,
    pub unload_fire: u32,
    pub ride_locks: u32,
    /// S8b: recon cars stopped at a spot; backing off from an AT threat;
    /// a spot given up; truck moves failed, fleeing plans, tanks served,
    /// services ended with rounds gained, drops ordered.
    pub recon_stops: u32,
    pub recon_backoffs: u32,
    pub recon_fails: u32,
    pub truck_fails: u32,
    pub truck_flees: u32,
    pub truck_serves: u32,
    pub stranded_served: u32,
    pub truck_drops_ordered: u32,
    /// S9 (`ai::guns`): guns taken off the tow at a site; of those emplaced,
    /// in cover (woods, hull-down or a pit); gun orders failed or given up;
    /// pits ordered; trail re-lays ordered; infantry sent to serve a gun.
    pub gun_unhitches: u32,
    pub guns_in_cover: u32,
    pub gun_ai_fails: u32,
    pub gun_pits: u32,
    pub gun_relays: u32,
    pub gun_recrew_orders: u32,
    /// S9 Fix: guns packed to move to another held point.
    pub gun_resites: u32,
    /// S10a-ai (`ai::fort`): fort jobs started, `Fortify` orders refused,
    /// plan-squads on a fort job and all plan-squads given infantry tasks
    /// (`builder_share_pct` = the first over the second), shelter moves,
    /// material runs by kind (index: truck, crate, walk).
    pub fort_jobs: u32,
    pub fort_refused: u32,
    pub fort_build_plans: u32,
    pub infantry_plans: u32,
    pub shelter_moves: u32,
    pub material_runs: [u32; 3],
    /// S10a-ai (`ai::breach`): breach jobs started, gaps opened, jobs
    /// given up, smoke thrown and suppression ordered for a breach, and
    /// attacks whose path went round known wire.
    pub breaches_started: u32,
    pub breaches_opened: u32,
    pub breaches_given_up: u32,
    pub breach_smokes: u32,
    pub breach_suppress: u32,
    pub wire_bypassed: u32,
    /// S10b-ai (`ai::obstacles`; counted only with their switch on): S10b
    /// jobs started and refused, AI lay sites chosen, sweeps wanted,
    /// started and done, demolitions wanted and started, gaps opened and
    /// demolitions given up, obstacle sites rejected by the own route rule
    /// and own routes found blocked, jobs alive over 900 s, starts or
    /// commands within `refused_m` of a goal that failed in the last 300 s
    /// (fixed windows, not keys: the `loose` control shows them).
    pub s10b_jobs: u32,
    pub s10b_refused: u32,
    pub lay_sites: u32,
    pub sweep_wanted: u32,
    pub sweeps_started: u32,
    pub sweeps_done: u32,
    pub demolish_wanted: u32,
    pub demolitions_started: u32,
    pub gaps_opened: u32,
    pub demolish_given_up: u32,
    pub own_route_rejects: u32,
    pub own_blocked: u32,
    pub jobs_overdue: u32,
    pub refused_reorders: u32,
    /// S10b-ai Build D: per plan with the bunker switched on, under
    /// `bunkers_max` and no bunker job, the first check that kept one from
    /// starting; and how bunker jobs ended (`fort::BUNKER_WHY` order).
    pub bunker_why: [u32; 16],
}

impl AiState {
    /// A fresh AI state whose first plan runs at `first_plan` (the
    /// fortify-test breach hook keeps one outside the match).
    pub fn new(first_plan: u64) -> AiState {
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
            spotters: BTreeMap::new(),
            overwatch: BTreeMap::new(),
            transports: BTreeMap::new(),
            ride_meas: BTreeMap::new(),
            ride_locks: BTreeMap::new(),
            trucks: BTreeMap::new(),
            guns: BTreeMap::new(),
            gun_recrews: BTreeMap::new(),
            forts: BTreeMap::new(),
            breaches: BTreeMap::new(),
            wire_known: Vec::new(),
            wire_cursor: 0,
            shelled: BTreeMap::new(),
            enemy_mortar_seen: false,
            bypass: BTreeMap::new(),
            material_run: None,
            sweeps: BTreeMap::new(),
            demolitions: BTreeMap::new(),
            refused: Vec::new(),
            suspects_seen: 0,
            blocked_pairs: Vec::new(),
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
        ents_len: usize,
    ) -> Result<(), &'static str> {
        let np = g.points.len();
        let bad_squad = |id: &SquadId| id.0 >= g.next_squad;
        let map_size = Fx::from_int(g.setup.map.size_m);
        let on_map =
            |p: FxVec2| p.x >= Fx::ZERO && p.y >= Fx::ZERO && p.x <= map_size && p.y <= map_size;
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
            // S5b.
            || !self.validate_spotters(g.next_squad, ents_len, tick)
            // S7.
            || self.overwatch.len() > MAX_OVERWATCH
            || self.overwatch.keys().any(bad_squad)
            || self.overwatch.values().any(|o| {
                let size = Fx::from_int(g.setup.map.size_m);
                let on = |p: FxVec2| {
                    p.x >= Fx::ZERO && p.y >= Fx::ZERO && p.x <= size && p.y <= size
                };
                o.since > tick
                    || o.backed_at.is_some_and(|t| t > tick)
                    || !on(o.anchor)
                    || !on(o.spot)
                    || !on(o.axis)
            })
            // S9.
            || self.guns.len() > guns::MAX_GUNS
            || self.gun_recrews.len() > guns::MAX_RECREW_TRIES
            || self.guns.iter().any(|(id, j)| {
                bad_squad(id)
                    || j.tow.as_ref().is_some_and(bad_squad)
                    || j.pit_by.as_ref().is_some_and(bad_squad)
                    || j.since > tick + 36_000
                    || j.moved_at > tick + 36_000
                    || !on_map(j.site)
                    || !on_map(j.facing)
                    || !on_map(j.moved_to)
                    || j.quiet_at > tick + 36_000
            })
            // S8a.
            || self.transports.len() > MAX_TRANSPORTS
            || self.transports.iter().any(|(id, j)| {
                bad_squad(id)
                    || j.squad.as_ref().is_some_and(bad_squad)
                    || j.since > tick + 36_000
                    || j.moved_at > tick
                    || j.moving_at > tick
                    || (j.pi != NO_POINT && j.pi as usize >= np)
                    || !on_map(j.point)
            })
            || self.ride_meas.len() > transport::MAX_RIDE_MEAS
            || self.ride_meas.iter().any(|(id, m)| {
                let size = Fx::from_int(g.setup.map.size_m);
                let on = |p: FxVec2| {
                    p.x >= Fx::ZERO && p.y >= Fx::ZERO && p.x <= size && p.y <= size
                };
                bad_squad(id) || m.order_tick > tick || !on(m.from) || !on(m.point)
            })
            || self.trucks.len() > truck::MAX_TRUCKS
            || self.trucks.iter().any(|(id, j)| {
                bad_squad(id)
                    || j.since > tick
                    || j.moved_at > tick
                    || j.client.is_some_and(|c| c.index() >= ents_len)
                    || j.gave_up.iter().flatten().any(|&p| !on_map(p))
            })
            || self.ride_locks.len() > transport::MAX_RIDE_MEAS
            || self.ride_locks.iter().any(|(id, l)| {
                let size = Fx::from_int(g.setup.map.size_m);
                bad_squad(id)
                    || l.point as usize >= np
                    || l.target.x < Fx::ZERO
                    || l.target.y < Fx::ZERO
                    || l.target.x > size
                    || l.target.y > size
            })
            // S10a-ai.
            || !self.validate_fort(g, player, tick)
            // S10b-ai.
            || !self.validate_s10b(g, player, tick)
            || self.pending.windows(2).any(|w| w[0].due > w[1].due)
            // Due one reaction delay after its plan at most (the save fuzz).
            || self.pending.last().is_some_and(|c| {
                g.setup.players.get(player.0 as usize).is_none_or(|ps| {
                    let d = g.setup.rules.difficulty.get(ps.difficulty);
                    c.due > tick + d.reaction_ticks.max(1) as u64
                })
            })
            || !crate::bounds::counters_ok(&self.counters)
            || !crate::bounds::counters_ok(&[self.issued, self.rejected])
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
                | CommandKind::FollowUnits { units, .. }
                | CommandKind::ReverseUnits { units, .. } => units.len() > crate::unit::MAX_UNITS,
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

impl AiState {
    /// S10a-ai: the fort and breach state of a loaded AI (spec 4): caps,
    /// sorted `wire_known`, jobs keyed by squads of `player` (a squad gone
    /// since the last plan is allowed: the next plan drops its job), points
    /// and blueprints in range, ticks not in the future, sites on the map.
    fn validate_fort(&self, g: &MatchState, player: PlayerId, tick: u64) -> bool {
        let np = g.points.len();
        let size = Fx::from_int(g.setup.map.size_m);
        let on_map = |p: &FxVec2| p.x >= Fx::ZERO && p.y >= Fx::ZERO && p.x <= size && p.y <= size;
        let own = |id: &SquadId| {
            id.0 < g.next_squad && g.squads.get(id).is_none_or(|q| q.owner == player)
        };
        let point = |p: u8| p == NO_POINT || (p as usize) < np;
        self.forts.len() <= MAX_FORT_JOBS
            && self.forts.iter().all(|(id, j)| {
                own(id)
                    && (j.fort as usize) < g.setup.forts.len()
                    && point(j.point)
                    && !j.line.is_empty()
                    && j.line.len() <= crate::fort::MAX_FORT_POINTS
                    && j.line.iter().all(on_map)
                    && j.start <= j.since
                    && j.since <= tick
                    && j.tries <= MAX_FORT_TRIES
            })
            && self.breaches.len() <= MAX_BREACHES
            && self.breaches.iter().all(|(id, b)| {
                own(id)
                    && (b.point as usize) < np
                    && on_map(&b.crossing)
                    && b.since <= tick
                    && b.tries <= MAX_BREACH_TRIES
                    && b.suppress_at.is_none_or(|t| t <= tick)
                    && b.smoke_at.is_none_or(|t| t <= tick)
            })
            && self.wire_known.len() <= MAX_WIRE_KNOWN
            && self.wire_known.windows(2).all(|w| w[0] < w[1])
            && self.shelled.len() <= np
            && self
                .shelled
                .iter()
                .all(|(&p, &t)| (p as usize) < np && t <= tick)
            && self.bypass.len() <= np
            && self
                .bypass
                .iter()
                .all(|(&p, &t)| (p as usize) < np && t <= tick)
            && self.material_run.is_none_or(|(id, _)| own(&id))
    }
}

/// The squad a command addresses, if any.
fn squad_of(kind: &CommandKind) -> Option<SquadId> {
    match kind {
        // S8a: the vehicle's squad is not named; its id stands for it.
        CommandKind::Embark { squads, .. } => squads.first().copied(),
        CommandKind::Move { squad, .. }
        | CommandKind::AttackMove { squad, .. }
        | CommandKind::Stop { squad }
        | CommandKind::Retreat { squad }
        | CommandKind::Dig { squad, .. }
        | CommandKind::FireMission { squad, .. } => Some(*squad),
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
        // S8a: the vehicles' squads.
        CommandKind::Disembark { vehicles, .. } => Some(vehicles),
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
    // S8b: recon cars and trucks have their own buys, never a tank's slot
    // (S8a's half-tracks displaced tanks there).
    let own_buy = ty
        .vehicle
        .and_then(|v| s.vehicles.get(v as usize))
        .is_some_and(|v| {
            matches!(
                v.role,
                frontline_content::VehicleRole::Recon
                    | frontline_content::VehicleRole::Truck
                    | frontline_content::VehicleRole::AtGun
                    | frontline_content::VehicleRole::Tractor
            )
        });
    if ty.has_tag(s.tag("crew")) || own_buy {
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
    /// S5b: has stood still for `ai::mortar::STILL_TICKS` (a mortar
    /// target that will still be there when the bombs land).
    still: bool,
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
#[derive(Clone)]
struct Mine {
    id: SquadId,
    role: AiKind,
    lead: FxVec2,
    strength_pct: u32,
    /// S10a-ai: the card's capability bits (`SquadTypeSetup::caps`).
    caps: u32,
}

impl Mine {
    fn can(&self, c: Capability) -> bool {
        self.caps & c.bit() != 0
    }
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
    /// S10a-ai: an own point whose finished trench still wants a work
    /// (`ai::fort`; false with `works` and `wire` off).
    wants_works: bool,
    /// S10a-ai fix F15: the builder the assign bonus holds here (one that
    /// could start the point's work now; `ai::fort::fort_holds`).
    hold: Option<SquadId>,
}

impl MatchState {
    /// Moves due AI commands (all AI players, in player order) into `out`.
    /// S6b (S5b-22): a fire mission whose squad holds no bombs when it
    /// comes due (the men carrying them fell during the reaction delay) is
    /// withdrawn (`AiCounters::fm_dry_drops`) instead of refused.
    pub(crate) fn take_ai_commands(
        &mut self,
        ents: &Entities,
        tick: u64,
        out: &mut Vec<crate::Command>,
    ) {
        let squads = &self.squads;
        let setup = &self.setup;
        for (p, ai) in self.ai.iter_mut().enumerate() {
            let Some(ai) = ai else { continue };
            let n = ai.pending.iter().take_while(|c| c.due <= tick).count();
            let mut dropped = 0u32;
            out.extend(
                ai.pending
                    .drain(..n)
                    .filter(|c| {
                        let CommandKind::FireMission { squad, .. } = &c.kind else {
                            return true;
                        };
                        let dry = setup.rules.fire_mission.is_some()
                            && squads.get(squad).is_some_and(|sq| {
                                crate::supply::mortar_bombs(setup, ents, sq).0 == 0
                            });
                        dropped += dry as u32;
                        !dry
                    })
                    .map(|c| crate::Command {
                        player: PlayerId(p as u8),
                        seq: c.seq,
                        kind: c.kind,
                    }),
            );
            ai.counters.fm_dry_drops += dropped;
        }
    }

    /// Counts a rejected AI command for `player`.
    /// S10a-ai: a refused `Fortify`, `LoadMaterials`, `CutWire` or
    /// `PlaceBangalore` also counts in `AiCounters::fort_refused` (T99).
    pub(crate) fn ai_rejected(&mut self, player: PlayerId, kind: &CommandKind) {
        if let Some(Some(ai)) = self.ai.get_mut(player.0 as usize) {
            ai.rejected += 1;
            if matches!(
                kind,
                CommandKind::Fortify { .. }
                    | CommandKind::LoadMaterials { .. }
                    | CommandKind::CutWire { .. }
                    | CommandKind::PlaceBangalore { .. }
            ) {
                ai.counters.fort_refused += 1;
            }
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
                    mortar_rays: mortar::PLAN_RAYS,
                    at_sites: Default::default(),
                    own_routes: Default::default(),
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
    /// S5b: sight-line rays the mortar planner may still cast this plan
    /// (`ai::mortar::PLAN_RAYS`).
    mortar_rays: usize,
    /// S10b-ai (R2): this plan's AT field sites by point (one vehicle path
    /// search per point per plan; derived, never saved).
    at_sites: std::cell::RefCell<Vec<(usize, Option<Vec<FxVec2>>)>>,
    /// S10b-ai (R6): this plan's own vehicle routes by point (base -> the
    /// point -> the nearest non-own point; derived, never saved).
    own_routes: std::cell::RefCell<Vec<(usize, Option<Vec<FxVec2>>)>>,
}

impl Planner<'_> {
    fn plan(&mut self) {
        let g = self.g;
        let me = self.me;
        self.ai
            .tasks
            .retain(|id, _| g.squads.get(id).is_some_and(|s| s.owner == me));
        self.ai
            .overwatch
            .retain(|id, _| g.squads.get(id).is_some_and(|s| s.owner == me));
        let mut busy: Vec<SquadId> = Vec::new();
        for c in &self.ai.pending {
            // S8a: an Embark keeps its vehicle busy too.
            if let CommandKind::Embark { vehicle, .. } = c.kind
                && let Some(vs) = self.ents.squad.get(vehicle.index()).copied().flatten()
            {
                busy.push(vs);
            }
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
            // S8a: squads walking to or riding a half-track belong to its
            // job (`ai::transport`).
            if matches!(sq.order, Order::Embark { .. }) {
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
                caps: ty.caps,
            });
        }

        // S9: guns, tows and crews are the gun jobs'.
        let guns = self.take_guns(&mut tanks, &mut mine);
        self.retreats(&mut mine, &known);
        // S10a-ai: shelled marks, then the fort jobs keep their squads.
        self.fort_marks(&known);
        self.fort_continue(&mut mine, &known, &kv, &busy);
        // S10a-ai: enemy wire by sight, then the breach jobs keep theirs.
        self.wire_sight();
        self.breach_continue(&mut mine, &busy);
        // S5a: refill trips, staggered per point.
        self.refills(&mut mine, &known);
        // S6: the HQ, crate runs, dry mortars to the HQ.
        self.supply(&mut mine, &tanks, &known);
        // S8a: half-track jobs take their vehicles out of the tank habits
        // (but support, which is the overwatch habit).
        let waiting = self.transport_jobs(&mut tanks, &known, &busy);
        self.ride_locks(&mut mine, &known);
        self.vehicle_upkeep(&mut tanks, &known, &kv);
        self.truck_jobs(&mut tanks, &known, &kv, &busy);
        self.crews(&crews, &known, &kv);
        self.buy(&kv);
        self.buy_recon_truck();
        self.buy_gun();
        let mut points = self.evaluate_points(&known, &kv);
        self.fort_points(&mut points);
        self.gun_jobs(&guns, &mut mine, &points, &known, &kv);
        self.fort_holds(&mine, &known, &kv, &mut points);
        let assigned = self.assign(&mine, &mut points);
        // S5a: Panzerfaust ambushes first; the hunts skip their squads.
        let ambushing = self.ambushes(&mine, &assigned, &points, &kv);
        let mut hunting = self.hunts(&mine, &assigned, &points, &known, &kv);
        hunting.extend(ambushing);
        hunting.sort_unstable();
        hunting.dedup();
        // S10a-ai: new fort jobs take free builders (claimed as hunts are).
        let building = self.fort_start(&mine, &assigned, &points, &known, &kv, &hunting);
        hunting.extend(building);
        // S10a-ai: new breach jobs take an attacker that can open wire.
        let breaching = self.breach_start(&mine, &assigned, &points, &hunting);
        hunting.extend(breaching);
        let escorting = self.vehicle_tasks(
            &tanks, &mine, &assigned, &points, &known, &kv, &busy, &hunting,
        );
        let mut claimed: Vec<SquadId> = hunting.iter().chain(&escorting).copied().collect();
        claimed.extend(self.fire_and_move(&mine, &assigned, &points, &known, &claimed));
        let loading = self.transport_loads(&waiting, &mine, &assigned, &points, &claimed);
        claimed.extend(loading);
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
                    still: self.ents.combat[i].still >= mortar::STILL_TICKS,
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
            // S7: `ai_weight = 0` keeps a card out of the AI's buys.
            if s.squad_types[t as usize].ai_weight == 0 {
                continue;
            }
            // S10a-ai (spec 3.1): `[ai.fort] cards = 0` skips the three
            // S10a cards as at `ai_weight` 0.
            if ai.fort.cards == 0 && fort::is_s10a_card(&s.squad_types[t as usize]) {
                continue;
            }
            if let Some(r) = mix_slot(s, t) {
                cards[r].push(t);
            }
        }
        // The slot's highest-weight card (ties: the first in the roster):
        // the one the AI saves up for.
        // S8a: a vehicle card the AI may save up for now (its unlock).
        let now = self.tick;
        let unlocked_card = |t: u16| {
            let vr = &s.rules.vehicles;
            match s.squad_types[t as usize].vehicle {
                None => true,
                Some(_) if g.is_transport_type(t) => {
                    now >= (vr.transport.unlock_s * s.tick_hz).min(vr.unlock_ticks) as u64
                }
                Some(_) => now >= vr.unlock_ticks as u64,
            }
        };
        let best = |r: usize| -> Option<u16> {
            cards[r].iter().copied().filter(|&t| unlocked_card(t)).fold(
                None,
                |b: Option<u16>, t| match b {
                    Some(b)
                        if s.squad_types[b as usize].ai_weight
                            >= s.squad_types[t as usize].ai_weight =>
                    {
                        Some(b)
                    }
                    _ => Some(t),
                },
            )
        };
        let mut counts = vec![0i64; slots];
        let mut alive_by_type = vec![0u32; s.squad_types.len()];
        for sq in g.squads.values().filter(|sq| sq.owner == self.me) {
            alive_by_type[sq.type_idx as usize] += 1;
            // S8a Fix (review M4): transports stay outside the tank share
            // of the vehicle slot (they would take the tanks' buys).
            if let Some(r) = mix_slot(s, sq.type_idx)
                && !g.is_outside_limit_type(sq.type_idx)
            {
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
        // S9: the guns' own budget (`ai::guns`) is not for other cards.
        let mut budget = pl.manpower as i64 - self.gun_pot();
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
                if let Some(r) = mix_slot(s, squad_type)
                    && !g.is_outside_limit_type(squad_type)
                {
                    counts[r] += 1;
                }
                alive_by_type[squad_type as usize] += 1;
                budget -= ty.cost as i64;
                pop += ty.pop(&s.vehicles) as i64;
                // S8a: transports are outside the vehicle limit.
                vehicles += (ty.vehicle.is_some() && !g.is_outside_limit_type(squad_type)) as u32;
                bought.push(squad_type);
            }
        }
        // A type can be bought when its cooldown is over by the time the
        // command is applied and no pending buy restarts it; S4: and its
        // card limits (`max_alive`, limit group) leave room.
        let due = self.due;
        let me = self.me;
        // S3: vehicles after the unlock and under the limit; S8a:
        // transports after their own unlock, outside the limit.
        let vr = &s.rules.vehicles;
        let unlocked = self.tick >= vr.unlock_ticks as u64;
        let transport_unlock = (vr.transport.unlock_s * s.tick_hz).min(vr.unlock_ticks) as u64;
        let transports_unlocked = self.tick >= transport_unlock;
        let vehicles_now = vehicles;
        let eligible = |t: u16| {
            let ty = &s.squad_types[t as usize];
            match ty.vehicle {
                None => true,
                Some(_) if g.is_transport_type(t) => transports_unlocked,
                Some(_) => unlocked && vehicles_now < vr.limit,
            }
        };
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
                && eligible(t)
                && due >= ty.initial_cooldown_ticks as u64
                && g.cooldown_left(me, t, due) == 0
                && !bought.iter().any(|&b| {
                    b == t || (group.is_some() && s.squad_types[b as usize].cooldown_group == group)
                })
        };
        // S3: vehicles after the unlock and under the limit (S8a: or a
        // transport after its unlock); AT teams after the unlock once an
        // enemy vehicle has been seen. Before the unlock the mix is the S1
        // one.
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
            && cards[r].iter().any(|&t| eligible(t))
        {
            mix[r] = ai.mix[r].pct;
        }
        let count_of = |counts: &[i64], r: Option<usize>| r.map_or(0, |r| counts[r]);
        // S8b: a truck in sight is no reason for an AT team.
        let armoured = kv
            .iter()
            .filter(|k| {
                s.vehicles[k.type_idx as usize].role != frontline_content::VehicleRole::Truck
            })
            .count();
        // S10a-ai (spec 3.1): a breach is wanted and no own squad can open
        // wire: the faction's assault card first (as an urgent AT team),
        // at most `breach_cards_max` of them alive.
        let mut urgent_breach = if ai.fort.cards != 0 && self.breach_wanted() {
            let is_assault = |t: u16| {
                let ty = &s.squad_types[t as usize];
                fort::is_s10a_card(ty) && ty.has(Capability::Breach)
            };
            let alive: u32 = (0..s.squad_types.len())
                .filter(|&t| is_assault(t as u16))
                .map(|t| alive_by_type[t])
                .sum();
            faction
                .roster
                .iter()
                .copied()
                .find(|&t| s.squad_types[t as usize].ai_weight > 0 && is_assault(t))
                .filter(|_| alive < ai.fort.breach_cards_max)
        } else {
            None
        };
        for _ in 0..self.g.setup.rules.ai.infantry.buys_per_plan {
            if let Some(t) = urgent_breach.take()
                && ready(t, &bought, &alive_by_type)
            {
                let ty = &s.squad_types[t as usize];
                let men = ty.pop(&s.vehicles) as i64;
                if budget >= ty.cost as i64 && pop + men <= s.rules.pop_cap as i64 {
                    self.ai
                        .emit(self.due, CommandKind::CallIn { squad_type: t });
                    budget -= ty.cost as i64;
                    pop += men;
                    bought.push(t);
                    alive_by_type[t as usize] += 1;
                    if let Some(r) = mix_slot(s, t) {
                        counts[r] += 1;
                    }
                    let c = &mut self.ai.counters;
                    if c.bought.len() < s.squad_types.len() {
                        c.bought.resize(s.squad_types.len(), 0);
                    }
                    c.bought[t as usize] += 1;
                    continue;
                }
            }
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
                && 2 * armoured as i64
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
            if !g.is_outside_limit_type(t) {
                counts[r] += 1;
            }
            let c = &mut self.ai.counters;
            if c.bought.len() < s.squad_types.len() {
                c.bought.resize(s.squad_types.len(), 0);
            }
            c.bought[t as usize] += 1;
            if Some(r) == veh_slot {
                c.vehicles_bought += 1;
                if !g.is_outside_limit_type(t) {
                    vehicles += 1;
                }
                if vehicles >= vr.limit && !cards[r].iter().any(|&x| g.is_transport_type(x)) {
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
                wants_works: false,
                hold: None,
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
                g.setup.squad_types[sq.type_idx as usize].has(Capability::FireMission)
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
                    // S10a-ai (spec 3.1): assault engineers go to attacks;
                    // the builder a point holds (fix F15: one that could
                    // start its wanted work now) stays for it.
                    (AiKind::Engineer, Rel::Own)
                        if !m.can(Capability::Breach)
                            && (!pt.trench_done || pt.hold == Some(m.id)) =>
                    {
                        80
                    }
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
        // S10a-ai (spec 3.3): one MG team into each built own nest; under
        // shelling the defender squad nearest the dugout shelters in it.
        let nests: Vec<Option<FxVec2>> = (0..points.len())
            .map(|i| {
                (points[i].rel == Rel::Own)
                    .then(|| self.nest_at(i))
                    .flatten()
            })
            .collect();
        let mut nest_taken = vec![false; points.len()];
        // S10b-ai R7: one squad defends from each built own bunker: the
        // point's MG team (lowest id), else its defender nearest the bunker;
        // the nest then takes a second MG team only.
        let bunkers: Vec<Option<(FxVec2, SquadId)>> = (0..points.len())
            .map(|i| {
                if points[i].rel != Rel::Own {
                    return None;
                }
                let free = assigned
                    .iter()
                    .filter(|&&(j, pi)| {
                        pi == i
                            && !claimed.contains(&mine[j].id)
                            && !escorting.contains(&mine[j].id)
                    })
                    .map(|&(j, _)| &mine[j]);
                self.bunker_crew(i, free)
            })
            .collect();
        let shelters: Vec<Option<(FxVec2, SquadId)>> = (0..points.len())
            .map(|i| {
                if points[i].rel != Rel::Own {
                    return None;
                }
                let at = self.shelter_at(i, known)?;
                assigned
                    .iter()
                    .filter(|&&(j, pi)| {
                        // Fix F19: never the crew of a deployed weapon.
                        pi == i
                            && !claimed.contains(&mine[j].id)
                            && !escorting.contains(&mine[j].id)
                            && !matches!(mine[j].role, AiKind::Mg | AiKind::AntiTank)
                    })
                    .map(|&(j, _)| &mine[j])
                    .min_by_key(|m| ((m.lead - at).length_sq_raw(), m.id))
                    .map(|m| (at, m.id))
            })
            .collect();
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
            // S10a-ai (spec 3.1): only squads with `dig_full` or
            // `build_structure` dig the trench.
            let wants_dig = m.role == AiKind::Engineer
                && (m.can(Capability::DigFull) || m.can(Capability::BuildStructure))
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
            // S10a-ai: a nest or dugout spot is a few metres from the
            // trench spot: the order must go there exactly.
            let mut exact = false;
            let mut shelter = false;
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
                    let target = match (nests[i], shelters[i]) {
                        (_, Some((at, sid))) if sid == m.id => {
                            shelter = true;
                            exact = true;
                            at
                        }
                        _ if bunkers[i].is_some_and(|(_, b)| b == m.id) => {
                            exact = true;
                            bunkers[i].map_or(target, |(at, _)| at)
                        }
                        (Some(at), _) if m.role == AiKind::Mg && !nest_taken[i] => {
                            nest_taken[i] = true;
                            exact = true;
                            at
                        }
                        _ => target,
                    };
                    (TaskKind::Defend, target)
                }
                Rel::Neutral => (TaskKind::Capture, ring_slot(pt.pos, k)),
                Rel::Enemy => (TaskKind::Attack, ring_slot(pt.pos, k)),
            };
            let target = self.terrain.clamp(target);
            if shelter && self.shelter_men(m.id, target) {
                self.task(m.id, kind, i as u8, target);
                continue;
            }
            let same = self
                .ai
                .tasks
                .get(&m.id)
                .is_some_and(|t| t.kind == kind && t.point == i as u8);
            let near =
                |a: FxVec2, b: FxVec2, r: i32| (a - b).length_sq_raw() <= sq_m(Fx::from_int(r));
            // S10a-ai: men sent one by one under a dugout roof come out
            // with the squad order once the shelter ends.
            let sheltered = pt.rel == Rel::Own
                && self.dugout_spot(i).is_some_and(|d| {
                    sq.members.iter().any(|u| {
                        let un = self.ents.unit.get(u.index());
                        un.is_some_and(|un| un.detached && near(un.nav.goal, d, 3))
                    })
                });
            let on_track = !sheltered
                && match &sq.order {
                    Order::Idle if exact => near(m.lead, target, 8),
                    // (Fix F6: only a built dugout; a slot-0 squad at an
                    // unfinished trench stands near the spot.)
                    Order::Idle
                        if pt.rel == Rel::Own
                            && self.dugout_built(i).is_some_and(|d| near(m.lead, d, 4)) =>
                    {
                        false
                    }
                    Order::Idle => near(m.lead, target, 20),
                    Order::Move { target: t } | Order::AttackMove { target: t } if exact => {
                        *t == target
                    }
                    // S10a-ai: back out of the dugout once the shelter ends.
                    Order::Move { target: t }
                        if pt.rel == Rel::Own && self.dugout_spot(i) == Some(*t) =>
                    {
                        false
                    }
                    Order::Move { target: t } | Order::AttackMove { target: t } => {
                        near(*t, target, 10)
                    }
                    _ => false,
                };
            if !(same && on_track) {
                self.ai.counters.shelter_moves += shelter as u32;
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
            let beaten = vs.gun_out(v)
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
            // S7: assault guns and tank destroyers hold overwatch.
            if self.overwatch(t, points, known, kv, target_point) {
                continue;
            }
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
            // S7: into a stopped open-topped vehicle first.
            if let Some((item, units, target)) = self.open_top_throw(sq, kv) {
                self.ai.emit(
                    self.due,
                    CommandKind::Throw {
                        units,
                        item,
                        target,
                    },
                );
                self.ai.counters.open_top_throws += 1;
                self.ai.last_throw.insert(m.id, tick);
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
            // S5b: a mortar's spotter stays out.
            if !pending
                && !self.ai.spotter_out(m.id)
                && !self.sheltering(m.id)
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
