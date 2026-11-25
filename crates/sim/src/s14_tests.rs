//! S14a Build A tests (docs/slices/S14-assault-kit.md 3): the SAVE 33
//! layout and its load checks, then the flame rules F1-F9 (every test
//! turns its switch on in the test only, with test-made flame defs). Each
//! test names its control: the edit that makes it fail.

use super::s6_tests::{flat_world_with, members, spawn, steps};
use super::*;
use crate::flame::{Clear, Close, Flame, Patch, STRIKE_RECOVER, STRIKE_WINDUP};
use crate::fx::FxVec3;
use crate::game::kill_class;

const RIFLES: &str = "ww2:ger_grenadiergruppe";
const US_RIFLES: &str = "ww2:usa_rifle_squad";

fn game_mut(s: &mut SimState) -> &mut MatchState {
    s.game.as_mut().unwrap()
}

fn close(s: &mut SimState, id: SimId) -> &mut Close {
    &mut s.entities.combat[id.index()].close
}

fn ai(s: &mut SimState) -> &mut crate::ai::AiState {
    game_mut(s).ai[1].as_mut().unwrap()
}

/// The first 4 m cell dug to trench depth.
fn dug_cell(w: &World) -> (u16, u16) {
    let t = &w.state.terrain;
    for gy in 0..t.dim() {
        for gx in 0..t.dim() {
            if t.dug_dm(gx, gy) >= crate::terrain::TRENCH_NAV_DM {
                return (gx as u16, gy as u16);
            }
        }
    }
    panic!("no dug cell");
}

/// The S14 staged save: a German rifle squad with a burning man and a man
/// mid-strike, a trench it is clearing, two particles in flight, a patch,
/// S14 counters and the S14 AI counters (player 1 is an AI).
/// Returns `(world, squad, burning man, striking man)`.
pub(super) fn staged() -> (World, SquadId, SimId, SimId) {
    let mut w = flat_world_with(1, |s| s.players[1].is_ai = true);
    let sq = spawn(&mut w, 0, RIFLES, 300, 300);
    spawn(&mut w, 1, US_RIFLES, 700, 700);
    steps(&mut w, 3);
    w.dig_trench_now(
        &[FxVec2::from_int(310, 290), FxVec2::from_int(310, 330)],
        -1,
    );
    steps(&mut w, 2);
    let m = members(&w, sq);
    let (burn, strike) = (m[1], m[2]);
    let tick = w.tick();
    let cell = dug_cell(&w);
    let wpn = 0u16;
    let s = &mut w.state;
    *close(s, burn) = Close {
        burn_until: tick + 60,
        burn_by: (1, kill_class::FLAME),
        ..Close::default()
    };
    *close(s, strike) = Close {
        strike: 12,
        strike_phase: STRIKE_WINDUP,
        strike_at: FxVec2::from_int(301, 300),
        ..Close::default()
    };
    let g = game_mut(s);
    let p = |x, y, z| FxVec3::new(Fx::from_int(x), Fx::from_int(y), Fx::from_int(z));
    g.flame = vec![
        Flame {
            id: 3,
            pos: p(320, 300, 1),
            vel: p(-25, 0, 1),
            ttl: 40,
            player: 1,
            shooter: SimId(0),
            weapon: wpn,
        },
        Flame {
            id: 5,
            pos: p(318, 301, 1),
            vel: p(-25, 1, 0),
            ttl: 30,
            player: 1,
            shooter: SimId(0),
            weapon: wpn,
        },
    ];
    g.next_flame = 6;
    g.patches = vec![Patch {
        id: 2,
        pos: p(305, 300, 0),
        ends_at: tick + 300,
        player: 1,
        by: (1, kill_class::FLAME),
    }];
    g.next_patch = 4;
    g.clears.insert(
        sq,
        Clear {
            at: FxVec2::from_int(310, 300),
            cell,
            from: cell,
            step: 1,
            phase: 1,
            phase_at: tick - 2,
        },
    );
    g.players[0].s14.flame_hits = 2;
    g.players[1].s14.flame_fired = 7;
    g.players[1].s14.patches_made = 1;
    let k = &mut g.ai[1].as_mut().unwrap().counters;
    k.flame_jobs = 1;
    k.flame_job_ends[4] = 1;
    k.refuels = 2;
    w.hash.mark_entity_chunk(burn.chunk());
    w.hash.mark_entity_chunk(strike.chunk());
    w.refresh_hashes();
    (w, sq, burn, strike)
}

/// Spec 3.1: the S14 state saves, loads and hashes. Control: an unedited
/// reload keeps the hash (each edit must change it).
#[test]
fn s14_layout_roundtrips_and_hashes() {
    let (w, sq, burn, strike) = staged();
    let bytes = w.save();
    let back = World::load(&bytes, 1).expect("the S14 state loads");
    assert_eq!(back.state(), w.state());
    assert_eq!(back.full_state_hash(), w.full_state_hash());
    assert_eq!(back.save(), bytes);
    let h = back.full_state_hash();
    assert_eq!(World::load(&back.save(), 1).unwrap().full_state_hash(), h);
    type Edit = Box<dyn Fn(&mut SimState)>;
    let edits: Vec<Edit> = vec![
        Box::new(move |s| close(s, burn).burn_until += 1),
        Box::new(move |s| close(s, burn).burn_by.1 = kill_class::RIFLE),
        Box::new(move |s| close(s, strike).strike -= 1),
        Box::new(move |s| close(s, strike).strike_phase = STRIKE_RECOVER),
        Box::new(move |s| close(s, strike).strike_at.x += Fx::ONE),
        Box::new(|s| game_mut(s).flame[0].ttl -= 1),
        Box::new(|s| game_mut(s).flame[1].pos.x -= Fx::ONE),
        Box::new(|s| game_mut(s).flame[1].vel.y = Fx::ZERO),
        Box::new(|s| game_mut(s).next_flame += 1),
        Box::new(|s| game_mut(s).patches[0].ends_at += 1),
        Box::new(|s| game_mut(s).patches[0].by.0 = 0),
        Box::new(|s| game_mut(s).next_patch += 1),
        Box::new(move |s| game_mut(s).clears.get_mut(&sq).unwrap().step += 1),
        Box::new(move |s| game_mut(s).clears.get_mut(&sq).unwrap().phase_at -= 1),
        Box::new(|s| game_mut(s).players[0].s14.flame_capped += 1),
        Box::new(|s| game_mut(s).players[1].s14.clears_done += 1),
        Box::new(|s| game_mut(s).players[1].kills_by[kill_class::MELEE as usize] += 1),
        Box::new(|s| game_mut(s).players[1].ammo.fired[crate::supply::fired_class::FLAME] += 1),
        Box::new(|s| ai(s).counters.flame_jobs += 1),
        Box::new(|s| ai(s).counters.flame_job_ends[0] += 1),
        Box::new(|s| ai(s).counters.flame_buys += 1),
        Box::new(|s| ai(s).counters.refuels += 1),
        Box::new(|s| ai(s).counters.clear_orders += 1),
    ];
    for (k, e) in edits.into_iter().enumerate() {
        let mut x = World::load(&bytes, 1).unwrap();
        e(&mut x.state);
        let y = World::load(&x.save(), 1).unwrap_or_else(|e| panic!("edit {k}: {e:?}"));
        assert_ne!(y.full_state_hash(), h, "edit {k} is not hashed");
    }
}

