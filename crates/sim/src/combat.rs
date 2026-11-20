//! Combat (S1 "Combat", "Suppression", "HE"; §5.7, §5.8, §5.10.4).
//!
//! Match-mode work per tick, in `World::step` stage order (spotting is in
//! `crate::spotting`):
//!
//! * **Weapons** (stage 4).
//!   1. *Marks:* a serial pass lists every live soldier with his feet, aim
//!      point and `seen_by` bits, so the parallel part reads no column it
//!      also writes.
//!   2. *Squad focus* (serial, ascending squad id): each squad that may
//!      engage keeps its focus target while it stays seen and in range of
//!      the leader, else takes the nearest such enemy.
//!   3. *Aim and fire* (per entity chunk, in parallel): a soldier may engage
//!      by his squad's order (`Idle` and `AttackMove` always; `Move` and `Dig`
//!      only while he stands still; `Retreat` and mortar crews never). He
//!      drops a target that is dead, unseen or out of range, and searches
//!      again when he has none or once a second: the squad focus first, then
//!      the nearest seen enemies in range, at most [`LOS_TRIES`] LOS rays
//!      from his firing muzzle (a ray through smoke counts as blocked,
//!      `crate::items`). A man winding up a throw does not fire. Opening
//!      fire costs half a second of aiming. S8a (6.4): a man whose weapon
//!      has a penetration table may instead pick a nearer seen enemy vehicle
//!      ([`vehicle_aim`]: an exposed crewman's head, else a plate in the
//!      load-time reach table); such rounds take the plate rule in the
//!      Damage stage (`crate::damage`), the rest stop on any plate.
//!      When his cooldown is up and his weapon is set up (MG: still for
//!      `setup_ticks`), he fires a real round at the target's aim point,
//!      compensated for drop, with angular error from the weapon spread
//!      scaled by movement, suppression or pin, and range.
//!      MGs ignore the focus and pick among the four nearest (seeded), so
//!      their bursts sweep and suppress a whole squad; set up, they lie
//!      prone.
//!   4. *Mortars* (serial): a squad on a FireMission order, holding, set up
//!      and in range fires one shell per interval (S5b: after laying, the
//!      adjusting rounds one at a time, then fire for effect; the mode per
//!      round, `crate::fire_mission`) at the target plus the mission's aim
//!      offset, scattered by the precision spread, on the high-angle arc of
//!      the lowest charge that reaches (`crate::indirect`; legacy weapons:
//!      flight time growing with range, about 60°). The order ends with its
//!      last round or bomb. Holding out of range, the team
//!      walks to 90 % of its range again, or drops the order when it already
//!      tried that spot (unreachable) or the target is inside the minimum.
//!   5. Requests are spawned in sorted order, HE shells first; small arms
//!      may not take the pool's last `SHELL_RESERVE` slots. Dropped rounds are
//!      counted (`Stats::shots_dropped`).
//! * **Ballistics** (stage 5, per projectile chunk): each 1/60 s substep of
//!   a small-arms round tests the swept segment against the hit capsules of
//!   enemy soldiers near it (spatial hash), then the terrain up to the
//!   nearest capsule hit, so earth (a parapet, a trench lip) blocks first.
//!   Enemies within `near_miss` of the segment (or of a ground impact) are
//!   near-missed. Shells only test terrain and burst where they land.
//! * **Damage** (stage 6, serial, in projectile slot order): hits take hit
//!   points and add suppression; near misses add the weapon's suppression
//!   (`combat.suppression.covered_near_miss_pct` of it for a man crouched
//!   below cover). A burst (S3b, physical-contact rule) deals its full
//!   damage as overpressure to soldiers within `blast_radius` whose body
//!   point is in clear line from the blast point (+`combat.blast.lift_mm`),
//!   then throws `frag_count` fragment rays that strike the first man,
//!   vehicle, wreck or earth in their way (`MatchState::fragments`; S8a:
//!   a fragment pierces a plate thinner than its `frag_pen_mm` there);
//!   it suppresses to the suppression radius (scaled by
//!   `combat.suppression.blast_behind_cover_pct` behind earth) and
//!   carves a crater. Thrown frags and satchels that go off this tick burst
//!   the same way with their own damage, suppression and crater (a satchel's
//!   levels trenches), after the projectile events, in item id order; smoke
//!   items start their cloud. Soldiers at 0 HP die in ascending id: they become
//!   corpses, leave their squad (squads left empty are deleted) and count
//!   as a loss and a kill.
//! * **Status** (stage 7, per entity chunk): suppression decays; stance is
//!   set (pinned → prone; in a trench → crouched below the lip, or lying
//!   on the floor when ordered prone, standing on the fire step while
//!   aiming or winding up a throw; elsewhere by the man's stance order (S1b): `Auto` → a set-up
//!   MG prone, suppressed crouched, else standing; `Stand` → standing,
//!   crouched when suppressed; `Crouch` and `Prone` as ordered) with
//!   the matching capsule heights; the stillness and shot timers advance;
//!   corpses count down and are removed.
//!
//! Detached men (S1b, `crate::unit`) engage by their own order (`Idle` and
//! `AttackMove` always, `Move` and `Dig` while still, `Retreat` never) and
//! ignore the squad focus; focus and mortar fire use the formation leader.
//!
//! **Launchers (S3).** An AT team ([`crate::game::Launcher`]) has one
//! launcher, carried by its first man; the others use the squad type's
//! sidearm (as do men past `weapon_men` in other squads, [`weapon_of`]),
//! and the squad focus uses the sidearm's range. After the soldiers' aim
//! and fire, serially in ascending squad id, the carrier
//! ([`MatchState::fire_launchers`]) engages only enemy vehicles he sees,
//! within the weapon's `engage_still` (or `engage_moving` for a vehicle
//! faster than the rules' moving speed): the `AttackUnit` vehicle first,
//! then the nearest, at most [`LOS_TRIES`] tried for a clear line of fire to
//! the centre of its visible mass, else its turret (S3b,
//! `crate::gunnery::aim_choice`). Opening fire costs half a second
//! of aiming (up on the fire step in a trench, as for any aiming man); the
//! rocket leads the target by his estimate of its averaged velocity
//! (`crate::gunnery::lead_estimate`), gets the weapon's
//! spread scaled as for small arms, and takes a shell slot. The next one
//! comes after the weapon's reload, twice that with no second man left.
//! When the carrier falls, the launcher waits `handover` and the first
//! living man takes it up. Rockets come back one per `items.resupply` while
//! the carrier stands in the own base. Firing reveals a man by his weapon's
//! `fire_reveal_pct` for `fire_reveal_ticks` (`crate::spotting`).
//!
//! No friendly fire in S1: rounds and blasts ignore the firing player's own
//! soldiers. Rounds have no drag, so the drop compensation is exact.

use frontline_content::setup::{CombatSetup, HeSetup, WeaponSetup};
use frontline_content::tuning::BodyTuning;
use frontline_content::vehicle::{AmmoMechanism, AmmoSetup};
use frontline_content::{ComponentKind, ItemKind, MatchSetup, WeaponClass};

use crate::armour::{self, ArmourHit, ArmourTables};
use crate::damage::{ArmourEv, Dealt, FireRolls, Inner};
use crate::ecs::{
    Combat, Entities, EntityKind, PlayerId, SimId, Stance, Transform, Velocity, Weapon,
};
use crate::fx::{Fx, FxVec2, FxVec3};
use crate::game::{MatchState, NavPhase, Order, PathReq, Squad, SquadId, kill_class};
use crate::hash::HashCache;
use crate::items::{BlastKind, Kit, Thrown};
use crate::projectile::{NO_AMMO, Projectile, ProjectileEnd, SUBSTEP_HZ, advance, finish};
use crate::rng::{Purpose, RngStream};
use crate::spatial::SpatialHash;
use crate::systems::FireRequest;
use crate::terrain::Heightfield;
use crate::unit::{FireMode, StanceOrder, Unit, UnitOrder};
use crate::vehicle::{VehicleDerived, VehicleFrame};
use crate::world::Stats;

// S3b: body geometry (`[combat.body]`), mortar laying (`[combat.mortar]`),
// bursts (`[combat.blast]`) and suppression details
// (`[combat.suppression]`) are rules data (`frontline_content::tuning`).

/// Most LOS rays per target search.
pub const LOS_TRIES: usize = 2;
/// MGs sweep: each search picks among this many nearest enemies (seeded),
/// ignoring the squad focus, so their fire suppresses a whole squad.
const SWEEP_POOL: usize = 4;
/// 9.81 m/s².
const GRAVITY: Fx = Fx::from_ratio(981, 100);

/// Whether a squad's order lets its men open fire.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum Engage {
    Never,
    /// Only men standing still.
    Still,
    Always,
}

pub(crate) fn engage(order: &Order) -> Engage {
    match order {
        Order::Retreat | Order::FireMission { .. } => Engage::Never,
        Order::Move { .. } | Order::Dig { .. } => Engage::Still,
        Order::Idle | Order::AttackMove { .. } | Order::Follow { .. } => Engage::Always,
        // S3c: walking to an abandoned vehicle and climbing in; S8a: to a
        // transport's door (riders never fire, `fire_chunk`).
        Order::Recrew { .. } | Order::Embark { .. } => Engage::Still,
        // Holding; area fire replaces target search (`aim_and_fire`).
        Order::Suppress { .. } => Engage::Still,
        // S10a: building or cutting, as digging.
        Order::Fortify { .. }
        | Order::CutWire { .. }
        | Order::ClearMines { .. }
        | Order::Demolish { .. } => Engage::Still,
    }
}

/// Engage rule of a soldier: his own order if detached, else his squad's.
pub(crate) fn engage_of(squad: &Order, u: &Unit) -> Engage {
    if !u.detached {
        return engage(squad);
    }
    match u.order {
        UnitOrder::Retreat => Engage::Never,
        UnitOrder::Move { .. } | UnitOrder::Dig { .. } => Engage::Still,
        UnitOrder::Idle | UnitOrder::AttackMove { .. } | UnitOrder::Follow { .. } => Engage::Always,
        UnitOrder::Suppress { .. } => Engage::Still,
    }
}

/// S3b: the line a soldier on a Suppress order fires at (his own order if
/// detached, else his squad's).
pub(crate) fn suppress_of(squad: &Order, u: &Unit) -> Option<[FxVec2; 2]> {
    match (u.detached, squad, &u.order) {
        (false, Order::Suppress { line }, _) | (true, _, UnitOrder::Suppress { line }) => {
            Some(*line)
        }
        _ => None,
    }
}

/// S3b: a suppression threshold (points) scaled by the squad type's
/// `steadiness_pct`.
#[inline]
pub(crate) fn steady(points: u32, steadiness_pct: u32) -> u32 {
    (points as u64 * steadiness_pct as u64 / 100).min(u32::MAX as u64) as u32
}

/// S3b Suppress: the aim point of a shot at `line` (a point on it at
/// fraction `t` of its length, `aim_cm` above the higher of the ground
/// and the undug surface there, so a trench line is aimed at over its
/// parapet), and the probe point `clear_cm` short of it toward `from`
/// (lifted as high above the ground there) that the line of fire must
/// reach. `None` beyond `range`.
pub(crate) fn suppress_aim(
    terrain: &Heightfield,
    rules: &CombatSetup,
    line: &[FxVec2; 2],
    t: Fx,
    from: FxVec3,
    range: Fx,
) -> Option<(FxVec3, FxVec3)> {
    suppress_aim_at(terrain, rules, line, t, from, range, rules.suppress.aim_cm)
}

/// [`suppress_aim`] with the aim height `aim_cm` given (a vehicle's HE).
pub(crate) fn suppress_aim_at(
    terrain: &Heightfield,
    rules: &CombatSetup,
    line: &[FxVec2; 2],
    t: Fx,
    from: FxVec3,
    range: Fx,
    aim_cm: u32,
) -> Option<(FxVec3, FxVec3)> {
    let s = &rules.suppress;
    let p = line[0] + (line[1] - line[0]) * t;
    let lift = Fx::from_ratio(aim_cm as i64, 100);
    let ground = |q: FxVec2| terrain.height_at(q).max(terrain.base_height_at(q));
    let aim = FxVec3::new(p.x, p.y, ground(p) + lift);
    if (aim.xy() - from.xy()).length_sq_raw() > range_sq(range) {
        return None;
    }
    let back = (from.xy() - p).normalize_or_zero() * Fx::from_ratio(s.clear_cm as i64, 100);
    let q = terrain.clamp(p + back);
    let probe = FxVec3::new(q.x, q.y, aim.z.max(ground(q) + lift));
    Some((aim, probe))
}

/// The squad's small-arms weapon (focus range; S4: the squad type's
/// `small_arms`, its longest-reaching non-crew weapon).
pub(crate) fn small_arms_of(setup: &MatchSetup, sq: &Squad) -> u16 {
    setup.squad_types[sq.type_idx as usize].small_arms
}

/// A live soldier as other soldiers see him this tick.
#[derive(Copy, Clone, Debug)]
pub(crate) struct Mark {
    pub id: SimId,
    pub owner: u8,
    pub pos: FxVec3,
    pub aim: FxVec3,
    pub seen_by: u16,
}

/// Where shooters aim: the middle of the upper body, the head and
/// shoulders of a man on a fire step, the middle of a prone man.
pub(crate) fn aim_point(pos: FxVec3, c: &Combat, body: &BodyTuning) -> FxVec3 {
    let dz = match c.stance {
        Stance::Prone => c.height.div_int(2),
        _ if c.in_trench => c.height - Fx::from_milli(body.head_aim_mm as i32),
        _ => c.height.mul_int(3).div_int(5),
    };
    FxVec3::new(pos.x, pos.y, pos.z + dz)
}

/// Middle of the body (blast damage target).
fn body_point(pos: FxVec3, c: &Combat) -> FxVec3 {
    FxVec3::new(pos.x, pos.y, pos.z + c.height.div_int(2))
}

/// S8a: a live vehicle as soldiers see it this tick (small arms against
/// thin armour and exposed crews, vehicles.md 6.4).
#[derive(Copy, Clone, Debug)]
pub(crate) struct VMark {
    pub id: SimId,
    pub owner: u8,
    pub seen_by: u16,
}

/// Lists live (manned, not abandoned) vehicles, ascending id.
pub(crate) fn build_vmarks(ents: &Entities, out: &mut Vec<VMark>) {
    out.clear();
    for i in 0..ents.len() {
        if ents.kind[i] == EntityKind::Vehicle {
            out.push(VMark {
                id: SimId(i as u32),
                owner: ents.owner[i].0,
                seen_by: ents.combat[i].seen_by,
            });
        }
    }
}

/// S8a: where soldier fire from `from` (player `owner`, weapon `wi`, which
/// must have a penetration table, S8a-18) aims at vehicle mark `m` within
/// `r` (range², Q32), if it may: (a) the head of a living crewman whose
/// seat rises over an open box's rim (S7-7), then (S8a-9) of a passenger
/// whose slot does, else (b)
/// the centre of the facing plate of the box whose plate the weapon beats
/// at this range (the load-time reach table: one comparison).
pub(crate) fn vehicle_aim(
    game: &MatchState,
    owner: u8,
    from: FxVec3,
    wi: u16,
    m: &VMark,
    r: u64,
) -> Option<FxVec3> {
    // S8a-18: only a weapon with a penetration table picks vehicles.
    let reach_tab = game.armour.reach.get(wi as usize)?;
    if reach_tab.is_empty() || m.owner == owner || m.seen_by & (1 << owner) == 0 {
        return None;
    }
    let f = game.vderived.frame(m.id)?;
    let d_sq = (f.hull.center.xy() - from.xy()).length_sq_raw();
    if d_sq > r {
        return None;
    }
    let vs = game.vehicles.get(&m.id)?;
    let plates = game.armour.vehicles.get(vs.type_idx as usize)?;
    for e in plates.exposed.iter().filter(|e| e.above_rim) {
        if vs.seats.get(e.seat as usize).is_some_and(|s| s.hp > 0) {
            let sb = crate::damage::part_box(f, &e.part);
            return Some(sb.center + sb.axes.z * (sb.half.z - Fx::from_ratio(15, 100)));
        }
    }
    // S8a-9: then a passenger whose slot rises over the rim.
    if let Some(geo) = game.vderived.geo.get(vs.type_idx as usize) {
        for n in 0..f.n_riders as usize {
            let k = f.rider_slot[n] as u32;
            if k < 16 && geo.slots_above_rim & (1 << k) != 0 {
                let sb = &f.riders[n];
                return Some(sb.center + sb.axes.z * (sb.half.z - Fx::from_ratio(15, 100)));
            }
        }
    }
    let reach = reach_tab.get(vs.type_idx as usize)?;
    let mut best: Option<(Fx, FxVec3)> = None;
    for (k, b) in f.armour_boxes().iter().enumerate() {
        let l = armour::to_local(b, from - b.center);
        let face = if l.x.abs() >= l.y.abs() {
            if l.x >= Fx::ZERO {
                armour::Face::Front
            } else {
                armour::Face::Rear
            }
        } else if l.y >= Fx::ZERO {
            armour::Face::Left
        } else {
            armour::Face::Right
        };
        let z = face.zone(armour::BoxPart(k as u8));
        let rz = reach.get(z as usize).copied().unwrap_or(Fx::ZERO);
        if rz <= Fx::ZERO || range_sq(rz) < d_sq {
            continue;
        }
        let n = match face {
            armour::Face::Front => b.axes.x * b.half.x,
            armour::Face::Rear => -(b.axes.x * b.half.x),
            armour::Face::Left => b.axes.y * b.half.y,
            _ => -(b.axes.y * b.half.y),
        };
        if best.is_none_or(|(br, _)| rz > br) {
            best = Some((rz, b.center + n));
        }
    }
    best.map(|(_, p)| p)
}

/// Lists live soldiers (`marks`, ascending id) and indexes them by id
/// (`mark_of`, `u32::MAX` for others).
pub(crate) fn build_marks(
    ents: &Entities,
    marks: &mut Vec<Mark>,
    mark_of: &mut Vec<u32>,
    body: &BodyTuning,
) {
    marks.clear();
    mark_of.clear();
    mark_of.resize(ents.len(), u32::MAX);
    for id in ents.soldiers() {
        let i = id.index();
        // S8a: passengers are struck through their vehicle (slot boxes),
        // never aimed at as men.
        if ents.unit[i].riding.is_some() {
            continue;
        }
        let c = &ents.combat[i];
        let pos = ents.transform[i].pos;
        mark_of[i] = marks.len() as u32;
        marks.push(Mark {
            id,
            owner: ents.owner[i].0,
            pos,
            aim: aim_point(pos, c, body),
            seen_by: c.seen_by,
        });
    }
}

#[inline]
fn range_sq(range: Fx) -> u64 {
    let r = range.raw().max(0) as u64;
    r * r
}

/// The mark of `t` if it is an enemy of `owner` that `owner` sees within
/// `range_sq` (Q32) of `from`.
fn valid_target<'a>(
    marks: &'a [Mark],
    mark_of: &[u32],
    t: SimId,
    owner: u8,
    from: FxVec2,
    range_sq: u64,
) -> Option<&'a Mark> {
    let k = *mark_of.get(t.index())?;
    let m = marks.get(k as usize)?;
    (m.owner != owner
        && m.seen_by & (1 << owner) != 0
        && (m.pos.xy() - from).length_sq_raw() <= range_sq)
        .then_some(m)
}

// ---------------------------------------------------------------- focus --

impl MatchState {
    /// Updates every squad's focus target (see the module docs).
    pub(crate) fn update_focus(&mut self, ents: &Entities, marks: &[Mark], mark_of: &[u32]) {
        let setup = &self.setup;
        for sq in self.squads.values_mut() {
            let ty = &setup.squad_types[sq.type_idx as usize];
            let w = &setup.weapons[small_arms_of(setup, sq) as usize];
            // Vehicles aim through their own gunnery (S3).
            if sq.members.is_empty()
                || ty.vehicle.is_some()
                || w.he.is_some()
                || engage(&sq.order) == Engage::Never
            {
                sq.focus = None;
                continue;
            }
            let lead = ents.transform[sq.lead(ents).index()].pos.xy();
            let owner = sq.owner.0;
            let r = range_sq(w.range_max.into());
            if sq
                .focus
                .is_some_and(|f| valid_target(marks, mark_of, f, owner, lead, r).is_some())
            {
                continue;
            }
            sq.focus = marks
                .iter()
                .filter(|m| m.owner != owner && m.seen_by & (1 << owner) != 0)
                .map(|m| ((m.pos.xy() - lead).length_sq_raw(), m.id))
                .filter(|(d, _)| *d <= r)
                .min()
                .map(|(_, id)| id);
        }
    }
}

// ----------------------------------------------------------- aim + fire --

/// Read-only inputs of [`aim_and_fire`].
pub(crate) struct FireCtx<'a> {
    pub kind: &'a [EntityKind],
    pub owner: &'a [PlayerId],
    pub transform: &'a [Transform],
    pub velocity: &'a [Velocity],
    pub squad: &'a [Option<SquadId>],
    pub unit: &'a [Unit],
    pub marks: &'a [Mark],
    pub mark_of: &'a [u32],
    /// S8a: live vehicles (small-arms targets).
    pub vmarks: &'a [VMark],
    pub game: &'a MatchState,
    pub terrain: &'a Heightfield,
    pub seed: u64,
    pub tick: u64,
}

#[inline]
fn is_still(v: &Velocity, body: &BodyTuning) -> bool {
    v.lin.length_sq_raw() <= range_sq(Fx::from_milli(body.still_mm_s as i32))
}

/// Muzzle of a soldier in his firing posture.
#[inline]
fn muzzle(pos: FxVec3, c: &Combat, body: &BodyTuning) -> FxVec3 {
    FxVec3::new(
        pos.x,
        pos.y,
        pos.z + c.fire_height - Fx::from_milli(body.muzzle_drop_mm as i32),
    )
}

/// Target search for soldier `i`: the squad focus (if any and valid),
/// then the `pool` nearest other seen enemies in range. Candidates are tried
/// from index `rot` (wrapping) and the first with a clear line from `from`
/// wins; at most [`LOS_TRIES`] rays.
fn search(
    ctx: &FireCtx,
    i: usize,
    from: FxVec3,
    r: u64,
    focus: Option<SimId>,
    pool: usize,
    rot: usize,
) -> Option<SimId> {
    let owner = ctx.owner[i].0;
    let xy = from.xy();
    // Candidates `(dist², id, mark index)`: the focus, then the nearest.
    let mut cands = [(u64::MAX, SimId(u32::MAX), 0u32); 1 + SWEEP_POOL];
    let mut n = 0usize;
    if let Some(f) = focus
        && valid_target(ctx.marks, ctx.mark_of, f, owner, xy, r).is_some()
    {
        cands[0] = (0, f, ctx.mark_of[f.index()]);
        n = 1;
    }
    let first = n;
    let cap = first + pool.min(SWEEP_POOL);
    for (k, m) in ctx.marks.iter().enumerate() {
        if Some(m.id) == focus || m.owner == owner || m.seen_by & (1 << owner) == 0 {
            continue;
        }
        let d = (m.pos.xy() - xy).length_sq_raw();
        if d > r {
            continue;
        }
        let key = (d, m.id, k as u32);
        let mut j = if n < cap {
            n += 1;
            n - 1
        } else if key < cands[cap - 1] {
            cap - 1
        } else {
            continue;
        };
        cands[j] = key;
        // Keep the nearest part sorted.
        while j > first && cands[j] < cands[j - 1] {
            cands.swap(j, j - 1);
            j -= 1;
        }
    }
    (0..n.min(LOS_TRIES)).find_map(|k| {
        let (_, id, m) = cands[(rot + k) % n];
        let aim = ctx.marks[m as usize].aim;
        (ctx.terrain.line_of_sight(from, aim)
            && !ctx
                .game
                .smoke_blocks(from, aim, ctx.tick, (id, SimId(i as u32))))
        .then_some(id)
    })
}

