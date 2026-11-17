//! S5b fire missions: modes, observers and the voice link, aim error and
//! ranging-in, the ranging memory, and the player's knowledge map (intel)
//! that map fire needs (docs/slices/S5b-mortars.md "Sim rules";
//! artillery-on-map.md §4.1).
//!
//! * **Modes.** A mission's mode is decided per round, when the round is
//!   fired: **Direct** when the mortar squad's own leader (not detached,
//!   within the voice link of the gunner) has a sight line to the target
//!   point, **Observed** when another linked observer of the player has one
//!   (a squad leader within `observe_leader_m` of the point, or an
//!   `observer` man within `observe_observer_m`; the mortar's own detached
//!   leader counts here, as a spotter), else **Map fire**. At most
//!   [`OBSERVE_RAYS`] rays per check, the own leader first, then ascending
//!   id; smoke blocks a ray as it blocks spotting.
//! * **Aim error.** The mission's `offset` is drawn once (length the mode's
//!   start error, bearing uniform, `Purpose::MISSION_AIM` keyed by squad id
//!   and order tick); every round lands at `target + offset` plus the
//!   weapon's precision spread. Each round an observer **sees** land shrinks
//!   the offset to `ranging_step_pct` of itself; map-fire rounds never range.
//! * **Phases.** Laying (`fire_direction_s`, or `direct_s` when Direct),
//!   then adjusting rounds one at a time (each waits for its burst plus
//!   `adjust_wait_s`) until the offset is at most `ffe_error_m` or
//!   `adjust_rounds_max` were fired, then fire for effect at the weapon's
//!   rate. Map fire goes straight to effect.
//! * **Knowledge rule.** An order is accepted only at a point the player
//!   knows: an own observer sees it now, a capture point covers it, an enemy
//!   is seen within `intel_snap_m`, or an intel cell (an enemy last seen
//!   there within `intel_keep_s`, or in a trench: the match) lies within
//!   `intel_snap_m`. Never blind into unknown ground.
//!
//! Legacy content (a rules def without `[fire_mission]`) runs every mission
//! as the old barrage: none of this applies (S5b-1 (A)).

use std::collections::BTreeMap;

use frontline_content::setup::member_trait;
use frontline_content::tuning::FireMissionTuning;
use serde::{Deserialize, Serialize};

use crate::ecs::{Entities, EntityKind, PlayerId, SimId};
use crate::fx::{Fx, FxVec2, FxVec3};
use crate::game::{MatchState, Order, Squad, SquadId};
use crate::terrain::Heightfield;

/// Most sight-line rays one mode check or burst observation casts.
pub const OBSERVE_RAYS: usize = 4;
/// Most rays the knowledge rule's "an own observer sees it" casts.
pub const KNOWN_RAYS: usize = 8;

/// Fire modes ([`Mission::mode`]).
pub mod mode {
    pub const DIRECT: u8 = 0;
    pub const OBSERVED: u8 = 1;
    pub const MAP: u8 = 2;
    pub const COUNT: usize = 3;
    pub const NAMES: [&str; COUNT] = ["direct", "observed", "map"];
}

/// Mission phases ([`Mission::phase`]).
pub mod phase {
    /// Before the first round (the fire command, laying the tube).
    pub const LAYING: u8 = 0;
    /// Single adjusting rounds, each waiting for its burst.
    pub const ADJUST: u8 = 1;
    /// Fire for effect at the weapon's rate.
    pub const EFFECT: u8 = 2;
    pub const NAMES: [&str; 3] = ["laying", "adjusting", "effect"];
}

/// [`IntelCell::flags`].
pub mod intel_flag {
    /// An enemy was seen there in a trench.
    pub const TRENCH: u8 = 1;
    /// An enemy vehicle was seen there.
    pub const VEHICLE: u8 = 2;
    pub const ALL: u8 = TRENCH | VEHICLE;
}

/// The state of one fire mission (saved, hashed, validated on load).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Mission {
    /// Never-reused mission number (`MatchState::next_mission`; its rounds
    /// carry it as `Projectile::mission`); 0 for legacy missions.
    pub id: u32,
    /// [`mode`] of the last round (at order time before the first).
    pub mode: u8,
    /// [`phase`].
    pub phase: u8,
    /// Aim offset from the target: where the group lands (m).
    pub offset: FxVec2,
    /// Length of the offset the mission started with (HUD ranging bar).
    pub err0: Fx,
    /// Tick the order was given (RNG key).
    pub order_tick: u64,
    /// No round before this tick (laying, adjust wait).
    pub ready_at: u64,
    /// An adjusting round is in the air; the next waits for its burst ...
    pub wait_burst: bool,
    /// ... or until this tick (a round that never bursts).
    pub burst_by: u64,
    /// Adjusting rounds fired.
    pub adjust: u8,
    /// Rounds an observer saw land.
    pub seen: u8,
    /// Rounds fired.
    pub fired: u16,
    /// Charge of the last round (index into the weapon's charges).
    pub charge: u8,
    /// Flight time of the last round (ticks).
    pub flight: u16,
    /// The observer of the last round (Direct: the own leader).
    pub observer: Option<SimId>,
    /// The mission started from a remembered ranging cell.
    pub ranged: bool,
    /// S11a2 K4: a gun mission aimed within `intel_snap_m` of one of the
    /// player's counter-battery markers (set by the sim at order time,
    /// never by a command; only on a gun squad's mission).
    pub cb: bool,
}

