//! Render snapshot for the presenter (§6 "Snapshots").
//!
//! This is the only place where sim values become `f32`. Nothing here is read
//! back by the sim. Coordinates are sim axes (x east, y north, z up, metres);
//! the Godot bridge converts to its y-up frame.
//!
//! Match worlds add per-soldier owner, squad and combat state (stance, hit
//! points, suppression, who sees him), corpses, the rounds fired and HE
//! bursts of the tick, per-squad and per-player summaries (manpower and
//! income, tickets and drain, pop, kills, losses, time holding points,
//! defeat), the capture points and the match outcome; bench worlds leave
//! those empty except the owner. S1b adds per soldier the stance order and
//! the detached flag, and for each detached man his own order and spot
//! ([`UnitSnapshot`], for order markers). Thrown items (S1b) add per
//! soldier his item counts and whether he is winding up a throw, per squad
//! the item totals (ability buttons), the items in flight or lying fused
//! ([`ThrownSnapshot`]), the smoke clouds ([`SmokeSnapshot`]) and a
//! [`BlastKind`] per explosion.
//!
//! S3 adds vehicles and wrecks ([`VehicleSnapshot`]), main-gun shells and
//! rockets in flight ([`ShellSnapshot`]), the tick's armour events
//! ([`ArmourEvent`]), and per player the vehicle count, limit and unlock
//! tick. The contract is frozen in `godot/docs/match_state.md` "Vehicles
//! (S3)"; the sim fills `vehicles` and the per-player fields from S3 sim
//! stage 2 on (shells and armour events with gunnery). Vehicle squads are
//! not in `squads` (a vehicle's squad is a squad of one; the vehicle is in
//! `vehicles`). `players[].pop` counts each live vehicle's pop.
//!
//! S3b: per player the re-call cooldown left per squad type
//! (`PlayerSnapshot::cooldowns`; no call-in queue any more), and the
//! tick's call-in results (`call_ins`: spawned squad or refusal).
//!
//! Fog of war: bit `p` of `soldier_visible` says player `p` sees that
//! soldier (own soldiers always). A presenter for local player `p` draws an
//! enemy only when his bit is set.
//!
//! Terrain: `dirty_tiles` lists the detail tiles that changed in this tick.
//! The presenter reads their heights with [`tile_heights_f32`] (or
//! `World::terrain().detail_tile(x, y)`) on the sim thread, when it copies
//! the snapshot out, and rebuilds only those chunks; a listed tile that is
//! gone was demoted back to the base surface. A presenter that skips
//! snapshots can catch up by comparing each tile's `changed_tick`.

use crate::combat::Explosion;
use crate::ecs::PlayerId;
use crate::ecs::{Entities, EntityKind, SimId, Stance};
use crate::game::{MatchState, NavPhase, Order, OrderKind, Outcome};
use crate::items::{BlastKind, cloud_now};
use crate::projectile::ProjectilePool;
use crate::rules::{Defeat, capture_full};
use crate::systems::FireRequest;
use crate::terrain::Heightfield;
use crate::unit::{StanceOrder, UnitOrder};
use frontline_content::{ITEM_KINDS, ItemKind};

/// `soldier_squad` value for a soldier without a squad.
pub const NO_SQUAD: u32 = u32::MAX;

/// One squad, for selection, cards and order markers.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SquadSnapshot {
    pub id: u32,
    /// Index into `MatchSetup::squad_types`.
    pub type_idx: u16,
    pub owner: u8,
    pub members: u8,
    /// Members that are detached (S1b; counted in `members` too).
    pub detached: u8,
    pub order: OrderKind,
    pub phase: NavPhase,
    /// Selection anchor: the formation leader's position `[x, y, z]`.
    pub anchor: [f32; 3],
    /// Where the squad is headed (its position when holding).
    pub goal: [f32; 2],
    /// Dig orders: trench progress in percent (0 otherwise).
    pub dig_pct: u8,
    /// Items the men carry, by [`ItemKind::index`] (frag, smoke, satchel).
    pub items: [u16; ITEM_KINDS],
    /// S10a: construction kg the squad carries (`LoadMaterials`).
    pub materials_kg: f32,
    /// S10b `clear_mines` orders: the lane and how far along it the
    /// sweepers are (`None` for other orders).
    pub clear_lane: Option<ClearLaneSnapshot>,
    /// S10b `demolish` orders: the target piece id.
    pub demolish: Option<u32>,
}

/// S10b: a `clear_mines` order's lane (`a` to `b`, `along` metres from `a`).
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct ClearLaneSnapshot {
    pub a: [f32; 2],
    pub b: [f32; 2],
    pub along: f32,
}

/// A thrown item in the air, or landed and waiting for its fuse.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ThrownSnapshot {
    pub id: u32,
    pub kind: ItemKind,
    /// Index into `MatchSetup::items`.
    pub item: u16,
    pub owner: u8,
    pub pos: [f32; 3],
    pub landed: bool,
}

/// A smoke cloud: a vertical cylinder standing at `pos`, as tall as it is
/// wide.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SmokeSnapshot {
    pub id: u32,
    pub pos: [f32; 3],
    pub radius: f32,
    /// 1 while it blocks every sight line, falling to 0 as it thins out.
    pub density: f32,
}

/// A detached soldier's own order (S1b), for order markers.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UnitSnapshot {
    pub id: SimId,
    pub owner: u8,
    pub squad: u32,
    /// `Idle`, `Move`, `AttackMove`, `Retreat`, `Dig`, `Follow` or (S3b)
    /// `Suppress`.
    pub order: OrderKind,
    pub phase: NavPhase,
    /// What the order aims at: the move target, the own base for a
    /// retreat, the middle of the trench line for a dig, his spot when idle,
    /// the middle of the line on Suppress (S3b).
    pub target: [f32; 2],
    /// His own spot: where he walks to and stands (his slot on the line,
    /// after cover-seeking once there).
    pub goal: [f32; 2],
}

/// A dead soldier, kept for 30 s (rules `combat.corpse_s`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CorpseSnapshot {
    pub id: SimId,
    /// `[x, y, z, yaw]`.
    pub pos: [f32; 4],
    pub owner: u8,
    /// Ticks until the body is removed.
    pub ticks_left: u16,
    /// Players that see the body (`Combat::seen_by`; spotting keeps checking
    /// corpses), so fog of war hides bodies like men.
    pub seen_by: u16,
}

/// S14: a flame particle in flight (`MatchState::flame`, never the
/// projectile pool), ascending id.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FlameSnapshot {
    pub id: u32,
    pub pos: [f32; 3],
    pub player: u8,
}

/// S14: a burning patch of ground, ascending id: burns within
/// `[flame] patch_radius_cm` of `pos` until it goes out.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PatchSnapshot {
    pub id: u32,
    /// `[x, y, z]`, z on the ground.
    pub pos: [f32; 3],
    pub player: u8,
    /// Ticks until it goes out.
    pub ticks_left: u32,
}

/// S12: a downed man (`EntityKind::Downed`), ascending id: he lies prone
/// where he fell (or where his carriers are).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CasualtySnapshot {
    pub id: SimId,
    /// `[x, y, z, yaw]`.
    pub pos: [f32; 4],
    pub owner: u8,
    /// Players that see him (as for corpses).
    pub seen_by: u16,
    /// Stabilised by a medic: he no longer bleeds out.
    pub stabilised: bool,
    /// Ticks until he bleeds out (0 when stabilised).
    pub bleed_ticks: u32,
    /// The medic tending him (`u32::MAX`: none) and ticks to go.
    pub tend: u32,
    pub tend_ticks: u32,
    /// His carriers (`u32::MAX`: none) and where they take him.
    pub carriers: [u32; 2],
    pub dest: Option<[f32; 2]>,
    /// Ticks until his evacuation in base (0: none).
    pub evac_ticks: u32,
}

/// S12 W8: one bed of an aid post.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BedSnapshot {
    pub man: SimId,
    /// Ticks until he is treated or evacuated (paused while unstaffed).
    pub ticks_left: u32,
    /// Evacuation (else treatment).
    pub evac: bool,
}

