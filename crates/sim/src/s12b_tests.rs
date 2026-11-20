//! S12a Build B tests (docs/slices/S12-wounds-medics.md 4 W6-W11, 6): the
//! medic rules and their commands. Every test turns `[wounds] enabled` on
//! in the test only. Each names its control.

use super::s6_tests::{flat_world_with, members, run, spawn, steps};
use super::s12_tests::stage_downed;
use super::*;
use crate::ecs::EntityKind;
use crate::wounds::WOUNDED;

const RIFLES: &str = "ww2:ger_grenadiergruppe";
const MEDICS: &str = "ww2:ger_sanitaetstrupp";

fn c(x: i32, y: i32) -> FxVec2 {
    FxVec2::from_int(x, y)
}

fn world(auto_tend_m: u32) -> World {
    flat_world_with(1, |s| {
        s.rules.wounds.enabled = 1;
        s.rules.wounds.auto_tend_m = auto_tend_m;
    })
}

fn cas(w: &World, m: SimId) -> crate::wounds::Casualty {
    w.game().unwrap().casualties[&m].clone()
}

fn s12(w: &World, p: usize) -> crate::wounds::S12Counters {
    w.game().unwrap().players[p].s12
}

fn kind(w: &World, m: SimId) -> EntityKind {
    w.state.entities.kind[m.index()]
}

fn pos(w: &World, m: SimId) -> FxVec2 {
    w.state.entities.transform[m.index()].pos.xy()
}

/// A rifle squad at (500, 500) with its third man downed (bleeding out in
/// 170 s) and a medic team at (`mx`, 500). Returns `(world, downed man,
/// rifle squad, medics)`.
fn scene(auto_tend_m: u32, mx: i32) -> (World, SimId, SquadId, Vec<SimId>) {
    let mut w = world(auto_tend_m);
    let sq = spawn(&mut w, 0, RIFLES, 500, 500);
    let mt = spawn(&mut w, 0, MEDICS, mx, 500);
    steps(&mut w, 3);
    let d = members(&w, sq)[2];
    let t = w.tick();
    stage_downed(&mut w, d, t + 170 * 30);
    let med = members(&w, mt);
    (w, d, sq, med)
}

fn cmd(w: &mut World, seq: u32, kind: CommandKind) -> bool {
    let before = w.stats().commands_applied;
    run(w, 0, seq, kind);
    w.stats().commands_applied > before
}

/// Steps until `f` holds (at most `max` ticks); the ticks it took.
fn until(w: &mut World, max: u64, f: impl Fn(&World) -> bool) -> Option<u64> {
    for k in 0..max {
        if f(w) {
            return Some(k);
        }
        w.step(&[]);
    }
    f(w).then_some(max)
}

/// W6 (row Stabilise): `Tend` sends the nearest medic; standing by the
/// man he stabilises him in `stabilise_s` (bleed-out off, counted), then
/// rejoins his squad. Control: a non-medic's `Tend` on a downed man is
/// refused, and with `[wounds] enabled` 0 every `Tend` is.
#[test]
fn w6_tend_stabilises_a_downed_man() {
    let (mut w, d, sq, med) = scene(0, 530);
    let rifle = members(&w, sq)[0];
    assert!(
        !cmd(
            &mut w,
            1,
            CommandKind::Tend {
                units: vec![rifle],
                target: d
            }
        ),
        "control: a rifleman cannot stabilise"
    );
    assert!(cmd(
        &mut w,
        2,
        CommandKind::Tend {
            units: med.clone(),
            target: d
        }
    ));
    let near = *med
        .iter()
        .min_by_key(|&&m| ((pos(&w, m) - pos(&w, d)).length_sq(), m))
        .unwrap();
    let k = until(&mut w, 60 * 30, |w| cas(w, d).bleed_out_at == 0).expect("stabilised");
    assert!(k >= 15 * 30, "stabilise_s at least: {k}");
    assert_eq!(s12(&w, 0).stabilised, 1);
    assert_eq!(cas(&w, d).tend, None);
    assert!(
        (pos(&w, near) - pos(&w, d)).length_sq() <= Fx::from_int(9),
        "the nearer medic went"
    );
    steps(&mut w, 2);
    assert!(!w.state.entities.unit[near.index()].detached, "he rejoined");
    // Stabilised: he no longer bleeds out.
    steps(&mut w, 200 * 30);
    assert_eq!(kind(&w, d), EntityKind::Downed);
    // Control: switches off refuses.
    let mut off = flat_world_with(1, |s| s.rules.wounds.enabled = 0);
    let q = spawn(&mut off, 0, MEDICS, 500, 500);
    steps(&mut off, 2);
    let ms = members(&off, q);
    assert!(!cmd(
        &mut off,
        1,
        CommandKind::Tend {
            units: vec![ms[0]],
            target: ms[1]
        }
    ));
}

