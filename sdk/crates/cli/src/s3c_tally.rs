//! S3c baseline counters for `skirmish` (report only: read from the world
//! after each tick, never fed back, so hashes do not change).
//!
//! * **Tank ammo and rearm** (S3b-42): returns of a tank into its own base
//!   after it had left it, how many came back low on ammo (every limited
//!   MG dry, or HE at `combat.suppress.tank_he_reserve` or below), and
//!   tank-seconds alive, with every limited MG dry, and with HE at the
//!   reserve or below.
//! * **Satchel hunts** (S3b-36): each stretch of an engineer squad on an
//!   AI `Hunt` task, and how it ended: `thrown` (a living man's satchel
//!   count fell), `wiped` (the squad is gone), `target_gone` (no enemy
//!   vehicle within [`HUNT_TARGET_M`] of the hunt spot), `target_moving`
//!   (the nearest one there moves faster than
//!   `ai.infantry.satchel_slow_mm_s`), `target_unseen` (it is there and
//!   slow but the player does not see it), `called_off` (there, slow and
//!   seen: the plan gave the squad another task); and the leader's
//!   distance to the spot when the hunt started.
//! * **Crew alive at a knock-out** (S3c bail-out baseline): living crew in
//!   the tick before each knock-out, by cause; for a crew knock-out at
//!   most `vehicles.damage.crew_ko_below` − 1. Exact for fires (they burn
//!   for seconds first); an upper bound for rack detonations (crew the
//!   same hit killed count as alive).
//! * **Where tanks die** (S3b-40): each own knock-out's position, and
//!   for knock-outs by an enemy main gun the nearest enemy tank of the
//!   killer that fired in the last [`SHOOTER_WINDOW_S`] seconds (its
//!   muzzle when it fired), in metres.
//! * **Bailed crews** (S3c T6): abandonments with living crew inside,
//!   those where at least one man got out, and the men who got out (each
//!   a new member of a crew squad) still alive [`CREW_ALIVE_S`] seconds
//!   later.
//! * **Capture timeline by base side** (S3b-40): per point, when each
//!   player first owned it, how long it held it, and the points owned at
//!   5, 10, 15 and 20 minutes.

use std::collections::BTreeMap;

use frontline_content::vehicle::AmmoMechanism;
use frontline_content::{ItemKind, MatchSetup, SquadRole};
use frontline_sim::{ArmourEventKind, PlayerId, SimId, SquadId, TaskKind, World};
use serde::Serialize;

/// A hunt's target is the nearest enemy vehicle within this many metres of
/// the hunt spot (the spot is `satchel_stand_m` beside its hull).
pub const HUNT_TARGET_M: i32 = 20;
/// A knock-out's shooter fired within this many seconds before it.
pub const SHOOTER_WINDOW_S: u64 = 3;
/// Minutes at which the points each player owns are recorded.
pub const POINT_MARKS_MIN: [u32; 4] = [5, 10, 15, 20];
/// A bailed-out crewman counts as surviving if alive this long after he
/// got out (S3c T6).
pub const CREW_ALIVE_S: u64 = 60;