/// A round fired this tick (muzzle flash, tracer, sound).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShotSnapshot {
    pub shooter: SimId,
    /// Index into `MatchSetup::weapons`.
    pub weapon: u16,
    /// Muzzle position.
    pub from: [f32; 3],
    /// Initial velocity (m/s).
    pub vel: [f32; 3],
}

/// An HE burst this tick (explosion, light flash, dust).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExplosionSnapshot {
    pub pos: [f32; 3],
    /// Index into `MatchSetup::weapons` for a shell, into
    /// `MatchSetup::items` for a frag or satchel.
    pub weapon: u16,
    pub kind: BlastKind,
}

/// One player's match status.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlayerSnapshot {
    pub manpower: u32,
    /// Manpower per minute right now (base, points held, AI difficulty).
    pub income_per_min: u32,
    pub tickets: u32,
    /// Tickets lost per minute right now.
    pub drain_per_min: u32,
    pub surrendered: bool,
    /// Set once the player is out.
    pub defeat: Option<Defeat>,
    /// Alive soldiers plus soldiers on order.
    pub pop: u32,
    pub pop_cap: u32,
    pub kills: u32,
    pub losses: u32,
    /// Points held now, and point-seconds held so far (end screen).
    pub points_held: u8,
    pub hold_s: u32,
    /// S3b: ticks until each squad type (index into
    /// `MatchSetup::squad_types`) can be called in again; 0 = ready.
    pub cooldowns: Vec<u32>,
    /// S3: vehicles alive, the rules' limit, and the tick from which
    /// vehicles can be called in.
    pub vehicles_counted: u8,
    pub vehicle_limit: u8,
    pub unlock_tick: u64,
    /// S5a: manpower paid for ammunition this match by supply kind (small
    /// arms, explosives, shells; µMP), and whether a refill of this player
    /// stood paused for lack of manpower within the last second.
    pub ammo_paid: [u64; 3],
    pub refill_paused: bool,
    /// S6: ticks until the base issues the next crate (0: ready), and the
    /// live crates (all, shells).
    pub crate_ready_ticks: u32,
    pub crates_live: [u16; 2],
}

// ---------------------------------------------------------------- S3 --

/// `VehicleSnapshot::flags` bits.
pub mod vehicle_flags {
    /// Moving faster than the rules' `moving_cm_s` (either direction).
    pub const MOVING: u16 = 1 << 0;
    /// Driving backwards.
    pub const REVERSING: u16 = 1 << 1;
    /// Stuck in a trench crossing or on blocked ground until freed.
    pub const BOGGED: u16 = 1 << 2;
    /// Cannot move: engine or a track destroyed, or no driver.
    pub const IMMOBILISED: u16 = 1 << 3;
    pub const BURNING: u16 = 1 << 4;
    /// The crew is repairing a component (`repairing` names it).
    pub const REPAIRING: u16 = 1 << 5;
    /// A track sample is over ground dug 1 m or more (speed capped).
    pub const CROSSING: u16 = 1 << 6;
    /// A wreck: owner 255, no crew, blocks rounds and vehicles.
    pub const WRECK: u16 = 1 << 7;
    /// A wreck knocked out less than [`super::WRECK_SMOKE_S`] ago (the
    /// presenter smokes it). Always with `WRECK`.
    pub const KNOCKED_OUT: u16 = 1 << 8;
    /// S3c: abandoned by its crew (owner 255 like a wreck, the last owner
    /// in `last_owner`; seen by everyone; can be re-crewed).
    pub const ABANDONED: u16 = 1 << 9;
    /// S3c: someone is climbing in (`Recrew` boarding).
    pub const BOARDING: u16 = 1 << 10;
    /// S15 R1: its rider slots are an exposed deck (`[riders] enabled` 1;
    /// the riders sit on it in the open, `soldier_riding` names them).
    pub const DECK: u16 = 1 << 11;
    /// S15 R3a-R3c: its riders' exit timer runs (suppressed, the vehicle
    /// struck, its gun fired): they jump off over the sides, stunned.
    pub const RIDERS_OFF: u16 = 1 << 12;
    /// The flags an enemy (or anyone, for a wreck) is sent.
    pub const PUBLIC: u16 =
        BURNING | WRECK | KNOCKED_OUT | ABANDONED | BOARDING | DECK | RIDERS_OFF;
}

/// A fresh wreck smokes this long (`vehicle_flags::KNOCKED_OUT`).
pub const WRECK_SMOKE_S: u32 = 60;
/// `VehicleSnapshot::owner` of a wreck.
pub const WRECK_OWNER: u8 = 255;
/// `VehicleSnapshot::repairing` and `ArmourEvent` "none" values.
pub const NONE_U8: u8 = 255;

/// One crew seat in `VehicleSnapshot::seats`, indexed by
/// `SeatRole::index` (D G C L R).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum SeatSnap {
    /// The vehicle has no such seat (or an enemy's: zeroed).
    #[default]
    Absent = 0,
    Ok = 1,
    /// HP below the rules' `wounded_hp`.
    Wounded = 2,
    /// Nobody in it: the crewman died or moved to another seat.
    Empty = 3,
    /// A crewman is moving in (`seat_swap_s`); counts as empty meanwhile.
    Swapping = 4,
}

/// `VehicleSnapshot::loaded`: the round in the breech.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum LoadedSnap {
    /// Unloaded (reloading, out of ammo) or an enemy's (zeroed).
    #[default]
    None = 0,
    /// The penetrator (AP, APHE or HEAT; the mount's first round).
    Ap = 1,
    He = 2,
    /// S3b: the special penetrator (the mount's third round, APCR).
    Apcr = 3,
}

/// One vehicle or wreck, ascending id. Sim axes; angles in radians.
///
/// The hull rotation is `Rz(yaw) · Ry(−pitch) · Rx(roll)`: `yaw` is the
/// heading counter-clockwise from east (as a soldier's), positive `pitch`
/// raises the nose, positive `roll` raises the left side. The turret turns
/// by `turret_yaw` about the hull's up axis (counter-clockwise seen from
/// above), the gun by `gun_pitch` about the turret's left axis (positive:
/// elevation). Own vehicles carry every field; for enemy vehicles the
/// fields marked "own only" are zeroed and `flags` keeps only
/// [`vehicle_flags::PUBLIC`]. Wrecks are visible to everyone.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VehicleSnapshot {
    pub id: u32,
    /// Its squad (a vehicle is a squad of one); [`NO_SQUAD`] for wrecks.
    pub squad: u32,
    /// Index into `MatchSetup::vehicles`.
    pub type_idx: u16,
    /// Player slot; [`WRECK_OWNER`] for wrecks.
    pub owner: u8,
    /// Hull origin: on the ground under the hull's centre.
    pub pos: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
    /// Hull-relative.
    pub turret_yaw: f32,
    pub gun_pitch: f32,
    /// Signed m/s along the hull's forward axis (negative: reversing).
    pub speed: f32,
    /// [`vehicle_flags`] bits.
    pub flags: u16,
    /// Players that see it (bit per slot; own slot always; every bit for
    /// wrecks). The bridge filters enemy vehicles by it.
    pub seen_by: u16,
    /// Own only: [`SeatSnap`] per `SeatRole::index`.
    pub seats: [u8; 5],
    /// Own only: HP % (0..=100) per `ComponentKind::index` (engine, track
    /// L, track R, gun, turret ring, ammo rack; S3b: transmission).
    pub components: [u8; frontline_content::COMPONENTS],
    /// Own only: the `ComponentKind::index` being repaired, [`NONE_U8`]
    /// for none.
    pub repairing: u8,
    /// Own only: main-gun rounds left, penetrator, HE, then (S3b) the
    /// special penetrator (0 on guns without one); the one in the breech
    /// included.
    pub ammo: [u16; 3],
    /// Own only: [`LoadedSnap`].
    pub loaded: u8,
    /// Own only: the ammo choice (`SetAmmo`: 0 auto, 1 AP, 2 HE, 3 APCR).
    pub ammo_choice: u8,
    /// Own only: main-gun reload progress, 0..=100 (100: ready).
    pub reload_pct: u8,
    /// Own only: the main gun's target vehicle id, `u32::MAX` none.
    pub target: u32,
    /// Own only: soldiers escorting it (`FollowUnits`).
    pub followers: u8,
    /// Own only (S3b): the `SuppressUnits` line it fires at, if any.
    pub suppress: Option<[[f32; 2]; 2]>,
    /// Own only (S3b Fix): MG rounds left in stowage, summed over the
    /// mounts that have a limit; `u32::MAX` when none has one.
    pub mg_rounds: u32,
    /// S8b: a truck's cargo by `SupplyKind::index` (grams), and its drop
    /// order: crates still to set down (0: none).
    pub cargo_g: [u32; 3],
    pub drop_left: u8,
    /// S3c: an abandoned vehicle's last owner (its `owner` is 255); the
    /// owner otherwise. Public.
    pub last_owner: u8,
    /// S3c: why it was abandoned (`bail::AB_*`), 0 when not. Public.
    pub abandon_cause: u8,
    /// S3c: its crew squad while abandoned ([`NO_SQUAD`]: none). Public.
    pub crew_squad: u32,
    /// S3c: crewmen still inside an abandoned vehicle. Public.
    pub inside: u8,
    /// S8a transports: passengers aboard and slots (0 for other vehicles),
    /// and the unload under way (0 none, 1 door, 2 over the sides). Public
    /// (the men are seen over the rim).
    pub riders: u8,
    pub capacity: u8,
    pub unload: u8,
    /// S8a: the first two MGs' yaw from the hull front (radians; pintles
    /// turn, other MGs read 0).
    pub mg_yaw: [f32; 2],
    /// S9: a served gun: `GunState` + 1 (0: not a gun). Public.
    pub gun_state: u8,
    /// S9: men at its slots, and slots. Public (the men are seen).
    pub gun_men: u8,
    pub gun_slots: u8,
    /// S9: the tow it is hitched to; the gun a tow pulls (`u32::MAX`: none).
    /// Public.
    pub hitched: u32,
    pub towing: u32,
    /// Own only (S9): the hold-fire range (m; 0: none).
    pub hold_fire_m: u16,
}

