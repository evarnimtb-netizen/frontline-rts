//! S10a Build A tests (docs/slices/S10a-fortifications.md 7): blueprints,
//! materials, pieces, damage gates, overhead cover, save and hash.

use frontline_content::setup::{Capability, StockKind, SupplyKind};

use super::s6_tests::{add_built, add_crate_at, def, flat_world_with, members, run, spawn, steps};
use super::*;
use crate::fort::{FortRefusal, WorkStep};
use crate::fx::FxVec3;
use crate::game::{Order, SquadId};

const HZ: u64 = 30;

/// The flat ww2 world; engineer squads get the field engineer caps (Build
/// B gives them in the content).
fn world() -> World {
    world_with(|_| {})
}

fn world_with(edit: impl FnOnce(&mut frontline_content::MatchSetup)) -> World {
    flat_world_with(1, |s| {
        for t in s.squad_types.iter_mut() {
            if t.id.contains("engineer") {
                t.caps |= Capability::BuildStructure.bit()
                    | Capability::PlaceWire.bit()
                    | Capability::ClearWire.bit();
            }
        }
        edit(s);
    })
}

fn c(x: i32, y: i32) -> FxVec2 {
    FxVec2::from_int(x, y)
}

fn fort(w: &World, id: &str) -> u16 {
    w.match_setup()
        .unwrap()
        .forts
        .iter()
        .position(|f| f.id == id)
        .unwrap() as u16
}

fn weapon(w: &World, id: &str) -> u16 {
    w.match_setup()
        .unwrap()
        .weapons
        .iter()
        .position(|x| x.id == id)
        .unwrap() as u16
}

fn item_of(w: &World, kind: frontline_content::ItemKind) -> u16 {
    w.match_setup()
        .unwrap()
        .items
        .iter()
        .position(|x| x.kind == kind)
        .unwrap() as u16
}

fn fs(w: &World) -> &crate::fort::FortState {
    w.game().unwrap().fort()
}

fn fortify(w: &mut World, p: u8, squad: SquadId, f: u16, line: Vec<FxVec2>) {
    run(
        w,
        p,
        0,
        CommandKind::Fortify {
            squad,
            fort: f,
            line,
        },
    );
}

fn refused(w: &World) -> Option<FortRefusal> {
    w.fort_refusals().last().map(|e| e.refusal)
}

fn carry(w: &mut World, s: SquadId, kg: u32) {
    w.edit_fort(|f| {
        f.carried_g.insert(s, kg * 1000);
    });
}

fn pieces_of(w: &World, f: u16) -> Vec<u32> {
    fs(w)
        .pieces
        .iter()
        .filter(|(_, q)| q.fort == f)
        .map(|(&id, _)| id)
        .collect()
}

#[test]
fn fortify_refused_without_capability() {
    let mut w = world();
    let f = fort(&w, "ww2:sandbag_wall");
    let rifles = spawn(&mut w, 0, "ww2:ger_grenadiergruppe", 500, 500);
    fortify(&mut w, 0, rifles, f, vec![c(500, 505), c(510, 505)]);
    assert_eq!(refused(&w), Some(FortRefusal::CannotBuild));
    assert!(fs(&w).works.is_empty());
    // A rifle squad cannot load construction material either.
    let eng = spawn(&mut w, 0, "ww2:ger_engineer_squad", 520, 500);
    fortify(&mut w, 0, eng, f, vec![c(520, 505), c(530, 505)]);
    assert_eq!(fs(&w).works.len(), 1);
    assert_eq!(fs(&w).accounts[0].works_started, 1);
}