/// One player's S3c baseline numbers.
#[derive(Clone, Debug, Default, Serialize)]
pub struct S3cPlayer {
    /// Own tanks driving back into the own base after leaving it, and how
    /// many of them came in low on ammo.
    pub base_returns: u32,
    pub base_returns_low_ammo: u32,
    /// Tank-seconds alive (not wrecks), with every limited MG dry, with HE
    /// at the reserve or below.
    pub tank_alive_s: u32,
    pub tank_mg_dry_s: u32,
    pub tank_he_low_s: u32,
    /// Tank-seconds with the limited MGs' stowage, summed, under a quarter.
    pub tank_mg_low_s: u32,
    /// Satchel hunts seen (stretches on a `Hunt` task of an engineer
    /// squad) and how they ended (`thrown`, `wiped`, `target_gone`,
    /// `target_moving`, `target_unseen`, `called_off`, `running` at the end
    /// of the match; module docs).
    pub satchel_hunts_seen: u32,
    pub satchel_hunt_ends: BTreeMap<String, u32>,
    /// Sum of the leader's distance to the hunt spot at the start (m).
    pub satchel_hunt_start_m: u32,
    /// Own knock-outs by cause, and the living crew in them the tick before
    /// (summed), and knock-outs with at least one living crewman.
    pub ko_by_cause: BTreeMap<String, u32>,
    pub ko_crew_alive: BTreeMap<String, u32>,
    pub ko_with_survivors: u32,
    /// Own knock-outs: `[x, y, z]` of the wreck, and for a main-gun
    /// knock-out with a shooter found, `[x, y, z]` of its muzzle (else
    /// all zero), metres.
    pub ko_spots: Vec<[i32; 6]>,
    /// Per capture point: seconds into the match this player first owned
    /// it (`None`: never), and seconds it owned it.
    pub first_capture_s: Vec<Option<u32>>,
    pub point_hold_s: Vec<u32>,
    /// Points owned at [`POINT_MARKS_MIN`] (`None`: the match ended first).
    pub points_at: Vec<Option<u8>>,
    /// S3c: own abandonments with living crew inside, those where at least
    /// one man got out; crewmen out (tracked to the end or 60 s) and alive
    /// 60 s after getting out.
    pub bail_with_crew: u32,
    pub bail_some_out: u32,
    pub crew_out_tracked: u32,
    pub crew_alive_60s: u32,
}

#[derive(Clone, Copy)]
struct Hunt {
    owner: u8,
    start_tick: u64,
    target: frontline_sim::FxVec2,
}

/// Per-tick bookkeeping behind [`S3cPlayer`].
pub struct S3cTally {
    pub players: Vec<S3cPlayer>,
    hz: u64,
    /// Tanks seen outside their base at least once.
    left_base: BTreeMap<SimId, bool>,
    /// Living crew per vehicle at the last look.
    crew: BTreeMap<SimId, u32>,
    /// Engineer squads on a hunt.
    hunts: BTreeMap<SquadId, Hunt>,
    /// Satchels each living man carried at the last look.
    satchels: BTreeMap<SimId, u8>,
    /// Owner of each point at the last look.
    owners: Vec<Option<u8>>,
    /// Last main-gun shot per vehicle: (tick, owner, muzzle).
    fired: BTreeMap<u32, (u64, u8, [f32; 3])>,
    /// S3c: abandoned vehicles: (last owner, men got out so far).
    bails: BTreeMap<u32, (u8, u32)>,
    /// S3c: crewmen out: (owner, tick out, checked).
    crewmen: BTreeMap<SimId, (u8, u64, bool)>,
}

impl S3cTally {
    pub fn new(setup: &MatchSetup) -> S3cTally {
        let n = setup.players.len();
        let np = setup.map.points.len();
        S3cTally {
            players: (0..n)
                .map(|_| S3cPlayer {
                    first_capture_s: vec![None; np],
                    point_hold_s: vec![0; np],
                    points_at: vec![None; POINT_MARKS_MIN.len()],
                    ..S3cPlayer::default()
                })
                .collect(),
            hz: setup.tick_hz.max(1) as u64,
            left_base: BTreeMap::new(),
            crew: BTreeMap::new(),
            hunts: BTreeMap::new(),
            satchels: BTreeMap::new(),
            owners: vec![None; np],
            fired: BTreeMap::new(),
            bails: BTreeMap::new(),
            crewmen: BTreeMap::new(),
        }
    }

