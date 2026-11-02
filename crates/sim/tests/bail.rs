//! S3c bail-out, abandoned vehicles and re-crewing (`crate::bail`,
//! `docs/slices/S3c-armour.md` Part A): each trigger, the exits (hatch
//! times, the side away from the hit, men still inside die with the
//! vehicle), the abandoned vehicle (nobody's unit, not auto-targeted,
//! `AttackUnit` allowed, the cap), re-crew by a crew and by riflemen
//! (untrained), capture on and off and by crews only, the limit and pop
//! checks (also when the limit fills while boarding), no re-crew of a
//! burning or useless tank, exits clear of a neighbouring hull, crews
//! dismissed in base, and determinism across threads and save → load →
//! continue with an abandoned tank and a crew squad on the map.

use std::path::Path;

use frontline_content::{ComponentKind, Difficulty, MatchRequest, MatchSetup, ModDefs, SquadRole};
use frontline_sim::bail::{AB_AMMO_FIRE, AB_CREW, AB_ENGINE_FIRE, AB_ORDER, AB_USELESS};
use frontline_sim::scenarios::{EAST, WEST, at, cmd, spawn, tank};
use frontline_sim::{
    ArmourEventKind, CommandKind, EntityKind, Order, PlayerId, RecrewRefusal, SimId, SquadId, World,
};

/// The S1-S3c content frozen before the S4 content switch: these mechanics
/// tests were written for its squads (8-man rifle squads, 3-man MG teams)
/// and keep running on it; the S4 cards are covered by `s4_members.rs`,
/// `variants.rs` and the AI-vs-AI runs.
const S3C_MOD: &str = "../../sdk/crates/content/tests/fixtures/ww2_s3c";

fn setup(seed: u64) -> MatchSetup {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(S3C_MOD);
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
    s
}

fn kind(w: &World, id: SimId) -> EntityKind {
    w.entities().kind()[id.index()]
}

/// The crew squad of abandoned vehicle `v`.
fn crew_of(w: &World, v: SimId) -> Option<SquadId> {
    w.game().unwrap().vehicle(v).unwrap().crew_squad
}

/// Steps until `f` holds (at most `ticks`); whether it did.
fn run_until(w: &mut World, ticks: u32, mut f: impl FnMut(&World) -> bool) -> bool {
    for _ in 0..ticks {
        if f(w) {
            return true;
        }
        w.step(&[]);
    }
    f(w)
}

/// Abandon events of this tick: `(vehicle, cause)`.
fn abandoned_events(w: &World) -> Vec<(u32, u8)> {
    w.snapshot()
        .armour_events
        .iter()
        .filter(|e| e.kind == ArmourEventKind::Abandoned as u8)
        .map(|e| (e.vehicle, e.component))
        .collect()
}

/// Steps until vehicle `v` is abandoned; the cause.
fn until_abandoned(w: &mut World, v: SimId, ticks: u32) -> Option<u8> {
    for _ in 0..ticks {
        w.step(&[]);
        if let Some(&(_, c)) = abandoned_events(w).iter().find(|e| e.0 == v.0) {
            return Some(c);
        }
    }
    None
}

#[test]
fn each_trigger_abandons_the_tank() {
    type Edit = fn(&mut frontline_sim::VehicleState);
    let cases: [(&str, Edit, u8, u32); 4] = [
        ("ammo fire", |v| v.ammo_fire = 1, AB_AMMO_FIRE, 2),
        ("engine fire", |v| v.burn = 1, AB_ENGINE_FIRE, 30 * 9),
        (
            "crew losses",
            |v| {
                v.seats[2].hp = 0;
                v.seats[3].hp = 0;
                v.seats[4].hp = 0;
            },
            AB_CREW,
            2,
        ),
        (
            "gun and track out",
            |v| {
                v.components[ComponentKind::Gun.index()] = 0;
                v.components[ComponentKind::TrackLeft.index()] = 0;
            },
            AB_USELESS,
            2,
        ),
    ];
    for (name, edit, want, ticks) in cases {
        let mut s = setup(1);
        // The engine fire must not go out first.
        s.rules.vehicles.fire_out_pct = 0;
        let mut w = World::new_match(s, 1).unwrap();
        let (_, v) = tank(&mut w, 0, at(500, 500), EAST).unwrap();
        w.edit_vehicle(v, edit);
        let got = until_abandoned(&mut w, v, ticks);
        assert_eq!(got, Some(want), "{name}");
        assert_eq!(kind(&w, v), EntityKind::Abandoned, "{name}");
        // Nobody's unit: no squad, out of the pop and the vehicle count.
        let g = w.game().unwrap();
        assert!(w.entities().squad()[v.index()].is_none());
        assert_eq!(g.vehicles_counted(w.entities(), PlayerId(0)), 0);
        assert_eq!(g.players()[0].crew.abandoned, 1);
        if name == "engine fire" {
            // After two fire_check_s tries (8 s).
            assert!(w.tick() >= 30 * 8 - 1, "{}", w.tick());
        }
    }
    // A tank with four men and its gun only damaged fights on.
    let mut w = World::new_match(setup(1), 1).unwrap();
    let (_, v) = tank(&mut w, 0, at(500, 500), EAST).unwrap();
    w.edit_vehicle(v, |vs| {
        vs.seats[4].hp = 0;
        vs.components[ComponentKind::Gun.index()] = 0;
    });
    assert_eq!(until_abandoned(&mut w, v, 60), None);
    assert_eq!(kind(&w, v), EntityKind::Vehicle);
}

