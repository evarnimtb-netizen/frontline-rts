//! S6 tests (docs/slices/S6-hq-crates.md "Targets", the cargo-test rows):
//! T12 determinism and save/load, T20-T26 rules, and one hash and one
//! load check per saved S6 field.

use frontline_content::setup::{StockKind, SupplyKind};

use super::*;
use crate::fx::FxVec3;
use crate::game::SquadId;
use crate::structures::{Crate, S6Account, Structure, SupplyRefusal, SupplyState};

pub(super) const HZ: u64 = 30;

/// The ww2 mod on flat open ground, both players human.
pub(super) fn flat_world(threads: usize) -> World {
    flat_world_with(threads, |_| {})
}

/// [`flat_world`] with an edit of the setup (after the humans are set).
pub(super) fn flat_world_with(
    threads: usize,
    edit: impl FnOnce(&mut frontline_content::MatchSetup),
) -> World {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2");
    let defs = frontline_content::ModDefs::load(&root).unwrap();
    let req = frontline_content::MatchRequest::skirmish(
        1,
        "ww2:germany",
        "ww2:usa",
        frontline_content::Difficulty::Normal,
    );
    let mut setup = defs.resolve(&req).unwrap();
    setup.players.iter_mut().for_each(|p| p.is_ai = false);
    setup.map.terrain.octaves.clear();
    setup.map.terrain.bands.clear();
    setup.map.woods.clear();
    // S11b Build C: the tests on this world were written with every S11b
    // switch off (shipped on since Build C); the S11b tests turn on what
    // they test.
    setup.s11b_off();
    setup.s12_off();
    setup.s13_off();
    edit(&mut setup);
    World::new_match(setup, threads).unwrap()
}

fn ty(w: &World, id: &str) -> u16 {
    w.match_setup().unwrap().squad_type_index(id).unwrap()
}

pub(super) fn def(w: &World, faction: u16, kind: StockKind, supply: Option<SupplyKind>) -> u16 {
    w.match_setup()
        .unwrap()
        .stock_of(faction, kind, supply)
        .unwrap()
}

pub(super) fn spawn(w: &mut World, p: u8, card: &str, x: i32, y: i32) -> SquadId {
    let t = ty(w, card);
    w.spawn_squad_at(
        PlayerId(p),
        t,
        FxVec2::from_int(x, y),
        FxVec2::new(Fx::ONE, Fx::ZERO),
    )
    .unwrap()
}

pub(super) fn run(w: &mut World, p: u8, seq: u32, kind: CommandKind) {
    w.step(&[Command {
        player: PlayerId(p),
        seq,
        kind,
    }]);
}

pub(super) fn steps(w: &mut World, n: u64) {
    for _ in 0..n {
        w.step(&[]);
    }
}

pub(super) fn members(w: &World, s: SquadId) -> Vec<SimId> {
    w.game().unwrap().squads[&s].members.clone()
}

/// The squad's mortar bombs (all men, tubes included).
fn bombs(w: &World, s: SquadId) -> u32 {
    let g = w.game().unwrap();
    crate::supply::mortar_bombs(g.setup(), &w.state.entities, &g.squads[&s]).0
}

/// Empties every man's bombs and magazine.
fn empty_bombs(w: &mut World, s: SquadId) {
    for m in members(w, s) {
        w.edit_soldier(m, |k, c| {
            k.rounds = [0; frontline_content::setup::MAX_FEEDS];
            c.mag = 0;
        });
    }
}

fn base_center(w: &World, p: u8) -> FxVec2 {
    w.game().unwrap().base_of(PlayerId(p))
}

/// A built structure of def `d` for player `p` at `pos` with `stock_kg`.
pub(super) fn built(w: &World, p: u8, d: u16, pos: FxVec2, stock_kg: [u32; 3]) -> Structure {
    let sd = &w.match_setup().unwrap().stocks[d as usize];
    let hp = |k: usize| sd.boxes.get(k).map_or(0, |b| b.hp);
    Structure {
        owner: PlayerId(p),
        def: d,
        pos,
        yaw: Fx::ZERO,
        builder: None,
        built: true,
        box_hp: [hp(0), hp(1), hp(2), hp(3)],
        radio: true,
        stock_g: stock_kg.map(|k| k * 1000),
        beds: Vec::new(),
    }
}

/// A set-down crate of def `d` at `pos`.
fn crate_at(w: &World, p: u8, d: u16, pos: FxVec2) -> Crate {
    let sd = &w.match_setup().unwrap().stocks[d as usize];
    Crate {
        owner: PlayerId(p),
        def: d,
        pos,
        carrier: None,
        dest: None,
        claim: None,
        g: sd.g,
        hp: sd.boxes[0].hp,
    }
}

pub(super) fn add_built(w: &mut World, p: u8, d: u16, pos: FxVec2, stock_kg: [u32; 3]) -> u32 {
    let s = built(w, p, d, pos, stock_kg);
    add_structure(w, s)
}

pub(super) fn add_crate_at(w: &mut World, p: u8, d: u16, pos: FxVec2) -> u32 {
    let c = crate_at(w, p, d, pos);
    add_crate(w, c)
}

pub(super) fn add_structure(w: &mut World, s: Structure) -> u32 {
    let mut id = 0;
    w.edit_supply6(|st| {
        id = st.next_structure;
        st.next_structure += 1;
        st.structures.insert(id, s);
    });
    id
}

fn add_crate(w: &mut World, c: Crate) -> u32 {
    let mut id = 0;
    w.edit_supply6(|st| {
        id = st.next_crate;
        st.next_crate += 1;
        st.crates.insert(id, c);
    });
    id
}

pub(super) fn acc(w: &World, p: usize) -> S6Account {
    w.game().unwrap().supply6().accounts[p]
}

// ------------------------------------------------------------------ T20 --

#[test]
fn t20_the_base_refills_no_bombs_and_a_dry_mortar_stays_dry() {
    let mut w = flat_world(1);
    let b = base_center(&w, 0);
    let home = spawn(
        &mut w,
        0,
        "ww2:ger_mortar_team",
        b.x.floor_int(),
        b.y.floor_int(),
    );
    let out = spawn(&mut w, 0, "ww2:ger_mortar_team", 500, 500);
    empty_bombs(&mut w, home);
    empty_bombs(&mut w, out);
    steps(&mut w, 60 * HZ);
    assert_eq!(bombs(&w, home), 0, "the base refilled bombs");
    assert_eq!(bombs(&w, out), 0, "a mortar with no source got bombs");
    let a = acc(&w, 0);
    assert_eq!((a.base_bombs, a.unsourced_bombs), (0, 0));
    // Small arms still refill at base (S5a unchanged): the mortar's
    // riflemen were full, so nothing to check beyond no bombs.
}

// ------------------------------------------------------------------ T21 --

