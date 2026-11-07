//! Staged S3 setups (`docs/slices/S3-armour.md` "`frontline-cli
//! armour-test`" and the staged autoplay scenes of "Done when" 7), used by
//! `frontline-cli armour-test` and the Godot bridge alike.
//!
//! A staged world starts from a resolved [`MatchSetup`] (the bridge and the
//! CLI load the defs; the sim does no file IO). [`staging`] turns it into a
//! test ground: both players human, plenty of manpower, vehicles unlocked,
//! and, unless the case needs Kessel Ridge's real terrain, flat open ground
//! (the map's noise, ridges and woods removed). The functions here then
//! place squads and vehicles with the world's setup tools
//! (`World::spawn_squad_at`, `World::place_vehicle`, `World::edit_vehicle`,
//! `World::dig_trench_now`, `World::dig_at_ditch_now`). Those are direct
//! state edits, never commands: a staged world is for tests and tools,
//! never a networked match. Everything after the staging is driven by real
//! commands.
//!
//! The armour-test cases stage along the line `y = `[`LANE_Y`] in the
//! middle of the map, far from both bases (whose starting squads stay
//! idle there: the AI is off), at ranges of at most 300 m so real
//! spotting applies. Each case takes the player whose side it measures
//! (`p`, the other side is `1 − p`), so a run can be repeated with the
//! factions swapped.

use frontline_content::setup::{BandSetup, Fixed, FixedVec2};
use frontline_content::variant::role_tag;
use frontline_content::{ComponentKind, MatchSetup, SquadRole, WeaponClass};

use crate::command::{Command, CommandKind};
use crate::ecs::{EntityKind, PlayerId, SimId};
use crate::fx::{Fx, FxVec2, FxVec3};
use crate::game::SquadId;
use crate::snapshot::vehicle_flags;
use crate::vehicle::AmmoChoice;
use crate::world::{SimError, World};

/// The staging line (m).
pub const LANE_Y: i32 = 512;
/// The trench of the infantry cases runs north–south through this x (m).
pub const TRENCH_X: i32 = 500;
/// Half the length of that trench (m).
pub const TRENCH_HALF_M: i32 = 22;
/// HP a toughened vehicle's seats and components get (case 1 targets):
/// no hit can knock it out.
pub const TOUGH_HP: u16 = 60_000;
/// Case 1: the shooter's main-gun reload (ticks), so a run collects its
/// hits quickly.
pub const FAST_RELOAD_TICKS: u32 = 6;
/// Case 3d: the crossing target's top speed (cm/s).
pub const CROSSING_CM_S: i32 = 600;

pub const EAST: FxVec2 = FxVec2::new(Fx::ONE, Fx::ZERO);
pub const WEST: FxVec2 = FxVec2::new(Fx::NEG_ONE, Fx::ZERO);
pub const NORTH: FxVec2 = FxVec2::new(Fx::ZERO, Fx::ONE);
pub const SOUTH: FxVec2 = FxVec2::new(Fx::ZERO, Fx::NEG_ONE);

/// `(x, y)` in whole metres.
pub fn at(x: i32, y: i32) -> FxVec2 {
    FxVec2::from_int(x, y)
}

/// The unit vector `deg` degrees counter-clockwise from east.
pub fn dir_deg(deg: i32) -> FxVec2 {
    let (s, c) = crate::armour::deg(deg).sin_cos();
    FxVec2::new(c, s)
}

fn yaw_of(f: FxVec2) -> Fx {
    Fx::atan2(f.y, f.x)
}

/// Turns a resolved match setup into a staging ground (module docs):
/// both players human, 100 000 manpower, no pop cap worries, vehicles
/// unlocked; with `flat`, the map's noise, ridges, valleys and woods are
/// removed.
pub fn staging(setup: &mut MatchSetup, flat: bool) {
    setup.players.iter_mut().for_each(|p| p.is_ai = false);
    setup.rules.start_manpower = 100_000;
    setup.rules.vehicles.unlock_ticks = 0;
    if flat {
        setup.map.terrain.octaves.clear();
        setup.map.terrain.bands.clear();
        setup.map.woods.clear();
    }
}

/// Adds a straight crest of `height_cm` along `from → to`, falling to the
/// ground `half_width_m` to either side (a terrain band).
pub fn add_crest(
    setup: &mut MatchSetup,
    from: FxVec2,
    to: FxVec2,
    height_cm: i32,
    half_width_m: i32,
) {
    let v = |p: FxVec2| FixedVec2 {
        x: Fixed(p.x.raw()),
        y: Fixed(p.y.raw()),
    };
    setup.map.terrain.bands.push(BandSetup {
        from: v(from),
        to: v(to),
        height: Fixed::from_cm(height_cm.max(0) as u32),
        half_width: Fixed::from_int(half_width_m),
    });
}

/// The first squad type of player `p`'s roster with `role` (S4: the
/// legacy role's tag, `frontline_content::variant::role_tag`).
pub fn type_of(w: &World, p: u8, role: SquadRole) -> Option<u16> {
    type_with_tag(w.match_setup()?, p, role_tag(role))
}

/// S4: the first squad type of player `p`'s roster carrying `tag`.
pub fn type_with_tag(s: &MatchSetup, p: u8, tag: &str) -> Option<u16> {
    let f = &s.factions[s.players.get(p as usize)?.faction as usize];
    f.roster.iter().copied().find(|&t| s.type_has_tag(t, tag))
}

/// S4: spawns player `p`'s first roster squad with `tag` at `center`.
pub fn spawn_tag(
    w: &mut World,
    p: u8,
    tag: &str,
    center: FxVec2,
    facing: FxVec2,
) -> Option<SquadId> {
    let t = type_with_tag(w.match_setup()?, p, tag)?;
    w.spawn_squad_at(PlayerId(p), t, center, facing)
}

/// Spawns player `p`'s squad of `role` at `center` facing `facing`.
pub fn spawn(
    w: &mut World,
    p: u8,
    role: SquadRole,
    center: FxVec2,
    facing: FxVec2,
) -> Option<SquadId> {
    let t = type_of(w, p, role)?;
    w.spawn_squad_at(PlayerId(p), t, center, facing)
}

/// Spawns player `p`'s tank at `pos` facing `facing`: `(squad, vehicle)`.
pub fn tank(w: &mut World, p: u8, pos: FxVec2, facing: FxVec2) -> Option<(SquadId, SimId)> {
    let sq = spawn(w, p, SquadRole::Vehicle, pos, facing)?;
    let v = *w.game()?.squad(sq)?.members.first()?;
    Some((sq, v))
}

/// Squad `sq`'s members (empty once it is gone).
pub fn members(w: &World, sq: SquadId) -> Vec<SimId> {
    w.game()
        .and_then(|g| g.squad(sq))
        .map_or_else(Vec::new, |s| s.members.clone())
}