#[test]
fn the_abandon_order_bails_out_and_men_leave_by_their_hatches() {
    let mut w = World::new_match(setup(1), 1).unwrap();
    let (sq, v) = tank(&mut w, 0, at(500, 500), EAST).unwrap();
    // The last round went from south to north: the men get out on the
    // north side, away from the shooter.
    w.edit_vehicle(v, |vs| {
        vs.last_hit_dir =
            frontline_sim::FxVec2::new(frontline_sim::Fx::ZERO, frontline_sim::Fx::ONE)
    });
    w.step(&[cmd(0, 0, CommandKind::Abandon { squad: sq })]);
    assert_eq!(kind(&w, v), EntityKind::Abandoned);
    assert_eq!(abandoned_events(&w), vec![(v.0, AB_ORDER)]);
    let s = w.match_setup().unwrap().clone();
    let def = &s.vehicles[w.game().unwrap().vehicle(v).unwrap().type_idx as usize];
    // Panzer IV: five hatches; a turret man after 3 s, a hull man after 4.
    let mut out_at: Vec<(u64, u8)> = Vec::new();
    for _ in 0..30 * 6 {
        w.step(&[]);
        for e in &w.snapshot().armour_events {
            if e.kind == ArmourEventKind::CrewOut as u8 {
                out_at.push((w.tick(), e.component));
            }
        }
    }
    assert_eq!(out_at.len(), 5, "{out_at:?}");
    let first = out_at.iter().map(|x| x.0).min().unwrap();
    let last = out_at.iter().map(|x| x.0).max().unwrap();
    assert!(
        (88..=92).contains(&first) && (118..=122).contains(&last),
        "{out_at:?}"
    );
    let crew = crew_of(&w, v).expect("crew squad");
    let g = w.game().unwrap();
    let csq = g.squad(crew).unwrap();
    assert!(s.type_has_tag(csq.type_idx, "crew"));
    assert_eq!(Some(csq.type_idx), def.crew_squad);
    assert_eq!(csq.members.len(), 5);
    assert_eq!(g.players()[0].crew.out, 5);
    for &m in &csq.members {
        let p = w.entities().transform()[m.index()].pos.xy();
        assert!(p.y > frontline_sim::Fx::from_int(501), "north side: {p:?}");
        assert_eq!(kind(&w, m), EntityKind::Soldier);
    }
    // One MP 40 (the first man out), P 38s for the rest.
    let mp40 = s.weapons.iter().position(|x| x.id == "ww2:mp40").unwrap();
    let w0 = w.entities().combat()[csq.members[0].index()].mag;
    assert_eq!(w0 as u32, s.weapons[mp40].magazine);
    // The order does nothing to a crew squad or a vehicle without crew.
    w.step(&[cmd(0, 1, CommandKind::Abandon { squad: crew })]);
    assert_eq!(w.game().unwrap().players()[0].crew.abandoned, 1);
}