    /// S3c: the bail-out events and crew squads of this tick.
    fn observe_crews(&mut self, w: &World) {
        let g = w.game().expect("match world");
        let s = g.setup();
        let ents = w.entities();
        let tick = w.tick();
        for e in &w.snapshot().armour_events {
            if e.kind == ArmourEventKind::Abandoned as u8 {
                let inside = w
                    .snapshot()
                    .vehicles
                    .iter()
                    .find(|v| v.id == e.vehicle)
                    .map_or(0, |v| v.inside);
                if inside > 0 {
                    if let Some(p) = self.players.get_mut(e.owner as usize) {
                        p.bail_with_crew += 1;
                    }
                    self.bails.insert(e.vehicle, (e.owner, 0));
                }
            } else if e.kind == ArmourEventKind::CrewOut as u8
                && let Some((owner, n)) = self.bails.get_mut(&e.vehicle)
            {
                *n += 1;
                if *n == 1
                    && let Some(p) = self.players.get_mut(*owner as usize)
                {
                    p.bail_some_out += 1;
                }
            }
        }
        let is_crew = |m: SimId| {
            ents.kind().get(m.index()) == Some(&frontline_sim::EntityKind::Soldier)
                && ents.squad()[m.index()]
                    .and_then(|q| g.squad(q))
                    .is_some_and(|q| s.squad_types[q.type_idx as usize].role == SquadRole::Crew)
        };
        for sq in g.squads().values() {
            if s.squad_types[sq.type_idx as usize].role != SquadRole::Crew {
                continue;
            }
            for &m in &sq.members {
                self.crewmen.entry(m).or_insert((sq.owner.0, tick, false));
            }
        }
        // A man is decided once: alive after the minute in his crew squad,
        // dead if he is a corpse first, alive if he left the squad another
        // way first (dismissed in base, or boarded a vehicle). Entries of
        // men no longer in a crew squad are dropped once decided, so a
        // reused id counts afresh.
        let due = CREW_ALIVE_S * self.hz;
        let mut gone: Vec<SimId> = Vec::new();
        for (&m, (owner, t, checked)) in self.crewmen.iter_mut() {
            let crew = is_crew(m);
            if !*checked {
                let verdict = if crew {
                    (tick >= *t + due).then_some(true)
                } else {
                    Some(ents.kind().get(m.index()) != Some(&frontline_sim::EntityKind::Corpse))
                };
                if let Some(alive) = verdict {
                    *checked = true;
                    if let Some(p) = self.players.get_mut(*owner as usize) {
                        p.crew_out_tracked += 1;
                        p.crew_alive_60s += alive as u32;
                    }
                }
            }
            if *checked && !crew {
                gone.push(m);
            }
        }
        for m in gone {
            self.crewmen.remove(&m);
        }
    }

