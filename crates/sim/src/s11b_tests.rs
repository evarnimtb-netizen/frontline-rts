//! S11b Build A tests (docs/slices/S11b.md 3, 4 X1, X3, G1-G4, 5): the
//! SAVE 29 layout and its load checks, the infantry-gun counters leaving
//! the S9 block, counter-fire that alternates, and the HE rifle grenade.
//! Each test names its control: the edit that makes it fail.

use super::s6_tests::{flat_world_with, members, spawn, steps};
use super::s11a2_ai_tests::{LEIG, MARK, Sent, add_shot, cf_scene, counters, plans};
use super::*;
use crate::FxVec3;
use crate::ai::AiArty;
use crate::game::{RgState, kill_class};
use crate::vehicle::GunState;
use frontline_content::ItemKind;

const RIFLES: &str = "ww2:usa_rifle_squad";
const RG: usize = ItemKind::RifleGrenade.index();

fn c(x: i32, y: i32) -> FxVec2 {
    FxVec2::from_int(x, y)
}

/// Issues rifle grenades (`[items] rifle_grenades` 1) and gives the men of
/// `slot` of card `squad` `n` of them: no card carries them before Build
/// B (spec 4 G1, the test scene of Build A step 8).
pub(super) fn arm(s: &mut frontline_content::MatchSetup, squad: &str, slot: &str, n: u8) {
    s.rules.items.rifle_grenades = 1;
    let it = s
        .items
        .iter()
        .position(|i| i.kind == ItemKind::RifleGrenade)
        .expect("the rifle-grenade item def") as u16;
    let t = s.squad_type_index(squad).unwrap() as usize;
    let ty = &mut s.squad_types[t];
    ty.items[RG] = Some(it);
    let mut any = false;
    for m in ty.members.iter_mut().filter(|m| m.slot == slot) {
        m.items[RG] = n;
        any = true;
    }
    assert!(any, "{squad} has no slot {slot}");
}

fn game_mut(s: &mut SimState) -> &mut MatchState {
    s.game.as_mut().unwrap()
}

fn ai0(s: &mut SimState) -> &mut crate::ai::AiState {
    game_mut(s).ai[0].as_mut().unwrap()
}

fn rg_mut(s: &mut SimState, q: SquadId) -> &mut RgState {
    game_mut(s).squads.get_mut(&q).unwrap().rg.as_mut().unwrap()
}

// ------------------------------------------------------------ layout --

/// The S11b staged save: player 1's US rifle squad (riflemen with 3 rifle
/// grenades) with a man mid-aim, player 0 an AI holding an `AiArty` with
/// `cb_last`, a few S11b counters set. Returns `(world, rifle squad, AI
/// record squad)`.
fn staged() -> (World, SquadId, SquadId) {
    let mut w = flat_world_with(1, |s| {
        arm(s, RIFLES, "rifleman", 3);
        s.players[0].is_ai = true;
    });
    let q = spawn(&mut w, 1, RIFLES, 400, 400);
    let q1 = spawn(&mut w, 0, "ww2:ger_grenadiergruppe", 900, 900);
    steps(&mut w, 5);
    let man = *members(&w, q).last().unwrap();
    let foe = members(&w, q1)[0];
    assert!(w.edit_squad(q, |sq| {
        sq.rg = Some(RgState {
            man: Some(man),
            target: Some(foe),
            at: c(400, 520),
            aim: 40,
            stow: 0,
            gap: 0,
            search: 2,
        })
    }));
    let tick = w.tick();
    assert!(w.with_ai_state(0, |ai| {
        ai.arty.insert(
            q1,
            AiArty {
                since: tick,
                last_order: tick,
                cb_last: true,
                ..AiArty::default()
            },
        );
        ai.counters.cb_alt_holds = 2;
    }));
    assert!(w.edit_player(PlayerId(1), |p| {
        p.s11b.rg_fired = 1;
        p.s11b.how_rounds = [0, 1, 0];
        p.kills_by[kill_class::RIFLE_GRENADE as usize] = 1;
    }));
    (w, q, q1)
}

/// Spec 3: the S11b state saves, loads and hashes. Control: an unedited
/// reload keeps the hash (each edit must change it).
#[test]
fn s11b_layout_roundtrips_and_hashes() {
    let (w, q, q1) = staged();
    let bytes = w.save();
    let back = World::load(&bytes, 1).expect("the S11b state loads");
    assert_eq!(back.game().unwrap().players(), w.game().unwrap().players());
    assert_eq!(back.game().unwrap().ai[0], w.game().unwrap().ai[0]);
    assert_eq!(
        back.game().unwrap().squads[&q],
        w.game().unwrap().squads[&q]
    );
    assert_eq!(back.full_state_hash(), w.full_state_hash());
    assert_eq!(back.save(), bytes);
    let h = back.full_state_hash();
    type Edit = Box<dyn Fn(&mut SimState)>;
    let edits: Vec<Edit> = vec![
        Box::new(move |s| rg_mut(s, q).aim -= 1),
        Box::new(move |s| rg_mut(s, q).at.x += Fx::ONE),
        Box::new(move |s| rg_mut(s, q).gap = 3),
        Box::new(move |s| rg_mut(s, q).search -= 1),
        Box::new(move |s| {
            let r = rg_mut(s, q);
            (r.aim, r.stow, r.target) = (0, 5, None);
        }),
        Box::new(move |s| {
            let r = rg_mut(s, q);
            (r.man, r.aim, r.target) = (None, 0, None);
        }),
        Box::new(move |s| rg_mut(s, q).target = Some(SimId(0))),
        Box::new(move |s| ai0(s).arty.get_mut(&q1).unwrap().cb_last = false),
        Box::new(|s| ai0(s).counters.cb_alt_holds += 1),
        Box::new(|s| ai0(s).counters.how_buys += 1),
        Box::new(|s| ai0(s).counters.how_pct_holds += 1),
        Box::new(|s| ai0(s).counters.how_mp_holds += 1),
        Box::new(|s| game_mut(s).players[1].s11b.arty_emplaced += 1),
        Box::new(|s| game_mut(s).players[1].s11b.arty_crew_lost += 1),
        Box::new(|s| game_mut(s).players[1].s11b.rg_fired += 1),
        Box::new(|s| game_mut(s).players[1].s11b.how_rounds[2] += 1),
        Box::new(|s| game_mut(s).players[1].kills_by[kill_class::RIFLE_GRENADE as usize] += 1),
    ];
    assert_eq!(World::load(&back.save(), 1).unwrap().full_state_hash(), h);
    for (k, e) in edits.into_iter().enumerate() {
        let mut x = World::load(&bytes, 1).unwrap();
        e(&mut x.state);
        let y = World::load(&x.save(), 1).unwrap_or_else(|e| panic!("edit {k}: {e:?}"));
        assert_ne!(y.full_state_hash(), h, "edit {k} is not hashed");
    }
}