/// One remembered target area of a mortar squad (saved, hashed).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct RangingCell {
    pub target: FxVec2,
    pub offset: FxVec2,
    pub seen: u8,
    /// Tick it was last written.
    pub at: u64,
    /// The gunner's position when it was written.
    pub from: FxVec2,
}

/// One cell of a player's knowledge map (saved, hashed).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct IntelCell {
    /// Tick of the last sighting there.
    pub seen: u64,
    /// [`intel_flag`] bits.
    pub flags: u8,
}

/// Why a fire mission was refused (the HUD shows it; the AI never emits
/// one the same check would refuse).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum FireMissionRefusal {
    /// The squad has no mortar (or is not the player's).
    NotMortar = 0,
    /// Rounds outside 1..=50.
    Rounds = 1,
    /// The target is off the map.
    OffMap = 2,
    /// Inside the weapon's minimum range.
    InsideMinimum = 3,
    /// No charge solution from where the tube stands.
    CannotReach = 4,
    /// No observer sees it and nothing was seen there.
    UnknownGround = 5,
    /// A counted squad holding no bombs.
    NoBombs = 6,
    /// S11a1 G2: every solution's arc meets the ground.
    Masked = 7,
    /// S11a1 G1: the gun is not emplaced, or its gunner or loader slot is
    /// unmanned.
    NotReady = 8,
}

impl FireMissionRefusal {
    pub const COUNT: usize = 9;
    pub const NAMES: [&str; Self::COUNT] = [
        "not_mortar",
        "rounds",
        "off_map",
        "inside_minimum",
        "cannot_reach",
        "unknown_ground",
        "no_bombs",
        "masked",
        "not_ready",
    ];

    /// The HUD's reason line.
    pub fn text(self) -> &'static str {
        match self {
            Self::NotMortar => "No mortar in the squad",
            Self::Rounds => "Rounds must be 1 to 50",
            Self::OffMap => "Off the map",
            Self::InsideMinimum => "Too close: inside the minimum range",
            Self::CannotReach => "Cannot reach: too close for the highest elevation",
            Self::UnknownGround => "Unknown ground: no observer sees it and nothing was seen there",
            Self::NoBombs => "No bombs",
            Self::Masked => "Masked: the ground blocks every arc",
            Self::NotReady => "Not ready: emplace the gun and man it",
        }
    }
}

/// A refused fire mission (presentation only: moved to the snapshot like
/// the call-in results, never saved or hashed).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct FireMissionEvent {
    pub player: PlayerId,
    pub squad: SquadId,
    pub refusal: FireMissionRefusal,
}

/// S5b: what the HUD's fire-mission tool shows at a point
/// ([`MatchState::fire_preview`]; read-only, never in a tick).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct FirePreview {
    /// The refusal the order would get now, if any.
    pub refusal: Option<FireMissionRefusal>,
    /// [`mode`] the first round would be fired in now, and its observer.
    pub mode: u8,
    pub observer: Option<SimId>,
    /// The start aim error (the remembered one after ranging) (m).
    pub error: Fx,
    /// The expected miss: the aim error plus half the precision spread's
    /// extent at that range (m).
    pub ring: Fx,
    /// A remembered ranging cell would be used.
    pub ranged: bool,
    /// Beyond the weapon's range: the team walks up first.
    pub walk: bool,
    /// From the gunner (m).
    pub range: Fx,
}

/// S5b per-player fire-mission counters (saved, hashed; reports and HUD).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct FmCounters {
    /// Missions accepted, by their mode at order time.
    pub missions: [u32; mode::COUNT],
    /// Orders refused, by [`FireMissionRefusal`].
    pub refused: [u32; FireMissionRefusal::COUNT],
    /// Bombs fired, by mode.
    pub bombs: [u32; mode::COUNT],
    /// Sum of the planned miss (aim error plus precision spread) of those
    /// bombs, by mode (mm).
    pub miss_mm: [u64; mode::COUNT],
    /// Bursts of observed or direct mission rounds a linked observer saw
    /// land / did not see (map-fire bursts are not looked for).
    pub seen: u32,
    pub unseen: u32,
    /// Adjusting rounds fired.
    pub adjust: u32,
    /// Sum of the bombs' flight times (ticks).
    pub flight_ticks: u64,
    /// Bombs fired by a gunner standing in the own base, by mode (the base
    /// battery, artillery-on-map.md §4.10).
    pub base_bombs: [u32; mode::COUNT],
    /// Missions that became observed (map → observed) or lost their
    /// observer (→ map) after the first round.
    pub upgrades: u32,
    pub downgrades: u32,
    /// Missions that ended for lack of a charge solution.
    pub no_solution: u32,
}