/// Spec 3.1 load validation, one case per check. Control: the staged save
/// itself loads (checked first), so each failure is the edit's.
#[test]
fn load_rejects_bad_s14_state() {
    let (w, sq, burn, strike) = staged();
    let bytes = w.save();
    World::load(&bytes, 1).expect("control: the staged save loads");
    let tick = w.tick();
    let rifle_squad = sq;
    type Edit = Box<dyn Fn(&mut SimState)>;
    let over_cap = |s: &mut SimState| {
        let g = game_mut(s);
        let one = g.flame[1];
        let n = g.s14_limits().particles;
        g.flame = (0..=n as u32)
            .map(|i| Flame { id: 10 + i, ..one })
            .collect();
        g.next_flame = 20 + n as u32;
    };
    let cases: Vec<(&str, Edit)> = vec![
        ("flame over the per-player cap", Box::new(over_cap)),
        (
            "flame ids not ascending",
            Box::new(|s| game_mut(s).flame[1].id = 3),
        ),
        (
            "flame id at next_flame",
            Box::new(|s| game_mut(s).next_flame = 5),
        ),
        ("flame ttl 0", Box::new(|s| game_mut(s).flame[0].ttl = 0)),
        (
            "flame of an unknown player",
            Box::new(|s| game_mut(s).flame[0].player = 9),
        ),
        (
            "flame of an unknown weapon",
            Box::new(|s| game_mut(s).flame[0].weapon = u16::MAX),
        ),
        (
            "flame off the map",
            Box::new(|s| game_mut(s).flame[0].pos.x = Fx::from_int(-1)),
        ),
        (
            "patch ids not ascending",
            Box::new(|s| {
                let g = game_mut(s);
                let p = g.patches[0];
                g.patches.push(p);
            }),
        ),
        (
            "patch id at next_patch",
            Box::new(|s| game_mut(s).next_patch = 2),
        ),
        (
            "patch end in the past",
            Box::new(move |s| game_mut(s).patches[0].ends_at = tick),
        ),
        (
            "patch credit of an unknown class",
            Box::new(|s| game_mut(s).patches[0].by.1 = kill_class::COUNT as u16),
        ),
        (
            "strike phase out of range",
            Box::new(move |s| close(s, strike).strike_phase = 3),
        ),
        (
            "strike phase with no ticks left",
            Box::new(move |s| close(s, strike).strike = 0),
        ),
        (
            "ticks left with no strike phase",
            Box::new(move |s| close(s, burn).strike = 4),
        ),
        (
            "burning with no credit",
            Box::new(move |s| close(s, burn).burn_by = (9, 0)),
        ),
        (
            "credit with no burn",
            Box::new(move |s| close(s, burn).burn_until = 0),
        ),
        (
            "clear cell not dug",
            Box::new(move |s| game_mut(s).clears.get_mut(&rifle_squad).unwrap().cell = (0, 0)),
        ),
        (
            "clear of no squad",
            Box::new(move |s| {
                let g = game_mut(s);
                let c = g.clears[&rifle_squad];
                g.clears.insert(SquadId(9999), c);
            }),
        ),
        (
            "clear phase out of range",
            Box::new(move |s| game_mut(s).clears.get_mut(&rifle_squad).unwrap().phase = 3),
        ),
        (
            "clear phase in the future",
            Box::new(move |s| {
                game_mut(s).clears.get_mut(&rifle_squad).unwrap().phase_at = tick + 1
            }),
        ),
        (
            "s14 counter out of range",
            Box::new(|s| game_mut(s).players[0].s14.burn_kills = u32::MAX),
        ),
    ];
    for (name, e) in cases {
        let mut x = World::load(&bytes, 1).unwrap();
        e(&mut x.state);
        assert!(World::load(&x.save(), 1).is_err(), "{name} loads");
    }
}

/// Plan 2: every S14 key is part of the setup (saved, hashed) and checked
/// against its range. Control: the unedited setup is valid, and each
/// in-range edit is valid too.
#[test]
fn load_rejects_bad_s14_keys() {
    let w = flat_world_with(1, |_| {});
    let base = w.game().unwrap().setup.clone();
    let h0 = postcard::to_allocvec(&base).unwrap();
    assert!(
        base.validate().is_empty(),
        "control: the unedited setup is valid"
    );
    type Edit = Box<dyn Fn(&mut frontline_content::MatchSetup)>;
    let good: Vec<Edit> = vec![
        Box::new(|s| s.rules.flame.enabled = 1),
        Box::new(|s| s.rules.flame.particles_per_player = 32),
        Box::new(|s| s.rules.flame.ttl_ms = 2000),
        Box::new(|s| s.rules.flame.near_cm = 250),
        Box::new(|s| s.rules.flame.burn_s = 5),
        Box::new(|s| s.rules.flame.burn_hp_per_s = 10),
        Box::new(|s| s.rules.flame.pulse_ms = 250),
        Box::new(|s| s.rules.flame.burn_suppression = 30),
        Box::new(|s| s.rules.flame.patch_radius_cm = 150),
        Box::new(|s| s.rules.flame.patch_s = 10),
        Box::new(|s| s.rules.flame.patch_hp_per_s = 0),
        Box::new(|s| s.rules.flame.patch_z_cm = 80),
        Box::new(|s| s.rules.flame.patch_merge_cm = 40),
        Box::new(|s| s.rules.flame.patches_max = 64),
        Box::new(|s| s.rules.flame.step_out = 0),
        Box::new(|s| s.rules.flame.step_out_m = 3),
        Box::new(|s| s.rules.melee.enabled = 1),
        Box::new(|s| s.rules.melee.reach_cm = 100),
        Box::new(|s| s.rules.melee.damage = 50),
        Box::new(|s| s.rules.melee.windup_ms = 600),
        Box::new(|s| s.rules.melee.recover_ms = 900),
        Box::new(|s| s.rules.melee.start_pad_cm = 20),
        Box::new(|s| s.rules.clear.enabled = 1),
        Box::new(|s| s.rules.clear.step_m = 6),
        Box::new(|s| s.rules.clear.steps_max = 4),
        Box::new(|s| s.rules.clear.throwers = 3),
        Box::new(|s| s.rules.clear.settle_s = 0),
        Box::new(|s| s.rules.clear.walk_cells = 32),
        Box::new(|s| s.rules.ai.flame.jobs = 1),
        Box::new(|s| s.rules.ai.flame.refuel = 1),
        Box::new(|s| s.rules.ai.flame.buy = 1),
        Box::new(|s| s.rules.ai.flame.clear = 1),
        Box::new(|s| s.rules.ai.flame.target_m = 30),
        Box::new(|s| s.rules.ai.flame.stage_m = 30),
        Box::new(|s| s.rules.ai.flame.stand_m = 15),
        Box::new(|s| s.rules.ai.flame.reserve_pct = 20),
        Box::new(|s| s.rules.ai.flame.job_max_s = 120),
        Box::new(|s| s.rules.ai.flame.stall_s = 30),
        Box::new(|s| s.rules.ai.flame.cards_max = 1),
        Box::new(|s| s.rules.ai.flame.clear_near_m = 10),
        Box::new(|s| s.rules.ai.flame.recheck_s = 30),
    ];
    for (k, e) in good.into_iter().enumerate() {
        let mut s = base.clone();
        e(&mut s);
        assert_ne!(
            postcard::to_allocvec(&s).unwrap(),
            h0,
            "edit {k} is not in the setup bytes"
        );
        assert!(s.validate().is_empty(), "edit {k}: {:?}", s.validate());
    }
    let bad: Vec<(&str, Edit)> = vec![
        ("flame enabled 2", Box::new(|s| s.rules.flame.enabled = 2)),
        (
            "particles_per_player 0",
            Box::new(|s| s.rules.flame.particles_per_player = 0),
        ),
        (
            "particles_per_player 257",
            Box::new(|s| s.rules.flame.particles_per_player = 257),
        ),
        ("ttl_ms 10001", Box::new(|s| s.rules.flame.ttl_ms = 10_001)),
        ("near_cm 0", Box::new(|s| s.rules.flame.near_cm = 0)),
        ("burn_s 0", Box::new(|s| s.rules.flame.burn_s = 0)),
        (
            "burn_hp_per_s 1001",
            Box::new(|s| s.rules.flame.burn_hp_per_s = 1001),
        ),
        ("pulse_ms 49", Box::new(|s| s.rules.flame.pulse_ms = 49)),
        (
            "burn_suppression 101",
            Box::new(|s| s.rules.flame.burn_suppression = 101),
        ),
        (
            "patch_radius_cm 1001",
            Box::new(|s| s.rules.flame.patch_radius_cm = 1001),
        ),
        ("patch_s 0", Box::new(|s| s.rules.flame.patch_s = 0)),
        ("patch_z_cm 0", Box::new(|s| s.rules.flame.patch_z_cm = 0)),
        (
            "patch_merge_cm 0",
            Box::new(|s| s.rules.flame.patch_merge_cm = 0),
        ),
        (
            "patches_max 257",
            Box::new(|s| s.rules.flame.patches_max = 257),
        ),
        ("step_out 2", Box::new(|s| s.rules.flame.step_out = 2)),
        ("step_out_m 51", Box::new(|s| s.rules.flame.step_out_m = 51)),
        ("melee enabled 2", Box::new(|s| s.rules.melee.enabled = 2)),
        ("reach_cm 0", Box::new(|s| s.rules.melee.reach_cm = 0)),
        (
            "windup_ms 4001",
            Box::new(|s| s.rules.melee.windup_ms = 4001),
        ),
        ("recover_ms 0", Box::new(|s| s.rules.melee.recover_ms = 0)),
        ("clear enabled 2", Box::new(|s| s.rules.clear.enabled = 2)),
        ("steps_max 51", Box::new(|s| s.rules.clear.steps_max = 51)),
        ("throwers 0", Box::new(|s| s.rules.clear.throwers = 0)),
        (
            "walk_cells 1025",
            Box::new(|s| s.rules.clear.walk_cells = 1025),
        ),
        ("ai.flame jobs 2", Box::new(|s| s.rules.ai.flame.jobs = 2)),
        (
            "ai.flame reserve_pct 101",
            Box::new(|s| s.rules.ai.flame.reserve_pct = 101),
        ),
        (
            "ai.flame cards_max 9",
            Box::new(|s| s.rules.ai.flame.cards_max = 9),
        ),
        (
            "ai.flame stall_s 0",
            Box::new(|s| s.rules.ai.flame.stall_s = 0),
        ),
    ];
    for (name, e) in bad {
        let mut s = base.clone();
        e(&mut s);
        assert!(!s.validate().is_empty(), "{name} is accepted");
    }
}