    /// Reads the world after a tick.
    pub fn observe(&mut self, w: &World) {
        self.observe_crews(w);
        let g = w.game().expect("match world");
        let s = g.setup();
        let ents = w.entities();
        let tick = w.tick();
        let whole_second = tick.is_multiple_of(self.hz);
        let snap = w.snapshot();

        // Knock-outs: the crew alive at the last look, where, by whom.
        for e in &snap.armour_events {
            if e.kind == ArmourEventKind::Fired as u8 {
                self.fired.insert(e.vehicle, (tick, e.owner, e.pos));
            }
        }
        for e in &snap.armour_events {
            if e.kind != ArmourEventKind::KnockedOut as u8 {
                continue;
            }
            let window = SHOOTER_WINDOW_S * self.hz;
            let d2 = |q: &[f32; 3]| (q[0] - e.pos[0]).powi(2) + (q[1] - e.pos[1]).powi(2);
            let shooter = self
                .fired
                .values()
                .filter(|(t, o, _)| *o == e.attacker && tick.saturating_sub(*t) <= window)
                .map(|(_, _, q)| *q)
                .min_by(|a, b| d2(a).total_cmp(&d2(b)));
            let r = |v: f32| v.round() as i32;
            let spot = match shooter {
                Some(q) => [
                    r(e.pos[0]),
                    r(e.pos[1]),
                    r(e.pos[2]),
                    r(q[0]),
                    r(q[1]),
                    r(q[2]),
                ],
                None => [r(e.pos[0]), r(e.pos[1]), r(e.pos[2]), 0, 0, 0],
            };
            let cause = match e.component {
                0 => "ko_crew",
                1 => "ko_ammo_rack",
                3 => "ko_ammo_fire",
                _ => "ko_fire",
            };
            let mut alive = self.crew.remove(&SimId(e.vehicle)).unwrap_or(0);
            if e.component == 0 {
                // S3c: a crew knock-out is every seat empty (vehicles that
                // cannot bail out).
                alive = 0;
            }
            self.fired.remove(&e.vehicle);
            if let Some(p) = self.players.get_mut(e.owner as usize) {
                p.ko_spots.push(spot);
                *p.ko_by_cause.entry(cause.into()).or_default() += 1;
                *p.ko_crew_alive.entry(cause.into()).or_default() += alive;
                p.ko_with_survivors += u32::from(alive > 0);
            }
        }

        // Tanks: crew, base returns, ammo state.
        let reserve = s.rules.combat.suppress.tank_he_reserve;
        for (&id, vs) in g.vehicles() {
            if vs.wreck_tick.is_some() {
                self.crew.remove(&id);
                continue;
            }
            let owner = ents.owner()[id.index()].0 as usize;
            let v = &s.vehicles[vs.type_idx as usize];
            self.crew
                .insert(id, vs.seats.iter().filter(|st| st.hp > 0).count() as u32);
            let limited: Vec<bool> = v
                .mounts
                .iter()
                .skip(1)
                .zip(&vs.mgs)
                .filter_map(|(m, st)| m.rounds.map(|n| st.spent >= n))
                .collect();
            let mg_dry = !limited.is_empty() && limited.iter().all(|&d| d);
            let (left, full) = v
                .mounts
                .iter()
                .skip(1)
                .zip(&vs.mgs)
                .filter_map(|(m, st)| m.rounds.map(|n| (n.saturating_sub(st.spent), n)))
                .fold((0u64, 0u64), |a, b| (a.0 + b.0 as u64, a.1 + b.1 as u64));
            let mg_low = full > 0 && left * 4 < full;
            let he_idx = v
                .main_gun()
                .ammo
                .iter()
                .position(|a| s.ammo[a.ammo as usize].mechanism == AmmoMechanism::He);
            let he_low = he_idx
                .and_then(|i| vs.ammo.get(i))
                .is_some_and(|&n| n as u32 <= reserve);
            let Some(p) = self.players.get_mut(owner) else {
                continue;
            };
            let in_base = vs.base_ticks > 0;
            let left = self.left_base.entry(id).or_insert(false);
            if !in_base {
                *left = true;
            } else if vs.base_ticks == 1 && *left {
                p.base_returns += 1;
                p.base_returns_low_ammo += u32::from(mg_dry || he_low);
            }
            if whole_second {
                p.tank_alive_s += 1;
                p.tank_mg_dry_s += u32::from(mg_dry);
                p.tank_mg_low_s += u32::from(mg_low);
                p.tank_he_low_s += u32::from(he_low);
            }
        }

        // Satchel hunts.
        let slow = frontline_sim::Fx::from_milli(s.rules.ai.infantry.satchel_slow_mm_s as i32);
        let near = frontline_sim::Fx::from_int(HUNT_TARGET_M).raw() as u64;
        let near = near * near;
        // Why a hunt toward `at` by `owner` could not go on.
        let target_state = |at: frontline_sim::FxVec2, owner: u8| -> &'static str {
            let v = g
                .vehicles()
                .iter()
                .filter(|(vid, vs)| {
                    vs.wreck_tick.is_none()
                        && ents.owner()[vid.index()].0 != owner
                        && (ents.transform()[vid.index()].pos.xy() - at).length_sq_raw() <= near
                })
                .min_by_key(|(vid, _)| {
                    (
                        (ents.transform()[vid.index()].pos.xy() - at).length_sq_raw(),
                        vid.0,
                    )
                });
            match v {
                None => "target_gone",
                Some((_, vs)) if vs.avg_vel.length() > slow => "target_moving",
                Some((vid, _)) if ents.combat()[vid.index()].seen_by & (1 << owner) == 0 => {
                    "target_unseen"
                }
                Some(_) => "called_off",
            }
        };
        let mut on_hunt: Vec<SquadId> = Vec::new();
        for p in 0..s.players.len() {
            let Some(ai) = g.ai(PlayerId(p as u8)) else {
                continue;
            };
            for (&sid, task) in &ai.tasks {
                if task.kind != TaskKind::Hunt {
                    continue;
                }
                let Some(sq) = g.squad(sid) else { continue };
                if s.squad_types[sq.type_idx as usize].role != SquadRole::Engineer {
                    continue;
                }
                on_hunt.push(sid);
                if let Some(h) = self.hunts.get_mut(&sid) {
                    h.target = task.target;
                    continue;
                }
                let lead = ents.transform()[sq.lead(ents).index()].pos.xy();
                let pl = &mut self.players[p];
                pl.satchel_hunts_seen += 1;
                pl.satchel_hunt_start_m += (lead - task.target).length().floor_int().max(0) as u32;
                self.hunts.insert(
                    sid,
                    Hunt {
                        owner: p as u8,
                        start_tick: tick,
                        target: task.target,
                    },
                );
            }
        }
        // Throws: a living man whose satchel count fell.
        let mut threw: Vec<SquadId> = Vec::new();
        let mut now: BTreeMap<SimId, u8> = BTreeMap::new();
        for id in ents.soldiers() {
            let n = ents.kit()[id.index()].items[ItemKind::Satchel.index()];
            if let Some(&was) = self.satchels.get(&id)
                && n < was
                && let Some(sq) = ents.squad()[id.index()]
            {
                threw.push(sq);
            }
            if n > 0 {
                now.insert(id, n);
            }
        }
        self.satchels = now;
        let ended: Vec<SquadId> = self
            .hunts
            .keys()
            .filter(|id| !on_hunt.contains(id) || threw.contains(id))
            .copied()
            .collect();
        for sid in ended {
            let h = self.hunts.remove(&sid).expect("listed");
            let why = if threw.contains(&sid) {
                "thrown"
            } else if g.squad(sid).is_none() {
                "wiped"
            } else {
                target_state(h.target, h.owner)
            };
            let _ = h.start_tick;
            *self.players[h.owner as usize]
                .satchel_hunt_ends
                .entry(why.into())
                .or_default() += 1;
        }

