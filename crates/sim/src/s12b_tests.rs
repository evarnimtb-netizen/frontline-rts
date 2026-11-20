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
    let (m, _) = cas(&w, d).tend.unwrap();
    // A stray round kills him.
    super::s12_tests::bullet(&mut w, 1, "ww2:kar98k", d, crate::wounds::ZONE_TORSO);
    assert_eq!(kind(&w, d), EntityKind::Corpse);
    steps(&mut w, 20 * 30);
    assert_eq!(s12(&w, 0).stabilised, 0);
    // Checker G4: his medic rejoined his squad.
    assert!(!w.state.entities.unit[m.index()].detached);
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
            units: med.clone(),
            target: d,
            to: c(500, 300)
        }
    ));
    steps(&mut w, 25 * 30);
    assert_eq!(kind(&w, d), EntityKind::Corpse);
    assert_eq!(s12(&w, 0).carried, 0);
    assert_eq!(s12(&w, 0).bled_out, 1);
    // Checker G4: his carriers rejoined their squad.
    for &m in &med {
        assert!(!w.state.entities.unit[m.index()].detached);
    }
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

// ------------------------------------------------------- W8, W9, W11 --

use frontline_content::setup::StockKind;

/// A built aid post of player 0 at (`x`, `y`) with a medic team standing
/// in it (held formation). Returns `(post id, medics)`.
fn post(w: &mut World, x: i32, y: i32) -> (u32, Vec<SimId>) {
    let d = super::s6_tests::def(w, 0, StockKind::Aid, None);
    let id = super::s6_tests::add_built(w, 0, d, c(x, y), [0; 3]);
    let mt = spawn(w, 0, MEDICS, x, y);
    steps(w, 3);
    (id, members(w, mt))
}

fn beds(w: &World, id: u32) -> Vec<crate::wounds::Bed> {
    w.game()
        .unwrap()
        .supply6
        .structures
        .get(&id)
        .map_or(Vec::new(), |s| s.beds.clone())
}

/// A man of a new rifle squad at (`x`, `y`) made wounded (40 HP) and sent
/// (detached) to stand at `to`.
fn wounded_to(w: &mut World, x: i32, y: i32, to: FxVec2, seq: u32) -> SimId {
    let sq = spawn(w, 0, RIFLES, x, y);
    steps(w, 2);
    let m = members(w, sq)[5];
    w.edit_soldier(m, |_, c| {
        c.hp = 40;
        c.wound.state = WOUNDED;
        c.wound.bandaged = true;
    });
    assert!(cmd(
        w,
        seq,
        CommandKind::MoveUnits {
            units: vec![m],
            target: to
        }
    ));
    m
}

/// W8 (R3, R4): a staffed post treats a detached wounded man in
/// `treat_s`: back to Fit at 75 HP, `treated`, he rejoins his squad. Hit
/// again he is never treated twice; only `Evacuate` takes him (a bed,
/// `evac_s`; unpaid: no refund). Control: the first treatment happens.
#[test]
fn w8_post_treats_once_then_evacuates_on_order() {
    let mut w = world(0);
    let (pid, _) = post(&mut w, 400, 400);
    let m = wounded_to(&mut w, 430, 400, c(404, 404), 1);
    until(&mut w, 40 * 30, |w| !beds(w, pid).is_empty()).expect("a bed");
    let k = until(&mut w, 70 * 30, |w| {
        w.state.entities.combat[m.index()].wound.treated
    })
    .expect("treated");
    assert!(k >= 55 * 30, "treat_s: {k}");
    let c0 = w.state.entities.combat[m.index()];
    assert_eq!((c0.hp, c0.wound.state), (75, crate::wounds::FIT));
    assert_eq!(s12(&w, 0).treated, 1);
    steps(&mut w, 2);
    assert!(!w.state.entities.unit[m.index()].detached, "he regrouped");
    // Hit again, back at the post: no second treatment.
    w.edit_soldier(m, |_, c| {
        c.hp = 40;
        c.wound.state = WOUNDED;
        c.wound.bandaged = true;
    });
    assert!(cmd(
        &mut w,
        2,
        CommandKind::MoveUnits {
            units: vec![m],
            target: c(404, 404)
        }
    ));
    steps(&mut w, 90 * 30);
    assert!(beds(&w, pid).is_empty(), "R4: never treated twice");
    assert_eq!(s12(&w, 0).treated, 1);
    assert!(cmd(&mut w, 3, CommandKind::Evacuate { units: vec![m] }));
    assert!(beds(&w, pid).iter().any(|b| b.man == m && b.evac));
    until(&mut w, 40 * 30, |w| kind(w, m) == EntityKind::Vacant).expect("evacuated");
    assert_eq!(s12(&w, 0).evacuated, 1);
    assert_eq!(s12(&w, 0).refund_mp, 0, "unpaid: no refund");
}