/// S3c: no bail-out trigger fires (the Abandon order still works).
pub fn no_bail(setup: &mut MatchSetup) {
    let b = &mut setup.rules.vehicles.bail;
    b.ammo_fire_bail = 0;
    b.engine_fire_bail_s = 0;
    b.crew_below = 0;
    b.useless_bail = 0;
    b.under_fire_s = 0;
}

/// Gives vehicle `v` [`TOUGH_HP`] in every seat and component (gun
/// optionally destroyed) and puts out a fire: case 1 targets take every
/// hit without being knocked out. Re-apply after each tick.
pub fn toughen(w: &mut World, v: SimId, gun: bool) -> bool {
    w.edit_vehicle(v, |vs| {
        for s in &mut vs.seats {
            if s.swap == 0 {
                s.hp = TOUGH_HP;
            }
        }
        for (k, c) in vs.components.iter_mut().enumerate() {
            *c = if !gun && k == ComponentKind::Gun.index() {
                0
            } else {
                TOUGH_HP
            };
        }
        vs.burn = 0;
        vs.ammo_fire = 0;
        vs.repairing = None;
        vs.repair_progress = 0;
    })
}

/// A finished trench through `(TRENCH_X, LANE_Y)` north–south,
/// [`TRENCH_HALF_M`] each way, parapet to the east (`front = -1` digs it
/// on the right of the south→north line).
pub fn trench_ns(w: &mut World) -> [FxVec2; 2] {
    let line = [
        at(TRENCH_X, LANE_Y - TRENCH_HALF_M),
        at(TRENCH_X, LANE_Y + TRENCH_HALF_M),
    ];
    w.dig_trench_now(&line, -1);
    line
}

/// A command for player `p` with sequence number `seq`.
pub fn cmd(p: u8, seq: u32, kind: CommandKind) -> Command {
    Command {
        player: PlayerId(p),
        seq,
        kind,
    }
}

/// A staged world and what was placed in it, in the order each case
/// documents.
pub struct Staged {
    pub world: World,
    pub tanks: Vec<(SquadId, SimId)>,
    pub squads: Vec<SquadId>,
    /// A trench or AT-ditch line, when the case has one.
    pub line: Vec<FxVec2>,
}

impl Staged {
    fn new(world: World) -> Staged {
        Staged {
            world,
            tanks: Vec::new(),
            squads: Vec::new(),
            line: Vec::new(),
        }
    }
}

fn need<T>(v: Option<T>, what: &str) -> Result<T, SimError> {
    v.ok_or_else(|| SimError::InvalidConfig(format!("staging: {what}")))
}

fn new_world(setup: MatchSetup, threads: usize) -> Result<World, SimError> {
    World::new_match(setup, threads)
}

// ------------------------------------------------------------- cases --

/// Case 1, penetration statistics: player `p`'s tank at `(350, LANE_Y)`
/// facing east fires at the enemy tank `range_m` east of it, whose front
/// faces it turned by `angle_deg` (90: its side). The shooter reloads in
/// [`FAST_RELOAD_TICKS`] and carries 500 rounds of each kind; ammo racks
/// never detonate and fires never start. The target's gun is destroyed;
/// the runner keeps it tough with [`toughen`]. Tanks: `[shooter, target]`.
pub fn pen_stats(
    mut setup: MatchSetup,
    threads: usize,
    p: u8,
    range_m: i32,
    angle_deg: i32,
) -> Result<Staged, SimError> {
    staging(&mut setup, true);
    setup.rules.vehicles.rack_detonate_pct = 0;
    setup.rules.vehicles.fire_start_pct = 0;
    setup.rules.vehicles.damage.rack_fire_pct = 0;
    // S3c: a toughened target (gun out) must never bail out, or the shooter
    // loses it after two penetrations ("disabled under fire").
    no_bail(&mut setup);
    for v in &mut setup.vehicles {
        if let Some(m) = v.mounts.first_mut() {
            m.ammo.iter_mut().for_each(|a| a.count = 500);
        }
    }
    for w in &mut setup.weapons {
        if w.class == WeaponClass::Cannon {
            w.reload_ticks = FAST_RELOAD_TICKS;
        }
    }
    let mut s = Staged::new(new_world(setup, threads)?);
    let w = &mut s.world;
    let shooter = need(tank(w, p, at(350, LANE_Y), EAST), "shooter")?;
    let facing = dir_deg(180 + angle_deg);
    let target = need(tank(w, 1 - p, at(350 + range_m, LANE_Y), facing), "target")?;
    toughen(w, target.1, false);
    s.tanks = vec![shooter, target];
    Ok(s)
}

/// Case 2, hull-down: player `p`'s tank sits behind a crest with only its
/// turret showing to the enemy tank 300 m west of it in the open. The
/// crest's height is halfway between the hull top and the turret top; the
/// tank is placed where the shooter's muzzle sees its turret top but
/// neither its hull top centre nor the hull's top front edge. Tanks:
/// `[hull-down, open]`.
pub fn hull_down(mut setup: MatchSetup, threads: usize, p: u8) -> Result<Staged, SimError> {
    staging(&mut setup, true);
    let ty = type_of_setup(&setup, p, SquadRole::Vehicle);
    let v = &setup.vehicles[need(ty, "hull-down tank")? as usize];
    let hull_top = Fx::from(v.hull.center.z) + Fx::from(v.hull.half.z);
    // S7: the upper box's top (the turret's, a casemate's).
    let up = &v.boxes[v.upper_box()];
    let turret_top = Fx::from(up.bounds.center.z)
        + Fx::from(up.bounds.half.z)
        + match up.frame {
            frontline_content::Frame::Turret => Fx::from(v.turret_pivot.z),
            frontline_content::Frame::Hull => Fx::ZERO,
        };
    let crest_x = 640;
    let crest_cm = ((hull_top + turret_top).div_int(2).mul_int(100)).floor_int();
    add_crest(
        &mut setup,
        at(crest_x, LANE_Y - 60),
        at(crest_x, LANE_Y + 60),
        crest_cm,
        8,
    );
    let mut s = Staged::new(new_world(setup, threads)?);
    let w = &mut s.world;
    let open = need(tank(w, 1 - p, at(crest_x - 290, LANE_Y), EAST), "open tank")?;
    let hd = need(tank(w, p, at(crest_x + 12, LANE_Y), WEST), "hull-down tank")?;
    // Back off from the crest until the hull is hidden.
    let mut placed = false;
    for d in 9..=30 {
        w.place_vehicle(hd.1, at(crest_x + d, LANE_Y), yaw_of(WEST));
        let g = need(w.game(), "match")?;
        let (Some(fo), Some(fh)) = (
            g.vehicle_derived().frame(open.1),
            g.vehicle_derived().frame(hd.1),
        ) else {
            continue;
        };
        let vs = need(g.vehicle(open.1), "open state")?;
        let ov = &g.setup().vehicles[vs.type_idx as usize];
        let muzzle = crate::gunnery::main_muzzle(fo, vs, ov.main_gun().expect("main gun"));
        let [turret, hull] = fh.spot_points(g.setup().rules.vehicles.driving.spot_drop_mm);
        let front =
            fh.hull.center + fh.hull.axes.x * fh.hull.half.x + fh.hull.axes.z * fh.hull.half.z;
        let t = w.terrain();
        if t.line_of_sight(muzzle, turret)
            && !t.line_of_sight(muzzle, hull)
            && !t.line_of_sight(muzzle, front)
        {
            placed = true;
            break;
        }
    }
    if !placed {
        return Err(SimError::InvalidConfig(
            "staging: no hull-down spot behind the crest".into(),
        ));
    }
    s.tanks = vec![hd, open];
    Ok(s)
}

