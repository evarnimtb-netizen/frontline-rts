//! S7 open-top exposure (`docs/slices/S7-casemate-td.md` 4, vehicles.md
//! 6.5): the M10's turret has no roof. Thrown items drop in, rest on the
//! floor, ride with the vehicle and burst inside; a mortar bomb through the
//! open top bursts among the crew; near misses reach the crew over the rim
//! but not through the walls; a closed tank is unchanged. Also the save
//! field `Thrown::inside` (saved, hashed, checked on load) and determinism
//! with an item inside across threads and save/load.

use std::path::Path;

use frontline_content::{Difficulty, MatchRequest, MatchSetup, ModDefs};
use frontline_sim::{
    Command, CommandKind, Fx, FxVec2, FxVec3, ItemKind, PlayerId, SimId, SquadId, World,
};

fn setup(seed: u64) -> MatchSetup {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2");
    let mut defs = ModDefs::load(&root).unwrap_or_else(|e| panic!("{e}"));
    let rules = defs
        .add_rules_override(&root.join("tests/rules_test_unlock.toml"))
        .unwrap_or_else(|e| panic!("{e}"));
    let mut req = MatchRequest::skirmish(seed, "ww2:germany", "ww2:usa", Difficulty::Normal);
    req.rules = rules;
    let mut s = defs.resolve(&req).unwrap_or_else(|e| panic!("{e}"));
    s.players.iter_mut().for_each(|p| p.is_ai = false);
    s.rules.start_manpower = 100_000;
    s.map.terrain.octaves.clear();
    s.map.terrain.bands.clear();
    s.map.woods.clear();
    // Keep the crew in place after losses (the test counts their hits).
    s.rules.vehicles.bail.crew_below = 0;
    s
}

fn xy(x: i32, y: i32) -> FxVec2 {
    FxVec2::from_int(x, y)
}

const EAST: FxVec2 = FxVec2::new(Fx::ONE, Fx::ZERO);
const WEST: FxVec2 = FxVec2::new(Fx::NEG_ONE, Fx::ZERO);

fn cmd(player: u8, seq: u32, kind: CommandKind) -> Command {
    Command {
        player: PlayerId(player),
        seq,
        kind,
    }
}

fn spawn(w: &mut World, player: u8, card: &str, at: FxVec2, f: FxVec2) -> (SquadId, SimId) {
    let t = w.match_setup().unwrap().squad_type_index(card).unwrap();
    let sq = w.spawn_squad_at(PlayerId(player), t, at, f).unwrap();
    (sq, w.game().unwrap().squad(sq).unwrap().members[0])
}

fn pos3(w: &World, id: SimId) -> FxVec3 {
    w.entities().transform()[id.index()].pos
}

/// The vehicle's crew HP, seat by seat.
fn crew_hp(w: &World, v: SimId) -> Vec<u16> {
    w.game()
        .unwrap()
        .vehicle(v)
        .unwrap()
        .seats
        .iter()
        .map(|s| s.hp)
        .collect()
}

fn disarm(w: &mut World, id: SimId) {
    assert!(w.edit_vehicle(id, |v| {
        v.ammo.iter_mut().for_each(|n| *n = 0);
        v.loaded = None;
    }));
}

/// A German rifleman with a stick grenade 15 m west of a disarmed, stopped
/// M10 (or Sherman) facing west; he throws at the vehicle's centre.
fn throw_world(seed: u64, card: &str) -> (World, SimId, SimId) {
    let mut w = World::new_match(setup(seed), 1).unwrap();
    let (_, v) = spawn(&mut w, 1, card, xy(415, 500), WEST);
    disarm(&mut w, v);
    let (sq, _) = spawn(&mut w, 0, "ww2:ger_grenadiergruppe", xy(400, 500), EAST);
    let men = w.game().unwrap().squad(sq).unwrap().members.clone();
    let man = *men
        .iter()
        .find(|m| w.entities().kit()[m.index()].items[ItemKind::Frag.index()] > 0)
        .expect("a man with a grenade");
    w.step(&[]);
    let target = pos3(&w, v).xy();
    w.step(&[cmd(
        0,
        1,
        CommandKind::Throw {
            units: vec![man],
            item: ItemKind::Frag,
            target,
        },
    )]);
    (w, v, man)
}