/// W8 (row Bed, "staff short: pause"): with one medic dead the post
/// pauses its bed (the finish tick moves on). Control: staffed it does
/// not move.
#[test]
fn w8_staff_short_pauses_the_beds() {
    for short in [false, true] {
        let mut w = world(0);
        let (pid, med) = post(&mut w, 400, 400);
        wounded_to(&mut w, 430, 400, c(404, 404), 1);
        until(&mut w, 40 * 30, |w| !beds(w, pid).is_empty()).expect("a bed");
        let done = beds(&w, pid)[0].done_at;
        if short {
            assert!(w.kill_soldier(med[1]));
        }
        steps(&mut w, 60);
        let done2 = beds(&w, pid)[0].done_at;
        if short {
            assert!(done2 >= done + 55, "paused {done} -> {done2}");
        } else {
            assert_eq!(done2, done);
        }
    }
}

/// W8 (row Bed, "tent broken: post gone"; "killed by HE in bed"): HE that
/// breaks the tent destroys the post. A downed man in a bed killed by a
/// stray round leaves the bed and nothing is evacuated. Control: an
/// untouched downed man in a bed is evacuated (W9 test).
#[test]
fn w8_tent_broken_and_killed_in_bed() {
    let mut w = world(0);
    let (pid, _) = post(&mut w, 400, 400);
    let sq = spawn(&mut w, 0, RIFLES, 405, 395);
    steps(&mut w, 2);
    let d = members(&w, sq)[4];
    let t = w.tick();
    stage_downed(&mut w, d, t + 170 * 30);
    w.state
        .game
        .as_mut()
        .unwrap()
        .casualties
        .get_mut(&d)
        .unwrap()
        .bleed_out_at = 0;
    w.refresh_hashes();
    until(&mut w, 5 * 30, |w| {
        beds(w, pid).iter().any(|b| b.man == d && b.evac)
    })
    .expect("a bed");
    super::s12_tests::bullet(&mut w, 1, "ww2:kar98k", d, crate::wounds::ZONE_TORSO);
    steps(&mut w, 2);
    assert!(beds(&w, pid).is_empty(), "the bed emptied");
    assert_eq!(s12(&w, 0).evacuated, 0);
    // HE on the tent.
    let he = super::s11a2_tests::weapon(&w, "ww2:grw34");
    let t = w.tick();
    super::s11a2_tests::burst_now(&mut w, 1, he, 0, c(400, 400), t);
    steps(&mut w, 2);
    assert!(
        !w.game().unwrap().supply6.structures.contains_key(&pid),
        "the post is gone"
    );
}

/// W8 (row Bed, "two posts: lower id"): a stabilised downed man inside two
/// staffed posts takes a bed in the lower id's. Control: the other post
/// stays empty.
#[test]
fn w8_two_posts_lower_id_takes_him() {
    let mut w = world(0);
    let (a, _) = post(&mut w, 400, 400);
    let (b, _) = post(&mut w, 412, 400);
    let sq = spawn(&mut w, 0, RIFLES, 406, 380);
    steps(&mut w, 2);
    let d = members(&w, sq)[0];
    assert!(w.place_soldier(d, c(406, 400)));
    let t = w.tick();
    stage_downed(&mut w, d, t + 170 * 30);
    w.state
        .game
        .as_mut()
        .unwrap()
        .casualties
        .get_mut(&d)
        .unwrap()
        .bleed_out_at = 0;
    w.refresh_hashes();
    until(&mut w, 5 * 30, |w| {
        !beds(w, a).is_empty() || !beds(w, b).is_empty()
    })
    .expect("a bed");
    assert!(a < b);
    assert_eq!(beds(&w, a).len(), 1);
    assert!(beds(&w, b).is_empty());
}