/// Spec 3 load validation, one case per check (`cb_last` may be either).
/// Control: the staged save itself loads (checked first), so each failure
/// is the edit's.
#[test]
fn load_rejects_bad_s11b_state() {
    let (mut w, q, _) = staged();
    let other = spawn(&mut w, 1, "ww2:usa_lmg_squad", 600, 400);
    let foe = members(&w, other)[0];
    let man = *members(&w, q).last().unwrap();
    let bytes = w.save();
    World::load(&bytes, 1).expect("control: the staged save loads");
    type Edit = Box<dyn Fn(&mut SimState)>;
    let cases: Vec<(&str, Edit)> = vec![
        (
            "no rg on a squad whose men carry the kind",
            Box::new(move |s| game_mut(s).squads.get_mut(&q).unwrap().rg = None),
        ),
        (
            "rg on a squad that carries none",
            Box::new(move |s| {
                game_mut(s).squads.get_mut(&other).unwrap().rg = Some(RgState::default())
            }),
        ),
        (
            "man not a member",
            Box::new(move |s| rg_mut(s, q).man = Some(foe)),
        ),
        (
            "man with nothing to do",
            Box::new(move |s| {
                let r = rg_mut(s, q);
                (r.aim, r.stow, r.target) = (0, 0, None);
            }),
        ),
        (
            "aim with no target",
            Box::new(move |s| rg_mut(s, q).target = None),
        ),
        (
            "a target while stowing",
            Box::new(move |s| {
                let r = rg_mut(s, q);
                (r.aim, r.stow) = (0, 5);
            }),
        ),
        (
            "target past the entities",
            Box::new(move |s| rg_mut(s, q).target = Some(SimId(u32::MAX >> 4))),
        ),
        (
            "aim with no man",
            Box::new(move |s| rg_mut(s, q).man = None),
        ),
        (
            "aim beyond the wind-up",
            Box::new(move |s| rg_mut(s, q).aim = 91),
        ),
        (
            "stow beyond the stow time",
            Box::new(move |s| {
                let r = rg_mut(s, q);
                (r.aim, r.stow, r.target) = (0, 91, None);
            }),
        ),
        (
            "aim and stow at once",
            Box::new(move |s| rg_mut(s, q).stow = 1),
        ),
        (
            "gap beyond the gap key",
            Box::new(move |s| rg_mut(s, q).gap = 15 * 30 + 1),
        ),
        (
            "search beyond a second",
            Box::new(move |s| rg_mut(s, q).search = 31),
        ),
        (
            "at off the map",
            Box::new(move |s| rg_mut(s, q).at = c(-1, 10)),
        ),
        (
            "kit count above the slot's full kit",
            Box::new(move |s| s.entities.kit[man.index()].items[RG] = 4),
        ),
        (
            "s11b counter out of range",
            Box::new(|s| game_mut(s).players[1].s11b.rg_fired = u32::MAX),
        ),
        (
            "AI counter out of range",
            Box::new(|s| ai0(s).counters.how_mp_holds = u32::MAX),
        ),
    ];
    for (name, e) in cases {
        let mut x = World::load(&bytes, 1).unwrap();
        e(&mut x.state);
        assert!(World::load(&x.save(), 1).is_err(), "{name}: loaded");
    }
    // The bounds are inclusive: aim at the wind-up (3 s), the gap at its key.
    let mut x = World::load(&bytes, 1).unwrap();
    rg_mut(&mut x.state, q).aim = 90;
    rg_mut(&mut x.state, q).gap = 15 * 30;
    World::load(&x.save(), 1).expect("aim and gap at their bounds load");
}

// --------------------------------------------------------------- X1 --

/// Emplaces gun `g` of player `p` (limbered first) and returns once it is
/// emplaced.
fn emplace(w: &mut World, p: u8, g: SimId) {
    w.edit_vehicle(g, |vs| {
        vs.served.as_mut().unwrap().state = GunState::Limbered;
    });
    w.step(&[Command {
        player: PlayerId(p),
        seq: 1,
        kind: CommandKind::Emplace {
            guns: vec![g],
            facing: c(500, 900),
        },
    }]);
    let t0 = w.tick();
    while w.game().unwrap().vehicles[&g]
        .served
        .as_ref()
        .unwrap()
        .state
        != GunState::Emplaced
    {
        assert!(w.tick() < t0 + 30 * 300, "not emplaced in 5 minutes");
        w.step(&[]);
    }
}