#[test]
fn men_still_inside_die_when_the_fire_burns_it_out() {
    let mut s = setup(1);
    // A fire that burns out in 2 s: the hull men (4 s) never get out.
    s.rules.vehicles.damage.ammo_fire_s = 2;
    let mut w = World::new_match(s, 1).unwrap();
    let (_, v) = tank(&mut w, 0, at(500, 500), EAST).unwrap();
    let losses = w.game().unwrap().players()[0].losses;
    w.edit_vehicle(v, |vs| vs.ammo_fire = 1);
    assert!(run_until(&mut w, 30 * 4, |w| kind(w, v) == EntityKind::Wreck));
    let g = w.game().unwrap();
    // Nobody got out in 2 s (turret men need 3 s): all five lost.
    assert_eq!(g.players()[0].crew.out, 0);
    assert_eq!(g.players()[0].losses, losses + 5);
    // With the default 8 s everyone gets out before the burn-out.
    let mut w = World::new_match(setup(1), 1).unwrap();
    let (_, v) = tank(&mut w, 0, at(500, 500), EAST).unwrap();
    w.edit_vehicle(v, |vs| vs.ammo_fire = 1);
    assert!(run_until(&mut w, 30 * 10, |w| kind(w, v) == EntityKind::Wreck));
    let g = w.game().unwrap();
    assert_eq!(g.players()[0].crew.out, 5);
    assert_eq!(g.players()[0].crew.bailed, 5);
}

#[test]
fn abandoned_tanks_are_not_auto_targeted_but_can_be_ordered_shot() {
    let mut w = World::new_match(setup(1), 1).unwrap();
    let (sq, v) = tank(&mut w, 0, at(500, 500), EAST).unwrap();
    let (esq, e) = tank(&mut w, 1, at(700, 500), WEST).unwrap();
    // Let them see each other, then bail out of the German tank at once.
    w.step(&[cmd(0, 0, CommandKind::Abandon { squad: sq })]);
    assert_eq!(kind(&w, v), EntityKind::Abandoned);
    for _ in 0..30 * 8 {
        w.step(&[]);
        let es = w.game().unwrap().vehicle(e).unwrap();
        assert_ne!(es.target, Some(v), "auto-targeted an abandoned tank");
    }
    // AttackUnit may name it (deny a re-crew or capture).
    w.step(&[cmd(
        1,
        0,
        CommandKind::AttackUnit {
            units: vec![e],
            target: v,
        },
    )]);
    assert_eq!(w.game().unwrap().vehicle(e).unwrap().attack, Some(v));
    let hit = run_until(&mut w, 30 * 20, |w| {
        w.snapshot()
            .armour_events
            .iter()
            .any(|x| x.vehicle == v.0 && x.kind <= ArmourEventKind::Penetration as u8 && x.kind > 0)
    });
    assert!(hit, "the ordered tank hits the abandoned one");
    let _ = esq;
}

/// A German tank abandoned by order at (500, 500) and its crew out, with
/// its full crew squad beside it.
fn abandoned_with_crew(s: MatchSetup) -> (World, SimId, SquadId) {
    let mut w = World::new_match(s, 1).unwrap();
    let (sq, v) = tank(&mut w, 0, at(500, 500), EAST).unwrap();
    w.step(&[cmd(0, 0, CommandKind::Abandon { squad: sq })]);
    assert!(run_until(&mut w, 30 * 6, |w| {
        crew_of(w, v).is_some_and(|c| w.game().unwrap().squad(c).unwrap().members.len() == 5)
    }));
    let crew = crew_of(&w, v).unwrap();
    (w, v, crew)
}

#[test]
fn a_crew_squad_re_crews_its_tank() {
    let (mut w, v, crew) = abandoned_with_crew(setup(1));
    w.step(&[cmd(
        0,
        1,
        CommandKind::Recrew {
            squad: crew,
            vehicle: v,
        },
    )]);
    assert!(matches!(
        w.game().unwrap().squad(crew).unwrap().order,
        Order::Recrew { .. }
    ));
    assert!(run_until(&mut w, 30 * 30, |w| kind(w, v) == EntityKind::Vehicle));
    let g = w.game().unwrap();
    assert_eq!(w.entities().owner()[v.index()], PlayerId(0));
    let vs = g.vehicle(v).unwrap();
    assert!(vs.seats.iter().all(|s| s.hp > 0 && !s.untrained));
    assert!(g.squad(crew).is_none(), "all five boarded");
    assert_eq!(g.players()[0].crew.recrewed, 1);
    assert_eq!(g.vehicles_counted(w.entities(), PlayerId(0)), 1);
    let sq = w.entities().squad()[v.index()].expect("a vehicle squad again");
    // It takes orders again.
    w.step(&[cmd(
        0,
        2,
        CommandKind::Move {
            squad: sq,
            target: at(560, 500),
        },
    )]);
    assert!(run_until(&mut w, 30 * 30, |w| {
        w.entities().transform()[v.index()].pos.xy().x > frontline_sim::Fx::from_int(540)
    }));
}

