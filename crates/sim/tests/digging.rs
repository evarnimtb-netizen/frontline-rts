//! Digging (S1 stage 2): the Dig order on Kessel Ridge, work rates, passes,
//! soldiers on the trench floor, trench cover against flat fire, dirty tile
//! reporting, and determinism with terrain edits (thread counts, save →
//! load → continue).

use std::path::Path;

use frontline_content::{Difficulty, MatchRequest, ModDefs};
use frontline_sim::{
    Command, CommandKind, Fx, FxVec2, FxVec3, NavPhase, Order, PlayerId, SquadId, TILE_POINTS,
    World, tile_heights_f32,
};

fn world(seed: u64, threads: usize) -> World {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2");
    let defs = ModDefs::load(&root).unwrap_or_else(|e| panic!("{e}"));
    let mut setup = defs
        .resolve(&MatchRequest::skirmish(
            seed,
            "ww2:germany",
            "ww2:usa",
            Difficulty::Normal,
        ))
        .unwrap_or_else(|e| panic!("{e}"));
    // These tests drive both sides by hand.
    setup.players.iter_mut().for_each(|p| p.is_ai = false);
    World::new_match(setup, threads).unwrap()
}

fn cmd(player: u8, seq: u32, kind: CommandKind) -> Command {
    Command {
        player: PlayerId(player),
        seq,
        kind,
    }
}

/// Player `p`'s squads of type `ty` (e.g. "ww2:ger_rifle_squad").
fn squads(w: &World, p: u8, ty: &str) -> Vec<SquadId> {
    let t = w.match_setup().unwrap().squad_type_index(ty).unwrap();
    w.game()
        .unwrap()
        .squads()
        .iter()
        .filter(|(_, s)| s.owner == PlayerId(p) && s.type_idx == t)
        .map(|(id, _)| *id)
        .collect()
}

fn leader_pos(w: &World, sq: SquadId) -> FxVec2 {
    let lead = w.game().unwrap().squad(sq).unwrap().members[0];
    w.entities().transform()[lead.index()].pos.xy()
}

/// A straight line of `len` m starting 15 m from the squad's leader toward
/// the map centre and running across that direction.
fn line_near(w: &World, sq: SquadId, len: i32) -> Vec<FxVec2> {
    let lead = leader_pos(w, sq);
    let to_mid = (FxVec2::from_int(512, 512) - lead).normalize_or_zero();
    let start = lead + to_mid * Fx::from_int(15);
    vec![start, start + to_mid.perp() * Fx::from_int(len)]
}

fn digging(w: &World, sq: SquadId) -> bool {
    matches!(
        w.game().unwrap().squad(sq).unwrap().order,
        Order::Dig { .. }
    )
}

/// Whether any of the squad's men has put work into the line.
fn started(w: &World, sq: SquadId) -> bool {
    match &w.game().unwrap().squad(sq).unwrap().order {
        Order::Dig { job, .. } => job.segments.iter().any(|g| g.work > 0),
        _ => true,
    }
}

/// Orders a dig and returns the seconds from the first spadeful to done.
fn dig_seconds(w: &mut World, sq: SquadId, line: Vec<FxVec2>) -> u64 {
    w.step(&[cmd(0, 0, CommandKind::Dig { squad: sq, line })]);
    assert!(digging(w, sq), "dig order rejected");
    for _ in 0..30 * 120 {
        if started(w, sq) {
            break;
        }
        w.step(&[]);
    }
    assert!(started(w, sq), "never started digging");
    let t0 = w.tick();
    while digging(w, sq) {
        w.step(&[]);
        assert!(w.tick() - t0 < 30 * 600, "dig never finished");
    }
    (w.tick() - t0) / 30
}

#[test]
fn rifle_squad_digs_20_m_in_three_to_four_minutes() {
    let mut w = world(1, 2);
    let sq = squads(&w, 0, "ww2:ger_rifle_squad")[0];
    let line = line_near(&w, sq, 20);
    let secs = dig_seconds(&mut w, sq, line);
    assert!((180..=240).contains(&secs), "took {secs} s");
}