/// X1 (spec 4): an infantry gun's emplacement and crew loss count in
/// `s11b`, not in `s9.guns_emplaced` / `s9.gun_crew_lost`. Control: an AT
/// gun's count in `s9` (and not in `s11b`).
#[test]
fn infantry_gun_counts_leave_the_s9_block() {
    let run = |card: &str| {
        let mut w = flat_world_with(1, |_| {});
        let sq = spawn(&mut w, 0, card, 500, 500);
        steps(&mut w, 2);
        let g = members(&w, sq)[0];
        emplace(&mut w, 0, g);
        let man = w.game().unwrap().vehicles[&g]
            .served
            .as_ref()
            .unwrap()
            .slots
            .iter()
            .find_map(|s| s.man)
            .unwrap();
        assert!(w.kill_soldier(man));
        let p = &w.game().unwrap().players[0];
        (
            p.s9.guns_emplaced,
            p.s9.gun_crew_lost,
            p.s11b.arty_emplaced,
            p.s11b.arty_crew_lost,
        )
    };
    assert_eq!(run(LEIG), (0, 0, 1, 1), "the leIG 18 counts in s11b");
    assert_eq!(
        run("ww2:ger_pak40_squad"),
        (1, 1, 0, 0),
        "control: the Pak 40 counts in s9"
    );
}

// --------------------------------------------------------------- X3 --

/// X3: with `cb_alternate` 1 a gun does not order counter-fire twice in a
/// row: after one counter-fire mission, new evidence (3 more rounds heard,
/// no A4 target) is held (`cb_alt_holds`). Control `cb_alternate` 0: a
/// second counter-fire mission.
#[test]
fn ai_gun_does_not_counter_fire_twice_in_a_row() {
    let run = |alt: u32| {
        let (mut w, gun) = cf_scene(1, 0, LEIG, 100, MARK, false, |s| {
            s.rules.ai.arty.cb_alternate = alt
        });
        let mut sent = Sent::default();
        plans(&mut w, 40, &mut sent);
        assert_eq!(counters(&w).cb_missions, 1, "the first marker is answered");
        assert!(ai_rec(&w, gun).cb_last, "the turn is counter-fire's");
        for _ in 0..3 {
            add_shot(&mut w, 0);
        }
        plans(&mut w, 40, &mut sent);
        let k = counters(&w);
        (
            sent.missions_near(gun, c(MARK.0, MARK.1), 1),
            k.cb_missions,
            k.cb_alt_holds > 0,
        )
    };
    assert_eq!(run(1), (1, 1, true), "held: one mission");
    assert_eq!(run(0), (2, 2, false), "control: a second counter-fire");
}

fn ai_rec(w: &World, gun: SquadId) -> AiArty {
    w.game().unwrap().ai[0].as_ref().unwrap().arty[&gun]
}

/// X3: after an ordinary (observed) mission the turn is counter-fire's
/// again: counter-fire, then the A4 target, then counter-fire on the new
/// evidence. Control: with no A4 target the evidence stays held.
#[test]
fn ai_counter_fire_resumes_after_an_observed_mission() {
    let run = |observed: bool| {
        let (mut w, gun) = cf_scene(1, 0, LEIG, 100, MARK, observed, |s| {
            s.rules.ai.arty.cb_alternate = 1
        });
        let mut sent = Sent::default();
        let t0 = w.tick();
        while counters(&w).cb_missions == 0 {
            assert!(w.tick() < t0 + 30 * 120, "no counter-fire in 2 minutes");
            w.step(&[]);
            sent.look(&w);
        }
        for _ in 0..3 {
            add_shot(&mut w, 0);
        }
        plans(&mut w, 120, &mut sent);
        let k = counters(&w);
        (
            sent.missions_near(gun, c(MARK.0, MARK.1), 1),
            k.arty_missions > 0,
            k.cb_alt_holds > 0,
        )
    };
    let (cf, a4, held) = run(true);
    assert_eq!(
        (cf, a4),
        (2, true),
        "counter-fire, the A4 target, counter-fire"
    );
    assert!(held, "held once before the A4 mission");
    assert_eq!(
        run(false),
        (1, false, true),
        "control: no A4 target, the evidence stays held"
    );
}

/// X3 (spec 5 "Counter-fire turn", save/load mid-job): the alternating
/// turn plays the same on 1 and 4 threads and through a save and load
/// after the first counter-fire mission (the `cb_last` flag set). Control:
/// the scene holds a counter-fire mission and an alternate hold.
#[test]
fn ai_counter_fire_alternate_deterministic_threads_and_save_load() {
    let end = 30 * 120;
    let go = |threads: usize, save_at: Option<u64>| {
        let (mut w, gun) = cf_scene(threads, 0, LEIG, 100, MARK, true, |s| {
            s.rules.ai.arty.cb_alternate = 1
        });
        let mut shots = 0;
        while w.tick() < end {
            if Some(w.tick()) == save_at {
                w = World::load(&w.save(), threads).unwrap();
            }
            w.step(&[]);
            if shots < 3 && counters(&w).cb_missions == 1 && ai_rec(&w, gun).cb_last {
                add_shot(&mut w, 0);
                shots += 1;
            }
        }
        let k = counters(&w);
        (w.full_state_hash(), k.cb_missions, k.cb_alt_holds)
    };
    let (h1, cb, held) = go(1, None);
    assert!(
        cb >= 1 && held > 0,
        "control: counter-fire and a hold ({cb}, {held})"
    );
    assert_eq!(go(4, None).0, h1, "4 threads");
    assert_eq!(go(1, Some(30 * 60)).0, h1, "save and load at 60 s");
}

// ------------------------------------------------------- rifle grenade --