/// `ShellSnapshot::kind`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum ShellKind {
    /// AP and APHE.
    #[default]
    Ap = 0,
    He = 1,
    /// HEAT (the S3 rockets).
    Heat = 2,
}

/// A main-gun shell or AT rocket in flight (small arms and mortar shells
/// stay in `projectiles`), in slot order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShellSnapshot {
    /// Projectile pool slot: stable while the shell flies (for
    /// interpolation); reused after it ends.
    pub slot: u32,
    /// The vehicle or soldier that fired it.
    pub shooter: u32,
    /// The firing player.
    pub player: u8,
    /// Index into `MatchSetup::ammo`.
    pub ammo: u16,
    /// [`ShellKind`].
    pub kind: u8,
    pub pos: [f32; 3],
    pub vel: [f32; 3],
}

/// `ArmourEvent::kind`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum ArmourEventKind {
    /// A vehicle fired its main gun (`component`: [`LoadedSnap`] of the
    /// round; `pos`: the muzzle).
    #[default]
    Fired = 0,
    /// A round glanced off a plate (`zone`, `ammo` set).
    Ricochet = 1,
    NonPenetration = 2,
    Penetration = 3,
    /// A component lost HP but still works (`component`:
    /// `ComponentKind::index`).
    ComponentHit = 4,
    /// A crewman was hurt or killed (`component`: `SeatRole::index`).
    CrewHit = 5,
    /// A component reached 0 HP (`component`: `ComponentKind::index`).
    Disabled = 6,
    /// The vehicle became a wreck (`component`: 0 too few crew, 1 ammo
    /// rack detonation, 2 engine fire burned out, 3 ammo fire burned out,
    /// S3b).
    KnockedOut = 7,
    /// The vehicle bogged in a trench crossing.
    Bogged = 8,
    /// S3c: the crew bailed out (`component`: `bail::AB_*`, 1 ammo fire,
    /// 2 engine fire, 3 crew losses, 4 gun and mobility out, 5 disabled
    /// under fire, 6 ordered). `attacker`: the last player to hit it.
    Abandoned = 9,
    /// S3c: one crewman got out (`component`: `SeatRole::index`; `pos`:
    /// where he stands).
    CrewOut = 10,
    /// S3c: an abandoned vehicle was manned again (`owner`: the new owner;
    /// `attacker`: the last owner; `component`: 1 own side, 2 captured).
    Recrewed = 11,
}

/// One armour event of this tick, in event order (main-gun and rocket
/// impacts, damage, knock-outs; small arms never make one).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ArmourEvent {
    /// [`ArmourEventKind`].
    pub kind: u8,
    pub vehicle: u32,
    /// The vehicle's owner when it happened (a knocked-out vehicle's last
    /// owner, not 255).
    pub owner: u8,
    /// The firing player, [`NONE_U8`] for none (fire, bogging).
    pub attacker: u8,
    /// Per kind, see [`ArmourEventKind`]; [`NONE_U8`] when unused.
    pub component: u8,
    /// `ArmourZone::index` of the plate hit, [`NONE_U8`] for none.
    pub zone: u8,
    /// Index into `MatchSetup::ammo` of the round, `u16::MAX` for none.
    pub ammo: u16,
    pub pos: [f32; 3],
    /// Running count of armour events in the match (saved and hashed), for
    /// direct-control prediction later.
    pub seq: u32,
}

/// S6: an HQ or dump (own and enemy; the bridge hides unseen enemy
/// ones), ascending id.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StructureSnapshot {
    pub id: u32,
    pub owner: u8,
    /// Index into `MatchSetup::stocks`.
    pub def: u16,
    pub pos: [f32; 2],
    pub yaw: f32,
    pub radius: f32,
    pub built: bool,
    /// While digging: the builder's pit progress (0..=100).
    pub build_pct: u8,
    pub radio: bool,
    /// Stock and caps, kg by supply kind.
    pub stock_kg: [f32; 3],
    pub cap_kg: [f32; 3],
    /// Hit points left of each def box (%).
    pub box_pct: Vec<u8>,
    /// S12 W8 (aid posts): the beds in use and the own medics staffing it
    /// now (it works at the def's `staff_min`).
    pub beds: Vec<BedSnapshot>,
    pub staff: u8,
}

/// S12 `Wound` bits of a soldier (`Snapshot::soldier_wound`).
pub mod wound_bits {
    /// Wounded (else fit).
    pub const WOUNDED: u8 = 1;
    pub const BANDAGED: u8 = 2;
    /// A leg wound.
    pub const LEGS: u8 = 4;
    /// Treated at an aid post once.
    pub const TREATED: u8 = 8;
    /// Dressing a wound now.
    pub const BANDAGING: u8 = 16;
}

/// S6: a crate, ascending id.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CrateSnapshot {
    pub id: u32,
    pub owner: u8,
    /// Index into `MatchSetup::stocks`.
    pub def: u16,
    /// `SupplyKind::index`.
    pub kind: u8,
    pub pos: [f32; 3],
    /// The man carrying it, `u32::MAX` for none.
    pub carrier: u32,
    pub kg: f32,
}

/// S10a: one standing fortification piece, ascending id.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FortPieceSnapshot {
    pub id: u32,
    pub owner: u8,
    /// Index into `MatchSetup::forts`.
    pub fort: u16,
    /// Index into `frontline_content::defs::PIECE_PARTS` (sandbag, roof,
    /// timber; the colour).
    pub part: u8,
    /// Box centre `[x, y, z]`, its yaw and half extents (along the facing,
    /// across, up).
    pub center: [f32; 3],
    pub yaw: f32,
    pub half: [f32; 3],
    /// Hit points left (%).
    pub hp_pct: u8,
}

/// S10a: one standing wire section, ascending id.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WireSnapshot {
    pub id: u32,
    pub owner: u8,
    pub fort: u16,
    /// End points `[x, y, z]` (z on the ground).
    pub a: [f32; 3],
    pub b: [f32; 3],
    /// Band width and height (m).
    pub band: f32,
    pub height: f32,
    pub hp_pct: u8,
}