fn type_of_setup(s: &MatchSetup, p: u8, role: SquadRole) -> Option<u16> {
    type_with_tag(s, p, role_tag(role)).and_then(|t| s.squad_types[t as usize].vehicle)
}

/// Case 3f: the tanks' sight (m), so that they see each other at 600 m
/// as if an observer had spotted them (× the 200 % size factor: 700 m).
pub const DUEL_600_SIGHT_M: i32 = 350;

/// Case 3's duels (Panzer IV is player 0's, the Sherman player 1's).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Duel {
    /// 300 m, both frontal.
    Frontal300,
    /// 300 m, the Panzer IV angled 30°.
    Angled300,
    /// 150 m, the Sherman on the Panzer IV's flank.
    Flank150,
    /// S3b: 150 m, the Panzer IV on the Sherman's flank (3c mirrored).
    PzFlank150,
    /// S3c (S3b-38 (A)): 600 m, both frontal (3f). Two still tanks 600 m
    /// apart never spot each other (sight 175 m × `spot_size_pct` 200 %),
    /// so the case stages them as spotted (by an observer): each tank
    /// type's sight is raised to [`DUEL_600_SIGHT_M`].
    Frontal600,
}

/// Case 3a–c, 3e: a duel; `flip` mirrors it east–west. Tanks: `[Panzer IV,
/// Sherman]`. S3b: in 3b the angled Panzer IV starts with its turret laid
/// on the Sherman (angling doctrine: hull angled, turret on the threat);
/// before, its turret sat on its hull line, 30° off, a staging artefact.
pub fn duel(
    mut setup: MatchSetup,
    threads: usize,
    kind: Duel,
    flip: bool,
) -> Result<Staged, SimError> {
    staging(&mut setup, true);
    if kind == Duel::Frontal600 {
        for v in &mut setup.vehicles {
            v.sight = frontline_content::Fixed::from_int(DUEL_600_SIGHT_M);
        }
    }
    let mut s = Staged::new(new_world(setup, threads)?);
    let w = &mut s.world;
    let mx = |x: i32| if flip { 1000 - x } else { x };
    let md = |d: FxVec2| if flip { FxVec2::new(-d.x, d.y) } else { d };
    let (pz_at, pz_f, sh_at, sh_f) = match kind {
        Duel::Frontal300 => (350, EAST, 650, WEST),
        Duel::Angled300 => (350, dir_deg(30), 650, WEST),
        Duel::Flank150 => (500, NORTH, 350, EAST),
        Duel::PzFlank150 => (350, EAST, 500, NORTH),
        Duel::Frontal600 => (200, EAST, 800, WEST),
    };
    let pz = need(tank(w, 0, at(mx(pz_at), LANE_Y), md(pz_f)), "Panzer IV")?;
    let sh = need(tank(w, 1, at(mx(sh_at), LANE_Y), md(sh_f)), "Sherman")?;
    if kind == Duel::Angled300 {
        // Lay the turret on the Sherman: the bearing less the hull yaw.
        let d = at(mx(sh_at), LANE_Y) - at(mx(pz_at), LANE_Y);
        let rel = (Fx::atan2(d.y, d.x) - yaw_of(md(pz_f))).wrap_angle();
        w.edit_vehicle(pz.1, |vs| vs.turret_yaw = rel);
    }
    s.tanks = vec![pz, sh];
    Ok(s)
}

/// S7: the first squad type of player `p`'s roster whose vehicle has
/// `role` (the casemate and open-top cases spawn the StuG and the M10).
pub fn type_with_vehicle_role(
    s: &MatchSetup,
    p: u8,
    role: frontline_content::VehicleRole,
) -> Option<u16> {
    let f = &s.factions[s.players.get(p as usize)?.faction as usize];
    f.roster.iter().copied().find(|&t| {
        s.squad_types[t as usize]
            .vehicle
            .and_then(|v| s.vehicles.get(v as usize))
            .is_some_and(|v| v.role == role)
    })
}

/// S7: spawns player `p`'s first vehicle of `role` at `pos` facing `facing`.
pub fn vehicle_of_role(
    w: &mut World,
    p: u8,
    role: frontline_content::VehicleRole,
    pos: FxVec2,
    facing: FxVec2,
) -> Option<(SquadId, SimId)> {
    let t = type_with_vehicle_role(w.match_setup()?, p, role)?;
    let sq = w.spawn_squad_at(PlayerId(p), t, pos, facing)?;
    let v = *w.game()?.squad(sq)?.members.first()?;
    Some((sq, v))
}

/// S7 casemate duels (armour-test cases 17 and 18; vehicles.md 10). The
/// StuG is player 0's assault gun, the Sherman player 1's tank.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum CasemateDuel {
    /// 17: 300 m, both frontal.
    Frontal300,
    /// 18: the Sherman 200 m away, 45° off the StuG's nose, facing it.
    Flank45,
}

/// Cases 17 and 18; `flip` mirrors east–west. Vehicles: `[StuG, Sherman]`.
pub fn casemate_duel(
    mut setup: MatchSetup,
    threads: usize,
    kind: CasemateDuel,
    flip: bool,
) -> Result<Staged, SimError> {
    staging(&mut setup, true);
    let mut s = Staged::new(new_world(setup, threads)?);
    let w = &mut s.world;
    let mx = |x: i32| if flip { 1000 - x } else { x };
    let md = |d: FxVec2| if flip { FxVec2::new(-d.x, d.y) } else { d };
    let (stug_at, sh_at) = match kind {
        CasemateDuel::Frontal300 => ((350, LANE_Y), (650, LANE_Y)),
        // 200 m at 45°: 141 m ahead and 141 m to the left.
        CasemateDuel::Flank45 => ((350, LANE_Y), (491, LANE_Y + 141)),
    };
    let stug_pos = at(mx(stug_at.0), stug_at.1);
    let sh_pos = at(mx(sh_at.0), sh_at.1);
    let to_stug = (stug_pos - sh_pos).normalize_or_zero();
    let stug = need(
        vehicle_of_role(
            w,
            0,
            frontline_content::VehicleRole::AssaultGun,
            stug_pos,
            md(EAST),
        ),
        "assault gun",
    )?;
    let sh = need(tank(w, 1, sh_pos, to_stug), "Sherman")?;
    s.tanks = vec![stug, sh];
    Ok(s)
}