#[test]
fn t21_a_crate_within_5_m_feeds_a_mortar_until_it_is_empty() {
    let mut w = flat_world(1);
    let m = spawn(&mut w, 0, "ww2:ger_mortar_team", 500, 500);
    steps(&mut w, 2 * HZ);
    empty_bombs(&mut w, m);
    let gunner = members(&w, m)[1];
    let at = w.state.entities.transform[gunner.index()].pos.xy();
    let d = def(&w, 0, StockKind::Crate, Some(SupplyKind::Shells));
    let c = add_crate_at(&mut w, 0, d, at + FxVec2::from_int(3, 0));
    // 32 kg holds 9 bombs of 3.5 kg; one per crate_bomb_s (4 s).
    steps(&mut w, 30 * HZ);
    let got = bombs(&w, m);
    assert!(
        (7..=8).contains(&got),
        "7-8 bombs in 30 s at 4 s each, got {got}"
    );
    steps(&mut w, 15 * HZ);
    assert_eq!(bombs(&w, m), 9);
    let left = w.game().unwrap().supply6().crates[&c].g;
    assert_eq!(left, 32_000 - 9 * 3_500, "the crate lost the bombs' grams");
    steps(&mut w, 10 * HZ);
    assert_eq!(
        bombs(&w, m),
        9,
        "a crate too light for a bomb feeds no more"
    );
    let a = acc(&w, 0);
    assert_eq!((a.crate_bombs, a.unsourced_bombs, a.base_bombs), (9, 0, 0));
    // The same crate 10 m away feeds nothing.
    let m2 = spawn(&mut w, 0, "ww2:ger_mortar_team", 700, 700);
    steps(&mut w, 2 * HZ);
    empty_bombs(&mut w, m2);
    let g2 = members(&w, m2)[1];
    let at2 = w.state.entities.transform[g2.index()].pos.xy();
    add_crate_at(&mut w, 0, d, at2 + FxVec2::from_int(10, 0));
    steps(&mut w, 20 * HZ);
    assert_eq!(bombs(&w, m2), 0);
}

#[test]
fn t21_an_hq_radius_feeds_a_mortar_at_the_structure_rate() {
    let mut w = flat_world(1);
    let m = spawn(&mut w, 0, "ww2:ger_mortar_team", 500, 500);
    steps(&mut w, 2 * HZ);
    empty_bombs(&mut w, m);
    let hq = def(&w, 0, StockKind::Hq, None);
    let s = built(&w, 0, hq, FxVec2::from_int(510, 500), [0, 0, 600]);
    add_structure(&mut w, s);
    steps(&mut w, 20 * HZ);
    // One per structure_bomb_s (2 s): 10 in 20 s, give or take the phase.
    let got = bombs(&w, m);
    assert!((9..=10).contains(&got), "got {got}");
    let a = acc(&w, 0);
    assert_eq!(a.structure_bombs, got);
    assert_eq!(a.unsourced_bombs, 0);
}

// ------------------------------------------------------------------ T22 --

#[test]
fn t22_stock_enters_only_from_crates_set_down_in_the_radius() {
    let mut w = flat_world(1);
    let hq = def(&w, 0, StockKind::Hq, None);
    let id = add_built(&mut w, 0, hq, FxVec2::from_int(500, 500), [0; 3]);
    steps(&mut w, 30 * HZ);
    assert_eq!(
        w.game().unwrap().supply6().structures[&id].stock_g,
        [0; 3],
        "stock from nowhere"
    );
    let d = def(&w, 0, StockKind::Crate, Some(SupplyKind::Shells));
    let c = add_crate_at(&mut w, 0, d, FxVec2::from_int(505, 505));
    w.step(&[]);
    let st = w.game().unwrap().supply6();
    assert_eq!(st.structures[&id].stock_g, [0, 0, 32_000]);
    assert!(!st.crates.contains_key(&c), "an emptied crate is gone");
    assert_eq!(acc(&w, 0).stock_in_g, 32_000);
    // Up to the cap: the rest stays in the crate.
    w.edit_supply6(|s| s.structures.get_mut(&id).unwrap().stock_g[2] = 590_000);
    let c2 = add_crate_at(&mut w, 0, d, FxVec2::from_int(505, 505));
    w.step(&[]);
    let st = w.game().unwrap().supply6();
    assert_eq!(st.structures[&id].stock_g[2], 600_000);
    assert_eq!(st.crates[&c2].g, 22_000);
}

// ------------------------------------------------------------------ T23 --

#[test]
fn t23_only_he_sets_off_a_cook_off() {
    let mut w = flat_world(1);
    let d = def(&w, 0, StockKind::Crate, Some(SupplyKind::Shells));
    let a = add_crate_at(&mut w, 0, d, FxVec2::from_int(500, 500));
    let b = add_crate_at(&mut w, 0, d, FxVec2::from_int(520, 500));
    w.step(&[]);
    let tick = w.tick();
    let g = w.state.game.as_mut().unwrap();
    let k = |g: &MatchState, id: u32| {
        g.static_boxes()
            .iter()
            .position(|s| s.of == crate::structures::StaticRef::Crate(id))
            .unwrap()
    };
    let (ka, kb) = (k(g, a), k(g, b));
    // A bullet stream breaks crate a: no cook-off.
    g.hit_static(ka, 1000, None, tick);
    assert!(!g.supply6.crates.contains_key(&a));
    assert!(g.supply6.cookoffs.is_empty());
    // HE breaks crate b: 32 kg / 16 = 2 bursts, owned by player 1.
    g.hit_static(kb, 1000, Some((1, crate::game::kill_class::MORTAR)), tick);
    assert_eq!(g.supply6.cookoffs.len(), 1);
    assert_eq!(g.supply6.cookoffs[0].left, 2);
    assert_eq!(g.supply6.cookoffs[0].player, 1);
    assert_eq!(g.supply6.accounts[0].cookoffs, 1);
    assert_eq!(g.supply6.accounts[0].crates_lost, 2);
    let w2 = World::from_state(w.state.clone(), 1).unwrap();
    let mut w = w2;
    let before = w.state.stats.explosions;
    steps(&mut w, 8 * HZ);
    assert_eq!(w.state.stats.explosions - before, 2, "two cook-off bursts");
    assert!(w.game().unwrap().supply6().cookoffs.is_empty());
}