/// S8a: the nearest vehicle soldier `i` with weapon `wi` may fire at from
/// `from` within `r` with a clear line: `(dist², id, aim)`.
fn search_vehicle(
    ctx: &FireCtx,
    i: usize,
    from: FxVec3,
    wi: u16,
    r: u64,
) -> Option<(u64, SimId, FxVec3)> {
    // Review L1: a weapon with no penetration table skips the loop.
    if ctx.vmarks.is_empty()
        || ctx
            .game
            .armour
            .reach
            .get(wi as usize)
            .is_none_or(|t| t.is_empty())
    {
        return None;
    }
    let owner = ctx.owner[i].0;
    let mut best: Option<(u64, SimId, FxVec3)> = None;
    for m in ctx.vmarks {
        let Some(aim) = vehicle_aim(ctx.game, owner, from, wi, m, r) else {
            continue;
        };
        let d = (aim.xy() - from.xy()).length_sq_raw();
        if best.is_some_and(|(bd, bid, _)| (bd, bid) <= (d, m.id)) {
            continue;
        }
        if ctx.terrain.line_of_sight(from, aim)
            && !ctx
                .game
                .smoke_blocks(from, aim, ctx.tick, (m.id, SimId(i as u32)))
        {
            best = Some((d, m.id, aim));
        }
    }
    best
}

/// Triangular roll in `[-s, s]`.
#[inline]
pub(crate) fn tri(r: &mut RngStream, s: Fx) -> Fx {
    (r.range_fx(-s, s) + r.range_fx(-s, s)).div_int(2)
}

/// Spread of a shot in percent of the weapon's (movement, suppression or
/// pin, range).
fn spread_pct(rules: &CombatSetup, c: &Combat, moving: bool, dist: Fx, range: Fx) -> i64 {
    let mut pct: i64 = 100;
    if moving {
        pct = pct * rules.moving_spread_pct as i64 / 100;
    }
    if c.pinned {
        pct = pct * rules.pinned_spread_pct as i64 / 100;
    } else {
        let per = rules.suppression.spread_per_point_pct as i64;
        pct = pct * (100 + (c.suppression / 1000) as i64 * per / 100) / 100;
    }
    let extra =
        rules.range_spread_pct as i64 * dist.raw().max(0) as i64 / range.raw().max(1) as i64;
    pct * (100 + extra) / 100
}

/// Velocity of a round fired from `from` at `aim` (drop-compensated, then
/// turned by triangular yaw and pitch errors of up to `spread` radians).
pub(crate) fn shot_velocity(
    from: FxVec3,
    aim: FxVec3,
    speed: Fx,
    spread: Fx,
    r: &mut RngStream,
) -> FxVec3 {
    let mut d = aim - from;
    let t = d.length().div(speed);
    d.z += GRAVITY.mul(t).mul(t).div_int(2);
    let dir = d.normalize_or_zero();
    let right = FxVec3::new(-dir.y, dir.x, Fx::ZERO).normalize_or_zero();
    let up = FxVec3::new(Fx::ZERO, Fx::ZERO, Fx::ONE);
    let turned = dir + right * tri(r, spread) + up * tri(r, spread);
    turned.normalize_or_zero() * speed
}

/// Ticks until the shot after shot number `shots` (1-based): the weapon's
/// milli-tick interval spread over successive shots, so the long-run rate is
/// exact (Bresenham on the soldier's shot counter, which is hashed).
pub(crate) fn interval_after(w: &WeaponSetup, shots: u32) -> u32 {
    let n = shots.max(1) as u64;
    let i = w.interval_milli as u64;
    (n * i / 1000 - (n - 1) * i / 1000) as u32
}

/// A whole number of ticks no shorter than the interval.
fn interval_ceil(w: &WeaponSetup) -> u32 {
    w.interval_milli.div_ceil(1000)
}

/// Substeps a round of `w` lives: 1.25 × its range in flight, plus two.
pub(crate) fn round_ttl(w: &WeaponSetup) -> u16 {
    let t = Fx::from(w.range_max)
        .mul_int(5)
        .div_int(4)
        .div(w.muzzle_speed.into());
    (t.mul_int(SUBSTEP_HZ).floor_int() + 2).clamp(2, u16::MAX as i32) as u16
}

/// Targeting and small-arms fire for one entity chunk (see the module
/// docs). Writes only this chunk's combat and weapon state; fire requests go
/// to `out` in id order.
pub(crate) fn aim_and_fire(
    chunk: usize,
    ctx: &FireCtx,
    combat: &mut [Combat],
    weapon: &mut [Weapon],
    kits: &mut [Kit],
    out: &mut Vec<FireRequest>,
) -> bool {
    let base = chunk * crate::ecs::CHUNK_SIZE;
    let setup = ctx.game.setup();
    let rules = &setup.rules.combat;
    let hz = setup.tick_hz;
    let mut changed = false;
    for (j, ((c, ws), kit)) in combat
        .iter_mut()
        .zip(weapon.iter_mut())
        .zip(kits.iter_mut())
        .enumerate()
    {
        let i = base + j;
        if ctx.kind[i] != EntityKind::Soldier {
            continue;
        }
        let Some(sq) = ctx.squad[i].and_then(|s| ctx.game.squad(s)) else {
            continue;
        };
        changed = true;
        ws.cooldown = ws.cooldown.saturating_sub(1);
        c.retarget = c.retarget.saturating_sub(1);
        // S8a: a passenger does not fire from the vehicle (v1).
        if ctx.unit[i].riding.is_some() {
            c.target = None;
            continue;
        }
        // S4: every man carries his own weapon (`Kit::weapon`); S5a: his
        // slot's sidearm while his main weapon is dry.
        let ty = &setup.squad_types[sq.type_idx as usize];
        let wi = crate::supply::active_weapon(ty, kit);
        let w = &setup.weapons[wi as usize];
        // A launcher carrier fires in `fire_launchers` (S3).
        if w.class == WeaponClass::Launcher {
            continue;
        }
        // S5a: counted rounds. A dry weapon reloads from his pouches once
        // it may, else he draws his sidearm, else he holds fire.
        let feed = ty.feed_slot(w);
        if feed.is_some() && w.he.is_none() && c.mag == 0 {
            if ws.cooldown > 0 || !crate::supply::reload_dry(setup, ty, kit, c, ws) {
                c.target = None;
                continue;
            }
            continue;
        }
        // S4: his training scales his aim time, spread and reload.
        let tr = training_of(setup, sq);
        let aim_ticks = pct_of(hz / 2, tr.aim_time_pct) as u16;
        let still = is_still(&ctx.velocity[i], &rules.body);
        let u = &ctx.unit[i];
        // S3b: a ducked man keeps his target but does not fire; coming up
        // again costs the half second of aiming.
        if c.ducked {
            ws.cooldown = ws.cooldown.max(aim_ticks);
            continue;
        }
        // A man winding up a throw does not fire; S6: nor one carrying a
        // crate.
        let may = w.he.is_none()
            && kit.windup.is_none()
            && !ctx.game.supply6().carries(SimId(i as u32))
            && match engage_of(&sq.order, u) {
                Engage::Never => false,
                Engage::Still => still,
                Engage::Always => true,
            };
        if !may {
            c.target = None;
            continue;
        }
        let owner = ctx.owner[i].0;
        let pos = ctx.transform[i].pos;
        // S3b Suppress: area fire at the line, no target search.
        if let Some(line) = suppress_of(&sq.order, u) {
            c.target = None;
            // S5a: a Suppress order fires to full range, except in hold.
            if u.fire == FireMode::Hold || ws.cooldown > 0 || (c.still as u32) < w.setup_ticks {
                continue;
            }
            let id = SimId(i as u32);
            let from = muzzle(pos, c, &rules.body);
            let range: Fx = w.range_max.into();
            let t = RngStream::new(ctx.seed, ctx.tick, id, Purpose::SUPPRESS)
                .range_fx(Fx::ZERO, Fx::ONE);
            let Some((aim, probe)) = suppress_aim(ctx.terrain, rules, &line, t, from, range) else {
                continue;
            };
            if !ctx.terrain.line_of_sight(from, probe) {
                continue;
            }
            let dist = (aim - from).length();
            let pct = spread_pct(rules, c, !still, dist, range) * tr.spread_pct as i64 / 100;
            let spread = Fx::from_raw(
                (Fx::from(w.spread).raw() as i64 * pct / 100).min(i32::MAX as i64) as i32,
            );
            let mut rng = RngStream::new(ctx.seed, ctx.tick, id, Purpose::AIM);
            let vel = shot_velocity(from, aim, w.muzzle_speed.into(), spread, &mut rng);
            fire_round(
                out,
                ws,
                c,
                kit,
                feed,
                w,
                id,
                wi,
                owner,
                from,
                vel,
                tr.reload_pct,
            );
            continue;
        }
        // S5a fire discipline: the reach his fire mode allows.
        let r = range_sq(fire_reach(&rules.fire, u.fire, w));
        let from = muzzle(pos, c, &rules.body);
        // S8a: a vehicle target (an exposed crew or a plate the weapon
        // beats) stays valid while `vehicle_aim` gives a point.
        let vmark = |t: SimId| ctx.vmarks.iter().find(|m| m.id == t);
        let mut vaim: Option<FxVec3> = None;
        if let Some(t) = c.target {
            let ok = if ctx.kind.get(t.index()) == Some(&EntityKind::Vehicle) {
                vaim = vmark(t).and_then(|m| vehicle_aim(ctx.game, owner, from, wi, m, r));
                vaim.is_some()
            } else {
                valid_target(ctx.marks, ctx.mark_of, t, owner, pos.xy(), r).is_some()
            };
            if !ok {
                c.target = None;
            }
        }
        if c.target.is_none() || c.retarget == 0 {
            let before = c.target;
            let man = if w.class == WeaponClass::Mg {
                let roll =
                    RngStream::new(ctx.seed, ctx.tick, SimId(i as u32), Purpose::SWEEP).next_u64();
                search(ctx, i, from, r, None, SWEEP_POOL, (roll >> 32) as usize)
            } else {
                let focus = if u.detached { None } else { sq.focus };
                search(ctx, i, from, r, focus, LOS_TRIES, 0)
            };
            // S8a: the nearest vehicle he can hurt, if nearer than the man
            // (soldiers win a tie).
            let veh = search_vehicle(ctx, i, from, wi, r);
            let man_d = man.map(|t| {
                (ctx.marks[ctx.mark_of[t.index()] as usize].pos.xy() - from.xy()).length_sq_raw()
            });
            c.target = match (man, veh) {
                (Some(_), Some((vd, vid, p))) if man_d.is_some_and(|md| vd < md) => {
                    vaim = Some(p);
                    Some(vid)
                }
                (Some(m), _) => {
                    vaim = None;
                    Some(m)
                }
                (None, Some((_, vid, p))) => {
                    vaim = Some(p);
                    Some(vid)
                }
                (None, None) => None,
            };
            c.retarget = if c.target.is_some() { hz } else { hz / 3 }.max(1) as u16;
            if c.target.is_some() && before.is_none() {
                // Aim before opening fire (and rise from cover); switching
                // targets mid-fight costs nothing.
                ws.cooldown = ws.cooldown.max(aim_ticks);
            }
        }
        let Some(t) = c.target else { continue };
        // S9 (S9-12 (A)): a man at a gun's slot fires his own weapon only
        // while the gun has no target and the enemy is within
        // `self_defence_m`.
        if let Some(s) = ctx.unit[i].serving
            && !crate::guns::may_fire_rifle(ctx.game, s.gun, pos.xy(), ctx.transform, t, ctx.kind)
        {
            c.target = None;
            continue;
        }
        if ws.cooldown > 0 || (c.still as u32) < w.setup_ticks {
            continue;
        }
        let id = SimId(i as u32);
        let aim = match vaim {
            Some(p) => p,
            None => ctx.marks[ctx.mark_of[t.index()] as usize].aim,
        };
        let dist = (aim - from).length();
        let range: Fx = w.range_max.into();
        let pct = spread_pct(rules, c, !still, dist, range) * tr.spread_pct as i64 / 100;
        let spread =
            Fx::from_raw((Fx::from(w.spread).raw() as i64 * pct / 100).min(i32::MAX as i64) as i32);
        let mut rng = RngStream::new(ctx.seed, ctx.tick, id, Purpose::AIM);
        let vel = shot_velocity(from, aim, w.muzzle_speed.into(), spread, &mut rng);
        fire_round(
            out,
            ws,
            c,
            kit,
            feed,
            w,
            id,
            wi,
            owner,
            from,
            vel,
            tr.reload_pct,
        );
    }
    changed
}

/// S5a: how far a man in fire mode `mode` opens fire with weapon `w`
/// (docs/slices/S5a-ammo.md "Fire discipline and fire modes"):
/// `discipline` within the weapon's `hold_beyond_pct` of its range (else
/// the rules'), `at_will` to it, `hold` within `hold_open_m`.
pub(crate) fn fire_reach(
    t: &frontline_content::tuning::FireTuning,
    mode: FireMode,
    w: &WeaponSetup,
) -> Fx {
    let full: Fx = w.range_max.into();
    match mode {
        FireMode::AtWill => full,
        FireMode::Discipline => {
            let pct = if w.hold_beyond_pct > 0 {
                w.hold_beyond_pct
            } else {
                t.hold_beyond_pct
            };
            // In i64: a 600 m range times 100 overflows Q16.16.
            Fx::from_raw((full.raw() as i64 * pct.min(100) as i64 / 100) as i32)
        }
        FireMode::Hold => Fx::from_int(t.hold_open_m.min(10_000) as i32).min(full),
    }
}

/// S5a MG bursts: the cooldown after shot number `shots` (1-based) of
/// weapon `w` within its magazine: the shot interval, plus the weapon's
/// `burst_pause_ticks` after every `burst_rounds` shots (counted on the
/// shot counter; a reload also pauses).
pub(crate) fn burst_cooldown(w: &WeaponSetup, shots: u32) -> u32 {
    let base = interval_after(w, shots);
    if w.burst_rounds > 0 && shots.is_multiple_of(w.burst_rounds) {
        base + w.burst_pause_ticks
    } else {
        base
    }
}

/// S4: the training profile of squad `sq`'s card.
#[inline]
pub(crate) fn training_of<'a>(
    setup: &'a MatchSetup,
    sq: &Squad,
) -> &'a frontline_content::setup::TrainingSetup {
    let t = setup.squad_types[sq.type_idx as usize].training;
    &setup.training[t as usize]
}

/// `v` scaled by `pct` percent (exact at 100).
#[inline]
pub(crate) fn pct_of(v: u32, pct: u32) -> u32 {
    (v as u64 * pct as u64 / 100).min(u32::MAX as u64) as u32
}

/// Requests one small-arms round and runs the magazine and cooldown.
/// S5a: with a feed slot the reload comes from the man's carried rounds
/// (none: the weapon is dry; on his sidearm the reload waits for
/// [`crate::supply::reload_dry`], which may switch back to his main
/// weapon); legacy weapons refill from nothing.
#[allow(clippy::too_many_arguments)]
fn fire_round(
    out: &mut Vec<FireRequest>,
    ws: &mut Weapon,
    c: &mut Combat,
    kit: &mut Kit,
    feed: Option<usize>,
    w: &WeaponSetup,
    id: SimId,
    wi: u16,
    owner: u8,
    from: FxVec3,
    vel: FxVec3,
    reload_pct: u32,
) {
    out.push(FireRequest {
        shooter: id,
        seq: 0,
        pos: from,
        vel,
        weapon: wi,
        player: owner,
        ttl: round_ttl(w),
        he: false,
        ammo: crate::projectile::NO_AMMO,
        mission: 0,
    });
    ws.shots = ws.shots.wrapping_add(1);
    c.since_shot = 0;
    c.mag = c.mag.saturating_sub(1);
    if c.mag == 0 {
        match feed {
            None => c.mag = w.magazine.min(u16::MAX as u32) as u16,
            Some(_) if kit.on_sidearm => {}
            Some(k) => {
                let take = (w.magazine.min(u16::MAX as u32) as u16).min(kit.rounds[k]);
                kit.rounds[k] -= take;
                c.mag = take;
            }
        }
        // S5a: an empty weapon with nothing to reload waits one interval
        // (a squadmate may feed it); `reload_dry` then runs the reload.
        ws.cooldown = if c.mag == 0 {
            interval_ceil(w)
        } else {
            pct_of(w.reload_ticks, reload_pct).max(interval_ceil(w))
        }
        .min(u16::MAX as u32) as u16;
    } else {
        ws.cooldown = burst_cooldown(w, ws.shots).min(u16::MAX as u32) as u16;
    }
}

// --------------------------------------------------------------- mortars --

impl MatchState {
    /// Fires the mortar squads' fire missions (see the module docs and
    /// `crate::fire_mission`). Runs after [`aim_and_fire`] counted their
    /// cooldowns down; squads in ascending id.
    pub(crate) fn fire_mortars(
        &mut self,
        ents: &mut Entities,
        terrain: &Heightfield,
        hash: &mut HashCache,
        seed: u64,
        tick: u64,
        out: &mut Vec<FireRequest>,
    ) {
        let ids: Vec<SquadId> = self
            .squads
            .iter()
            .filter(|(_, sq)| {
                !sq.members.is_empty() && matches!(sq.order, Order::FireMission { .. })
            })
            .map(|(&id, _)| id)
            .collect();
        let mut repath = Vec::new();
        for id in ids {
            if let Some(spot) = self.fire_mission_tick(ents, terrain, hash, seed, tick, id, out) {
                repath.push((id, spot));
            }
        }
        for (id, spot) in repath {
            self.request_path(id, spot);
        }
    }

    /// One squad's mission this tick; `Some(spot)`: walk there (back into
    /// range).
    #[allow(clippy::too_many_arguments)]
    fn fire_mission_tick(
        &mut self,
        ents: &mut Entities,
        terrain: &Heightfield,
        hash: &mut HashCache,
        seed: u64,
        tick: u64,
        id: SquadId,
        out: &mut Vec<FireRequest>,
    ) -> Option<FxVec2> {
        use crate::fire_mission::{mode, phase};
        let setup = &self.setup;
        let mortar = &setup.rules.combat.mortar;
        let per_point = setup.rules.combat.suppression.spread_per_point_pct as i64;
        let hz = setup.tick_hz as u64;
        let sq = &self.squads[&id];
        // S4: the first man in formation order carrying a mortar who is
        // not detached fires it (with every one of them detached or fallen,
        // nobody).
        let gunner = self.mortar_gunner(ents, sq)?;
        let Order::FireMission { target, rounds, m } = sq.order else {
            return None;
        };
        if sq.nav.phase != NavPhase::Holding || rounds == 0 {
            return None;
        }
        let wi = ents.kit[gunner.index()].weapon;
        let w = &setup.weapons[wi as usize];
        let i = gunner.index();
        let pos = ents.transform[i].pos;
        let d = (target - pos.xy()).length();
        let (min, max): (Fx, Fx) = (w.range_min.into(), w.range_max.into());
        if d < min || d > max {
            // Holding out of range (the gunner fell and the next man stands
            // elsewhere, or the walk ended short): walk to 90 % of the range
            // again, unless the last walk already went for that spot; then
            // it cannot be reached and the order ends.
            let facing = (target - pos.xy()).normalize_or_zero();
            let spot = terrain.clamp(target - facing * max.mul_int(9).div_int(10));
            let tried = (spot - sq.nav.goal).length_sq_raw()
                <= range_sq(Fx::from_int(mortar.barrage_retry_m as i32));
            if d < min || tried {
                self.end_mission(id, ents, tick);
                return None;
            }
            return Some(spot);
        }
        let fm = setup.rules.fire_mission.clone();
        let mut m = m;
        let mut obs_mode = None;
        if let Some(fm) = &fm {
            if tick < m.ready_at {
                return None;
            }
            if m.wait_burst {
                if tick < m.burst_by {
                    return None;
                }
                // The adjusting round never burst (off the map, expired).
                m.wait_burst = false;
                m.ready_at = tick + fm.adjust_wait_s as u64 * hz;
                self.set_mission(id, m);
                return None;
            }
            if ents.weapon[i].cooldown > 0 || (ents.combat[i].still as u32) < w.setup_ticks {
                return None;
            }
            let (new, obs) = self.mode_for(ents, terrain, id, pos.xy(), target, tick);
            let counters = &mut self.players[sq.owner.0 as usize].fm;
            if m.phase == phase::LAYING {
                // The mode at order time gave the start error; an observer
                // lost (or gained) while laying counts as a change.
                if m.mode != mode::MAP && new == mode::MAP {
                    counters.downgrades += 1;
                } else if m.mode == mode::MAP && new != mode::MAP {
                    counters.upgrades += 1;
                }
                m.phase = if new == mode::MAP
                    || (m.ranged && m.offset.length() <= Fx::from_int(fm.ffe_error_m as i32))
                {
                    phase::EFFECT
                } else {
                    phase::ADJUST
                };
            } else if m.mode == mode::MAP && new != mode::MAP {
                // The point came into a linked observer's view: range in
                // from the map-fire offset.
                m.phase = phase::ADJUST;
                m.adjust = 0;
                counters.upgrades += 1;
            } else if m.mode != mode::MAP && new == mode::MAP {
                // The observer is lost: fire on the last data.
                m.phase = phase::EFFECT;
                counters.downgrades += 1;
            }
            // Fire for effect once an observer saw a round land close
            // enough, or after the most adjusting rounds (an observer who
            // never sees one land never calls it on the error alone).
            if m.phase == phase::ADJUST
                && m.adjust > 0
                && ((m.seen > 0 && m.offset.length() <= Fx::from_int(fm.ffe_error_m as i32))
                    || m.adjust as u32 >= fm.adjust_rounds_max)
            {
                m.phase = phase::EFFECT;
            }
            m.mode = new;
            m.observer = obs;
            obs_mode = Some(new);
        } else if ents.weapon[i].cooldown > 0 || (ents.combat[i].still as u32) < w.setup_ticks {
            return None;
        }
        // Aim: the mission's offset, then the precision spread on top.
        let c = &ents.combat[i];
        let mut rng = RngStream::new(seed, tick, gunner, Purpose::SHELL_AIM);
        let pct = 100 + (c.suppression / 1000) as i64 * per_point / 100;
        let sp = d.mul(w.spread.into()).raw() as i64 * pct / 100;
        let sp = Fx::from_raw(sp.min(i32::MAX as i64) as i32);
        let aim_xy =
            terrain.clamp(target + m.offset + FxVec2::new(tri(&mut rng, sp), tri(&mut rng, sp)));
        let from = FxVec3::new(
            pos.x,
            pos.y,
            pos.z + Fx::from_milli(mortar.muzzle_mm as i32),
        );
        let (vel, flight, charge) = match &w.indirect {
            Some(ind) => {
                let to = aim_xy.extend(terrain.height_at(aim_xy));
                let Some(shot) = crate::indirect::solve(ind, from, to) else {
                    // No charge reaches from here: the mission ends.
                    self.players[sq.owner.0 as usize].fm.no_solution += 1;
                    self.end_mission(id, ents, tick);
                    return None;
                };
                (shot.vel, shot.flight, shot.charge)
            }
            None => {
                let dxy = aim_xy - from.xy();
                let flight = dxy
                    .length()
                    .mul(Fx::from_milli(mortar.flight_milli as i32))
                    .sqrt()
                    .max(Fx::from_int(mortar.min_flight_s as i32));
                let rise = terrain.height_at(aim_xy) - from.z;
                let vz = (rise + GRAVITY.mul(flight).mul(flight).div_int(2)).div(flight);
                (
                    FxVec3::new(dxy.x.div(flight), dxy.y.div(flight), vz),
                    flight,
                    0,
                )
            }
        };
        // S5a: each bomb from the gunner's, then his crew's (within
        // `crew_feed_m`); none left: the mission ends.
        let owner = sq.owner;
        let (members, type_idx) = (sq.members.clone(), sq.type_idx);
        if !crate::supply::take_bomb(&self.setup, ents, &members, type_idx, gunner, wi) {
            self.end_mission(id, ents, tick);
            return None;
        }
        let ttl = (flight + Fx::from_int(4)).mul_int(SUBSTEP_HZ).floor_int();
        out.push(FireRequest {
            shooter: gunner,
            seq: 0,
            pos: from,
            vel,
            weapon: wi,
            player: owner.0,
            ttl: ttl.clamp(2, u16::MAX as i32) as u16,
            he: true,
            ammo: crate::projectile::NO_AMMO,
            mission: m.id,
        });
        let w = &self.setup.weapons[wi as usize];
        let ws = &mut ents.weapon[i];
        ws.shots = ws.shots.wrapping_add(1);
        ws.cooldown = interval_ceil(w).max(w.reload_ticks).min(u16::MAX as u32) as u16;
        ents.combat[i].since_shot = 0;
        hash.mark_entity_chunk(gunner.chunk());
        let flight_ticks = flight.mul_int(hz as i32).round_int().max(0) as u64;
        if let (Some(md), Some(_)) = (obs_mode, &fm) {
            m.fired = m.fired.saturating_add(1);
            m.charge = charge;
            m.flight = flight_ticks.min(u16::MAX as u64) as u16;
            let c = &mut self.players[owner.0 as usize].fm;
            c.bombs[md as usize] += 1;
            c.miss_mm[md as usize] += ((aim_xy - target).length().raw().max(0) as u64 * 1000) >> 16;
            c.flight_ticks += flight_ticks;
            if crate::supply::in_own_base(&self.setup, owner, pos.xy()) {
                c.base_bombs[md as usize] += 1;
            }
            if m.phase == phase::ADJUST {
                m.wait_burst = true;
                m.burst_by = tick + flight_ticks + 4 * hz;
                m.adjust = m.adjust.saturating_add(1);
                c.adjust += 1;
            }
        }
        let left = rounds - 1;
        let sq = self.squads.get_mut(&id).expect("live");
        if let Order::FireMission { rounds, m: sm, .. } = &mut sq.order {
            *rounds = left;
            *sm = m;
        }
        if left == 0 {
            self.end_mission(id, ents, tick);
        }
        None
    }