#[test]
fn fortify_refused_at_cap_spacing_slope_and_enemy_base() {
    let mut w = world();
    let wall = fort(&w, "ww2:sandbag_wall");
    let nest = fort(&w, "ww2:mg_nest");
    let eng = spawn(&mut w, 0, "ww2:ger_engineer_squad", 500, 500);
    // Cap: 40 segments stand (4 x 20 m).
    for k in 0..4 {
        assert!(w.build_now(
            wall,
            &[c(300, 300 + 10 * k), c(320, 300 + 10 * k)],
            PlayerId(0)
        ));
    }
    assert_eq!(pieces_of(&w, wall).len(), 40);
    fortify(&mut w, 0, eng, wall, vec![c(500, 505), c(504, 505)]);
    assert_eq!(refused(&w), Some(FortRefusal::Cap));
    // Spacing: an MG nest 5 m from another.
    assert!(w.build_now(nest, &[c(600, 600), c(610, 600)], PlayerId(0)));
    fortify(&mut w, 0, eng, nest, vec![c(605, 600), c(615, 600)]);
    assert_eq!(refused(&w), Some(FortRefusal::BadSpot));
    // Enemy base and off the map.
    let enemy = w.game().unwrap().base_of(PlayerId(1));
    fortify(&mut w, 0, eng, nest, vec![enemy, enemy + c(5, 0)]);
    assert_eq!(refused(&w), Some(FortRefusal::BadSpot));
    fortify(&mut w, 0, eng, nest, vec![c(-5, 10), c(5, 10)]);
    assert_eq!(refused(&w), Some(FortRefusal::BadSpot));
    // Slope: the wall of a dug trench.
    assert!(w.dig_pit_now(
        &[c(700, 700), c(720, 700)],
        frontline_content::DigProfile::Trench
    ));
    let steep = (0..60)
        .map(|k| {
            FxVec2::new(
                Fx::from_int(710),
                Fx::from_int(700) + Fx::from_milli(50 * k),
            )
        })
        .find(|&p| crate::fort::slope_pct(&w.state.terrain, p) > 25)
        .expect("a steep trench wall");
    fortify(&mut w, 0, eng, nest, vec![steep, steep + c(5, 0)]);
    assert_eq!(refused(&w), Some(FortRefusal::BadSpot));
    assert!(fs(&w).works.is_empty());
}

#[test]
fn load_materials_paid_in_base_only() {
    let mut w = world();
    let base = w.game().unwrap().base_of(PlayerId(0));
    let eng = spawn(&mut w, 0, "ww2:ger_engineer_squad", 0, 0);
    w.place_squad(eng, base, FxVec2::new(Fx::ONE, Fx::ZERO));
    let men = members(&w, eng).len() as u32;
    let mp0 = w.game().unwrap().players[0].manpower;
    run(
        &mut w,
        0,
        0,
        CommandKind::LoadMaterials {
            squad: eng,
            kg: 1000,
        },
    );
    let g = fs(&w).carried_g[&eng];
    // Up to carry_kg_per_man (20) a man, 100 MP per t.
    assert_eq!(g, 20_000 * men);
    let paid = mp0 - w.game().unwrap().players[0].manpower;
    assert_eq!(paid as u64, (g as u64 * 100).div_ceil(1_000_000));
    assert_eq!(fs(&w).accounts[0].materials_loaded_g, g as u64);
    // Outside the base: refused, nothing paid.
    let out = spawn(&mut w, 0, "ww2:ger_engineer_squad", 500, 500);
    let mp1 = w.game().unwrap().players[0].manpower;
    run(
        &mut w,
        0,
        1,
        CommandKind::LoadMaterials { squad: out, kg: 20 },
    );
    assert_eq!(refused(&w), Some(FortRefusal::NotInBase));
    assert!(!fs(&w).carried_g.contains_key(&out));
    assert_eq!(w.game().unwrap().players[0].manpower, mp1);
}