/// W6 (row Stabilise, "two medics: lower id tends"; "pinned: pauses").
/// Both medics stand by the man: the lower id tends. Pinned for a while,
/// the finish tick moves by as many ticks. Control: unpinned it does not.
#[test]
fn w6_two_medics_lower_id_tends_and_pinned_pauses() {
    for pin in [false, true] {
        let (mut w, d, _, med) = scene(0, 530);
        let at = pos(&w, d);
        assert!(cmd(
            &mut w,
            1,
            CommandKind::MoveUnits {
                units: med.clone(),
                target: at
            }
        ));
        until(&mut w, 40 * 30, |w| cas(w, d).tend.is_some()).expect("a tend starts");
        let (m, done) = cas(&w, d).tend.unwrap();
        // Both stand there by now or soon; the lower id of those within
        // reach took it.
        let lo = *med.iter().min().unwrap();
        if (pos(&w, lo) - at).length_sq() <= Fx::from_int(4) {
            assert_eq!(m, lo, "lower id tends");
        }
        let pin_k = 90u64;
        let pin_pts = w.match_setup().unwrap().rules.combat.pin_suppression * 4;
        for _ in 0..pin_k {
            if pin {
                w.state.entities.combat[m.index()].suppression = pin_pts * 1000;
            }
            w.step(&[]);
        }
        let (_, done2) = cas(&w, d).tend.expect("still tending");
        if pin {
            assert!(done2 >= done + pin_k / 2, "paused: {done} -> {done2}");
        } else {
            assert_eq!(done2, done, "control: unpinned keeps the tick");
        }
    }
}

/// W6 (row Stabilise, "medic dies" and "killed mid-tend"): a dead medic
/// ends the tend and the man keeps his bleed-out tick; a man killed by a
/// stray round mid-tend is gone and nothing is counted. Control: an
/// undisturbed tend stabilises (first test).
#[test]
fn w6_medic_dies_or_casualty_killed_mid_tend() {
    let (mut w, d, _, med) = scene(0, 503);
    let bleed = cas(&w, d).bleed_out_at;
    assert!(cmd(
        &mut w,
        1,
        CommandKind::Tend {
            units: med.clone(),
            target: d
        }
    ));
    until(&mut w, 30 * 30, |w| cas(w, d).tend.is_some()).expect("a tend starts");
    let (m, _) = cas(&w, d).tend.unwrap();
    assert!(w.kill_soldier(m));
    steps(&mut w, 2);
    let cs = cas(&w, d);
    assert_eq!((cs.tend, cs.bleed_out_at), (None, bleed), "keeps bleeding");
    // Killed mid-tend.
    let (mut w, d, _, med) = scene(0, 503);
    assert!(cmd(
        &mut w,
        1,
        CommandKind::Tend {
            units: med,
            target: d
        }
    ));
    until(&mut w, 30 * 30, |w| cas(w, d).tend.is_some()).expect("a tend starts");
    // A stray round kills him.
    super::s12_tests::bullet(&mut w, 1, "ww2:kar98k", d, crate::wounds::ZONE_TORSO);
    assert_eq!(kind(&w, d), EntityKind::Corpse);
    steps(&mut w, 20 * 30);
    assert_eq!(s12(&w, 0).stabilised, 0);
}

/// W6 auto-tend: a medic team holding within `auto_tend_m` goes to the
/// downed man on its own and stabilises him. Control: with `auto_tend_m`
/// 0 nobody goes.
#[test]
fn w6_medics_tend_on_their_own() {
    for auto in [25, 0] {
        let (mut w, d, _, _) = scene(auto, 515);
        let got = until(&mut w, 50 * 30, |w| cas(w, d).bleed_out_at == 0);
        assert_eq!(got.is_some(), auto > 0, "auto_tend_m {auto}");
    }
}