/// S10a: one site being dug or built, ascending id.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FortWorkSnapshot {
    pub id: u32,
    pub owner: u8,
    pub fort: u16,
    pub builder: u32,
    /// `WorkStep as u8`: 0 dig, 1 waiting for materials, 2 build.
    pub step: u8,
    /// The site of the current step (a line: its current segment) and yaw.
    pub site: [f32; 3],
    pub yaw: f32,
    /// The whole line (point and over_hq: `[pos, toward]`).
    pub line: Vec<[f32; 2]>,
    /// Segment being built (lines; 0 otherwise) and the number of segments.
    pub seg: u8,
    pub segments: u8,
    /// Progress of the current step (0..=100): pit dug, build work done.
    pub progress_pct: u8,
    /// kg of the current step drawn so far (0..=100 %).
    pub materials_pct: u8,
}

/// S10a: a laid Bangalore charge, ascending id.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChargeSnapshot {
    pub id: u32,
    pub owner: u8,
    /// Index into `MatchSetup::items`.
    pub item: u16,
    pub from: [f32; 3],
    pub to: [f32; 3],
    /// Ticks until it bursts.
    pub fuse_ticks: u32,
    /// S10b: the piece a demolition charge is tamped against (`None`: a
    /// Bangalore).
    pub piece: Option<u32>,
}

/// S10b: one laid mine, ascending id. Every mine is listed; hiding the
/// ones the local player does not know is the presenter's job (design 7.6,
/// "a presentation rule").
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MineSnapshot {
    pub id: u32,
    pub owner: u8,
    /// The minefield blueprint (index into `MatchSetup::forts`).
    pub fort: u16,
    /// `[x, y, z]`, z on the ground.
    pub pos: [f32; 3],
    /// Players who know where it is (bit per player).
    pub known: u8,
}

/// S10a: fortifications (match mode only). Enemy entries are listed too;
/// the bridge hides the ones the local player has not seen.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FortSnapshot {
    pub pieces: Vec<FortPieceSnapshot>,
    pub wire: Vec<WireSnapshot>,
    pub works: Vec<FortWorkSnapshot>,
    pub charges: Vec<ChargeSnapshot>,
    /// S10b.
    pub mines: Vec<MineSnapshot>,
}

/// One capture point, in map order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PointSnapshot {
    pub pos: [f32; 2],
    pub radius: f32,
    pub value: u32,
    pub owner: Option<u8>,
    /// Who is capturing it, and how far (0..=100).
    pub capturer: Option<u8>,
    pub progress_pct: u8,
    pub contested: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RenderSnapshot {
    /// Tick this snapshot shows (ticks completed).
    pub tick: u64,
    /// Per live soldier, in ascending id order: `[x, y, z, yaw]`.
    pub soldiers: Vec<[f32; 4]>,
    /// Ids matching `soldiers`, for interpolating between snapshots.
    pub soldier_ids: Vec<SimId>,
    /// Owner (player slot) per entry of `soldiers`.
    pub soldier_owner: Vec<u8>,
    /// Squad id per entry of `soldiers`, or [`NO_SQUAD`] (match mode only).
    pub soldier_squad: Vec<u32>,
    /// Per entry of `soldiers` (match mode only): stance, hit points,
    /// suppression (0..=100), pinned, and the players that see him
    /// (bit per player slot, own player always set).
    pub soldier_stance: Vec<Stance>,
    pub soldier_hp: Vec<u16>,
    pub soldier_suppression: Vec<u8>,
    pub soldier_pinned: Vec<bool>,
    /// S3b Fix: ducked below a trench lip (not firing), per entry.
    pub soldier_ducked: Vec<bool>,
    pub soldier_visible: Vec<u16>,
    /// Per entry of `soldiers` (match mode only, S1b): the stance order
    /// (`SetStance`) and whether he is detached from his squad's formation.
    pub soldier_stance_order: Vec<StanceOrder>,
    pub soldier_detached: Vec<bool>,
    /// Per entry of `soldiers` (match mode only, S1b): items carried by
    /// [`ItemKind::index`], and whether he is winding up a throw.
    pub soldier_items: Vec<[u8; ITEM_KINDS]>,
    pub soldier_throwing: Vec<bool>,
    /// Per entry of `soldiers` (match mode only, S4): the weapon he carries
    /// (index into `MatchSetup::weapons`) and his `[[members]]` slot.
    pub soldier_weapon: Vec<u16>,
    pub soldier_member_slot: Vec<u8>,
    /// Per entry of `soldiers` (match mode only, S5a): the weapon he fights
    /// with now (his slot's sidearm while his main weapon is dry), the
    /// rounds he holds for it (pouches and magazine; `u16::MAX` for
    /// uncounted weapons: legacy content, launchers), whether he is on his
    /// sidearm, and his fire mode (`FireMode as u8`).
    pub soldier_active_weapon: Vec<u16>,
    pub soldier_rounds: Vec<u16>,
    pub soldier_sidearm: Vec<bool>,
    pub soldier_fire: Vec<u8>,
    /// S8a, per entry of `soldiers` (match mode only): the transport he
    /// rides (`u32::MAX`: on foot) and his passenger slot.
    pub soldier_riding: Vec<u32>,
    pub soldier_ride_slot: Vec<u8>,
    /// S12, per entry of `soldiers` (match mode only): [`wound_bits`].
    pub soldier_wound: Vec<u8>,
    /// S14, per entry of `soldiers` (match mode only): he is burning (a
    /// flame hit or burning ground, `[flame] burn_s`).
    pub soldier_burning: Vec<bool>,
    /// S14 M3, per entry of `soldiers` (match mode only): his strike phase
    /// (`crate::flame::STRIKE_NONE` 0, `STRIKE_WINDUP` 1, `STRIKE_RECOVER`
    /// 2: the strike landed or missed at the start of the recovery).
    pub soldier_strike: Vec<u8>,
    /// S15 P1, per entry of `soldiers` (match mode only): he wears a body
    /// plate (`[body_armour] enabled` 1; the setup drops plates otherwise).
    /// A stop is counted on his player (`s15.plate_stops`), no event.
    pub soldier_plate: Vec<bool>,
    /// Detached soldiers, ascending id (match mode only).
    pub units: Vec<UnitSnapshot>,
    /// Dead soldiers, ascending id (match mode only).
    pub corpses: Vec<CorpseSnapshot>,
    /// S12: downed men, ascending id (match mode only).
    pub casualties: Vec<CasualtySnapshot>,
    /// Rounds fired this tick, in fire order (match mode only).
    pub shots: Vec<ShotSnapshot>,
    /// HE bursts this tick, in order (match mode only).
    pub explosions: Vec<ExplosionSnapshot>,
    /// Thrown items, ascending id (match mode only).
    pub thrown: Vec<ThrownSnapshot>,
    /// Smoke clouds, ascending id (match mode only).
    pub smoke: Vec<SmokeSnapshot>,
    /// S14: flame particles in flight, ascending id (match mode only).
    pub flame: Vec<FlameSnapshot>,
    /// S14: burning ground, ascending id (match mode only).
    pub patches: Vec<PatchSnapshot>,
    /// Per live projectile, in slot order: `[x, y, z]`.
    pub projectiles: Vec<[f32; 3]>,
    /// Vehicles and wrecks, ascending id (match mode only, S3).
    pub vehicles: Vec<VehicleSnapshot>,
    /// Main-gun shells and rockets in flight, slot order (S3).
    pub shells: Vec<ShellSnapshot>,
    /// Armour events this tick, in order (S3).
    pub armour_events: Vec<ArmourEvent>,
    /// Call-in results this tick, in command order (match mode only,
    /// S3b): the squad that spawned or why the call-in was refused.
    pub call_ins: Vec<crate::game::CallInEvent>,
    /// S3c: this tick's refused `Recrew` commands (the feed says why).
    pub recrew_refusals: Vec<crate::bail::RecrewEvent>,
    /// S5b: this tick's refused fire missions (the HUD says why).
    pub fire_mission_refusals: Vec<crate::fire_mission::FireMissionEvent>,
    /// S6: this tick's refused S6 commands (the HUD says why).
    pub supply_refusals: Vec<crate::structures::SupplyEvent>,
    /// S10a: this tick's refused S10a commands (the HUD says why).
    pub fort_refusals: Vec<crate::fort::FortEvent>,
    /// S10a: pieces, wire, sites and charges (match mode only).
    pub fort: FortSnapshot,
    /// S6: HQs and dumps, ascending id (match mode only).
    pub structures: Vec<StructureSnapshot>,
    /// S6: crates, ascending id (match mode only).
    pub crates: Vec<CrateSnapshot>,
    /// S4: crew weapons picked up in the last tick.
    pub pickups: Vec<crate::game::PickupEvent>,
    /// Squads in ascending id order (match mode only).
    pub squads: Vec<SquadSnapshot>,
    /// Per player slot (match mode only).
    pub players: Vec<PlayerSnapshot>,
    /// Capture points in map order (match mode only).
    pub points: Vec<PointSnapshot>,
    /// Running, won or drawn.
    pub outcome: Outcome,
    /// Winner's player slot once the match is over.
    pub winner: Option<u8>,
    /// Terrain detail tiles `(x, y)` changed in this tick, ascending
    /// `(y, x)`. Tile `(x, y)` covers `[x, x+1) × [y, y+1) · TILE_M` metres.
    pub dirty_tiles: Vec<(u16, u16)>,
}