/// S11a1 per-player counters of infantry-gun fire (saved, hashed; the
/// `s11a1` JSON block). An `inf_gun`'s missions and rounds count here only,
/// never in [`FmCounters`] or the S9 gun counters.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct S11Counters {
    /// Gun missions accepted, by their mode at order time.
    pub gun_missions: [u32; mode::COUNT],
    /// Gun mission rounds fired, by mode.
    pub gun_rounds: [u32; mode::COUNT],
    /// Sum of the planned miss of those rounds, by mode (mm).
    pub gun_miss_mm: [u64; mode::COUNT],
    /// Orders refused `Masked` / `NotReady`.
    pub masked: u32,
    pub not_ready: u32,
    /// Rounds whose observer was linked by radio only.
    pub radio_rounds: u32,
    /// Rounds fired on the low / high root.
    pub low_rounds: u32,
    pub high_rounds: u32,
    /// Gun missions ended by a seen enemy within `gun_break_m` (G6).
    pub gun_breaks: u32,
    /// Manpower paid for infantry-gun rounds (micro-MP).
    pub gun_shell_micro: u64,
    /// Men killed by infantry guns.
    pub gun_kills: u32,
    /// Infantry guns that arrived.
    pub guns_bought: u32,
    /// Direct (non-mission) rounds of infantry guns.
    pub gun_direct_rounds: u32,
    /// Gun mission bursts with an own man within `[ai.arty] danger_close_m`.
    pub near_own_bursts: u32,
}

/// S11a2 per-player counters of friendly fire and counter-battery (saved,
/// hashed; the `s11a2` JSON block of Build B). Index `[0]` mortar, `[1]`
/// infantry gun where a pair is kept.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct S11a2Counters {
    /// Own men killed by an own mortar / gun mission burst (F4).
    pub ff_kills: [u32; 2],
    /// Own men hurt and not killed by an own mission burst (per burst).
    pub ff_wounds: u32,
    /// Own mission bursts that hurt an own man.
    pub ff_bursts: u32,
    /// Mortar mission bursts with an own man within `[ai.arty]
    /// danger_close_m` (the gun twin is `S11Counters::near_own_bursts`).
    pub mortar_near_own_bursts: u32,
    /// Own gun rounds the enemy counted as evidence (K1).
    pub cb_heard: u32,
    /// Markers this player made (K2) ...
    pub cb_marks: u32,
    /// ... and the sum of their first errors (mm).
    pub cb_err_first_mm: u64,
    /// Spots this player forgot: decayed to none or pushed out (K1, K3).
    pub cb_dropped: u32,
    /// Gun missions aimed at an own marker (K4) ...
    pub cb_missions: u32,
    /// ... their rounds by the round's mode ...
    pub cb_rounds: [u32; mode::COUNT],
    /// ... their bursts, and those within `cb_near_m` of a living enemy
    /// infantry gun.
    pub cb_bursts: u32,
    pub cb_near_bursts: u32,
}

/// One burst of a mission round (Damage stage), for the observation pass.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) struct MissionBurst {
    /// The round's mission id as the projectile carries it (with
    /// [`CB_ROUND`] for a counter-fire round).
    pub mission: u32,
    pub pos: FxVec3,
    /// The firing player.
    pub player: u8,
}

/// S11a2 fix B-2: the high bit of a projectile's `mission` marks a round
/// of a counter-fire mission, so its burst counts (`cb_bursts`,
/// `cb_near_bursts`) after the mission ended too. Mission ids stay below
/// it (`next_mission` counts missions).
pub(crate) const CB_ROUND: u32 = 1 << 31;

/// One entry of the diagnostic burst log (`World::enable_burst_log`,
/// mortar-bench): not saved, not hashed, never read by the sim.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct BurstRecord {
    pub tick: u64,
    pub mission: u32,
    pub pos: FxVec3,
    /// Whether a linked observer saw it land.
    pub seen: bool,
    /// Its mission was still going (else nobody looked for it).
    pub live: bool,
    /// The mission's aim offset length after this burst (m).
    pub offset: Fx,
}

#[inline]
fn m(v: u32) -> Fx {
    Fx::from_int(v.min(i32::MAX as u32 / 65536) as i32)
}

#[inline]
fn within(a: FxVec2, b: FxVec2, r: Fx) -> bool {
    let rr = r.raw().max(0) as u64;
    (a - b).length_sq_raw() <= rr * rr
}

/// `v` scaled by `pct` percent.
pub(crate) fn pct_of_vec(v: FxVec2, pct: u32) -> FxVec2 {
    FxVec2::new(
        Fx::from_raw((v.x.raw() as i64 * pct as i64 / 100) as i32),
        Fx::from_raw((v.y.raw() as i64 * pct as i64 / 100) as i32),
    )
}

/// The mode's start aim error at range `d`.
pub(crate) fn start_error(fm: &FireMissionTuning, map: bool, d: Fx) -> Fx {
    let (pct, min) = if map {
        (fm.map_fire_error_pct, fm.map_fire_error_min_m)
    } else {
        (fm.first_round_error_pct, fm.first_round_error_min_m)
    };
    Fx::from_raw((d.raw() as i64 * pct as i64 / 100) as i32).max(m(min))
}

/// G4: the start error of a mission, gun keys for a gun (`gun_*`), the
/// mortar keys otherwise (same as `start_error`).
pub(crate) fn start_error_for(fm: &FireMissionTuning, map: bool, d: Fx, gun: bool) -> Fx {
    if !gun {
        return start_error(fm, map, d);
    }
    let (pct, min) = if map {
        (fm.gun_map_fire_error_pct, fm.gun_map_fire_error_min_m)
    } else {
        (fm.gun_first_round_error_pct, fm.gun_first_round_error_min_m)
    };
    Fx::from_raw((d.raw() as i64 * pct as i64 / 100) as i32).max(m(min))
}