    /// Stores a squad's mission state.
    pub(crate) fn set_mission(&mut self, id: SquadId, m: crate::fire_mission::Mission) {
        if let Some(sq) = self.squads.get_mut(&id)
            && let Order::FireMission { m: sm, .. } = &mut sq.order
        {
            *sm = m;
        }
    }

    /// Ends a squad's mission (`Idle`); an observed or direct mission with
    /// a seen round leaves its final offset in the ranging memory.
    pub(crate) fn end_mission(&mut self, id: SquadId, ents: &Entities, tick: u64) {
        let gunner = self.squads.get(&id).and_then(|sq| self.tube_pos(ents, sq));
        let fm = self.setup.rules.fire_mission.clone();
        let Some(sq) = self.squads.get_mut(&id) else {
            return;
        };
        if let (Order::FireMission { target, m, .. }, Some(fm), Some(g)) = (&sq.order, &fm, gunner)
            && m.seen > 0
            && m.mode != crate::fire_mission::mode::MAP
        {
            let (t, o, n) = (*target, m.offset, m.seen);
            crate::fire_mission::remember(&mut sq.ranging, fm, t, o, n, tick, g);
        }
        sq.order = Order::Idle;
    }
}

// ------------------------------------------------------------- launchers --

impl MatchState {
    /// Fires the AT teams' launchers (S3; see the module docs), serially in
    /// ascending squad id after [`aim_and_fire`] counted the carriers'
    /// cooldowns down. Reads the vehicle frames of this tick's Movement.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn fire_launchers(
        &mut self,
        ents: &mut Entities,
        terrain: &Heightfield,
        hash: &mut HashCache,
        seed: u64,
        tick: u64,
        out: &mut Vec<FireRequest>,
        rockets: &mut u64,
    ) {
        // S4: crew weapons of fallen men are picked up first (the
        // launcher's hand-over is one of them).
        let dropping: Vec<SquadId> = self
            .squads
            .iter()
            .filter(|(_, sq)| !sq.drops.is_empty())
            .map(|(&id, _)| id)
            .collect();
        for id in dropping {
            self.pick_up(ents, hash, id);
        }
        let ids: Vec<SquadId> = self
            .squads
            .iter()
            .filter(|(_, sq)| sq.launcher.is_some())
            .map(|(&id, _)| id)
            .collect();
        for id in ids {
            self.fire_launcher(ents, terrain, hash, seed, tick, id, out, rockets);
        }
        // S4: single-shot AT items (Panzerfausts), serially in ascending
        // squad id.
        let ids: Vec<SquadId> = self
            .squads
            .iter()
            .filter(|(_, sq)| sq.pzf.is_some())
            .map(|(&id, _)| id)
            .collect();
        for id in ids {
            self.fire_single_shot(ents, terrain, hash, seed, tick, id, out, rockets);
        }
        // S11b G2-G3: rifle grenades, serially in ascending squad id (no
        // squad holds `rg` with `[items] rifle_grenades` 0).
        let ids: Vec<SquadId> = self
            .squads
            .iter()
            .filter(|(_, sq)| sq.rg.is_some())
            .map(|(&id, _)| id)
            .collect();
        for id in ids {
            self.fire_rifle_grenades(ents, terrain, hash, seed, tick, id, out);
        }
    }

    /// S11b G2-G3 (docs/slices/S11b.md 4): squad `id`'s rifle grenades.
    /// Every third of a second with no aim and no gap, the nearest (man,
    /// seen enemy man on foot) pair inside `[min_range, range]` with a
    /// clear line of fire (at most [`LOS_TRIES`] tried) starts an aim of
    /// `windup_ticks` at the target's ground point. The aim breaks (no gap)
    /// when the man may no longer fire (the single-shot rule: pinned,
    /// ducked, riding, hold fire) or the target dies or leaves the band
    /// (he need not stay in sight). At its end the grenade flies the low root of the
    /// drag-free arc to the point (`indirect::low_root`; no root, no shot)
    /// with two `tri` draws of spread; the man stows the launcher for
    /// `stow_ticks` (his rifle silent) and the squad waits
    /// `rifle_grenade_gap_ticks`. An ordinary shell burst (`mission` 0).
    #[allow(clippy::too_many_arguments)]
    fn fire_rifle_grenades(
        &mut self,
        ents: &mut Entities,
        terrain: &Heightfield,
        hash: &mut HashCache,
        seed: u64,
        tick: u64,
        id: SquadId,
        out: &mut Vec<FireRequest>,
    ) {
        let setup = &self.setup;
        let hz = setup.tick_hz.max(1);
        let sq = &self.squads[&id];
        let ty = &setup.squad_types[sq.type_idx as usize];
        let k = ItemKind::RifleGrenade.index();
        let Some((_, it)) = ty.kit(&setup.items, ItemKind::RifleGrenade) else {
            return;
        };
        let Some(wi) = it.weapon else { return };
        let w = &setup.weapons[wi as usize];
        let Some(ammo) = w.ammo else { return };
        let speed: Fx = setup
            .ammo
            .get(ammo as usize)
            .map_or(w.muzzle_speed, |a| a.speed)
            .into();
        let mut p = sq.rg.expect("rifle-grenade squad");
        let before = p;
        p.gap = p.gap.saturating_sub(1);
        p.search = p.search.saturating_sub(1);
        let body = &setup.rules.combat.body;
        let owner = sq.owner;
        let band = |from: FxVec2, at: FxVec2| {
            let d = (at - from).length_sq_raw();
            range_sq(it.range_min.into()) <= d && d <= range_sq(it.range_max.into())
        };
        // Whether man `m` may fire one now (the single-shot rule).
        let able = |ents: &Entities, m: SimId| {
            let i = m.index();
            sq.members.contains(&m)
                && ents.kind[i] == EntityKind::Soldier
                && ents.kit[i].items[k] > 0
                && ents.kit[i].windup.is_none()
                && !ents.combat[i].ducked
                && !ents.combat[i].pinned
                && ents.unit[i].riding.is_none()
                && match engage_of(&sq.order, &ents.unit[i]) {
                    Engage::Never => false,
                    Engage::Still => is_still(&ents.velocity[i], body),
                    Engage::Always => true,
                }
        };
        // An enemy man on foot this owner sees.
        let foe = |ents: &Entities, t: SimId| {
            let ti = t.index();
            ents.kind.get(ti) == Some(&EntityKind::Soldier)
                && ents.owner[ti] != owner
                && ents.unit[ti].riding.is_none()
                && ents.combat[ti].seen_by & (1 << owner.0) != 0
        };
        if let Some(m) = p.man {
            let i = m.index();
            if p.aim > 0 {
                let from = ents.transform[i].pos.xy();
                // The target must live and stay in the band (spec 5); he
                // need not stay in sight while the launcher is fitted.
                let ok = able(ents, m)
                    && p.target.is_some_and(|t| {
                        // Ids are not generational (review A-2): a slot
                        // reused by an own man is not the target.
                        ents.kind.get(t.index()) == Some(&EntityKind::Soldier)
                            && ents.owner[t.index()] != owner
                            && band(from, ents.transform[t.index()].pos.xy())
                    })
                    && band(from, p.at);
                if !ok {
                    // A broken aim: no gap.
                    (p.man, p.target, p.aim) = (None, None, 0);
                } else {
                    // Fitting and aiming: his rifle waits.
                    let ws = &mut ents.weapon[i];
                    ws.cooldown = ws.cooldown.max(2);
                    hash.mark_entity_chunk(m.chunk());
                    p.aim -= 1;
                    if p.aim == 0 {
                        let muzzle = muzzle(ents.transform[i].pos, &ents.combat[i], body);
                        let to = FxVec3::new(p.at.x, p.at.y, terrain.height_at(p.at));
                        p.target = None;
                        p.man = None;
                        if let Some((vel, flight)) = crate::indirect::low_root(muzzle, to, speed) {
                            let spread = Fx::from_raw(
                                (Fx::from(w.spread).raw() as i64
                                    * training_of(setup, sq).spread_pct as i64
                                    / 100)
                                    .clamp(0, i32::MAX as i64)
                                    as i32,
                            );
                            let dir = vel.normalize_or_zero();
                            let right = FxVec3::new(-dir.y, dir.x, Fx::ZERO).normalize_or_zero();
                            let up = FxVec3::new(Fx::ZERO, Fx::ZERO, Fx::ONE);
                            let mut rng = RngStream::new(seed, tick, m, Purpose::RIFLE_GRENADE);
                            let (a, b) = (tri(&mut rng, spread), tri(&mut rng, spread));
                            let shot = vel + (right * a + up * b) * speed;
                            let ttl = (flight.mul_int(5).div_int(4).mul_int(SUBSTEP_HZ).floor_int()
                                + 2)
                            .clamp(2, u16::MAX as i32) as u16;
                            out.push(FireRequest {
                                shooter: m,
                                seq: 0,
                                pos: muzzle,
                                vel: shot,
                                weapon: wi,
                                player: owner.0,
                                ttl: ttl.max(round_ttl(w)),
                                he: true,
                                ammo,
                                mission: 0,
                            });
                            ents.kit[i].items[k] -= 1;
                            ents.combat[i].since_shot = 0;
                            if let Some(pl) = self.players.get_mut(owner.0 as usize) {
                                pl.s11b.rg_fired += 1;
                            }
                            p.gap = setup
                                .rules
                                .items
                                .rifle_grenade_gap_ticks
                                .min(u16::MAX as u32) as u16;
                            p.stow = it.stow_ticks.min(u16::MAX as u32) as u16;
                            if p.stow > 0 {
                                p.man = Some(m);
                                let ws = &mut ents.weapon[i];
                                ws.cooldown = ws.cooldown.max(2);
                            }
                        }
                    }
                }
            } else if p.stow > 0 {
                // Stowing the launcher: his rifle stays silent.
                if sq.members.contains(&m) && ents.kind[i] == EntityKind::Soldier {
                    p.stow -= 1;
                    let ws = &mut ents.weapon[i];
                    ws.cooldown = ws.cooldown.max(2);
                    hash.mark_entity_chunk(m.chunk());
                } else {
                    p.stow = 0;
                }
                if p.stow == 0 {
                    p.man = None;
                }
            }
        }
        if p.man.is_none() && p.gap == 0 && p.search == 0 {
            p.search = (hz / 3).max(1) as u16;
            // (distance², man, target): nearest first, ties by ids.
            let mut cands: Vec<(u64, SimId, SimId)> = Vec::new();
            for &m in &sq.members {
                if !able(ents, m) {
                    continue;
                }
                let at = ents.transform[m.index()].pos.xy();
                for t in ents.soldiers() {
                    if !foe(ents, t) {
                        continue;
                    }
                    let tp = ents.transform[t.index()].pos.xy();
                    if band(at, tp) {
                        cands.push(((tp - at).length_sq_raw(), m, t));
                    }
                }
            }
            cands.sort_unstable();
            for &(_, m, t) in cands.iter().take(LOS_TRIES) {
                let i = m.index();
                let from = muzzle(ents.transform[i].pos, &ents.combat[i], body);
                let ti = t.index();
                let aim = aim_point(ents.transform[ti].pos, &ents.combat[ti], body);
                if !terrain.line_of_sight(from, aim) || self.smoke_blocks(from, aim, tick, (t, m)) {
                    continue;
                }
                p.man = Some(m);
                p.target = Some(t);
                p.at = ents.transform[ti].pos.xy();
                p.aim = it.windup_ticks.clamp(1, u16::MAX as u32) as u16;
                let ws = &mut ents.weapon[i];
                ws.cooldown = ws.cooldown.max(2);
                hash.mark_entity_chunk(m.chunk());
                break;
            }
        }
        if p != before {
            self.squads.get_mut(&id).expect("live").rg = Some(p);
        }
    }

    /// S4 single-shot AT items (module docs "Panzerfaust"): the squad's
    /// man with one aims `windup_ticks` at a seen enemy vehicle within the
    /// item weapon's engagement distance, then fires it as that launcher
    /// weapon (a physical round, a shell slot) and the tube is spent; his
    /// own weapon is idle while he aims. One per squad per
    /// `items.single_shot_gap_ticks`; the `AttackUnit` vehicle first.
    #[allow(clippy::too_many_arguments)]
    fn fire_single_shot(
        &mut self,
        ents: &mut Entities,
        terrain: &Heightfield,
        hash: &mut HashCache,
        seed: u64,
        tick: u64,
        id: SquadId,
        out: &mut Vec<FireRequest>,
        rockets: &mut u64,
    ) {
        let setup = &self.setup;
        let hz = setup.tick_hz.max(1);
        let sq = &self.squads[&id];
        let ty = &setup.squad_types[sq.type_idx as usize];
        let k = ItemKind::SingleShotAt.index();
        let Some((_, it)) = ty.kit(&setup.items, ItemKind::SingleShotAt) else {
            return;
        };
        let Some(wi) = it.weapon else { return };
        let w = &setup.weapons[wi as usize];
        let mut p = sq.pzf.expect("single-shot squad");
        let before = p;
        p.gap = p.gap.saturating_sub(1);
        p.search = p.search.saturating_sub(1);
        let body = &setup.rules.combat.body;
        let owner = sq.owner;
        // Whether man `m` may fire one now (the launcher carrier's rule).
        let able = |ents: &Entities, m: SimId| {
            let i = m.index();
            ents.kit[i].items[k] > 0
                && ents.kit[i].windup.is_none()
                && !ents.combat[i].ducked
                && !ents.combat[i].pinned
                && ents.unit[i].riding.is_none()
                && match engage_of(&sq.order, &ents.unit[i]) {
                    Engage::Never => false,
                    Engage::Still => is_still(&ents.velocity[i], body),
                    Engage::Always => true,
                }
        };
        if let (Some(m), Some(t)) = (p.man, p.target) {
            let i = m.index();
            let ok = sq.members.contains(&m)
                && able(ents, m)
                && self.launcher_target_ok(ents, owner, ents.transform[i].pos.xy(), w, t);
            if !ok {
                (p.man, p.target, p.aim) = (None, None, 0);
            } else {
                // Aiming: his own weapon waits.
                let ws = &mut ents.weapon[i];
                ws.cooldown = ws.cooldown.max(2);
                hash.mark_entity_chunk(m.chunk());
                p.aim = p.aim.saturating_sub(1);
                if p.aim == 0 {
                    let from = muzzle(ents.transform[i].pos, &ents.combat[i], body);
                    if let (Some(ammo), Some(tf)) = (w.ammo, self.vderived.frame(t)) {
                        let at = crate::gunnery::aim_at(from, tf, p.aim_turret);
                        let speed: Fx = setup
                            .ammo
                            .get(ammo as usize)
                            .map_or(w.muzzle_speed, |a| a.speed)
                            .into();
                        let vr = &setup.rules.vehicles;
                        let vel = self.vehicles.get(&t).map_or(FxVec2::ZERO, |v| {
                            if v.avg_vel.length() > Fx::from(vr.moving_speed) {
                                crate::gunnery::lead_estimate(
                                    v.avg_vel,
                                    seed,
                                    m,
                                    t,
                                    ents.kit[i].items[k],
                                    vr.gunnery.lead_error_pct,
                                    0,
                                    100,
                                )
                            } else {
                                v.avg_vel
                            }
                        });
                        let (dir, _) = crate::gunnery::solution(from, at, vel, speed);
                        let dist = (at - from).length();
                        let still = is_still(&ents.velocity[i], body);
                        let pct = spread_pct(
                            &setup.rules.combat,
                            &ents.combat[i],
                            !still,
                            dist,
                            w.range_max.into(),
                        ) * training_of(setup, sq).spread_pct as i64
                            / 100;
                        let spread = Fx::from_raw(
                            (Fx::from(w.spread).raw() as i64 * pct / 100).min(i32::MAX as i64)
                                as i32,
                        );
                        let mut rng = RngStream::new(seed, tick, m, Purpose::SINGLE_SHOT);
                        let right = FxVec3::new(-dir.y, dir.x, Fx::ZERO).normalize_or_zero();
                        let up = FxVec3::new(Fx::ZERO, Fx::ZERO, Fx::ONE);
                        let shot =
                            (dir + right * tri(&mut rng, spread) + up * tri(&mut rng, spread))
                                .normalize_or_zero();
                        out.push(FireRequest {
                            shooter: m,
                            seq: 0,
                            pos: from,
                            vel: shot * speed,
                            weapon: wi,
                            player: owner.0,
                            ttl: round_ttl(w),
                            he: true,
                            ammo,
                            mission: 0,
                        });
                        *rockets += 1;
                        ents.kit[i].items[k] -= 1;
                        ents.combat[i].since_shot = 0;
                        let ws = &mut ents.weapon[i];
                        ws.cooldown = ws.cooldown.max((hz / 2) as u16);
                        if let Some(pl) = self.players.get_mut(owner.0 as usize) {
                            pl.s4.single_shots += 1;
                        }
                    }
                    p.gap = setup.rules.items.single_shot_gap_ticks.min(u16::MAX as u32) as u16;
                    (p.man, p.target) = (None, None);
                }
            }
        }
        if p.man.is_none() && p.gap == 0 && p.search == 0 {
            p.search = (hz / 3).max(1) as u16;
            // The `AttackUnit` vehicle first, then the nearest (man,
            // vehicle) pair; at most `LOS_TRIES` lines of fire tried.
            let mut cands: Vec<(u64, SimId, SimId)> = Vec::new();
            for &m in &sq.members {
                if !able(ents, m) {
                    continue;
                }
                let at = ents.transform[m.index()].pos.xy();
                for (&t, ts) in &self.vehicles {
                    if ts.wreck_tick.is_some() || !self.launcher_target_ok(ents, owner, at, w, t) {
                        continue;
                    }
                    let d = (ents.transform[t.index()].pos.xy() - at).length_sq_raw();
                    let key = if p.attack == Some(t) { d / 4 } else { d.max(1) };
                    cands.push((key, m, t));
                }
            }
            cands.sort_unstable();
            for &(_, m, t) in cands.iter().take(LOS_TRIES) {
                let i = m.index();
                let from = muzzle(ents.transform[i].pos, &ents.combat[i], body);
                let Some(tf) = self.vderived.frame(t) else {
                    continue;
                };
                let Some(part) = crate::gunnery::aim_choice(from, tf, terrain) else {
                    continue;
                };
                let at = crate::gunnery::aim_at(from, tf, part != armour::BoxPart::HULL);
                if self.smoke_blocks(from, at, tick, (t, m)) {
                    continue;
                }
                p.man = Some(m);
                p.target = Some(t);
                p.aim_turret = part != armour::BoxPart::HULL;
                p.aim = it.windup_ticks.clamp(1, u16::MAX as u32) as u16;
                let ws = &mut ents.weapon[i];
                ws.cooldown = ws.cooldown.max(2);
                hash.mark_entity_chunk(m.chunk());
                break;
            }
        }
        if p != before {
            self.squads.get_mut(&id).expect("live").pzf = Some(p);
        }
    }

    /// S4 crew weapons (module docs "Crew weapons"): counts squad `id`'s
    /// drops down; a drop whose time ran out goes to the nearest living
    /// man not carrying a crew weapon (crew assistants and ammo bearers
    /// first, ties by ascending id), who drops his own weapon and takes
    /// aim afresh. With nobody to take it, it is lost.
    fn pick_up(&mut self, ents: &mut Entities, hash: &mut HashCache, id: SquadId) {
        use frontline_content::setup::member_trait;
        let setup = &self.setup;
        let sq = self.squads.get_mut(&id).expect("live");
        let ty = &setup.squad_types[sq.type_idx as usize];
        let mut k = 0;
        while k < sq.drops.len() {
            let d = &mut sq.drops[k];
            d.ticks = d.ticks.saturating_sub(1);
            if d.ticks > 0 {
                k += 1;
                continue;
            }
            let d = sq.drops.remove(k);
            // S4 review L5: only a man within reach of the weapon takes it.
            let reach = range_sq(Fx::from_int(
                setup.rules.items.crew_pickup_reach_m.min(i32::MAX as u32) as i32,
            ));
            let mut candidates = 0u32;
            let picker = sq
                .members
                .iter()
                .copied()
                .filter(|m| !setup.weapons[ents.kit[m.index()].weapon as usize].crew)
                .inspect(|_| candidates += 1)
                .filter(|m| (ents.transform[m.index()].pos.xy() - d.at).length_sq_raw() <= reach)
                .min_by_key(|m| {
                    let kit = &ents.kit[m.index()];
                    let traits = ty.members.get(kit.slot as usize).map_or(0, |s| s.traits);
                    let helper =
                        traits & (member_trait::CREW_ASSISTANT | member_trait::AMMO_BEARER);
                    let dist = (ents.transform[m.index()].pos.xy() - d.at).length_sq_raw();
                    (helper == 0, dist, m.0)
                });
            let Some(p) = picker else {
                if candidates > 0 {
                    // Men left, none near: it waits (checked every tick).
                    sq.drops
                        .insert(k, crate::game::WeaponDrop { ticks: 1, ..d });
                    k += 1;
                }
                continue;
            };
            let i = p.index();
            let w = &setup.weapons[d.weapon as usize];
            // S5a: the rounds in his own weapon go back to his pouches.
            if crate::supply::counted(ty) {
                let own = &setup.weapons[crate::supply::active_weapon(ty, &ents.kit[i]) as usize];
                if let Some(k) = ty.feed_slot(own)
                    && own.he.is_none()
                {
                    let back = ents.combat[i].mag;
                    ents.kit[i].rounds[k] = ents.kit[i].rounds[k].saturating_add(back);
                }
                ents.kit[i].on_sidearm = false;
            }
            ents.kit[i].weapon = d.weapon;
            // He drops his own weapon and target and takes aim afresh.
            ents.combat[i].target = None;
            ents.combat[i].retarget = 0;
            // A fresh belt (S5a: the rounds left in it); never the count of
            // the magazine he just emptied into his pouches.
            ents.combat[i].mag = if crate::supply::counted(ty) {
                d.mag
            } else {
                w.magazine.min(u16::MAX as u32) as u16
            };
            match &mut sq.launcher {
                Some(l) if d.weapon == ty.weapon && l.carrier.is_none() => l.carrier = Some(p),
                // A gun or tube is set up again.
                _ => ents.combat[i].still = 0,
            }
            hash.mark_entity_chunk(p.chunk());
            if let Some(pl) = self.players.get_mut(sq.owner.0 as usize) {
                pl.s4.pickups += 1;
            }
            self.pickup_events.push(crate::game::PickupEvent {
                squad: id,
                man: p,
                weapon: d.weapon,
            });
        }
    }

    /// Whether vehicle `t` is a launcher target for a man of `owner` at
    /// `from` with weapon `w`: a live enemy vehicle he sees within the
    /// engagement distance for its motion (`engage_still`, or
    /// `engage_moving` above the rules' moving speed), whatever the man's
    /// fire mode (S5a-30 (A): a man holding fire in an ambush fires by the
    /// same rule).
    fn launcher_target_ok(
        &self,
        ents: &Entities,
        owner: PlayerId,
        from: FxVec2,
        w: &WeaponSetup,
        t: SimId,
    ) -> bool {
        let ti = t.index();
        if ents.kind.get(ti) != Some(&EntityKind::Vehicle)
            || ents.owner[ti] == owner
            || ents.combat[ti].seen_by & (1 << owner.0) == 0
        {
            return false;
        }
        let Some(vs) = self.vehicles.get(&t) else {
            return false;
        };
        let moving = vs.avg_vel.length() > Fx::from(self.setup.rules.vehicles.moving_speed);
        let reach: Fx = if moving {
            w.engage_moving.into()
        } else {
            w.engage_still.into()
        };
        let reach = reach.min(w.range_max.into());
        (ents.transform[ti].pos.xy() - from).length_sq_raw() <= range_sq(reach)
    }

    #[allow(clippy::too_many_arguments)]
    fn fire_launcher(
        &mut self,
        ents: &mut Entities,
        terrain: &Heightfield,
        hash: &mut HashCache,
        seed: u64,
        tick: u64,
        id: SquadId,
        out: &mut Vec<FireRequest>,
        rockets: &mut u64,
    ) {
        let setup = &self.setup;
        let hz = setup.tick_hz.max(1);
        let sq = &self.squads[&id];
        let ty = &setup.squad_types[sq.type_idx as usize];
        let w = &setup.weapons[ty.weapon as usize];
        let mut l = sq.launcher.clone().expect("launcher squad");
        // S4: while the carrier's launcher waits for the next man (a
        // `WeaponDrop`, picked up in `pick_up`), nobody fires it.
        let Some(c) = l.carrier else {
            self.squads.get_mut(&id).expect("live").launcher = Some(l);
            return;
        };
        let i = c.index();
        let owner = ents.owner[i];
        let pos = ents.transform[i].pos;
        let body = &self.setup.rules.combat.body;
        let still = is_still(&ents.velocity[i], body);
        let may = ents.kit[i].windup.is_none()
            && !ents.combat[i].ducked
            && l.rounds > 0
            // S8a: not from a transport.
            && ents.unit[i].riding.is_none()
            && match engage_of(&sq.order, &ents.unit[i]) {
                Engage::Never => false,
                Engage::Still => still,
                Engage::Always => true,
            };
        let loader = sq.members.len() >= 2;
        hash.mark_entity_chunk(c.chunk());
        if !may {
            ents.combat[i].target = None;
            self.squads.get_mut(&id).expect("live").launcher = Some(l);
            return;
        }
        let from = muzzle(pos, &ents.combat[i], &self.setup.rules.combat.body);
        let ok = |t: SimId| self.launcher_target_ok(ents, owner, pos.xy(), w, t);
        let before = ents.combat[i].target;
        let mut target = before.filter(|&t| ok(t));
        if target.is_none() || ents.combat[i].retarget == 0 {
            // The ordered vehicle first, then the nearest; at most
            // `LOS_TRIES` candidates get a line-of-fire check.
            let mut cands: Vec<(u64, SimId)> = Vec::new();
            for (&t, ts) in &self.vehicles {
                if ts.wreck_tick.is_some() || !ok(t) {
                    continue;
                }
                let d = (ents.transform[t.index()].pos.xy() - pos.xy()).length_sq_raw();
                cands.push((if l.attack == Some(t) { 0 } else { d.max(1) }, t));
            }
            cands.sort_unstable();
            target = None;
            for &(_, t) in cands.iter().take(LOS_TRIES) {
                let Some(tf) = self.vderived.frame(t) else {
                    continue;
                };
                let Some(part) = crate::gunnery::aim_choice(from, tf, terrain) else {
                    continue;
                };
                let at = crate::gunnery::aim_at(from, tf, part != armour::BoxPart::HULL);
                if self.smoke_blocks(from, at, tick, (t, c)) {
                    continue;
                }
                target = Some(t);
                l.aim_turret = part != armour::BoxPart::HULL;
                break;
            }
            ents.combat[i].retarget = if target.is_some() { hz } else { hz / 3 }.max(1) as u16;
            if target.is_some() && before.is_none() {
                // Aim (and rise to the fire step) before the first rocket.
                let ws = &mut ents.weapon[i];
                ws.cooldown = ws.cooldown.max((hz / 2) as u16);
            }
        }
        ents.combat[i].target = target;
        let fire = target.filter(|_| {
            ents.weapon[i].cooldown == 0 && (ents.combat[i].still as u32) >= w.setup_ticks
        });
        if let (Some(t), Some(ammo)) = (fire, w.ammo)
            && let Some(tf) = self.vderived.frame(t)
        {
            let at = crate::gunnery::aim_at(from, tf, l.aim_turret);
            let speed: Fx = setup
                .ammo
                .get(ammo as usize)
                .map_or(w.muzzle_speed, |a| a.speed)
                .into();
            // S3b: the gunner's estimate of a moving vehicle's velocity
            // (no ranging in: each rocket, by the carrier's shot count).
            let vr = &setup.rules.vehicles;
            let vel = self.vehicles.get(&t).map_or(FxVec2::ZERO, |v| {
                if v.avg_vel.length() > Fx::from(vr.moving_speed) {
                    crate::gunnery::lead_estimate(
                        v.avg_vel,
                        seed,
                        c,
                        t,
                        ents.weapon[i].shots as u8,
                        vr.gunnery.lead_error_pct,
                        0,
                        100,
                    )
                } else {
                    v.avg_vel
                }
            });
            let (dir, _) = crate::gunnery::solution(from, at, vel, speed);
            let dist = (at - from).length();
            let pct = spread_pct(
                &setup.rules.combat,
                &ents.combat[i],
                !still,
                dist,
                w.range_max.into(),
            );
            let spread = Fx::from_raw(
                (Fx::from(w.spread).raw() as i64 * pct / 100).min(i32::MAX as i64) as i32,
            );
            let mut rng = RngStream::new(seed, tick, c, Purpose::AIM);
            let right = FxVec3::new(-dir.y, dir.x, Fx::ZERO).normalize_or_zero();
            let up = FxVec3::new(Fx::ZERO, Fx::ZERO, Fx::ONE);
            let shot = (dir + right * tri(&mut rng, spread) + up * tri(&mut rng, spread))
                .normalize_or_zero();
            out.push(FireRequest {
                shooter: c,
                seq: 0,
                pos: from,
                vel: shot * speed,
                weapon: ty.weapon,
                player: owner.0,
                ttl: round_ttl(w),
                he: true,
                ammo,
                mission: 0,
            });
            l.rounds -= 1;
            *rockets += 1;
            self.players[owner.0 as usize].rockets += 1;
            let ws = &mut ents.weapon[i];
            ws.shots = ws.shots.wrapping_add(1);
            // 16 s without the loader (the gunner loads himself).
            let reload = w.reload_ticks * if loader { 1 } else { 2 };
            ws.cooldown = reload.min(u16::MAX as u32) as u16;
            ents.combat[i].since_shot = 0;
        }
        self.squads.get_mut(&id).expect("live").launcher = Some(l);
    }
}