/// Case 3d: player `p`'s tank at `(350, LANE_Y)` facing east and the
/// enemy tank 300 m east, either still or (with `moving`) about to cross
/// north at [`CROSSING_CM_S`] (the runner orders the move). The target's
/// gun is destroyed and it is kept tough. Tanks: `[shooter, target]`.
pub fn crossing_target(
    mut setup: MatchSetup,
    threads: usize,
    p: u8,
    moving: bool,
) -> Result<Staged, SimError> {
    staging(&mut setup, true);
    setup.rules.vehicles.rack_detonate_pct = 0;
    setup.rules.vehicles.fire_start_pct = 0;
    setup.rules.vehicles.damage.rack_fire_pct = 0;
    // S3c: a toughened target (gun out) must never bail out, or the shooter
    // loses it after two penetrations ("disabled under fire").
    no_bail(&mut setup);
    for v in &mut setup.vehicles {
        v.mobility.speed = Fixed::from_cm(CROSSING_CM_S as u32);
        v.mobility.reverse = v.mobility.reverse.min(v.mobility.speed);
    }
    let mut s = Staged::new(new_world(setup, threads)?);
    let w = &mut s.world;
    let shooter = need(tank(w, p, at(350, LANE_Y), EAST), "shooter")?;
    let y0 = if moving { LANE_Y - 120 } else { LANE_Y };
    let target = need(tank(w, 1 - p, at(650, y0), NORTH), "target")?;
    toughen(w, target.1, false);
    s.tanks = vec![shooter, target];
    Ok(s)
}

/// Case 4's flank lane and HE stand-off (m). The spec stages 60 m and
/// 100 m, which are exactly the bazooka's `engage_moving_m` and
/// `engage_still_m`: at those distances it never fires, so the lane and
/// the stand-off are 10 m closer.
pub const AT_FLANK_M: i32 = 50;
pub const AT_HE_M: i32 = 90;

/// Case 4's variants.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum AtRun {
    /// The tank drives past [`AT_FLANK_M`] east of the team, north and
    /// back.
    Flank,
    /// The tank drives at the team from 300 m.
    Front,
    /// The tank stands [`AT_HE_M`] off with HE loaded.
    He,
}

/// Case 4: player `p`'s AT team in the north–south trench, an enemy tank
/// per `run` (the runner gives the moves). Squads: `[AT team]`; tanks:
/// `[tank]`; line: the trench.
pub fn at_team(
    mut setup: MatchSetup,
    threads: usize,
    p: u8,
    run: AtRun,
) -> Result<Staged, SimError> {
    staging(&mut setup, true);
    let mut s = Staged::new(new_world(setup, threads)?);
    let w = &mut s.world;
    s.line = trench_ns(w).to_vec();
    let team = need(
        spawn(w, p, SquadRole::AntiTank, at(TRENCH_X, LANE_Y), EAST),
        "AT team",
    )?;
    let (pos, f) = match run {
        AtRun::Flank => (at(TRENCH_X + AT_FLANK_M, LANE_Y - 110), NORTH),
        AtRun::Front => (at(TRENCH_X + 300, LANE_Y), WEST),
        AtRun::He => (at(TRENCH_X + AT_HE_M, LANE_Y), WEST),
    };
    let t = need(tank(w, 1 - p, pos, f), "tank")?;
    if run == AtRun::He {
        let seq = 1;
        w.step(&[cmd(
            1 - p,
            seq,
            CommandKind::SetAmmo {
                units: vec![t.1],
                ammo: AmmoChoice::He,
            },
        )]);
    }
    s.squads = vec![team];
    s.tanks = vec![t];
    Ok(s)
}

/// Cases 5 and 6: player `p`'s lone tank against an enemy rifle squad in
/// the north–south trench (with `at_team`, an AT team in it too). Case 5
/// places the tank 100 m east, case 6 (`advance`) 300 m east; it faces
/// west. Squads: `[rifles, AT team?]`; tanks: `[tank]`.
pub fn lone_tank(
    mut setup: MatchSetup,
    threads: usize,
    p: u8,
    with_at: bool,
    advance: bool,
) -> Result<Staged, SimError> {
    staging(&mut setup, true);
    let mut s = Staged::new(new_world(setup, threads)?);
    let w = &mut s.world;
    s.line = trench_ns(w).to_vec();
    let rifles = need(
        spawn(w, 1 - p, SquadRole::Rifle, at(TRENCH_X, LANE_Y), EAST),
        "rifles",
    )?;
    s.squads.push(rifles);
    if with_at {
        let team = need(
            spawn(
                w,
                1 - p,
                SquadRole::AntiTank,
                at(TRENCH_X, LANE_Y + 12),
                EAST,
            ),
            "AT team",
        )?;
        s.squads.push(team);
    }
    let x = if advance {
        TRENCH_X + 300
    } else {
        TRENCH_X + 100
    };
    s.tanks.push(need(tank(w, p, at(x, LANE_Y), WEST), "tank")?);
    Ok(s)
}

/// Case 7: player `p`'s rifle squad (and, with `with_tank`, a tank in
/// front of it) 150 m east of an enemy rifle squad in the north–south
/// trench. Squads: `[attackers, defenders]`; tanks: `[tank?]`.
pub fn assault(
    mut setup: MatchSetup,
    threads: usize,
    p: u8,
    with_tank: bool,
) -> Result<Staged, SimError> {
    staging(&mut setup, true);
    let mut s = Staged::new(new_world(setup, threads)?);
    let w = &mut s.world;
    s.line = trench_ns(w).to_vec();
    let defenders = need(
        spawn(w, 1 - p, SquadRole::Rifle, at(TRENCH_X, LANE_Y), EAST),
        "defenders",
    )?;
    let start = TRENCH_X + 150;
    let behind = if with_tank { 10 } else { 0 };
    let attackers = need(
        spawn(w, p, SquadRole::Rifle, at(start + behind, LANE_Y), WEST),
        "attackers",
    )?;
    if with_tank {
        s.tanks
            .push(need(tank(w, p, at(start, LANE_Y), WEST), "tank")?);
    }
    s.squads = vec![attackers, defenders];
    Ok(s)
}

/// Case 8's variants.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum SatchelRun {
    /// Engineers 40 m east of a stationary enemy tank.
    Open,
    /// The same with an own rifle squad 80 m east of the tank firing at it.
    Supported,
    /// Engineers in the north–south trench; the tank crosses it westward.
    Trench,
}