#[test]
fn engineers_dig_twice_as_fast_per_man() {
    // 8 riflemen on 20 m and 5 engineers on 12.5 m both have 2.5 m a man.
    let mut w = world(2, 2);
    let rifle = squads(&w, 0, "ww2:ger_rifle_squad")[0];
    let eng = squads(&w, 0, "ww2:ger_engineer_squad")[0];
    let line = line_near(&w, rifle, 20);
    let rifle_s = dig_seconds(&mut w, rifle, line);
    let lead = leader_pos(&w, eng);
    let to_mid = (FxVec2::from_int(512, 512) - lead).normalize_or_zero();
    let start = lead + to_mid * Fx::from_int(15);
    let line = vec![start, start + to_mid.perp() * Fx::from_milli(12_500)];
    let eng_s = dig_seconds(&mut w, eng, line);
    assert!(
        eng_s * 2 >= rifle_s * 9 / 10 && eng_s * 2 <= rifle_s * 11 / 10,
        "engineers {eng_s} s vs riflemen {rifle_s} s"
    );
}

#[test]
fn mg_and_mortar_teams_cannot_dig() {
    let mut w = world(3, 1);
    let mortar = w
        .match_setup()
        .unwrap()
        .squad_type_index("ww2:ger_mortar_team")
        .unwrap();
    let mg = w
        .match_setup()
        .unwrap()
        .squad_type_index("ww2:ger_mg_team")
        .unwrap();
    w.step(&[
        cmd(0, 0, CommandKind::CallIn { squad_type: mortar }),
        cmd(0, 1, CommandKind::CallIn { squad_type: mg }),
    ]);
    for _ in 0..30 * 60 {
        w.step(&[]);
    }
    let teams: Vec<SquadId> = ["ww2:ger_mortar_team", "ww2:ger_mg_team"]
        .iter()
        .flat_map(|t| squads(&w, 0, t))
        .collect();
    assert_eq!(teams.len(), 2, "call-ins did not arrive");
    let before = w.stats().commands_rejected;
    let orders: Vec<Command> = teams
        .iter()
        .enumerate()
        .map(|(k, &sq)| {
            let line = line_near(&w, sq, 10);
            cmd(0, 10 + k as u32, CommandKind::Dig { squad: sq, line })
        })
        .collect();
    w.step(&orders);
    assert_eq!(w.stats().commands_rejected - before, 2);
}

#[test]
fn trench_deepens_in_passes_and_diggers_stand_on_the_floor() {
    let mut w = world(4, 2);
    let sq = squads(&w, 0, "ww2:ger_engineer_squad")[0];
    let line = line_near(&w, sq, 10);
    w.step(&[cmd(
        0,
        0,
        CommandKind::Dig {
            squad: sq,
            line: line.clone(),
        },
    )]);
    // Depth at the first man's spot, sampled each second.
    let spot = line[0] + (line[1] - line[0]).div_int(10);
    let depth = |w: &World| w.terrain().base_height_at(spot) - w.terrain().height_at(spot);
    let mut seen = vec![depth(&w)];
    let mut dirty_ticks = 0;
    let mut buf = Vec::new();
    while digging(&w, sq) {
        w.step(&[]);
        if !w.snapshot().dirty_tiles.is_empty() {
            dirty_ticks += 1;
            // Every reported tile exists and changed this tick.
            for &(x, y) in &w.snapshot().dirty_tiles {
                let t = w.terrain().detail_tile(x, y).expect("dirty tile exists");
                assert_eq!(t.changed_tick(), w.tick() - 1);
                assert!(tile_heights_f32(w.terrain(), x, y, &mut buf));
                assert_eq!(buf.len(), TILE_POINTS as usize * TILE_POINTS as usize);
            }
        }
        if w.tick().is_multiple_of(30) && *seen.last().unwrap() != depth(&w) {
            seen.push(depth(&w));
        }
        assert!(w.tick() < 30 * 600, "dig never finished");
    }
    // Monotone, in at least 6 visible steps, ending at the full 1.6 m.
    assert!(seen.windows(2).all(|p| p[1] > p[0]), "{seen:?}");
    assert!(seen.len() >= 7, "{seen:?}");
    let full = Fx::from_milli(1600);
    assert!((*seen.last().unwrap() - full).abs() <= Fx::from_milli(5));
    // Passes rebuild tiles a handful of times, not every tick.
    assert!(
        (8..=8 * 5 * 4).contains(&dirty_ticks),
        "{dirty_ticks} dirty ticks"
    );
    // Finished: idle, holding, and every man on the trench floor.
    let s = w.game().unwrap().squad(sq).unwrap();
    assert_eq!(s.order, Order::Idle);
    assert_eq!(s.nav.phase, NavPhase::Holding);
    for &m in &s.members {
        let p = w.entities().transform()[m.index()].pos;
        let floor = w.terrain().base_height_at(p.xy()) - full;
        // Within the arrive radius of his spot, so possibly at the foot of
        // a wall (0.25 m detail interpolation): allow 0.25 m.
        assert!((p.z - floor).abs() <= Fx::from_milli(250), "{p:?}");
    }
    // The path grid sees the trench.
    let (gx, gy) = w.terrain().nearest_grid(spot);
    assert!(w.terrain().dug_dm(gx, gy) >= 10);
}