/// Copies detail tile `(x, y)`'s heights into `out` as `f32` metres (64 × 64,
/// row-major, see `DetailTile::heights`). Returns `false` (and leaves `out`
/// empty) if the tile does not exist, i.e. that chunk is the base surface.
pub fn tile_heights_f32(terrain: &Heightfield, x: u16, y: u16, out: &mut Vec<f32>) -> bool {
    out.clear();
    match terrain.detail_tile(x, y) {
        Some(t) => {
            out.extend(t.heights().iter().map(|h| h.to_f32()));
            true
        }
        None => false,
    }
}

impl RenderSnapshot {
    /// Refills `self` in place (reuses allocations).
    pub(crate) fn fill(
        &mut self,
        tick: u64,
        ents: &Entities,
        pool: &ProjectilePool,
        game: Option<&MatchState>,
        dirty_tiles: &[(u16, u16)],
    ) {
        self.tick = tick;
        self.dirty_tiles.clear();
        self.dirty_tiles.extend_from_slice(dirty_tiles);
        self.soldiers.clear();
        self.soldier_ids.clear();
        self.soldier_owner.clear();
        self.soldier_squad.clear();
        self.soldier_stance.clear();
        self.soldier_hp.clear();
        self.soldier_suppression.clear();
        self.soldier_pinned.clear();
        self.soldier_ducked.clear();
        self.soldier_visible.clear();
        self.soldier_stance_order.clear();
        self.soldier_detached.clear();
        self.soldier_items.clear();
        self.soldier_throwing.clear();
        self.soldier_weapon.clear();
        self.soldier_member_slot.clear();
        self.soldier_active_weapon.clear();
        self.soldier_rounds.clear();
        self.soldier_sidearm.clear();
        self.soldier_fire.clear();
        self.soldier_riding.clear();
        self.soldier_ride_slot.clear();
        self.soldier_wound.clear();
        self.soldier_burning.clear();
        self.soldier_strike.clear();
        self.soldier_plate.clear();
        self.units.clear();
        self.corpses.clear();
        self.casualties.clear();
        for (i, kind) in ents.kind().iter().enumerate() {
            let t = &ents.transform()[i];
            let [x, y, z] = t.pos.to_f32();
            let owner = ents.owner()[i].0;
            let c = &ents.combat()[i];
            match kind {
                EntityKind::Soldier => {
                    self.soldiers.push([x, y, z, t.yaw.to_f32()]);
                    self.soldier_ids.push(SimId(i as u32));
                    self.soldier_owner.push(owner);
                    if game.is_some() {
                        self.soldier_squad
                            .push(ents.squad()[i].map_or(NO_SQUAD, |s| s.0));
                        self.soldier_stance.push(c.stance);
                        self.soldier_hp.push(c.hp);
                        self.soldier_suppression
                            .push((c.suppression / 1000).min(100) as u8);
                        self.soldier_pinned.push(c.pinned);
                        self.soldier_ducked.push(c.ducked);
                        // S17b (checker after A, 5): bit p set when p's team
                        // sight sees him or p is his ally (alone: as before).
                        self.soldier_visible
                            .push(game.map_or(c.seen_by | (1 << owner), |g| {
                                g.teams.visible_mask(owner, c.seen_by)
                            }));
                        let u = &ents.unit()[i];
                        self.soldier_stance_order.push(u.stance);
                        self.soldier_detached.push(u.detached);
                        let k = &ents.kit()[i];
                        self.soldier_items.push(k.items);
                        self.soldier_throwing.push(k.windup.is_some());
                        self.soldier_weapon.push(k.weapon);
                        self.soldier_member_slot.push(k.slot);
                        // S5a: rounds for the weapon in hand.
                        let held = game.and_then(|g| {
                            let sq = g.squad((*ents.squad())[i]?)?;
                            let setup = g.setup();
                            let ty = &setup.squad_types[sq.type_idx as usize];
                            let wi = crate::supply::active_weapon(ty, k);
                            let w = &setup.weapons[wi as usize];
                            let n = ty.feed_slot(w).map(|f| {
                                crate::supply::man_total(setup, ty, k, c, f)
                                    + if w.he.is_some() { c.mag as u32 } else { 0 }
                            });
                            Some((wi, n))
                        });
                        self.soldier_active_weapon
                            .push(held.map_or(k.weapon, |h| h.0));
                        self.soldier_rounds.push(
                            held.and_then(|h| h.1)
                                .map_or(u16::MAX, |n| n.min(u16::MAX as u32 - 1) as u16),
                        );
                        self.soldier_sidearm.push(k.on_sidearm);
                        self.soldier_fire.push(u.fire as u8);
                        self.soldier_riding
                            .push(u.riding.map_or(u32::MAX, |r| r.vehicle.0));
                        self.soldier_ride_slot.push(u.riding.map_or(0, |r| r.slot));
                        let w = &c.wound;
                        self.soldier_burning.push(c.close.burn_until > tick);
                        self.soldier_strike.push(c.close.strike_phase);
                        self.soldier_plate.push(game.is_some_and(|g| {
                            let setup = g.setup();
                            setup.rules.body_armour.enabled == 1
                                && ents.squad()[i]
                                    .and_then(|q| g.squad(q))
                                    .and_then(|sq| setup.squad_types.get(sq.type_idx as usize))
                                    .and_then(|ty| ty.members.get(k.slot as usize))
                                    .is_some_and(|m| m.plate.is_some())
                        }));
                        self.soldier_wound.push(
                            [
                                (w.state == crate::wounds::WOUNDED, wound_bits::WOUNDED),
                                (w.bandaged, wound_bits::BANDAGED),
                                (w.legs, wound_bits::LEGS),
                                (w.treated, wound_bits::TREATED),
                                (w.bandaging > 0, wound_bits::BANDAGING),
                            ]
                            .iter()
                            .filter(|b| b.0)
                            .fold(0, |a, b| a | b.1),
                        );
                        if u.detached
                            && let Some(g) = game
                        {
                            let target = match &u.order {
                                UnitOrder::Idle => u.nav.goal,
                                UnitOrder::Move { target } | UnitOrder::AttackMove { target } => {
                                    *target
                                }
                                UnitOrder::Retreat => g.base_of(PlayerId(owner)),
                                UnitOrder::Follow { .. } => u.nav.goal,
                                UnitOrder::Suppress { line } => (line[0] + line[1]).div_int(2),
                                UnitOrder::Dig { job } => {
                                    g.unit_digs().get(job).map_or(u.nav.goal, |d| {
                                        let line = &d.line;
                                        let len = crate::squad::polyline_length(line);
                                        crate::squad::point_along(line, len.div_int(2))
                                    })
                                }
                            };
                            self.units.push(UnitSnapshot {
                                id: SimId(i as u32),
                                owner,
                                squad: ents.squad()[i].map_or(NO_SQUAD, |s| s.0),
                                order: u.order.kind(),
                                phase: u.nav.phase,
                                target: target.to_f32(),
                                goal: u.nav.goal.to_f32(),
                            });
                        }
                    }
                }
                // S12: a downed man (`casualties`).
                EntityKind::Downed => {
                    let cs = game.and_then(|g| g.casualties.get(&SimId(i as u32)));
                    let left = |at: u64| at.saturating_sub(tick).min(u32::MAX as u64) as u32;
                    self.casualties.push(CasualtySnapshot {
                        id: SimId(i as u32),
                        pos: [x, y, z, t.yaw.to_f32()],
                        owner,
                        seen_by: c.seen_by,
                        stabilised: cs.is_some_and(|k| k.bleed_out_at == 0),
                        bleed_ticks: cs.map_or(0, |k| left(k.bleed_out_at)),
                        tend: cs.and_then(|k| k.tend).map_or(u32::MAX, |m| m.0.0),
                        tend_ticks: cs.and_then(|k| k.tend).map_or(0, |m| left(m.1)),
                        carriers: cs.map_or([u32::MAX; 2], |k| {
                            k.carriers.map(|m| m.map_or(u32::MAX, |m| m.0))
                        }),
                        dest: cs.and_then(|k| k.dest).map(|d| d.to_f32()),
                        evac_ticks: cs.map_or(0, |k| left(k.evac_at)),
                    });
                }
                EntityKind::Corpse => self.corpses.push(CorpseSnapshot {
                    id: SimId(i as u32),
                    pos: [x, y, z, t.yaw.to_f32()],
                    owner,
                    ticks_left: c.corpse,
                    seen_by: c.seen_by,
                }),
                EntityKind::Vacant
                | EntityKind::Vehicle
                | EntityKind::Wreck
                | EntityKind::Abandoned => {}
            }
        }
        self.projectiles.clear();
        self.projectiles
            .extend(pool.slots().iter().flatten().map(|p| p.pos.to_f32()));
        self.vehicles.clear();
        if let Some(g) = game {
            self.fill_vehicles(tick, ents, g);
        }

        self.squads.clear();
        self.players.clear();
        self.points.clear();
        self.structures.clear();
        self.crates.clear();
        self.thrown.clear();
        self.smoke.clear();
        self.flame.clear();
        self.patches.clear();
        self.outcome = Outcome::Running;
        self.winner = None;
        let Some(g) = game else {
            return;
        };
        for (id, sq) in g.squads() {
            if sq.members.is_empty() || g.is_vehicle_squad(*id) {
                continue;
            }
            let lead = sq.lead(ents);
            let mut items = [0u16; ITEM_KINDS];
            for m in &sq.members {
                for (n, c) in items.iter_mut().zip(ents.kit()[m.index()].items) {
                    *n += c as u16;
                }
            }
            self.squads.push(SquadSnapshot {
                id: id.0,
                type_idx: sq.type_idx,
                owner: sq.owner.0,
                members: sq.members.len() as u8,
                detached: sq
                    .members
                    .iter()
                    .filter(|m| ents.unit()[m.index()].detached)
                    .count() as u8,
                order: sq.order.kind(),
                phase: sq.nav.phase,
                anchor: ents.transform()[lead.index()].pos.to_f32(),
                goal: sq.nav.goal.to_f32(),
                dig_pct: match &sq.order {
                    Order::Dig { job, .. } => {
                        job.progress_pct(job.rules(&g.setup().rules), g.setup().tick_hz)
                    }
                    _ => 0,
                },
                items,
                materials_kg: g.fort().carried_g.get(id).map_or(0.0, |&gr| {
                    crate::fx::Fx::from_ratio(gr as i64, 1000).to_f32()
                }),
                clear_lane: match &sq.order {
                    Order::ClearMines { a, b, along, .. } => Some(ClearLaneSnapshot {
                        a: a.to_f32(),
                        b: b.to_f32(),
                        along: along.to_f32(),
                    }),
                    _ => None,
                },
                demolish: match &sq.order {
                    Order::Demolish { piece, .. } => Some(*piece),
                    _ => None,
                },
            });
        }
        let items = &g.setup().items;
        for t in g.thrown() {
            self.thrown.push(ThrownSnapshot {
                id: t.id,
                kind: items[t.item as usize].kind,
                item: t.item,
                owner: t.owner.0,
                pos: t.pos.to_f32(),
                landed: t.landed,
            });
        }
        for c in g.smoke() {
            let Some(s) = items[c.item as usize].smoke.as_ref() else {
                continue;
            };
            let (r, density) = cloud_now(s, c.age);
            self.smoke.push(SmokeSnapshot {
                id: c.id,
                pos: c.pos.to_f32(),
                radius: r.to_f32(),
                density: crate::fx::Fx::from_ratio(density as i64, 1000).to_f32(),
            });
        }
        self.flame.extend(g.flame().iter().map(|f| FlameSnapshot {
            id: f.id,
            pos: f.pos.to_f32(),
            player: f.player,
        }));
        self.patches
            .extend(g.patches().iter().map(|q| PatchSnapshot {
                id: q.id,
                pos: q.pos.to_f32(),
                player: q.player,
                ticks_left: q.ends_at.saturating_sub(tick).min(u32::MAX as u64) as u32,
            }));
        for (p, pl) in g.players().iter().enumerate() {
            let id = PlayerId(p as u8);
            self.players.push(PlayerSnapshot {
                manpower: pl.manpower,
                income_per_min: g.income_per_min(id),
                tickets: pl.tickets,
                drain_per_min: g.drain_per_min(id),
                surrendered: pl.surrendered,
                defeat: pl.defeat,
                pop: g.population(ents, id),
                pop_cap: g.pop_cap_of(id),
                kills: pl.kills,
                losses: pl.losses,
                points_held: g.points().iter().filter(|c| c.owner == Some(id)).count() as u8,
                hold_s: (pl.point_ticks / g.setup().tick_hz as u64) as u32,
                cooldowns: (0..pl.ready_at.len())
                    .map(|t| g.cooldown_left(id, t as u16, tick))
                    .collect(),
                vehicles_counted: g.vehicles_counted(ents, id).min(u8::MAX as u32) as u8,
                vehicle_limit: g.setup().rules.vehicles.limit.min(u8::MAX as u32) as u8,
                unlock_tick: g.setup().rules.vehicles.unlock_ticks as u64,
                ammo_paid: pl.ammo.paid,
                refill_paused: pl.ammo.paused_at > 0
                    && pl.ammo.paused_at + g.setup().tick_hz as u64 >= tick,
                crate_ready_ticks: g.supply6().accounts.get(p).map_or(0, |a| {
                    a.issue_at.saturating_sub(tick).min(u32::MAX as u64) as u32
                }),
                crates_live: {
                    let (n, sh) = g.supply6().crates_of(g.setup(), id);
                    [
                        n.min(u16::MAX as u32) as u16,
                        sh.min(u16::MAX as u32) as u16,
                    ]
                },
            });
        }
        let r = &g.setup().rules;
        let full = capture_full(r.capture_ticks).max(1);
        for (c, ps) in g.points().iter().zip(&g.setup().map.points) {
            self.points.push(PointSnapshot {
                pos: crate::fx::FxVec2::from(ps.pos).to_f32(),
                radius: crate::fx::Fx::from(r.capture_radius).to_f32(),
                value: ps.value,
                owner: c.owner.map(|o| o.0),
                capturer: c.capturer.map(|o| o.0),
                progress_pct: (c.progress as u64 * 100 / full as u64) as u8,
                contested: c.contested,
            });
        }
        self.fill_supply(tick, ents, g);
        self.outcome = g.outcome();
        if let Outcome::Won { winner } = g.outcome() {
            self.winner = Some(winner.0);
        }
    }