/// W9: a stabilised downed man in his own base is evacuated after
/// `evac_s`: no corpse, pop freed, `evacuated`, a refund of his card's
/// per-man price x 40 % when he was paid, once. Control: unpaid, no refund.
#[test]
fn w9_evacuation_in_base_refunds_paid_men_once() {
    for paid in [true, false] {
        let mut w = world(0);
        let base = w.game().unwrap().base_of(PlayerId(0));
        let sq = spawn(&mut w, 0, RIFLES, base.x.floor_int(), base.y.floor_int());
        steps(&mut w, 2);
        let d = members(&w, sq)[3];
        let t = w.tick();
        stage_downed(&mut w, d, t + 170 * 30);
        w.state.entities.combat[d.index()].wound.paid = paid;
        w.state
            .game
            .as_mut()
            .unwrap()
            .casualties
            .get_mut(&d)
            .unwrap()
            .bleed_out_at = 0;
        w.refresh_hashes();
        let (mp, pop) = (
            w.game().unwrap().players[0].manpower,
            w.game().unwrap().population(&w.state.entities, PlayerId(0)),
        );
        let k = until(&mut w, 40 * 30, |w| kind(w, d) == EntityKind::Vacant).expect("evacuated");
        assert!(k >= 29 * 30, "evac_s: {k}");
        let ty =
            &w.match_setup().unwrap().squad_types[w.game().unwrap().squads[&sq].type_idx as usize];
        let refund = if paid {
            ty.cost / ty.men as u32 * 40 / 100
        } else {
            0
        };
        let g = w.game().unwrap();
        let _ = mp;
        assert_eq!(g.players[0].s12.refund_mp, refund);
        assert_eq!(g.players[0].s12.evacuated, 1);
        assert_eq!(g.population(&w.state.entities, PlayerId(0)), pop - 1);
        assert!(g.casualties.is_empty());
        steps(&mut w, 60);
        assert_eq!(w.game().unwrap().players[0].s12.refund_mp, refund, "once");
    }
}

/// W9 at a post (row Bed, save/load mid-job): a stabilised downed man in
/// a staffed post's bed is evacuated after `evac_s`; a save taken with him
/// in the bed plays on to the same hash. Control: the unsaved run.
#[test]
fn w9_post_evacuates_a_stabilised_man_and_save_load_keeps_the_hash() {
    let mut w = world(0);
    let (pid, _) = post(&mut w, 400, 400);
    let sq = spawn(&mut w, 0, RIFLES, 405, 395);
    steps(&mut w, 2);
    let d = members(&w, sq)[4];
    let t = w.tick();
    stage_downed(&mut w, d, t + 170 * 30);
    w.state
        .game
        .as_mut()
        .unwrap()
        .casualties
        .get_mut(&d)
        .unwrap()
        .bleed_out_at = 0;
    w.refresh_hashes();
    until(&mut w, 5 * 30, |w| !beds(w, pid).is_empty()).expect("a bed");
    steps(&mut w, 10 * 30);
    let mut x = World::load(&w.save(), 1).unwrap();
    for _ in 0..25 * 30 {
        w.step(&[]);
        x.step(&[]);
    }
    assert_eq!(x.full_state_hash(), w.full_state_hash());
    assert_eq!(kind(&x, d), EntityKind::Vacant);
    assert_eq!(s12(&x, 0).evacuated, 1);
}