/// The M7 launcher and its round (spec G1 def files).
fn launcher(w: &World) -> (u16, u16) {
    let s = w.match_setup().unwrap();
    let wi = s
        .weapons
        .iter()
        .position(|x| x.id == "ww2:m7_launcher")
        .unwrap() as u16;
    (wi, s.weapons[wi as usize].ammo.unwrap())
}

/// Strips a squad's rounds (it can neither shoot nor kill with its rifles).
fn unarm(w: &mut World, q: SquadId) {
    for m in members(w, q) {
        w.edit_soldier(m, |k, c| {
            k.rounds = [0; frontline_content::setup::MAX_FEEDS];
            c.mag = 0;
        });
    }
}

/// The G scene: player 1's unarmed US rifle squad (riflemen with three
/// rifle grenades) at (300, 500), facing an unarmed German squad at
/// (300 + `d`, 500); with `single` only its first man lives. `edit`
/// changes the setup last. Returns `(world, grenadiers, first German)`.
fn rg_scene(
    threads: usize,
    d: i32,
    single: bool,
    edit: impl FnOnce(&mut frontline_content::MatchSetup),
) -> (World, SquadId, SimId) {
    let mut w = flat_world_with(threads, |s| {
        arm(s, RIFLES, "rifleman", 3);
        edit(s);
    });
    let q = spawn(&mut w, 1, RIFLES, 300, 500);
    let t = spawn(&mut w, 0, "ww2:ger_grenadiergruppe", 300 + d, 500);
    unarm(&mut w, q);
    unarm(&mut w, t);
    let men = members(&w, t);
    if single {
        for &m in &men[1..] {
            assert!(w.kill_soldier(m));
        }
    }
    (w, q, men[0])
}

fn fired(w: &World) -> u32 {
    w.game().unwrap().players[1].s11b.rg_fired
}

fn rg(w: &World, q: SquadId) -> RgState {
    w.game().unwrap().squads[&q].rg.unwrap()
}

/// Steps until `n` grenades were fired or `secs` passed; the tick of each
/// new shot.
fn shots(w: &mut World, n: u32, secs: u64) -> Vec<u64> {
    let mut at = Vec::new();
    let end = w.tick() + 30 * secs;
    let mut last = fired(w);
    while w.tick() < end && (at.len() as u32) < n {
        w.step(&[]);
        if fired(w) != last {
            last = fired(w);
            at.push(w.tick());
        }
    }
    at
}

/// G2-G3: a rifleman fits the launcher and fires at a seen enemy man at
/// 100 m (one grenade off his kit). Control: the target unseen (the US
/// side sees 5 m): no grenade.
#[test]
fn rifle_grenadier_fires_at_seen_infantry() {
    let run = |seen: bool| {
        let (mut w, q, _) = rg_scene(1, 100, false, |s| {
            if !seen {
                for t in s.squad_types.iter_mut() {
                    t.sight = frontline_content::Fixed::from_int(5);
                }
            }
        });
        let n = shots(&mut w, 1, 30).len();
        let left: u32 = members(&w, q)
            .iter()
            .map(|m| w.entities().kit()[m.index()].items[RG] as u32)
            .sum();
        (n, left)
    };
    // Build B (C5): three riflemen (two of the five are now the
    // `rifle_grenadier` slot, which `arm` leaves empty): 9 grenades.
    assert_eq!(run(true), (1, 8), "one grenade fired");
    assert_eq!(run(false), (0, 9), "control: unseen, none");
}

/// Integrates a drag-free round from `from` with `vel` (the sim's
/// semi-implicit substeps) until it falls through `to`'s height; the
/// horizontal miss from `to` (m).
fn miss(from: FxVec3, vel: FxVec3, to: FxVec3) -> Fx {
    let mut p = crate::projectile::Projectile {
        pos: from,
        vel,
        owner: SimId(0),
        drag: Fx::ZERO,
        ttl: u16::MAX,
        weapon: 0,
        player: 0,
        ammo: 0,
        origin: from,
        mission: 0,
    };
    for _ in 0..60 * 30 {
        let next = crate::projectile::advance(&mut p);
        if p.vel.z < Fx::ZERO && next.z <= to.z {
            // Interpolate the crossing inside the substep.
            let s = (p.pos.z - to.z).div(p.pos.z - next.z);
            let at = p.pos + (next - p.pos) * s;
            return (at.xy() - to.xy()).length();
        }
        p.pos = next;
    }
    Fx::from_int(9999)
}

/// G3: the low root of the drag-free arc lands the grenade on its aim
/// point: spread 0, 60 m, 120 m and the full 180 m, level and 5 m up,
/// within 3 m (measured: under 0.1 m). Control: the `gunnery::solution`
/// aim (flat aim plus the drop) misses by over 8 m at 150 m.
#[test]
fn rifle_grenade_lands_on_its_aim_point() {
    let speed = Fx::from_int(50);
    let from = FxVec3::new(Fx::from_int(100), Fx::from_int(100), Fx::from_ratio(3, 2));
    for d in [60, 120, 180] {
        for up in [0, 5] {
            let to = FxVec3::new(Fx::from_int(100 + d), Fx::from_int(100), Fx::from_int(up));
            let (vel, flight) = crate::indirect::low_root(from, to, speed).expect("a root");
            assert!(flight > Fx::ZERO);
            let m = miss(from, vel, to);
            eprintln!(
                "rifle grenade aim: {d} m, {up} m up: miss {} mm",
                m.mul_int(1000).floor_int()
            );
            assert!(m <= Fx::from_int(3), "{d} m, {up} m up: missed by {m:?}");
        }
    }
    // Beyond the reach of 50 m/s (255 m drag-free): no root.
    let far = FxVec3::new(Fx::from_int(400), Fx::from_int(100), Fx::ZERO);
    assert!(crate::indirect::low_root(from, far, speed).is_none());
    // Control: the flat aim with the drop added falls short at 150 m.
    let to = FxVec3::new(Fx::from_int(250), Fx::from_int(100), Fx::ZERO);
    let (dir, _) = crate::gunnery::solution(from, to, FxVec2::ZERO, speed);
    assert!(
        miss(from, dir * speed, to) > Fx::from_int(8),
        "control: the flat aim misses"
    );
}