/// W6 buddy bandage (row Bandage, buddy half): a rifleman sent by `Tend`
/// dresses a wounded man held under fire in `bandage_s`; one dressing is
/// counted; he rejoins. Control: under the same fire with no buddy the man
/// does not bandage himself.
#[test]
fn w6_buddy_bandages_under_fire() {
    for buddy in [true, false] {
        let mut w = world(0);
        let sq = spawn(&mut w, 0, RIFLES, 500, 500);
        steps(&mut w, 3);
        let ms = members(&w, sq);
        let (hurt, b) = (ms[2], ms[5]);
        w.edit_soldier(hurt, |_, c| {
            c.hp = 40;
            c.wound.state = WOUNDED;
            c.wound.by = (1, crate::game::kill_class::RIFLE);
        });
        let crouch = w.match_setup().unwrap().rules.combat.crouch_suppression;
        if buddy {
            assert!(cmd(
                &mut w,
                1,
                CommandKind::Tend {
                    units: vec![b],
                    target: hurt
                }
            ));
        }
        for _ in 0..45 * 30 {
            w.state.entities.combat[hurt.index()].suppression = (crouch + 5) * 1000;
            w.step(&[]);
        }
        let wd = w.state.entities.combat[hurt.index()].wound;
        assert_eq!(wd.bandaged, buddy, "buddy {buddy}");
        assert_eq!(s12(&w, 0).bandaged, buddy as u32);
        if buddy {
            assert!(!w.state.entities.unit[b.index()].detached, "he rejoined");
        }
    }
}

/// W6 (row Stabilise, save/load mid-job): a save taken mid-tend loads and
/// plays on to the same hash. Control: the unsaved run is the reference.
#[test]
fn w6_save_load_mid_tend_keeps_the_hash() {
    let (mut w, d, _, med) = scene(0, 503);
    assert!(cmd(
        &mut w,
        1,
        CommandKind::Tend {
            units: med,
            target: d
        }
    ));
    until(&mut w, 30 * 30, |w| cas(w, d).tend.is_some()).expect("a tend starts");
    steps(&mut w, 60);
    let mut x = World::load(&w.save(), 1).unwrap();
    assert_eq!(x.full_state_hash(), w.full_state_hash());
    for _ in 0..20 * 30 {
        w.step(&[]);
        x.step(&[]);
    }
    assert_eq!(x.full_state_hash(), w.full_state_hash());
    assert_eq!(cas(&x, d).bleed_out_at, 0, "stabilised after the load");
}

/// Steps until casualty `d` is set down (no carriers), at most `max`
/// ticks; the ticks it took.
fn carried(w: &mut World, d: SimId, max: u64) -> Option<u64> {
    until(w, max, |w| {
        w.game()
            .unwrap()
            .casualties
            .get(&d)
            .is_none_or(|c| c.carriers[0].is_none())
    })
}

/// W7 (row Carry): two medics carry a downed man on a stretcher to the
/// point and set him down there (`carried`); two riflemen drag him, slower
/// (`drag_speed_pct` 30 against `stretcher_speed_pct` 60). A second pair
/// is refused. Control: the stretcher arrives first.
#[test]
fn w7_stretcher_and_drag_carry_him_to_the_point() {
    let mut took = Vec::new();
    for stretcher in [true, false] {
        let (mut w, d, sq, med) = scene(0, 503);
        let units = if stretcher {
            med.clone()
        } else {
            members(&w, sq)[3..5].to_vec()
        };
        let to = c(500, 450);
        assert!(cmd(
            &mut w,
            1,
            CommandKind::Carry {
                units,
                target: d,
                to
            }
        ));
        let cs = cas(&w, d);
        assert!(cs.carriers.iter().all(|x| x.is_some()) && cs.dest == Some(to));
        let other = if stretcher {
            members(&w, sq)[3..5].to_vec()
        } else {
            med.clone()
        };
        assert!(
            !cmd(
                &mut w,
                2,
                CommandKind::Carry {
                    units: other,
                    target: d,
                    to
                }
            ),
            "a second pair is refused"
        );
        let k = carried(&mut w, d, 200 * 30).expect("set down");
        assert_eq!(kind(&w, d), EntityKind::Downed);
        assert!(
            (pos(&w, d) - to).length_sq() <= Fx::from_int(9),
            "at the point"
        );
        assert_eq!(s12(&w, 0).carried, 1);
        took.push(k);
    }
    assert!(took[0] < took[1], "stretcher {} drag {}", took[0], took[1]);
}