        // Points.
        let secs = tick / self.hz;
        for (i, c) in g.points().iter().enumerate() {
            let o = c.owner.map(|o| o.0);
            if let Some(o) = o
                && let Some(p) = self.players.get_mut(o as usize)
            {
                if whole_second {
                    p.point_hold_s[i] += 1;
                }
                if p.first_capture_s[i].is_none() {
                    p.first_capture_s[i] = Some(secs as u32);
                }
            }
            self.owners[i] = o;
        }
        if whole_second {
            for (k, m) in POINT_MARKS_MIN.iter().enumerate() {
                if secs == *m as u64 * 60 {
                    for (p, pl) in self.players.iter_mut().enumerate() {
                        pl.points_at[k] =
                            Some(self.owners.iter().filter(|o| **o == Some(p as u8)).count() as u8);
                    }
                }
            }
        }
    }

    /// Closes hunts still running at the end of the match.
    pub fn finish(&mut self) {
        for h in std::mem::take(&mut self.hunts).into_values() {
            *self.players[h.owner as usize]
                .satchel_hunt_ends
                .entry("running".into())
                .or_default() += 1;
        }
    }
}

/// One base side's capture numbers over a `--seeds` run.
#[derive(Clone, Debug, Default, Serialize)]
pub struct BaseSide {
    pub base: u8,
    pub matches: u32,
    pub wins: u32,
    /// Per point: matches in which this side owned it first (before the
    /// other side ever did), the mean second of its first capture (over
    /// matches where it took the point), and its mean share of match time
    /// holding it (%).
    pub first_owner: Vec<u32>,
    pub first_capture_mean_s: Vec<Option<f64>>,
    pub hold_share_pct: Vec<f64>,
    /// Mean points owned at [`POINT_MARKS_MIN`] (matches still running).
    pub points_at_mean: Vec<Option<f64>>,
}

/// S3c lines of a `--seeds` run, by faction and by base side.
#[derive(Clone, Debug, Default, Serialize)]
pub struct S3cSummary {
    /// By faction: base returns (low ammo), tank-time shares with MGs dry
    /// and HE low (%), satchel hunts and their ends, mean start distance.
    pub factions: Vec<S3cFaction>,
    pub sides: Vec<BaseSide>,
    pub point_names: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct S3cFaction {
    pub faction: String,
    pub base_returns: u32,
    pub base_returns_low_ammo: u32,
    pub tank_alive_s: u32,
    pub mg_dry_pct: f64,
    pub mg_low_pct: f64,
    pub he_low_pct: f64,
    pub satchel_hunts: u32,
    pub satchel_hunt_ends: BTreeMap<String, u32>,
    pub satchel_hunt_start_mean_m: f64,
    pub knockouts: u32,
    pub ko_with_survivors: u32,
    /// S3c T6 (see [`S3cPlayer`]).
    pub bail_with_crew: u32,
    pub bail_some_out: u32,
    pub crew_out_tracked: u32,
    pub crew_alive_60s: u32,
    /// Mean living crew at a knock-out, by cause.
    pub ko_crew_alive_mean: BTreeMap<String, f64>,
}

/// One match for [`summarise`]: its players' factions, bases and S3c
/// numbers, the winner, and its length in seconds.
pub struct MatchView<'a> {
    pub factions: Vec<&'a str>,
    pub bases: Vec<u8>,
    pub s3c: Vec<&'a S3cPlayer>,
    pub winner: Option<u8>,
    pub seconds: f64,
}