/// Spawns this tick's sorted fire requests into the pool: HE shells first,
/// then small arms while more than [`crate::projectile::SHELL_RESERVE`] slots
/// are free. Returns `(fired, dropped)`.
pub(crate) fn spawn_rounds(
    reqs: &[FireRequest],
    pool: &mut crate::projectile::ProjectilePool,
    hash: &mut HashCache,
) -> (u64, u64) {
    let (mut fired, mut dropped) = (0u64, 0u64);
    for he in [true, false] {
        for r in reqs.iter().filter(|r| r.he == he) {
            fired += 1;
            if !he && pool.free_count() <= crate::projectile::SHELL_RESERVE {
                dropped += 1;
                continue;
            }
            let p = Projectile {
                pos: r.pos,
                vel: r.vel,
                owner: r.shooter,
                drag: Fx::ZERO,
                ttl: r.ttl.max(1),
                weapon: r.weapon,
                player: r.player,
                ammo: r.ammo,
                origin: r.pos,
                mission: r.mission,
            };
            match pool.spawn(p) {
                Some(slot) => hash.mark_pool_slot(slot),
                None => dropped += 1,
            }
        }
    }
    (fired, dropped)
}

// ------------------------------------------------------------ ballistics --

/// What a projectile did to soldiers, vehicles or the ground (merged in
/// slot order).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum Blow {
    Hit(SimId),
    NearMiss(SimId),
    /// A shell burst at this point (on the surface for a ground hit; at the
    /// hit point on a soldier).
    Burst(FxVec3),
    /// A shell met a vehicle's or wreck's plate (S3). The Damage stage
    /// rolls everything (`crate::damage`); a bursting round bursts there
    /// too.
    Armour {
        vehicle: SimId,
        hit: ArmourHit,
        /// The round's velocity at impact.
        vel: FxVec3,
        /// Its muzzle point (penetration range).
        origin: FxVec3,
    },
    /// A small-arms round stopped on a vehicle or wreck: no damage, no
    /// event (counted only).
    Stopped(SimId),
    /// S6: a round that does not burst stopped on static box `idx` (this
    /// tick's `MatchState::statics`): it takes `bullet_box_damage` (small
    /// arms) or `shell_box_damage` (a main-gun round). S10a: the second
    /// field is the range from the muzzle (a piece's armour gate).
    Static(u32, Fx),
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) struct BallisticEvent {
    pub weapon: u16,
    pub player: u8,
    /// The round's ammo (`NO_AMMO` for small arms and mortars), its pool
    /// slot (RNG keys) and shooter.
    pub ammo: u16,
    pub slot: u32,
    pub shooter: SimId,
    pub blow: Blow,
    /// S5b: the round's fire mission (0: none).
    pub mission: u32,
}

/// Hit capsule `(a, b, radius)` of a soldier: a vertical segment standing or
/// crouched, a horizontal one along the heading when prone.
pub(crate) fn capsule(t: &Transform, c: &Combat, body: &BodyTuning) -> (FxVec3, FxVec3, Fx) {
    let pos = t.pos;
    if c.stance == Stance::Prone {
        let (s, co) = t.yaw.sin_cos();
        let half = Fx::from_milli(body.prone_half_mm as i32);
        let f = FxVec3::new(co.mul(half), s.mul(half), Fx::ZERO);
        let r = Fx::from_milli(body.prone_radius_mm as i32);
        let mid = FxVec3::new(pos.x, pos.y, pos.z + r);
        return (mid - f, mid + f, r);
    }
    let r = Fx::from_milli(body.radius_mm as i32);
    let top = (c.height - r).max(r);
    (
        FxVec3::new(pos.x, pos.y, pos.z + r),
        FxVec3::new(pos.x, pos.y, pos.z + top),
        r,
    )
}

/// Closest points of segments `p1 + s·d1` and `p2 + t·d2` (`s, t` in
/// `[0, 1]`; after Ericson, *Real-Time Collision Detection* 5.1.9). Inputs
/// are relative to a nearby origin so all products stay in Q16.16.
/// Returns `(s, distance²)`.
pub(crate) fn seg_seg(p1: FxVec3, d1: FxVec3, p2: FxVec3, d2: FxVec3) -> (Fx, Fx) {
    let eps = Fx::from_raw(16);
    let r = p1 - p2;
    let a = d1.dot(d1);
    let e = d2.dot(d2);
    let f = d2.dot(r);
    let clamp01 = |v: Fx| v.clamp(Fx::ZERO, Fx::ONE);
    let (s, t);
    if a <= eps && e <= eps {
        (s, t) = (Fx::ZERO, Fx::ZERO);
    } else if a <= eps {
        s = Fx::ZERO;
        t = clamp01(f.div(e));
    } else {
        let c = d1.dot(r);
        if e <= eps {
            t = Fx::ZERO;
            s = clamp01((-c).div(a));
        } else {
            let b = d1.dot(d2);
            let denom = a.mul(e) - b.mul(b);
            let mut s0 = if denom > eps {
                clamp01((b.mul(f) - c.mul(e)).div(denom))
            } else {
                Fx::ZERO
            };
            let mut t0 = (b.mul(s0) + f).div(e);
            if t0 < Fx::ZERO {
                t0 = Fx::ZERO;
                s0 = clamp01((-c).div(a));
            } else if t0 > Fx::ONE {
                t0 = Fx::ONE;
                s0 = clamp01((b - c).div(a));
            }
            (s, t) = (s0, t0);
        }
    }
    let gap = (p1 + d1 * s) - (p2 + d2 * t);
    (s, gap.dot(gap))
}

/// Read-only inputs of [`fly`].
pub(crate) struct FlightCtx<'a> {
    pub ents: &'a Entities,
    pub spatial: &'a SpatialHash,
    pub terrain: &'a Heightfield,
    pub weapons: &'a [WeaponSetup],
    pub near_miss: Fx,
    /// Hit capsules (`[combat.body]`).
    pub body: &'a BodyTuning,
    /// S3: vehicles and wrecks (`None` when there are none), their plates,
    /// and the rounds.
    pub vehicles: Option<&'a VehicleDerived>,
    pub armour: &'a ArmourTables,
    pub ammo: &'a [AmmoSetup],
    /// S6: this tick's structure boxes and set-down crates.
    pub statics: &'a [crate::structures::StaticBox],
}

/// A soldier near a round's segment this substep.
struct Near {
    id: SimId,
    /// His hit capsule's axis (S3b: near misses are measured to the body,
    /// head to feet, not to its middle).
    axis: (FxVec3, FxVec3),
    /// Unclamped position along the segment where it passes his middle
    /// (picks the one substep that counts the pass).
    s: Fx,
    /// Squared distance from the segment to his axis.
    dist_sq: Fx,
}

/// The broadphase pad around a vehicle's hull origin: its horizontal reach
/// plus the height of hull and turret (m).
const VEHICLE_SPHERE_PAD_M: i32 = 3;

/// The first vehicle or wreck box the segment `from → from + d` enters,
/// among those not owned by `player` (wrecks belong to nobody): `(t, id,
/// hit)`, earliest first, ties by id. One bounding-sphere test per nearby
/// vehicle, then at most two box slab tests (counted in `tests`).
fn first_box(
    ctx: &FlightCtx,
    vd: &VehicleDerived,
    from: FxVec3,
    d: FxVec3,
    player: u8,
    tests: &mut u64,
) -> Option<(Fx, SimId, ArmourHit)> {
    let mid = from + d.div_int(2);
    let pad = Fx::from_int(VEHICLE_SPHERE_PAD_M);
    let reach = d.length().div_int(2) + vd.max_reach + pad;
    let mut best: Option<(Fx, SimId, ArmourHit)> = None;
    vd.index.for_each_near(mid.xy(), reach, |id| {
        let Some(f) = vd.frame(id) else { return };
        if !f.wreck && f.owner.0 == player {
            return;
        }
        let Some(a) = ctx.armour.vehicles.get(f.type_idx as usize) else {
            return;
        };
        if !armour::segment_near(from, d, f.origin, f.reach + pad) {
            return;
        }
        *tests += 2;
        if let Some(h) = armour::hit_armour(from, d, f, a)
            && best.is_none_or(|(bt, bid, _)| (h.t, id) < (bt, bid))
        {
            best = Some((h.t, id, h));
        }
    });
    best
}

/// The tie rule between a box hit and a capsule hit at segment fractions
/// `box_t` and `capsule_t`: the earlier wins, the box on a tie. (The
/// terrain test that follows includes its end point, so terrain wins a tie
/// with either; ties within boxes or capsules go to the lower id.)
#[inline]
fn box_before(box_t: Option<Fx>, capsule_t: Option<Fx>) -> bool {
    box_t.is_some_and(|b| capsule_t.is_none_or(|c| b <= c))
}

/// Integrates one chunk of match projectile slots for `substeps` substeps
/// (see the module docs). Finished projectiles are cleared and reported in
/// `ended`; their effects go to `events`, both in slot order. `box_tests`
/// counts the armour slab tests.
#[allow(clippy::too_many_arguments)]
pub(crate) fn fly(
    chunk: usize,
    slots: &mut [Option<Projectile>],
    ctx: &FlightCtx,
    substeps: u32,
    ended: &mut Vec<(u32, ProjectileEnd)>,
    events: &mut Vec<BallisticEvent>,
    box_tests: &mut u64,
) -> bool {
    let base = chunk * crate::ecs::CHUNK_SIZE;
    let ents = ctx.ents;
    let mut near: Vec<Near> = Vec::new();
    let mut changed = false;
    for (j, slot) in slots.iter_mut().enumerate() {
        let Some(p) = slot else { continue };
        changed = true;
        let slot_id = (base + j) as u32;
        let shell = p.ammo != NO_AMMO;
        // Mortar shells test only the ground and vehicles (as in S1);
        // main-gun shells and rockets also soldiers, like small arms.
        let mortar = !shell
            && ctx
                .weapons
                .get(p.weapon as usize)
                .is_some_and(|w| w.he.is_some());
        let bursts = mortar
            || ctx
                .ammo
                .get(p.ammo as usize)
                .is_some_and(|a| a.mechanism.bursts());
        // S8a (6.4): small arms with a penetration table take the plate
        // rule in the Damage stage; the others stop on any plate.
        let pens = !shell
            && !mortar
            && ctx
                .weapons
                .get(p.weapon as usize)
                .is_some_and(|w| !w.pen.is_empty());
        let (weapon, player, shooter, ammo, mission) =
            (p.weapon, p.player, p.owner, p.ammo, p.mission);
        let ev = |blow| BallisticEvent {
            weapon,
            player,
            ammo,
            slot: slot_id,
            shooter,
            blow,
            mission,
        };
        for _ in 0..substeps {
            let from = p.pos;
            let next = advance(p);
            let d = next - from;
            let len = d.length();
            let boxed = ctx
                .vehicles
                .and_then(|vd| first_box(ctx, vd, from, d, player, box_tests));
            // S6: a structure box or crate in front of any vehicle box takes
            // its place (strictly nearer; a tie goes to the vehicle).
            let sboxed = if ctx.statics.is_empty() {
                None
            } else {
                crate::structures::first_static(ctx.statics, from, d, player)
            };
            let static_first = sboxed.is_some_and(|(st, _)| boxed.is_none_or(|(vt, _, _)| st < vt));
            let any_box_t = if static_first {
                sboxed.map(|b| b.0)
            } else {
                boxed.map(|b| b.0)
            };
            // Soldiers near the segment, nearest capsule hit first.
            let mut hit: Option<(Fx, SimId)> = None;
            near.clear();
            if !mortar {
                let mid = from + d.div_int(2);
                let reach = len.div_int(2) + ctx.near_miss + Fx::from_int(2);
                let len_sq = d.dot(d);
                ctx.spatial.for_each_near(mid.xy(), reach, |id| {
                    let i = id.index();
                    if ents.owner[i].0 == player || id == shooter {
                        return;
                    }
                    let t = &ents.transform[i];
                    let c = &ents.combat[i];
                    // Local coordinates around the soldier keep products small.
                    let o = t.pos;
                    let (a, b, r) = capsule(t, c, ctx.body);
                    let (s, dsq) = seg_seg(from - o, d, a - o, b - a);
                    if dsq <= r.mul(r) && hit.is_none_or(|(hs, hid)| (s, id) < (hs, hid)) {
                        hit = Some((s, id));
                    }
                    let centre = body_point(o, c);
                    let rel = centre - from;
                    let su = if len_sq > Fx::ZERO {
                        rel.dot(d).div(len_sq)
                    } else {
                        Fx::ZERO
                    };
                    near.push(Near {
                        id,
                        axis: (a, b),
                        s: su,
                        dist_sq: dsq,
                    });
                });
            }
            // Earliest hit wins; at equal t: terrain, then box, then
            // capsule. A box hit re-tests the terrain in front of it at
            // 0.25 m steps (a crest just in front of a hull blocks).
            let box_first = box_before(any_box_t, hit.map(|h| h.0));
            let mut end_s = Fx::ONE;
            let ground = if box_first {
                let bt = any_box_t.expect("box first");
                end_s = bt;
                ctx.terrain.segment_hit_fine(from, from + d * bt)
            } else {
                if let Some((s, _)) = hit {
                    end_s = s;
                }
                ctx.terrain.segment_hit(from, from + d * end_s)
            };
            if let Some(g) = ground {
                end_s = if len > Fx::ZERO {
                    (g - from).length().div(len)
                } else {
                    Fx::ZERO
                };
            }
            let capsule_hit = if ground.is_none() && !box_first {
                hit
            } else {
                None
            };
            let box_hit = if ground.is_none() && box_first && !static_first {
                boxed
            } else {
                None
            };
            let static_hit = if ground.is_none() && box_first && static_first {
                sboxed
            } else {
                None
            };
            // Near misses: passed (closest approach inside the segment and
            // before the round stopped), or close to where it struck earth.
            let nm_sq = ctx.near_miss.mul(ctx.near_miss);
            for n in &near {
                if Some(n.id) == capsule_hit.map(|h| h.1) {
                    continue;
                }
                let passed = n.s >= Fx::ZERO && n.s < end_s && n.dist_sq <= nm_sq;
                let by_impact = ground.is_some_and(|g| {
                    let (a, b) = n.axis;
                    seg_seg(g - a, FxVec3::ZERO, FxVec3::ZERO, b - a).1 <= nm_sq
                });
                if passed || by_impact {
                    events.push(ev(Blow::NearMiss(n.id)));
                }
            }
            if let Some(g) = ground {
                p.pos = g;
                if bursts {
                    let surface = FxVec3::new(g.x, g.y, ctx.terrain.height_at(g.xy()));
                    events.push(ev(Blow::Burst(surface)));
                }
                ended.push((slot_id, ProjectileEnd::GroundHit));
                *slot = None;
                break;
            }
            if let Some((st, k)) = static_hit {
                p.pos = from + d * st;
                events.push(ev(if bursts {
                    Blow::Burst(p.pos)
                } else {
                    Blow::Static(k as u32, (p.pos - p.origin).xy().length())
                }));
                ended.push((slot_id, ProjectileEnd::VehicleHit));
                *slot = None;
                break;
            }
            if let Some((_, vid, h)) = box_hit {
                p.pos = h.point;
                // S7: a small-arms round through an open face or into an
                // exposed seat goes on to the crew (`damage::open_hit`);
                // S8a: one that may beat the plate, and every shield hit,
                // take the plate rule (`damage::resolve`).
                let blow = if shell || mortar || pens || h.via != armour::HitVia::Plate {
                    Blow::Armour {
                        vehicle: vid,
                        hit: h,
                        vel: p.vel,
                        origin: p.origin,
                    }
                } else {
                    Blow::Stopped(vid)
                };
                events.push(ev(blow));
                ended.push((slot_id, ProjectileEnd::VehicleHit));
                *slot = None;
                break;
            }
            if let Some((s, id)) = capsule_hit {
                p.pos = from + d * s;
                events.push(ev(if bursts {
                    Blow::Burst(p.pos)
                } else {
                    Blow::Hit(id)
                }));
                ended.push((slot_id, ProjectileEnd::SoldierHit));
                *slot = None;
                break;
            }
            if let Some(end) = finish(p, next, ctx.terrain) {
                ended.push((slot_id, end));
                *slot = None;
                break;
            }
        }
    }
    changed
}