#[test]
fn materials_drawn_carried_then_crate_then_truck() {
    let mut w = world();
    let eng = spawn(&mut w, 0, "ww2:ger_engineer_squad", 500, 500);
    carry(&mut w, eng, 10);
    let cd = def(&w, 0, StockKind::Crate, Some(SupplyKind::Construction));
    let cr = add_crate_at(&mut w, 0, cd, c(505, 500));
    let truck = spawn(&mut w, 0, "ww2:ger_opel_blitz_squad", 510, 500);
    let tv = members(&w, truck)[0];
    w.edit_vehicle(tv, |v| v.material_g = 80_000);
    let ents = w.state.entities.clone();
    let g = w.state.game.as_mut().unwrap();
    let t = g.setup.rules.fortify.clone().unwrap();
    let got = g.draw_materials(&ents, PlayerId(0), eng, c(505, 502), 100_000, &t);
    assert_eq!(got, 100_000);
    assert!(!g.fort.carried_g.contains_key(&eng), "carried first");
    assert!(
        !g.supply6.crates.contains_key(&cr),
        "then the crate (40 kg)"
    );
    assert_eq!(g.vehicles[&tv].material_g, 30_000, "then the truck (50 kg)");
    assert_eq!(g.fort.accounts[0].materials_used_g, 100_000);
}

#[test]
fn work_waits_without_materials() {
    let mut w = world();
    let f = fort(&w, "ww2:sandbag_wall");
    let eng = spawn(&mut w, 0, "ww2:ger_engineer_squad", 500, 500);
    fortify(&mut w, 0, eng, f, vec![c(500, 505), c(502, 505)]);
    steps(&mut w, 60 * HZ);
    let wk = fs(&w).works.values().next().expect("waiting").clone();
    assert_eq!(wk.step, WorkStep::Wait);
    assert!(pieces_of(&w, f).is_empty());
    carry(&mut w, eng, 30);
    steps(&mut w, 60 * HZ);
    assert_eq!(pieces_of(&w, f).len(), 1);
    assert!(fs(&w).works.is_empty());
}

#[test]
fn construction_crate_never_enters_a_stock() {
    let mut w = world();
    let hq = def(&w, 0, StockKind::Hq, None);
    let cd = def(&w, 0, StockKind::Crate, Some(SupplyKind::Construction));
    let sid = add_built(&mut w, 0, hq, c(500, 500), [0, 0, 0]);
    let cr = add_crate_at(&mut w, 0, cd, c(501, 501));
    steps(&mut w, 10 * HZ);
    let g = w.game().unwrap();
    assert_eq!(g.supply6().structures[&sid].stock_g, [0, 0, 0]);
    assert_eq!(g.supply6().crates[&cr].g, 40_000);
    // Issued in base: paid, counted outside the three-wide arrays.
    let base = g.base_of(PlayerId(0));
    let eng = spawn(&mut w, 0, "ww2:ger_engineer_squad", 0, 0);
    w.place_squad(eng, base, FxVec2::new(Fx::ONE, Fx::ZERO));
    run(
        &mut w,
        0,
        0,
        CommandKind::IssueCrate {
            squad: eng,
            kind: SupplyKind::Construction,
        },
    );
    let g = w.game().unwrap();
    assert_eq!(g.supply6().crates.len(), 2);
    assert_eq!(g.supply6().accounts[0].crates_issued, [0, 0, 0]);
    assert_eq!(g.fort().accounts[0].materials_loaded_g, 40_000);
}

#[test]
fn sandbag_wall_builds_segment_by_segment() {
    let mut w = world();
    let f = fort(&w, "ww2:sandbag_wall");
    let eng = spawn(&mut w, 0, "ww2:ger_engineer_squad", 500, 500);
    carry(&mut w, eng, 90);
    fortify(&mut w, 0, eng, f, vec![c(500, 506), c(506, 506)]);
    let mut seen = Vec::new();
    for _ in 0..(180 * HZ) {
        steps(&mut w, 1);
        let n = pieces_of(&w, f).len();
        if seen.last() != Some(&n) {
            seen.push(n);
        }
        if fs(&w).works.is_empty() {
            break;
        }
    }
    assert_eq!(seen, vec![0, 1, 2, 3], "one segment at a time");
    assert_eq!(w.game().unwrap().squads[&eng].order, Order::Idle);
    assert_eq!(fs(&w).accounts[0].pieces_built, 3);
}