// ------------------------------------------------------- F1-F9 rules --

/// A test-made flamethrower (Build B adds the real ones): the German
/// Grenadiergruppe's MG 42 man carries a copy of the MG 42 turned into a
/// Flame-class weapon (30 m, 25 m/s, 20 damage, 35 suppression, a 60-unit
/// tank on the MG's feed, no set-up time). Returns the weapon index.
fn make_flamer(s: &mut frontline_content::MatchSetup) -> u16 {
    let t = s.squad_type_index(RIFLES).unwrap() as usize;
    let mg = s.squad_types[t].members[1].weapon as usize;
    let mut w = s.weapons[mg].clone();
    w.id = "test:flamer".into();
    w.class = frontline_content::WeaponClass::Flame;
    w.range_max = frontline_content::Fixed::from_int(30);
    w.muzzle_speed = frontline_content::Fixed::from_int(25);
    w.damage = 20;
    w.suppression = 35;
    w.magazine = 60;
    w.setup_ticks = 0;
    w.hold_beyond_pct = 100;
    w.pen.clear();
    w.core = None;
    w.engage_still = w.engage_still.min(w.range_max);
    w.engage_moving = w.engage_moving.min(w.range_max);
    s.weapons.push(w);
    let wi = (s.weapons.len() - 1) as u16;
    s.squad_types[t].members[1].weapon = wi;
    wi
}

/// A flat match with the test flamer squad of player 0 at (300, 300) and
/// a US rifle squad of player 1 `gap` m east, `[flame] enabled` = `on`
/// and `edit` applied. Returns `(world, flamer squad, the flame man,
/// flame weapon)`.
fn flame_scene(
    on: u32,
    gap: i32,
    edit: impl FnOnce(&mut frontline_content::MatchSetup),
) -> (World, SquadId, SimId, u16) {
    let mut wi = 0;
    let mut w = flat_world_with(1, |s| {
        s.rules.flame.enabled = on;
        wi = make_flamer(s);
        edit(s);
    });
    let sq = spawn(&mut w, 0, RIFLES, 300, 300);
    if gap > 0 {
        spawn(&mut w, 1, US_RIFLES, 300 + gap, 300);
    }
    let man = members(&w, sq)[1];
    (w, sq, man, wi)
}

fn s14_of(w: &World, p: usize) -> crate::flame::S14Counters {
    w.game().unwrap().players[p].s14.clone()
}

/// F1: with `[flame] enabled` 0 the flamethrower holds fire (no particle,
/// no pool round from him); with 1 he fires particles and never a pool
/// round. Control: the same scene with the switch on fires.
#[test]
fn flame_fires_particles_only_when_on() {
    for on in [0, 1] {
        let (mut w, _sq, man, wi) = flame_scene(on, 14, |_| {});
        let mut pool_rounds = 0;
        let mut peak = 0;
        for _ in 0..240 {
            w.step(&[]);
            pool_rounds += w
                .state
                .projectiles
                .slots()
                .iter()
                .flatten()
                .filter(|p| p.weapon == wi)
                .count();
            peak = peak.max(w.game().unwrap().flame().len());
        }
        assert_eq!(pool_rounds, 0, "on {on}: a flame round in the pool");
        let fired = s14_of(&w, 0).flame_fired;
        if on == 0 {
            assert_eq!((fired, peak), (0, 0));
            assert!(w.state.entities.combat[man.index()].target.is_none());
        } else {
            assert!(
                fired > 0 && peak > 0,
                "control: on 1 fires ({fired}, {peak})"
            );
            assert_eq!(
                w.game().unwrap().players[0].ammo.fired[crate::supply::fired_class::FLAME],
                fired
            );
        }
    }
}

/// F1, F9: one request over the per-player cap is dropped and counted in
/// `flame_capped`, never in `shots_dropped`; live particles never exceed
/// the cap. Control: the default cap (64) drops none in the same scene.
#[test]
fn flame_cap_counts_capped() {
    for cap in [4, 64] {
        let (mut w, ..) = flame_scene(1, 14, |s| s.rules.flame.particles_per_player = cap);
        let dropped0 = w.state.stats.shots_dropped;
        for _ in 0..240 {
            w.step(&[]);
            assert!(w.game().unwrap().flame().len() <= cap as usize);
        }
        let k = s14_of(&w, 0);
        assert_eq!(
            w.state.stats.shots_dropped, dropped0,
            "cap {cap}: pool drops"
        );
        if cap == 4 {
            assert!(k.flame_capped > 0, "the cap of 4 drops requests");
        } else {
            assert_eq!(k.flame_capped, 0, "control: the default cap drops none");
        }
    }
}

/// F2-F4, F8: particles strike enemy men in the open: each struck man
/// burns (refreshed per hit), takes damage credited to class FLAME, and
/// near misses suppress. Control: the switch off leaves them unhurt.
#[test]
fn flame_hits_burn_and_credit_flame() {
    for on in [0, 1] {
        let (mut w, ..) = flame_scene(on, 12, |s| {
            // The enemy holds fire: the flame alone hurts him.
            let _ = s;
        });
        let enemy: Vec<SimId> = w
            .game()
            .unwrap()
            .squads
            .values()
            .filter(|q| q.owner.0 == 1)
            .flat_map(|q| q.members.clone())
            .collect();
        let mut burned = false;
        for _ in 0..600 {
            w.step(&[]);
            burned |= enemy
                .iter()
                .any(|m| w.state.entities.combat[m.index()].close.burn_until > 0);
        }
        let g = w.game().unwrap();
        let k = &g.players[0];
        let flame_out = k.kills_by[kill_class::FLAME as usize]
            + k.s12.downed_by_class[kill_class::FLAME as usize];
        if on == 0 {
            assert!(!burned && flame_out == 0 && k.s14.flame_hits == 0);
        } else {
            assert!(k.s14.flame_hits > 0, "particles struck");
            assert!(burned, "a struck man burns");
            assert!(flame_out > 0, "men out to class FLAME");
            assert!(k.s14.patches_made > 0, "the stream lands and burns");
        }
    }
}