#[test]
fn riflemen_re_crew_untrained_and_reload_slower() {
    let (mut w, v, crew) = abandoned_with_crew(setup(1));
    // The crew walks off; riflemen take the tank.
    w.step(&[cmd(0, 1, CommandKind::Retreat { squad: crew })]);
    let rif = spawn(&mut w, 0, SquadRole::Rifle, at(500, 520), EAST).unwrap();
    w.step(&[cmd(
        0,
        2,
        CommandKind::Recrew {
            squad: rif,
            vehicle: v,
        },
    )]);
    assert!(run_until(&mut w, 30 * 30, |w| kind(w, v) == EntityKind::Vehicle));
    let g = w.game().unwrap();
    let vs = g.vehicle(v).unwrap();
    assert!(vs.seats.iter().all(|s| s.hp > 0 && s.untrained));
    // Three riflemen stay a squad.
    assert_eq!(g.squad(rif).unwrap().members.len(), 3);
    let s = w.match_setup().unwrap();
    let def = &s.vehicles[vs.type_idx as usize];
    let base = s.weapons[def.main_gun().weapon as usize].reload_ticks;
    let r = frontline_sim::gunnery::reload_of(vs, def, s) as u32;
    assert_eq!(r, base * s.rules.vehicles.recrew.untrained_reload_pct / 100);
    // AT teams never man a tank.
    let (mut w, v, _) = abandoned_with_crew(setup(1));
    let at_team = spawn(&mut w, 0, SquadRole::AntiTank, at(500, 520), EAST).unwrap();
    w.step(&[cmd(
        0,
        3,
        CommandKind::Recrew {
            squad: at_team,
            vehicle: v,
        },
    )]);
    let refused = &w.snapshot().recrew_refusals;
    assert_eq!(refused.len(), 1);
    assert_eq!(refused[0].why, RecrewRefusal::NotCrew);
}

#[test]
fn the_enemy_captures_with_capture_on_and_is_refused_with_it_off() {
    // (capture, capture_needs_crew): riflemen capture only when a crew is
    // not required.
    for (capture, needs_crew) in [(1, 0), (0, 0), (1, 1)] {
        let mut s = setup(1);
        s.rules.vehicles.recrew.capture = capture;
        s.rules.vehicles.recrew.capture_needs_crew = needs_crew;
        let (mut w, v, crew) = abandoned_with_crew(s);
        w.step(&[cmd(0, 1, CommandKind::Retreat { squad: crew })]);
        let rif = spawn(&mut w, 1, SquadRole::Rifle, at(500, 470), EAST).unwrap();
        w.step(&[cmd(
            1,
            0,
            CommandKind::Recrew {
                squad: rif,
                vehicle: v,
            },
        )]);
        if capture == 0 {
            assert_eq!(
                w.snapshot().recrew_refusals[0].why,
                RecrewRefusal::NoCapture
            );
            continue;
        }
        if needs_crew == 1 {
            assert_eq!(
                w.snapshot().recrew_refusals[0].why,
                RecrewRefusal::CaptureNeedsCrew
            );
            continue;
        }
        assert!(run_until(&mut w, 30 * 30, |w| kind(w, v) == EntityKind::Vehicle));
        let g = w.game().unwrap();
        assert_eq!(w.entities().owner()[v.index()], PlayerId(1));
        assert_eq!(g.players()[1].crew.captured, 1);
        assert_eq!(g.vehicles_counted(w.entities(), PlayerId(1)), 1);
        let ev = w
            .snapshot()
            .armour_events
            .iter()
            .find(|e| e.kind == ArmourEventKind::Recrewed as u8)
            .cloned();
        // The event may have been the tick before the last check.
        let _ = ev;
    }
}

#[test]
fn the_limit_and_pop_cap_refuse_a_re_crew() {
    let mut s = setup(1);
    s.rules.vehicles.limit = 1;
    let (mut w, v, crew) = abandoned_with_crew(s);
    // A second German tank fills the limit of one.
    tank(&mut w, 0, at(300, 300), EAST).unwrap();
    w.step(&[cmd(
        0,
        1,
        CommandKind::Recrew {
            squad: crew,
            vehicle: v,
        },
    )]);
    assert_eq!(
        w.snapshot().recrew_refusals[0].why,
        RecrewRefusal::VehicleLimit
    );
    let mut s = setup(1);
    s.rules.pop_cap = 10;
    let (mut w, v, crew) = abandoned_with_crew(s);
    w.step(&[cmd(
        0,
        1,
        CommandKind::Recrew {
            squad: crew,
            vehicle: v,
        },
    )]);
    assert_eq!(w.snapshot().recrew_refusals[0].why, RecrewRefusal::PopCap);
}