/// G3 in the match: the grenade bursts on the target's ground point with
/// spread 0 (the first burst within 3 m of where the man stood). Control:
/// its kill or wound counts as a rifle grenade (G4 burst class).
#[test]
fn rifle_grenade_bursts_at_the_target_in_the_match() {
    let (mut w, q, _) = rg_scene(1, 100, false, |s| {
        let wi = s
            .weapons
            .iter()
            .position(|x| x.id == "ww2:m7_launcher")
            .unwrap();
        s.weapons[wi].spread = frontline_content::Fixed::ZERO;
    });
    let mut aim = None;
    let t0 = w.tick();
    while w.snapshot().explosions.is_empty() || fired(&w) == 0 {
        assert!(w.tick() < t0 + 30 * 60, "no burst in a minute");
        w.step(&[]);
        if aim.is_none() && rg(&w, q).target.is_some() {
            aim = Some(rg(&w, q).at);
        }
    }
    let aim = aim.expect("an aim");
    let e = &w.snapshot().explosions[0];
    let burst = FxVec2::new(
        Fx::from_milli((e.pos[0] * 1000.0) as i32),
        Fx::from_milli((e.pos[1] * 1000.0) as i32),
    );
    assert!(
        (burst - aim).length() <= Fx::from_int(3),
        "burst {burst:?} vs aim {aim:?}"
    );
}

/// G2: no grenade at a seen man inside `min_range_m` or beyond
/// `range_m`: the band is moved around a man 100 m off (a squad's
/// formation spreads its men some 20 m, so the band, not the man, is
/// moved): minimum 150 m, then range 60 m. Control: the band of the def
/// (30-180 m): one.
#[test]
fn rifle_grenade_respects_its_range_band() {
    use frontline_content::Fixed;
    let n = |lo: i32, hi: i32| {
        let (mut w, _, t) = rg_scene(1, 100, true, |s| {
            let i = s
                .items
                .iter()
                .position(|i| i.kind == ItemKind::RifleGrenade)
                .unwrap();
            (s.items[i].range_min, s.items[i].range_max) =
                (Fixed::from_int(lo), Fixed::from_int(hi));
        });
        let mut seen = 0;
        for _ in 0..30 * 30 {
            w.step(&[]);
            seen += u32::from(w.entities().combat()[t.index()].seen_by & 2 != 0);
        }
        assert!(seen > 0, "the target is seen");
        fired(&w).min(1)
    };
    assert_eq!(n(150, 180), 0, "inside the minimum");
    assert_eq!(n(10, 60), 0, "beyond the range");
    assert_eq!(n(30, 180), 1, "control: inside the band");
}

/// G3: one grenade a squad per `rifle_grenade_gap_s` (15 s); the firer's
/// rifle is silent while he stows the launcher (3 s). Control gap 0: the
/// next shot one wind-up (3 s) after the stow.
#[test]
fn rifle_grenade_gap_and_stow() {
    let run = |gap: u32| {
        let (mut w, q, _) = rg_scene(1, 100, false, |s| {
            s.rules.items.rifle_grenade_gap_ticks = gap * 30
        });
        let first = shots(&mut w, 1, 30);
        assert_eq!(first.len(), 1);
        let r = rg(&w, q);
        let man = r.man.expect("stowing");
        assert_eq!((r.aim, r.stow, r.target), (0, 90, None), "stowing 3 s");
        // Silent while stowing.
        for _ in 0..89 {
            w.step(&[]);
            assert!(
                w.entities().weapon()[man.index()].cooldown > 0,
                "his rifle waits"
            );
        }
        w.step(&[]);
        let r = rg(&w, q);
        assert_eq!(r.stow, 0, "stowed");
        // With a gap the squad waits; with none the next aim may start.
        assert!(r.man.is_none() || r.target.is_some());
        let next = shots(&mut w, 1, 40);
        next[0] - first[0]
    };
    let gap = run(15);
    assert!(gap >= 15 * 30, "not before the gap: {gap} ticks");
    let free = run(0);
    assert!(
        (6 * 30..15 * 30).contains(&free),
        "control: stow and wind-up only: {free}"
    );
}

/// Spec 5 "Grenade aim": two grenadiers in one squad aim one at a time
/// (one `man`; shots at least a wind-up apart with gap 0 and no stow).
/// Control: the squad holds two grenadiers with grenades left (both fire
/// in the end).
#[test]
fn two_grenadiers_aim_one_at_a_time() {
    let (mut w, q, _) = rg_scene(1, 100, false, |s| {
        s.rules.items.rifle_grenade_gap_ticks = 0;
        let i = s
            .items
            .iter()
            .position(|i| i.kind == ItemKind::RifleGrenade)
            .unwrap();
        s.items[i].stow_ticks = 0;
        // Two riflemen with one grenade each, the rest none.
        let t = s.squad_type_index(RIFLES).unwrap() as usize;
        for m in s.squad_types[t].members.iter_mut() {
            m.items[RG] = 0;
        }
        let k = s.squad_types[t]
            .members
            .iter()
            .position(|m| m.slot == "scout")
            .unwrap();
        s.squad_types[t].members[k].items[RG] = 1;
    });
    let at = shots(&mut w, 2, 40);
    assert_eq!(at.len(), 2, "both scouts fire");
    assert!(at[1] - at[0] >= 90, "one aim at a time: {at:?}");
    let firers: Vec<SimId> = members(&w, q)
        .into_iter()
        .filter(|m| w.entities().kit()[m.index()].items[RG] == 0)
        .collect();
    assert!(firers.len() >= 2);
}

