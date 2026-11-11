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

/// A truck of `p` abandoned by its crew at (x, y) with `kg` of shells.
fn abandoned_truck(w: &mut World, p: u8, x: i32, y: i32, kg: u32) -> SimId {
    let (sq, v) = spawn(w, p, "ww2:ger_opel_blitz_squad", xy(x, y));
    w.edit_vehicle(v, |vs| {
        vs.cargo_g[SupplyKind::Shells.index()] = kg * 1000;
        vs.drop = Some(frontline_sim::CrateDrop {
            point: xy(x + 40, y),
            count: 2,
            kind: SupplyKind::Shells,
            next_tick: 0,
        });
    });
    w.step(&[]);
    let seq = w.tick() as u32;
    w.step(&[cmd(p, seq, CommandKind::Abandon { squad: sq })]);
    for _ in 0..30 * 40 {
        w.step(&[]);
        if w.game().unwrap().vehicle(v).unwrap().abandoned.is_some() {
            break;
        }
    }
    assert!(w.game().unwrap().vehicle(v).unwrap().abandoned.is_some());
    v
}

/// Review of S8b (M1): an abandoned truck scrapped over the cap leaves no
/// cargo on the wreck, so the save loads, and the kilograms count as lost.
#[test]
fn a_scrapped_truck_loses_its_cargo_and_the_save_still_loads() {
    let mut s = setup(2);
    s.rules.vehicles.bail.abandoned_cap = 1;
    let mut w = World::new_match(s, 1).unwrap();
    let first = abandoned_truck(&mut w, 0, 500, 500, 320);
    assert_eq!(
        w.game().unwrap().vehicle(first).unwrap().drop,
        None,
        "a crew that left drops no more crates"
    );
    // The crates it dropped before the crew left are not lost.
    let held = w.game().unwrap().vehicle(first).unwrap().cargo_g[SupplyKind::Shells.index()];
    assert!(held > 0);
    for _ in 0..30 {
        w.step(&[]);
    }
    let second = abandoned_truck(&mut w, 0, 540, 500, 100);
    for _ in 0..30 {
        w.step(&[]);
    }
    let g = w.game().unwrap();
    let vs = g.vehicle(first).unwrap();
    assert!(vs.wreck_tick.is_some(), "the oldest was scrapped");
    assert_eq!(vs.cargo_g, [0; 3], "a wreck holds no cargo");
    assert_eq!(g.vehicle(second).unwrap().wreck_tick, None);
    assert_eq!(g.supply6().accounts[0].truck_lost_g, held as u64);
    let w2 = World::load(&w.save(), 1).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(w2.full_state_hash(), w.full_state_hash());
}

/// A square trench ring of half width `h` round (x, y): wheels cannot leave.
fn trench_ring(w: &mut World, x: i32, y: i32, h: i32) {
    let line: Vec<FxVec2> = [(-h, -h), (h, -h), (h, h), (-h, h), (-h, -h)]
        .iter()
        .map(|(a, b)| xy(x + a, y + b))
        .collect();
    assert!(w.dig_trench_now(&line, 1));
    w.step(&[]);
}

/// Review of S8b (M3): an AI truck that cannot leave a ring is sent to its
/// drop once: the point is remembered, the wheels stop, the crates come off
/// where it stands, and no job loops on the same goal.
///
/// S10a-ai: the drop here was wanted by what the AI happened to buy (a
/// point's defenders short of rounds); the engineer cards in the buy mix
/// change that, so this scene runs with the fort switches off (the S10a
/// buys) and `..._with_the_fort_switches_on` proves the rule with a fixed
/// want.
#[test]
#[ignore = "seven game minutes; the release run has it"]
fn an_ai_truck_does_not_loop_on_a_goal_it_cannot_reach() {
    let mut s = setup(3);
    s.players[0].is_ai = true;
    let f = &mut s.rules.ai.fort;
    (f.works, f.wire, f.breach, f.cards) = (0, 0, 0, 0);
    let mut w = World::new_match(s, 1).unwrap();
    trench_ring(&mut w, 620, 620, 24);
    let (tsq, t) = spawn(&mut w, 0, "ww2:ger_opel_blitz_squad", xy(620, 620));
    w.edit_vehicle(t, |vs| vs.cargo_g = [100_000, 100_000, 300_000]);
    spawn(&mut w, 0, "ww2:ger_pz4_h_squad", xy(740, 620));
    for _ in 0..30 * 60 * 7 {
        w.step(&[]);
    }
    let g = w.game().unwrap();
    let ai = g.ai(PlayerId(0)).unwrap();
    let job = ai.trucks[&tsq];
    assert_eq!(ai.counters.truck_drops_ordered, 1, "no second order");
    assert!(job.gave_up.iter().any(Option::is_some), "goal remembered");
    assert_eq!(g.vehicle(t).unwrap().drop, None);
    assert!(
        g.vehicle(t).unwrap().cargo_g[SupplyKind::Shells.index()] < 300_000,
        "a goal it cannot reach ends like an arrival"
    );
}