/// Steps until the first thrown item is at rest (or gone); its index 0.
fn until_landed(w: &mut World) -> Option<frontline_sim::Thrown> {
    for _ in 0..30 * 6 {
        w.step(&[]);
        if let Some(t) = w.game().unwrap().thrown().first()
            && t.landed
        {
            return Some(t.clone());
        }
    }
    None
}

#[test]
fn a_grenade_into_a_stopped_m10_lands_inside_rides_and_hits_the_crew() {
    let mut inside_runs = 0;
    let mut hit_runs = 0;
    for seed in 1..=4u64 {
        let (mut w, m10, _) = throw_world(seed, "ww2:usa_m10_squad");
        let before = crew_hp(&w, m10);
        let t = until_landed(&mut w).expect("the grenade landed");
        if t.inside.is_none() {
            continue;
        }
        inside_runs += 1;
        let i = t.inside.unwrap();
        assert_eq!(i.vehicle, m10);
        assert_eq!(i.part, 1, "the turret box");
        assert_eq!(w.game().unwrap().players()[0].s7.open_top_throws, 1);
        // It rides: the M10 drives off and the grenade stays in its turret.
        let sq = w.entities().squad()[m10.index()].unwrap();
        w.step(&[cmd(
            1,
            1,
            CommandKind::Move {
                squad: sq,
                target: xy(470, 500),
            },
        )]);
        let landed_at = t.pos;
        let mut moved = Fx::ZERO;
        while let Some(t) = w.game().unwrap().thrown().first().cloned() {
            let d = t.pos.xy() - pos3(&w, m10).xy();
            assert!(d.length() < Fx::from_int(2), "it fell out: {d:?}");
            moved = (t.pos - landed_at).length();
            w.step(&[]);
        }
        assert!(moved > Fx::from_ratio(1, 10), "it did not ride: {moved:?}");
        let after = crew_hp(&w, m10);
        if after.iter().zip(&before).any(|(a, b)| a < b) {
            hit_runs += 1;
        }
        assert!(w.game().unwrap().players()[0].s7.open_top_hits > 0 || after == before);
    }
    assert!(inside_runs >= 2, "only {inside_runs} of 4 dropped in");
    assert!(hit_runs >= 2, "crew hit in only {hit_runs} runs");
}

#[test]
fn a_grenade_at_a_closed_tank_drops_to_the_ground() {
    for seed in 1..=3u64 {
        let (mut w, sh, _) = throw_world(seed, "ww2:usa_m4a3_75_squad");
        let before = crew_hp(&w, sh);
        let t = until_landed(&mut w).expect("landed");
        assert!(t.inside.is_none());
        let ground = w.terrain().height_at(t.pos.xy());
        assert_eq!(t.pos.z, ground, "it rests on the ground");
        for _ in 0..30 * 5 {
            w.step(&[]);
        }
        assert_eq!(crew_hp(&w, sh), before, "a closed tank's crew is safe");
    }
}

fn mortar(w: &World) -> u16 {
    w.match_setup()
        .unwrap()
        .weapons
        .iter()
        .position(|x| x.id == "ww2:grw34")
        .unwrap() as u16
}

#[test]
fn a_bomb_through_the_open_top_bursts_among_the_crew() {
    for (card, open) in [
        ("ww2:usa_m10_squad", true),
        ("ww2:usa_m4a3_75_squad", false),
    ] {
        let mut w = World::new_match(setup(3), 1).unwrap();
        let (_, v) = spawn(&mut w, 1, card, xy(500, 500), WEST);
        disarm(&mut w, v);
        w.step(&[]);
        let before = crew_hp(&w, v);
        // The turret pivot is 20 cm behind the hull centre (both types).
        let at = pos3(&w, v).xy() + FxVec2::new(Fx::from_ratio(2, 10), Fx::ZERO);
        let weapon = mortar(&w);
        assert!(w.drop_shell_now(0, weapon, at));
        for _ in 0..30 {
            w.step(&[]);
        }
        let after = crew_hp(&w, v);
        let hurt = after.iter().zip(&before).filter(|(a, b)| a < b).count();
        if open {
            assert!(hurt >= 1, "no crewman hurt: {before:?} -> {after:?}");
        } else {
            assert_eq!(hurt, 0, "the Sherman's roof stops an 8 mm bomb");
        }
    }
}