// ---------------------------------------------------------------- damage --

/// A burst this tick, for the snapshot.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) struct Explosion {
    pub pos: FxVec3,
    /// Weapon index (shells) or item index (frags, satchels).
    pub weapon: u16,
    pub kind: BlastKind,
}

/// One HE burst: a mortar shell or a thrown item going off.
pub(crate) struct Burst<'a> {
    /// On the surface.
    pub pos: FxVec3,
    pub player: u8,
    pub he: &'a HeSetup,
    pub damage: u32,
    /// Suppression points at the blast point.
    pub suppression: u32,
    /// Satchel: the crater levels trenches (`Heightfield::crater_cut`).
    pub cut: bool,
    /// Crater depth roll.
    pub key: (SimId, Purpose),
    /// `game::kill_class` of the men it kills (S3 report).
    pub class: u16,
    /// S7: a thrown item going off inside an open box (vehicle, box).
    pub inside: Option<(SimId, u8)>,
    /// S10b R20: a demolition charge tamped against this piece.
    pub piece: Option<u32>,
    /// S11a2 F1: a fire-mission round's burst with `friendly_fire` on: it
    /// hurts the firing player's own men (blast, suppression, fragments),
    /// and own statics and hulls stop its fragments undamaged.
    pub ff: bool,
}

/// Mutable world parts the damage stage needs.
pub(crate) struct DamageCtx<'a> {
    pub ents: &'a mut Entities,
    pub terrain: &'a mut Heightfield,
    pub spatial: &'a SpatialHash,
    pub hash: &'a mut HashCache,
    pub stats: &'a mut Stats,
    pub seed: u64,
    pub tick: u64,
    /// S8a: passengers dropped to 0 HP inside a vehicle (`damage::deal`),
    /// killed with the rest at the end of the Damage stage.
    pub fallen: Vec<Fallen>,
    /// S11a2 F4: own men killed by an own fire-mission burst this tick
    /// (pushed on the killing hit only); the kill loop credits no kill
    /// for them.
    pub ff_dead: Vec<SimId>,
    /// S11a2 F4: own men hit by the burst being dealt (scratch, cleared
    /// per burst).
    pub ff_hit: Vec<SimId>,
}

/// A soldier at 0 HP this tick: `(victim, killer's player, kill class)`.
pub(crate) type Fallen = (SimId, u8, u16);

/// Takes `dmg` hit points from soldier `id`; one who drops to 0 joins
/// `dead` with the player who killed him and the `game::kill_class`.
/// Returns whether this hit killed him (S11a2 F4).
fn hurt(
    ents: &mut Entities,
    hash: &mut HashCache,
    dead: &mut Vec<Fallen>,
    id: SimId,
    dmg: u32,
    by: (u8, u16),
) -> bool {
    let c = &mut ents.combat[id.index()];
    if c.hp == 0 {
        return false;
    }
    c.hp = c.hp.saturating_sub(dmg.min(u16::MAX as u32) as u16);
    let killed = c.hp == 0;
    if killed {
        dead.push((id, by.0, by.1));
    }
    hash.mark_entity_chunk(id.chunk());
    killed
}

/// S11a2 F4: [`hurt`] by burst `b`; an own man hit by an own mission burst
/// is noted (`ff_hit`) and, on the killing hit, joins `ff_dead`.
fn hurt_by(ctx: &mut DamageCtx, dead: &mut Vec<Fallen>, b: &Burst, id: SimId, dmg: u32) {
    let killed = hurt(ctx.ents, ctx.hash, dead, id, dmg, (b.player, b.class));
    if b.ff && ctx.ents.owner[id.index()].0 == b.player {
        ctx.ff_hit.push(id);
        if killed {
            ctx.ff_dead.push(id);
        }
    }
}

/// Most fragment rays thrown in one tick (S3b engine budget): when this
/// tick's bursts ask for more, each throws its share, scaled down
/// proportionally (`Stats::frag_rays_capped`).
pub const MAX_FRAG_RAYS_PER_TICK: u32 = 4096;

/// A ray that starts this close to a struck vehicle's box and goes into it
/// is stopped by that vehicle (a burst on a plate) (m).
const STRUCK_PROBE: Fx = Fx::from_ratio(1, 10);

/// [`frag_share`] for the world tests.
#[cfg(test)]
pub(crate) fn frag_share_for_test(count: u32, requested: u64) -> u32 {
    frag_share(count, requested)
}

/// Fragment rays of burst `b` this tick: its `frag_count`, scaled down
/// when the tick's bursts together ask for more than the budget.
fn frag_share(count: u32, requested: u64) -> u32 {
    let cap = MAX_FRAG_RAYS_PER_TICK as u64;
    if requested <= cap {
        count
    } else {
        (count as u64 * cap / requested) as u32
    }
}

impl MatchState {
    /// One HE burst (see the module docs): suppression within the
    /// suppression radius, overpressure within `blast_radius` with a clear
    /// line, `rays` fragment rays, the crater. `struck` is the vehicle or
    /// wreck a bursting round struck (it stops rays that go into it, and
    /// its plate hit is its damage).
    #[allow(clippy::too_many_arguments)]
    fn burst(
        &mut self,
        ctx: &mut DamageCtx,
        b: &Burst,
        rays: u32,
        struck: Option<SimId>,
        rules: &CombatSetup,
        dead: &mut Vec<Fallen>,
        fires: &mut FireRolls,
        events: &mut Vec<ArmourEv>,
    ) {
        let pos = b.pos;
        let blast = &rules.blast;
        let lift = FxVec3::new(pos.x, pos.y, pos.z + Fx::from_milli(blast.lift_mm as i32));
        let he = b.he;
        let blast_r: Fx = he.blast_radius.into();
        let supp_r: Fx = he.suppression_radius.into();
        let mut ids = Vec::new();
        ctx.spatial
            .for_each_near(pos.xy(), supp_r.max(blast_r), |id| ids.push(id));
        ids.sort_unstable();
        ids.dedup();
        ctx.ff_hit.clear();
        for id in ids {
            let i = id.index();
            // S11a2 F2: an own mission burst spares no own man.
            let own = ctx.ents.owner[i].0 == b.player;
            if ctx.ents.kind[i] != EntityKind::Soldier || (own && !b.ff) {
                continue;
            }
            let c = ctx.ents.combat[i];
            if c.hp == 0 {
                continue;
            }
            let body = body_point(ctx.ents.transform[i].pos, &c);
            let dist = (body - pos).length();
            if dist > supp_r && dist > blast_r {
                continue;
            }
            // S10a: overhead cover: no piece of another player between
            // (S11a2 F2: an own victim is covered by any piece, his own too).
            let skip_owner = if own { u8::MAX } else { b.player };
            let clear = ctx.terrain.line_of_sight(lift, body)
                && self.out_of_box(b, lift, body)
                && !self.piece_between(lift, body, skip_owner);
            if dist <= supp_r {
                let mut add = (b.suppression as i64 * 1000 * (supp_r - dist).raw() as i64
                    / supp_r.raw().max(1) as i64) as u32;
                if !clear {
                    add =
                        (add as u64 * rules.suppression.blast_behind_cover_pct as u64 / 100) as u32;
                }
                add_suppression(&mut ctx.ents.combat[i], add);
                ctx.hash.mark_entity_chunk(id.chunk());
            }
            // Overpressure: close in, with a clear line.
            if clear && dist <= blast_r {
                hurt_by(ctx, dead, b, id, b.damage);
            }
        }
        // S10b R20: a demolition charge's piece takes its tamped damage
        // (whatever its owner, no line check) instead of the overpressure.
        let tamped = b.piece.and_then(|pid| {
            self.statics
                .0
                .iter()
                .position(|sb| sb.of == crate::structures::StaticRef::Piece(pid))
                .map(|k| (k, pid))
        });
        if let Some((k, pid)) = tamped {
            self.demolish_piece(k, pid, b.damage, (b.player, b.class), ctx.tick);
        }
        // S6: overpressure on structure boxes and crates within the blast
        // radius with a clear line (ascending index).
        if blast_r > Fx::ZERO && !self.statics.0.is_empty() {
            let near: Vec<usize> = self
                .statics
                .0
                .iter()
                .enumerate()
                .filter(|&(k, sb)| {
                    tamped.is_none_or(|t| t.0 != k)
                        && sb.owner != b.player
                        && sb.reach >= Fx::ZERO
                        && crate::armour::box_dist_sq_raw(&sb.frame, pos)
                            <= (blast_r.raw() as u64).pow(2)
                        && ctx.terrain.line_of_sight(lift, sb.frame.center)
                })
                .map(|(k, _)| k)
                .collect();
            for k in near {
                self.hit_static(k, b.damage, Some((b.player, b.class)), ctx.tick);
            }
        }
        // S10a: overpressure on enemy wire (rule 4; nothing without wire).
        if !self.fort.wire.is_empty() && !self.fort_off {
            self.blast_wire(
                ctx.terrain,
                pos,
                lift,
                blast_r,
                b.damage,
                b.player,
                b.key.1 == Purpose::CHARGE_CRATER,
            );
        }
        // S10b R12: mines within the blast radius go off next tick
        // (nothing without mines).
        if !self.fort.mines.is_empty() && !self.fort_off {
            self.blast_mines(ctx.terrain, pos.xy(), lift, blast_r, ctx.tick);
        }
        self.fragments(ctx, b, rays, struck, rules, dead, fires, events);
        // S11a2 F4: the burst's own hits (wounds: men hit and still alive).
        if b.ff && !ctx.ff_hit.is_empty() {
            let mut hit = std::mem::take(&mut ctx.ff_hit);
            hit.sort_unstable();
            hit.dedup();
            let wounds = hit
                .iter()
                .filter(|id| ctx.ents.combat[id.index()].hp > 0)
                .count() as u32;
            if let Some(p) = self.players.get_mut(b.player as usize) {
                let c = &mut p.s11a2;
                c.ff_bursts = c.ff_bursts.saturating_add(1);
                c.ff_wounds = c.ff_wounds.saturating_add(wounds);
            }
            hit.clear();
            ctx.ff_hit = hit;
        }
        // S7: the exposed crew of open-topped vehicles.
        self.blast_open_crew(
            ctx,
            pos,
            he,
            b.damage,
            (b.player, b.class),
            b.inside,
            fires,
            events,
        );
        // A burst on a vehicle (or a man) well above the ground leaves it as
        // it is (S3).
        if pos.z > ctx.terrain.height_at(pos.xy()) + Fx::from_milli(blast.crater_lift_mm as i32) {
            return;
        }
        let mut rng = RngStream::new(ctx.seed, ctx.tick, b.key.0, b.key.1);
        let pct = rng.range_i32(
            blast.crater_depth_min_pct as i32,
            blast.crater_depth_max_pct as i32,
        );
        let depth = Fx::from(he.crater_depth).mul_int(pct).div_int(100);
        if b.cut {
            ctx.terrain
                .crater_cut(pos.xy(), he.crater_radius.into(), depth, ctx.tick);
        } else {
            // S3b: loose spoil (a parapet) under the burst flies.
            ctx.terrain.crater_spoil(
                pos.xy(),
                he.crater_radius.into(),
                depth,
                self.setup.rules.dig.common.spoil_crater_pct,
                ctx.tick,
            );
        }
    }

    /// S7: whether the line `from → to` from a burst inside an open box
    /// leaves it through an open face (always true for a burst outside).
    fn out_of_box(&self, b: &Burst, from: FxVec3, to: FxVec3) -> bool {
        let Some((v, bx)) = b.inside else {
            return true;
        };
        let (Some(f), Some(vs)) = (self.vderived.frame(v), self.vehicles.get(&v)) else {
            return true;
        };
        let plates = &self.armour.vehicles[vs.type_idx as usize];
        let k = (bx as usize).min((f.n_boxes as usize).max(1) - 1);
        let dir = (to - from).normalize_or_zero();
        let (_, face) = armour::exit_of(&f.boxes[k], from, dir);
        plates.face(armour::BoxPart(k as u8), face).open
    }