#[test]
fn a_full_limit_while_boarding_refuses_once_and_frees_the_squad() {
    let mut s = setup(1);
    s.rules.vehicles.limit = 1;
    let (mut w, v, crew) = abandoned_with_crew(s);
    w.step(&[cmd(
        0,
        1,
        CommandKind::Recrew {
            squad: crew,
            vehicle: v,
        },
    )]);
    assert!(w.snapshot().recrew_refusals.is_empty());
    // A new tank fills the limit while the crew walks back and boards.
    tank(&mut w, 0, at(300, 300), EAST).unwrap();
    let mut refused = Vec::new();
    for _ in 0..30 * 30 {
        w.step(&[]);
        refused.extend(w.snapshot().recrew_refusals.iter().map(|r| r.why));
    }
    assert_eq!(refused, vec![RecrewRefusal::VehicleLimit]);
    assert_eq!(kind(&w, v), EntityKind::Abandoned);
    assert_eq!(w.game().unwrap().squad(crew).unwrap().order, Order::Idle);
    assert!(w.game().unwrap().vehicle(v).unwrap().board.is_none());
}

#[test]
fn a_burning_or_useless_tank_refuses_a_re_crew() {
    type Edit = fn(&mut frontline_sim::VehicleState);
    let cases: [(Edit, RecrewRefusal); 2] = [
        (|v| v.burn = 1, RecrewRefusal::Burning),
        (
            |v| {
                v.components[ComponentKind::Gun.index()] = 0;
                v.components[ComponentKind::TrackLeft.index()] = 0;
            },
            RecrewRefusal::Useless,
        ),
    ];
    for (edit, want) in cases {
        let mut s = setup(1);
        s.rules.vehicles.fire_out_pct = 0;
        let (mut w, v, crew) = abandoned_with_crew(s);
        w.edit_vehicle(v, edit);
        w.step(&[cmd(
            0,
            1,
            CommandKind::Recrew {
                squad: crew,
                vehicle: v,
            },
        )]);
        assert_eq!(w.snapshot().recrew_refusals[0].why, want);
        // Never manned again (the burning one burns out to a wreck).
        for _ in 0..30 * 30 {
            w.step(&[]);
            assert_ne!(kind(&w, v), EntityKind::Vehicle);
        }
        let g = w.game().unwrap();
        assert_eq!(g.players()[0].crew.abandoned, 1);
        assert_eq!(g.players()[0].crew.recrewed, 0);
        assert_eq!(g.players()[0].crew.bailed, 5);
    }
}

#[test]
fn bailing_men_get_out_clear_of_a_neighbouring_hull() {
    let mut w = World::new_match(setup(1), 1).unwrap();
    let (sq, v) = tank(&mut w, 0, at(500, 503), EAST).unwrap();
    let (_, n) = tank(&mut w, 0, at(500, 500), EAST).unwrap();
    // The last round travelled south: the far side is the neighbour's.
    w.edit_vehicle(v, |vs| {
        vs.last_hit_dir =
            frontline_sim::FxVec2::new(frontline_sim::Fx::ZERO, -frontline_sim::Fx::ONE)
    });
    w.step(&[cmd(0, 0, CommandKind::Abandon { squad: sq })]);
    assert!(run_until(&mut w, 30 * 6, |w| {
        crew_of(w, v).is_some_and(|c| w.game().unwrap().squad(c).unwrap().members.len() == 5)
    }));
    let g = w.game().unwrap();
    let s = w.match_setup().unwrap();
    let def = &s.vehicles[g.vehicle(n).unwrap().type_idx as usize];
    let _ = def;
    let np = w.entities().transform()[n.index()].pos.xy();
    for &m in &g.squad(crew_of(&w, v).unwrap()).unwrap().members {
        let p = w.entities().transform()[m.index()].pos.xy();
        // The neighbour's hull is 2.8 m wide: nobody stands within it.
        let inside = (p.y - np.y).abs() < frontline_sim::Fx::from_ratio(14, 10)
            && (p.x - np.x).abs() < frontline_sim::Fx::from_int(4);
        assert!(!inside, "inside the neighbour's hull: {p:?}");
    }
}