/// Risk 7 (S4 crew-weapon pickup): the flame man falls with fuel in his
/// tank; the crew assistant takes the flamethrower with exactly that
/// fuel. Control: the fuel he had is not the full tank (a fresh tank would
/// pass a weaker check).
#[test]
fn flame_handover_keeps_fuel() {
    let (mut w, sq, man, wi) = flame_scene(1, 0, |_| {});
    steps(&mut w, 3);
    assert!(w.edit_soldier(man, |_, c| c.mag = 23));
    let assistant = members(&w, sq)[2];
    assert!(w.kill_soldier(man));
    let mut took = false;
    for _ in 0..600 {
        w.step(&[]);
        if w.state.entities.kit[assistant.index()].weapon == wi {
            took = true;
            break;
        }
    }
    assert!(took, "the assistant takes the flamethrower");
    assert_eq!(
        w.state.entities.combat[assistant.index()].mag,
        23,
        "his fuel is the fallen man's"
    );
}

/// F5: a landing within `patch_merge_cm` of a patch refreshes it (keeping
/// the older id), one farther makes a new one, and one at `patches_max`
/// is dropped and counted. Control: with room, the far landing is kept.
#[test]
fn patches_merge_refresh_and_cap() {
    for max in [1u32, 8] {
        let (mut w, ..) = flame_scene(1, 0, |s| s.rules.flame.patches_max = max);
        steps(&mut w, 2);
        let tick = w.tick();
        let p = |x: i32| FxVec3::new(Fx::from_int(x), Fx::from_int(500), Fx::ZERO);
        let g = game_mut(&mut w.state);
        g.next_patch = 5;
        g.patches = vec![Patch {
            id: 4,
            pos: p(500),
            ends_at: tick + 30,
            player: 0,
            by: (0, kill_class::FLAME),
        }];
        // Two particles about to land: one on the patch, one 10 m away.
        let fl = |id, x| Flame {
            id,
            pos: FxVec3::new(Fx::from_int(x), Fx::from_int(500), Fx::from_milli(100)),
            vel: FxVec3::new(Fx::ZERO, Fx::ZERO, Fx::from_int(-10)),
            ttl: 60,
            player: 0,
            shooter: SimId(0),
            weapon: 0,
        };
        g.flame = vec![fl(0, 500), fl(1, 510)];
        g.next_flame = 2;
        w.step(&[]);
        let g = w.game().unwrap();
        assert!(g.flame().is_empty(), "both landed");
        let ps = g.patches();
        assert_eq!(ps[0].id, 4, "the older patch is kept");
        assert!(ps[0].ends_at > tick + 30, "and refreshed");
        if max == 1 {
            assert_eq!(ps.len(), 1);
            assert_eq!(g.players[0].s14.patches_dropped, 1, "one over the cap");
        } else {
            assert_eq!(ps.len(), 2, "control: with room the far landing burns");
            assert_eq!(ps[1].id, 5);
        }
    }
}

/// F5 (S14-1 (A)): burning ground burns every man in it, own men too,
/// credited to the patch (own burns are own fire, not kills). Control:
/// `patch_hp_per_s` and `burn_hp_per_s` 0 leave him unhurt.
#[test]
fn patch_burns_any_owner() {
    for hp in [0u32, 20] {
        let (mut w, sq, ..) = flame_scene(1, 0, |s| {
            // The patch also sets him burning: the control turns both off.
            (s.rules.flame.patch_hp_per_s, s.rules.flame.burn_hp_per_s) = (hp, hp);
            // He stays in it (F6 is its own test).
            s.rules.flame.step_out = 0;
        });
        steps(&mut w, 2);
        let victim = members(&w, sq)[5];
        let at = w.state.entities.transform[victim.index()].pos;
        let tick = w.tick();
        let g = game_mut(&mut w.state);
        g.patches = vec![Patch {
            id: 0,
            pos: at,
            ends_at: tick + 300,
            player: 0,
            by: (0, kill_class::FLAME),
        }];
        g.next_patch = 1;
        let hp0 = w.state.entities.combat[victim.index()].hp;
        for _ in 0..60 {
            w.step(&[]);
        }
        let c = &w.state.entities.combat[victim.index()];
        if hp == 0 {
            assert_eq!(c.hp, hp0, "control: a harmless patch");
        } else {
            assert!(c.hp < hp0, "his own patch burns him");
            assert_eq!(s14_of(&w, 0).patch_hits, 0, "own burns are not hits");
        }
    }
}

/// F9 (RE done-when 2, plan 3.2): a pool saturated by staged rifle fire
/// (40 rounds a tick) and three mortar bombs; two flame men `SuppressUnits`
/// at empty ground 20 m in front of them (inside the flamethrower's 30 m),
/// 50 m from the pool fire. With `[flame] enabled` 1 they fire particles,
/// with 0 they hold; pool drops, staged rounds refused and every pool slot
/// (every shell among them) are equal tick for tick, and live particles
/// stay within the cap. Control: the run with the switch on does fire.
#[test]
fn flame_never_takes_pool_slots() {
    let scene = |on| {
        let (mut w, sq, man, wi) = flame_scene(on, 0, |s| {
            s.rules.projectile_cap = 72;
            let f = s.weapons.last_mut().unwrap();
            // No reveal: their fire must not change what the enemy sees.
            (f.fire_reveal_pct, f.fire_reveal_ticks) = (100, 0);
            let t = s.squad_type_index(RIFLES).unwrap() as usize;
            let w = s.weapons.len() as u16 - 1;
            s.squad_types[t].members[3].weapon = w;
        });
        let second = members(&w, sq)[3];
        assert_eq!(w.state.entities.kit[second.index()].weapon, wi);
        let line = vec![FxVec2::from_int(300, 320), FxVec2::from_int(304, 320)];
        super::s6_tests::run(
            &mut w,
            0,
            0,
            CommandKind::SuppressUnits {
                units: vec![man, second],
                line,
            },
        );
        w
    };
    let setup = scene(1).match_setup().unwrap().clone();
    let rifle = setup
        .weapons
        .iter()
        .position(|x| x.id == "ww2:kar98k")
        .unwrap() as u16;
    let mortar = setup
        .weapons
        .iter()
        .position(|x| x.id == "ww2:grw34")
        .unwrap() as u16;
    let (mut on, mut off) = (scene(1), scene(0));
    let mut peak = 0;
    let mut refused = [0u32; 2];
    for t in 0..300u64 {
        for (r, w) in [&mut on, &mut off].into_iter().enumerate() {
            for k in 0..40 {
                // Pool fire 50 m beyond the flame point (300, 320).
                let a = FxVec2::from_int(250 + k, 370);
                let z = w.terrain().height_at(a) + Fx::from_int(2);
                let to = FxVec2::from_int(250 + k, 600).extend(z);
                if !w.fire_round_now(0, rifle, a.extend(z), to) {
                    refused[r] += 1;
                }
            }
            if t % 60 == 0 && t < 180 {
                // A full pool refuses a bomb as it refuses a round.
                if !w.drop_shell_now(0, mortar, FxVec2::from_int(300, 420)) {
                    refused[r] += 1000;
                }
            }
            w.step(&[]);
        }
        assert_eq!(
            on.state.stats.shots_dropped, off.state.stats.shots_dropped,
            "tick {t}: drops"
        );
        assert_eq!(refused[0], refused[1], "tick {t}: staged rounds refused");
        assert!(
            on.state.projectiles.slots() == off.state.projectiles.slots(),
            "tick {t}: pool"
        );
        peak = peak.max(on.game().unwrap().flame().len());
    }
    assert!(refused[0] > 0, "the pool is saturated");
    assert!(s14_of(&on, 0).flame_fired > 0, "control: they fire");
    let fl: std::collections::BTreeSet<SimId> = on
        .game()
        .unwrap()
        .flame()
        .iter()
        .map(|p| p.shooter)
        .collect();
    assert!(fl.len() <= 2);
    assert!(peak <= 64);
}