    /// `n` fragment rays of burst `b` (S3b, the physical-contact rule): from
    /// `combat.blast.lift_mm` above the burst, azimuth stratified (ray `k`
    /// at `k · 360° / n` plus a triangular jitter of half a stratum),
    /// elevation uniform in the HE def's band above the ground plane
    /// (`[-max, max]` for a burst well above the ground: on a vehicle or a
    /// man). S3c: a ground burst where the ground falls away within
    /// `combat.blast.frag_fall_probe_cm` (8 samples) adds rays below the
    /// band, down to the steepest fall's angle, at the band's density
    /// (at most `n` more; flat ground adds none). Each ray stops at the first enemy soldier capsule, vehicle,
    /// wreck or barrel box it meets within `frag_reach`, unless the earth
    /// stops it first (`Heightfield::segment_hit`, so trench walls and
    /// parapets block). A man struck takes `frag_damage` falling linearly
    /// to `frag_far_pct` at full reach, and the rules' hit suppression; an
    /// enemy vehicle struck in a hull side's track band takes
    /// `frag_track_damage` on that track. Friendly men are not in the way
    /// (no friendly fire); a man whose capsule holds the ray origin (the
    /// burst is on him) is left to the overpressure. RNG `FRAGMENT` /
    /// `ITEM_FRAGMENT` keyed by the burst's key, rays in order.
    #[allow(clippy::too_many_arguments)]
    fn fragments(
        &mut self,
        ctx: &mut DamageCtx,
        b: &Burst,
        n: u32,
        struck: Option<SimId>,
        rules: &CombatSetup,
        dead: &mut Vec<Fallen>,
        fires: &mut FireRolls,
        events: &mut Vec<ArmourEv>,
    ) {
        let he = b.he;
        let reach: Fx = he.frag_reach.into();
        if n == 0 || reach <= Fx::ZERO {
            return;
        }
        ctx.stats.frag_rays += n as u64;
        let pos = b.pos;
        let o = FxVec3::new(
            pos.x,
            pos.y,
            pos.z + Fx::from_milli(rules.blast.lift_mm as i32),
        );
        let air = pos.z
            > ctx.terrain.height_at(pos.xy()) + Fx::from_milli(rules.blast.crater_lift_mm as i32);
        let [e0, e1] = he.frag_elev_deg;
        let (elo, ehi) = if air {
            (armour::deg(-e1), armour::deg(e1))
        } else {
            (armour::deg(e0), armour::deg(e1))
        };
        // S3c (B8): where the ground falls away beside a ground burst,
        // extra rays go down over the fall at the band's density.
        let (fall_lo, extra) = if air || rules.blast.frag_fall_probe_cm == 0 || ehi <= elo {
            (elo, 0)
        } else {
            let r = Fx::from_ratio(rules.blast.frag_fall_probe_cm as i64, 100);
            let h0 = ctx.terrain.height_at(pos.xy());
            let mut drop = Fx::ZERO;
            for k in 0..8 {
                let (s, c) = Fx::from_ratio(k as i64 * 785, 1000).sin_cos(); // k · 45°
                let at = ctx.terrain.clamp(pos.xy() + FxVec2::new(c, s) * r);
                drop = drop.max(h0 - ctx.terrain.height_at(at));
            }
            if drop <= Fx::ZERO {
                (elo, 0)
            } else {
                let lo = elo - Fx::atan2(drop, r);
                let extra = ((n as i64 * (elo - lo).raw() as i64) / (ehi - elo).raw() as i64)
                    .clamp(0, n as i64) as u32;
                (lo, extra)
            }
        };
        // Enemy men within reach, ascending id: (id, feet, capsule).
        let mut ids = Vec::new();
        ctx.spatial
            .for_each_near(pos.xy(), reach + Fx::ONE, |id| ids.push(id));
        ids.sort_unstable();
        ids.dedup();
        let mut men: Vec<(SimId, FxVec3, FxVec3, FxVec3, Fx)> = Vec::new();
        for id in ids {
            let i = id.index();
            let c = &ctx.ents.combat[i];
            // S11a2 F3: an own mission burst's fragments find own men too.
            if ctx.ents.kind[i] != EntityKind::Soldier
                || (ctx.ents.owner[i].0 == b.player && !b.ff)
                || c.hp == 0
            {
                continue;
            }
            let t = &ctx.ents.transform[i];
            let (a, e, r) = capsule(t, c, &rules.body);
            // The burst is on him: the overpressure's.
            let (_, inside) = seg_seg(o - t.pos, FxVec3::ZERO, a - t.pos, e - a);
            if inside <= r.mul(r) {
                continue;
            }
            men.push((id, t.pos, a, e, r));
        }
        // Vehicles and wrecks within reach, ascending id.
        let mut vids = Vec::new();
        if !self.vderived.frames.is_empty() {
            self.vderived
                .index
                .for_each_near(pos.xy(), reach + self.vderived.max_reach, |id| {
                    vids.push(id)
                });
            vids.sort_unstable();
            vids.dedup();
        }
        let frames: Vec<VehicleFrame> = vids
            .iter()
            .filter_map(|&id| self.vderived.frame(id).copied())
            .collect();
        let purpose = if b.key.1 == Purpose::ITEM_CRATER {
            Purpose::ITEM_FRAGMENT
        } else if b.key.1 == Purpose::COOKOFF_CRATER {
            Purpose::COOKOFF_FRAGMENT
        } else if b.key.1 == Purpose::CHARGE_CRATER {
            Purpose::CHARGE_FRAGMENT
        } else if b.key.1 == Purpose::DEMO_CRATER {
            Purpose::DEMO_FRAGMENT
        } else {
            Purpose::FRAGMENT
        };
        // S6: live static boxes within reach not of this burst's player:
        // (index, frame). S11a2 F3: an own mission burst's rays stop at own
        // boxes too (they take nothing: see the hit below).
        let statics: Vec<(usize, crate::vehicle::BoxFrame)> = self
            .statics
            .0
            .iter()
            .enumerate()
            .filter(|(_, sb)| {
                (sb.owner != b.player || b.ff)
                    && sb.reach >= Fx::ZERO
                    && armour::segment_near(o, FxVec3::ZERO, sb.frame.center, reach + sb.reach)
            })
            .map(|(k, sb)| (k, sb.frame))
            .collect();
        let mut static_hits: Vec<(usize, u32)> = Vec::new();
        // S10a: wire sections struck (rule 4), in ray order.
        let mut wire_hits: Vec<(u32, u32)> = Vec::new();
        let wire_pct = self
            .setup
            .rules
            .fortify
            .as_ref()
            .map_or(0, |f| f.wire_frag_pct);
        let mut rng = RngStream::new(ctx.seed, ctx.tick, b.key.0, purpose);
        let stratum = Fx::TAU.div_int(n as i32);
        let half = stratum.div_int(2);
        let pad = Fx::from_int(VEHICLE_SPHERE_PAD_M);
        ctx.stats.frag_rays += extra as u64;
        let mut tracks: Vec<(SimId, ComponentKind, u32)> = Vec::new();
        // S7: seats and parts of open-topped vehicles struck, in ray order.
        let mut crew: Vec<(SimId, Inner, u32)> = Vec::new();
        let x_stratum = Fx::TAU.div_int(extra.max(1) as i32);
        let x_half = x_stratum.div_int(2);
        for k in 0..n + extra {
            // The band's rays first (as without the fall), then the extra
            // ones below it.
            let el = if k < n {
                let az = stratum.mul_int(k as i32) + half + tri(&mut rng, half);
                (az, rng.range_fx(elo, ehi))
            } else {
                let j = (k - n) as i32;
                let az = x_stratum.mul_int(j) + x_half + tri(&mut rng, x_half);
                (az, rng.range_fx(fall_lo, elo))
            };
            let (az, el) = el;
            let (sa, ca) = az.sin_cos();
            let (se, ce) = el.sin_cos();
            let dir = FxVec3::new(ca.mul(ce), sa.mul(ce), se);
            let d = dir * reach;
            let mut man: Option<(Fx, usize)> = None;
            for (j, &(_, feet, a, e, r)) in men.iter().enumerate() {
                let (s, dsq) = seg_seg(o - feet, d, a - feet, e - a);
                if dsq <= r.mul(r) && man.is_none_or(|(ms, _)| s < ms) {
                    man = Some((s, j));
                }
            }
            let mut boxed: Option<(Fx, usize, ArmourHit)> = None;
            let mut blocked = false;
            for (j, f) in frames.iter().enumerate() {
                if let Some((iv, ib)) = b.inside
                    && iv == f.id
                {
                    // S7: a burst inside an open box: rays leave only
                    // through its open face (and then miss this vehicle).
                    let plates = &self.armour.vehicles[f.type_idx as usize];
                    let bx = (ib as usize).min((f.n_boxes as usize).max(1) - 1);
                    let (_, face) = armour::exit_of(&f.boxes[bx], o, dir);
                    if !plates.face(armour::BoxPart(bx as u8), face).open {
                        blocked = true;
                    }
                    continue;
                }
                if Some(f.id) == struck {
                    // A burst on its plate: rays into it stop there.
                    let probe = o + dir * STRUCK_PROBE;
                    if f.armour_boxes()
                        .iter()
                        .any(|b| armour::box_dist_sq_raw(b, probe) == 0)
                    {
                        blocked = true;
                        continue;
                    }
                    // S7: a ray leaving the plate may still reach the
                    // vehicle's exposed crew or an open face (never another
                    // of its plates: closed vehicles as before).
                    let plates = &self.armour.vehicles[f.type_idx as usize];
                    if (!plates.exposed.is_empty() || f.n_riders > 0)
                        && let Some(h) = armour::hit_armour(o, d, f, plates)
                        && h.via != armour::HitVia::Plate
                        && boxed.is_none_or(|(bt, _, _)| h.t < bt)
                    {
                        boxed = Some((h.t, j, h));
                    }
                    continue;
                }
                if !armour::segment_near(o, d, f.origin, f.reach + pad) {
                    continue;
                }
                let plates = &self.armour.vehicles[f.type_idx as usize];
                if let Some(h) = armour::hit_armour(o, d, f, plates)
                    && boxed.is_none_or(|(bt, _, _)| h.t < bt)
                {
                    boxed = Some((h.t, j, h));
                }
            }
            if blocked {
                continue;
            }
            // S6: a structure box or crate nearer than any man or vehicle
            // stops the ray (unless the earth does first).
            let sbox = statics
                .iter()
                .filter_map(|(k, f)| armour::segment_box(f, o, d).map(|h| (h.0, *k)))
                .min();
            // S10a: a fragment crossing enemy wire below its height before
            // the ray stops deals its share and flies on (rule 4).
            if !self.fort.wire.is_empty() && !self.fort_off {
                let stop = [man.map(|m| m.0), boxed.map(|x| x.0), sbox.map(|x| x.0)]
                    .into_iter()
                    .flatten()
                    .min()
                    .unwrap_or(Fx::ONE);
                for (id, f) in self.frag_wire(ctx.terrain, o, d, stop, b.player) {
                    let lost = Fx::from_int(100 - he.frag_far_pct.min(100) as i32).mul(f);
                    let pct = (Fx::from_int(100) - lost).max(Fx::ZERO);
                    let dmg = (he.frag_damage as i64 * pct.raw() as i64 / (100i64 << Fx::FRAC_BITS))
                        as u32;
                    wire_hits.push((id, dmg * wire_pct / 100));
                }
            }
            if let Some((st, k)) = sbox
                && boxed.is_none_or(|(bt, _, _)| st < bt)
                && man.is_none_or(|(ms, _)| st < ms)
            {
                // S11a2 F3: an own box only stops the ray.
                let own_box = self.statics.0.get(k).is_some_and(|sb| sb.owner == b.player);
                if !own_box && ctx.terrain.segment_hit(o, o + d * st).is_none() {
                    let lost = Fx::from_int(100 - he.frag_far_pct.min(100) as i32).mul(st);
                    let pct = (Fx::from_int(100) - lost).max(Fx::ZERO);
                    let dmg = (he.frag_damage as i64 * pct.raw() as i64 / (100i64 << Fx::FRAC_BITS))
                        as u32;
                    // S10a: a piece takes a fragment only when the
                    // fragment's pen there beats its armour (S8a rule).
                    let armour = self.statics.0.get(k).map_or(0, |sb| sb.armour_mm);
                    let pierces = armour == 0
                        || Fx::from_raw(
                            ((he.frag_pen_mm as i64 * pct.raw() as i64) / 100).min(i32::MAX as i64)
                                as i32,
                        ) > Fx::from_int(armour as i32);
                    if pierces {
                        static_hits.push((k, dmg));
                    }
                    ctx.stats.frag_hits += 1;
                }
                continue;
            }
            let box_first = box_before(boxed.map(|x| x.0), man.map(|m| m.0));
            let end = match (box_first, boxed, man) {
                (true, Some((t, _, _)), _) => t,
                (false, _, Some((s, _))) => s,
                _ => continue,
            };
            if ctx.terrain.segment_hit(o, o + d * end).is_some() {
                continue;
            }
            if box_first {
                let (bt, j, h) = boxed.expect("box first");
                let f = &frames[j];
                if f.wreck || f.owner.0 == b.player {
                    continue;
                }
                let lost = Fx::from_int(100 - he.frag_far_pct.min(100) as i32).mul(bt);
                let pct = (Fx::from_int(100) - lost).max(Fx::ZERO);
                let dmg =
                    (he.frag_damage as i64 * pct.raw() as i64 / (100i64 << Fx::FRAC_BITS)) as u32;
                // S8a thin-plate rule (S7-8): the fragment's penetration
                // falls with distance as its damage does.
                let frag_pen = Fx::from_raw(
                    ((he.frag_pen_mm as i64 * pct.raw() as i64) / 100).min(i32::MAX as i64) as i32,
                );
                let plates = &self.armour.vehicles[f.type_idx as usize];
                let pierces = |h: &ArmourHit, t: Fx| {
                    frag_pen > Fx::ZERO
                        && armour::effective_thickness(
                            armour::impact_cos(dir, h.normal),
                            t,
                            Fx::ZERO,
                        ) < frag_pen
                };
                // S8a: a shield the fragment pierces lets it on to what is
                // behind it on this vehicle (one more hit), else stops it.
                let h = match h.via {
                    armour::HitVia::Shield(k) => {
                        let t = plates.shields.get(k as usize).copied().unwrap_or(Fx::ZERO);
                        if !pierces(&h, t) {
                            continue;
                        }
                        ctx.stats.frag_plate_pierces += 1;
                        let rest = d * (Fx::ONE - bt);
                        match armour::hit_armour_past(h.point, rest, f, plates, Some(k)) {
                            Some(next) => next,
                            None => continue,
                        }
                    }
                    _ => h,
                };
                let pierced = h.via == armour::HitVia::Plate
                    && h.part != armour::BoxPart::BARREL
                    && pierces(&h, plates.face(h.part, h.face).thickness);
                if pierced {
                    ctx.stats.frag_plate_pierces += 1;
                }
                // S7: through an open face or into an exposed seat (S8a: or
                // through a thin plate): the first seat or component inside
                // takes the fragment.
                if h.via != armour::HitVia::Plate || pierced {
                    let inner = match h.via {
                        armour::HitVia::Seat(k) => Some(Inner::Seat(k)),
                        armour::HitVia::Rider(k) => Some(Inner::Rider(k)),
                        armour::HitVia::Shield(_) => None,
                        _ => {
                            let v = &self.setup.vehicles[f.type_idx as usize];
                            let boxes = crate::damage::inner_boxes(f, v);
                            let bx = (h.part.0 as usize).min((f.n_boxes as usize).max(1) - 1);
                            let len = armour::exit_of(&f.boxes[bx], h.point, dir).0;
                            crate::damage::ray_boxes(&boxes, h.point, dir * len, 1)
                                .first()
                                .map(|&k| boxes[k].1)
                        }
                    };
                    if let Some(inner) = inner
                        && dmg > 0
                    {
                        crew.push((f.id, inner, dmg));
                        ctx.stats.frag_hits += 1;
                    }
                    continue;
                }
                if he.frag_track_damage == 0 {
                    continue;
                }
                if armour::in_track_band(&h, f.hull.half.z, plates.track_band_pct) {
                    let side =
                        crate::damage::band_part(&self.setup.vehicles[f.type_idx as usize], &h);
                    match tracks.iter_mut().find(|(v, c, _)| *v == f.id && *c == side) {
                        Some(t) => t.2 += he.frag_track_damage,
                        None => tracks.push((f.id, side, he.frag_track_damage)),
                    }
                    ctx.stats.frag_hits += 1;
                }
            } else {
                let (s, j) = man.expect("man first");
                let id = men[j].0;
                // 100 % at the burst, `frag_far_pct` at full reach.
                let lost = Fx::from_int(100 - he.frag_far_pct.min(100) as i32).mul(s);
                let pct = (Fx::from_int(100) - lost).max(Fx::ZERO);
                let dmg =
                    (he.frag_damage as i64 * pct.raw() as i64 / (100i64 << Fx::FRAC_BITS)) as u32;
                add_suppression(
                    &mut ctx.ents.combat[id.index()],
                    rules.hit_suppression * 1000,
                );
                hurt_by(ctx, dead, b, id, dmg);
                ctx.stats.frag_hits += 1;
            }
        }
        // S6: boxes struck, in ray order (HE: a broken box with shells or
        // explosives cooks off).
        for (k, dmg) in static_hits {
            self.hit_static(k, dmg, Some((b.player, b.class)), ctx.tick);
        }
        // S10a: wire struck, in ray order.
        let how = if b.key.1 == Purpose::CHARGE_CRATER {
            crate::fort::WireGone::Blown
        } else {
            crate::fort::WireGone::Broken
        };
        for (id, dmg) in wire_hits {
            if self
                .fort
                .hurt_wire(id, dmg, crate::ecs::PlayerId(b.player), how)
            {
                self.wire_gone(id);
            }
        }
        // Tracks struck, ascending vehicle id (no fire, no rack: no
        // knock-out).
        tracks.sort_unstable_by_key(|&(v, c, _)| (v, c));
        let mut k = 0;
        while k < tracks.len() {
            let id = tracks[k].0;
            let dealt: Vec<Dealt> = tracks[k..]
                .iter()
                .take_while(|t| t.0 == id)
                .map(|&(_, c, dmg)| Dealt {
                    inner: Inner::Part(c),
                    dmg,
                })
                .collect();
            k += dealt.len();
            if ctx.ents.kind.get(id.index()) != Some(&EntityKind::Vehicle) {
                continue;
            }
            let vs = self.vehicles.get_mut(&id).expect("vehicle state");
            vs.repairing = None;
            vs.repair_progress = 0;
            vs.last_attacker = Some(b.player);
            self.deal(
                ctx,
                id,
                &dealt,
                (b.player, kill_class::OTHER),
                id,
                fires,
                events,
                pos,
                NO_AMMO,
            );
        }
        // S7: open-topped vehicles' crew and parts struck, ascending
        // vehicle id (ray order within one).
        crew.sort_by_key(|c| c.0);
        let mut k = 0;
        while k < crew.len() {
            let id = crew[k].0;
            let dealt: Vec<Dealt> = crew[k..]
                .iter()
                .take_while(|c| c.0 == id)
                .map(|&(_, inner, dmg)| Dealt { inner, dmg })
                .collect();
            k += dealt.len();
            self.hit_open(ctx, id, &dealt, (b.player, b.class), pos, fires, events);
        }
    }
}

#[inline]
pub(crate) fn add_suppression(c: &mut Combat, milli: u32) {
    c.suppression = c.suppression.saturating_add(milli).min(100_000);
}

/// Whether a man is crouched below cover (in a trench, not on the step).
#[inline]
fn covered(c: &Combat) -> bool {
    c.in_trench && c.height < c.fire_height
}

impl MatchState {
    /// The `game::kill_class` of a round (S3 report): its ammo for main-gun
    /// shells and rockets, else its weapon class; an MG fired by a vehicle
    /// is a vehicle MG.
    pub(crate) fn round_class(
        &self,
        ents: &Entities,
        weapon: u16,
        ammo: u16,
        shooter: SimId,
    ) -> u16 {
        let base = self.base_class(ents, weapon, ammo, shooter);
        // S7: a round from a vehicle (alive, wreck or abandoned since)
        // carries its type, for the per-type report.
        match self.vehicles.get(&shooter) {
            Some(vs)
                if matches!(
                    ents.kind.get(shooter.index()),
                    Some(EntityKind::Vehicle | EntityKind::Wreck | EntityKind::Abandoned)
                ) =>
            {
                kill_class::with_vehicle(base, vs.type_idx)
            }
            _ => base,
        }
    }

    /// [`MatchState::round_class`] without the shooting vehicle's type.
    fn base_class(&self, ents: &Entities, weapon: u16, ammo: u16, shooter: SimId) -> u16 {
        let class = self.setup.weapons.get(weapon as usize).map(|w| w.class);
        if class == Some(WeaponClass::Launcher) {
            // S11b G4: a launcher round whose ammo is HE is a rifle
            // grenade; HEAT stays a rocket.
            let he = self
                .setup
                .ammo
                .get(ammo as usize)
                .is_some_and(|a| a.mechanism == AmmoMechanism::He);
            return if he {
                kill_class::RIFLE_GRENADE
            } else {
                kill_class::ROCKET
            };
        }
        if let Some(a) = self.setup.ammo.get(ammo as usize) {
            return match a.mechanism {
                AmmoMechanism::He => kill_class::TANK_HE,
                _ => kill_class::TANK_AP,
            };
        }
        match class {
            Some(WeaponClass::Rifle) => kill_class::RIFLE,
            // S3c: a crew's pistol counts with the SMGs (close range).
            Some(WeaponClass::Smg | WeaponClass::Pistol) => kill_class::SMG,
            Some(WeaponClass::Mg) => {
                if matches!(
                    ents.kind.get(shooter.index()),
                    Some(EntityKind::Vehicle | EntityKind::Wreck | EntityKind::Abandoned)
                ) {
                    kill_class::VEHICLE_MG
                } else {
                    kill_class::MG
                }
            }
            Some(WeaponClass::Mortar) => kill_class::MORTAR,
            Some(WeaponClass::Cannon) => kill_class::TANK_AP,
            _ => kill_class::OTHER,
        }
    }

    /// The blast a bursting event sets off, if any: mortar shells burst by
    /// their weapon's HE, main-gun HE and rockets by their round's blast
    /// (`(he, damage, suppression)`).
    fn event_blast(&self, e: &BallisticEvent) -> Option<(HeSetup, u32, u32)> {
        match self.setup.ammo.get(e.ammo as usize) {
            Some(a) => a
                .blast
                .as_ref()
                .map(|b| (b.blast.clone(), b.damage, b.suppression)),
            None => {
                let w = self.setup.weapons.get(e.weapon as usize)?;
                w.he.clone().map(|h| (h, w.damage, w.suppression))
            }
        }
    }

    /// Applies this tick's ballistic events in order, then the thrown items
    /// that go off now (`crate::items`, in id order: blasts, smoke), then
    /// kills soldiers left at 0 HP (see the module docs). Bursts are
    /// reported in `explosions`; `fused` is scratch.
    pub(crate) fn apply_blows(
        &mut self,
        ctx: &mut DamageCtx,
        events: &[BallisticEvent],
        explosions: &mut Vec<Explosion>,
        fused: &mut Vec<Thrown>,
        armour_events: &mut Vec<ArmourEv>,
    ) {
        let rules = self.setup.rules.combat.clone();
        // (victim, killer, class) in the order they fell.
        let mut dead: Vec<Fallen> = Vec::new();
        let mut fires = FireRolls::default();
        // S5b: bursts of fire-mission rounds, for the observers.
        let mut mbursts: Vec<crate::fire_mission::MissionBurst> = Vec::new();
        self.take_fused(fused);
        // S10b: (burst key, piece) of the demolition charges due now.
        let mut demo: Vec<(u32, u32)> = Vec::new();
        // S10a: Bangalore charges due now burst as items (rule 6).
        if !self.fort_off {
            self.take_charges(ctx.terrain, ctx.tick, fused, &mut demo);
            // S10b: mines set off now burst the same way (R3-R7).
            self.take_mines(ctx.ents, ctx.spatial, ctx.terrain, ctx.tick, fused);
        }
        // S10b: men a mine burst dropped (`men_lost_to_mines`).
        let mut mine_dead: Vec<SimId> = Vec::new();
        // S3b: the fragment budget, shared by this tick's bursts in
        // proportion to what each asks for.
        let requested: u64 = events
            .iter()
            .filter(|e| matches!(e.blow, Blow::Burst(_) | Blow::Armour { .. }))
            .filter_map(|e| self.event_blast(e))
            .map(|(h, _, _)| h.frag_count as u64)
            .chain(fused.iter().filter_map(|t| {
                self.setup.items[t.item as usize]
                    .he
                    .as_ref()
                    .map(|h| h.blast.frag_count as u64)
            }))
            .sum();
        for (k, e) in events.iter().enumerate() {
            let burst_at = match e.blow {
                Blow::Hit(id) => {
                    if ctx.ents.kind[id.index()] != EntityKind::Soldier {
                        continue;
                    }
                    let w = &self.setup.weapons[e.weapon as usize];
                    ctx.stats.soldier_hits += 1;
                    add_suppression(
                        &mut ctx.ents.combat[id.index()],
                        rules.hit_suppression * 1000,
                    );
                    let class = self.round_class(ctx.ents, e.weapon, e.ammo, e.shooter);
                    hurt(
                        ctx.ents,
                        ctx.hash,
                        &mut dead,
                        id,
                        w.damage,
                        (e.player, class),
                    );
                    None
                }
                Blow::NearMiss(id) => {
                    let w = &self.setup.weapons[e.weapon as usize];
                    let c = &mut ctx.ents.combat[id.index()];
                    if ctx.ents.kind[id.index()] != EntityKind::Soldier || c.hp == 0 {
                        continue;
                    }
                    ctx.stats.near_misses += 1;
                    let mut add = w.suppression * 1000;
                    if covered(c) {
                        add = (add as u64 * rules.suppression.covered_near_miss_pct as u64 / 100)
                            as u32;
                    }
                    add_suppression(c, add);
                    ctx.hash.mark_entity_chunk(id.chunk());
                    None
                }
                Blow::Stopped(v) => {
                    ctx.stats.rounds_stopped += 1;
                    self.suppress_riders(ctx, v, e.weapon, &rules);
                    None
                }
                Blow::Static(k, range) => {
                    ctx.stats.rounds_stopped += 1;
                    let sp = self.setup.rules.supply.as_ref();
                    let piece = self
                        .statics
                        .0
                        .get(k as usize)
                        .filter(|sb| {
                            !self.fort_off
                                && matches!(sb.of, crate::structures::StaticRef::Piece(_))
                        })
                        .map(|sb| sb.armour_mm);
                    let dmg = match piece {
                        // S10a: a bullet damages a piece only when its pen
                        // at that range beats the piece's armour; a solid
                        // main-gun round takes `shot_piece_damage`.
                        Some(armour) if e.ammo == NO_AMMO => {
                            let w = &self.setup.weapons[e.weapon as usize];
                            let reach =
                                armour::small_reach(w, Fx::from_int(armour as i32), Fx::ONE);
                            if reach > Fx::ZERO && range <= reach {
                                sp.map_or(0, |s| s.bullet_box_damage)
                            } else {
                                0
                            }
                        }
                        Some(_) => self
                            .setup
                            .rules
                            .fortify
                            .as_ref()
                            .map_or(0, |f| f.shot_piece_damage),
                        None if e.ammo == NO_AMMO => sp.map_or(0, |s| s.bullet_box_damage),
                        None => sp.map_or(0, |s| s.shell_box_damage),
                    };
                    self.hit_static(k as usize, dmg, None, ctx.tick);
                    None
                }
                Blow::Burst(pos) => Some(pos),
                Blow::Armour { vehicle, .. } => {
                    if e.ammo == NO_AMMO {
                        self.suppress_riders(ctx, vehicle, e.weapon, &rules);
                    }
                    self.armour_blow(ctx, e, &mut fires, armour_events)
                }
            };
            let struck = match e.blow {
                Blow::Armour { vehicle, .. } => Some(vehicle),
                _ => None,
            };
            let Some(pos) = burst_at else { continue };
            if e.mission != 0 {
                mbursts.push(crate::fire_mission::MissionBurst {
                    mission: e.mission,
                    pos,
                    player: e.player,
                });
            }
            let Some((he, damage, suppression)) = self.event_blast(e) else {
                continue;
            };
            ctx.stats.explosions += 1;
            explosions.push(Explosion {
                pos,
                weapon: e.weapon,
                kind: BlastKind::Shell,
            });
            let b = Burst {
                pos,
                player: e.player,
                he: &he,
                damage,
                suppression,
                cut: false,
                key: (SimId(k as u32), Purpose::CRATER),
                class: self.round_class(ctx.ents, e.weapon, e.ammo, e.shooter),
                piece: None,
                inside: None,
                // S11a2 F1: a fire-mission round (mortar or gun, any mode).
                ff: e.mission != 0
                    && self
                        .setup
                        .rules
                        .fire_mission
                        .as_ref()
                        .is_some_and(|f| f.friendly_fire != 0),
            };
            let rays = frag_share(he.frag_count, requested);
            ctx.stats.frag_rays_capped += (he.frag_count - rays) as u64;
            self.burst(
                ctx,
                &b,
                rays,
                struck,
                &rules,
                &mut dead,
                &mut fires,
                armour_events,
            );
            self.blast_tracks(
                ctx,
                pos,
                &he,
                damage,
                e.player,
                struck,
                e.ammo,
                &mut fires,
                armour_events,
            );
        }
        // S6: cook-off bursts due now (ascending cook-off id).
        for (id, pos, player, class, ammo) in self.due_cookoffs(ctx.tick) {
            let Some(bl) = self
                .setup
                .ammo
                .get(ammo as usize)
                .and_then(|a| a.blast.clone())
            else {
                continue;
            };
            ctx.stats.explosions += 1;
            explosions.push(Explosion {
                pos,
                weapon: u16::MAX,
                kind: BlastKind::Shell,
            });
            let b = Burst {
                pos,
                player,
                he: &bl.blast,
                damage: bl.damage,
                suppression: bl.suppression,
                cut: false,
                key: (SimId(id), Purpose::COOKOFF_CRATER),
                class,
                inside: None,
                piece: None,
                ff: false,
            };
            let rays = frag_share(bl.blast.frag_count, requested);
            self.burst(
                ctx,
                &b,
                rays,
                None,
                &rules,
                &mut dead,
                &mut fires,
                armour_events,
            );
        }
        for t in fused.iter() {
            let it = &self.setup.items[t.item as usize];
            let mine = it.kind == frontline_content::ItemKind::Mine;
            // S10b: a demolition charge's target piece (never a mine's
            // burst, which shares the u32::MAX thrower; review R3).
            let demo_piece = (t.thrower == SimId(u32::MAX) && !mine)
                .then(|| demo.iter().find(|d| d.0 == t.id).map(|d| d.1))
                .flatten();
            if let Some(h) = it.he.clone() {
                ctx.stats.explosions += 1;
                let kind = if h.cuts_trenches {
                    BlastKind::Satchel
                } else {
                    BlastKind::Frag
                };
                explosions.push(Explosion {
                    pos: t.pos,
                    weapon: t.item,
                    kind,
                });
                let b = Burst {
                    pos: t.pos,
                    player: t.owner.0,
                    he: &h.blast,
                    damage: h.damage,
                    suppression: h.suppression,
                    cut: h.cuts_trenches,
                    key: (
                        SimId(t.id),
                        if it.kind == frontline_content::ItemKind::Bangalore {
                            Purpose::CHARGE_CRATER
                        } else if demo_piece.is_some() {
                            Purpose::DEMO_CRATER
                        } else {
                            Purpose::ITEM_CRATER
                        },
                    ),
                    class: if h.cuts_trenches {
                        kill_class::SATCHEL
                    } else {
                        kill_class::FRAG
                    },
                    inside: t
                        .inside
                        .filter(|i| self.vderived.frame(i.vehicle).is_some())
                        .map(|i| (i.vehicle, i.part)),
                    piece: demo_piece,
                    ff: false,
                };
                // S7: inside an open box, its inner rays first.
                if b.inside.is_some() {
                    self.inside_item(ctx, t, &b, &mut fires, armour_events);
                }
                let rays = frag_share(h.blast.frag_count, requested);
                ctx.stats.frag_rays_capped += (h.blast.frag_count - rays) as u64;
                let before = dead.len();
                self.burst(
                    ctx,
                    &b,
                    rays,
                    None,
                    &rules,
                    &mut dead,
                    &mut fires,
                    armour_events,
                );
                if mine {
                    mine_dead.extend(dead[before..].iter().map(|f| f.0));
                }
                if b.inside.is_some() {
                    continue;
                }
                // S10b: vehicles this mine immobilises (`vehicles_mined`).
                let mobile: Vec<SimId> = if mine {
                    self.vehicles
                        .iter()
                        .filter(|(_, vs)| {
                            !vs.immobilised(&self.setup.vehicles[vs.type_idx as usize])
                        })
                        .map(|(&id, _)| id)
                        .collect()
                } else {
                    Vec::new()
                };
                self.blast_tracks(
                    ctx,
                    t.pos,
                    &h.blast,
                    h.damage,
                    t.owner.0,
                    None,
                    NO_AMMO,
                    &mut fires,
                    armour_events,
                );
                let mined = mobile
                    .iter()
                    .filter(|id| {
                        self.vehicles.get(id).is_some_and(|vs| {
                            vs.immobilised(&self.setup.vehicles[vs.type_idx as usize])
                        })
                    })
                    .count() as u32;
                if mined > 0
                    && let Some(a) = self.fort.accounts.get_mut(t.owner.0 as usize)
                {
                    a.vehicles_mined = a.vehicles_mined.saturating_add(mined);
                }
            }
        }
        for t in fused.iter() {
            if self.setup.items[t.item as usize].smoke.is_some() {
                self.start_smoke(t.item, t.pos);
            }
        }
        dead.append(&mut ctx.fallen);
        dead.sort_unstable();
        ctx.ff_dead.sort_unstable();
        for (id, by, class) in dead {
            if !mine_dead.is_empty()
                && mine_dead.contains(&id)
                && ctx.ents.kind[id.index()] == EntityKind::Soldier
                && let Some(a) = self
                    .fort
                    .accounts
                    .get_mut(ctx.ents.owner[id.index()].0 as usize)
            {
                a.men_lost_to_mines = a.men_lost_to_mines.saturating_add(1);
            }
            self.kill(ctx.ents, ctx.hash, id, rules.corpse_ticks);
            ctx.stats.deaths += 1;
            // S11a2 F4: killed by his own side's mission burst: a loss (in
            // `kill`) and no kill; mortar or gun by the burst's class.
            if !ctx.ff_dead.is_empty() && ctx.ff_dead.binary_search(&id).is_ok() {
                if let Some(p) = self.players.get_mut(by as usize) {
                    let k = (kill_class::base(class) != kill_class::MORTAR) as usize;
                    p.s11a2.ff_kills[k] = p.s11a2.ff_kills[k].saturating_add(1);
                }
                continue;
            }
            if let Some(p) = self.players.get_mut(by as usize) {
                p.kills += 1;
                p.kills_by[kill_class::base(class) as usize] += 1;
                // S7: per shooting vehicle type.
                if let Some(n) =
                    kill_class::vehicle(class).and_then(|t| p.s7.kills_by_vehicle.get_mut(t))
                {
                    *n += 1;
                }
            }
            // S9: with a captured gun.
            if let Some(t) = kill_class::vehicle(class) {
                self.note_gun_kill(by as usize, t, 1);
            }
        }
        self.sweep_vehicle_refs(ctx.ents);
        if !mbursts.is_empty() {
            self.observe_bursts(ctx.ents, ctx.terrain, &mbursts, ctx.tick);
        }
    }