#[test]
fn squads_arriving_next_to_a_trench_take_cover_in_it() {
    let mut w = world(6, 2);
    let eng = squads(&w, 0, "ww2:ger_engineer_squad")[0];
    let rifle = squads(&w, 0, "ww2:ger_rifle_squad")[0];
    let line = line_near(&w, eng, 20);
    dig_seconds(&mut w, eng, line.clone());
    // Send the riflemen to a spot 5 m behind the middle of the trench.
    let mid = line[0] + (line[1] - line[0]).div_int(2);
    let to_base = (leader_pos(&w, rifle) - mid).normalize_or_zero();
    let target = mid + to_base * Fx::from_int(5);
    w.step(&[cmd(
        0,
        1,
        CommandKind::Move {
            squad: rifle,
            target,
        },
    )]);
    while !matches!(w.game().unwrap().squad(rifle).unwrap().order, Order::Idle) {
        w.step(&[]);
        assert!(w.tick() < 30 * 900, "never arrived");
    }
    // Arrived counts from 2 m off the slot; let them step down.
    for _ in 0..30 * 10 {
        w.step(&[]);
    }
    let s = w.game().unwrap().squad(rifle).unwrap();
    let deep = Fx::from_milli(1000);
    let in_trench = s
        .members
        .iter()
        .filter(|m| {
            let p = w.entities().transform()[m.index()].pos;
            w.terrain().base_height_at(p.xy()) - p.z >= deep
        })
        .count();
    assert!(in_trench >= 6, "only {in_trench} of 8 in the trench");
}