/// F1, F9 (RE done-when 2): two flame men of one player stay within the
/// per-player cap together, the excess counted in `flame_capped`.
/// Control: each of them fires (the cap binds two, not one).
#[test]
fn two_flamers_stay_in_cap() {
    let (mut w, sq, man, wi) = flame_scene(1, 14, |s| {
        s.rules.flame.particles_per_player = 6;
        let t = s.squad_type_index(RIFLES).unwrap() as usize;
        let w = s.weapons.len() as u16 - 1;
        s.squad_types[t].members[3].weapon = w;
    });
    let second = members(&w, sq)[3];
    assert_eq!(w.state.entities.kit[second.index()].weapon, wi);
    let mut fired = [false; 2];
    for _ in 0..240 {
        w.step(&[]);
        let fl = w.game().unwrap().flame();
        assert!(fl.len() <= 6, "live {} over the cap", fl.len());
        for p in fl {
            fired[0] |= p.shooter == man;
            fired[1] |= p.shooter == second;
        }
    }
    assert_eq!(fired, [true, true], "control: both fire");
    assert!(s14_of(&w, 0).flame_capped > 0);
}

// ------------------------------------------------------- F2 slit, F6 --

/// Particles of player 1 thrown down through the bunker's slit (S10b
/// layout: player 0's bunker at (500, 500) facing +x, five men in the pit,
/// their heads under the slit's bottom) from 0.4 m in front of the front
/// walls, 30-36 cm over the slit's bottom and across its width, flying at
/// (-12, 0, -5) m/s; `[flame] enabled` 1. Returns (player 1's S14 counters, men of
/// the five hurt). `closed`: the front walls meet (no slit).
fn slit_flame(closed: bool) -> (crate::flame::S14Counters, usize) {
    let mut w = super::s10b_tests::bunker_world(1, |d| {
        if closed {
            for k in [4, 5] {
                d.pieces[k].half[1] = frontline_content::setup::Fixed::from_ratio(185, 200);
                d.pieces[k].offset[1] = frontline_content::setup::Fixed::from_ratio(
                    if k == 4 { -925 } else { 925 },
                    1000,
                );
            }
        }
    });
    game_mut(&mut w.state).setup.rules.flame.enabled = 1;
    let (_, men) = super::s10b_tests::bunker_at(&mut w, true);
    let before: Vec<u16> = men
        .iter()
        .map(|m| w.state.entities.combat[m.index()].hp)
        .collect();
    let bottom = super::s10b_tests::fs(&w)
        .pieces
        .values()
        .find(|q| q.part == 4)
        .unwrap()
        .z;
    let mut parts = Vec::new();
    for (k, (dz, dy)) in [
        (300, -250),
        (320, 0),
        (340, 250),
        (360, -100),
        (310, 100),
        (330, 0),
    ]
    .into_iter()
    .enumerate()
    {
        parts.push(Flame {
            id: k as u32,
            pos: FxVec3::new(
                Fx::from_int(502) + Fx::from_milli(300),
                Fx::from_int(500) + Fx::from_milli(dy),
                bottom + Fx::from_milli(dz),
            ),
            vel: FxVec3::new(Fx::from_int(-12), Fx::ZERO, Fx::from_int(-5)),
            ttl: 60,
            player: 1,
            shooter: SimId(u32::MAX - 1),
            weapon: 0,
        });
    }
    assert!(w.stage_s14(parts, Vec::new()));
    super::s10b_tests::hold(&mut w, &men, 60);
    let hurt = men
        .iter()
        .zip(&before)
        .filter(|(m, b)| {
            w.state.entities.kind[m.index()] != crate::ecs::EntityKind::Soldier
                || w.state.entities.combat[m.index()].hp < **b
        })
        .count();
    (s14_of(&w, 1), hurt)
}

/// F2 (RE done-when 1, the bunker falls by geometry): particles through
/// the slit gap strike the men inside (counted in `flame_in_bunker_hits`)
/// or burn on the pit floor. Control: with the slit closed the front walls
/// stop every particle: no hit, no man hurt.
#[test]
fn flame_through_the_slit_hits_men_inside() {
    let (k, hurt) = slit_flame(false);
    assert!(
        k.flame_hits > 0 && hurt > 0,
        "through the slit: {k:?}, {hurt} hurt"
    );
    assert_eq!(
        k.flame_in_bunker_hits, k.flame_hits,
        "every hit is under the roof"
    );
    let (k, hurt) = slit_flame(true);
    assert_eq!((k.flame_hits, hurt), (0, 0), "control: closed slit");
}

/// F6: with `step_out` 1, men in formation whose feet are in burning
/// ground take a detached move out of it and leave; with 0 they stay.
/// Control: `step_out` 0.
#[test]
fn men_step_out_of_burning_ground() {
    for so in [0u32, 1] {
        let (mut w, sq, ..) = flame_scene(1, 0, |s| {
            s.rules.flame.step_out = so;
            (s.rules.flame.patch_hp_per_s, s.rules.flame.burn_hp_per_s) = (0, 0);
        });
        steps(&mut w, 2);
        let m = members(&w, sq)[5];
        let at = w.state.entities.transform[m.index()].pos;
        let tick = w.tick();
        let g = game_mut(&mut w.state);
        g.patches = vec![Patch {
            id: 0,
            pos: at,
            ends_at: tick + 900,
            player: 1,
            by: (1, kill_class::FLAME),
        }];
        g.next_patch = 1;
        steps(&mut w, 150);
        let now = w.state.entities.transform[m.index()].pos;
        let away = (now.xy() - at.xy()).length();
        let stepped = s14_of(&w, 0).stepped_out;
        if so == 0 {
            assert_eq!(stepped, 0, "control: no step-out");
            assert!(away < Fx::ONE, "he stays ({away:?})");
        } else {
            assert!(stepped > 0 && w.state.entities.unit[m.index()].detached);
            assert!(away > Fx::from_int(2), "he left the patch ({away:?})");
        }
    }
}

/// F6: a man with a move order of his own (detached, moving) and a gun
/// crew or rider are left alone; only men in formation or idle step out.
/// Control: the same man idle steps out (previous test).
#[test]
fn moving_man_does_not_step_out() {
    let (mut w, sq, ..) = flame_scene(1, 0, |s| {
        (s.rules.flame.patch_hp_per_s, s.rules.flame.burn_hp_per_s) = (0, 0);
    });
    steps(&mut w, 2);
    let m = members(&w, sq)[5];
    let at = w.state.entities.transform[m.index()].pos;
    super::s6_tests::run(
        &mut w,
        0,
        0,
        CommandKind::MoveUnits {
            units: vec![m],
            target: FxVec2::from_int(200, 300),
        },
    );
    let tick = w.tick();
    let g = game_mut(&mut w.state);
    g.patches = vec![Patch {
        id: 0,
        pos: at,
        ends_at: tick + 900,
        player: 1,
        by: (1, kill_class::FLAME),
    }];
    g.next_patch = 1;
    steps(&mut w, 30);
    assert_eq!(s14_of(&w, 0).stepped_out, 0, "his own move is kept");
}

// ------------------------------------------------------------- F7 fuel --

/// A test-made fuel cartridge (supply kind explosives, `g` grams a unit)
/// as the test flamer's feed: a new feed slot of the flamer squad type,
/// the flame man's load one tank (`magazine`). Returns the feed slot.
/// Build B replaces it with the mod's fuel cartridges.
fn fuel_cartridge(s: &mut frontline_content::MatchSetup, wi: u16, g: u32) -> usize {
    use frontline_content::setup::{CartridgeSetup, Feed};
    s.cartridges.push(CartridgeSetup {
        id: "test:fuel".into(),
        name: "test fuel".into(),
        kind: frontline_content::SupplyKind::Explosives,
        g,
    });
    let feed = Feed::Cartridge((s.cartridges.len() - 1) as u16);
    let w = &mut s.weapons[wi as usize];
    w.feed = Some(feed);
    let tank = w.magazine as u16;
    let t = s.squad_type_index(RIFLES).unwrap() as usize;
    let ty = &mut s.squad_types[t];
    let k = ty.feeds.len();
    assert!(k < frontline_content::setup::MAX_FEEDS, "a free feed slot");
    ty.feeds.push(feed);
    ty.members[1].rounds[k] = tank;
    ty.feed_cap[k] = ty.feed_cap[k].max(tank);
    k
}