/// The largest start error any mission can have on this map (load check).
pub(crate) fn max_error(fm: &FireMissionTuning, map_size: i32) -> Fx {
    let diag = Fx::from_int(map_size).mul_int(3).div_int(2);
    start_error(fm, true, diag)
        .max(start_error(fm, false, diag))
        .max(start_error_for(fm, true, diag, true))
        .max(start_error_for(fm, false, diag, true))
}

impl MatchState {
    /// S5b: the squad's leader (its observer): the first living man of a
    /// `leader` slot, else the man who leads the formation now. `None` for
    /// vehicle squads.
    pub fn squad_leader(&self, ents: &Entities, sq: &Squad) -> Option<SimId> {
        let ty = &self.setup.squad_types[sq.type_idx as usize];
        if ty.vehicle.is_some() || sq.members.is_empty() {
            return None;
        }
        sq.members
            .iter()
            .copied()
            .find(|m| {
                ty.members
                    .get(ents.kit[m.index()].slot as usize)
                    .is_some_and(|s| s.traits & member_trait::LEADER != 0)
            })
            .or_else(|| Some(sq.lead(ents)))
    }

    /// S5b: the man firing a squad's mortar: the first non-detached member
    /// carrying an HE weapon.
    pub fn mortar_gunner(&self, ents: &Entities, sq: &Squad) -> Option<SimId> {
        sq.members.iter().copied().find(|m| {
            !ents.unit[m.index()].detached
                && self.setup.weapons[ents.kit[m.index()].weapon as usize]
                    .he
                    .is_some()
        })
    }

    /// S11a1: where squad `sq`'s tube stands: its mortar gunner, or for a
    /// gun squad with an `[indirect]` main gun, the gun.
    pub(crate) fn tube_pos(&self, ents: &Entities, sq: &Squad) -> Option<FxVec2> {
        if self.gun_indirect_weapon(sq).is_some() {
            let g = *sq.members.first()?;
            return Some(ents.transform[g.index()].pos.xy());
        }
        self.mortar_gunner(ents, sq)
            .map(|g| ents.transform[g.index()].pos.xy())
    }

    /// The observers of squad `q` (`(man, observe range)`): its leader and
    /// its `observer` men.
    fn observers_of(
        &self,
        ents: &Entities,
        fm: &FireMissionTuning,
        q: &Squad,
        out: &mut Vec<(SimId, Fx)>,
    ) {
        let ty = &self.setup.squad_types[q.type_idx as usize];
        if ty.vehicle.is_some() {
            return;
        }
        let leader = self.squad_leader(ents, q);
        let any_obs = ty
            .members
            .iter()
            .any(|s| s.traits & member_trait::OBSERVER != 0);
        for &mm in &q.members {
            let i = mm.index();
            if ents.kind[i] != EntityKind::Soldier || ents.combat[i].hp == 0 {
                continue;
            }
            let obs = any_obs
                && ty
                    .members
                    .get(ents.kit[i].slot as usize)
                    .is_some_and(|s| s.traits & member_trait::OBSERVER != 0);
            let lead = Some(mm) == leader;
            let r = match (obs, lead) {
                (true, true) => m(fm.observe_observer_m.max(fm.observe_leader_m)),
                (true, false) => m(fm.observe_observer_m),
                (false, true) => m(fm.observe_leader_m),
                (false, false) => continue,
            };
            out.push((mm, r));
        }
    }

    /// The eye point of an observer (fire-step height in a trench).
    pub(crate) fn observer_eye(&self, ents: &Entities, o: SimId) -> FxVec3 {
        let i = o.index();
        let c = &ents.combat[i];
        let eye_h = if c.in_trench { c.fire_height } else { c.height };
        crate::spotting::head(
            ents.transform[i].pos,
            eye_h,
            Fx::from_milli(self.setup.rules.combat.body.eye_drop_mm as i32),
        )
    }

    /// S11a1: the firing unit of squad `id`: its squads (a gun squad and
    /// the crew squad serving it; a mortar squad alone) and the man whose
    /// voice gives Direct (the crew's leader; the mortar squad's leader).
    pub(crate) fn firing_unit(
        &self,
        ents: &Entities,
        id: SquadId,
    ) -> (Vec<SquadId>, Option<SimId>) {
        let Some(sq) = self.squads.get(&id) else {
            return (Vec::new(), None);
        };
        if self.gun_indirect_weapon(sq).is_none() {
            return (vec![id], self.squad_leader(ents, sq));
        }
        let crew = sq
            .members
            .first()
            .and_then(|g| self.vehicles.get(g))
            .and_then(|vs| vs.served.as_ref())
            .and_then(|sv| sv.crew)
            .filter(|q| self.squads.get(q).is_some_and(|c| c.owner == sq.owner));
        match crew {
            Some(q) => (vec![id, q], self.squad_leader(ents, &self.squads[&q])),
            None => (vec![id], None),
        }
    }