/// Bombs bursting on the engine deck behind the turret (fragments rise
/// over the rim into the exposed crew) and 3-6 m around the vehicle: an
/// M10's crew is hit by some deck bursts; a Sherman's never (the walls and
/// the roof stop every fragment).
#[test]
fn bursts_reach_open_top_crew_over_the_rim_only() {
    let run = |card: &str| {
        let mut hurt = 0;
        for k in 0..12i64 {
            let mut w = World::new_match(setup(10 + k as u64), 1).unwrap();
            let (_, v) = spawn(&mut w, 1, card, xy(500, 500), EAST);
            disarm(&mut w, v);
            w.step(&[]);
            let before = crew_hp(&w, v);
            let at = if k < 6 {
                // The deck, 1.8-2.6 m behind the hull centre.
                let back = Fx::from_ratio(180 + 16 * k, 100);
                pos3(&w, v).xy() - FxVec2::new(back, Fx::from_ratio(k * 10 - 30, 100))
            } else {
                let (s, c) = Fx::from_ratio(k * 1047, 1000).sin_cos(); // k · 60°
                pos3(&w, v).xy() + FxVec2::new(c, s) * Fx::from_int(3 + (k % 4) as i32)
            };
            let weapon = mortar(&w);
            assert!(w.drop_shell_now(0, weapon, at));
            for _ in 0..30 {
                w.step(&[]);
            }
            let after = crew_hp(&w, v);
            hurt += usize::from(after.iter().zip(&before).any(|(a, b)| a < b));
        }
        hurt
    };
    assert_eq!(run("ww2:usa_m4a3_75_squad"), 0, "a Sherman's crew is hit");
    let m10 = run("ww2:usa_m10_squad");
    assert!(m10 >= 2, "only {m10} of 12 bursts hurt the M10 crew");
}

/// Save field `Thrown::inside`: saved, loaded, hashed, checked on load; a
/// fight with an item inside is the same at 1/2/3/8 threads and across
/// save, load and continue.
#[test]
fn an_item_inside_is_saved_hashed_checked_and_deterministic() {
    // Find a seed whose grenade drops in.
    let seed = (1..=8u64)
        .find(|&s| {
            let (mut w, _, _) = throw_world(s, "ww2:usa_m10_squad");
            until_landed(&mut w).is_some_and(|t| t.inside.is_some())
        })
        .expect("a grenade dropped in");
    let (mut w, m10, _) = throw_world(seed, "ww2:usa_m10_squad");
    let t = until_landed(&mut w).unwrap();
    let i = t.inside.unwrap();
    let base = w.full_state_hash();
    let loaded = World::load(&w.save(), 2).unwrap();
    assert_eq!(loaded.full_state_hash(), base);
    assert_eq!(loaded.game().unwrap().thrown()[0].inside, Some(i));
    let mut e = World::load(&w.save(), 1).unwrap();
    assert!(e.edit_thrown(0, |t| t.inside.as_mut().unwrap().local.x +=
        Fx::from_ratio(1, 10)));
    assert_ne!(e.full_state_hash(), base, "inside is not hashed");
    assert_eq!(e.full_state_hash(), e.state_hash());
    // Refused on load: outside the box, a closed box, no vehicle, in flight.
    let bad: [fn(&mut frontline_sim::Thrown); 4] = [
        |t| t.inside.as_mut().unwrap().local.z = Fx::from_int(3),
        |t| t.inside.as_mut().unwrap().part = 0,
        |t| t.inside.as_mut().unwrap().vehicle = SimId(9000),
        |t| t.landed = false,
    ];
    for (k, edit) in bad.into_iter().enumerate() {
        let mut b = World::load(&w.save(), 1).unwrap();
        assert!(b.edit_thrown(0, edit));
        assert!(World::load(&b.save(), 1).is_err(), "bad edit {k} accepted");
    }
    let _ = m10;
    // Determinism: from the throw on, across threads and save/load.
    let run = |threads: usize, split: Option<u64>| {
        let (w0, _, _) = throw_world(seed, "ww2:usa_m10_squad");
        let mut w = World::load(&w0.save(), threads).unwrap();
        let mut hashes = Vec::new();
        for t in 0..30 * 8u64 {
            if Some(t) == split {
                w = World::load(&w.save(), threads).unwrap();
            }
            w.step(&[]);
            if t % 15 == 0 {
                hashes.push(w.state_hash());
            }
        }
        (hashes, w.full_state_hash())
    };
    let base = run(1, None);
    for threads in [2, 3, 8] {
        assert_eq!(run(threads, None), base, "{threads} threads");
    }
    assert_eq!(run(3, Some(30 * 3)), base, "save/load/continue");
}