/// F7: fuel is an explosives-kind cartridge, so the S5a base refill serves
/// it with no flame code: a dry flame man in his own base gets fuel back,
/// each unit paid at `explosives_mp_per_t` for its grams (`paid` and
/// `refilled` of the explosives kind grow together). Control: the same dry
/// man 150 m out of his base gets none and pays nothing. Seen: the base
/// serves it at the bomb cadence `max(base_bomb_s * hz, 1)`, so with
/// `base_bomb_s` 0 one unit a tick (a 60-unit tank in 2 s), not the
/// plan's one unit per 2 s (S14-log, for Build B and the owner).
#[test]
fn flame_fuel_resupplies_as_explosives() {
    let g = 200;
    for home in [false, true] {
        let (mut wi, mut k) = (0, 0);
        let mut w = flat_world_with(1, |s| {
            wi = make_flamer(s);
            k = fuel_cartridge(s, wi, g);
        });
        let b = w.game().unwrap().base_of(PlayerId(0));
        let at = if home {
            b
        } else {
            b + FxVec2::from_int(150, 0)
        };
        let sq = spawn(&mut w, 0, RIFLES, at.x.floor_int(), at.y.floor_int());
        let man = members(&w, sq)[1];
        assert_eq!(w.state.entities.kit[man.index()].weapon, wi);
        assert!(w.edit_soldier(man, |kit, c| {
            kit.rounds[k] = 0;
            c.mag = 0;
        }));
        let ex = frontline_content::SupplyKind::Explosives as usize;
        let a0 = w.game().unwrap().players[0].ammo;
        steps(&mut w, 4 * super::s6_tests::HZ);
        let a1 = &w.game().unwrap().players[0].ammo;
        let fuel = w.state.entities.kit[man.index()].rounds[k] as u32
            + w.state.entities.combat[man.index()].mag as u32;
        let units = a1.refilled[ex] - a0.refilled[ex];
        let paid = a1.paid[ex] - a0.paid[ex];
        let price = crate::supply::micro_price(
            w.match_setup().unwrap(),
            frontline_content::SupplyKind::Explosives,
            g,
        );
        if home {
            assert!(
                fuel > 0 && units >= fuel,
                "fuel back in base ({fuel}, {units} units)"
            );
            assert!(price > 0);
            assert_eq!(
                paid,
                units as u64 * price,
                "each unit at the explosives price"
            );
        } else {
            assert_eq!(
                (fuel, units, paid),
                (0, 0, 0),
                "control: no source out of base"
            );
        }
    }
}

// ------------------------------------------------------------ snapshot --

/// Step 8: the snapshot lists every live particle and patch (ids,
/// positions, players, patch ticks left) and flags burning men. Control:
/// `[flame] enabled` 0 lists none and flags no one.
#[test]
fn snapshot_shows_flame_patches_and_burning() {
    for on in [0u32, 1] {
        let (mut w, ..) = flame_scene(on, 14, |_| {});
        let (mut parts, mut burning) = (0, 0);
        for _ in 0..240 {
            w.step(&[]);
            let g = w.game().unwrap();
            let s = w.snapshot();
            assert_eq!(s.flame.len(), g.flame().len());
            for (a, b) in s.flame.iter().zip(g.flame()) {
                assert_eq!((a.id, a.pos, a.player), (b.id, b.pos.to_f32(), b.player));
            }
            assert_eq!(s.patches.len(), g.patches().len());
            for (a, b) in s.patches.iter().zip(g.patches()) {
                assert_eq!((a.id, a.player), (b.id, b.player));
                assert_eq!(a.ticks_left as u64, b.ends_at - w.tick());
            }
            assert_eq!(s.soldier_burning.len(), s.soldiers.len());
            for (k, id) in s.soldier_ids.iter().enumerate() {
                let c = &w.state.entities.combat[id.index()];
                assert_eq!(s.soldier_burning[k], c.close.burn_until > w.tick());
            }
            parts += s.flame.len();
            burning += s.soldier_burning.iter().filter(|b| **b).count();
        }
        if on == 0 {
            assert_eq!((parts, burning), (0, 0), "control: switch off");
        } else {
            assert!(parts > 0 && burning > 0, "flame on: {parts} {burning}");
        }
    }
}

// ------------------------------------------- Build B: checker fixes --

/// Player 0's man `m` of `w` set burning by player 1 (FLAME) for `ticks`.
fn set_burning(w: &mut World, m: SimId, ticks: u64) {
    let tick = w.tick();
    *close(&mut w.state, m) = Close {
        burn_until: tick + ticks,
        burn_by: (1, kill_class::FLAME),
        ..Close::default()
    };
    w.hash.mark_entity_chunk(m.chunk());
    w.refresh_hashes();
}

/// A patch of player `by` at `at`, burning `ticks`.
fn put_patch(w: &mut World, at: FxVec3, by: u8, ticks: u64) {
    let tick = w.tick();
    let g = game_mut(&mut w.state);
    let id = g.next_patch;
    g.patches.push(Patch {
        id,
        pos: at,
        ends_at: tick + ticks,
        player: by,
        by: (by, kill_class::FLAME),
    });
    g.next_patch = id + 1;
}

/// F3 (checker 2): a man downed by his burn keeps burning, and the next
/// pulse kills him (W3), credited to FLAME as a finish. Control: before
/// the fix `down()` reset `Close`, so he lay there unburnt.
#[test]
fn downed_man_keeps_burning() {
    let (mut w, sq, ..) = flame_scene(1, 0, |s| {
        s.rules.wounds.enabled = 1;
        s.rules.flame.step_out = 0;
    });
    steps(&mut w, 2);
    let m = members(&w, sq)[5];
    assert!(w.edit_soldier(m, |_, c| c.hp = 10));
    set_burning(&mut w, m, 900);
    let mut downed_at = None;
    for _ in 0..300 {
        w.step(&[]);
        let k = w.state.entities.kind[m.index()];
        if k == crate::ecs::EntityKind::Downed && downed_at.is_none() {
            downed_at = Some(w.tick());
            assert!(
                w.state.entities.combat[m.index()].close.burn_until > w.tick(),
                "a downed man keeps burning"
            );
        }
        if k == crate::ecs::EntityKind::Corpse {
            break;
        }
    }
    let at = downed_at.expect("his burn downs him");
    assert_eq!(
        w.state.entities.kind[m.index()],
        crate::ecs::EntityKind::Corpse,
        "the next pulse kills him"
    );
    let pulse = 500 * super::s6_tests::HZ / 1000;
    assert!(w.tick() <= at + pulse + 1, "within a pulse");
    let p1 = &w.game().unwrap().players[1];
    assert_eq!(p1.s12.finished_by_class[kill_class::FLAME as usize], 1);
    assert_eq!(p1.s14.burn_downs, 1);
}

/// F5, F8 (S14-1 (A)): an own patch that kills an own man is own fire, not
/// a kill (no `kills_by`, no `burn_kills`, no `patch_hits`; S11a2
/// `ff_kills`). Control: an enemy patch kills him as a FLAME kill.
#[test]
fn own_patch_burn_is_not_a_kill() {
    for by in [0u8, 1] {
        let (mut w, sq, ..) = flame_scene(1, 0, |s| {
            s.rules.flame.step_out = 0;
            s.rules.flame.patch_hp_per_s = 1000;
        });
        steps(&mut w, 2);
        let m = members(&w, sq)[5];
        let at = w.state.entities.transform[m.index()].pos;
        put_patch(&mut w, at, by, 300);
        steps(&mut w, 40);
        assert_ne!(
            w.state.entities.kind[m.index()],
            crate::ecs::EntityKind::Soldier,
            "by {by}: the patch kills him"
        );
        let g = w.game().unwrap();
        let p = &g.players[by as usize];
        if by == 0 {
            assert_eq!(p.kills_by[kill_class::FLAME as usize], 0, "no own kill");
            assert_eq!((p.s14.burn_kills, p.s14.patch_hits), (0, 0));
            assert!(p.s11a2.ff_kills.iter().sum::<u32>() > 0, "own fire");
        } else {
            assert_eq!(p.kills_by[kill_class::FLAME as usize], 1, "control");
            assert_eq!(p.s14.burn_kills, 1);
            assert!(p.s14.patch_hits > 0);
        }
    }
}