/// W7 (row Carry, "carrier hit: set down"): a carrier wounded in transit
/// sets the man down where he is; the other carrier halts. Control: an
/// untouched carry reaches the point (first test).
#[test]
fn w7_a_hit_carrier_sets_him_down() {
    let (mut w, d, _, med) = scene(0, 503);
    let to = c(500, 430);
    assert!(cmd(
        &mut w,
        1,
        CommandKind::Carry {
            units: med.clone(),
            target: d,
            to
        }
    ));
    until(&mut w, 60 * 30, |w| {
        w.state.entities.unit[med[0].index()].order == (crate::unit::UnitOrder::Move { target: to })
    })
    .expect("lifted");
    steps(&mut w, 5 * 30);
    let p0 = pos(&w, d);
    w.edit_soldier(med[1], |_, c| {
        c.hp = 40;
        c.wound.state = WOUNDED;
    });
    steps(&mut w, 2);
    let cs = cas(&w, d);
    assert_eq!((cs.carriers, cs.dest), ([None, None], None));
    assert!(
        (pos(&w, d) - p0).length_sq() <= Fx::from_int(1),
        "set down there"
    );
    assert!((pos(&w, d) - to).length_sq() > Fx::from_int(100));
    steps(&mut w, 60);
    assert!(
        w.state.entities.velocity[med[0].index()].lin.length_sq() <= Fx::from_milli(10),
        "the other carrier halted"
    );
    assert_eq!(s12(&w, 0).carried, 0);
}

/// W7 (row Carry, "dies in transit"): an unstabilised man keeps bleeding
/// while carried and dies on the way; nothing is counted carried.
/// Control: bleeding out later he arrives (first test).
#[test]
fn w7_he_bleeds_out_in_transit() {
    let (mut w, d, _, med) = scene(0, 503);
    let t = w.tick();
    w.state
        .game
        .as_mut()
        .unwrap()
        .casualties
        .get_mut(&d)
        .unwrap()
        .bleed_out_at = t + 20 * 30;
    w.refresh_hashes();
    assert!(cmd(
        &mut w,
        1,
        CommandKind::Carry {
            units: med,
            target: d,
            to: c(500, 300)
        }
    ));
    steps(&mut w, 25 * 30);
    assert_eq!(kind(&w, d), EntityKind::Corpse);
    assert_eq!(s12(&w, 0).carried, 0);
    assert_eq!(s12(&w, 0).bled_out, 1);
}

/// W7: carriers do not fire. With an enemy squad in range the carriers
/// hold no target while they carry. Control: their squadmates do.
#[test]
fn w7_carriers_do_not_fire() {
    let (mut w, d, sq, _) = scene(0, 540);
    let ms = members(&w, sq);
    let carriers = ms[3..5].to_vec();
    assert!(cmd(
        &mut w,
        1,
        CommandKind::Carry {
            units: carriers.clone(),
            target: d,
            to: c(500, 400)
        }
    ));
    spawn(&mut w, 1, "ww2:usa_rifle_squad", 500, 640);
    let mut others = false;
    for _ in 0..20 * 30 {
        w.step(&[]);
        for &m in &carriers {
            if kind(&w, m) == EntityKind::Soldier
                && w.game()
                    .unwrap()
                    .casualties
                    .get(&d)
                    .is_some_and(|c| c.carriers.contains(&Some(m)))
            {
                assert_eq!(
                    w.state.entities.combat[m.index()].target,
                    None,
                    "a carrier aims"
                );
            }
        }
        others |= ms.iter().any(|&m| {
            !carriers.contains(&m)
                && kind(&w, m) == EntityKind::Soldier
                && w.state.entities.combat[m.index()].target.is_some()
        });
    }
    assert!(others, "control: the squad engages");
}

/// W7 (row Carry, save/load mid-job): a save taken mid-carry plays on to
/// the same hash. Control: the unsaved run is the reference.
#[test]
fn w7_save_load_mid_carry_keeps_the_hash() {
    let (mut w, d, _, med) = scene(0, 503);
    assert!(cmd(
        &mut w,
        1,
        CommandKind::Carry {
            units: med,
            target: d,
            to: c(500, 450)
        }
    ));
    steps(&mut w, 15 * 30);
    assert!(cas(&w, d).carriers[0].is_some(), "mid-carry");
    let mut x = World::load(&w.save(), 1).unwrap();
    for _ in 0..30 * 30 {
        w.step(&[]);
        x.step(&[]);
    }
    assert_eq!(x.full_state_hash(), w.full_state_hash());
    assert_eq!(s12(&x, 0).carried, 1);
}