    /// S8a: a small-arms round striking transport `v` is a near miss for
    /// its passengers (covered: `covered_near_miss_pct`).
    fn suppress_riders(&self, ctx: &mut DamageCtx, v: SimId, weapon: u16, rules: &CombatSetup) {
        let Some(vs) = self.vehicles.get(&v) else {
            return;
        };
        if vs.riders.iter().all(|r| r.is_none()) {
            return;
        }
        let Some(w) = self.setup.weapons.get(weapon as usize) else {
            return;
        };
        let add = (w.suppression as u64 * 1000 * rules.suppression.covered_near_miss_pct as u64
            / 100)
            .min(u32::MAX as u64) as u32;
        for m in vs.riders.iter().flatten() {
            add_suppression(&mut ctx.ents.combat[m.index()], add);
            ctx.hash.mark_entity_chunk(m.chunk());
        }
    }

    /// Turns soldier `id` into a corpse and takes him out of his squad.
    pub(crate) fn kill(
        &mut self,
        ents: &mut Entities,
        hash: &mut HashCache,
        id: SimId,
        corpse_ticks: u32,
    ) {
        let i = id.index();
        let owner = ents.owner[i].0 as usize;
        // S4: what he carried (a crew weapon is dropped where he fell);
        // S5a: with the rounds left in it.
        let carried = ents.kit[i].weapon;
        let left_in = if ents.kit[i].on_sidearm {
            0
        } else {
            ents.combat[i].mag
        };
        let fell_at = ents.transform[i].pos.xy();
        // S8a: a passenger leaves his slot; his body lies on the vehicle's
        // ground level where he sat.
        if let Some(r) = ents.unit[i].riding {
            if let Some(vs) = self.vehicles.get_mut(&r.vehicle)
                && vs.riders.get(r.slot as usize) == Some(&Some(id))
            {
                vs.riders[r.slot as usize] = None;
            }
            ents.transform[i].pos.z = ents.transform[r.vehicle.index()].pos.z;
            if let Some(p) = self.players.get_mut(owner) {
                p.s8a.riders_lost = p.s8a.riders_lost.saturating_add(1);
            }
        }
        // S9: a gun crewman leaves his slot.
        if let Some(s) = ents.unit[i].serving
            && let Some(sv) = self
                .vehicles
                .get_mut(&s.gun)
                .and_then(|v| v.served.as_mut())
            && let Some(sl) = sv.slots.get_mut(s.slot as usize)
            && sl.man == Some(id)
        {
            sl.man = None;
            // S11b X1: an infantry gun's crew counts in `s11b`.
            let inf_gun = self.vehicles.get(&s.gun).is_some_and(|v| {
                self.setup.vehicles[v.type_idx as usize].role
                    == frontline_content::VehicleRole::InfGun
            });
            if let Some(p) = self.players.get_mut(owner) {
                if inf_gun {
                    p.s11b.arty_crew_lost = p.s11b.arty_crew_lost.saturating_add(1);
                } else {
                    p.s9.gun_crew_lost = p.s9.gun_crew_lost.saturating_add(1);
                }
            }
        }
        // S10a-ai: a man killed inside an enemy wire band.
        if !self.fort.wire.is_empty()
            && !self.fort_off
            && self
                .fort
                .in_enemy_wire(&self.setup.forts, fell_at, PlayerId(owner as u8))
            && let Some(a) = self.fort.accounts.get_mut(owner)
        {
            a.men_lost_in_wire = a.men_lost_in_wire.saturating_add(1);
        }
        ents.kind[i] = EntityKind::Corpse;
        ents.velocity[i] = Velocity::default();
        let prone = Fx::from_milli(self.setup.rules.combat.body.prone_mm as i32);
        let c = &mut ents.combat[i];
        *c = Combat {
            stance: Stance::Prone,
            height: prone,
            fire_height: prone,
            corpse: corpse_ticks.clamp(1, u16::MAX as u32) as u16,
            seen_by: c.seen_by,
            ..Combat::default()
        };
        ents.unit[i] = Unit::default();
        ents.kit[i] = Kit::default();
        hash.mark_entity_chunk(id.chunk());
        if let Some(p) = self.players.get_mut(owner) {
            p.losses += 1;
        }
        let Some(sid) = ents.squad[i].take() else {
            return;
        };
        let Some(sq) = self.squads.get_mut(&sid) else {
            return;
        };
        if let Some(k) = sq.members.iter().position(|m| *m == id) {
            sq.members.remove(k);
            if k < sq.nav.slots.len() {
                sq.nav.slots.remove(k);
            }
        }
        if sq.focus == Some(id) {
            sq.focus = None;
        }
        // S4: a crew weapon (an AT team's launcher since S3) waits
        // `handover_ticks` for the next man (`pick_up`); other weapons stay
        // with the man who carried them.
        if let Some(p) = &mut sq.pzf
            && p.man == Some(id)
        {
            (p.man, p.target, p.aim) = (None, None, 0);
        }
        // S11b (review A-1): the rifle grenadier's aim or stow ends with
        // him (a save in this tick must load).
        if let Some(r) = &mut sq.rg
            && r.man == Some(id)
        {
            (r.man, r.target, r.aim, r.stow) = (None, None, 0, 0);
        }
        let ty = &self.setup.squad_types[sq.type_idx as usize];
        if ty.vehicle.is_none() {
            let w = &self.setup.weapons[carried as usize];
            if let Some(l) = &mut sq.launcher
                && l.carrier == Some(id)
            {
                l.carrier = None;
            }
            if w.crew && sq.drops.len() < crate::game::MAX_DROPS {
                sq.drops.push(crate::game::WeaponDrop {
                    weapon: carried,
                    at: fell_at,
                    ticks: w.handover_ticks.min(u16::MAX as u32) as u16,
                    mag: left_in,
                });
            }
        }
        for q in &mut self.path_queue.queues {
            q.retain(|r| *r != PathReq::Unit(id));
        }
        if sq.members.is_empty() {
            self.lose_squad_kg(sid);
            self.squads.remove(&sid);
            for q in &mut self.path_queue.queues {
                q.retain(|r| *r != PathReq::Squad(sid));
            }
        }
        // S10a: carried construction kg shrink with the squad (nothing to
        // do without any).
        if !self.fort_off {
            self.trim_carried();
        }
    }
}

// ---------------------------------------------------------------- status --

/// Read-only inputs of [`status`].
pub(crate) struct StatusCtx<'a> {
    pub kind: &'a [EntityKind],
    pub transform: &'a [Transform],
    pub velocity: &'a [Velocity],
    pub weapon: &'a [Weapon],
    pub squad: &'a [Option<SquadId>],
    pub unit: &'a [Unit],
    pub kit: &'a [Kit],
    pub game: &'a MatchState,
    pub terrain: &'a Heightfield,
}

/// Suppression decay, stance and timers for one entity chunk; corpses that
/// ran out go to `gone` (see the module docs).
pub(crate) fn status(
    chunk: usize,
    ctx: &StatusCtx,
    combat: &mut [Combat],
    gone: &mut Vec<SimId>,
) -> bool {
    let base = chunk * crate::ecs::CHUNK_SIZE;
    let setup = ctx.game.setup();
    let rules = &setup.rules.combat;
    let body = &rules.body;
    let aim_ticks = (setup.tick_hz / 2) as u16;
    let parapet: Fx = setup.rules.dig.parapet_height.into();
    let exposure: Fx = rules.fire_exposure.into();
    let mut changed = false;
    for (j, c) in combat.iter_mut().enumerate() {
        let i = base + j;
        match ctx.kind[i] {
            EntityKind::Corpse => {
                changed = true;
                c.corpse = c.corpse.saturating_sub(1);
                if c.corpse == 0 {
                    gone.push(SimId(i as u32));
                }
                continue;
            }
            EntityKind::Soldier => {}
            // S12: a downed man's timers run in `crate::wounds`.
            EntityKind::Vacant
            | EntityKind::Vehicle
            | EntityKind::Wreck
            | EntityKind::Abandoned
            | EntityKind::Downed => continue,
        }
        changed = true;
        let sq = ctx.squad[i].and_then(|s| ctx.game.squad(s));
        // S4: training scales the decay and the steadiness.
        let tr = sq.map(|s| training_of(setup, s));
        // S3b: a ducked man's meter decays slower (`duck_decay_pct`).
        let decay = if c.ducked {
            (rules.suppression_decay_milli as u64 * rules.suppression.duck_decay_pct as u64 / 100)
                as u32
        } else {
            rules.suppression_decay_milli
        };
        let decay = tr.map_or(decay, |t| pct_of(decay, t.suppression_decay_pct));
        let u = &ctx.unit[i];
        let retreating = if u.detached {
            u.order == UnitOrder::Retreat
        } else {
            sq.is_some_and(|s| s.order == Order::Retreat)
        };
        let weapon = sq.map(|_| &setup.weapons[ctx.kit[i].weapon as usize]);
        c.suppression = c.suppression.saturating_sub(decay);
        let points = c.suppression / 1000;
        // S3b: the squad type's steadiness scales the pin and duck
        // thresholds (the veterancy hook).
        let steadiness = sq.map_or(100, |s| {
            pct_of(
                setup.squad_types[s.type_idx as usize].steadiness_pct,
                training_of(setup, s).steadiness_pct,
            )
        });
        c.pinned = points >= steady(rules.pin_suppression, steadiness) && !retreating;
        let pos = ctx.transform[i].pos;
        let base_z = if ctx.terrain.has_detail_at(pos.xy()) {
            ctx.terrain.base_height_at(pos.xy())
        } else {
            pos.z
        };
        c.in_trench = base_z - pos.z >= Fx::from_milli(body.trench_depth_mm as i32);
        // S3b duck state (docs/slices/S3b-balance.md B1): in a trench, at
        // the duck threshold, until below the release threshold.
        let sup = &rules.suppression;
        let duck_at = if c.ducked {
            sup.duck_release_points
        } else {
            sup.duck_points
        };
        c.ducked = c.in_trench && !c.pinned && points >= steady(duck_at, steadiness);
        let suppressing = sq.is_some_and(|s| suppress_of(&s.order, u).is_some());
        if is_still(&ctx.velocity[i], body) {
            c.still = c.still.saturating_add(1);
        } else {
            c.still = 0;
        }
        c.since_shot = c.since_shot.saturating_add(1);
        // Winding up a throw counts as aiming: up on the fire step.
        let aiming = ((c.target.is_some() || suppressing)
            && (ctx.weapon[i].cooldown <= aim_ticks || c.since_shot < aim_ticks))
            || ctx.kit[i].windup.is_some();
        let crouched = points >= rules.crouch_suppression;
        let (stand, crouch, prone) = (
            Fx::from_milli(body.stand_mm as i32),
            Fx::from_milli(body.crouch_mm as i32),
            Fx::from_milli(body.prone_mm as i32),
        );
        let step = (base_z + parapet + exposure - pos.z).max(stand);
        // Suppression only ever lowers the ordered stance.
        (c.stance, c.height, c.fire_height) = if c.pinned {
            (Stance::Prone, prone, prone)
        } else if c.in_trench {
            if aiming && !c.ducked {
                (Stance::Stand, step, step)
            } else if u.stance == StanceOrder::Prone {
                (Stance::Prone, prone, step)
            } else {
                (Stance::Crouch, crouch, step)
            }
        } else {
            match u.stance {
                StanceOrder::Auto
                    if weapon.is_some_and(|w| {
                        w.class == WeaponClass::Mg && c.still as u32 >= w.setup_ticks
                    }) =>
                {
                    // A set-up MG fires lying down.
                    (Stance::Prone, prone, prone)
                }
                StanceOrder::Auto | StanceOrder::Stand if !crouched => {
                    (Stance::Stand, stand, stand)
                }
                StanceOrder::Auto | StanceOrder::Stand | StanceOrder::Crouch => {
                    (Stance::Crouch, crouch, crouch)
                }
                StanceOrder::Prone => (Stance::Prone, prone, prone),
            }
        };
    }
    changed
}