#[test]
fn t23_a_bullet_stops_at_a_box() {
    // A rifle round fired through an enemy crate stops on it and takes
    // bullet_box_damage.
    let mut w = flat_world(1);
    let d = def(&w, 0, StockKind::Crate, Some(SupplyKind::Shells));
    let c = add_crate_at(&mut w, 0, d, FxVec2::from_int(500, 500));
    w.step(&[]);
    let s = w.match_setup().unwrap();
    let wk = s
        .weapons
        .iter()
        .position(|x| x.id == "ww2:m1_garand")
        .unwrap() as u16;
    let h = w.state.terrain.height_at(FxVec2::from_int(480, 500));
    let from = FxVec3::new(
        Fx::from_int(480),
        Fx::from_int(500),
        h + Fx::from_milli(200),
    );
    let slot = w
        .state
        .projectiles
        .spawn(crate::projectile::Projectile {
            pos: from,
            vel: FxVec3::new(Fx::from_int(800), Fx::ZERO, Fx::ZERO),
            owner: SimId(9999),
            drag: Fx::ZERO,
            ttl: 120,
            weapon: wk,
            player: 1,
            ammo: crate::projectile::NO_AMMO,
            origin: from,
            mission: 0,
        })
        .unwrap();
    w.hash.mark_pool_slot(slot);
    steps(&mut w, 3);
    let hp = w.game().unwrap().supply6().crates[&c].hp;
    assert_eq!(hp, 60 - 4, "the bullet took bullet_box_damage");
}

/// Spawns a round of `weapon` (and `ammo`, `None`: small arms) for
/// `player` at `from` with velocity `vel` (review S6 F6: staged real
/// projectiles, not direct `hit_static` calls).
fn fire(w: &mut World, player: u8, weapon: &str, ammo: Option<&str>, from: FxVec3, vel: FxVec3) {
    let s = w.match_setup().unwrap();
    let wk = s.weapons.iter().position(|x| x.id == weapon).unwrap() as u16;
    let ak = ammo.map_or(crate::projectile::NO_AMMO, |a| {
        s.ammo.iter().position(|x| x.id == a).unwrap() as u16
    });
    let slot = w
        .state
        .projectiles
        .spawn(crate::projectile::Projectile {
            pos: from,
            vel,
            owner: SimId(9999),
            drag: Fx::ZERO,
            ttl: 120,
            weapon: wk,
            player,
            ammo: ak,
            origin: from,
            mission: 0,
        })
        .unwrap();
    w.hash.mark_pool_slot(slot);
}

/// 20 cm above the ground at (x, y).
fn low(w: &World, x: i32, y: i32) -> FxVec3 {
    let h = w.state.terrain.height_at(FxVec2::from_int(x, y));
    FxVec3::new(Fx::from_int(x), Fx::from_int(y), h + Fx::from_milli(200))
}

#[test]
fn t23_an_he_burst_beside_a_crate_cooks_it_off_and_ap_does_not() {
    let mut w = flat_world(1);
    let d = def(&w, 0, StockKind::Crate, Some(SupplyKind::Shells));
    let a = add_crate_at(&mut w, 0, d, FxVec2::from_int(500, 500));
    let b = add_crate_at(&mut w, 0, d, FxVec2::from_int(500, 520));
    w.step(&[]);
    // An enemy 75 mm HE shell falls on the ground 0.8 m from crate a: the
    // Damage stage's overpressure and fragments break it, and it cooks off.
    let h = w.state.terrain.height_at(FxVec2::from_int(500, 500));
    let from = FxVec3::new(
        Fx::from_ratio(4992, 10),
        Fx::from_int(500),
        h + Fx::from_int(3),
    );
    fire(
        &mut w,
        1,
        "ww2:m3_75mm",
        Some("ww2:75mm_m48"),
        from,
        FxVec3::new(Fx::ZERO, Fx::ZERO, Fx::from_int(-300)),
    );
    steps(&mut w, 3);
    let g = w.game().unwrap().supply6();
    assert!(!g.crates.contains_key(&a), "the HE burst broke crate a");
    assert_eq!(g.accounts[0].cookoffs, 1, "and it cooked off");
    // An enemy 75 mm AP round straight into crate b: shell_box_damage
    // breaks it (60 hp), but solid shot sets nothing off.
    let from = low(&w, 480, 520);
    fire(
        &mut w,
        1,
        "ww2:m3_75mm",
        Some("ww2:75mm_m61"),
        from,
        FxVec3::new(Fx::from_int(600), Fx::ZERO, Fx::ZERO),
    );
    steps(&mut w, 3);
    let g = w.game().unwrap().supply6();
    assert!(!g.crates.contains_key(&b), "the AP round broke crate b");
    assert_eq!(g.accounts[0].crates_lost, 2);
    assert_eq!(g.accounts[0].cookoffs, 1, "no cook-off from solid shot");
}

#[test]
fn f1_one_ap_round_does_not_destroy_an_hq() {
    // Review S6 F1: shell_box_damage stays well below the stock box's hp.
    let mut w = flat_world(1);
    let hq = def(&w, 0, StockKind::Hq, None);
    let id = add_built(&mut w, 0, hq, FxVec2::from_int(500, 500), [100, 50, 300]);
    w.step(&[]);
    // The stock box (part 0) lies at x -0.3 m along the facing (+x).
    let from = FxVec3::new(
        Fx::from_ratio(4997, 10),
        Fx::from_int(480),
        w.state.terrain.height_at(FxVec2::from_int(500, 500)) + Fx::from_ratio(1, 2),
    );
    fire(
        &mut w,
        1,
        "ww2:m3_75mm",
        Some("ww2:75mm_m61"),
        from,
        FxVec3::new(Fx::ZERO, Fx::from_int(600), Fx::ZERO),
    );
    steps(&mut w, 3);
    let g = w.game().unwrap().supply6();
    let s = g
        .structures
        .get(&id)
        .expect("one AP round destroyed the HQ");
    let full = w.match_setup().unwrap().stocks[hq as usize].boxes[0].hp;
    let shell = w
        .match_setup()
        .unwrap()
        .rules
        .supply
        .as_ref()
        .unwrap()
        .shell_box_damage;
    assert_eq!(s.box_hp[0], full - shell, "the round struck the stock box");
    assert_eq!(s.stock_g[2], 300_000, "the stock is intact");
}