/// A sandbag wall piece of `owner` across x = 500 at y 500.
fn wall_at(w: &mut World, owner: u8) -> u32 {
    let f = fort(w, "ww2:sandbag_wall");
    assert!(w.build_now(f, &[c(500, 499), c(500, 501)], PlayerId(owner)));
    *pieces_of(w, f).last().unwrap()
}

fn hp(w: &World, id: u32) -> Option<u32> {
    fs(w).pieces.get(&id).map(|q| q.hp)
}

/// A point `z_cm` above the ground at (x, y).
fn at(w: &World, x: i32, y: i32, z_cm: i32) -> FxVec3 {
    let g = w.state.terrain.height_at(c(x, y));
    FxVec3::new(
        Fx::from_int(x),
        Fx::from_int(y),
        g + Fx::from_ratio(z_cm as i64, 100),
    )
}

#[test]
fn rifle_rounds_do_not_damage_sandbags() {
    let mut w = world();
    let id = wall_at(&mut w, 0);
    let rifle = weapon(&w, "ww2:m1_garand");
    for _ in 0..20 {
        let (from, to) = (at(&w, 480, 500, 50), at(&w, 520, 500, 50));
        assert!(w.fire_round_now(1, rifle, from, to));
        steps(&mut w, HZ);
    }
    assert_eq!(hp(&w, id), Some(400));
}

#[test]
fn he_and_shot_damage_pieces() {
    let mut w = world();
    let id = wall_at(&mut w, 0);
    let mortar = weapon(&w, "ww2:m1_mortar");
    assert!(w.drop_shell_now(1, mortar, c(501, 500)));
    steps(&mut w, 2 * HZ);
    let after_he = hp(&w, id).expect("a wall survives one bomb");
    assert!(after_he < 400, "overpressure takes the bomb's damage");
    shoot_shot(&mut w, 1);
    let shot = w
        .match_setup()
        .unwrap()
        .rules
        .fortify
        .clone()
        .unwrap()
        .shot_piece_damage;
    assert!(
        hp(&w, id).is_none_or(|h| h + shot <= after_he),
        "a shot takes at least shot_piece_damage"
    );
}

/// An AP round of the Sherman's gun for `player` at the wall across x=500.
fn shoot_shot(w: &mut World, player: u8) {
    let s = w.match_setup().unwrap();
    let wi = s
        .weapons
        .iter()
        .position(|x| x.id == "ww2:kwk40_l48")
        .unwrap() as u16;
    let ai = s
        .ammo
        .iter()
        .position(|a| a.id == "ww2:75mm_pzgr39")
        .unwrap() as u16;
    let (from, to) = (at(w, 470, 500, 50), at(w, 530, 500, 50));
    assert!(w.fire_shell_now(player, wi, ai, from, to));
    steps(w, HZ);
}

#[test]
fn fragments_below_armour_do_not_damage_pieces() {
    let frag_hits = |armour: u32| {
        let mut w = world_with(|s| {
            for f in s.forts.iter_mut() {
                for p in f.pieces.iter_mut() {
                    p.armour_mm = armour;
                }
            }
        });
        let id = wall_at(&mut w, 0);
        let sq = spawn(&mut w, 1, "ww2:usa_rifle_squad", 560, 560);
        let thrower = members(&w, sq)[0];
        let frag = item_of(&w, frontline_content::ItemKind::Frag);
        // 4 m off: outside the grenade's blast, inside its fragments.
        assert!(w.drop_item_now(thrower, frag, c(504, 500), Fx::ONE));
        steps(&mut w, 5 * HZ);
        400 - hp(&w, id).unwrap_or(0)
    };
    assert_eq!(frag_hits(100), 0, "armour 100 stops every fragment");
    assert!(frag_hits(1) > 0, "a thin piece takes fragments");
}