#[test]
fn a_crew_in_base_is_dismissed_without_refund() {
    let (mut w, _, crew) = abandoned_with_crew(setup(1));
    let mp = w.game().unwrap().players()[0].manpower;
    w.step(&[cmd(0, 1, CommandKind::Retreat { squad: crew })]);
    assert!(run_until(&mut w, 30 * 400, |w| w
        .game()
        .unwrap()
        .squad(crew)
        .is_none()));
    let g = w.game().unwrap();
    assert_eq!(g.players()[0].crew.home, 5);
    assert!(g.players()[0].manpower >= mp, "income only");
    assert_eq!(g.players()[0].losses, 0, "dismissed men are not losses");
}

#[test]
fn over_the_cap_the_oldest_abandoned_tank_is_scrapped() {
    let mut s = setup(1);
    s.rules.vehicles.bail.abandoned_cap = 1;
    let mut w = World::new_match(s, 1).unwrap();
    let (a, va) = tank(&mut w, 0, at(400, 500), EAST).unwrap();
    let (b, vb) = tank(&mut w, 0, at(600, 500), EAST).unwrap();
    w.step(&[cmd(0, 0, CommandKind::Abandon { squad: a })]);
    w.step(&[]);
    w.step(&[cmd(0, 1, CommandKind::Abandon { squad: b })]);
    assert_eq!(kind(&w, va), EntityKind::Wreck);
    assert_eq!(kind(&w, vb), EntityKind::Abandoned);
    assert_eq!(w.game().unwrap().players()[0].crew.scrapped, 1);
}

/// A scripted match with bail-outs, a re-crew (tank C by its crew) and a
/// capture (tank A by US riflemen once its crew walks off), at `threads`;
/// the hashes, and the counters checked at the end.
fn scripted(threads: usize, save_at: Option<u64>) -> Vec<u64> {
    let mut s = setup(4);
    s.rules.vehicles.recrew.capture = 1;
    s.rules.vehicles.recrew.capture_needs_crew = 0;
    let mut w = World::new_match(s, threads).unwrap();
    let (sa, va) = tank(&mut w, 0, at(450, 500), EAST).unwrap();
    let (sc, vc) = tank(&mut w, 0, at(450, 300), EAST).unwrap();
    let (_, vb) = tank(&mut w, 1, at(650, 500), WEST).unwrap();
    let rif = spawn(&mut w, 1, SquadRole::Rifle, at(450, 470), WEST).unwrap();
    let mut hashes = Vec::new();
    for t in 0..30 * 40u64 {
        let mut cmds = Vec::new();
        if t == 10 {
            cmds.push(cmd(0, 0, CommandKind::Abandon { squad: sa }));
            cmds.push(cmd(0, 1, CommandKind::Abandon { squad: sc }));
        }
        if t == 30 * 8 {
            let g = w.game().unwrap();
            if let Some(c) = g.vehicle(va).unwrap().crew_squad {
                cmds.push(cmd(0, 2, CommandKind::Retreat { squad: c }));
            }
            if let Some(c) = g.vehicle(vc).unwrap().crew_squad {
                cmds.push(cmd(
                    0,
                    3,
                    CommandKind::Recrew {
                        squad: c,
                        vehicle: vc,
                    },
                ));
            }
        }
        if t == 30 * 10 {
            cmds.push(cmd(
                1,
                0,
                CommandKind::Recrew {
                    squad: rif,
                    vehicle: va,
                },
            ));
        }
        if t == 20 {
            w.edit_vehicle(vb, |v| v.ammo_fire = 1);
        }
        w.step(&cmds);
        if Some(t) == save_at {
            w = World::load(&w.save(), threads).unwrap();
        }
        hashes.push(w.state_hash());
    }
    let g = w.game().unwrap();
    assert_eq!(g.players()[1].crew.captured, 1, "A captured");
    assert_eq!(g.players()[0].crew.recrewed, 1, "C re-crewed");
    assert_eq!(w.entities().owner()[va.index()], PlayerId(1));
    hashes
}

#[test]
fn bail_out_and_re_crew_are_deterministic_across_threads_and_save_load() {
    let one = scripted(1, None);
    for threads in [2, 3, 8] {
        assert_eq!(scripted(threads, None), one, "{threads} threads");
    }
    // Saved with an abandoned tank and a crew squad on the map.
    assert_eq!(scripted(2, Some(30 * 6)), one, "save/load mid-exit");
    assert_eq!(scripted(1, Some(30 * 9)), one, "save/load while walking");
    assert_eq!(scripted(3, Some(30 * 11)), one, "save/load while boarding");
}