#[test]
fn f3_ai_bearers_carry_crates_to_a_far_tank_that_waits_for_them() {
    // Review S6 F3 (owner 2026-11-01): an AI tank low on rounds, mobile,
    // far from the base and any HQ, is not driven back while the bearers
    // carry shells crates to it; it rearms from them where it stands.
    let mut w = flat_world_with(1, |s| s.players[0].is_ai = true);
    let base = base_center(&w, 0);
    let mid = FxVec2::from_int(512, 512);
    let at = base + (mid - base).normalize_or_zero() * Fx::from_int(400);
    let tt = ty(&w, "ww2:ger_pz4_h_squad");
    let sq = w
        .spawn_squad_at(PlayerId(0), tt, at, FxVec2::new(Fx::ONE, Fx::ZERO))
        .unwrap();
    let v = members(&w, sq)[0];
    w.edit_vehicle(v, |vs| {
        vs.ammo.iter_mut().for_each(|a| *a = (*a).min(1));
        vs.loaded = None;
    });
    let bt = ty(&w, "ww2:ger_munitionstraeger");
    let near = at - (mid - base).normalize_or_zero() * Fx::from_int(40);
    let b = w
        .spawn_squad_at(PlayerId(0), bt, near, FxVec2::new(Fx::ONE, Fx::ZERO))
        .unwrap();
    let d = def(&w, 0, StockKind::Crate, Some(SupplyKind::Shells));
    for m in members(&w, b) {
        let mut c = crate_at(&w, 0, d, near);
        c.carrier = Some(m);
        add_crate(&mut w, c);
    }
    let rounds = |w: &World| -> u32 {
        let vs = &w.game().unwrap().vehicles[&v];
        vs.ammo.iter().map(|&a| a as u32).sum::<u32>() + u32::from(vs.loaded.is_some())
    };
    let start = rounds(&w);
    let mut t = 0;
    // Four crates of 32 kg: the tank gets six rounds before any trip.
    while rounds(&w) < start + 6 {
        w.step(&[]);
        t += 1;
        assert!(
            t < 240 * HZ,
            "the tank got no rounds from the crates in 4 min"
        );
        let g = w.game().unwrap();
        let ai = g.ai[0].as_ref().unwrap();
        assert_ne!(
            ai.tasks.get(&sq).map(|k| k.kind),
            Some(crate::ai::TaskKind::Rearm),
            "the tank was sent back to rearm"
        );
    }
    let g = w.game().unwrap();
    assert!(g.ai[0].as_ref().unwrap().counters.tank_crate_runs >= 1);
    let n = members(&w, b).len() as u64;
    let left: u64 = g.supply6().crates.values().map(|c| c.g as u64).sum();
    let full = w.match_setup().unwrap().stocks[d as usize].g as u64;
    assert!(left < n * full, "the rounds came from the crates");
    assert_eq!(
        acc(&w, 0).crates_delivered,
        n as u32,
        "every crate set down"
    );
}

#[test]
fn f2_ai_bearers_pick_up_a_stranded_shells_crate() {
    // Review S6 F2: a set-down own shells crate with no mortar or tank in
    // reach, outside the base and every radius, is taken up again.
    let mut w = flat_world_with(1, |s| s.players[0].is_ai = true);
    let base = base_center(&w, 0);
    let mid = FxVec2::from_int(512, 512);
    let away = (mid - base).normalize_or_zero();
    let bt = ty(&w, "ww2:ger_munitionstraeger");
    let at = base + away * Fx::from_int(250);
    let b = w
        .spawn_squad_at(PlayerId(0), bt, at, FxVec2::new(Fx::ONE, Fx::ZERO))
        .unwrap();
    let d = def(&w, 0, StockKind::Crate, Some(SupplyKind::Shells));
    let c = add_crate_at(&mut w, 0, d, at + away * Fx::from_int(30));
    let mut t = 0;
    loop {
        w.step(&[]);
        t += 1;
        let g = w.game().unwrap();
        let carrier = g.supply6().crates.get(&c).and_then(|x| x.carrier);
        if carrier.is_some_and(|m| members(&w, b).contains(&m)) {
            break;
        }
        assert!(t < 90 * HZ, "the stranded crate was not picked up in 90 s");
    }
    let ai = w.game().unwrap().ai[0].as_ref().unwrap();
    assert_eq!(ai.counters.s6_crate_pickups, 1);
}

// ------------------------------------------------------------------ T24 --

#[test]
fn t24_a_tank_at_an_hq_rearms_at_the_d3_rates_from_the_stock() {
    let mut w = flat_world(1);
    let sq = spawn(&mut w, 0, "ww2:ger_pz4_h_squad", 500, 500);
    let v = members(&w, sq)[0];
    let hq = def(&w, 0, StockKind::Hq, None);
    let id = add_built(&mut w, 0, hq, FxVec2::from_int(505, 500), [0, 0, 600]);
    w.edit_vehicle(v, |vs| {
        vs.ammo.iter_mut().for_each(|a| *a = 0);
        vs.loaded = None;
    });
    let t0 = w.tick();
    let n = 61 * HZ;
    steps(&mut w, n);
    // Due ticks: (tick + id) % every == 0 over the ticks stepped.
    let due = |every: u64| {
        (t0..t0 + n)
            .filter(|t| (t + v.0 as u64).is_multiple_of(every))
            .count() as u32
    };
    let a = acc(&w, 0);
    // Rounds in the rack plus the one in the breech.
    let vs = &w.game().unwrap().vehicles[&v];
    let mut per: Vec<u32> = vs.ammo.iter().map(|&x| x as u32).collect();
    if let Some(l) = vs.loaded {
        per[l as usize] += 1;
    }
    let held: u32 = per.iter().sum();
    assert_eq!(a.tank_rounds_fwd, due(4 * HZ), "one standard round per 4 s");
    assert_eq!(
        a.tank_rare_fwd,
        due(30 * HZ).min(2),
        "one rare round per 30 s"
    );
    assert_eq!(held, a.tank_rounds_fwd + a.tank_rare_fwd);
    // The stock fell by the rounds' grams.
    let s = w.match_setup().unwrap();
    let vt = &s.vehicles[w.game().unwrap().vehicles[&v].type_idx as usize];
    let ammo = &vt.main_ammo();
    let mut g = 0u32;
    for (k, &n) in per.iter().enumerate() {
        g += n * s.ammo[ammo[k].ammo as usize].supply.unwrap().g;
    }
    let stock = w.game().unwrap().supply6().structures[&id].stock_g[2];
    assert_eq!(stock, 600_000 - g);
    // An empty HQ rearms nothing.
    w.edit_supply6(|st| st.structures.get_mut(&id).unwrap().stock_g = [0; 3]);
    let before = acc(&w, 0).tank_rounds_fwd;
    steps(&mut w, 20 * HZ);
    assert_eq!(acc(&w, 0).tank_rounds_fwd, before);
}

// ------------------------------------------------------------------ T25 --