/// Spec 5 "Grenade aim": the target dies mid-aim: the aim is cleared with
/// no gap and no grenade flies. Control: the target lives: the shot.
#[test]
fn grenade_aim_clears_when_the_target_leaves_the_band() {
    let run = |kill: bool| {
        let (mut w, q, t) = rg_scene(1, 100, true, |_| {});
        let t0 = w.tick();
        while rg(&w, q).target.is_none() {
            assert!(w.tick() < t0 + 30 * 30, "no aim");
            w.step(&[]);
        }
        if kill {
            assert!(w.kill_soldier(t));
        }
        steps(&mut w, 2);
        let r = rg(&w, q);
        let broken = (r.man, r.target, r.gap);
        steps(&mut w, 120);
        (broken, fired(&w))
    };
    let ((man, target, gap), n) = run(true);
    assert_eq!(
        (man, target, gap, n),
        (None, None, 0, 0),
        "cleared, no gap, no shot"
    );
    let ((man, _, _), n) = run(false);
    assert!(man.is_some() && n == 1, "control: aiming, then the shot");
}

/// G2: a pinned grenadier does not fire (the squad is held at full
/// suppression). Control: not pinned: he fires.
#[test]
fn pinned_grenadier_does_not_fire() {
    let run = |pin: bool| {
        let (mut w, q, _) = rg_scene(1, 100, false, |_| {});
        for _ in 0..30 * 20 {
            if pin {
                for m in members(&w, q) {
                    w.edit_soldier(m, |_, c| {
                        c.suppression = 100_000;
                        c.pinned = true;
                    });
                }
            }
            w.step(&[]);
        }
        fired(&w)
    };
    assert_eq!(run(true), 0, "pinned: none");
    assert!(run(false) >= 1, "control: fired");
}

/// G2-G3 determinism: the grenade scene plays the same on 1 and 4 threads
/// and through a save and load mid-aim and mid-flight. Control: the scene
/// holds an aim and a flight at the save ticks.
#[test]
fn rifle_grenade_deterministic_threads_and_save_load() {
    let end = 30 * 40;
    let (mut w, q, _) = rg_scene(1, 100, false, |_| {});
    let (mut aiming, mut flying) = (None, None);
    while w.tick() < end {
        w.step(&[]);
        if aiming.is_none() && rg(&w, q).aim > 10 {
            aiming = Some(w.tick());
        }
        if flying.is_none() && fired(&w) > 0 && w.snapshot().explosions.is_empty() {
            flying = Some(w.tick());
        }
    }
    let h1 = w.full_state_hash();
    assert!(fired(&w) >= 2, "control: grenades fired");
    let at = [aiming.expect("mid-aim"), flying.expect("mid-flight")];
    let go = |threads: usize, reload: bool| {
        let (mut w, _, _) = rg_scene(threads, 100, false, |_| {});
        while w.tick() < end {
            if reload && at.contains(&w.tick()) {
                w = World::load(&w.save(), threads).unwrap();
            }
            w.step(&[]);
        }
        w.full_state_hash()
    };
    assert_eq!(go(4, false), h1, "4 threads");
    assert_eq!(go(1, true), h1, "save and load mid-aim and mid-flight");
}

/// G4: a rifle-grenade round is its own kill class; a Panzerfaust round
/// stays `ROCKET` (control).
#[test]
fn rifle_grenade_kill_is_its_own_class() {
    let (w, q, _) = rg_scene(1, 100, true, |_| {});
    let g = w.game().unwrap();
    let man = members(&w, q)[0];
    let (wi, ammo) = launcher(&w);
    assert_eq!(
        g.round_class(&w.state.entities, wi, ammo, man),
        kill_class::RIFLE_GRENADE
    );
    assert_eq!(
        kill_class::NAMES[kill_class::RIFLE_GRENADE as usize],
        "rifle_grenade"
    );
    let s = w.match_setup().unwrap();
    let pzf = s.weapons.iter().position(|x| x.id == "ww2:pzf60").unwrap();
    let pammo = s.weapons[pzf].ammo.unwrap();
    assert_eq!(
        g.round_class(&w.state.entities, pzf as u16, pammo, man),
        kill_class::ROCKET,
        "control: the Panzerfaust"
    );
}

/// G4: the base does not refill rifle grenades (crate-only by design): a
/// rifleman with none, 60 s in his own base, still has none. Control: his
/// hand grenade is back.
#[test]
fn rifle_grenades_do_not_refill_in_base() {
    let mut w = flat_world_with(1, |s| arm(s, RIFLES, "rifleman", 3));
    let s = w.match_setup().unwrap();
    let base = &s.map.bases[s.players[1].base as usize];
    let at = FxVec2::from(base.center);
    let q = w
        .spawn_squad_at(
            PlayerId(1),
            s.squad_type_index(RIFLES).unwrap(),
            at,
            FxVec2::new(Fx::ONE, Fx::ZERO),
        )
        .unwrap();
    let man = *members(&w, q).last().unwrap();
    let frag = ItemKind::Frag.index();
    let full = w.entities().kit()[man.index()].items;
    assert!(full[RG] == 3 && full[frag] >= 1, "a rifleman: {full:?}");
    w.edit_soldier(man, |k, _| {
        k.items[RG] = 0;
        k.items[frag] = 0;
    });
    steps(&mut w, 30 * 60);
    let k = w.entities().kit()[man.index()].items;
    assert_eq!(k[RG], 0, "no rifle grenade from the base");
    assert!(k[frag] >= 1, "control: his hand grenade is back");
}