/// F4: a particle passing 2 m beside a man (inside `near_cm` 300, outside
/// his capsule) suppresses him. Control: `near_cm` 100 does not.
#[test]
fn near_miss_particle_suppresses() {
    for near in [300u32, 100] {
        let (mut w, sq, ..) = flame_scene(1, 0, |s| {
            s.rules.flame.near_cm = near;
            (s.rules.flame.patch_hp_per_s, s.rules.flame.burn_hp_per_s) = (0, 0);
            s.rules.flame.burn_suppression = 0;
        });
        steps(&mut w, 2);
        let m = members(&w, sq)[5];
        let at = w.state.entities.transform[m.index()].pos;
        let start = FxVec3::new(
            at.x - Fx::from_int(6),
            at.y + Fx::from_int(2),
            at.z + Fx::ONE,
        );
        assert!(w.stage_s14(
            vec![Flame {
                id: 0,
                pos: start,
                vel: FxVec3::new(Fx::from_int(25), Fx::ZERO, Fx::ZERO),
                ttl: 60,
                player: 1,
                shooter: SimId(u32::MAX - 1),
                weapon: 0,
            }],
            Vec::new(),
        ));
        let mut peak = 0;
        for _ in 0..30 {
            w.step(&[]);
            peak = peak.max(w.state.entities.combat[m.index()].suppression);
        }
        assert_eq!(s14_of(&w, 1).flame_hits, 0, "near {near}: no hit");
        if near == 300 {
            assert!(peak > 0, "the near miss suppresses");
        } else {
            assert_eq!(peak, 0, "control: 2 m is beyond 1 m");
        }
    }
}

/// F2, F5 (checker 5): a particle landing on a static box's top (the
/// bunker's roof) burns there, on the roof. Control: one landing on open
/// ground burns at the ground's height.
#[test]
fn flame_on_a_box_top_makes_a_patch() {
    let mut w = super::s10b_tests::bunker_world(1, |_| {});
    super::s10b_tests::bunker_at(&mut w, true);
    let top = w
        .game()
        .unwrap()
        .static_boxes()
        .iter()
        .max_by_key(|b| (b.frame.center.z + b.frame.half.z).raw())
        .map(|b| (b.frame.center, b.frame.center.z + b.frame.half.z))
        .unwrap();
    for open in [false, true] {
        let mut x = World::load(&w.save(), 1).unwrap();
        let xy = if open {
            FxVec2::from_int(530, 530)
        } else {
            top.0.xy()
        };
        let z = top.1 + Fx::from_milli(500);
        assert!(x.stage_s14(
            vec![Flame {
                id: 0,
                pos: FxVec3::new(xy.x, xy.y, z),
                vel: FxVec3::new(Fx::ZERO, Fx::ZERO, Fx::from_int(-10)),
                ttl: 60,
                player: 1,
                shooter: SimId(u32::MAX - 1),
                weapon: 0,
            }],
            Vec::new(),
        ));
        steps(&mut x, 20);
        let g = x.game().unwrap();
        assert!(g.flame().is_empty(), "open {open}: it landed");
        let ps = g.patches();
        assert_eq!(ps.len(), 1, "open {open}: one patch");
        let ground = x.terrain().height_at(xy);
        if open {
            assert!((ps[0].pos.z - ground).abs() < Fx::from_milli(100));
        } else {
            assert!(
                (ps[0].pos.z - top.1).abs() < Fx::from_milli(100),
                "on the roof ({:?} vs {:?})",
                ps[0].pos.z,
                top.1
            );
        }
    }
}

/// F7 (checker 4): the own base refills fuel one unit per `[flame]
/// base_refill_ms`. Control: 500 ms refills four times as many as the
/// default 2000 ms in the same 4 s.
#[test]
fn flame_fuel_refills_at_base_refill_ms() {
    let mut got = Vec::new();
    for ms in [2000u32, 500] {
        let (mut wi, mut k) = (0, 0);
        let mut w = flat_world_with(1, |s| {
            wi = make_flamer(s);
            k = fuel_cartridge(s, wi, 200);
            s.rules.flame.base_refill_ms = ms;
        });
        let b = w.game().unwrap().base_of(PlayerId(0));
        let sq = spawn(&mut w, 0, RIFLES, b.x.floor_int(), b.y.floor_int());
        let man = members(&w, sq)[1];
        assert!(w.edit_soldier(man, |kit, c| {
            kit.rounds[k] = 0;
            c.mag = 0;
        }));
        let ex = frontline_content::SupplyKind::Explosives as usize;
        let a0 = w.game().unwrap().players[0].ammo.refilled[ex];
        let ticks = 4 * super::s6_tests::HZ;
        steps(&mut w, ticks);
        let units = w.game().unwrap().players[0].ammo.refilled[ex] - a0;
        let every = ms as u64 * super::s6_tests::HZ / 1000;
        let want = (ticks / every) as u32;
        assert!(
            units + 1 >= want && units <= want,
            "{ms} ms: {units} units, about {want}"
        );
        got.push(units);
    }
    assert!(got[1] >= 3 * got[0], "control: {got:?}");
}

/// F1 (checker 7): a flame man with an empty tank and no fuel left draws
/// his sidearm (the S5a rule) and fires no particle. Control: with fuel
/// he fires particles and keeps the flamethrower.
#[test]
fn dry_flamer_draws_his_sidearm() {
    for dry in [false, true] {
        // His own fuel feed (no squadmate carries it to share).
        let mut k = 0;
        let (mut w, _sq, man, _wi) = flame_scene(1, 14, |s| {
            let wi = s.weapons.len() as u16 - 1;
            k = fuel_cartridge(s, wi, 200);
        });
        let s = w.match_setup().unwrap();
        let t = s.squad_type_index(RIFLES).unwrap() as usize;
        assert!(
            s.squad_types[t].members[1].sidearm.is_some(),
            "he carries a P 38"
        );
        if dry {
            assert!(w.edit_soldier(man, |kit, c| {
                kit.rounds[k] = 0;
                c.mag = 0;
            }));
        }
        // While he lives (the enemy 14 m off may kill him).
        let mut on_side = false;
        for _ in 0..90 {
            w.step(&[]);
            if w.state.entities.kind[man.index()] == crate::ecs::EntityKind::Soldier {
                on_side |= w.state.entities.kit[man.index()].on_sidearm;
            }
        }
        let fired = s14_of(&w, 0).flame_fired;
        if dry {
            assert!(on_side, "dry: the sidearm");
            assert_eq!(fired, 0, "dry: no particle");
        } else {
            assert!(!on_side && fired > 0, "control: he flames");
        }
    }
}

/// F6 (checker 6): a step-out point must be walkable: not inside a static
/// box (a bunker's wall). Control: open ground is.
#[test]
fn step_out_points_are_walkable() {
    let mut w = super::s10b_tests::bunker_world(1, |_| {});
    super::s10b_tests::bunker_at(&mut w, true);
    let t = w.terrain();
    let g = w.game().unwrap();
    let wall = g
        .static_boxes()
        .iter()
        .find(|b| {
            let c = b.frame.center;
            c.z - b.frame.half.z <= t.height_at(c.xy()) + Fx::from_milli(300)
        })
        .expect("a box standing on the ground");
    let c = wall.frame.center.xy();
    let from = FxVec3::new(
        Fx::from_int(530),
        Fx::from_int(530),
        t.height_at(FxVec2::from_int(530, 530)),
    );
    assert!(
        !g.walkable(t, from, c.extend(t.height_at(c))),
        "inside a wall"
    );
    let open = FxVec2::from_int(532, 530);
    assert!(
        g.walkable(t, from, open.extend(t.height_at(open))),
        "control: open ground"
    );
}