#[test]
fn t25_caps_and_a_dead_carrier() {
    let mut w = flat_world(1);
    let hq = def(&w, 0, StockKind::Hq, None);
    let dump = def(&w, 0, StockKind::Dump, None);
    add_built(&mut w, 0, hq, FxVec2::from_int(400, 400), [0; 3]);
    for k in 0..3 {
        add_built(&mut w, 0, dump, FxVec2::from_int(420 + 30 * k, 400), [0; 3]);
    }
    let zt = spawn(&mut w, 0, "ww2:ger_zugtrupp", 600, 600);
    let mut seq = 1;
    for (d, why) in [(hq, SupplyRefusal::Cap), (dump, SupplyRefusal::Cap)] {
        run(
            &mut w,
            0,
            seq,
            CommandKind::Build {
                squad: zt,
                def: d,
                pos: FxVec2::from_int(620, 600),
                facing: FxVec2::new(Fx::ONE, Fx::ZERO),
            },
        );
        seq += 1;
        assert_eq!(w.snapshot().supply_refusals[0].refusal, why);
    }
    // 12 live crates: the 13th is refused.
    let sa = def(&w, 0, StockKind::Crate, Some(SupplyKind::SmallArms));
    for k in 0..12 {
        add_crate_at(&mut w, 0, sa, FxVec2::from_int(300 + k, 300));
    }
    let b = base_center(&w, 0);
    let bearers = spawn(
        &mut w,
        0,
        "ww2:ger_munitionstraeger",
        b.x.floor_int(),
        b.y.floor_int(),
    );
    run(
        &mut w,
        0,
        seq,
        CommandKind::IssueCrate {
            squad: bearers,
            kind: SupplyKind::SmallArms,
        },
    );
    seq += 1;
    assert_eq!(w.snapshot().supply_refusals[0].refusal, SupplyRefusal::Cap);
    // A dead carrier delivers nothing: his crate lies where he fell.
    w.edit_supply6(|s| s.crates.clear());
    run(
        &mut w,
        0,
        seq,
        CommandKind::IssueCrate {
            squad: bearers,
            kind: SupplyKind::Shells,
        },
    );
    assert!(w.snapshot().supply_refusals.is_empty());
    let (cid, carrier) = {
        let (id, c) = w.game().unwrap().supply6().crates.iter().next().unwrap();
        (*id, c.carrier.unwrap())
    };
    w.kill_soldier(carrier);
    w.step(&[]);
    let c = &w.game().unwrap().supply6().crates[&cid];
    assert_eq!(c.carrier, None);
    assert_eq!(acc(&w, 0).crates_delivered, 0);
}

// ------------------------------------------------------------------ T26 --

#[test]
fn t26_a_crate_costs_kg_times_price_and_draws_are_free() {
    let mut w = flat_world(1);
    let b = base_center(&w, 0);
    let bearers = spawn(
        &mut w,
        0,
        "ww2:ger_munitionstraeger",
        b.x.floor_int(),
        b.y.floor_int(),
    );
    w.edit_player(PlayerId(0), |p| {
        p.manpower = 100;
        p.ammo.credit = 0;
    });
    run(
        &mut w,
        0,
        1,
        CommandKind::IssueCrate {
            squad: bearers,
            kind: SupplyKind::Shells,
        },
    );
    // 32 kg x 1000 MP per t = 32 MP (plus the income of one tick).
    let g = w.game().unwrap();
    let paid = g.players()[0].ammo.paid[SupplyKind::Shells.index()];
    assert_eq!(paid, 32_000_000, "32 MP in µMP");
    assert_eq!(acc(&w, 0).crate_micro, 32_000_000);
    assert_eq!(acc(&w, 0).crates_issued, [0, 0, 1]);
    // No manpower: no crate.
    w.edit_player(PlayerId(0), |p| {
        p.manpower = 0;
        p.ammo.credit = 0;
    });
    steps(&mut w, 11 * HZ);
    w.edit_player(PlayerId(0), |p| p.manpower = 0);
    run(
        &mut w,
        0,
        2,
        CommandKind::IssueCrate {
            squad: bearers,
            kind: SupplyKind::Shells,
        },
    );
    assert!(matches!(
        w.snapshot().supply_refusals[0].refusal,
        SupplyRefusal::Manpower { .. }
    ));
    // Draws are free: a mortar fed from a crate costs nothing more.
    let m = spawn(&mut w, 0, "ww2:ger_mortar_team", 500, 500);
    steps(&mut w, 2 * HZ);
    empty_bombs(&mut w, m);
    let gunner = members(&w, m)[1];
    let at = w.state.entities.transform[gunner.index()].pos.xy();
    let d = def(&w, 0, StockKind::Crate, Some(SupplyKind::Shells));
    add_crate_at(&mut w, 0, d, at + FxVec2::from_int(2, 0));
    let paid0 = w.game().unwrap().players()[0].ammo.paid;
    steps(&mut w, 12 * HZ);
    assert!(bombs(&w, m) >= 2);
    assert_eq!(w.game().unwrap().players()[0].ammo.paid, paid0);
}

// ------------------------------------------------------------ build/retreat --

#[test]
fn build_digs_the_pit_then_the_hq_stands_and_retreat_goes_there() {
    let mut w = flat_world(1);
    let zt = spawn(&mut w, 0, "ww2:ger_zugtrupp", 400, 400);
    let hq = def(&w, 0, StockKind::Hq, None);
    w.edit_player(PlayerId(0), |p| p.manpower = 500);
    run(
        &mut w,
        0,
        1,
        CommandKind::Build {
            squad: zt,
            def: hq,
            pos: FxVec2::from_int(410, 400),
            facing: FxVec2::new(Fx::ZERO, Fx::ONE),
        },
    );
    assert!(w.snapshot().supply_refusals.is_empty());
    let mp = w.game().unwrap().players()[0].manpower;
    assert!(mp < 500 - 100 + 5, "the HQ's 100 MP were paid");
    let id = *w
        .game()
        .unwrap()
        .supply6()
        .structures
        .keys()
        .next()
        .unwrap();
    let mut t = 0;
    while !w.game().unwrap().supply6().structures[&id].built {
        w.step(&[]);
        t += 1;
        assert!(t < 6 * 60 * HZ, "the HQ pit was not dug in 6 min");
    }
    // The pit is dug and has a parapet all round.
    let terrain = &w.state.terrain;
    let c = FxVec2::from_int(410, 400);
    assert!(terrain.base_height_at(c) - terrain.height_at(c) >= Fx::from_ratio(13, 10));
    // Review S6 F1: both boxes stand on the floor, their tops at least
    // 30 cm below the ground the pit was dug in.
    w.step(&[]);
    let terrain = &w.state.terrain;
    let boxes: Vec<_> = w
        .game()
        .unwrap()
        .static_boxes()
        .iter()
        .filter(|b| matches!(b.of, crate::structures::StaticRef::Structure(i, _) if i == id))
        .map(|b| b.frame)
        .collect();
    assert_eq!(boxes.len(), 2);
    for f in boxes {
        let top = f.center.z + f.half.z;
        assert!(
            top < terrain.base_height_at(f.center.xy()) - Fx::from_ratio(3, 10),
            "a box stands above the pit"
        );
    }
    assert_eq!(acc(&w, 0).structures_built, 1);
    assert!(acc(&w, 0).hq_built_at > 0);
    // A retreat from 90 m ahead of the HQ (away from the base) goes to the
    // HQ; from 90 m behind it, to the base (the HQ is not rearward); a
    // tank crew always goes to the base (review F4).
    let base = base_center(&w, 0);
    let away = (c - base).normalize_or_zero();
    let ahead = c + away * Fx::from_int(90);
    let behind = c - away * Fx::from_int(90);
    let sp = |w: &mut World, card: &str, at: FxVec2| {
        let t = ty(w, card);
        w.spawn_squad_at(PlayerId(0), t, at, FxVec2::new(Fx::ONE, Fx::ZERO))
            .unwrap()
    };
    let r = sp(&mut w, "ww2:ger_grenadiergruppe", ahead);
    let r2 = sp(&mut w, "ww2:ger_grenadiergruppe", behind);
    let crew = sp(&mut w, "ww2:ger_tank_crew", ahead);
    for (k, q) in [r, r2, crew].into_iter().enumerate() {
        run(&mut w, 0, 10 + k as u32, CommandKind::Retreat { squad: q });
    }
    let goal = |w: &World, q| w.game().unwrap().squads[&q].nav.goal;
    assert_eq!(goal(&w, r), c, "ahead of the HQ: retreat to it");
    assert_eq!(goal(&w, r2), base, "behind the HQ: retreat to the base");
    assert_eq!(goal(&w, crew), base, "a tank crew retreats to the base");
    // A second build order for an HQ is refused at the cap.
    run(
        &mut w,
        0,
        3,
        CommandKind::Build {
            squad: zt,
            def: hq,
            pos: FxVec2::from_int(450, 450),
            facing: FxVec2::new(Fx::ONE, Fx::ZERO),
        },
    );
    assert_eq!(w.snapshot().supply_refusals[0].refusal, SupplyRefusal::Cap);
}