    /// S11a1 G5: the living men of squad `q` with the `radio` trait.
    fn radio_men(&self, ents: &Entities, q: &Squad) -> impl Iterator<Item = SimId> {
        let ty = &self.setup.squad_types[q.type_idx as usize];
        let any = ty
            .members
            .iter()
            .any(|s| s.traits & member_trait::RADIO != 0);
        q.members.iter().copied().filter(move |&mm| {
            let i = mm.index();
            any && ents.kind[i] == EntityKind::Soldier
                && ents.combat[i].hp > 0
                && ty
                    .members
                    .get(ents.kit[i].slot as usize)
                    .is_some_and(|s| s.traits & member_trait::RADIO != 0)
        })
    }

    /// Linked observers of squad `id` whose observe range reaches `point`:
    /// `(direct, man)`, the own non-detached leader first, then ascending
    /// id. Linked: within `observer_link_m` of the tube at `gunner` (voice),
    /// or (S11a1 G5, `radio_link` 1) a living `radio` man of the
    /// observer's squad within the link of him and one of the firing unit
    /// within the link of the tube.
    fn linked(
        &self,
        ents: &Entities,
        fm: &FireMissionTuning,
        id: SquadId,
        gunner: FxVec2,
        point: FxVec2,
    ) -> Vec<(bool, SimId)> {
        let Some(sq) = self.squads.get(&id) else {
            return Vec::new();
        };
        let (unit, own_leader) = self.firing_unit(ents, id);
        let link = m(fm.observer_link_m);
        let at = |o: SimId| ents.transform[o.index()].pos.xy();
        let unit_radio = fm.radio_link != 0
            && unit.iter().any(|q| {
                self.radio_men(ents, &self.squads[q])
                    .any(|r| within(at(r), gunner, link))
            });
        let mut obs = Vec::new();
        let mut out = Vec::new();
        for (&qid, q) in &self.squads {
            if q.owner != sq.owner {
                continue;
            }
            obs.clear();
            self.observers_of(ents, fm, q, &mut obs);
            for &(o, r) in &obs {
                let p = at(o);
                if !within(p, point, r) {
                    continue;
                }
                if !within(p, gunner, link)
                    && !(unit_radio && self.radio_men(ents, q).any(|x| within(at(x), p, link)))
                {
                    continue;
                }
                let direct =
                    unit.contains(&qid) && Some(o) == own_leader && !ents.unit[o.index()].detached;
                out.push((direct, o));
            }
        }
        out.sort_unstable_by_key(|&(d, o)| (!d, o));
        out
    }

    /// The first of up to [`OBSERVE_RAYS`] linked observers with a clear
    /// sight line to `to` (smoke blocks it).
    fn first_clear(
        &self,
        ents: &Entities,
        terrain: &Heightfield,
        cands: &[(bool, SimId)],
        to: FxVec3,
        tick: u64,
    ) -> Option<(bool, SimId)> {
        cands.iter().take(OBSERVE_RAYS).copied().find(|&(_, o)| {
            let eye = self.observer_eye(ents, o);
            terrain.line_of_sight(eye, to) && !self.smoke_blocks(eye, to, tick, (o, o))
        })
    }

    /// S5b: the mode a round of squad `id` (gunner at `gunner`) at `target`
    /// would be fired in now, and its observer. Read-only; at most
    /// [`OBSERVE_RAYS`] rays. Legacy rules: always Direct, no observer.
    pub fn mode_for(
        &self,
        ents: &Entities,
        terrain: &Heightfield,
        id: SquadId,
        gunner: FxVec2,
        target: FxVec2,
        tick: u64,
    ) -> (u8, Option<SimId>) {
        let Some(fm) = self.setup.rules.fire_mission.as_ref() else {
            return (mode::DIRECT, None);
        };
        let cands = self.linked(ents, fm, id, gunner, target);
        let to =
            target.extend(ground_point(terrain, target) + Fx::from_milli(fm.point_lift_mm as i32));
        match self.first_clear(ents, terrain, &cands, to, tick) {
            Some((true, o)) => (mode::DIRECT, Some(o)),
            Some((false, o)) => (mode::OBSERVED, Some(o)),
            None => (mode::MAP, None),
        }
    }

