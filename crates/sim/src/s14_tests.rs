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