    /// S6: structures and crates.
    fn fill_supply(&mut self, tick: u64, ents: &Entities, g: &MatchState) {
        let setup = g.setup();
        let kg = |gr: u32| crate::fx::Fx::from_ratio(gr as i64, 1000).to_f32();
        for (&id, s) in &g.supply6().structures {
            let d = &setup.stocks[s.def as usize];
            let build_pct = match s.builder.and_then(|b| g.squad(b)) {
                Some(sq) => match &sq.order {
                    Order::Dig { job, .. } => {
                        job.progress_pct(job.rules(&setup.rules), setup.tick_hz)
                    }
                    _ => 0,
                },
                None => 100,
            };
            self.structures.push(StructureSnapshot {
                id,
                owner: s.owner.0,
                def: s.def,
                pos: s.pos.to_f32(),
                yaw: s.yaw.to_f32(),
                radius: crate::fx::Fx::from(d.radius).to_f32(),
                built: s.built,
                build_pct,
                radio: s.radio,
                stock_kg: [kg(s.stock_g[0]), kg(s.stock_g[1]), kg(s.stock_g[2])],
                cap_kg: [kg(d.cap_g[0]), kg(d.cap_g[1]), kg(d.cap_g[2])],
                box_pct: d
                    .boxes
                    .iter()
                    .enumerate()
                    .map(|(k, b)| (s.box_hp[k] as u64 * 100 / b.hp.max(1) as u64) as u8)
                    .collect(),
                beds: s
                    .beds
                    .iter()
                    .map(|b| BedSnapshot {
                        man: b.man,
                        ticks_left: b.done_at.saturating_sub(tick).min(u32::MAX as u64) as u32,
                        evac: b.evac,
                    })
                    .collect(),
                staff: if d.kind == frontline_content::setup::StockKind::Aid {
                    g.post_staff(ents, s)
                } else {
                    0
                },
            });
        }
        for (&id, c) in &g.supply6().crates {
            let d = &setup.stocks[c.def as usize];
            let pos = match c.carrier {
                Some(m) if m.index() < ents.len() => ents.transform()[m.index()].pos.to_f32(),
                _ => [c.pos.x.to_f32(), c.pos.y.to_f32(), 0.0],
            };
            self.crates.push(CrateSnapshot {
                id,
                owner: c.owner.0,
                def: c.def,
                kind: d.supply.map_or(0, |k| k.index() as u8),
                pos,
                carrier: c.carrier.map_or(u32::MAX, |m| m.0),
                kg: kg(c.g),
            });
        }
    }