// ------------------------------------------- stage A review fixes (B) --

/// Review A-1: the grenadier killed mid-aim takes his aim with him, so a
/// save in that tick loads. Control: a kill of another man leaves the aim.
#[test]
fn killed_grenadier_mid_aim_saves_and_loads() {
    let run = |own: bool| {
        let (mut w, q, _) = rg_scene(1, 100, false, |_| {});
        let t0 = w.tick();
        while rg(&w, q).aim < 10 {
            assert!(w.tick() < t0 + 30 * 30, "no aim");
            w.step(&[]);
        }
        let man = rg(&w, q).man.unwrap();
        let other = *members(&w, q).iter().find(|m| **m != man).unwrap();
        assert!(w.kill_soldier(if own { man } else { other }));
        let r = rg(&w, q);
        let w2 = World::load(&w.save(), 1).expect("the save loads");
        assert_eq!(w2.full_state_hash(), w.full_state_hash());
        (r.man, r.target, r.aim, r.stow)
    };
    assert_eq!(run(true), (None, None, 0, 0), "cleared with him");
    let (man, target, aim, _) = run(false);
    assert!(
        man.is_some() && target.is_some() && aim > 0,
        "control: aiming"
    );
}

/// Review A-1: a rifleman stowing his launcher boards an abandoned tank
/// as crew (`bail.rs` boarding): the state ends with him and the save in
/// that tick loads. Control: the men who stay keep theirs (the record is
/// only cleared for a man who boards).
#[test]
fn boarding_grenadier_saves_and_loads() {
    const TANK: &str = "ww2:usa_m4a3_75_squad";
    // The scene: player 1's tank abandoned at (500, 500) by order, its
    // crew walked off; the rifle squad (grenades issued) boards it.
    let scene = || {
        let mut w = flat_world_with(1, |s| arm(s, RIFLES, "rifleman", 3));
        let tq = spawn(&mut w, 1, TANK, 500, 500);
        let v = members(&w, tq)[0];
        super::s6_tests::run(&mut w, 1, 1, CommandKind::Abandon { squad: tq });
        let t0 = w.tick();
        while w.entities().kind[v.index()] != EntityKind::Abandoned {
            assert!(w.tick() < t0 + 30 * 20, "not abandoned");
            w.step(&[]);
        }
        steps(&mut w, 30 * 6);
        let crew = w.game().unwrap().vehicles[&v].crew_squad;
        if let Some(c) = crew {
            super::s6_tests::run(&mut w, 1, 2, CommandKind::Retreat { squad: c });
        }
        let q = spawn(&mut w, 1, RIFLES, 500, 520);
        super::s6_tests::run(
            &mut w,
            1,
            3,
            CommandKind::Recrew {
                squad: q,
                vehicle: v,
            },
        );
        (w, q, v)
    };
    // First run: which men board.
    let (mut w, q, v) = scene();
    let men0 = members(&w, q);
    let t0 = w.tick();
    while w.entities().kind[v.index()] != EntityKind::Vehicle {
        assert!(w.tick() < t0 + 30 * 40, "not re-crewed");
        w.step(&[]);
    }
    let left = members(&w, q);
    let boarder = *men0
        .iter()
        .find(|m| !left.contains(m))
        .expect("a man boards");
    let stayer = left[0];
    // Second run: hold `who` stowing (no aim to break) until the tick the
    // men board, then save and load.
    let run = |who: SimId| {
        let (mut w, q, v) = scene();
        loop {
            let s = w.tick();
            assert!(s < t0 + 30 * 40, "not re-crewed");
            assert!(w.edit_squad(q, |sq| {
                sq.rg = Some(RgState {
                    man: Some(who),
                    stow: 60,
                    ..RgState::default()
                })
            }));
            w.step(&[]);
            if w.entities().kind[v.index()] == EntityKind::Vehicle {
                break;
            }
        }
        let r = rg(&w, q);
        let w2 = World::load(&w.save(), 1).expect("the save loads");
        assert_eq!(w2.full_state_hash(), w.full_state_hash());
        r.man
    };
    assert_eq!(run(boarder), None, "the boarder's state ends");
    assert_eq!(run(stayer), Some(stayer), "control: a man who stays");
}

/// Review A-2: entity ids are not generational. A target whose slot now
/// holds a man of the firer's own side breaks the aim. Control: the
/// enemy target keeps it.
#[test]
fn grenade_aim_breaks_when_the_target_slot_turns_friendly() {
    let run = |own: bool| {
        let (mut w, q, t) = rg_scene(1, 100, true, |_| {});
        let t0 = w.tick();
        while rg(&w, q).aim < 10 {
            assert!(w.tick() < t0 + 30 * 30, "no aim");
            w.step(&[]);
        }
        if own {
            w.state.entities.owner[t.index()] = PlayerId(1);
            w.edit_soldier(t, |_, _| {});
        }
        steps(&mut w, 2);
        rg(&w, q).target
    };
    assert_eq!(run(true), None, "own man in the slot: aim broken");
    assert!(run(false).is_some(), "control: still aiming");
}

/// Review A-3: a fired rifle grenade is not a Panzerfaust (nor a rocket)
/// in `ammo.fired`; it is counted in `s11b.rg_fired`. Control: the
/// grenade was fired.
#[test]
fn rifle_grenade_is_not_counted_as_a_panzerfaust() {
    use crate::supply::fired_class as f;
    let (mut w, _, _) = rg_scene(1, 100, false, |_| {});
    assert_eq!(shots(&mut w, 1, 30).len(), 1, "control: one fired");
    let a = &w.game().unwrap().players[1].ammo.fired;
    assert_eq!((a[f::PANZERFAUST], a[f::ROCKET]), (0, 0));
}