/// W11 (R6): `Reinforce` fills a squad to its card size from base with
/// rifle kit (the first plain slot), per-man price, the card's cooldown
/// started; the men walk to the squad. Refused with the switch 0, during
/// the cooldown, for a full squad, at the pop cap. Control: before the
/// losses the squad is full and refused.
#[test]
fn w11_reinforce_fills_the_squad_from_base() {
    let mut w = flat_world_with(1, |s| {
        s.rules.wounds.enabled = 1;
        s.rules.wounds.reinforce = 1;
    });
    let sq = spawn(&mut w, 0, RIFLES, 500, 500);
    steps(&mut w, 3);
    assert!(
        !cmd(&mut w, 1, CommandKind::Reinforce { squad: sq }),
        "full"
    );
    let ms = members(&w, sq);
    for &m in &ms[6..9] {
        assert!(w.kill_soldier(m));
    }
    steps(&mut w, 1);
    let (t, mp0) = (
        w.game().unwrap().squads[&sq].type_idx,
        w.game().unwrap().players[0].manpower,
    );
    let ty = w.match_setup().unwrap().squad_types[t as usize].clone();
    assert!(cmd(&mut w, 2, CommandKind::Reinforce { squad: sq }));
    let now = members(&w, sq);
    assert_eq!(now.len(), ty.men as usize);
    let price = ty.cost / ty.men as u32;
    let g = w.game().unwrap();
    assert_eq!(mp0 - g.players[0].manpower, 3 * price);
    assert_eq!(
        (
            g.players[0].s12.reinforced_men,
            g.players[0].s12.reinforce_mp
        ),
        (3, 3 * price)
    );
    assert!(
        g.cooldown_left(PlayerId(0), t, w.tick()) > 0,
        "cooldown started"
    );
    let kar = super::s11a2_tests::weapon(&w, "ww2:kar98k");
    let base = g.base_of(PlayerId(0));
    let new: Vec<SimId> = now.iter().copied().filter(|m| !ms.contains(m)).collect();
    for &m in &new {
        assert_eq!(w.state.entities.kit[m.index()].weapon, kar, "rifle kit");
        assert!(w.state.entities.combat[m.index()].wound.paid);
        assert!(
            (pos(&w, m) - base).length_sq() < Fx::from_int(150 * 150),
            "at the base"
        );
    }
    let lead = now[0];
    until(&mut w, 400 * 30, |w| {
        new.iter()
            .all(|&m| (pos(w, m) - pos(w, lead)).length_sq() < Fx::from_int(30 * 30))
    })
    .expect("they walked to the squad");
    // Refusals: a gap again, but in the cooldown; then the switch off.
    assert!(w.kill_soldier(new[0]));
    steps(&mut w, 1);
    let cd = w.game().unwrap().cooldown_left(PlayerId(0), t, w.tick());
    if cd > 0 {
        assert!(
            !cmd(&mut w, 3, CommandKind::Reinforce { squad: sq }),
            "cooldown"
        );
    }
    let mut off = wounds_on_reinforce_off();
    let q = spawn(&mut off, 0, RIFLES, 500, 500);
    steps(&mut off, 2);
    assert!(off.kill_soldier(members(&off, q)[7]));
    steps(&mut off, 1);
    assert!(
        !cmd(&mut off, 1, CommandKind::Reinforce { squad: q }),
        "switch 0"
    );
    // Pop cap.
    let mut cap = flat_world_with(1, |s| {
        s.rules.wounds.enabled = 1;
        s.rules.wounds.reinforce = 1;
    });
    let q = spawn(&mut cap, 0, RIFLES, 500, 500);
    steps(&mut cap, 2);
    assert!(cap.kill_soldier(members(&cap, q)[7]));
    steps(&mut cap, 1);
    let pop = cap
        .game()
        .unwrap()
        .population(&cap.state.entities, PlayerId(0));
    cap.state.game.as_mut().unwrap().setup.rules.pop_cap = pop;
    assert!(
        !cmd(&mut cap, 1, CommandKind::Reinforce { squad: q }),
        "pop cap"
    );
}

fn wounds_on_reinforce_off() -> World {
    flat_world_with(1, |s| s.rules.wounds.enabled = 1)
}

/// W11 (row Reinforce, save/load mid-job): a save taken while the new men
/// walk from base plays on to the same hash. Control: the unsaved run.
#[test]
fn w11_save_load_while_replacements_walk() {
    let mut w = flat_world_with(1, |s| {
        s.rules.wounds.enabled = 1;
        s.rules.wounds.reinforce = 1;
    });
    let sq = spawn(&mut w, 0, RIFLES, 500, 500);
    steps(&mut w, 3);
    assert!(w.kill_soldier(members(&w, sq)[8]));
    steps(&mut w, 1);
    assert!(cmd(&mut w, 1, CommandKind::Reinforce { squad: sq }));
    steps(&mut w, 10 * 30);
    let mut x = World::load(&w.save(), 1).unwrap();
    for _ in 0..20 * 30 {
        w.step(&[]);
        x.step(&[]);
    }
    assert_eq!(x.full_state_hash(), w.full_state_hash());
}