    /// S5b: the fire-mission tool's preview for squad `id` at `target`
    /// (the refusal, the mode, the expected miss). Read-only: the HUD calls
    /// it on the presenter's side of the sim thread, outside any tick.
    /// `None` when the squad does not exist.
    pub fn fire_preview(
        &self,
        ents: &Entities,
        terrain: &Heightfield,
        id: SquadId,
        target: FxVec2,
        rounds: u16,
        tick: u64,
    ) -> Option<FirePreview> {
        let sq = self.squads.get(&id)?;
        let refusal = self.mission_refusal(ents, terrain, id, target, rounds, tick);
        let ty = &self.setup.squad_types[sq.type_idx as usize];
        let w = &self.setup.weapons[ty.weapon as usize];
        let g = self
            .mortar_gunner(ents, sq)
            .unwrap_or_else(|| sq.lead(ents));
        let gp = ents.transform[g.index()].pos.xy();
        let range = (target - gp).length();
        let max = Fx::from(w.range_max);
        let walk = range > max;
        // Beyond range the team walks to 90 % of it and fires from there
        // (`order_fire_mission`): the error and spread are of that range.
        let fire_range = if walk {
            range.min(max.mul_int(9).div_int(10))
        } else {
            range
        };
        let spread = fire_range.mul(w.spread.into()).div_int(2);
        let Some(fm) = self.setup.rules.fire_mission.as_ref() else {
            return Some(FirePreview {
                refusal,
                mode: mode::DIRECT,
                observer: None,
                error: Fx::ZERO,
                ring: spread,
                ranged: false,
                walk,
                range,
            });
        };
        let (md, observer) = if refusal.is_some() {
            (mode::MAP, None)
        } else {
            self.mode_for(ents, terrain, id, gp, target, tick)
        };
        let mut error = start_error(fm, md == mode::MAP, fire_range);
        let mut ranged = false;
        if md != mode::MAP {
            let mut cells = sq.ranging.clone();
            prune_ranging(&mut cells, fm, tick, self.setup.tick_hz, gp);
            if let Some(c) = ranged_cell(&cells, fm, target).filter(|c| c.offset.length() < error) {
                error = c.offset.length();
                ranged = true;
            }
        }
        Some(FirePreview {
            refusal,
            mode: md,
            observer,
            error,
            ring: error + spread,
            ranged,
            walk,
            range,
        })
    }