#[test]
fn own_rounds_pass_own_pieces() {
    let mut w = world();
    let id = wall_at(&mut w, 0);
    shoot_shot(&mut w, 0);
    assert_eq!(hp(&w, id), Some(400), "own shot passes");
    shoot_shot(&mut w, 1);
    assert!(hp(&w, id).is_none_or(|h| h < 400), "the enemy's stops");
}

#[test]
fn interrupted_work_is_removed() {
    let mut w = world();
    let f = fort(&w, "ww2:sandbag_wall");
    let eng = spawn(&mut w, 0, "ww2:ger_engineer_squad", 500, 500);
    carry(&mut w, eng, 30);
    fortify(&mut w, 0, eng, f, vec![c(500, 505), c(502, 505)]);
    steps(&mut w, 5 * HZ);
    assert_eq!(fs(&w).works.len(), 1);
    run(
        &mut w,
        0,
        1,
        CommandKind::Move {
            squad: eng,
            target: c(520, 520),
        },
    );
    steps(&mut w, 1);
    assert!(fs(&w).works.is_empty());
    assert!(!fs(&w).carried_g.contains_key(&eng), "its kg are lost");
    assert!(pieces_of(&w, f).is_empty());
}

#[test]
fn mg_nest_digs_its_pit_then_builds_its_ring() {
    let mut w = world();
    let f = fort(&w, "ww2:mg_nest");
    let eng = spawn(&mut w, 0, "ww2:ger_engineer_squad", 500, 500);
    carry(&mut w, eng, 60);
    fortify(&mut w, 0, eng, f, vec![c(500, 510), c(500, 520)]);
    assert!(matches!(
        w.game().unwrap().squads[&eng].order,
        Order::Dig { .. }
    ));
    assert_eq!(fs(&w).works.values().next().unwrap().step, WorkStep::Dig);
    for _ in 0..(600 * HZ / 30) {
        steps(&mut w, 30);
        if fs(&w).works.is_empty() {
            break;
        }
    }
    assert_eq!(pieces_of(&w, f).len(), 3);
    let t = &w.state.terrain;
    let dug = t.base_height_at(c(500, 510)) - t.height_at(c(500, 510));
    assert!(dug > Fx::from_ratio(8, 10), "the pit is dug ({dug:?})");
}

/// A dugout of player 0 at (500, 500) (pit dug, roof built) with five men
/// of a rifle squad under the roof.
fn dugout(w: &mut World) -> (u32, Vec<SimId>) {
    let f = fort(w, "ww2:dugout");
    let pit = w.match_setup().unwrap().rules.dugout_pit.pit_line;
    let half = Fx::from(pit).div_int(2).max(Fx::from_milli(100));
    let x = Fx::from_int(500);
    let y = Fx::from_int(500);
    assert!(w.dig_pit_now(
        &[FxVec2::new(x - half, y), FxVec2::new(x + half, y)],
        frontline_content::DigProfile::DugoutPit
    ));
    assert!(w.build_now(f, &[c(500, 500), c(510, 500)], PlayerId(0)));
    let roof = pieces_of(w, f)[0];
    let sq = spawn(w, 0, "ww2:ger_grenadiergruppe", 500, 500);
    let men: Vec<SimId> = members(w, sq).into_iter().take(5).collect();
    for (k, &m) in members(w, sq).iter().enumerate() {
        let p = if k < 5 {
            FxVec2::new(x + Fx::from_milli(400 * (k as i32 - 2)), y)
        } else {
            c(560 + k as i32, 560)
        };
        w.place_soldier(m, p);
    }
    steps(w, 1);
    (roof, men)
}

fn alive(w: &World, men: &[SimId]) -> usize {
    men.iter()
        .filter(|m| w.state.entities.combat[m.index()].hp > 0)
        .count()
}

#[test]
fn dugout_roof_survives_an_81mm_hit() {
    let mut w = world();
    let (roof, men) = dugout(&mut w);
    let mortar = weapon(&w, "ww2:m1_mortar");
    assert!(w.drop_shell_now(1, mortar, c(500, 500)));
    steps(&mut w, 2 * HZ);
    assert!(hp(&w, roof).is_some_and(|h| h > 0), "the roof stands");
    assert_eq!(alive(&w, &men), 5, "nobody under it is lost");
}