/// Case 8: player `p`'s engineers against an enemy tank. Squads:
/// `[engineers, rifles?]`; tanks: `[tank]`; line: the trench (Trench).
pub fn satchel(
    mut setup: MatchSetup,
    threads: usize,
    p: u8,
    run: SatchelRun,
) -> Result<Staged, SimError> {
    staging(&mut setup, true);
    if run == SatchelRun::Trench {
        // "A tank crosses at 1 m/s": driven unscripted, a tank brakes from
        // 6.5 m/s at the first dug sample and is over a 1.6 m trench at
        // 2–4 m/s, so the crossing cap never shows. Here it drives at
        // 1 m/s throughout.
        for v in &mut setup.vehicles {
            v.mobility.speed = Fixed::ONE;
            v.mobility.reverse = v.mobility.reverse.min(Fixed::ONE);
        }
    }
    let mut s = Staged::new(new_world(setup, threads)?);
    let w = &mut s.world;
    match run {
        SatchelRun::Open | SatchelRun::Supported => {
            let t = need(tank(w, 1 - p, at(TRENCH_X, LANE_Y), NORTH), "tank")?;
            let eng = need(
                spawn(w, p, SquadRole::Engineer, at(TRENCH_X + 40, LANE_Y), WEST),
                "engineers",
            )?;
            s.tanks.push(t);
            s.squads.push(eng);
            if run == SatchelRun::Supported {
                let r = need(
                    spawn(w, p, SquadRole::Rifle, at(TRENCH_X + 80, LANE_Y + 10), WEST),
                    "rifles",
                )?;
                s.squads.push(r);
            }
        }
        SatchelRun::Trench => {
            s.line = trench_ns(w).to_vec();
            let eng = need(
                spawn(w, p, SquadRole::Engineer, at(TRENCH_X, LANE_Y), EAST),
                "engineers",
            )?;
            let t = need(tank(w, 1 - p, at(TRENCH_X + 15, LANE_Y), WEST), "tank")?;
            s.tanks.push(t);
            s.squads.push(eng);
        }
    }
    Ok(s)
}

/// Case 9: player `p`'s tank 100 m east of an enemy rifle squad, in the
/// north–south trench or in the open, HE loaded. The vehicles' MGs reach
/// only 1 m, so the HE rounds alone do the damage. Men crouched below a
/// trench lip are never seen, so a bait squad of `p`'s riflemen stands
/// 140 m east of the defenders: its rifles and SMGs do no damage and no
/// suppression, but the defenders rise to the fire step to shoot at it
/// and are seen (in both variants, alike). Squads: `[rifles, bait]`;
/// tanks: `[tank]`; line: the trench (if any).
pub fn he_infantry(
    mut setup: MatchSetup,
    threads: usize,
    p: u8,
    trench: bool,
) -> Result<Staged, SimError> {
    staging(&mut setup, true);
    let mgs: Vec<u16> = setup
        .vehicles
        .iter()
        .flat_map(|v| v.mg_mounts().iter().map(|m| m.weapon))
        .collect();
    for k in mgs {
        let w = &mut setup.weapons[k as usize];
        w.range_max = Fixed::from_int(1);
        w.engage_still = w.engage_still.min(w.range_max);
        w.engage_moving = w.engage_moving.min(w.range_max);
    }
    let faction = setup.players[p as usize].faction;
    let bait: Vec<u16> = setup
        .squad_types
        .iter()
        .enumerate()
        .filter(|&(k, t)| t.faction == faction && setup.type_has_tag(k as u16, "line"))
        .flat_map(|(_, t)| {
            t.members
                .iter()
                .flat_map(|m| std::iter::once(m.weapon).chain(m.sidearm))
                .collect::<Vec<_>>()
        })
        .collect();
    for k in bait {
        let w = &mut setup.weapons[k as usize];
        w.damage = 0;
        w.suppression = 0;
    }
    let mut s = Staged::new(new_world(setup, threads)?);
    let w = &mut s.world;
    if trench {
        s.line = trench_ns(w).to_vec();
    }
    let rifles = need(
        spawn(w, 1 - p, SquadRole::Rifle, at(TRENCH_X, LANE_Y), EAST),
        "rifles",
    )?;
    let bait = need(
        spawn(
            w,
            p,
            SquadRole::Rifle,
            at(TRENCH_X + 140, LANE_Y + 40),
            WEST,
        ),
        "bait",
    )?;
    let t = need(tank(w, p, at(TRENCH_X + 100, LANE_Y), WEST), "tank")?;
    w.step(&[cmd(
        p,
        1,
        CommandKind::SetAmmo {
            units: vec![t.1],
            ammo: AmmoChoice::He,
        },
    )]);
    s.squads = vec![rifles, bait];
    s.tanks.push(t);
    Ok(s)
}

/// Case 10: player `p`'s mortar team 250 m east of an enemy tank at
/// `(TRENCH_X, LANE_Y)`. Squads: `[mortar]`; tanks: `[tank]`.
pub fn mortar_tank(mut setup: MatchSetup, threads: usize, p: u8) -> Result<Staged, SimError> {
    staging(&mut setup, true);
    let mut s = Staged::new(new_world(setup, threads)?);
    let w = &mut s.world;
    let t = need(tank(w, 1 - p, at(TRENCH_X, LANE_Y), EAST), "tank")?;
    let m = need(
        spawn(w, p, SquadRole::Mortar, at(TRENCH_X + 250, LANE_Y), WEST),
        "mortar",
    )?;
    s.squads.push(m);
    s.tanks.push(t);
    Ok(s)
}

/// Case 11: an enemy rifle squad in a straight east–west trench (parapet
/// north) through `(TRENCH_X, LANE_Y)`; player `p`'s tank 50 m off its
/// east end looking along it (`end_on`), or 50 m in front of it (north).
/// Squads: `[rifles]`; tanks: `[tank]`; line: the trench.
pub fn enfilade(
    mut setup: MatchSetup,
    threads: usize,
    p: u8,
    end_on: bool,
) -> Result<Staged, SimError> {
    staging(&mut setup, true);
    let mut s = Staged::new(new_world(setup, threads)?);
    let w = &mut s.world;
    let line = [at(TRENCH_X - 20, LANE_Y), at(TRENCH_X + 20, LANE_Y)];
    w.dig_trench_now(&line, 1);
    s.line = line.to_vec();
    let rifles = need(
        spawn(w, 1 - p, SquadRole::Rifle, at(TRENCH_X, LANE_Y), NORTH),
        "rifles",
    )?;
    let t = if end_on {
        need(tank(w, p, at(TRENCH_X + 70, LANE_Y), WEST), "tank")?
    } else {
        need(tank(w, p, at(TRENCH_X, LANE_Y + 50), SOUTH), "tank")?
    };
    s.squads.push(rifles);
    s.tanks.push(t);
    Ok(s)
}