#[test]
fn trench_edge_stops_fire_at_a_crouched_man() {
    let mut w = world(5, 2);
    let sq = squads(&w, 0, "ww2:ger_rifle_squad")[0];
    let line = line_near(&w, sq, 12);
    dig_seconds(&mut w, sq, line.clone());
    let facing = (line[1] - line[0]).perp().normalize_or_zero();
    // The parapet faces the enemy, i.e. this side or the other.
    let s = w.game().unwrap().squad(sq).unwrap();
    let front = if s.nav.facing.dot_raw(facing) > 0 {
        facing
    } else {
        -facing
    };
    let t = w.terrain();
    let mid = line[0] + (line[1] - line[0]).div_int(2);
    let ground = t.base_height_at(mid);
    // Crouched head on the floor: 1.2 m above it, 0.4 m below ground.
    let head = mid.extend(ground - Fx::from_milli(400));
    // Just above the parapet.
    let up = mid.extend(ground + Fx::from_milli(600));
    // A rifleman 40 m in front, raised until the ground between would not
    // hide the crouched man's head: the ray clears the undug surface
    // everywhere beyond the parapet (2.1 m out). Whatever stops it is then
    // the trench's edge: the parapet, the lip or the wall.
    let shooter_xy = mid + front * Fx::from_int(40);
    let clears_base = |m: FxVec3| {
        (1..400).all(|k| {
            let p = m + (head - m).div_int(400).mul_int(k);
            (head.xy() - p.xy()).length() <= Fx::from_milli(2200)
                || p.z > t.base_height_at(p.xy()) + Fx::from_milli(20)
        })
    };
    let muzzle = (0..40)
        .map(|k| shooter_xy.extend(t.height_at(shooter_xy) + Fx::from_milli(400 + 250 * k)))
        .find(|&m| clears_base(m) && t.line_of_sight(m, up))
        .expect("trench line in view from some height");
    let hit = t.segment_hit(muzzle, head).expect("earth stops the shot");
    let short = (head.xy() - hit.xy()).length();
    assert!(
        short >= Fx::from_milli(300) && short <= Fx::from_milli(2200),
        "hit {short:?} m short of the head"
    );
    assert!(!t.line_of_sight(muzzle, head));
    assert!(t.segment_hit(muzzle, up).is_none());
}

/// Both players dig with every squad that can, at tick 1, after a short
/// walk; checks the incremental hash against a full recompute each publish.
fn dig_script(w: &World, tick: u64) -> Vec<Command> {
    if tick != 1 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (p, rifle, eng) in [
        (0u8, "ww2:ger_rifle_squad", "ww2:ger_engineer_squad"),
        (1u8, "ww2:usa_rifle_squad", "ww2:usa_engineer_squad"),
    ] {
        for (k, sq) in squads(w, p, rifle)
            .into_iter()
            .chain(squads(w, p, eng))
            .enumerate()
        {
            let line = line_near(w, sq, 8 + 4 * k as i32);
            out.push(cmd(p, k as u32, CommandKind::Dig { squad: sq, line }));
        }
    }
    out
}

fn run_to(w: &mut World, until: u64) -> Vec<(u64, u64)> {
    let mut out = Vec::new();
    while w.tick() < until {
        let cmds = dig_script(w, w.tick());
        if let Some(h) = w.step(&cmds) {
            assert_eq!(h, w.full_state_hash(), "tick {}", w.tick());
            out.push((w.tick(), h));
        }
    }
    out
}

#[test]
fn digging_is_identical_across_thread_counts() {
    let reference = run_to(&mut world(21, 1), 2400);
    for threads in [2, 3, 8] {
        let mut w = world(21, threads);
        let got = run_to(&mut w, 2400);
        assert_eq!(got, reference, "hashes differ with {threads} threads");
        assert!(w.terrain().detail().tiles().len() >= 4, "nothing was dug");
    }
}

#[test]
fn digging_survives_save_load_continue() {
    let mut straight = world(31, 2);
    let straight_hashes = run_to(&mut straight, 2400);

    let mut first = world(31, 2);
    run_to(&mut first, 1500);
    // Mid-dig: tiles exist and squads are still at work.
    assert!(!first.terrain().detail().tiles().is_empty());
    assert!(first.game().unwrap().squads().values().any(
        |s| matches!(&s.order, Order::Dig { job, .. } if job.segments.iter().any(|g| g.work > 0))
    ));
    let bytes = first.save();
    let mut resumed = World::load(&bytes, 3).unwrap();
    assert_eq!(resumed.state(), first.state());
    assert_eq!(resumed.state_hash(), first.state_hash());
    drop(first);
    let resumed_hashes = run_to(&mut resumed, 2400);
    let tail: Vec<_> = straight_hashes
        .into_iter()
        .filter(|(t, _)| *t > 1500)
        .collect();
    assert_eq!(resumed_hashes, tail);
    assert_eq!(resumed.save(), straight.save(), "final save bytes differ");
}