#[test]
fn dugout_roof_breaks_under_a_satchel() {
    let mut w = world();
    let (roof, _) = dugout(&mut w);
    let sq = spawn(&mut w, 1, "ww2:usa_engineer_squad", 560, 540);
    let thrower = members(&w, sq)[0];
    let satchel = item_of(&w, frontline_content::ItemKind::Satchel);
    assert!(w.drop_item_now(thrower, satchel, c(500, 500), Fx::from_int(2)));
    steps(&mut w, 5 * HZ);
    assert_eq!(hp(&w, roof), None, "the roof breaks");
    assert_eq!(fs(&w).accounts[0].pieces_lost, 1);
}

#[test]
fn dugout_roof_breaks_under_heavier_he() {
    // A test-only 105 mm: the 81 mm with damage 400 (verify).
    let mut w = world_with(|s| {
        let k = s
            .weapons
            .iter()
            .position(|x| x.id == "ww2:m1_mortar")
            .unwrap();
        s.weapons[k].damage = 400;
    });
    let (roof, _) = dugout(&mut w);
    let heavy = weapon(&w, "ww2:m1_mortar");
    assert!(w.drop_shell_now(1, heavy, c(500, 500)));
    steps(&mut w, 2 * HZ);
    assert_eq!(hp(&w, roof), None);
}

#[test]
fn hq_roof_only_over_an_own_built_hq() {
    let mut w = world();
    let f = fort(&w, "ww2:hq_dugout");
    let eng = spawn(&mut w, 0, "ww2:ger_engineer_squad", 480, 480);
    fortify(&mut w, 0, eng, f, vec![c(500, 500), c(510, 500)]);
    assert_eq!(refused(&w), Some(FortRefusal::BadSpot), "no HQ");
    let enemy_hq = def(&w, 1, StockKind::Hq, None);
    add_built(&mut w, 1, enemy_hq, c(500, 500), [0, 0, 0]);
    fortify(&mut w, 0, eng, f, vec![c(500, 500), c(510, 500)]);
    assert_eq!(refused(&w), Some(FortRefusal::BadSpot), "an enemy HQ");
    let hq = def(&w, 0, StockKind::Hq, None);
    let sid = add_built(&mut w, 0, hq, c(600, 600), [0, 0, 0]);
    fortify(&mut w, 0, eng, f, vec![c(602, 601), c(610, 600)]);
    let wk = fs(&w).works.values().next().expect("accepted").clone();
    assert_eq!(wk.hq, Some(sid));
    assert_eq!(wk.line[0], c(600, 600), "snapped to the HQ");
    let eng2 = spawn(&mut w, 0, "ww2:ger_engineer_squad", 620, 620);
    fortify(&mut w, 0, eng2, f, vec![c(600, 600), c(610, 600)]);
    // One roof per HQ (and limit_per_player 1): refused.
    assert!(matches!(
        refused(&w),
        Some(FortRefusal::BadSpot | FortRefusal::Cap)
    ));
    assert_eq!(fs(&w).works.len(), 1);
}

#[test]
fn hq_roof_goes_with_its_hq() {
    let mut w = world();
    let f = fort(&w, "ww2:hq_dugout");
    let hq = def(&w, 0, StockKind::Hq, None);
    let sid = add_built(&mut w, 0, hq, c(600, 600), [0, 0, 0]);
    assert!(w.build_now(f, &[c(600, 600), c(610, 600)], PlayerId(0)));
    let roof = pieces_of(&w, f)[0];
    assert_eq!(fs(&w).pieces[&roof].hq, Some(sid));
    w.edit_supply6(|s| {
        s.structures.remove(&sid);
    });
    steps(&mut w, 1);
    assert!(pieces_of(&w, f).is_empty());
    assert_eq!(fs(&w).accounts[0].pieces_lost, 1);
}

