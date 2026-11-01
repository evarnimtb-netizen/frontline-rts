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
    /// The flags an enemy (or anyone, for a wreck) is sent.
    pub const PUBLIC: u16 = BURNING | WRECK | KNOCKED_OUT | ABANDONED | BOARDING;
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
    /// S3c: an abandoned vehicle's last owner (its `owner` is 255); the
    /// owner otherwise. Public.
    pub last_owner: u8,
    /// S3c: why it was abandoned (`bail::AB_*`), 0 when not. Public.
    pub abandon_cause: u8,
    /// S3c: its crew squad while abandoned ([`NO_SQUAD`]: none). Public.
    pub crew_squad: u32,
    /// S3c: crewmen still inside an abandoned vehicle. Public.
    pub inside: u8,
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
    /// Detached soldiers, ascending id (match mode only).
    pub units: Vec<UnitSnapshot>,
    /// Dead soldiers, ascending id (match mode only).
    pub corpses: Vec<CorpseSnapshot>,
    /// Rounds fired this tick, in fire order (match mode only).
    pub shots: Vec<ShotSnapshot>,
    /// HE bursts this tick, in order (match mode only).
    pub explosions: Vec<ExplosionSnapshot>,
    /// Thrown items, ascending id (match mode only).
    pub thrown: Vec<ThrownSnapshot>,
    /// Smoke clouds, ascending id (match mode only).
    pub smoke: Vec<SmokeSnapshot>,
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
        self.units.clear();
        self.corpses.clear();
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
                        self.soldier_visible.push(c.seen_by | (1 << owner));
                        let u = &ents.unit()[i];
                        self.soldier_stance_order.push(u.stance);
                        self.soldier_detached.push(u.detached);
                        let k = &ents.kit()[i];
                        self.soldier_items.push(k.items);
                        self.soldier_throwing.push(k.windup.is_some());
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
        self.thrown.clear();
        self.smoke.clear();
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
                pop_cap: g.setup().rules.pop_cap,
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
        self.outcome = g.outcome();
        if let Outcome::Won { winner } = g.outcome() {
            self.winner = Some(winner.0);
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
            let gun = &setup.weapons[v.main_gun().weapon as usize];
            let reload_total = gun.reload_ticks.max(1);
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
                turret_yaw: vs.turret_yaw.to_f32(),
                gun_pitch: vs.gun_pitch.to_f32(),
                speed: vs.speed.to_f32(),
                flags,
                seen_by: if wreck || abandoned {
                    all
                } else {
                    ents.combat()[i].seen_by | (1 << owner)
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
                    .mounts
                    .iter()
                    .skip(1)
                    .zip(&vs.mgs)
                    .filter_map(|(m, st)| m.rounds.map(|n| n.saturating_sub(st.spent)))
                    .reduce(|a, b| a.saturating_add(b))
                    .unwrap_or(u32::MAX),
                last_owner: owner,
                abandon_cause: vs.abandon_cause,
                crew_squad: vs.crew_squad.map_or(NO_SQUAD, |q| q.0),
                inside: if abandoned {
                    vs.seats.iter().filter(|s| s.hp > 0).count() as u8
                } else {
                    0
                },
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