// ------------------------------------------------- Build B content --

/// Build B step 2 (spec 4 C1-C2): the real howitzer cards need no sim
/// code: spawned in the crest scene of S11a1 on flat ground (gun at
/// (50, 500), a Grenadier squad seeing (650, 500)), a mission 600 m off
/// is accepted and a round is fired, counted in `s11.gun_rounds` and in
/// `s11b.how_rounds` (the card's `field_howitzer` tag; control: the
/// leIG 18's round is not). Control: `gun_missions` 0 refuses it
/// `NotMortar`.
#[test]
fn a_howitzer_card_takes_a_fire_mission() {
    use super::s11a1_tests::{crest_gun, mission, order};
    use crate::fire_mission::FireMissionRefusal;
    let t = c(650, 500);
    for (card, how) in [
        ("ww2:ger_lefh18_squad", 1),
        ("ww2:usa_105mm_m2a1_squad", 1),
        ("ww2:ger_leig18_squad", 0),
    ] {
        let (mut w, sq) = crest_gun(card, false);
        assert_eq!(order(&mut w, sq, t, 2), None, "{card} accepts");
        assert!(mission(&w, sq).is_some(), "{card} on the mission");
        let t0 = w.tick();
        let p = |w: &World| {
            let p = &w.game().unwrap().players[0];
            (
                p.s11.gun_rounds.iter().sum::<u32>(),
                p.s11b.how_rounds.iter().sum::<u32>(),
            )
        };
        while p(&w).0 == 0 {
            assert!(w.tick() < t0 + 30 * 180, "{card}: no round in 180 s");
            w.step(&[]);
        }
        // Control: the leIG 18 (no `field_howitzer` tag) is no howitzer.
        assert_eq!(p(&w), (1, how), "{card}: howitzer rounds");
    }
    // Control: gun missions off.
    let mut w = flat_world_with(1, |s| {
        s.rules.fire_mission.as_mut().unwrap().gun_missions = 0
    });
    steps(&mut w, 2);
    let sq = spawn(&mut w, 0, "ww2:ger_lefh18_squad", 50, 500);
    spawn(&mut w, 0, "ww2:ger_grenadiergruppe", 600, 500);
    steps(&mut w, 30);
    assert_eq!(order(&mut w, sq, t, 2), Some(FireMissionRefusal::NotMortar));
}

/// C4: each light mortar's table reaches its game range (275 m kz. GrW 42,
/// 370 m 60 mm; the weapon's `range_m`, which bounds the AI's band and the
/// table range; the solver itself reaches about 1,124 m / 1,800 m: the
/// band is the AI's, not a refusal) and its minimum (60 m, 90 m),
/// and the solver refuses 10 m inside the minimum (no charge rises
/// steeply enough). Control: the heavy tubes' game range is 600 m.
#[test]
fn light_mortars_reach_their_game_range_and_minimum() {
    use crate::indirect::solve;
    let w = flat_world_with(1, |_| {});
    let s = w.match_setup().unwrap();
    let wp = |id: &str| {
        s.weapons
            .iter()
            .find(|w| w.id == id)
            .unwrap_or_else(|| panic!("{id}"))
    };
    let from = FxVec3::new(Fx::from_int(100), Fx::from_int(500), Fx::ZERO);
    let at = |d: i32| FxVec3::new(Fx::from_int(100 + d), Fx::from_int(500), Fx::ZERO);
    for (id, min, max) in [("ww2:kz_grw42", 60, 275), ("ww2:m2_60mm", 90, 370)] {
        let w = wp(id);
        let ind = w.indirect.as_ref().unwrap();
        let (lo, hi): (Fx, Fx) = (w.range_min.into(), w.range_max.into());
        let table: Fx = ind.range.into();
        assert_eq!(
            (lo, hi, table),
            (Fx::from_int(min), Fx::from_int(max), hi),
            "{id}"
        );
        assert!(solve(ind, from, at(max)).is_some(), "{id} reaches {max} m");
        assert!(solve(ind, from, at(min)).is_some(), "{id} reaches {min} m");
        assert!(
            solve(ind, from, at(min - 10)).is_none(),
            "{id} refuses {} m",
            min - 10
        );
    }
    for id in ["ww2:grw34", "ww2:m1_mortar"] {
        let hi: Fx = wp(id).range_max.into();
        assert_eq!(hi, Fx::from_int(600), "control: {id}");
    }
}

/// C7 (D 848): the artillery group holds at most 2 gun cards alive a
/// player: with the leIG 18 and the leFH 18 alive a third gun card (a
/// second leIG 18, under its own `max_alive` 2) is refused `GroupLimit`.
/// Control: the group entry removed (no card in a group): it is allowed.
#[test]
fn artillery_group_caps_two() {
    let run = |group: bool| {
        let mut w = flat_world_with(1, |s| {
            s.rules.start_manpower = 100_000;
            if !group {
                s.rules.limit_groups.clear();
                for t in s.squad_types.iter_mut() {
                    t.limit_group = None;
                }
            }
        });
        spawn(&mut w, 0, LEIG, 300, 300);
        spawn(&mut w, 0, "ww2:ger_lefh18_squad", 300, 400);
        steps(&mut w, 2);
        let g = w.game().unwrap();
        let t = g.setup.squad_type_index(LEIG).unwrap();
        g.call_in_check(w.entities(), PlayerId(0), t, 30 * 3600)
    };
    assert!(
        matches!(
            run(true),
            Err(crate::game::CallInRefusal::GroupLimit { .. })
        ),
        "{:?}",
        run(true)
    );
    assert_eq!(run(false), Ok(()), "control: no group");
}