#[test]
fn a_carried_crate_is_placed_and_walks_the_squad_slower() {
    let mut w = flat_world(1);
    let b = base_center(&w, 0);
    let bearers = spawn(
        &mut w,
        0,
        "ww2:ger_munitionstraeger",
        b.x.floor_int(),
        b.y.floor_int(),
    );
    run(
        &mut w,
        0,
        1,
        CommandKind::IssueCrate {
            squad: bearers,
            kind: SupplyKind::Shells,
        },
    );
    let carrier = w
        .game()
        .unwrap()
        .supply6()
        .crates
        .values()
        .next()
        .unwrap()
        .carrier
        .unwrap();
    assert!(w.game().unwrap().supply6().carries(carrier));
    let dest = b + FxVec2::from_int(60, 0);
    let dest = FxVec2::new(dest.x, dest.y);
    run(
        &mut w,
        0,
        2,
        CommandKind::PlaceCrate {
            squad: bearers,
            pos: dest,
        },
    );
    let mut t = 0;
    while w.game().unwrap().supply6().carries(carrier) {
        w.step(&[]);
        t += 1;
        assert!(t < 120 * HZ, "the crate was not set down");
    }
    // 60 m at 2.9 m/s x 70 %: at least 29 s.
    assert!(t >= 25 * HZ, "set down after {t} ticks: not slowed");
    let c = w.game().unwrap().supply6().crates.values().next().unwrap();
    assert!((c.pos - dest).length() < Fx::from_int(5));
    // Pick it up again.
    let cid = *w.game().unwrap().supply6().crates.keys().next().unwrap();
    run(
        &mut w,
        0,
        3,
        CommandKind::PickUpCrate {
            squad: bearers,
            crate_id: cid,
        },
    );
    steps(&mut w, 10 * HZ);
    assert!(
        w.game().unwrap().supply6().crates[&cid].carrier.is_some(),
        "picked up"
    );
}

// ------------------------------------------------------------------ T12 --

/// A staged S6 scene: an HQ with stock, a mortar firing next to a crate, a
/// tank rearming, a crate carried, an enemy HE cook-off pending.
fn scene(threads: usize) -> World {
    let mut w = flat_world(threads);
    let m = spawn(&mut w, 0, "ww2:ger_mortar_team", 500, 500);
    let sq = spawn(&mut w, 0, "ww2:ger_pz4_h_squad", 520, 470);
    let v = members(&w, sq)[0];
    spawn(&mut w, 1, "ww2:usa_rifle_squad", 560, 520);
    let hq = def(&w, 0, StockKind::Hq, None);
    add_built(&mut w, 0, hq, FxVec2::from_int(515, 480), [100, 50, 300]);
    let d = def(&w, 0, StockKind::Crate, Some(SupplyKind::Shells));
    add_crate_at(&mut w, 0, d, FxVec2::from_int(495, 505));
    let d1 = def(&w, 1, StockKind::Crate, Some(SupplyKind::Shells));
    add_crate_at(&mut w, 1, d1, FxVec2::from_int(565, 525));
    w.edit_vehicle(v, |vs| vs.ammo.iter_mut().for_each(|a| *a /= 2));
    run(
        &mut w,
        0,
        1,
        CommandKind::FireMission {
            squad: m,
            target: FxVec2::from_int(565, 525),
            rounds: 12,
        },
    );
    w
}

#[test]
fn t12_s6_scene_is_deterministic_across_threads_and_save_load() {
    let n = 40 * HZ;
    let mut hashes = Vec::new();
    for threads in [1, 2, 3, 8] {
        let mut w = scene(threads);
        steps(&mut w, n);
        hashes.push(w.state_hash());
        assert_eq!(w.state_hash(), w.full_state_hash());
    }
    assert!(hashes.windows(2).all(|h| h[0] == h[1]), "{hashes:x?}");
    // Save at the half, load, continue: the same hash.
    let mut w = scene(2);
    steps(&mut w, n / 2);
    let mut l = World::load(&w.save(), 3).unwrap();
    assert_eq!(l.state(), w.state());
    steps(&mut l, n / 2);
    assert_eq!(l.state_hash(), hashes[0]);
    // Something happened: stock moved, the mortar drew bombs.
    let a = acc(&l, 0);
    assert!(a.structure_bombs + a.crate_bombs + a.tank_rounds_fwd > 0);
}

// ---------------------------------------------- per-field hash and load --

type StateEdit = Box<dyn Fn(&mut SupplyState)>;

