//! S8b Build B (`docs/slices/S8b-trucks-recon.md` 7, 12): a supply truck in
//! a match: determinism at 1/2/3/8 threads across save, load and continue
//! while it serves a tank, drops crates and a car reverses.

use std::path::Path;

use frontline_content::setup::SupplyKind;
use frontline_content::{Difficulty, MatchRequest, MatchSetup, ModDefs};
use frontline_sim::{CommandKind, Fx, FxVec2, PlayerId, SimId, SquadId, World};

fn setup(seed: u64) -> MatchSetup {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2");
    let mut defs = ModDefs::load(&root).unwrap_or_else(|e| panic!("{e}"));
    let mut req = MatchRequest::skirmish(seed, "ww2:germany", "ww2:usa", Difficulty::Normal);
    req.rules = defs
        .add_rules_override(&root.join("tests/rules_test_unlock.toml"))
        .unwrap_or_else(|e| panic!("{e}"));
    let mut s = defs.resolve(&req).unwrap_or_else(|e| panic!("{e}"));
    s.players.iter_mut().for_each(|p| p.is_ai = false);
    s.rules.start_manpower = 100_000;
    s.map.terrain.octaves.clear();
    s.map.terrain.bands.clear();
    s.map.woods.clear();
    s.rules.vehicles.bail.crew_below = 0;
    s
}

fn xy(x: i32, y: i32) -> FxVec2 {
    FxVec2::from_int(x, y)
}

fn spawn(w: &mut World, p: u8, card: &str, at: FxVec2) -> (SquadId, SimId) {
    let t = w.match_setup().unwrap().squad_type_index(card).unwrap();
    let sq = w
        .spawn_squad_at(PlayerId(p), t, at, FxVec2::new(Fx::ONE, Fx::ZERO))
        .unwrap();
    (sq, w.game().unwrap().squad(sq).unwrap().members[0])
}

fn cmd(p: u8, seq: u32, kind: CommandKind) -> frontline_sim::Command {
    frontline_sim::scenarios::cmd(p, seq, kind)
}

fn start(enemy: bool) -> (World, [SimId; 2]) {
    let mut w = World::new_match(setup(11), 1).unwrap();
    let (_, truck) = spawn(&mut w, 0, "ww2:ger_opel_blitz_squad", xy(500, 500));
    let (_, drop_truck) = spawn(&mut w, 0, "ww2:ger_opel_blitz_squad", xy(500, 540));
    let (_, tank) = spawn(&mut w, 0, "ww2:ger_pz4_h_squad", xy(512, 500));
    w.edit_vehicle(truck, |v| v.cargo_g = [50_000, 20_000, 300_000]);
    w.edit_vehicle(drop_truck, |v| v.cargo_g = [0, 0, 128_000]);
    w.edit_vehicle(tank, |v| {
        v.ammo.iter_mut().for_each(|a| *a = 0);
        v.loaded = None;
        v.mgs[0].spent = 300;
    });
    let (car_sq, _) = spawn(&mut w, 0, "ww2:ger_sdkfz234_1_squad", xy(520, 560));
    if enemy {
        spawn(&mut w, 1, "ww2:usa_rifle_squad", xy(640, 500));
    }
    let seq = w.tick() as u32;
    w.step(&[
        cmd(
            0,
            seq,
            CommandKind::DropCrates {
                vehicle: drop_truck,
                point: xy(540, 540),
                count: 2,
                kind: SupplyKind::Shells,
            },
        ),
        cmd(
            0,
            seq + 1,
            CommandKind::Move {
                squad: car_sq,
                target: xy(460, 575),
            },
        ),
    ]);
    (w, [truck, drop_truck])
}

#[test]
fn a_truck_serving_is_deterministic_at_every_thread_count_and_across_saves() {
    let (mut s0, _) = start(true);
    for _ in 0..30 * 3 {
        s0.step(&[]);
    }
    let save = s0.save();
    let run = |threads: usize, split: bool| {
        let mut w = World::load(&save, threads).unwrap();
        let mut hashes = Vec::new();
        for t in 0..30 * 10u64 {
            if split && t == 30 * 5 {
                w = World::load(&w.save(), threads).unwrap();
            }
            w.step(&[]);
            if t % 15 == 0 {
                hashes.push(w.state_hash());
            }
        }
        (hashes, w.full_state_hash())
    };
    let reference = run(1, false);
    for threads in [1, 2, 3, 8] {
        assert_eq!(run(threads, true), reference, "threads {threads}");
    }
}

#[test]
fn the_cargo_counters_balance_through_serving_and_dropping() {
    let (mut w, trucks) = start(false);
    for _ in 0..30 * 45 {
        w.step(&[]);
    }
    let a = w.game().unwrap().supply6().accounts[0];
    let held: u64 = trucks
        .iter()
        .flat_map(|t| w.game().unwrap().vehicle(*t).unwrap().cargo_g)
        .map(|g| g as u64)
        .sum();
    assert_eq!(a.truck_crates_dropped, 2, "both crates off");
    assert_eq!(
        a.truck_out_g + a.truck_dropped_g + a.truck_lost_g + held,
        (50_000 + 20_000 + 300_000 + 128_000),
        "cargo balance"
    );
}