/// F6 (checker 6): a gun crewman at his gun and a rider in his vehicle
/// stay in burning ground. Control: men in formation step out
/// (`men_step_out_of_burning_ground`).
#[test]
fn crews_and_riders_do_not_step_out() {
    let mut w = flat_world_with(1, |s| {
        s.rules.flame.enabled = 1;
        (s.rules.flame.patch_hp_per_s, s.rules.flame.burn_hp_per_s) = (0, 0);
    });
    // A towed gun card arrives hitched with its crew aboard the tow.
    let t = w
        .match_setup()
        .unwrap()
        .squad_type_index("ww2:ger_pak40_squad")
        .unwrap();
    super::s6_tests::run(&mut w, 0, 0, CommandKind::CallIn { squad_type: t });
    let tow = *w
        .game()
        .unwrap()
        .vehicles
        .iter()
        .find(|(_, v)| v.towing.is_some())
        .unwrap()
        .0;
    let rider = w.game().unwrap().riders_of(tow)[0];
    // A gun emplaced at a site with its crew in their slots.
    let gsq = spawn(&mut w, 0, "ww2:ger_pak40_squad", 500, 500);
    let gun = members(&w, gsq)[0];
    let crew = w.game().unwrap().vehicles[&gun]
        .served
        .as_ref()
        .unwrap()
        .slots
        .iter()
        .find_map(|s| s.man)
        .unwrap();
    for m in [rider, crew] {
        let at = w.state.entities.transform[m.index()].pos;
        put_patch(&mut w, at, 1, 900);
    }
    steps(&mut w, 30);
    assert_eq!(s14_of(&w, 0).stepped_out, 0);
    assert!(w.state.entities.unit[rider.index()].riding.is_some());
    assert!(w.state.entities.unit[crew.index()].serving.is_some());
}

/// Checker 9: a request dropped at the cap is not counted in
/// `fired[FLAME]` (only particles made are). Control: `flame_capped` > 0,
/// so requests were dropped.
#[test]
fn capped_flame_is_not_counted_fired() {
    let (mut w, ..) = flame_scene(1, 14, |s| s.rules.flame.particles_per_player = 4);
    steps(&mut w, 240);
    let p = &w.game().unwrap().players[0];
    assert!(p.s14.flame_capped > 0, "control: requests dropped");
    assert_eq!(
        p.ammo.fired[crate::supply::fired_class::FLAME],
        p.s14.flame_fired
    );
}

/// Checker 10: burn damage per pulse loses nothing to truncation: 1 HP/s
/// at 500 ms pulses takes 10 HP in 10 s (the old formula took 0).
/// Control: `burn_hp_per_s` 0 takes none.
#[test]
fn burn_pulses_lose_no_hp() {
    for hp in [1u32, 0] {
        let (mut w, sq, ..) = flame_scene(1, 0, |s| {
            s.rules.flame.burn_hp_per_s = hp;
            s.rules.flame.burn_s = 60;
        });
        steps(&mut w, 2);
        let m = members(&w, sq)[5];
        let hp0 = w.state.entities.combat[m.index()].hp;
        set_burning(&mut w, m, 60 * super::s6_tests::HZ);
        steps(&mut w, 10 * super::s6_tests::HZ);
        let lost = (hp0 - w.state.entities.combat[m.index()].hp) as u32;
        if hp == 1 {
            assert!((9..=11).contains(&lost), "lost {lost}");
        } else {
            assert_eq!(lost, 0, "control");
        }
    }
}

/// Checker 13: `Close::strike` (a u8 tick count) holds the longest
/// wind-up and recovery the ranges allow at every allowed tick rate.
/// Control: 4001 ms is out of range.
#[test]
fn strike_ticks_fit_a_u8() {
    for hz in frontline_content::TICK_HZ_ALLOWED {
        assert!(4000 * hz as u64 / 1000 <= u8::MAX as u64, "{hz} Hz");
    }
    let w = flat_world_with(1, |_| {});
    let mut s = w.game().unwrap().setup.clone();
    (s.rules.melee.windup_ms, s.rules.melee.recover_ms) = (4000, 4000);
    assert!(s.validate().is_empty());
    s.rules.melee.windup_ms = 4001;
    assert!(!s.validate().is_empty(), "control");
}

/// Checker 14: `ClearTrench` and `Strike` are refused while their
/// switches are 0. Control: the refusal is the S14 one (`NotAllowed`).
#[test]
fn strike_and_clear_are_refused_while_off() {
    let mut w = flat_world_with(1, |_| {});
    let sq = spawn(&mut w, 0, RIFLES, 300, 300);
    w.dig_trench_now(
        &[FxVec2::from_int(310, 290), FxVec2::from_int(310, 330)],
        -1,
    );
    steps(&mut w, 2);
    super::s6_tests::run(
        &mut w,
        0,
        0,
        CommandKind::ClearTrench {
            squad: sq,
            at: FxVec2::from_int(310, 300),
        },
    );
    // The refusals of this tick only.
    let r = w.fort_refusals();
    assert_eq!(r.len(), 1, "ClearTrench refused");
    assert_eq!(
        r.last().unwrap().refusal,
        crate::fort::FortRefusal::NotAllowed
    );
    assert!(w.game().unwrap().clears().is_empty());
    let m = members(&w, sq)[0];
    super::s6_tests::run(&mut w, 0, 1, CommandKind::Strike { units: vec![m] });
    let r = w.fort_refusals();
    assert_eq!(r.len(), 1, "Strike refused");
    assert_eq!(
        r.last().unwrap().refusal,
        crate::fort::FortRefusal::NotAllowed
    );
}

/// Spec 3.1 load checks the checker listed (checker 8), one case each on
/// a staged save with a downed man and a tank squad added. Control: that
/// save loads.
#[test]
fn load_rejects_bad_s14_state_more() {
    let (mut w, sq, burn, strike) = staged();
    let downed = members(&w, sq)[3];
    assert!(w.down_soldier(downed, (1, kill_class::RIFLE)));
    let tank = spawn(&mut w, 0, "ww2:ger_pz4_h_squad", 400, 400);
    let bytes = w.save();
    World::load(&bytes, 1).expect("control: the staged save loads");
    type Edit = Box<dyn Fn(&mut SimState)>;
    let cases: Vec<(&str, Edit)> = vec![
        (
            "burn end beyond the longest burn",
            Box::new(move |s| close(s, burn).burn_until = u64::MAX / 2),
        ),
        (
            "strike on a downed man",
            Box::new(move |s| {
                *close(s, downed) = Close {
                    strike: 4,
                    strike_phase: STRIKE_WINDUP,
                    ..Close::default()
                }
            }),
        ),
        (
            "clear step over steps_max",
            Box::new(move |s| game_mut(s).clears.get_mut(&sq).unwrap().step = 200),
        ),
        (
            "clear of a vehicle squad",
            Box::new(move |s| {
                let g = game_mut(s);
                let c = g.clears[&sq];
                g.clears.insert(tank, c);
            }),
        ),
        (
            "clear point off the map",
            Box::new(move |s| game_mut(s).clears.get_mut(&sq).unwrap().at.x = Fx::from_int(-5)),
        ),
        (
            "patches over the cap",
            Box::new(|s| {
                let g = game_mut(s);
                let one = g.patches[0];
                let n = g.s14_limits().patches as u32;
                g.patches = (0..=n).map(|i| Patch { id: 10 + i, ..one }).collect();
                g.next_patch = 20 + n;
            }),
        ),
        (
            "flame ttl over the longest life",
            Box::new(|s| game_mut(s).flame[0].ttl = u16::MAX),
        ),
        (
            "flame shooter out of range",
            Box::new(|s| game_mut(s).flame[0].shooter = SimId(u32::MAX)),
        ),
    ];
    let _ = strike;
    for (name, e) in cases {
        let mut x = World::load(&bytes, 1).unwrap();
        e(&mut x.state);
        assert!(World::load(&x.save(), 1).is_err(), "{name} loads");
    }
}