#[test]
fn every_s6_field_is_saved_hashed_and_checked_on_load() {
    let mut w = scene(1);
    steps(&mut w, 5 * HZ);
    w.edit_supply6(|s| {
        s.cookoffs.push(crate::structures::CookOff {
            id: s.next_cookoff,
            pos: FxVec3::new(Fx::from_int(10), Fx::from_int(10), Fx::ZERO),
            player: 1,
            class: 3,
            ammo: 0,
            left: 2,
            next_tick: 1 << 40,
            every: 30,
        });
        s.next_cookoff += 1;
    });
    // ammo 0 must have a blast for the cook-off to be valid.
    let he = w
        .match_setup()
        .unwrap()
        .ammo
        .iter()
        .position(|a| a.blast.is_some())
        .unwrap() as u16;
    w.edit_supply6(|s| s.cookoffs[0].ammo = he);
    let w = World::from_state(w.state.clone(), 1).unwrap();
    let loaded = World::load(&w.save(), 2).unwrap();
    assert_eq!(loaded.state(), w.state());
    assert_eq!(loaded.full_state_hash(), w.full_state_hash());
    // Every field is listed (a new one fails to compile here).
    let Structure {
        owner: _,
        def: _,
        pos: _,
        yaw: _,
        builder: _,
        built: _,
        box_hp: _,
        radio: _,
        stock_g: _,
        beds: _,
    } = w
        .game()
        .unwrap()
        .supply6()
        .structures
        .values()
        .next()
        .unwrap()
        .clone();
    let Crate {
        owner: _,
        def: _,
        pos: _,
        carrier: _,
        dest: _,
        claim: _,
        g: _,
        hp: _,
    } = w
        .game()
        .unwrap()
        .supply6()
        .crates
        .values()
        .next()
        .unwrap()
        .clone();
    let S6Account {
        issue_at: _,
        hq_lost_at: _,
        hq_built_at: _,
        structures_built: _,
        structures_lost: _,
        crates_issued: _,
        crate_micro: _,
        crates_delivered: _,
        stock_in_g: _,
        base_bombs: _,
        structure_bombs: _,
        crate_bombs: _,
        unsourced_bombs: _,
        fwd_rounds: _,
        fwd_items: _,
        tank_rounds_fwd: _,
        tank_rare_fwd: _,
        tank_crate_rounds: _,
        rack_credit_micro: _,
        rack_credit_mp: _,
        cookoffs: _,
        crates_lost: _,
        stock_out_g: _,
        stock_lost_g: _,
        truck_loaded_g: _,
        truck_load_micro: _,
        truck_out_g: _,
        truck_dropped_g: _,
        truck_lost_g: _,
        truck_crates_dropped: _,
        truck_tank_rounds: _,
        truck_men_rounds: _,
        fwd_belt_rounds: _,
        cargo_detonations: _,
    } = S6Account::default();
    fn st(s: &mut SupplyState) -> &mut Structure {
        s.structures.values_mut().next().unwrap()
    }
    fn cr(s: &mut SupplyState) -> &mut Crate {
        s.crates.values_mut().next().unwrap()
    }
    fn ac(s: &mut SupplyState) -> &mut S6Account {
        &mut s.accounts[0]
    }
    let one = Fx::ONE;
    let edits: Vec<(&str, StateEdit)> = vec![
        ("structure.owner", Box::new(|s| st(s).owner = PlayerId(1))),
        ("structure.def", Box::new(|s| st(s).def ^= 1)),
        ("structure.pos", Box::new(move |s| st(s).pos.x += one)),
        ("structure.yaw", Box::new(move |s| st(s).yaw += one)),
        ("structure.built", Box::new(|s| st(s).built ^= true)),
        (
            "structure.builder",
            Box::new(|s| st(s).builder = Some(SquadId(0))),
        ),
        ("structure.box_hp", Box::new(|s| st(s).box_hp[0] -= 1)),
        ("structure.radio", Box::new(|s| st(s).radio ^= true)),
        ("structure.stock_g", Box::new(|s| st(s).stock_g[1] -= 1)),
        (
            "structures.key",
            Box::new(|s| {
                let v = s.structures.remove(&0).unwrap();
                s.structures.insert(1, v);
            }),
        ),
        ("next_structure", Box::new(|s| s.next_structure += 1)),
        ("crate.owner", Box::new(|s| cr(s).owner = PlayerId(1))),
        ("crate.def", Box::new(|s| cr(s).def ^= 1)),
        ("crate.pos", Box::new(move |s| cr(s).pos.y += one)),
        (
            "crate.carrier",
            Box::new(|s| cr(s).carrier = Some(SimId(0))),
        ),
        (
            "crate.dest",
            Box::new(move |s| cr(s).dest = Some(FxVec2::from_int(1, 1))),
        ),
        ("crate.claim", Box::new(|s| cr(s).claim = Some(SquadId(0)))),
        ("crate.g", Box::new(|s| cr(s).g -= 1)),
        ("crate.hp", Box::new(|s| cr(s).hp -= 1)),
        ("next_crate", Box::new(|s| s.next_crate += 1)),
        ("cookoff.id", Box::new(|s| s.cookoffs[0].id ^= 1)),
        ("cookoff.pos", Box::new(move |s| s.cookoffs[0].pos.z += one)),
        ("cookoff.player", Box::new(|s| s.cookoffs[0].player ^= 1)),
        ("cookoff.class", Box::new(|s| s.cookoffs[0].class ^= 1)),
        ("cookoff.ammo", Box::new(|s| s.cookoffs[0].ammo ^= 1)),
        ("cookoff.left", Box::new(|s| s.cookoffs[0].left += 1)),
        (
            "cookoff.next_tick",
            Box::new(|s| s.cookoffs[0].next_tick += 1),
        ),
        ("cookoff.every", Box::new(|s| s.cookoffs[0].every += 1)),
        ("next_cookoff", Box::new(|s| s.next_cookoff += 1)),
        ("acc.issue_at", Box::new(|s| ac(s).issue_at += 1)),
        ("acc.hq_lost_at", Box::new(|s| ac(s).hq_lost_at += 1)),
        ("acc.hq_built_at", Box::new(|s| ac(s).hq_built_at += 1)),
        (
            "acc.structures_built",
            Box::new(|s| ac(s).structures_built += 1),
        ),
        (
            "acc.structures_lost",
            Box::new(|s| ac(s).structures_lost += 1),
        ),
        (
            "acc.crates_issued",
            Box::new(|s| ac(s).crates_issued[1] += 1),
        ),
        ("acc.crate_micro", Box::new(|s| ac(s).crate_micro += 1)),
        (
            "acc.crates_delivered",
            Box::new(|s| ac(s).crates_delivered += 1),
        ),
        ("acc.stock_in_g", Box::new(|s| ac(s).stock_in_g += 1)),
        ("acc.base_bombs", Box::new(|s| ac(s).base_bombs += 1)),
        (
            "acc.structure_bombs",
            Box::new(|s| ac(s).structure_bombs += 1),
        ),
        ("acc.crate_bombs", Box::new(|s| ac(s).crate_bombs += 1)),
        (
            "acc.unsourced_bombs",
            Box::new(|s| ac(s).unsourced_bombs += 1),
        ),
        ("acc.fwd_rounds", Box::new(|s| ac(s).fwd_rounds += 1)),
        ("acc.fwd_items", Box::new(|s| ac(s).fwd_items += 1)),
        (
            "acc.tank_rounds_fwd",
            Box::new(|s| ac(s).tank_rounds_fwd += 1),
        ),
        ("acc.tank_rare_fwd", Box::new(|s| ac(s).tank_rare_fwd += 1)),
        (
            "acc.tank_crate_rounds",
            Box::new(|s| ac(s).tank_crate_rounds += 1),
        ),
        (
            "acc.rack_credit_micro",
            Box::new(|s| ac(s).rack_credit_micro += 1),
        ),
        (
            "acc.rack_credit_mp",
            Box::new(|s| ac(s).rack_credit_mp += 1),
        ),
        ("acc.cookoffs", Box::new(|s| ac(s).cookoffs += 1)),
        ("acc.crates_lost", Box::new(|s| ac(s).crates_lost += 1)),
        ("acc.stock_out_g", Box::new(|s| ac(s).stock_out_g += 1)),
        ("acc.stock_lost_g", Box::new(|s| ac(s).stock_lost_g += 1)),
        (
            "acc.truck_loaded_g",
            Box::new(|s| ac(s).truck_loaded_g[1] += 1),
        ),
        (
            "acc.truck_load_micro",
            Box::new(|s| ac(s).truck_load_micro += 1),
        ),
        ("acc.truck_out_g", Box::new(|s| ac(s).truck_out_g += 1)),
        (
            "acc.truck_dropped_g",
            Box::new(|s| ac(s).truck_dropped_g += 1),
        ),
        ("acc.truck_lost_g", Box::new(|s| ac(s).truck_lost_g += 1)),
        (
            "acc.truck_crates_dropped",
            Box::new(|s| ac(s).truck_crates_dropped += 1),
        ),
        (
            "acc.truck_tank_rounds",
            Box::new(|s| ac(s).truck_tank_rounds += 1),
        ),
        (
            "acc.truck_men_rounds",
            Box::new(|s| ac(s).truck_men_rounds += 1),
        ),
        (
            "acc.fwd_belt_rounds",
            Box::new(|s| ac(s).fwd_belt_rounds += 1),
        ),
        (
            "acc.cargo_detonations",
            Box::new(|s| ac(s).cargo_detonations += 1),
        ),
    ];
    let base = w.full_state_hash();
    for (name, edit) in &edits {
        let mut s = w.state.clone();
        edit(&mut s.game.as_mut().unwrap().supply6);
        // Hashed (validity aside: the hash covers what a save holds).
        let h = crate::hash::full_hash(&s, &w.config_bytes, &w.setup_bytes);
        assert_ne!(h, base, "{name} is not hashed");
    }
    // Validated on load: each of these is rejected.
    let us_hq = def(&w, 1, StockKind::Hq, None);
    let bad: Vec<(&str, StateEdit)> = vec![
        // Review S6 F5: builder, faction and map checks.
        (
            "structure builder missing",
            Box::new(|s| {
                st(s).built = false;
                st(s).builder = Some(SquadId(9_999));
            }),
        ),
        (
            "structure def of the other faction",
            Box::new(move |s| st(s).def = us_hq),
        ),
        // S9-8: a crate may name an enemy faction's def (captured).
        (
            "crate off the map",
            Box::new(|s| cr(s).pos.x = Fx::from_int(-5)),
        ),
        (
            "crate dest off the map",
            Box::new(|s| cr(s).dest = Some(FxVec2::from_int(5, 5_000))),
        ),
        (
            "cookoff off the map",
            Box::new(|s| s.cookoffs[0].pos.y = Fx::from_int(-5)),
        ),
        ("structure id >= next", Box::new(|s| s.next_structure = 0)),
        (
            "structure def a crate",
            Box::new(|s| {
                let k = s.crates.values().next().unwrap().def;
                st(s).def = k;
            }),
        ),
        ("structure def out of range", Box::new(|s| st(s).def = 999)),
        ("structure owner", Box::new(|s| st(s).owner = PlayerId(7))),
        (
            "structure off the map",
            Box::new(|s| st(s).pos.x = Fx::from_int(-5)),
        ),
        ("structure box hp", Box::new(|s| st(s).box_hp[0] = 100_000)),
        (
            "structure stock over cap",
            Box::new(|s| st(s).stock_g[2] = 10_000_000),
        ),
        (
            "built with builder",
            Box::new(|s| st(s).builder = Some(SquadId(0))),
        ),
        ("unbuilt without builder", Box::new(|s| st(s).built = false)),
        ("crate id >= next", Box::new(|s| s.next_crate = 0)),
        ("crate def out of range", Box::new(|s| cr(s).def = 999)),
        ("crate grams", Box::new(|s| cr(s).g = 1_000_000)),
        ("crate hp 0", Box::new(|s| cr(s).hp = 0)),
        (
            "crate carrier",
            Box::new(|s| cr(s).carrier = Some(SimId(60_000))),
        ),
        (
            "crate claim",
            Box::new(|s| cr(s).claim = Some(SquadId(9_999))),
        ),
        ("cookoff ammo", Box::new(|s| s.cookoffs[0].ammo = 999)),
        ("cookoff left 0", Box::new(|s| s.cookoffs[0].left = 0)),
        ("cookoff id", Box::new(|s| s.next_cookoff = 0)),
        (
            "accounts len",
            Box::new(|s| s.accounts.pop().map(|_| ()).unwrap()),
        ),
        (
            "hq cap",
            Box::new(|s| {
                let v = st(s).clone();
                s.structures.insert(s.next_structure, v);
                s.next_structure += 1;
            }),
        ),
        (
            "crate cap",
            Box::new(|s| {
                let c = cr(s).clone();
                for _ in 0..13 {
                    s.crates.insert(s.next_crate, c.clone());
                    s.next_crate += 1;
                }
            }),
        ),
    ];
    for (name, edit) in &bad {
        let mut s = w.state.clone();
        edit(&mut s.game.as_mut().unwrap().supply6);
        let mut w2 = World::from_state(w.state.clone(), 1).unwrap();
        w2.state = s;
        assert!(World::load(&w2.save(), 1).is_err(), "{name} was accepted");
    }
    // A carrier must be alive (review S6 F5): the same own man loads while
    // he lives and is rejected once dead.
    let mut s = w.state.clone();
    let g = s.game.as_ref().unwrap();
    let man = (0..s.entities.len())
        .map(|i| SimId(i as u32))
        .find(|m| {
            let i = m.index();
            s.entities.kind[i] == crate::ecs::EntityKind::Soldier
                && s.entities.owner[i] == PlayerId(0)
                && s.entities.combat[i].hp > 0
                && !g.supply6().carries(*m)
        })
        .unwrap();
    {
        let c = cr(&mut s.game.as_mut().unwrap().supply6);
        c.carrier = Some(man);
        c.dest = None;
        c.claim = None;
    }
    let load = |st: &SimState| {
        let mut w2 = World::from_state(w.state.clone(), 1).unwrap();
        w2.state = st.clone();
        World::load(&w2.save(), 1)
    };
    assert!(load(&s).is_ok(), "a live carrier was rejected");
    s.entities.combat[man.index()].hp = 0;
    assert!(load(&s).is_err(), "a dead carrier was accepted");
}