pub fn summarise(matches: &[MatchView], point_names: Vec<String>) -> S3cSummary {
    let np = point_names.len();
    let mut fac: BTreeMap<String, (S3cFaction, u32, BTreeMap<String, u32>)> = BTreeMap::new();
    let mut sides: Vec<BaseSide> = (0..2)
        .map(|b| BaseSide {
            base: b,
            first_owner: vec![0; np],
            first_capture_mean_s: vec![None; np],
            hold_share_pct: vec![0.0; np],
            points_at_mean: vec![None; POINT_MARKS_MIN.len()],
            ..BaseSide::default()
        })
        .collect();
    let mut cap_sum = vec![vec![(0u64, 0u32); np]; 2];
    let mut at_sum = vec![vec![(0u64, 0u32); POINT_MARKS_MIN.len()]; 2];
    for m in matches {
        for (p, s) in m.s3c.iter().enumerate() {
            let (f, start_m, crew) = fac.entry(m.factions[p].to_string()).or_insert_with(|| {
                (
                    S3cFaction {
                        faction: m.factions[p].to_string(),
                        ..S3cFaction::default()
                    },
                    0,
                    BTreeMap::new(),
                )
            });
            f.base_returns += s.base_returns;
            f.base_returns_low_ammo += s.base_returns_low_ammo;
            f.tank_alive_s += s.tank_alive_s;
            f.mg_dry_pct += s.tank_mg_dry_s as f64;
            f.mg_low_pct += s.tank_mg_low_s as f64;
            f.he_low_pct += s.tank_he_low_s as f64;
            f.satchel_hunts += s.satchel_hunts_seen;
            *start_m += s.satchel_hunt_start_m;
            for (k, v) in &s.satchel_hunt_ends {
                *f.satchel_hunt_ends.entry(k.clone()).or_default() += v;
            }
            f.knockouts += s.ko_by_cause.values().sum::<u32>();
            f.ko_with_survivors += s.ko_with_survivors;
            f.bail_with_crew += s.bail_with_crew;
            f.bail_some_out += s.bail_some_out;
            f.crew_out_tracked += s.crew_out_tracked;
            f.crew_alive_60s += s.crew_alive_60s;
            for (k, v) in &s.ko_by_cause {
                *f.ko_crew_alive_mean.entry(k.clone()).or_default() += *v as f64;
            }
            for (k, v) in &s.ko_crew_alive {
                *crew.entry(k.clone()).or_default() += v;
            }
            let b = (m.bases[p] as usize).min(1);
            let side = &mut sides[b];
            side.matches += 1;
            side.wins += u32::from(m.winner == Some(p as u8));
            let other = &m.s3c[1 - p.min(1)];
            for (i, cap) in cap_sum[b].iter_mut().enumerate() {
                let mine = s.first_capture_s[i];
                let theirs = other.first_capture_s.get(i).copied().flatten();
                if let Some(t) = mine {
                    cap.0 += t as u64;
                    cap.1 += 1;
                    if theirs.is_none_or(|o| t < o) {
                        side.first_owner[i] += 1;
                    }
                }
                side.hold_share_pct[i] += s.point_hold_s[i] as f64 * 100.0 / m.seconds.max(1.0);
            }
            for (k, a) in s.points_at.iter().enumerate() {
                if let Some(n) = a {
                    at_sum[b][k].0 += *n as u64;
                    at_sum[b][k].1 += 1;
                }
            }
        }
    }
    let r1 = |x: f64| (x * 10.0).round() / 10.0;
    for (b, side) in sides.iter_mut().enumerate() {
        let n = side.matches.max(1) as f64;
        for (i, &(sum, k)) in cap_sum[b].iter().enumerate() {
            side.hold_share_pct[i] = r1(side.hold_share_pct[i] / n);
            side.first_capture_mean_s[i] = (k > 0).then(|| r1(sum as f64 / k as f64));
        }
        for (k, (sum, c)) in at_sum[b].iter().enumerate() {
            side.points_at_mean[k] =
                (*c > 0).then(|| (*sum as f64 / *c as f64 * 100.0).round() / 100.0);
        }
    }
    let factions = fac
        .into_values()
        .map(|(mut f, start_m, crew)| {
            let t = f.tank_alive_s.max(1) as f64;
            f.mg_dry_pct = r1(f.mg_dry_pct * 100.0 / t);
            f.mg_low_pct = r1(f.mg_low_pct * 100.0 / t);
            f.he_low_pct = r1(f.he_low_pct * 100.0 / t);
            f.satchel_hunt_start_mean_m = r1(start_m as f64 / f.satchel_hunts.max(1) as f64);
            for (k, v) in f.ko_crew_alive_mean.iter_mut() {
                *v =
                    (crew.get(k).copied().unwrap_or(0) as f64 / v.max(1.0) * 100.0).round() / 100.0;
            }
            f
        })
        .collect();
    S3cSummary {
        factions,
        sides,
        point_names,
    }
}