/// Case 12: player `p`'s tank and a finished north–south trench (a second
/// spot 200 m south of it is open ground for the timing baseline). With
/// `mortar`, an own mortar team stands 250 m east to crater the trench
/// first. Tanks: `[tank]`; squads: `[mortar?]`; line: the trench.
pub fn crossing(
    mut setup: MatchSetup,
    threads: usize,
    p: u8,
    mortar: bool,
) -> Result<Staged, SimError> {
    staging(&mut setup, true);
    let mut s = Staged::new(new_world(setup, threads)?);
    let w = &mut s.world;
    s.line = trench_ns(w).to_vec();
    let t = need(tank(w, p, at(TRENCH_X + 20, LANE_Y), WEST), "tank")?;
    s.tanks.push(t);
    if mortar {
        let m = need(
            spawn(w, p, SquadRole::Mortar, at(TRENCH_X + 250, LANE_Y), WEST),
            "mortar",
        )?;
        s.squads.push(m);
    }
    Ok(s)
}

/// Case 13: a 30 m AT ditch north–south through `(TRENCH_X, LANE_Y)`
/// (berm west), player `p`'s tank 40 m east of it. Tanks: `[tank]`; line:
/// the ditch.
pub fn at_ditch(mut setup: MatchSetup, threads: usize, p: u8) -> Result<Staged, SimError> {
    staging(&mut setup, true);
    let mut s = Staged::new(new_world(setup, threads)?);
    let w = &mut s.world;
    let line = vec![at(TRENCH_X, LANE_Y - 15), at(TRENCH_X, LANE_Y + 15)];
    if !w.dig_at_ditch_now(&line, 1) {
        return Err(SimError::InvalidConfig("staging: AT ditch".into()));
    }
    s.line = line;
    s.tanks
        .push(need(tank(w, p, at(TRENCH_X + 40, LANE_Y), WEST), "tank")?);
    Ok(s)
}

/// Case 14 (`--stress`): Kessel Ridge's real terrain with the limits
/// lifted (50 vehicles per player, pop cap 2000): 50 tanks and 300
/// riflemen (50 squads of 6) per side, facing each other across the
/// ridge. The runner keeps them moving. Tanks and squads in spawn order.
pub fn stress(mut setup: MatchSetup, threads: usize) -> Result<Staged, SimError> {
    staging(&mut setup, false);
    setup.rules.vehicles.limit = 50;
    setup.rules.pop_cap = 2000;
    let mut s = Staged::new(new_world(setup, threads)?);
    let w = &mut s.world;
    for p in 0..2u8 {
        let (x0, f) = if p == 0 { (250, EAST) } else { (650, WEST) };
        for k in 0..50 {
            let pos = at(x0 + 10 * (k % 10), 300 + 40 * (k / 10));
            s.tanks.push(need(tank(w, p, pos, f), "tank")?);
        }
        let men = w.match_setup().map_or(6, |st| {
            type_of(w, p, SquadRole::Rifle)
                .map_or(6, |t| st.squad_types[t as usize].men.max(1) as i32)
        });
        let n = (300 + men - 1) / men;
        for k in 0..n {
            let pos = at(x0 + 12 * (k % 7), 520 + 22 * (k / 7));
            s.squads
                .push(need(spawn(w, p, SquadRole::Rifle, pos, f), "rifles")?);
        }
    }
    Ok(s)
}

// ------------------------------------------------------- Godot scenes --

/// The staged autoplay scenes (`--autoplay_scene armour_<name>`).
pub const SCENES: [&str; 13] = [
    "callin_locked",
    "vehicle_card",
    "tank_firing",
    "penetration_marker",
    "bogged_in_trench",
    "at_ditch",
    "at_team_firing",
    "burning",
    "escort",
    // S3c: a tank abandoned (gun and a track out) with its crew beside it.
    "bailout",
    // S6: a dug-in HQ with stock, its Zugtrupp, a mortar in its ring and
    // ammunition bearers carrying shells crates up to it.
    "hq_supply",
    // S7: a StuG III (casemate) and an M10 (open-topped turret) side by
    // side, stopped and not firing (the blockouts for the screenshot).
    "casemate_td",
    // S8a: a loaded SdKfz 251 and a loaded M3A1 side by side, stopped,
    // their MGs dry (the blockouts and seated riders for the screenshot).
    "halftracks",
];

/// A staged scene: the world, the commands to send on the first step, the
/// most ticks to run before its condition must hold, and where to point
/// the camera. Player 0 is the viewer.
pub struct Scene {
    pub world: World,
    pub commands: Vec<Command>,
    pub max_ticks: u64,
    pub focus: FxVec2,
}