    /// S10a: fills `fort` (pieces, wire, sites, charges, ascending id).
    /// Called after `fill` with the terrain (wire ends and sites sit on
    /// the ground); clears `fort` without a match.
    pub(crate) fn fill_fort(
        &mut self,
        tick: u64,
        terrain: &Heightfield,
        game: Option<&MatchState>,
    ) {
        self.fort.pieces.clear();
        self.fort.wire.clear();
        self.fort.works.clear();
        self.fort.charges.clear();
        self.fort.mines.clear();
        let Some(g) = game else {
            return;
        };
        let f = g.fort();
        if f.is_empty() {
            return;
        }
        let setup = g.setup();
        let pct = |a: u64, b: u64| (a.min(b) * 100 / b.max(1)) as u8;
        let on = |p: crate::fx::FxVec2| [p.x.to_f32(), p.y.to_f32(), terrain.height_at(p).to_f32()];
        // Entries naming no blueprint (a test edit before validation) are
        // skipped: the snapshot never panics on state.
        for (&id, q) in &f.pieces {
            let Some(b) = setup
                .forts
                .get(q.fort as usize)
                .and_then(|d| d.pieces.get(q.part as usize))
            else {
                continue;
            };
            let half = [
                crate::fx::Fx::from(b.half[0]).to_f32(),
                crate::fx::Fx::from(b.half[1]).to_f32(),
                crate::fx::Fx::from(b.half[2]).to_f32(),
            ];
            self.fort.pieces.push(FortPieceSnapshot {
                id,
                owner: q.owner.0,
                fort: q.fort,
                part: b.part,
                center: [
                    q.pos.x.to_f32(),
                    q.pos.y.to_f32(),
                    (q.z + crate::fx::Fx::from(b.half[2])).to_f32(),
                ],
                yaw: q.yaw.to_f32(),
                half,
                hp_pct: pct(q.hp as u64, b.hp as u64),
            });
        }
        for (&id, w) in &f.wire {
            let Some(d) = setup.forts.get(w.fort as usize) else {
                continue;
            };
            let (band, height, hp) = d.wire.as_ref().map_or((0.0, 0.0, 1), |ws| {
                (
                    crate::fx::Fx::from(ws.band).to_f32(),
                    crate::fx::Fx::from(ws.height).to_f32(),
                    ws.hp,
                )
            });
            self.fort.wire.push(WireSnapshot {
                id,
                owner: w.owner.0,
                fort: w.fort,
                a: on(w.a),
                b: on(w.b),
                band,
                height,
                hp_pct: pct(w.hp as u64, hp as u64),
            });
        }
        for (&id, w) in &f.works {
            let Some(d) = setup.forts.get(w.fort as usize) else {
                continue;
            };
            if w.line.is_empty() {
                continue;
            }
            let (site, yaw) = g.site_of(w);
            let progress_pct = match w.step {
                crate::fort::WorkStep::Dig => match g.squad(w.builder).map(|q| &q.order) {
                    Some(Order::Dig { job, .. }) => {
                        job.progress_pct(job.rules(&setup.rules), setup.tick_hz)
                    }
                    _ => 0,
                },
                crate::fort::WorkStep::Wait => 0,
                crate::fort::WorkStep::Build => {
                    pct(w.work, d.work_ticks as u64 * d.work_men as u64 * 100)
                }
            };
            let segments = if d.shape == frontline_content::setup::FortShape::Line {
                crate::fort::segments(&w.line, d.segment).min(255) as u8
            } else {
                1
            };
            self.fort.works.push(FortWorkSnapshot {
                id,
                owner: w.owner.0,
                fort: w.fort,
                builder: w.builder.0,
                step: w.step as u8,
                site: on(site),
                yaw: yaw.to_f32(),
                line: w.line.iter().map(|p| p.to_f32()).collect(),
                seg: w.seg,
                segments,
                progress_pct,
                materials_pct: if w.step == crate::fort::WorkStep::Build {
                    100
                } else {
                    pct(w.got_g as u64, d.materials_g as u64)
                },
            });
        }
        for c in &f.charges {
            self.fort.charges.push(ChargeSnapshot {
                id: c.id,
                owner: c.owner.0,
                item: c.item,
                from: on(c.from),
                to: on(c.to),
                fuse_ticks: c.fuse_tick.saturating_sub(tick).min(u32::MAX as u64) as u32,
                piece: c.piece,
            });
        }
        for (&id, m) in &f.mines {
            self.fort.mines.push(MineSnapshot {
                id,
                owner: m.owner.0,
                fort: m.fort,
                pos: on(m.pos),
                known: m.known,
            });
        }
    }