/// An edit of the fort state (hash and load tests).
type FortEdit = Box<dyn Fn(&mut crate::fort::FortState)>;

/// A world with every kind of S10a state.
fn busy() -> (World, SquadId) {
    let mut w = world();
    let wall = fort(&w, "ww2:sandbag_wall");
    let wire = fort(&w, "ww2:barbed_wire");
    let eng = spawn(&mut w, 0, "ww2:ger_engineer_squad", 500, 500);
    assert!(w.build_now(wall, &[c(400, 400), c(406, 400)], PlayerId(0)));
    assert!(w.build_now(wire, &[c(400, 420), c(410, 420)], PlayerId(1)));
    carry(&mut w, eng, 60);
    fortify(&mut w, 0, eng, wall, vec![c(500, 505), c(504, 505)]);
    steps(&mut w, 2 * HZ);
    (w, eng)
}

#[test]
fn fort_state_saves_loads_and_hashes() {
    let (w, _) = busy();
    assert!(!fs(&w).works.is_empty() && !fs(&w).wire.is_empty());
    let bytes = w.save();
    let mut back = World::load(&bytes, 1).unwrap();
    assert_eq!(back.state_hash(), w.state_hash());
    assert_eq!(back.game().unwrap().fort(), fs(&w));
    // Each saved list is hashed.
    let h = back.state_hash();
    let edits: Vec<FortEdit> = vec![
        Box::new(|f| f.next_piece += 1),
        Box::new(|f| f.pieces.values_mut().next().unwrap().hp -= 1),
        Box::new(|f| f.wire.values_mut().next().unwrap().hp -= 1),
        Box::new(|f| f.works.values_mut().next().unwrap().work += 1),
        Box::new(|f| f.accounts[1].wire_cut += 1),
        Box::new(|f| f.next_charge += 1),
    ];
    for e in edits {
        let mut x = World::load(&bytes, 1).unwrap();
        x.edit_fort(|f| e(f));
        assert_ne!(x.state_hash(), h);
    }
    steps(&mut back, HZ);
}

#[test]
fn load_rejects_bad_fort_state() {
    let (w, eng) = busy();
    let bytes = w.save();
    let bad: Vec<FortEdit> = vec![
        Box::new(|f| f.pieces.values_mut().next().unwrap().hp = 0),
        Box::new(|f| f.pieces.values_mut().next().unwrap().fort = 99),
        Box::new(|f| f.wire.values_mut().next().unwrap().hp = 10_000),
        Box::new(|f| f.next_piece = 0),
        Box::new(|f| {
            f.accounts.pop();
        }),
        Box::new(|f| f.works.values_mut().next().unwrap().builder = SquadId(999)),
        Box::new(move |f| {
            f.carried_g.insert(eng, 10_000_000);
        }),
        Box::new(|f| {
            f.charges.push(crate::fort::Charge {
                id: 0,
                owner: PlayerId(0),
                item: 0,
                from: FxVec2::from_int(1, 1),
                to: FxVec2::from_int(2, 2),
                fuse_tick: 5,
            });
            f.next_charge = 1;
        }),
    ];
    for (k, e) in bad.into_iter().enumerate() {
        let mut x = World::load(&bytes, 1).unwrap();
        x.edit_fort(|f| e(f));
        assert!(World::load(&x.save(), 1).is_err(), "bad state {k} loaded");
    }
}

#[test]
fn empty_fort_state_changes_nothing() {
    let run_ai = |off: bool| {
        let mut w = flat_world_with(1, |s| s.players.iter_mut().for_each(|p| p.is_ai = true));
        w.state.game.as_mut().unwrap().fort_off = off;
        steps(&mut w, 600);
        assert!(w.game().unwrap().fort().is_empty());
        (w.state_hash(), w.state.entities.clone())
    };
    let (h_on, e_on) = run_ai(false);
    let (h_off, e_off) = run_ai(true);
    assert_eq!(h_on, h_off);
    assert!(e_on == e_off, "entity state differs");
}