/// Builds scene `name` (one of [`SCENES`]) from a resolved setup. Every
/// scene is flat staging ground with vehicles unlocked, except
/// `callin_locked`, which keeps the setup's terrain and sets a 4:00
/// unlock (the condition is checked at once).
pub fn scene(name: &str, mut setup: MatchSetup, threads: usize) -> Result<Scene, SimError> {
    let locked = name == "callin_locked";
    staging(&mut setup, !locked);
    if locked {
        setup.rules.vehicles.unlock_ticks = 240 * setup.tick_hz;
    }
    let mut w = new_world(setup, threads)?;
    let mut commands = Vec::new();
    let mut focus = at(TRENCH_X, LANE_Y);
    let max_ticks;
    match name {
        "callin_locked" => {
            focus = w.game().map_or(focus, |g| g.base_of(PlayerId(0)));
            max_ticks = 1;
        }
        "vehicle_card" => {
            let t = need(tank(&mut w, 0, at(TRENCH_X, LANE_Y), EAST), "tank")?;
            commands.push(cmd(
                0,
                1,
                CommandKind::SetAmmo {
                    units: vec![t.1],
                    ammo: AmmoChoice::He,
                },
            ));
            max_ticks = 30;
        }
        "tank_firing" | "penetration_marker" => {
            let kind = if name == "tank_firing" {
                Duel::Frontal300
            } else {
                Duel::Flank150
            };
            let (pz_at, pz_f, sh_at, sh_f) = match kind {
                Duel::Flank150 => (500, NORTH, 350, EAST),
                _ => (350, EAST, 650, WEST),
            };
            need(tank(&mut w, 0, at(pz_at, LANE_Y), pz_f), "Panzer IV")?;
            need(tank(&mut w, 1, at(sh_at, LANE_Y), sh_f), "Sherman")?;
            max_ticks = 30 * 60;
        }
        "bogged_in_trench" => {
            trench_ns(&mut w);
            let t = need(tank(&mut w, 0, at(TRENCH_X, LANE_Y), WEST), "tank")?;
            let ticks = w
                .match_setup()
                .map_or(0, |s| s.vehicles[0].mobility.unbog_ticks[1].max(30 * 20));
            w.edit_vehicle(t.1, |vs| {
                vs.bog = ticks;
                vs.crossing = true;
            });
            max_ticks = 30;
        }
        "at_ditch" => {
            let line = vec![at(TRENCH_X, LANE_Y - 15), at(TRENCH_X, LANE_Y + 15)];
            w.dig_at_ditch_now(&line, 1);
            need(tank(&mut w, 0, at(TRENCH_X + 40, LANE_Y), WEST), "tank")?;
            need(
                spawn(
                    &mut w,
                    0,
                    SquadRole::Engineer,
                    at(TRENCH_X + 20, LANE_Y + 25),
                    WEST,
                ),
                "engineers",
            )?;
            max_ticks = 30;
        }
        "at_team_firing" => {
            trench_ns(&mut w);
            need(
                spawn(&mut w, 0, SquadRole::AntiTank, at(TRENCH_X, LANE_Y), EAST),
                "AT team",
            )?;
            need(tank(&mut w, 1, at(TRENCH_X + 70, LANE_Y), NORTH), "tank")?;
            max_ticks = 30 * 60;
        }
        "burning" => {
            let t = need(tank(&mut w, 0, at(TRENCH_X, LANE_Y), EAST), "tank")?;
            w.edit_vehicle(t.1, |vs| vs.burn = 1);
            max_ticks = 30;
        }
        "bailout" => {
            // The gun and the left track out: the crew bail out (S3c
            // trigger 4) and stand beside the hull, away from the last hit
            // (none: the left side).
            let t = need(tank(&mut w, 0, at(TRENCH_X, LANE_Y), EAST), "tank")?;
            w.edit_vehicle(t.1, |vs| {
                vs.components[ComponentKind::Gun.index()] = 0;
                vs.components[ComponentKind::TrackLeft.index()] = 0;
                vs.seats[4].hp = 0;
            });
            max_ticks = 30 * 10;
        }
        "escort" => {
            let t = need(tank(&mut w, 0, at(TRENCH_X, LANE_Y), WEST), "tank")?;
            let r = need(
                spawn(&mut w, 0, SquadRole::Rifle, at(TRENCH_X + 12, LANE_Y), WEST),
                "rifles",
            )?;
            commands.push(cmd(
                0,
                1,
                CommandKind::FollowUnits {
                    units: members(&w, r),
                    vehicle: t.1,
                },
            ));
            commands.push(cmd(
                0,
                2,
                CommandKind::Move {
                    squad: t.0,
                    target: at(TRENCH_X - 150, LANE_Y),
                },
            ));
            focus = at(TRENCH_X - 20, LANE_Y);
            max_ticks = 30 * 20;
        }
        "casemate_td" => {
            use frontline_content::VehicleRole;
            let ag = need(
                vehicle_of_role(
                    &mut w,
                    0,
                    VehicleRole::AssaultGun,
                    at(TRENCH_X, LANE_Y + 6),
                    EAST,
                ),
                "assault gun",
            )?;
            let td = need(
                vehicle_of_role(
                    &mut w,
                    1,
                    VehicleRole::TankDestroyer,
                    at(TRENCH_X + 4, LANE_Y - 6),
                    dir_deg(135),
                ),
                "tank destroyer",
            )?;
            // Hold fire: no rounds aboard either.
            for v in [ag.1, td.1] {
                w.edit_vehicle(v, |vs| {
                    vs.ammo.iter_mut().for_each(|n| *n = 0);
                    vs.loaded = None;
                });
            }
            focus = at(TRENCH_X + 2, LANE_Y);
            max_ticks = 30 * 5;
        }
        "halftracks" => {
            use frontline_content::VehicleRole;
            let a = need(
                vehicle_of_role(
                    &mut w,
                    0,
                    VehicleRole::Transport,
                    at(TRENCH_X, LANE_Y + 5),
                    EAST,
                ),
                "German half-track",
            )?;
            let b = need(
                vehicle_of_role(
                    &mut w,
                    1,
                    VehicleRole::Transport,
                    at(TRENCH_X + 3, LANE_Y - 5),
                    dir_deg(160),
                ),
                "US half-track",
            )?;
            for (p, v, back) in [
                (0u8, a.1, at(TRENCH_X - 14, LANE_Y + 5)),
                (1, b.1, at(TRENCH_X + 16, LANE_Y - 9)),
            ] {
                // The squad that fits: the first line squad of the roster
                // no larger than the slots.
                let s = w.match_setup().expect("match");
                let cap = w
                    .game()
                    .and_then(|g| g.vehicle(v))
                    .map_or(0, |vs| vs.riders.len());
                let f = s.players[p as usize].faction;
                let t = s.factions[f as usize]
                    .roster
                    .iter()
                    .copied()
                    .find(|&t| {
                        let ty = &s.squad_types[t as usize];
                        s.type_word(t) == "rifle"
                            && ty.vehicle.is_none()
                            && (ty.men as usize) <= cap
                    })
                    .ok_or_else(|| SimError::InvalidConfig("no squad fits".into()))?;
                let sq = w
                    .spawn_squad_at(PlayerId(p), t, back, EAST)
                    .ok_or_else(|| SimError::InvalidConfig("squad not placed".into()))?;
                if !w.seat_now(sq, v) {
                    return Err(SimError::InvalidConfig("squad not seated".into()));
                }
                // Hold fire: the MGs dry.
                w.edit_vehicle(v, |vs| {
                    vs.mgs.iter_mut().for_each(|m| m.spent = u32::MAX / 2)
                });
            }
            focus = at(TRENCH_X + 1, LANE_Y);
            max_ticks = 30 * 5;
        }
        "hq_supply" => {
            let s = w.match_setup().expect("match");
            let f = s.players[0].faction;
            let hq = s
                .stock_of(f, frontline_content::setup::StockKind::Hq, None)
                .ok_or_else(|| SimError::InvalidConfig("no HQ def".into()))?;
            let hq_at = at(TRENCH_X, LANE_Y);
            w.build_structure_now(PlayerId(0), hq, hq_at, Fx::ZERO, [140, 60, 380])
                .ok_or_else(|| SimError::InvalidConfig("HQ not placed".into()))?;
            need(
                spawn_tag(&mut w, 0, "hq", at(TRENCH_X + 8, LANE_Y + 6), EAST),
                "HQ squad",
            )?;
            need(
                spawn(
                    &mut w,
                    0,
                    SquadRole::Mortar,
                    at(TRENCH_X - 6, LANE_Y - 12),
                    EAST,
                ),
                "mortar",
            )?;
            let b = need(
                spawn_tag(&mut w, 0, "bearers", at(TRENCH_X - 70, LANE_Y + 10), EAST),
                "bearers",
            )?;
            // Two crates already in hand (issued in the base, staged here).
            let shells = w
                .match_setup()
                .and_then(|s| {
                    s.stock_of(
                        f,
                        frontline_content::setup::StockKind::Crate,
                        Some(frontline_content::setup::SupplyKind::Shells),
                    )
                })
                .ok_or_else(|| SimError::InvalidConfig("no shells crate def".into()))?;
            let men = members(&w, b);
            let (g, hp) = {
                let d = &w.match_setup().expect("match").stocks[shells as usize];
                (d.g, d.boxes[0].hp)
            };
            w.edit_supply6(|st| {
                for &m in men.iter().take(2) {
                    let id = st.next_crate;
                    st.next_crate += 1;
                    st.crates.insert(
                        id,
                        crate::structures::Crate {
                            owner: PlayerId(0),
                            def: shells,
                            pos: at(TRENCH_X - 70, LANE_Y + 10),
                            carrier: Some(m),
                            dest: None,
                            claim: None,
                            g,
                            hp,
                        },
                    );
                }
            });
            commands.push(cmd(
                0,
                1,
                CommandKind::PlaceCrate {
                    squad: b,
                    pos: at(TRENCH_X - 4, LANE_Y + 8),
                },
            ));
            focus = at(TRENCH_X - 25, LANE_Y);
            max_ticks = 30 * 30;
        }
        _ => return Err(SimError::InvalidConfig(format!("unknown scene {name}"))),
    }
    Ok(Scene {
        world: w,
        commands,
        max_ticks,
        focus,
    })
}