    /// Fills `vehicles`: every vehicle and wreck, ascending id, all fields
    /// (the bridge zeroes the own-only ones for enemies).
    fn fill_vehicles(&mut self, tick: u64, ents: &Entities, g: &MatchState) {
        use crate::vehicle::AmmoChoice;
        use frontline_content::{COMPONENTS, SEAT_ROLES, SeatRole};
        let setup = g.setup();
        let vr = &setup.rules.vehicles;
        let all = ((1u32 << g.players().len()) - 1) as u16;
        let moving = crate::fx::Fx::from(vr.moving_speed);
        let smoke_ticks = WRECK_SMOKE_S as u64 * setup.tick_hz as u64;
        for (&id, vs) in g.vehicles() {
            let i = id.index();
            let v = &setup.vehicles[vs.type_idx as usize];
            let t = &ents.transform()[i];
            let wreck = vs.wreck_tick.is_some();
            let abandoned = vs.abandoned.is_some();
            let owner = ents.owner()[i].0;
            let mut flags = 0u16;
            {
                use vehicle_flags::*;
                let mut set = |f: u16, on: bool| {
                    if on {
                        flags |= f;
                    }
                };
                set(MOVING, vs.speed.abs() > moving);
                set(REVERSING, vs.speed < crate::fx::Fx::ZERO);
                set(BOGGED, vs.bog > 0 || vs.on_blocked);
                set(IMMOBILISED, !wreck && !abandoned && vs.immobilised(v));
                set(ABANDONED, abandoned);
                set(BOARDING, vs.board.is_some());
                set(
                    DECK,
                    setup.rules.riders.enabled == 1
                        && v.transport.as_ref().is_some_and(|t| t.exposed),
                );
                set(RIDERS_OFF, vs.rider_off.is_some());
                set(BURNING, vs.burn > 0 || vs.ammo_fire > 0);
                set(REPAIRING, vs.repairing.is_some());
                set(CROSSING, vs.crossing);
                set(WRECK, wreck);
                set(
                    KNOCKED_OUT,
                    vs.wreck_tick
                        .is_some_and(|w| tick.saturating_sub(w) < smoke_ticks),
                );
            }
            let mut seats = [SeatSnap::Absent as u8; SEAT_ROLES];
            for role in SeatRole::ALL {
                let Some(st) = vs.seat(v, role) else { continue };
                let snap = if st.swap > 0 {
                    SeatSnap::Swapping
                } else if st.hp == 0 {
                    SeatSnap::Empty
                } else if (st.hp as u32) < vr.wounded_hp {
                    SeatSnap::Wounded
                } else {
                    SeatSnap::Ok
                };
                seats[role.index()] = snap as u8;
            }
            let mut components = [0u8; COMPONENTS];
            for c in &v.components {
                let hp = vs.component(c.kind) as u32;
                components[c.kind.index()] = (hp * 100 / c.hp.max(1)).min(100) as u8;
            }
            // S8b: a truck has no gun (never index `mounts[0]`).
            let reload_total = v
                .main_gun()
                .map_or(1, |g| setup.weapons[g.weapon as usize].reload_ticks.max(1));
            self.vehicles.push(VehicleSnapshot {
                id: id.0,
                squad: ents.squad()[i].map_or(NO_SQUAD, |s| s.0),
                type_idx: vs.type_idx,
                owner: if wreck || abandoned {
                    WRECK_OWNER
                } else {
                    owner
                },
                pos: t.pos.to_f32(),
                yaw: t.yaw.to_f32(),
                pitch: vs.pitch.to_f32(),
                roll: vs.roll.to_f32(),
                // S8a: a vehicle without a main gun shows its first MG's
                // yaw here (a pintle turns; the presenter turns its shield).
                turret_yaw: if v.main_gun().is_none() {
                    vs.mgs.first().map_or(0.0, |m| m.yaw.to_f32())
                } else {
                    vs.turret_yaw.to_f32()
                },
                gun_pitch: vs.gun_pitch.to_f32(),
                speed: vs.speed.to_f32(),
                flags,
                seen_by: if wreck || abandoned {
                    all
                } else {
                    g.teams.visible_mask(owner, ents.combat()[i].seen_by)
                },
                seats,
                components,
                repairing: vs.repairing.unwrap_or(NONE_U8),
                ammo: [0usize, 1, 2].map(|k| {
                    vs.ammo.get(k).copied().unwrap_or(0) + (vs.loaded == Some(k as u8)) as u16
                }),
                loaded: match vs.loaded {
                    None => LoadedSnap::None,
                    Some(1) => LoadedSnap::He,
                    Some(2) => LoadedSnap::Apcr,
                    Some(_) => LoadedSnap::Ap,
                } as u8,
                ammo_choice: match vs.ammo_choice {
                    AmmoChoice::Auto => 0,
                    AmmoChoice::Ap => 1,
                    AmmoChoice::He => 2,
                    AmmoChoice::Apcr => 3,
                },
                reload_pct: (100 - (vs.reload as u32 * 100 / reload_total).min(100)) as u8,
                target: vs
                    .target
                    .filter(|t| ents.kind().get(t.index()) == Some(&EntityKind::Vehicle))
                    .map_or(u32::MAX, |t| t.0),
                followers: vs.followers.len().min(u8::MAX as usize) as u8,
                suppress: vs.suppress.map(|l| [l[0].to_f32(), l[1].to_f32()]),
                mg_rounds: v
                    .mg_mounts()
                    .iter()
                    .zip(&vs.mgs)
                    .filter_map(|(m, st)| m.rounds.map(|n| n.saturating_sub(st.spent)))
                    .reduce(|a, b| a.saturating_add(b))
                    .unwrap_or(u32::MAX),
                cargo_g: vs.cargo_g,
                drop_left: vs.drop.map_or(0, |d| d.count),
                last_owner: owner,
                abandon_cause: vs.abandon_cause,
                crew_squad: vs.crew_squad.map_or(NO_SQUAD, |q| q.0),
                inside: if abandoned {
                    vs.seats.iter().filter(|s| s.hp > 0).count() as u8
                } else {
                    0
                },
                riders: vs.riders.iter().filter(|r| r.is_some()).count() as u8,
                capacity: vs.riders.len().min(u8::MAX as usize) as u8,
                unload: match vs.unload {
                    None => 0,
                    Some(u) if u.over_sides => 2,
                    Some(_) => 1,
                },
                mg_yaw: [0usize, 1].map(|k| vs.mgs.get(k).map_or(0.0, |m| m.yaw.to_f32())),
                gun_state: vs.served.as_ref().map_or(0, |s| s.state as u8 + 1),
                gun_men: vs.served.as_ref().map_or(0, |s| {
                    s.slots.iter().filter(|x| x.man.is_some()).count() as u8
                }),
                gun_slots: vs
                    .served
                    .as_ref()
                    .map_or(0, |s| s.slots.len().min(u8::MAX as usize) as u8),
                hitched: vs
                    .served
                    .as_ref()
                    .and_then(|s| s.hitched)
                    .map_or(u32::MAX, |t| t.0),
                towing: vs.towing.map_or(u32::MAX, |t| t.0),
                hold_fire_m: vs.served.as_ref().map_or(0, |s| s.hold_fire_m),
            });
        }
    }

    /// Refills `shells`: main-gun shells and rockets in flight, slot order.
    pub(crate) fn fill_shells(
        &mut self,
        pool: &crate::projectile::ProjectilePool,
        game: Option<&MatchState>,
    ) {
        use frontline_content::AmmoMechanism;
        self.shells.clear();
        let Some(g) = game else { return };
        for (slot, p) in pool.slots().iter().enumerate() {
            let Some(p) = p else { continue };
            let Some(a) = g.setup().ammo.get(p.ammo as usize) else {
                continue;
            };
            let kind = match a.mechanism {
                AmmoMechanism::Ap | AmmoMechanism::Aphe => ShellKind::Ap,
                AmmoMechanism::He => ShellKind::He,
                AmmoMechanism::Heat => ShellKind::Heat,
            };
            self.shells.push(ShellSnapshot {
                slot: slot as u32,
                shooter: p.owner.0,
                player: p.player,
                ammo: p.ammo,
                kind: kind as u8,
                pos: p.pos.to_f32(),
                vel: p.vel.to_f32(),
            });
        }
    }

    /// Refills this tick's shots, bursts and armour events.
    pub(crate) fn fill_events(
        &mut self,
        shots: &[FireRequest],
        explosions: &[Explosion],
        armour: &[crate::damage::ArmourEv],
    ) {
        self.shots.clear();
        self.shots.extend(shots.iter().map(|r| ShotSnapshot {
            shooter: r.shooter,
            weapon: r.weapon,
            from: r.pos.to_f32(),
            vel: r.vel.to_f32(),
        }));
        self.armour_events.clear();
        self.armour_events
            .extend(armour.iter().map(|e| ArmourEvent {
                kind: e.kind as u8,
                vehicle: e.vehicle.0,
                owner: e.owner,
                attacker: e.attacker,
                component: e.component,
                zone: e.zone,
                ammo: e.ammo,
                pos: e.pos.to_f32(),
                seq: e.seq,
            }));
        self.explosions.clear();
        self.explosions
            .extend(explosions.iter().map(|e| ExplosionSnapshot {
                pos: e.pos.to_f32(),
                weapon: e.weapon,
                kind: e.kind,
            }));
    }
}