    /// S5b: the lasting part of the knowledge rule: a capture point's area
    /// (grown by `intel_snap_m`) or an intel cell within `intel_snap_m`
    /// covers `pt`. No rays; what it accepts stays known for at least the
    /// cell's keep time, so the AI orders only such points (an enemy seen
    /// this instant may be gone when the order applies). Legacy rules:
    /// every point.
    pub fn known_by_record(&self, p: PlayerId, pt: FxVec2) -> bool {
        let Some(fm) = self.setup.rules.fire_mission.as_ref() else {
            return true;
        };
        let s = &self.setup;
        let snap = m(fm.intel_snap_m);
        // (b) a capture point's area, grown by the snap distance.
        let cap = Fx::from(s.rules.capture_radius) + snap;
        if s.map.points.iter().any(|c| within(c.pos.into(), pt, cap)) {
            return true;
        }
        // S11a2 K4: (d) one of the player's counter-battery markers
        // (it reveals nothing else).
        if self.near_own_marker(p, pt) {
            return true;
        }
        // (c) an intel cell within the snap distance.
        if let Some(intel) = self.players.get(p.0 as usize).map(|pl| &pl.intel)
            && !intel.is_empty()
        {
            let cell = m(fm.intel_cell_m);
            let cols = intel_cols(s.map.size_m, fm.intel_cell_m) as i32;
            let reach = (fm.intel_snap_m / fm.intel_cell_m.max(1)) as i32 + 1;
            let cx = pt.x.div(cell).floor_int();
            let cy = pt.y.div(cell).floor_int();
            for y in (cy - reach)..=(cy + reach) {
                for x in (cx - reach)..=(cx + reach) {
                    if x < 0 || y < 0 || x >= cols || y >= cols {
                        continue;
                    }
                    if intel.contains_key(&((y * cols + x) as u32))
                        && within(cell_centre(cell, x, y), pt, snap)
                    {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// S5b knowledge rule: whether player `p` knows point `pt` (module
    /// docs). Legacy rules: every point.
    pub fn known_point(
        &self,
        ents: &Entities,
        terrain: &Heightfield,
        p: PlayerId,
        pt: FxVec2,
        tick: u64,
    ) -> bool {
        let Some(fm) = self.setup.rules.fire_mission.as_ref() else {
            return true;
        };
        let snap = m(fm.intel_snap_m);
        // (b) and (c): a capture point or an intel cell.
        if self.known_by_record(p, pt) {
            return true;
        }
        // (d) an enemy seen now within the snap distance.
        let bit = 1u16 << p.0;
        for i in 0..ents.len() {
            let k = ents.kind[i];
            if !matches!(k, EntityKind::Soldier | EntityKind::Vehicle)
                || ents.owner[i] == p
                || ents.combat[i].seen_by & bit == 0
                || (k == EntityKind::Soldier && ents.combat[i].hp == 0)
            {
                continue;
            }
            if within(ents.transform[i].pos.xy(), pt, snap) {
                return true;
            }
        }
        // (a) an own observer sees it now (nearest first).
        let mut obs = Vec::new();
        let mut cands: Vec<(u64, SimId)> = Vec::new();
        for q in self.squads.values().filter(|q| q.owner == p) {
            obs.clear();
            self.observers_of(ents, fm, q, &mut obs);
            for &(o, r) in &obs {
                let at = ents.transform[o.index()].pos.xy();
                if within(at, pt, r) {
                    cands.push(((at - pt).length_sq_raw(), o));
                }
            }
        }
        cands.sort_unstable();
        let to = pt.extend(ground_point(terrain, pt) + Fx::from_milli(fm.point_lift_mm as i32));
        cands.iter().take(KNOWN_RAYS).any(|&(_, o)| {
            let eye = self.observer_eye(ents, o);
            terrain.line_of_sight(eye, to) && !self.smoke_blocks(eye, to, tick, (o, o))
        })
    }

    /// S5b: marks the cells of every enemy each player sees and drops
    /// expired cells (Status stage, every `intel_period_s`; players in
    /// order, entities ascending).
    pub(crate) fn update_intel(&mut self, ents: &Entities, tick: u64) {
        let Some(fm) = self.setup.rules.fire_mission.clone() else {
            return;
        };
        let hz = self.setup.tick_hz as u64;
        if !tick.is_multiple_of(fm.intel_period_s as u64 * hz) {
            return;
        }
        let cell = m(fm.intel_cell_m);
        let cols = intel_cols(self.setup.map.size_m, fm.intel_cell_m) as i32;
        let keep = fm.intel_keep_s as u64 * hz;
        let trench_keep = fm.intel_trench_keep_s as u64 * hz;
        for (p, pl) in self.players.iter_mut().enumerate() {
            let bit = 1u16 << p;
            for i in 0..ents.len() {
                let k = ents.kind[i];
                let soldier = k == EntityKind::Soldier;
                if !(soldier || k == EntityKind::Vehicle)
                    || ents.owner[i].0 as usize == p
                    || ents.combat[i].seen_by & bit == 0
                    || (soldier && ents.combat[i].hp == 0)
                {
                    continue;
                }
                let at = ents.transform[i].pos.xy();
                let x = at.x.div(cell).floor_int().clamp(0, cols - 1);
                let y = at.y.div(cell).floor_int().clamp(0, cols - 1);
                let c = pl.intel.entry((y * cols + x) as u32).or_default();
                c.seen = tick;
                if soldier && ents.combat[i].in_trench {
                    c.flags |= intel_flag::TRENCH;
                }
                if !soldier {
                    c.flags |= intel_flag::VEHICLE;
                }
            }
            pl.intel.retain(|_, c| {
                let age = tick.saturating_sub(c.seen);
                age <= keep
                    || (c.flags & intel_flag::TRENCH != 0
                        && (trench_keep == 0 || age <= trench_keep))
            });
        }
    }

    /// S11a2 K4: player `owner`'s counter-fire burst at `p` counts
    /// `cb_bursts`, and `cb_near_bursts` within `cb_near_m` of a living
    /// enemy infantry gun (a report; live or ended mission, fix B-2).
    fn count_cb_burst(&mut self, ents: &Entities, fm: &FireMissionTuning, owner: usize, p: FxVec2) {
        let r = m(fm.cb_near_m);
        let near = self.vehicles.iter().any(|(&v, vs)| {
            ents.kind.get(v.index()) == Some(&EntityKind::Vehicle)
                && ents.owner[v.index()].0 as usize != owner
                && self.setup.vehicles[vs.type_idx as usize].role
                    == frontline_content::VehicleRole::InfGun
                && within(ents.transform[v.index()].pos.xy(), p, r)
        });
        let Some(pl) = self.players.get_mut(owner) else {
            return;
        };
        let c = &mut pl.s11a2;
        c.cb_bursts = c.cb_bursts.saturating_add(1);
        c.cb_near_bursts = c.cb_near_bursts.saturating_add(near as u32);
    }

    /// S5b: the linked observers of the missions whose rounds burst this
    /// tick look for the burst (Damage stage, serial, burst order). A seen
    /// burst of an observed or direct mission shrinks its aim offset; the
    /// burst of an adjusting round releases the next.
    pub(crate) fn observe_bursts(
        &mut self,
        ents: &Entities,
        terrain: &Heightfield,
        bursts: &[MissionBurst],
        tick: u64,
    ) {
        let Some(fm) = self.setup.rules.fire_mission.clone() else {
            return;
        };
        let hz = self.setup.tick_hz as u64;
        for b in bursts {
            let mid = b.mission & !CB_ROUND;
            let Some((&id, _)) = self
                .squads
                .iter()
                .find(|(_, sq)| matches!(&sq.order, Order::FireMission { m, .. } if m.id == mid))
            else {
                // The mission ended (its last round is still in the air, or
                // the squad got another order): nobody adjusts on it. A
                // counter-fire round's burst still counts (fix B-2).
                if b.mission & CB_ROUND != 0 {
                    self.count_cb_burst(ents, &fm, b.player as usize, b.pos.xy());
                }
                if let Some(log) = self.burst_log.as_mut() {
                    log.push(BurstRecord {
                        tick,
                        mission: mid,
                        pos: b.pos,
                        seen: false,
                        live: false,
                        offset: Fx::ZERO,
                    });
                }
                continue;
            };
            let sq = &self.squads[&id];
            let Order::FireMission { target, m: mi, .. } = sq.order else {
                continue;
            };
            let gunner = self.tube_pos(ents, sq);
            let gun = self.gun_indirect_weapon(sq).is_some();
            let seen = mi.mode != mode::MAP
                && gunner.is_some_and(|g| {
                    let cands = self.linked(ents, &fm, id, g, b.pos.xy());
                    let to = FxVec3::new(
                        b.pos.x,
                        b.pos.y,
                        b.pos.z + Fx::from_milli(fm.burst_seen_lift_mm as i32),
                    );
                    self.first_clear(ents, terrain, &cands, to, tick).is_some()
                });
            let owner = sq.owner.0 as usize;
            let sq = self.squads.get_mut(&id).expect("found");
            let Order::FireMission { m: mi, .. } = &mut sq.order else {
                continue;
            };
            if seen {
                mi.offset = pct_of_vec(mi.offset, fm.ranging_step_pct);
                mi.seen = mi.seen.saturating_add(1);
                let (off, n) = (mi.offset, mi.seen);
                if let Some(g) = gunner {
                    remember(&mut sq.ranging, &fm, target, off, n, tick, g);
                }
            }
            let Order::FireMission { m: mi, .. } = &mut sq.order else {
                continue;
            };
            if mi.wait_burst {
                mi.wait_burst = false;
                mi.ready_at = tick + fm.adjust_wait_s as u64 * hz;
            }
            let off = mi.offset.length();
            let map = mi.mode == mode::MAP;
            let mcb = mi.cb;
            // Bursts within `[ai.arty] danger_close_m` of an own man (a
            // report; S11a1 for guns, S11a2 for mortars).
            let r = m(self.setup.rules.ai.arty.danger_close_m);
            let p = b.pos.xy();
            let near = self.squads.values().any(|q| {
                q.owner.0 as usize == owner
                    && q.members.iter().any(|&mm| {
                        let i = mm.index();
                        ents.kind[i] == EntityKind::Soldier
                            && ents.combat[i].hp > 0
                            && within(ents.transform[i].pos.xy(), p, r)
                    })
            });
            // S11a1: a gun's bursts count in `S11Counters` only.
            if gun {
                // S11a2 K4: a counter-fire burst, near a living enemy
                // infantry gun or not (a report).
                if mcb {
                    self.count_cb_burst(ents, &fm, owner, p);
                }
                let c = &mut self.players[owner].s11;
                c.near_own_bursts = c.near_own_bursts.saturating_add(near as u32);
            } else {
                let c = &mut self.players[owner].s11a2;
                c.mortar_near_own_bursts = c.mortar_near_own_bursts.saturating_add(near as u32);
                let c = &mut self.players[owner].fm;
                if seen {
                    c.seen += 1;
                } else if !map {
                    c.unseen += 1;
                }
            }
            if let Some(log) = self.burst_log.as_mut() {
                log.push(BurstRecord {
                    tick,
                    mission: mid,
                    pos: b.pos,
                    seen,
                    live: true,
                    offset: off,
                });
            }
        }
    }
}

/// The height of a ground point an observer adjusts on: the surface, or
/// the undug ground level over a trench or crater (an observer sees where
/// a trench is without seeing its floor).
pub(crate) fn ground_point(terrain: &Heightfield, p: FxVec2) -> Fx {
    terrain.height_at(p).max(terrain.base_height_at(p))
}

/// Cells per side of the intel grid.
pub fn intel_cols(size_m: i32, cell_m: u32) -> u32 {
    (size_m.max(1) as u32).div_ceil(cell_m.max(1))
}

fn cell_centre(cell: Fx, x: i32, y: i32) -> FxVec2 {
    FxVec2::new(
        cell.mul_int(x) + cell.div_int(2),
        cell.mul_int(y) + cell.div_int(2),
    )
}

/// Writes (or refreshes) the ranging cell of `target` in a squad's memory,
/// replacing the oldest when full.
pub(crate) fn remember(
    cells: &mut Vec<RangingCell>,
    fm: &FireMissionTuning,
    target: FxVec2,
    offset: FxVec2,
    seen: u8,
    tick: u64,
    from: FxVec2,
) {
    let cell = RangingCell {
        target,
        offset,
        seen,
        at: tick,
        from,
    };
    if let Some(c) = cells
        .iter_mut()
        .find(|c| within(c.target, target, m(fm.ranging_cell_m)))
    {
        *c = cell;
        return;
    }
    if cells.len() >= fm.ranging_max_cells as usize {
        // Oldest first; ties: the earlier entry.
        let (k, _) = cells
            .iter()
            .enumerate()
            .min_by_key(|(k, c)| (c.at, *k))
            .expect("full");
        cells.remove(k);
    }
    cells.push(cell);
}

/// Drops ranging cells that expired or that the tube (now at `from`) has
/// moved away from.
pub(crate) fn prune_ranging(
    cells: &mut Vec<RangingCell>,
    fm: &FireMissionTuning,
    tick: u64,
    hz: u32,
    from: FxVec2,
) {
    let expiry = fm.ranging_expiry_s as u64 * hz as u64;
    cells.retain(|c| {
        tick.saturating_sub(c.at) <= expiry && within(c.from, from, m(fm.ranging_keep_move_m))
    });
}

/// The remembered cell for `target`, if any.
pub(crate) fn ranged_cell<'a>(
    cells: &'a [RangingCell],
    fm: &FireMissionTuning,
    target: FxVec2,
) -> Option<&'a RangingCell> {
    cells
        .iter()
        .find(|c| within(c.target, target, m(fm.ranging_cell_m)))
}

/// Intel map validation (load).
pub(crate) fn intel_ok(
    intel: &BTreeMap<u32, IntelCell>,
    fm: Option<&FireMissionTuning>,
    size_m: i32,
    tick: u64,
) -> bool {
    let Some(fm) = fm else {
        return intel.is_empty();
    };
    let cols = intel_cols(size_m, fm.intel_cell_m) as u64;
    intel.iter().all(|(&k, c)| {
        (k as u64) < cols * cols && c.seen <= tick && c.flags & !intel_flag::ALL == 0
    })
}