/// The S3c lines of the `--summary` text.
pub fn text(s: &S3cSummary) -> String {
    let mut out = String::from(
        "S3c baseline (report only):\n\
         faction        tank base returns (low ammo)  MG dry % (<1/4)  HE<=reserve %  \
         satchel hunts: seen, ends, mean start m  KOs (with living crew), mean crew alive by cause\n",
    );
    let join = |m: &BTreeMap<String, u32>| {
        m.iter()
            .map(|(k, v)| format!("{k} {v}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    for f in &s.factions {
        let crew = f
            .ko_crew_alive_mean
            .iter()
            .map(|(k, v)| format!("{k} {v:.2}"))
            .collect::<Vec<_>>()
            .join(", ");
        out += &format!(
            "{:<14} {:>4} ({})  {:>16.1} ({:.1})  {:>12.1}  {} [{}] {:.0} m  {} ({}) [{}]\n",
            f.faction,
            f.base_returns,
            f.base_returns_low_ammo,
            f.mg_dry_pct,
            f.mg_low_pct,
            f.he_low_pct,
            f.satchel_hunts,
            join(&f.satchel_hunt_ends),
            f.satchel_hunt_start_mean_m,
            f.knockouts,
            f.ko_with_survivors,
            crew,
        );
    }
    let pct = |a: u32, b: u32| {
        if b == 0 {
            "-".to_string()
        } else {
            format!("{:.0} %", a as f64 * 100.0 / b as f64)
        }
    };
    for f in &s.factions {
        out += &format!(
            "S3c crews {}: abandonments with crew inside {}, some man out {} ({}); men out {} \
             (checked), alive {} s later {} ({})\n",
            f.faction,
            f.bail_with_crew,
            f.bail_some_out,
            pct(f.bail_some_out, f.bail_with_crew),
            f.crew_out_tracked,
            CREW_ALIVE_S,
            f.crew_alive_60s,
            pct(f.crew_alive_60s, f.crew_out_tracked),
        );
    }
    out += "base side  matches wins  per point (first owner / mean first capture s / hold %):";
    for n in &s.point_names {
        out += &format!(" {n:>16}");
    }
    out += "  mean points at 5/10/15/20 min\n";
    for side in &s.sides {
        out += &format!(
            "base {}     {:>7} {:>4}  {:>53}",
            side.base, side.matches, side.wins, ""
        );
        for i in 0..s.point_names.len() {
            let cap = side.first_capture_mean_s[i].map_or("-".into(), |v| format!("{v:.0}"));
            out += &format!(
                " {:>16}",
                format!(
                    "{}/{}/{:.0}",
                    side.first_owner[i], cap, side.hold_share_pct[i]
                )
            );
        }
        let at: Vec<String> = side
            .points_at_mean
            .iter()
            .map(|v| v.map_or("-".into(), |x| format!("{x:.2}")))
            .collect();
        out += &format!("  {}\n", at.join("/"));
    }
    out
}