/// Whether scene `name`'s snapshot condition holds now (the autoplay
/// asserts it before its screenshot).
pub fn scene_ready(name: &str, w: &World) -> bool {
    let snap = w.snapshot();
    let own = |f: u16| {
        snap.vehicles
            .iter()
            .any(|v| v.owner == 0 && v.flags & f != 0)
    };
    match name {
        "callin_locked" => snap
            .players
            .first()
            .is_some_and(|p| w.tick() < p.unlock_tick),
        "vehicle_card" => snap
            .vehicles
            .iter()
            .any(|v| v.owner == 0 && v.ammo_choice == AmmoChoice::He as u8),
        "tank_firing" => snap
            .armour_events
            .iter()
            .any(|e| e.kind == crate::snapshot::ArmourEventKind::Fired as u8),
        "penetration_marker" => snap
            .armour_events
            .iter()
            .any(|e| e.kind == crate::snapshot::ArmourEventKind::Penetration as u8),
        "bogged_in_trench" => own(vehicle_flags::BOGGED),
        "at_ditch" => w.game().is_some_and(|g| !g.at_ditches().is_empty()),
        "at_team_firing" => snap
            .shells
            .iter()
            .any(|s| s.player == 0 && s.kind == crate::snapshot::ShellKind::Heat as u8),
        "burning" => own(vehicle_flags::BURNING),
        // S3c: abandoned, every living man out.
        "bailout" => snap.vehicles.iter().any(|v| {
            v.last_owner == 0
                && v.flags & vehicle_flags::ABANDONED != 0
                && v.inside == 0
                && v.crew_squad != crate::snapshot::NO_SQUAD
        }),
        "escort" => snap
            .vehicles
            .iter()
            .any(|v| v.owner == 0 && v.followers > 0),
        // S7: both vehicles alive and drawn.
        "casemate_td" => {
            snap.vehicles
                .iter()
                .filter(|v| v.flags & vehicle_flags::WRECK == 0)
                .count()
                >= 2
                && w.tick() >= 30
        }
        // S8a: both half-tracks alive with men aboard.
        "halftracks" => {
            snap.vehicles
                .iter()
                .filter(|v| v.flags & vehicle_flags::WRECK == 0 && v.riders > 0)
                .count()
                >= 2
                && w.tick() >= 30
        }
        // S6: a built own HQ with stock, and a crate on a man's back.
        "hq_supply" => {
            snap.structures
                .iter()
                .any(|s| s.owner == 0 && s.built && s.stock_kg[2] > 0.0)
                && snap
                    .crates
                    .iter()
                    .any(|c| c.owner == 0 && c.carrier != u32::MAX)
        }
        _ => false,
    }
}

/// Tracks of live vehicles that a blast at `pos` with `lethal` radius must
/// break by the rule of `damage::blast_tracks`: an anchor of the track
/// within the radius (less `margin`, against rounding of a reported
/// position) and in clear line from just above the blast point. In
/// ascending vehicle id (case 8's hard check).
pub fn tracks_in_blast(
    w: &World,
    pos: FxVec3,
    lethal: Fx,
    margin: Fx,
) -> Vec<(SimId, ComponentKind)> {
    let Some(g) = w.game() else {
        return Vec::new();
    };
    let s = g.setup();
    let lift = FxVec3::new(
        pos.x,
        pos.y,
        pos.z + Fx::from_milli(s.rules.combat.blast.lift_mm as i32),
    );
    let reach = lethal - margin;
    let mut out = Vec::new();
    for (&id, vs) in g.vehicles() {
        if w.entities().kind()[id.index()] != EntityKind::Vehicle {
            continue;
        }
        let Some(f) = g.vehicle_derived().frame(id) else {
            continue;
        };
        let v = &s.vehicles[vs.type_idx as usize];
        for kind in [ComponentKind::TrackLeft, ComponentKind::TrackRight] {
            let hit = v.components.iter().filter(|c| c.kind == kind).any(|c| {
                c.anchors.iter().any(|a| {
                    let q = crate::damage::anchor_at(f, a.frame, crate::vehicle::v3(a.pos));
                    (q - pos).length() <= reach && w.terrain().line_of_sight(lift, q)
                })
            });
            if hit {
                out.push((id, kind));
            }
        }
    }
    out
}

/// The 3D distance from `p` to the nearest live vehicle's track anchors
/// of `kind` (diagnostics for case 8's hard check): `(vehicle, distance)`.
pub fn nearest_track(w: &World, p: FxVec3, kind: ComponentKind) -> Option<(SimId, Fx)> {
    let g = w.game()?;
    let s = g.setup();
    let mut best: Option<(SimId, Fx)> = None;
    for (&id, vs) in g.vehicles() {
        if w.entities().kind()[id.index()] != EntityKind::Vehicle {
            continue;
        }
        let Some(f) = g.vehicle_derived().frame(id) else {
            continue;
        };
        let v = &s.vehicles[vs.type_idx as usize];
        for c in v.components.iter().filter(|c| c.kind == kind) {
            for a in &c.anchors {
                let q = crate::damage::anchor_at(f, a.frame, crate::vehicle::v3(a.pos));
                let d = (q - p).length();
                if best.is_none_or(|(_, b)| d < b) {
                    best = Some((id, d));
                }
            }
        }
    }
    best
}