/// S10a-ai twin of `an_ai_truck_does_not_loop_on_a_goal_it_cannot_reach`
/// with every fort switch on: a built own HQ out of stock outside the ring
/// wants crates; the truck is sent there, gives the point up, and while it
/// remembers the point (`GAVE_UP_GOALS` = 4, latest first) never orders it
/// again. (With the switches off the same scene re-orders the HQ once the
/// point has been pushed out by four later give-ups, about 6 minutes on:
/// an S8b rule, not a fort habit.)
#[test]
#[ignore = "seven game minutes; the release run has it"]
fn an_ai_truck_does_not_loop_on_a_goal_it_cannot_reach_with_the_fort_switches_on() {
    let mut s = setup(3);
    s.players[0].is_ai = true;
    let f = &s.rules.ai.fort;
    assert!(f.works != 0 && f.wire != 0 && f.breach != 0 && f.cards != 0);
    let mut w = World::new_match(s, 1).unwrap();
    let hq = {
        let s = w.match_setup().unwrap();
        s.stock_of(
            s.players[0].faction,
            frontline_content::setup::StockKind::Hq,
            None,
        )
        .unwrap()
    };
    let hq_at = xy(700, 700);
    w.build_structure_now(PlayerId(0), hq, hq_at, Fx::ZERO, [0, 0, 0])
        .unwrap();
    trench_ring(&mut w, 620, 620, 24);
    let (tsq, t) = spawn(&mut w, 0, "ww2:ger_opel_blitz_squad", xy(620, 620));
    w.edit_vehicle(t, |vs| vs.cargo_g = [100_000, 100_000, 300_000]);
    spawn(&mut w, 0, "ww2:ger_pz4_h_squad", xy(740, 620));
    let near =
        |a: FxVec2, b: FxVec2| (a - b).length_sq_raw() <= (FxVec2::from_int(40, 0)).length_sq_raw();
    let (mut orders, mut gave_up_hq) = (0u32, false);
    let mut last = None;
    for _ in 0..30 * 60 * 7 {
        w.step(&[]);
        let g = w.game().unwrap();
        let ai = g.ai(PlayerId(0)).unwrap();
        let Some(job) = ai.trucks.get(&tsq).copied() else {
            continue;
        };
        if ai.counters.truck_drops_ordered > orders {
            orders = ai.counters.truck_drops_ordered;
            // The goal of a new order is not one it remembered giving up.
            let before: Option<frontline_sim::ai::AiTruck> = last;
            let remembered =
                before.is_some_and(|b| b.gave_up.iter().flatten().any(|&p| near(p, job.moved_to)));
            assert!(
                !remembered,
                "order {orders} re-sends a remembered goal {:?}",
                job.moved_to
            );
        }
        gave_up_hq |= job.gave_up.iter().flatten().any(|&p| near(p, hq_at));
        last = Some(job);
    }
    assert!(orders >= 1, "the empty HQ wants crates");
    assert!(gave_up_hq, "the HQ goal was given up and remembered");
}

/// Review of S8b (M2): a recon car under way backs off from known armour:
/// driving on when it faces away, reversing when the threat is ahead.
#[test]
fn a_recon_car_under_way_backs_off_from_known_armour() {
    let car = "ww2:ger_sdkfz234_1_squad";
    let enemy = "ww2:usa_m4a3_75_squad";
    for (seed, tank_at, must_reverse) in [
        (1u64, xy(300, 306), false),
        (2, xy(300, 306), false),
        (3, xy(530, 400), true),
        (4, xy(530, 400), true),
    ] {
        let mut s = setup(seed);
        s.players[0].is_ai = true;
        s.rules.ai.recon.at_threat_m = 200;
        s.rules.vehicles.bail.under_fire_s = 0;
        let mut w = World::new_match(s, 1).unwrap();
        let (csq, c) = spawn(&mut w, 0, car, xy(400, 400));
        let (_, tank) = spawn(&mut w, 1, enemy, tank_at);
        // It only has to be known: it has no shot, and hits do not make its
        // crew leave within the test.
        let n = w.game().unwrap().vehicle(tank).unwrap().ammo.len();
        w.edit_vehicle(tank, |vs| {
            vs.ammo = vec![0; n];
            vs.loaded = None;
            vs.mgs.iter_mut().for_each(|m| m.spent = u32::MAX / 2);
        });
        let mut backed = false;
        for _ in 0..30 * 12 {
            w.step(&[]);
            let g = w.game().unwrap();
            let pos = w.entities().transform()[c.index()].pos.xy();
            let Some(order) = g.squad(csq).map(|q| q.order.clone()) else {
                break;
            };
            if let frontline_sim::Order::Move { target } = order
                && g.ai(PlayerId(0)).unwrap().counters.recon_backoffs > 0
                && (target - tank_at).length_sq_raw() > (pos - tank_at).length_sq_raw()
                && (!must_reverse || g.vehicle(c).unwrap().reverse)
            {
                backed = true;
                break;
            }
        }
        assert!(
            backed,
            "seed {seed}: no back-off from a tank {}",
            w.game().unwrap().vehicle(tank).is_some()
        );
    }
}