/// S12 snapshot: a downed man is in `casualties` (not `corpses`) with his
/// tend; a wounded man's `soldier_wound` bits; an aid post's beds and
/// staff. Control: a fit man's bits are 0 and a dump has no staff.
#[test]
fn s12_snapshot_shows_casualties_wounds_and_beds() {
    use crate::snapshot::wound_bits;
    let (mut w, d, _, med) = scene(0, 503);
    assert!(cmd(
        &mut w,
        1,
        CommandKind::Tend {
            units: med.clone(),
            target: d
        }
    ));
    until(&mut w, 30 * 30, |w| cas(w, d).tend.is_some()).expect("tending");
    let s = w.snapshot();
    assert!(s.corpses.iter().all(|c| c.id != d));
    let k = s.casualties.iter().find(|c| c.id == d).expect("listed");
    assert!(!k.stabilised && k.bleed_ticks > 0 && k.tend != u32::MAX && k.tend_ticks > 0);
    let (pid, _) = post(&mut w, 400, 400);
    let m = wounded_to(&mut w, 430, 400, c(404, 404), 2);
    until(&mut w, 40 * 30, |w| !beds(w, pid).is_empty()).expect("a bed");
    w.step(&[]);
    let s = w.snapshot();
    let st = s.structures.iter().find(|t| t.id == pid).unwrap();
    assert_eq!(st.beds.len(), 1);
    assert_eq!(st.beds[0].man, m);
    assert!(st.beds[0].ticks_left > 0 && !st.beds[0].evac);
    assert_eq!(st.staff, 2);
    let i = s.soldier_ids.iter().position(|&x| x == m).unwrap();
    assert_eq!(
        s.soldier_wound[i],
        wound_bits::WOUNDED | wound_bits::BANDAGED
    );
    let fit = s.soldier_ids.iter().position(|&x| x == med[0]).unwrap();
    assert_eq!(s.soldier_wound[fit], 0, "control: fit");
    assert_eq!(s.soldier_wound.len(), s.soldier_ids.len());
}

// --------------------------------------------------- checker G1-G6, G9 --

/// Checker G1: a wounded man in a treatment bed who goes down leaves the
/// bed; the bed never makes a downed man Fit, and a save loads. Control:
/// an undisturbed man is treated (`w8_post_treats_once_...`).
#[test]
fn g1_downed_in_a_treatment_bed_leaves_it() {
    let mut w = world(0);
    let (pid, _) = post(&mut w, 400, 400);
    let m = wounded_to(&mut w, 430, 400, c(404, 404), 1);
    until(&mut w, 40 * 30, |w| beds(w, pid).iter().any(|b| b.man == m)).expect("a bed");
    assert!(w.down_soldier(m, (1, crate::game::kill_class::RIFLE)));
    steps(&mut w, 2);
    assert!(
        beds(&w, pid).iter().all(|b| b.man != m || b.evac),
        "left the bed"
    );
    steps(&mut w, 70 * 30);
    assert_ne!(kind(&w, m), EntityKind::Soldier, "never Fit again");
    assert!(!w.state.entities.combat[m.index()].wound.treated);
    assert_eq!(s12(&w, 0).treated, 0);
    assert!(World::load(&w.save(), 1).is_ok(), "loadable");
}

/// Checker G2: a man rejoined to his squad while in a treatment bed is
/// treated with his chunk marked: the incremental hash equals the full
/// one. Control: the same check before the treatment.
#[test]
fn g2_treatment_marks_the_mans_chunk() {
    let mut w = world(0);
    let (pid, _) = post(&mut w, 400, 400);
    let m = wounded_to(&mut w, 404, 398, c(404, 404), 1);
    until(&mut w, 40 * 30, |w| beds(w, pid).iter().any(|b| b.man == m)).expect("a bed");
    assert!(w.edit_unit(m, |u| u.rejoin()));
    w.step(&[]);
    assert_eq!(w.state_hash(), w.full_state_hash(), "control");
    until(&mut w, 70 * 30, |w| {
        w.state.entities.combat[m.index()].wound.treated
    })
    .expect("treated");
    assert_eq!(w.state_hash(), w.full_state_hash());
}

/// Checker G3: a medic mid-stabilise is no buddy: a suppressed wounded
/// man by him stays undressed and the tend goes on to stabilise. Control:
/// a plain detached rifleman by him dresses him (`w6_buddy_...`).
#[test]
fn g3_a_tending_medic_is_no_buddy() {
    let (mut w, d, sq, med) = scene(0, 503);
    assert!(cmd(
        &mut w,
        1,
        CommandKind::Tend {
            units: med,
            target: d
        }
    ));
    until(&mut w, 30 * 30, |w| cas(w, d).tend.is_some()).expect("a tend starts");
    let (m, _) = cas(&w, d).tend.unwrap();
    let hurt = members(&w, sq)[0];
    w.edit_soldier(hurt, |_, c| {
        c.hp = 60;
        c.wound.state = WOUNDED;
    });
    let at = pos(&w, m) + c(1, 0);
    assert!(cmd(
        &mut w,
        2,
        CommandKind::MoveUnits {
            units: vec![hurt],
            target: at
        }
    ));
    let crouch = w.match_setup().unwrap().rules.combat.crouch_suppression;
    let mut stabilised = false;
    for _ in 0..30 * 30 {
        w.state.entities.combat[hurt.index()].suppression = (crouch + 5) * 1000;
        w.step(&[]);
        stabilised |= cas(&w, d).bleed_out_at == 0;
        if !stabilised {
            assert!(w.state.entities.unit[m.index()].detached, "kept tending");
        }
    }
    assert!(stabilised);
    assert!(!w.state.entities.combat[hurt.index()].wound.bandaged);
}