/// Fresh combat state for a soldier of squad type `ty` (match spawns).
pub(crate) fn spawn_combat(hp: u32, w: &WeaponSetup, body: &BodyTuning) -> Combat {
    let stand = Fx::from_milli(body.stand_mm as i32);
    Combat {
        hp: hp.clamp(1, u16::MAX as u32) as u16,
        height: stand,
        fire_height: stand,
        mag: w.magazine.clamp(1, u16::MAX as u32) as u16,
        since_shot: u16::MAX,
        ..Combat::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dig::stamp;
    use crate::game::Ctx;
    use crate::terrain::TerrainKind;
    use frontline_content::MatchSetup;

    fn v(x: i32, y: i32, z: i32) -> FxVec3 {
        FxVec3::new(Fx::from_milli(x), Fx::from_milli(y), Fx::from_milli(z))
    }

    #[test]
    fn segment_distance_matches_geometry() {
        // A horizontal segment passing 0.2 m beside a vertical capsule axis.
        let (s, d) = seg_seg(
            v(-5000, 200, 1000),
            v(10_000, 0, 0),
            v(0, 0, 0),
            v(0, 0, 1750),
        );
        assert!((s - Fx::HALF).abs() <= Fx::from_milli(2), "{s:?}");
        assert!((d - Fx::from_milli(40)).abs() <= Fx::from_milli(2), "{d:?}");
        // Passing over the top: closest to the capsule's top end.
        let (_, d) = seg_seg(
            v(-5000, 0, 2750),
            v(10_000, 0, 0),
            v(0, 0, 0),
            v(0, 0, 1750),
        );
        assert!((d - Fx::ONE).abs() <= Fx::from_milli(2), "{d:?}");
        // Parallel segments.
        let (_, d) = seg_seg(v(0, 300, 0), v(0, 0, 1000), v(0, 0, 0), v(0, 0, 1000));
        assert!((d - Fx::from_milli(90)).abs() <= Fx::from_milli(2), "{d:?}");
    }

    #[test]
    fn fractional_fire_interval_keeps_the_rate() {
        let mut w = setup().weapons[0].clone();
        // 1200 rpm at 30 Hz: 1.5 ticks, so 1, 2, 1, 2 ...
        w.interval_milli = 1500;
        let gaps: Vec<u32> = (1..=6).map(|n| interval_after(&w, n)).collect();
        assert_eq!(gaps, [1, 2, 1, 2, 1, 2]);
        // 500 rpm: 3.6 ticks; 1000 shots take exactly 3600 ticks.
        w.interval_milli = 3600;
        let total: u32 = (1..=1000).map(|n| interval_after(&w, n)).sum();
        assert_eq!(total, 3600);
        assert_eq!(interval_ceil(&w), 4);
    }

    #[test]
    fn earliest_hit_tie_rule() {
        let h = Fx::HALF;
        assert!(box_before(Some(h), Some(h)), "box before capsule on a tie");
        assert!(box_before(Some(h), Some(h + Fx::EPSILON)));
        assert!(!box_before(Some(h + Fx::EPSILON), Some(h)));
        assert!(box_before(Some(Fx::ONE), None));
        assert!(!box_before(None, Some(h)));
        // Terrain wins a tie: the re-test includes the segment's end, so a
        // box face lying exactly on the ground surface is reached as ground.
        let t = Heightfield::generate(TerrainKind::Flat, 256, 1);
        let z = t.height_at(FxVec2::from_int(100, 100));
        let a = FxVec3::new(Fx::from_int(90), Fx::from_int(100), z + Fx::ONE);
        let b = FxVec3::new(Fx::from_int(100), Fx::from_int(100), z);
        assert_eq!(t.segment_hit_fine(a, b), Some(b));
    }

    #[test]
    fn engage_rules_follow_the_order() {
        assert_eq!(engage(&Order::Retreat), Engage::Never);
        assert_eq!(engage(&Order::Idle), Engage::Always);
        let target = FxVec2::ZERO;
        assert_eq!(engage(&Order::Move { target }), Engage::Still);
        assert_eq!(engage(&Order::AttackMove { target }), Engage::Always);
    }

    fn setup() -> MatchSetup {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2");
        let defs = frontline_content::ModDefs::load(&root).unwrap();
        let req = frontline_content::MatchRequest::skirmish(
            1,
            "ww2:germany",
            "ww2:usa",
            frontline_content::Difficulty::Normal,
        );
        defs.resolve(&req).unwrap()
    }

    /// Flat ground at height 0 with one US rifle squad (player 1).
    struct Rig {
        game: MatchState,
        ents: Entities,
        terrain: Heightfield,
        hash: HashCache,
        spatial: SpatialHash,
        squad: SquadId,
    }

    /// A trench along x = 500 from y = 480 to 520, parapet to the east.
    const TRENCH_X: i32 = 500;

    fn rig(trench: bool) -> Rig {
        let setup = setup();
        let mut terrain = Heightfield::generate(TerrainKind::Flat, setup.map.size_m, 1);
        if trench {
            let line = [
                FxVec2::from_int(TRENCH_X, 480),
                FxVec2::from_int(TRENCH_X, 520),
            ];
            stamp(
                &mut terrain,
                &line,
                -1,
                Fx::ZERO,
                Fx::from_int(40),
                8,
                8,
                &setup.rules.dig,
                0,
            );
        }
        let ty = setup.squad_type_index("ww2:usa_rifle_squad").unwrap();
        let mut game = MatchState::new(setup);
        let mut ents = Entities::default();
        let mut hash = HashCache::default();
        let mut ctx = Ctx {
            ents: &mut ents,
            terrain: &terrain,
            hash: &mut hash,
            tick: 0,
        };
        let squad = game.spawn_squad(&mut ctx, PlayerId(1), ty);
        let spatial = SpatialHash::new(game.setup().map.size_m);
        Rig {
            game,
            ents,
            terrain,
            hash,
            spatial,
            squad,
        }
    }

    impl Rig {
        fn members(&self) -> Vec<SimId> {
            self.game.squad(self.squad).unwrap().members.clone()
        }

        /// Puts member `k` at `(x, y)` on the surface and runs the status
        /// stage once (stance, capsule heights).
        fn put(&mut self, k: usize, x: i32, y: i32) -> SimId {
            let id = self.members()[k];
            let xy = FxVec2::from_int(x, y);
            self.ents.transform[id.index()].pos = xy.extend(self.terrain.height_at(xy));
            self.ents.transform[id.index()].yaw = Fx::PI;
            id
        }

        fn status(&mut self) {
            let ctx = StatusCtx {
                kind: &self.ents.kind,
                transform: &self.ents.transform,
                velocity: &self.ents.velocity,
                weapon: &self.ents.weapon,
                squad: &self.ents.squad,
                unit: &self.ents.unit,
                kit: &self.ents.kit,
                game: &self.game,
                terrain: &self.terrain,
            };
            let mut gone = Vec::new();
            status(0, &ctx, &mut self.ents.combat, &mut gone);
            self.spatial.rebuild(&self.ents);
        }

        /// Flies one exactly aimed rifle round (player 0) from `from` at
        /// `aim`; returns what it struck.
        fn shoot(&self, from: FxVec3, aim: FxVec3) -> (ProjectileEnd, Vec<BallisticEvent>) {
            let setup = self.game.setup();
            let kar = setup
                .weapons
                .iter()
                .position(|w| w.id == "ww2:kar98k")
                .unwrap();
            let w = &setup.weapons[kar];
            let mut rng = RngStream::new(1, 0, SimId(0), Purpose::AIM);
            let vel = shot_velocity(from, aim, w.muzzle_speed.into(), Fx::ZERO, &mut rng);
            let mut slots = vec![Some(Projectile {
                pos: from,
                vel,
                owner: SimId(9999),
                drag: Fx::ZERO,
                ttl: round_ttl(w),
                weapon: kar as u16,
                player: 0,
                ammo: crate::projectile::NO_AMMO,
                origin: from,
                mission: 0,
            })];
            let ctx = FlightCtx {
                ents: &self.ents,
                spatial: &self.spatial,
                terrain: &self.terrain,
                weapons: &setup.weapons,
                near_miss: setup.rules.combat.near_miss.into(),
                body: &setup.rules.combat.body,
                vehicles: None,
                armour: self.game.armour(),
                ammo: &setup.ammo,
                statics: &[],
            };
            let (mut ended, mut events) = (Vec::new(), Vec::new());
            for _ in 0..60 {
                fly(0, &mut slots, &ctx, 2, &mut ended, &mut events, &mut 0);
                if let Some(&(_, end)) = ended.first() {
                    return (end, events);
                }
            }
            panic!("round never ended");
        }

        fn burst(&mut self, at: FxVec2) {
            let mortar = self
                .game
                .setup()
                .weapons
                .iter()
                .position(|w| w.he.is_some())
                .unwrap() as u16;
            let pos = at.extend(self.terrain.height_at(at));
            let mut stats = Stats::default();
            let mut ctx = DamageCtx {
                ents: &mut self.ents,
                terrain: &mut self.terrain,
                spatial: &self.spatial,
                hash: &mut self.hash,
                stats: &mut stats,
                seed: 1,
                tick: 1,
                fallen: Vec::new(),
                ff_dead: Vec::new(),
                ff_hit: Vec::new(),
            };
            let ev = [BallisticEvent {
                weapon: mortar,
                player: 0,
                ammo: NO_AMMO,
                slot: 0,
                shooter: SimId(0),
                blow: Blow::Burst(pos),
                mission: 0,
            }];
            self.game.apply_blows(
                &mut ctx,
                &ev,
                &mut Vec::new(),
                &mut Vec::new(),
                &mut Vec::new(),
            );
        }

        fn hp(&self, id: SimId) -> u16 {
            self.ents.combat[id.index()].hp
        }
    }

    #[test]
    fn trench_lip_stops_rounds_until_the_man_rises_to_fire() {
        let mut r = rig(true);
        let man = r.put(0, TRENCH_X, 500);
        r.status();
        let c = r.ents.combat[man.index()];
        assert!(c.in_trench && c.stance == Stance::Crouch, "{c:?}");
        let pos = r.ents.transform[man.index()].pos;
        let depth = Fx::from(r.game.setup().rules.dig.depth);
        assert_eq!(pos.z, -depth, "on the trench floor");
        // A rifleman 100 m east (the parapet side), lying on the ground.
        let muzzle = v((TRENCH_X + 100) * 1000, 500_000, 250);
        // Crouched: head below the ground (0.6 m at the 1.8 m depth). The lip takes the round.
        let (end, ev) = r.shoot(
            muzzle,
            aim_point(pos, &c, &r.game.setup().rules.combat.body.clone()),
        );
        assert_eq!(end, ProjectileEnd::GroundHit, "{ev:?}");
        assert!(!ev.iter().any(|e| matches!(e.blow, Blow::Hit(_))));
        // He rises to fire: 0.3 m above the parapet, and only that shows.
        r.ents.combat[man.index()].target = Some(SimId(9999));
        r.status();
        let c = r.ents.combat[man.index()];
        assert_eq!(c.stance, Stance::Stand);
        let top = pos.z + c.height;
        // Parapet 0.3 m, plus 0.3 m exposed.
        assert!(
            (top - Fx::from_milli(600)).abs() <= Fx::from_milli(1),
            "{top:?}"
        );
        let (end, ev) = r.shoot(
            muzzle,
            aim_point(pos, &c, &r.game.setup().rules.combat.body.clone()),
        );
        assert_eq!(end, ProjectileEnd::SoldierHit);
        assert_eq!(ev.last().unwrap().blow, Blow::Hit(man));
        // His chest is still behind the parapet.
        let chest = FxVec3::new(pos.x, pos.y, Fx::from_milli(100));
        let (end, _) = r.shoot(muzzle, chest);
        assert_eq!(end, ProjectileEnd::GroundHit);
        // The same man standing in the open is hit in the chest.
        let mut open = rig(false);
        let man = open.put(0, TRENCH_X, 500);
        open.status();
        let chest = FxVec3::new(pos.x, pos.y, Fx::from_milli(1100));
        let (end, ev) = open.shoot(muzzle, chest);
        assert_eq!(end, ProjectileEnd::SoldierHit);
        assert_eq!(ev.last().unwrap().blow, Blow::Hit(man));
    }

    #[test]
    fn near_misses_suppress_and_misses_pass_by() {
        let mut r = rig(false);
        let man = r.put(0, 500, 500);
        r.status();
        // 1 m to his side at head height: a near miss, no hit.
        let muzzle = v(600_000, 501_000, 1500);
        let (end, ev) = r.shoot(muzzle, v(400_000, 501_000, 1500));
        assert_ne!(end, ProjectileEnd::SoldierHit);
        assert_eq!(
            ev.iter().filter(|e| e.blow == Blow::NearMiss(man)).count(),
            1,
            "{ev:?}"
        );
        // 3 m away: nothing.
        let (_, ev) = r.shoot(v(600_000, 503_000, 1500), v(400_000, 503_000, 1500));
        assert!(ev.is_empty(), "{ev:?}");
        // Own rounds never touch him (no friendly fire).
        let mut slots = vec![Some(Projectile {
            pos: v(510_000, 500_000, 1000),
            vel: v(-760_000, 0, 0),
            owner: SimId(9999),
            drag: Fx::ZERO,
            ttl: 60,
            weapon: 0,
            player: 1,
            ammo: crate::projectile::NO_AMMO,
            origin: FxVec3::ZERO,
            mission: 0,
        })];
        let ctx = FlightCtx {
            ents: &r.ents,
            spatial: &r.spatial,
            terrain: &r.terrain,
            weapons: &r.game.setup().weapons,
            near_miss: Fx::from_int(2),
            body: &r.game.setup().rules.combat.body,
            vehicles: None,
            armour: r.game.armour(),
            ammo: &r.game.setup().ammo,
            statics: &[],
        };
        let (mut ended, mut events) = (Vec::new(), Vec::new());
        fly(0, &mut slots, &ctx, 2, &mut ended, &mut events, &mut 0);
        assert!(events.is_empty() && ended.is_empty());
    }

    #[test]
    fn suppression_crouches_then_pins() {
        let mut r = rig(false);
        let man = r.put(0, 500, 500);
        let rules = r.game.setup().rules.combat.clone();
        let decay = rules.suppression_decay_milli;
        let check = |r: &mut Rig, points: u32, stance: Stance, pinned: bool| {
            r.ents.combat[man.index()].suppression = points * 1000 + decay;
            r.status();
            let c = r.ents.combat[man.index()];
            assert_eq!((c.stance, c.pinned), (stance, pinned), "at {points}");
            assert_eq!(c.suppression, points * 1000, "decays per tick");
        };
        check(&mut r, rules.crouch_suppression - 1, Stance::Stand, false);
        check(&mut r, rules.crouch_suppression, Stance::Crouch, false);
        check(&mut r, rules.pin_suppression - 1, Stance::Crouch, false);
        check(&mut r, rules.pin_suppression, Stance::Prone, true);
        let c = r.ents.combat[man.index()];
        assert_eq!(c.height, Fx::from_milli(rules.body.prone_mm as i32));
        // A retreating squad runs even under fire.
        r.game.squads.get_mut(&r.squad).unwrap().order = Order::Retreat;
        check(&mut r, rules.pin_suppression, Stance::Crouch, false);
        // Pinned soldiers do not move.
        r.game.squads.get_mut(&r.squad).unwrap().order = Order::Idle;
        check(&mut r, 100, Stance::Prone, true);
        let before = r.ents.transform[man.index()].pos;
        let planned = vec![FxVec2::from_int(2, 0); r.ents.len()];
        crate::systems::integrate_soldiers(
            &r.ents.kind,
            &mut r.ents.transform,
            &mut r.ents.velocity,
            &r.ents.combat,
            &planned,
            &r.terrain,
            30,
            crate::systems::BENCH_STEEP,
        );
        assert_eq!(r.ents.transform[man.index()].pos, before);
        // Near misses add the weapon's suppression; full decay takes
        // 100 / decay-per-second seconds.
        r.ents.combat[man.index()].suppression = 0;
        let mut stats = Stats::default();
        let mut ctx = DamageCtx {
            ents: &mut r.ents,
            terrain: &mut r.terrain,
            spatial: &r.spatial,
            hash: &mut r.hash,
            stats: &mut stats,
            seed: 1,
            tick: 1,
            fallen: Vec::new(),
            ff_dead: Vec::new(),
            ff_hit: Vec::new(),
        };
        let mg = r
            .game
            .setup()
            .weapons
            .iter()
            .position(|w| w.class == WeaponClass::Mg)
            .unwrap();
        let ev = BallisticEvent {
            weapon: mg as u16,
            player: 0,
            ammo: NO_AMMO,
            slot: 0,
            shooter: SimId(0),
            blow: Blow::NearMiss(man),
            mission: 0,
        };
        r.game.apply_blows(
            &mut ctx,
            &[ev; 3],
            &mut Vec::new(),
            &mut Vec::new(),
            &mut Vec::new(),
        );
        let per = r.game.setup().weapons[mg].suppression;
        assert_eq!(r.ents.combat[man.index()].suppression, 3 * per * 1000);
        assert_eq!(stats.near_misses, 3);
    }

    impl Rig {
        /// Runs [`aim_and_fire`] once for the whole store; the requests.
        fn fire(&mut self, tick: u64) -> Vec<FireRequest> {
            let body = self.game.setup().rules.combat.body.clone();
            let (mut marks, mut mark_of) = (Vec::new(), Vec::new());
            build_marks(&self.ents, &mut marks, &mut mark_of, &body);
            let mut vmarks = Vec::new();
            build_vmarks(&self.ents, &mut vmarks);
            let ctx = FireCtx {
                kind: &self.ents.kind,
                owner: &self.ents.owner,
                transform: &self.ents.transform,
                velocity: &self.ents.velocity,
                squad: &self.ents.squad,
                unit: &self.ents.unit,
                marks: &marks,
                mark_of: &mark_of,
                vmarks: &vmarks,
                game: &self.game,
                terrain: &self.terrain,
                seed: 1,
                tick,
            };
            let mut out = Vec::new();
            aim_and_fire(
                0,
                &ctx,
                &mut self.ents.combat,
                &mut self.ents.weapon,
                &mut self.ents.kit,
                &mut out,
            );
            out
        }
    }

    /// S3b B1: a man in a trench ducks at the duck threshold (scaled by
    /// steadiness), stays down (no rising to the step, no fire, no throw)
    /// until below the release threshold, and never ducks in the open.
    #[test]
    fn men_in_a_trench_duck_under_fire_until_it_slackens() {
        let mut r = rig(true);
        let man = r.put(0, TRENCH_X, 500);
        let rules = r.game.setup().rules.combat.clone();
        let (duck, release) = (
            rules.suppression.duck_points,
            rules.suppression.duck_release_points,
        );
        assert!(release < duck && duck < rules.pin_suppression);
        let decay = rules.suppression_decay_milli;
        let set = |r: &mut Rig, points: u32| {
            r.ents.combat[man.index()].suppression = points * 1000 + decay;
            r.status();
            r.ents.combat[man.index()]
        };
        // He has a target: below the threshold he rises to fire.
        r.ents.combat[man.index()].target = Some(SimId(9999));
        let c = set(&mut r, duck - 1);
        assert!(
            c.in_trench && !c.ducked && c.stance == Stance::Stand,
            "{c:?}"
        );
        let c = set(&mut r, duck);
        assert!(c.ducked && c.stance == Stance::Crouch, "{c:?}");
        assert_eq!(c.target, Some(SimId(9999)), "keeps his target");
        // Hysteresis: still down between the two thresholds.
        let c = set(&mut r, release);
        assert!(c.ducked && c.stance == Stance::Crouch);
        // Ducked: no round, no throw.
        r.ents.weapon[man.index()].cooldown = 0;
        assert!(r.fire(5).iter().all(|f| f.shooter != man));
        assert!(
            r.ents.weapon[man.index()].cooldown > 0,
            "coming up costs aim time"
        );
        let frag = r.game.can_throw(
            &r.ents,
            man,
            frontline_content::ItemKind::Frag,
            FxVec2::from_int(TRENCH_X + 20, 500),
        );
        assert!(frag.is_none());
        let c = set(&mut r, release - 1);
        assert!(!c.ducked && c.stance == Stance::Stand, "{c:?}");
        // Pinned supersedes ducked.
        let c = set(&mut r, rules.pin_suppression);
        assert!(c.pinned && !c.ducked);
        // Steadier men duck later.
        let ty = r.game.squad(r.squad).unwrap().type_idx as usize;
        r.game.setup.squad_types[ty].steadiness_pct = 200;
        let c = set(&mut r, 0);
        assert!(!c.ducked);
        let c = set(&mut r, duck * 2 - 1);
        assert!(!c.ducked, "steadiness 200 %: ducks at twice the points");
        let c = set(&mut r, duck * 2);
        assert!(c.ducked);
        // In the open nobody ducks.
        let mut open = rig(false);
        let man = open.put(0, TRENCH_X, 500);
        open.ents.combat[man.index()].suppression = 60_000;
        open.status();
        let c = open.ents.combat[man.index()];
        assert!(!c.in_trench && !c.ducked && c.stance == Stance::Crouch);
    }

    /// S3b B2: men on a Suppress order fire at the line with no spotted
    /// target (there is no enemy at all here), within range and with a
    /// clear line, and rise to the fire step in a trench to do it.
    #[test]
    fn suppressing_men_fire_at_the_line_without_a_target() {
        let mut r = rig(false);
        for k in 0..r.members().len() {
            r.put(k, 600, 490 + k as i32 * 3);
        }
        r.status();
        let line = [FxVec2::from_int(500, 490), FxVec2::from_int(500, 510)];
        r.game.squads.get_mut(&r.squad).unwrap().order = Order::Suppress { line };
        let men = r.members();
        for m in &men {
            r.ents.weapon[m.index()].cooldown = 0;
            r.ents.combat[m.index()].still = 1000;
        }
        let shots = r.fire(7);
        assert_eq!(shots.len(), men.len(), "every man fires");
        for f in &shots {
            // Toward the line (west), at about the line's distance.
            assert!(f.vel.x < Fx::ZERO, "{f:?}");
            assert_eq!(f.player, 1);
        }
        assert!(
            men.iter()
                .all(|m| r.ents.combat[m.index()].target.is_none())
        );
        // Out of range: nothing.
        let far = [FxVec2::from_int(100, 490), FxVec2::from_int(100, 510)];
        r.game.squads.get_mut(&r.squad).unwrap().order = Order::Suppress { line: far };
        for m in &men {
            r.ents.weapon[m.index()].cooldown = 0;
        }
        assert!(r.fire(8).is_empty());
        // In a trench a suppressing man stands on the fire step.
        let mut t = rig(true);
        let man = t.put(0, TRENCH_X, 500);
        t.game.squads.get_mut(&t.squad).unwrap().order = Order::Suppress {
            line: [FxVec2::from_int(600, 500), FxVec2::from_int(600, 500)],
        };
        t.ents.weapon[man.index()].cooldown = 0;
        t.status();
        assert_eq!(t.ents.combat[man.index()].stance, Stance::Stand);
    }

    #[test]
    fn stance_orders_hold_unless_suppression_forces_lower() {
        let mut r = rig(false);
        let man = r.put(0, 500, 500);
        let rules = r.game.setup().rules.combat.clone();
        let decay = rules.suppression_decay_milli;
        let check = |r: &mut Rig, order: StanceOrder, points: u32, want: Stance| {
            r.ents.unit[man.index()].stance = order;
            r.ents.combat[man.index()].suppression = points * 1000 + decay;
            r.status();
            let c = r.ents.combat[man.index()];
            assert_eq!(c.stance, want, "{order:?} at {points}");
            let b = &rules.body;
            let h = match want {
                Stance::Stand => b.stand_mm,
                Stance::Crouch => b.crouch_mm,
                Stance::Prone => b.prone_mm,
            } as i32;
            assert_eq!(
                (c.height, c.fire_height),
                (Fx::from_milli(h), Fx::from_milli(h))
            );
        };
        let (crouch, pin) = (rules.crouch_suppression, rules.pin_suppression);
        use Stance as S;
        use StanceOrder as O;
        for (order, calm, shaken, pinned) in [
            (O::Auto, S::Stand, S::Crouch, S::Prone),
            (O::Stand, S::Stand, S::Crouch, S::Prone),
            (O::Crouch, S::Crouch, S::Crouch, S::Prone),
            (O::Prone, S::Prone, S::Prone, S::Prone),
        ] {
            check(&mut r, order, 0, calm);
            check(&mut r, order, crouch, shaken);
            check(&mut r, order, pin, pinned);
        }
        // An ordered stance beats the set-up MG's automatic prone.
        let mg = r
            .game
            .setup()
            .squad_types
            .iter()
            .position(|t| r.game.setup().weapons[t.weapon as usize].class == WeaponClass::Mg)
            .unwrap();
        r.game.squads.get_mut(&r.squad).unwrap().type_idx = mg as u16;
        // S4: the man carries the MG himself (`Kit::weapon`).
        r.ents.kit[man.index()].weapon = r.game.setup().squad_types[mg].weapon;
        r.ents.combat[man.index()].still = u16::MAX;
        check(&mut r, O::Auto, 0, S::Prone);
        check(&mut r, O::Crouch, 0, S::Crouch);
    }

    #[test]
    fn stance_orders_in_a_trench_still_crouch_and_rise_to_fire() {
        let mut r = rig(true);
        let man = r.put(0, TRENCH_X, 500);
        for order in [
            StanceOrder::Auto,
            StanceOrder::Stand,
            StanceOrder::Crouch,
            StanceOrder::Prone,
        ] {
            r.ents.unit[man.index()].stance = order;
            r.ents.combat[man.index()].target = None;
            r.status();
            let low = r.ents.combat[man.index()];
            let want = if order == StanceOrder::Prone {
                Stance::Prone
            } else {
                Stance::Crouch
            };
            assert!(low.in_trench && low.stance == want, "{order:?}: {low:?}");
            // Below the lip: near misses count half.
            assert!(covered(&low), "{order:?}");
            // Rises to the fire step to shoot, whatever the order.
            r.ents.combat[man.index()].target = Some(SimId(9999));
            r.status();
            let up = r.ents.combat[man.index()];
            assert_eq!(up.stance, Stance::Stand, "{order:?}");
            assert_eq!(up.height, low.fire_height, "{order:?}");
            assert_eq!(up.height, up.fire_height);
        }
    }

    #[test]
    fn trench_walls_stop_blast_damage_except_near_direct_hits() {
        // Men crouched on the floor; bursts 3 m to either side of the
        // trench line (inside the lethal radius) do no damage.
        let mut r = rig(true);
        let a = r.put(0, TRENCH_X, 495);
        let b = r.put(1, TRENCH_X, 505);
        r.status();
        assert!(r.ents.combat[a.index()].in_trench);
        r.burst(FxVec2::from_int(TRENCH_X + 3, 495));
        r.burst(FxVec2::from_int(TRENCH_X - 3, 505));
        assert_eq!((r.hp(a), r.hp(b)), (100, 100));
        // Suppressed though (halved behind earth).
        assert!(r.ents.combat[a.index()].suppression > 0);
        // A shell in the trench, 2 m along it from a: lethal.
        r.burst(FxVec2::from_int(TRENCH_X, 497));
        assert_eq!(r.ents.kind[a.index()], EntityKind::Corpse);
        assert_eq!(r.game.players[1].losses, 1);
        assert_eq!(r.game.players[0].kills, 1);
        assert!(!r.game.squad(r.squad).unwrap().members.contains(&a));
        // In the open the same 3 m burst kills.
        let mut open = rig(false);
        let a = open.put(0, TRENCH_X, 495);
        open.status();
        open.burst(FxVec2::from_int(TRENCH_X + 3, 495));
        assert_eq!(open.ents.kind[a.index()], EntityKind::Corpse);
        // Each burst leaves a crater.
        assert!(open.terrain.height_at(FxVec2::from_int(TRENCH_X + 3, 495)) < Fx::from_milli(-300));
    }

    #[test]
    fn last_man_down_deletes_the_squad() {
        let mut r = rig(false);
        let n = r.members().len();
        for k in 0..n {
            r.put(k, 500 + k as i32, 500);
        }
        r.status();
        // S3b: fragments, not a radius: men in a line shield each other,
        // so it takes a few bursts at points along the line.
        for k in 0..40 {
            if r.game.squad(r.squad).is_none() {
                break;
            }
            r.burst(FxVec2::from_int(500 + (k % n as i32), 501));
        }
        assert!(r.game.squad(r.squad).is_none());
        assert_eq!(r.game.players[1].losses, n as u32);
        assert!(r.ents.soldiers().next().is_none());
        // Corpses count down and are reported for removal.
        let corpse_ticks = r.game.setup().rules.combat.corpse_ticks;
        let mut gone = Vec::new();
        for _ in 0..corpse_ticks {
            let ctx = StatusCtx {
                kind: &r.ents.kind,
                transform: &r.ents.transform,
                velocity: &r.ents.velocity,
                weapon: &r.ents.weapon,
                squad: &r.ents.squad,
                unit: &r.ents.unit,
                kit: &r.ents.kit,
                game: &r.game,
                terrain: &r.terrain,
            };
            status(0, &ctx, &mut r.ents.combat, &mut gone);
        }
        assert_eq!(gone.len(), n);
    }
}