/// Checker G4: the men on a job rejoin when it ends early: a casualty
/// killed mid-tend frees his medic; one dying mid-carry frees his
/// carriers; a wounded man who dies, or dresses himself, before his buddy
/// arrives frees the buddy. Control: the tender of a living casualty
/// stays detached.
#[test]
fn g4_job_men_rejoin_when_the_job_ends() {
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
    let (m, _) = cas(&w, d).tend.unwrap();
    assert!(w.state.entities.unit[m.index()].detached, "control");
    super::s12_tests::bullet(&mut w, 1, "ww2:kar98k", d, crate::wounds::ZONE_TORSO);
    steps(&mut w, 2);
    assert!(!w.state.entities.unit[m.index()].detached, "medic rejoined");
    // Dies mid-carry.
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
            units: med.clone(),
            target: d,
            to: c(500, 300)
        }
    ));
    steps(&mut w, 25 * 30);
    assert_eq!(kind(&w, d), EntityKind::Corpse);
    for &m in &med {
        assert!(
            !w.state.entities.unit[m.index()].detached,
            "carrier rejoined"
        );
    }
    // A buddy on his way: the man dies, or dresses himself.
    for dies in [true, false] {
        let mut w = world(0);
        let sq = spawn(&mut w, 0, RIFLES, 500, 500);
        let far = spawn(&mut w, 0, RIFLES, 500, 560);
        steps(&mut w, 3);
        let hurt = members(&w, sq)[2];
        let b = members(&w, far)[0];
        w.edit_soldier(hurt, |_, c| {
            c.hp = 40;
            c.wound.state = WOUNDED;
        });
        assert!(cmd(
            &mut w,
            1,
            CommandKind::Tend {
                units: vec![b],
                target: hurt
            }
        ));
        steps(&mut w, 30);
        assert!(w.state.entities.unit[b.index()].detached, "on his way");
        if dies {
            assert!(w.kill_soldier(hurt));
        } else {
            until(&mut w, 20 * 30, |w| {
                w.state.entities.combat[hurt.index()].wound.bandaged
            })
            .expect("dressed himself");
            assert!(
                (pos(&w, b) - pos(&w, hurt)).length_sq() > Fx::from_int(9),
                "before the buddy came"
            );
        }
        steps(&mut w, 2);
        assert!(!w.state.entities.unit[b.index()].detached, "buddy rejoined");
    }
}

/// Checker G5: `Reinforce` on a holding squad keeps its slots and adds
/// one per man; the new men walk detached (not in the column). Control:
/// the old men's slots are the ones before.
#[test]
fn g5_reinforce_keeps_the_squads_slots() {
    let mut w = flat_world_with(1, |s| {
        s.rules.wounds.enabled = 1;
        s.rules.wounds.reinforce = 1;
    });
    let sq = spawn(&mut w, 0, RIFLES, 500, 500);
    steps(&mut w, 3);
    let ms = members(&w, sq);
    assert!(w.kill_soldier(ms[8]));
    steps(&mut w, 60);
    let before = w.game().unwrap().squads[&sq].nav.clone();
    assert!(matches!(
        before.phase,
        crate::game::NavPhase::Holding | crate::game::NavPhase::Arriving
    ));
    assert!(cmd(&mut w, 1, CommandKind::Reinforce { squad: sq }));
    let after = &w.game().unwrap().squads[&sq].nav;
    assert_eq!(after.phase, before.phase);
    assert_eq!(after.slots.len(), before.slots.len() + 1);
    assert_eq!(after.slots[..before.slots.len()], before.slots[..]);
    let new = *members(&w, sq).last().unwrap();
    assert!(w.state.entities.unit[new.index()].detached);
    assert!(World::load(&w.save(), 1).is_ok(), "loadable");
}

