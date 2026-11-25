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
    // Slope: tested on a ridge (`slope_reads_the_undug_ground_along_lines`).
    assert!(fs(&w).works.is_empty());
}

/// A flat world with a north-south ridge along x = 700 (8 m high, 6 m to
/// each side).
fn ridge_world() -> World {
    use frontline_content::setup::{BandSetup, Fixed, FixedVec2};
    world_with(|s| {
        s.map.terrain.bands.push(BandSetup {
            from: FixedVec2::from_m([700, 550]),
            to: FixedVec2::from_m([700, 850]),
            height: Fixed::from_int(8),
            half_width: Fixed::from_int(6),
        })
    })
}

#[test]
fn slope_reads_the_undug_ground_along_lines() {
    let mut w = ridge_world();
    let wall = fort(&w, "ww2:sandbag_wall");
    let nest = fort(&w, "ww2:mg_nest");
    let eng = spawn(&mut w, 0, "ww2:ger_engineer_squad", 600, 600);
    // A point on the ridge's flank.
    let steep = (0..60)
        .map(|k| {
            FxVec2::new(
                Fx::from_int(694) + Fx::from_milli(100 * k),
                Fx::from_int(700),
            )
        })
        .find(|&p| crate::fort::slope_pct(&w.state.terrain, p) > 25)
        .expect("a steep flank");
    fortify(&mut w, 0, eng, nest, vec![steep, steep + c(5, 0)]);
    assert_eq!(refused(&w), Some(FortRefusal::BadSpot));
    // A wall whose two ends lie on flat ground but which crosses the
    // ridge: refused (sampled along its length).
    assert_eq!(crate::fort::slope_pct(&w.state.terrain, c(690, 650)), 0);
    assert_eq!(crate::fort::slope_pct(&w.state.terrain, c(710, 650)), 0);
    fortify(&mut w, 0, eng, wall, vec![c(690, 650), c(710, 650)]);
    assert_eq!(refused(&w), Some(FortRefusal::BadSpot));
    assert!(fs(&w).works.is_empty());
    // Beside a dug trench (1.5 m off its centre line, a sample falls in
    // it): the undug ground is flat, so the wall is accepted.
    assert!(w.dig_pit_now(
        &[c(300, 300), c(320, 300)],
        frontline_content::DigProfile::Trench
    ));
    let by = FxVec2::new(Fx::from_int(305), Fx::from_milli(301_500));
    let t = &w.state.terrain;
    assert!(t.base_height_at(by + c(0, -1)) - t.height_at(by + c(0, -1)) > Fx::from_ratio(1, 2));
    fortify(&mut w, 0, eng, wall, vec![by, by + c(10, 0)]);
    assert_eq!(fs(&w).works.len(), 1, "{:?}", refused(&w));
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
    // Twenty Garand rounds at a wall (armour `armour`, or none) with a man
    // of the wall's owner 10 m behind it: (wall hp, the man's hp).
    let volley = |armour: Option<u32>| {
        let mut w = world_with(|s| {
            for f in s.forts.iter_mut() {
                for p in f.pieces.iter_mut() {
                    p.armour_mm = armour.unwrap_or(100);
                }
            }
        });
        let id = armour.map(|_| wall_at(&mut w, 0));
        let sq = spawn(&mut w, 0, "ww2:ger_grenadiergruppe", 300, 300);
        let man = members(&w, sq)[0];
        w.place_soldier(man, c(510, 500));
        steps(&mut w, 1);
        let rifle = weapon(&w, "ww2:m1_garand");
        for _ in 0..20 {
            let (from, to) = (at(&w, 480, 500, 50), at(&w, 520, 500, 50));
            assert!(w.fire_round_now(1, rifle, from, to));
            steps(&mut w, HZ);
        }
        (
            id.and_then(|id| hp(&w, id)),
            w.state.entities.combat[man.index()].hp,
        )
    };
    let full = {
        let mut w = world();
        let sq = spawn(&mut w, 0, "ww2:ger_grenadiergruppe", 300, 300);
        w.state.entities.combat[members(&w, sq)[0].index()].hp
    };
    // Armour 100: no damage, and the rounds stop at the wall.
    assert_eq!(volley(Some(100)), (Some(400), full));
    // Control: the same rounds hit the man with no wall in the way...
    assert!(volley(None).1 < full, "the rounds reach the man");
    // ...and damage a thin (1 mm) wall they reach.
    let (thin, man) = volley(Some(1));
    assert!(thin.is_none_or(|h| h < 400), "a thin wall takes the rounds");
    assert_eq!(man, full, "and stops them");
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
    let (roof, men) = dugout_pit(w, true);
    (roof.expect("roofed"), men)
}

/// [`dugout`], or (`roofed` false) only its pit with the five men in it.
fn dugout_pit(w: &mut World, roofed: bool) -> (Option<u32>, Vec<SimId>) {
    let f = fort(w, "ww2:dugout");
    let pit = w.match_setup().unwrap().rules.dugout_pit.pit_line;
    let half = Fx::from(pit).div_int(2).max(Fx::from_milli(100));
    let x = Fx::from_int(500);
    let y = Fx::from_int(500);
    assert!(w.dig_pit_now(
        &[FxVec2::new(x - half, y), FxVec2::new(x + half, y)],
        frontline_content::DigProfile::DugoutPit
    ));
    let roof = roofed.then(|| {
        assert!(w.build_now(f, &[c(500, 500), c(510, 500)], PlayerId(0)));
        pieces_of(w, f)[0]
    });
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

/// Hit points of `men` (a full man has `combat.hp` of a fresh soldier).
fn hps(w: &World, men: &[SimId]) -> Vec<u16> {
    men.iter()
        .map(|m| w.state.entities.combat[m.index()].hp)
        .collect()
}

#[test]
fn dugout_roof_survives_an_81mm_hit() {
    let bomb = |roofed: bool| {
        let mut w = world();
        let (roof, men) = dugout_pit(&mut w, roofed);
        let before = hps(&w, &men);
        let mortar = weapon(&w, "ww2:m1_mortar");
        assert!(w.drop_shell_now(1, mortar, c(500, 500)));
        steps(&mut w, 2 * HZ);
        let dmg = w.match_setup().unwrap().weapons[mortar as usize].damage;
        (roof.map(|r| hp(&w, r)), before, hps(&w, &men), dmg)
    };
    // The bomb bursts on the roof: the roof takes its overpressure damage
    // and stands; the men under it are not touched.
    let (roof, before, after, dmg) = bomb(true);
    assert_eq!(roof, Some(Some(250 - dmg)), "the roof takes the bomb");
    assert_eq!(after, before, "nobody under it is hurt");
    // Control: the same bomb into the open pit hurts them.
    let (_, before, after, _) = bomb(false);
    assert!(
        after.iter().zip(&before).any(|(a, b)| a < b),
        "the open pit is hit"
    );
}

/// The top of piece `id` (m).
fn top_of(w: &World, id: u32) -> Fx {
    let q = &fs(w).pieces[&id];
    let b = &w.match_setup().unwrap().forts[q.fort as usize].pieces[q.part as usize];
    q.z + Fx::from(b.half[2]).mul_int(2)
}

/// Steps `ticks`, holding `men` where they stand (they would run from
/// the item before its fuse ends). The height of the first burst of item
/// `item`, if one went off.
fn hold_and_step(w: &mut World, men: &[SimId], item: u16, ticks: u64) -> Option<f32> {
    let spots: Vec<FxVec2> = men
        .iter()
        .map(|m| w.state.entities.transform[m.index()].pos.xy())
        .collect();
    let mut z = None;
    for _ in 0..ticks {
        for (m, p) in men.iter().zip(&spots) {
            w.place_soldier(*m, *p);
        }
        w.step(&[]);
        if z.is_none()
            && let Some(e) = w.snapshot().explosions.iter().find(|e| e.weapon == item)
        {
            z = Some(e.pos[2]);
        }
    }
    z
}

/// Drops item `kind` of player 1 from 2 m above `roof`'s top at (500,
/// 500) and steps 5 s with `men` held in place. The burst's height.
fn drop_on_roof(
    w: &mut World,
    roof: u32,
    men: &[SimId],
    kind: frontline_content::ItemKind,
) -> Option<f32> {
    // The thrower's squad far off (out of the fight).
    let sq = spawn(w, 1, "ww2:usa_engineer_squad", 900, 100);
    let thrower = members(w, sq)[0];
    let item = item_of(w, kind);
    let p = c(500, 500);
    let above = top_of(w, roof) + Fx::from_int(2) - w.state.terrain.height_at(p);
    assert!(w.drop_item_now(thrower, item, p, above));
    hold_and_step(w, men, item, 5 * HZ)
}

#[test]
fn dugout_roof_breaks_under_a_satchel() {
    // A satchel dropped onto the roof comes to rest on it and bursts there
    // (S10a fix S7: before, it fell through onto the pit floor): its
    // overpressure (300) breaks the roof (250 hp). The men under it are
    // not hurt: the roof stops that burst's overpressure (men are blasted
    // before boxes). Its fragments come after the roof broke and could
    // pass the broken box; on this staging none reaches the men.
    let mut w = world();
    let (roof, men) = dugout(&mut w);
    let top = top_of(&w, roof).to_f32();
    let before = hps(&w, &men);
    let z =
        drop_on_roof(&mut w, roof, &men, frontline_content::ItemKind::Satchel).expect("it burst");
    assert!(
        (z - top).abs() < 0.05,
        "on the roof: burst at {z}, top {top}"
    );
    assert_eq!(hp(&w, roof), None, "the roof breaks");
    assert_eq!(fs(&w).accounts[0].pieces_lost, 1);
    assert_eq!(hps(&w, &men), before, "the men under it are covered");
}

#[test]
fn a_grenade_on_a_dugout_roof_hurts_nobody_inside() {
    // A hand grenade on the roof: it rests and bursts there; the roof takes
    // its overpressure (150 < 250 hp) and stands; the men are untouched.
    let mut w = world();
    let (roof, men) = dugout(&mut w);
    let top = top_of(&w, roof).to_f32();
    let before = hps(&w, &men);
    let z = drop_on_roof(&mut w, roof, &men, frontline_content::ItemKind::Frag).expect("it burst");
    assert!(
        (z - top).abs() < 0.05,
        "on the roof: burst at {z}, top {top}"
    );
    assert!(hp(&w, roof).is_some_and(|h| h < 250), "the roof takes it");
    assert_eq!(hps(&w, &men), before, "nobody inside is hurt");
    // Control: the same grenade into the open pit.
    let mut w = world();
    let (_, men) = dugout_pit(&mut w, false);
    let before = hps(&w, &men);
    let sq = spawn(&mut w, 1, "ww2:usa_engineer_squad", 900, 100);
    let thrower = members(&w, sq)[0];
    let frag = item_of(&w, frontline_content::ItemKind::Frag);
    assert!(w.drop_item_now(thrower, frag, c(500, 500), Fx::from_int(2)));
    hold_and_step(&mut w, &men, frag, 5 * HZ).expect("it burst");
    let after = hps(&w, &men);
    assert!(
        after.iter().zip(&before).any(|(a, b)| a < b),
        "the open pit is hit"
    );
}

#[test]
fn an_item_on_a_piece_over_the_map_edge_lands_on_the_map_and_saves() {
    // A wall built along the map's east edge overhangs it (half its 0.6 m
    // thickness). A grenade coming down steeply just inside the edge hits
    // the top past the edge in the step that leaves the map: it lands on
    // the ground at the clamped point (re-review R1), and the save loads.
    let mut w = world();
    let size = w.match_setup().unwrap().map.size_m;
    let f = fort(&w, "ww2:sandbag_wall");
    assert!(w.build_now(f, &[c(size, 499), c(size, 501)], PlayerId(0)));
    let wall = *pieces_of(&w, f).last().unwrap();
    let sq = spawn(&mut w, 1, "ww2:usa_engineer_squad", 900, 100);
    let thrower = members(&w, sq)[0];
    let frag = item_of(&w, frontline_content::ItemKind::Frag);
    let from = FxVec3::new(
        Fx::from_int(size) - Fx::from_ratio(2, 100),
        Fx::from_int(500),
        top_of(&w, wall) + Fx::from_ratio(25, 100),
    );
    let g = w.state.game.as_mut().unwrap();
    g.thrown.push(crate::items::Thrown {
        id: g.next_thrown,
        item: frag,
        owner: PlayerId(1),
        thrower,
        from,
        vel: FxVec3::new(Fx::from_int(6), Fx::ZERO, Fx::from_int(-30)),
        step: 0,
        pos: from,
        landed: false,
        fuse: 90,
        inside: None,
    });
    g.next_thrown += 1;
    steps(&mut w, 2);
    let t = &w.game().unwrap().thrown[0];
    assert!(t.landed);
    assert!(
        w.state.terrain.contains(t.pos.xy()),
        "on the map: {:?}",
        t.pos
    );
    assert!(World::load(&w.save(), 1).is_ok(), "the save loads");
}

#[test]
fn an_item_on_a_roof_falls_when_the_roof_breaks() {
    // Two satchels on one dugout roof, the second 1 s later: the first
    // breaks the roof; the second, at rest on it, falls to the pit floor
    // and bursts there, not in mid-air (re-review R2).
    let mut w = world();
    let (roof, men) = dugout(&mut w);
    let top = top_of(&w, roof).to_f32();
    let sq = spawn(&mut w, 1, "ww2:usa_engineer_squad", 900, 100);
    let thrower = members(&w, sq)[0];
    let satchel = item_of(&w, frontline_content::ItemKind::Satchel);
    let above =
        |w: &World, p: FxVec2| top_of(w, roof) + Fx::from_int(2) - w.state.terrain.height_at(p);
    let p1 = c(500, 500);
    let h = above(&w, p1);
    assert!(w.drop_item_now(thrower, satchel, p1, h));
    steps(&mut w, HZ);
    let p2 = FxVec2::new(Fx::from_milli(500_500), Fx::from_int(500));
    let h = above(&w, p2);
    assert!(w.drop_item_now(thrower, satchel, p2, h));
    // Each burst's height and the ground under the second just before it
    // (a satchel's crater reshapes the ground).
    let mut zs = Vec::new();
    for _ in 0..(6 * HZ) {
        for &m in &men {
            w.place_soldier(m, c(560, 540));
        }
        let ground = w.state.terrain.height_at(p2).to_f32();
        w.step(&[]);
        zs.extend(
            w.snapshot()
                .explosions
                .iter()
                .filter(|e| e.weapon == satchel)
                .map(|e| (e.pos[2], ground)),
        );
    }
    assert_eq!(zs.len(), 2, "both burst: {zs:?}");
    assert!(
        (zs[0].0 - top).abs() < 0.05,
        "the first on the roof: {zs:?}"
    );
    assert_eq!(hp(&w, roof), None, "the roof breaks");
    assert!(
        (zs[1].0 - zs[1].1).abs() < 0.05 && zs[1].0 < top - 1.0,
        "the second on the ground below the roof: {zs:?}"
    );
}

#[test]
fn a_wall_piece_covers_a_man_from_overpressure() {
    // A satchel (fragments off: overpressure only) 1.5 m in front of an
    // enemy wall, its owner's man 1.2 m behind it (inside the 4 m blast):
    // the wall stops the overpressure (rule 3, `piece_between`); without
    // the wall he is hit.
    let blast = |walled: bool| {
        let mut w = world_with(|s| {
            for it in s.items.iter_mut() {
                if let Some(he) = it.he.as_mut() {
                    he.blast.frag_count = 0;
                }
            }
        });
        if walled {
            wall_at(&mut w, 0);
        }
        let sq = spawn(&mut w, 0, "ww2:ger_grenadiergruppe", 300, 300);
        let man = members(&w, sq)[0];
        w.place_soldier(man, FxVec2::new(Fx::from_milli(501_200), Fx::from_int(500)));
        steps(&mut w, 1);
        let before = w.state.entities.combat[man.index()].hp;
        let ts = spawn(&mut w, 1, "ww2:usa_engineer_squad", 900, 100);
        let thrower = members(&w, ts)[0];
        let satchel = item_of(&w, frontline_content::ItemKind::Satchel);
        let p = FxVec2::new(Fx::from_milli(498_500), Fx::from_int(500));
        assert!(w.drop_item_now(thrower, satchel, p, Fx::from_ratio(1, 10)));
        hold_and_step(&mut w, &[man], satchel, 5 * HZ).expect("it burst");
        (before, w.state.entities.combat[man.index()].hp)
    };
    let (before, after) = blast(true);
    assert_eq!(after, before, "covered by the wall");
    let (before, after) = blast(false);
    assert!(after < before, "hit in the open");
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
fn hq_roof_refused_over_an_already_roofed_hq() {
    // The cap raised to 3: only the "already roofed" rule can refuse.
    let mut w = world_with(|s| {
        for f in s.forts.iter_mut().filter(|f| f.id == "ww2:hq_dugout") {
            f.limit_per_player = 3;
        }
    });
    let f = fort(&w, "ww2:hq_dugout");
    let hq = def(&w, 0, StockKind::Hq, None);
    let a = add_built(&mut w, 0, hq, c(600, 600), [0, 0, 0]);
    let b = add_built(&mut w, 0, hq, c(700, 600), [0, 0, 0]);
    let eng = spawn(&mut w, 0, "ww2:ger_engineer_squad", 620, 620);
    fortify(&mut w, 0, eng, f, vec![c(600, 600), c(610, 600)]);
    assert_eq!(fs(&w).works.values().next().unwrap().hq, Some(a));
    // A second roof over the same HQ (a work under way): BadSpot.
    let eng2 = spawn(&mut w, 0, "ww2:ger_engineer_squad", 640, 620);
    fortify(&mut w, 0, eng2, f, vec![c(600, 600), c(610, 600)]);
    assert_eq!(refused(&w), Some(FortRefusal::BadSpot));
    assert_eq!(fs(&w).works.len(), 1);
    // Over a built roof too.
    assert!(w.build_now(f, &[c(700, 600), c(710, 600)], PlayerId(0)));
    assert!(fs(&w).pieces.values().any(|q| q.hq == Some(b)));
    fortify(&mut w, 0, eng2, f, vec![c(700, 600), c(710, 600)]);
    assert_eq!(refused(&w), Some(FortRefusal::BadSpot));
    // A third, unroofed HQ: accepted.
    let third = add_built(&mut w, 0, hq, c(800, 600), [0, 0, 0]);
    fortify(&mut w, 0, eng2, f, vec![c(800, 600), c(810, 600)]);
    assert!(fs(&w).works.values().any(|wk| wk.hq == Some(third)));
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
                piece: None,
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

/// B3 run 2 cut the M17 rifle grenade's burst (blast 160 cm, 64 fragments;
/// B3-2 (A)). This test's match was found with the Mk 2 hand grenade's
/// burst it had before: restore that blast radius and fragment count.
fn m17_burst_pre_b3(s: &mut frontline_content::MatchSetup) {
    let mk2 = s
        .items
        .iter()
        .find(|i| i.id == "ww2:mk2_grenade")
        .and_then(|i| i.he.as_ref())
        .map(|h| (h.blast.blast_radius, h.blast.frag_count))
        .expect("the Mk 2 grenade's burst");
    let m17 = s
        .ammo
        .iter_mut()
        .find(|a| a.id == "ww2:m17_rifle_grenade")
        .and_then(|a| a.blast.as_mut())
        .expect("the M17's burst");
    (m17.blast.blast_radius, m17.blast.frag_count) = mk2;
}

#[test]
fn empty_fort_state_changes_nothing() {
    // An AI-vs-AI skirmish on the real map (hills, woods) for 15 game
    // minutes, once as built and once with the test switch `fort_off`
    // sending every S10a hook down its pre-S10a path (wire pace and path
    // layer, piece statics, thrown items on pieces, overhead cover, wire
    // blast and fragments, piece damage gates, charges, the fort stage):
    // nothing is built, so each hook must leave play exactly as it was.
    // Full state hashes are compared every second. (What the switch cannot
    // reach, the new defs and setup bytes, is the 40/160-set neutral.py
    // comparison against the base binary.)
    let run_ai = |off: bool| {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2");
        let defs = frontline_content::ModDefs::load(&root).unwrap();
        let req = frontline_content::MatchRequest::skirmish(
            3,
            "ww2:germany",
            "ww2:usa",
            frontline_content::Difficulty::Normal,
        );
        let mut setup = defs.resolve(&req).unwrap();
        setup.players.iter_mut().for_each(|p| p.is_ai = true);
        // S10a-ai: the fortification AI off (the S10a AI).
        let f = &mut setup.rules.ai.fort;
        (f.works, f.wire, f.breach, f.cards) = (0, 0, 0, 0);
        // S10b-ai: its habits off too.
        super::s10a_ai_tests::s10b_ai_off(&mut setup);
        // S12b Build C: and wounds and the medic AI (found without).
        setup.s12_off();
        setup.s13_off();
        // B3 run 2: and the M17's old burst (the fight was found with it).
        m17_burst_pre_b3(&mut setup);
        // S11a1 Build C: and the artillery AI (the fight was found without).
        let a = &mut setup.rules.ai.arty;
        (a.buy, a.fo, a.prep) = (0, 0, 0);
        // S11a2 Build B: and its switches (the fight was found without).
        (a.ff_safe, a.counter_fire) = (0, 0);
        let fm = setup.rules.fire_mission.as_mut().unwrap();
        (fm.friendly_fire, fm.counter_battery) = (0, 0);
        let mut w = World::new_match(setup, 1).unwrap();
        w.state.game.as_mut().unwrap().fort_off = off;
        let mut hashes = Vec::new();
        for t in 0..15 * 60 * HZ {
            w.step(&[]);
            if t % HZ == 0 {
                hashes.push(w.full_state_hash());
            }
        }
        assert!(w.game().unwrap().fort().is_empty());
        let losses: u32 = w.game().unwrap().players.iter().map(|p| p.losses).sum();
        (
            hashes,
            losses,
            w.state.stats.throws,
            w.state.stats.frag_rays,
        )
    };
    let (on, losses, throws, rays) = run_ai(false);
    let (off, ..) = run_ai(true);
    assert!(
        losses > 0 && throws > 0 && rays > 0,
        "a fight: {losses} losses, {throws} throws, {rays} fragment rays"
    );
    let first = on.iter().zip(&off).position(|(a, b)| a != b);
    assert_eq!(first, None, "hashes differ from second {first:?}");
}

// ------------------------------------------------------------ Build B --

/// Enemy (player 1) wire from `a` to `b` (`World::build_now`).
fn wire_line(w: &mut World, a: FxVec2, b: FxVec2) {
    let f = fort(w, "ww2:barbed_wire");
    assert!(w.build_now(f, &[a, b], PlayerId(1)));
}

/// Sections whose middle lies within `[x0, x1]` (m).
fn sections_between(w: &World, x0: i32, x1: i32) -> Vec<u32> {
    fs(w)
        .wire
        .iter()
        .filter(|(_, s)| {
            let mx = (s.a.x + s.b.x).div_int(2);
            mx >= Fx::from_int(x0) && mx <= Fx::from_int(x1)
        })
        .map(|(&id, _)| id)
        .collect()
}

/// A rifleman of a fresh player 0 squad at `(x, y)`, walking detached to
/// `to`.
fn walker(w: &mut World, x: i32, y: i32, to: FxVec2) -> SimId {
    let s = spawn(w, 0, "ww2:ger_grenadiergruppe", x, y);
    let m = members(w, s)[1];
    run(
        w,
        0,
        0,
        CommandKind::MoveUnits {
            units: vec![m],
            target: to,
        },
    );
    m
}

/// Steps until `m` is past `y_end` (or `max` ticks): per tick, the
/// position he was steered from and the speed he was given.
fn walk(w: &mut World, m: SimId, y_end: i32, max: u64) -> Vec<(FxVec2, Fx)> {
    let mut out = Vec::new();
    for _ in 0..max {
        let from = w.state.entities.transform[m.index()].pos.xy();
        w.step(&[]);
        let moving = w.state.entities.unit[m.index()].nav.phase == NavPhase::Moving;
        if moving {
            out.push((from, w.state.entities.mover[m.index()].speed));
        }
        if w.state.entities.transform[m.index()].pos.y > Fx::from_int(y_end) {
            break;
        }
    }
    out
}

fn in_band(w: &World, p: FxVec2) -> bool {
    w.game()
        .unwrap()
        .fort()
        .in_wire(&w.match_setup().unwrap().forts, p)
}

#[test]
fn wire_slows_infantry_to_wire_speed_pct() {
    let mut w = world();
    wire_line(&mut w, c(400, 500), c(600, 500));
    assert_eq!(fs(&w).wire.len(), 100);
    let m = walker(&mut w, 500, 485, c(500, 515));
    let trace = walk(&mut w, m, 510, 60 * HZ);
    let outside: Vec<Fx> = trace
        .iter()
        .filter(|(p, _)| !in_band(&w, *p))
        .map(|x| x.1)
        .collect();
    let inside: Vec<Fx> = trace
        .iter()
        .filter(|(p, _)| in_band(&w, *p))
        .map(|x| x.1)
        .collect();
    assert!(
        inside.len() > 5,
        "he never stood in the band: {}",
        inside.len()
    );
    let full = outside[0];
    assert!(full > Fx::ZERO && outside.iter().all(|&v| v == full));
    // Exactly wire_speed_pct (20) of the same man's speed outside.
    let slow = full.mul_int(20).div_int(100);
    assert!(inside.iter().all(|&v| v == slow), "{inside:?} vs {slow:?}");
    assert!(w.state.entities.transform[m.index()].pos.y > Fx::from_int(510));
}

#[test]
fn cut_gap_restores_full_speed() {
    let mut w = world();
    wire_line(&mut w, c(400, 500), c(600, 500));
    let eng = spawn(&mut w, 0, "ww2:ger_engineer_squad", 500, 490);
    // Four cuts: the sections from x = 496 to 504 (an 8 m gap).
    for k in 0..4 {
        run(
            &mut w,
            0,
            k,
            CommandKind::CutWire {
                squad: eng,
                at: c(497 + 2 * k as i32, 500),
            },
        );
        assert!(matches!(
            w.game().unwrap().squads[&eng].order,
            Order::CutWire { .. }
        ));
        for _ in 0..40 * HZ {
            w.step(&[]);
            if w.game().unwrap().squads[&eng].order == Order::Idle {
                break;
            }
        }
    }
    assert_eq!(fs(&w).accounts[0].wire_cut, 4);
    assert!(sections_between(&w, 496, 504).is_empty());
    assert_eq!(fs(&w).wire.len(), 96);
    // A man through the 8 m gap keeps his full speed.
    let m = walker(&mut w, 500, 470, c(500, 530));
    let trace = walk(&mut w, m, 520, 60 * HZ);
    let full = trace[0].1;
    let crossing: Vec<&(FxVec2, Fx)> = trace
        .iter()
        .filter(|(p, _)| (p.y - Fx::from_int(500)).abs() <= Fx::ONE)
        .collect();
    assert!(!crossing.is_empty());
    assert!(crossing.iter().all(|(p, v)| *v == full && !in_band(&w, *p)));
}

#[test]
fn infantry_path_goes_through_a_gap() {
    let mut w = world();
    wire_line(&mut w, c(400, 500), c(600, 500));
    // A 12 m gap at x 510..522 (six sections).
    let gap = sections_between(&w, 510, 522);
    assert_eq!(gap.len(), 6);
    w.edit_fort(|f| {
        for id in &gap {
            f.wire.remove(id);
        }
    });
    let m = walker(&mut w, 505, 470, c(505, 530));
    let trace = walk(&mut w, m, 520, 90 * HZ);
    let cross: Vec<Fx> = trace
        .iter()
        .filter(|(p, _)| (p.y - Fx::from_int(500)).abs() <= Fx::ONE)
        .map(|(p, _)| p.x)
        .collect();
    assert!(!cross.is_empty());
    assert!(
        cross
            .iter()
            .all(|&x| x >= Fx::from_int(510) && x <= Fx::from_int(522)),
        "crossed at {cross:?}"
    );
    // Through the gap at full speed.
    assert!(trace.iter().all(|(p, _)| !in_band(&w, *p)));
}

#[test]
fn vehicle_crushes_wire() {
    let mut w = world();
    wire_line(&mut w, c(480, 500), c(520, 500));
    let tank = spawn(&mut w, 0, "ww2:ger_pz4_h_squad", 500, 470);
    let v = members(&w, tank)[0];
    run(
        &mut w,
        0,
        0,
        CommandKind::Move {
            squad: tank,
            target: c(500, 540),
        },
    );
    for _ in 0..60 * HZ {
        w.step(&[]);
        if w.state.entities.transform[v.index()].pos.y > Fx::from_int(515) {
            break;
        }
    }
    assert!(w.state.entities.transform[v.index()].pos.y > Fx::from_int(515));
    let gone = 20 - fs(&w).wire.len() as u32;
    assert!(gone >= 2, "{gone} sections crushed");
    assert_eq!(fs(&w).accounts[0].wire_crushed, gone);
    // Sections well away from the hull stand.
    assert_eq!(sections_between(&w, 480, 490).len(), 5);
    assert_eq!(sections_between(&w, 510, 520).len(), 5);
}

fn item_id(w: &World, id: &str) -> u16 {
    w.match_setup()
        .unwrap()
        .items
        .iter()
        .position(|x| x.id == id)
        .unwrap() as u16
}

#[test]
fn he_burst_cuts_wire_in_blast_radius() {
    let mut w = world();
    wire_line(&mut w, c(470, 500), c(530, 500));
    let ts = spawn(&mut w, 0, "ww2:ger_grenadiergruppe", 200, 200);
    let thrower = members(&w, ts)[0];
    let satchel = item_id(&w, "ww2:geballte_ladung");
    assert!(w.drop_item_now(thrower, satchel, c(500, 500), Fx::ONE));
    steps(&mut w, 4 * HZ);
    // Overpressure (blast 4 m, damage 300 >= hp 200) breaks every section
    // within the blast radius; fragments reach 8 m at most.
    assert!(sections_between(&w, 497, 503).is_empty());
    assert_eq!(sections_between(&w, 470, 490).len(), 10);
    assert_eq!(sections_between(&w, 510, 530).len(), 10);
    // Not a Bangalore: no counter.
    assert_eq!(fs(&w).accounts[0].wire_blown, 0);
}

#[test]
fn fragments_damage_wire_and_fly_on() {
    // The same stick grenade 3 m short of the wire with enemy men 2-4 m
    // behind it, with and without the wire: the wire takes fragment
    // damage and the men take exactly the same hits (no ray stops).
    let play = |wired: bool| {
        let mut w = world();
        if wired {
            wire_line(&mut w, c(480, 500), c(520, 500));
        }
        let ts = spawn(&mut w, 0, "ww2:ger_grenadiergruppe", 200, 200);
        let thrower = members(&w, ts)[0];
        let foes = spawn(&mut w, 1, "ww2:usa_rifle_squad", 500, 503);
        let frag = item_id(&w, "ww2:stielhandgranate");
        assert!(w.drop_item_now(thrower, frag, c(500, 497), Fx::ONE));
        steps(&mut w, 4 * HZ);
        let hp: Vec<u16> = members(&w, foes)
            .iter()
            .map(|m| w.state.entities.combat[m.index()].hp)
            .collect();
        let wire_hp: u32 = fs(&w).wire.values().map(|s| s.hp).sum();
        (hp, wire_hp, fs(&w).wire.len())
    };
    let (hp_w, wire_hp, n) = play(true);
    let (hp_open, _, _) = play(false);
    assert_eq!(n, 20, "a grenade breaks no section");
    assert!(wire_hp < 20 * 200, "no fragment hit the wire");
    assert_eq!(hp_w, hp_open);
}

#[test]
fn cut_wire_needs_clear_wire() {
    let mut w = world();
    wire_line(&mut w, c(480, 500), c(520, 500));
    let rifles = spawn(&mut w, 0, "ww2:ger_grenadiergruppe", 500, 490);
    run(
        &mut w,
        0,
        0,
        CommandKind::CutWire {
            squad: rifles,
            at: c(500, 500),
        },
    );
    assert_eq!(refused(&w), Some(FortRefusal::CannotBuild));
    assert_eq!(w.game().unwrap().squads[&rifles].order, Order::Idle);
    // An engineer squad, but no wire within cut_reach_m (25) of the point.
    let eng = spawn(&mut w, 0, "ww2:ger_engineer_squad", 500, 600);
    run(
        &mut w,
        0,
        1,
        CommandKind::CutWire {
            squad: eng,
            at: c(500, 560),
        },
    );
    assert_eq!(refused(&w), Some(FortRefusal::NoWire));
    // Each clear_wire card may.
    for (k, card) in [
        "ww2:ger_engineer_squad",
        "ww2:ger_sturmpioniergruppe",
        "ww2:ger_baupioniergruppe",
    ]
    .into_iter()
    .enumerate()
    {
        let s = spawn(&mut w, 0, card, 500, 490);
        run(
            &mut w,
            0,
            2 + k as u32,
            CommandKind::CutWire {
                squad: s,
                at: c(500, 501),
            },
        );
        assert!(
            matches!(w.game().unwrap().squads[&s].order, Order::CutWire { .. }),
            "{card}"
        );
    }
}

#[test]
fn cut_time_follows_tier() {
    for (card, secs) in [
        ("ww2:ger_engineer_squad", 10),
        ("ww2:ger_sturmpioniergruppe", 5),
        ("ww2:ger_baupioniergruppe", 20),
    ] {
        let mut w = world();
        wire_line(&mut w, c(480, 500), c(520, 500));
        let s = spawn(&mut w, 0, card, 501, 495);
        run(
            &mut w,
            0,
            0,
            CommandKind::CutWire {
                squad: s,
                at: c(501, 500),
            },
        );
        let target = sections_between(&w, 500, 502);
        assert_eq!(target.len(), 1);
        // Ticks with work added: from the first tick whose work shows (the
        // command's own tick included) to the one that removed it.
        let mut first = None;
        let mut done = None;
        for t in 0..60 * HZ {
            if t > 0 {
                w.step(&[]);
            }
            if first.is_none()
                && matches!(
                    w.game().unwrap().squads[&s].order,
                    Order::CutWire { work, .. } if work > 0
                )
            {
                first = Some(t);
            }
            if !fs(&w).wire.contains_key(&target[0]) {
                done = Some(t);
                break;
            }
        }
        let ticks = done.expect("cut") - first.expect("started") + 1;
        assert_eq!(ticks, secs * HZ, "{card}");
        assert_eq!(fs(&w).accounts[0].wire_cut, 1);
        assert_eq!(fs(&w).wire.len(), 19);
    }
}

fn place(w: &mut World, seq: u32, s: SquadId, target: FxVec2) {
    let units = members(w, s);
    run(w, 0, seq, CommandKind::PlaceBangalore { units, target });
}

#[test]
fn bangalore_bursts_along_its_length() {
    let mut w = world();
    let s = spawn(&mut w, 0, "ww2:ger_sturmpioniergruppe", 500, 480);
    let item = item_id(&w, "ww2:gestreckte_ladung");
    place(&mut w, 0, s, c(500, 500));
    let mut charge = None;
    let mut bursts = Vec::new();
    for _ in 0..40 * HZ {
        w.step(&[]);
        if charge.is_none() {
            charge = fs(&w).charges.first().cloned();
        }
        let b: Vec<[f32; 3]> = w
            .snapshot()
            .explosions
            .iter()
            .filter(|e| e.weapon == item)
            .map(|e| e.pos)
            .collect();
        if !b.is_empty() {
            bursts = b;
            break;
        }
    }
    let ch = charge.expect("a charge was laid");
    assert_eq!(fs(&w).accounts[0].bangalores, 1);
    assert!(fs(&w).charges.is_empty());
    // Laid from within 2.5 m of the target, 3 m long toward it.
    assert!((ch.from - c(500, 500)).length() <= Fx::from_ratio(5, 2));
    let len = (ch.to - ch.from).length();
    assert!((len - Fx::from_int(3)).abs() < Fx::from_ratio(1, 20));
    // Three bursts in one tick at 1/6, 3/6, 5/6 of its length.
    assert_eq!(bursts.len(), 3);
    for (k, b) in bursts.iter().enumerate() {
        let want = ch.from + (ch.to - ch.from).mul_int(2 * k as i32 + 1).div_int(6);
        let got = FxVec2::new(
            Fx::from_milli((b[0] * 1000.0).round() as i32),
            Fx::from_milli((b[1] * 1000.0).round() as i32),
        );
        assert!((got - want).length() < Fx::from_ratio(1, 20), "{k}");
    }
}

#[test]
fn bangalore_clears_a_gap_and_counts_blown() {
    let mut w = world();
    // A north-south line 20 m east of the section's line (its men stand
    // along y, facing east: the carrier walks straight to it).
    wire_line(&mut w, c(520, 480), c(520, 520));
    let s = spawn(&mut w, 0, "ww2:usa_assault_section", 500, 500);
    place(&mut w, 0, s, c(520, 500));
    for _ in 0..40 * HZ {
        w.step(&[]);
        if fs(&w).accounts[0].bangalores == 1 && fs(&w).charges.is_empty() {
            break;
        }
    }
    steps(&mut w, 2);
    let a = fs(&w).accounts[0].clone();
    assert_eq!(a.bangalores, 1);
    // The two sections either side of the target are gone.
    assert!(fs(&w).wire.values().all(|x| {
        let mid = x.a + (x.b - x.a).div_int(2);
        (mid - c(520, 500)).length() > Fx::from_int(2)
    }));
    let gone = 20 - fs(&w).wire.len() as u32;
    assert!(gone >= 2);
    assert_eq!(a.wire_blown, gone);
    // The placer lives (no friendly blast).
    assert!(
        members(&w, s)
            .iter()
            .all(|m| w.state.entities.combat[m.index()].hp > 0)
    );
}

#[test]
fn bangalore_needs_breach_and_an_item() {
    let mut w = world();
    let k = frontline_content::ItemKind::Bangalore.index();
    // No breach: rifles and the field engineers.
    for (seq, card) in ["ww2:ger_grenadiergruppe", "ww2:ger_engineer_squad"]
        .into_iter()
        .enumerate()
    {
        let s = spawn(&mut w, 0, card, 500, 480);
        place(&mut w, seq as u32, s, c(500, 500));
        assert_eq!(refused(&w), Some(FortRefusal::CannotBuild), "{card}");
    }
    // Breach but no Bangalore left.
    let sp = spawn(&mut w, 0, "ww2:ger_sturmpioniergruppe", 520, 480);
    for m in members(&w, sp) {
        w.edit_soldier(m, |kit, _| kit.items[k] = 0);
    }
    place(&mut w, 5, sp, c(520, 500));
    assert_eq!(refused(&w), Some(FortRefusal::CannotBuild));
    // With one: the nearest carrier takes it out and walks.
    let sp = spawn(&mut w, 0, "ww2:ger_sturmpioniergruppe", 540, 480);
    let before: u32 = members(&w, sp)
        .iter()
        .map(|m| w.state.entities.kit[m.index()].items[k] as u32)
        .sum();
    assert_eq!(before, 2);
    place(&mut w, 6, sp, c(540, 500));
    let placing: Vec<SimId> = members(&w, sp)
        .into_iter()
        .filter(|m| {
            w.state.entities.kit[m.index()]
                .windup
                .is_some_and(|x| x.kind == frontline_content::ItemKind::Bangalore)
        })
        .collect();
    assert_eq!(placing.len(), 1);
    assert!(w.state.entities.unit[placing[0].index()].detached);
    let after: u32 = members(&w, sp)
        .iter()
        .map(|m| w.state.entities.kit[m.index()].items[k] as u32)
        .sum();
    assert_eq!(after, 1);
}

// ------------------------------------------------------------ Build C --

#[test]
fn fort_snapshot_lists_pieces_wire_sites_charges() {
    let mut w = world();
    // Nothing yet: an empty fort snapshot.
    w.step(&[]);
    assert_eq!(w.snapshot().fort, crate::snapshot::FortSnapshot::default());
    // A wall piece (player 0), enemy wire (player 1), a waiting site with
    // carried kg on another squad, and a laid charge.
    let piece = wall_at(&mut w, 0);
    wire_line(&mut w, c(600, 480), c(600, 520));
    let wall = fort(&w, "ww2:sandbag_wall");
    let eng = spawn(&mut w, 0, "ww2:ger_engineer_squad", 400, 400);
    fortify(&mut w, 0, eng, wall, vec![c(400, 405), c(404, 405)]);
    let carrier = spawn(&mut w, 0, "ww2:ger_engineer_squad", 300, 300);
    carry(&mut w, carrier, 12);
    let sp = spawn(&mut w, 0, "ww2:ger_sturmpioniergruppe", 700, 480);
    place(&mut w, 3, sp, c(700, 500));
    let mut laid = false;
    for _ in 0..30 * HZ {
        w.step(&[]);
        if !fs(&w).charges.is_empty() {
            laid = true;
            break;
        }
    }
    assert!(laid, "a charge was laid");
    let s = w.snapshot();
    let f = &s.fort;
    // Pieces: the wall piece with its box and full hp.
    let q = f
        .pieces
        .iter()
        .find(|q| q.id == piece)
        .expect("the wall piece");
    assert_eq!(q.owner, 0);
    assert_eq!(q.fort, wall);
    assert_eq!(q.part, 0, "sandbag");
    assert_eq!(q.hp_pct, 100);
    assert!((q.center[0] - 500.0).abs() < 0.5 && (q.center[1] - 500.0).abs() < 0.5);
    assert!(q.half.iter().all(|&h| h > 0.0));
    let ids: Vec<u32> = f.pieces.iter().map(|q| q.id).collect();
    assert!(ids.windows(2).all(|p| p[0] < p[1]), "ascending id");
    // Wire: twenty 2 m sections of player 1 with the def's band.
    assert_eq!(f.wire.len(), fs(&w).wire.len());
    assert_eq!(f.wire.len(), 20);
    assert!(f.wire.iter().all(|x| x.owner == 1 && x.hp_pct == 100));
    assert!(f.wire.iter().all(|x| (x.band - 1.5).abs() < 1e-3));
    assert!(f.wire.iter().all(|x| (x.height - 0.9).abs() < 1e-3));
    // The site: waiting for its kg, no progress, two segments.
    assert_eq!(f.works.len(), 1);
    let wk = &f.works[0];
    assert_eq!((wk.owner, wk.fort, wk.builder), (0, wall, eng.0));
    assert_eq!(wk.step, WorkStep::Wait as u8);
    assert_eq!((wk.progress_pct, wk.materials_pct), (0, 0));
    assert_eq!((wk.seg, wk.segments), (0, 2));
    assert_eq!(wk.line.len(), 2);
    // The carried kg on the carrier's squad.
    let sq = s.squads.iter().find(|q| q.id == carrier.0).unwrap();
    assert!((sq.materials_kg - 12.0).abs() < 1e-3);
    assert!(
        s.squads
            .iter()
            .filter(|q| q.id != carrier.0)
            .all(|q| q.materials_kg == 0.0)
    );
    // The charge, its fuse counting down.
    assert_eq!(f.charges.len(), 1);
    let ch = &f.charges[0];
    assert_eq!(ch.owner, 0);
    assert_eq!(ch.item, item_id(&w, "ww2:gestreckte_ladung"));
    assert!(ch.fuse_ticks > 0 && ch.fuse_ticks <= 8 * HZ as u32);
    // A refusal shows in `fort_refusals` for that tick only.
    let rifles = spawn(&mut w, 0, "ww2:ger_grenadiergruppe", 200, 200);
    fortify(&mut w, 0, rifles, wall, vec![c(200, 205), c(202, 205)]);
    assert_eq!(
        w.snapshot().fort_refusals.last().map(|e| e.refusal),
        Some(FortRefusal::CannotBuild)
    );
    w.step(&[]);
    assert!(w.snapshot().fort_refusals.is_empty());
}

// ------------------------------------------------------- Fix (review) --

#[test]
fn carried_kg_follow_casualties_and_save() {
    let mut w = world();
    let eng = spawn(&mut w, 0, "ww2:ger_engineer_squad", 500, 500);
    let men = members(&w, eng);
    let per = 20_000u32;
    carry(&mut w, eng, 20 * men.len() as u32);
    // One man killed: the kg shrink to what the rest may carry.
    assert!(w.kill_soldier(men[0]));
    assert_eq!(fs(&w).carried_g[&eng], per * (men.len() as u32 - 1));
    steps(&mut w, 2);
    let back = World::load(&w.save(), 1).expect("loads after a casualty");
    assert_eq!(back.game().unwrap().fort(), fs(&w));
    // Every man killed: the entry goes with the squad.
    for &m in &men[1..] {
        assert!(w.kill_soldier(m));
    }
    assert!(!fs(&w).carried_g.contains_key(&eng));
    steps(&mut w, 2);
    assert!(World::load(&w.save(), 1).is_ok(), "loads after a wipe-out");
}

#[test]
fn wrecked_truck_loses_its_material_and_saves() {
    let mut w = world();
    let truck = spawn(&mut w, 0, "ww2:ger_opel_blitz_squad", 510, 500);
    let tv = members(&w, truck)[0];
    w.edit_vehicle(tv, |v| v.material_g = 80_000);
    assert!(w.wreck_vehicle(tv));
    assert_eq!(w.game().unwrap().vehicles[&tv].material_g, 0);
    steps(&mut w, 2);
    assert!(World::load(&w.save(), 1).is_ok(), "loads after the wreck");
}

#[test]
fn truck_auto_load_counts_its_material() {
    let mut w = world();
    let base = w.game().unwrap().base_of(PlayerId(0));
    let truck = spawn(&mut w, 0, "ww2:ger_opel_blitz_squad", 0, 0);
    let tv = members(&w, truck)[0];
    w.place_squad(truck, base, FxVec2::new(Fx::ONE, Fx::ZERO));
    w.edit_vehicle(tv, |v| v.cargo_g = [0; 3]);
    let cap = {
        let g = w.game().unwrap();
        let ty = g.vehicles[&tv].type_idx as usize;
        g.setup.vehicles[ty].supply.unwrap().cap_g
    };
    // Material for all but 100 kg of the cap, then the auto-load.
    run(
        &mut w,
        0,
        0,
        CommandKind::LoadMaterials {
            squad: truck,
            kg: (cap / 1000 - 100) as u16,
        },
    );
    assert_eq!(w.game().unwrap().vehicles[&tv].material_g, cap - 100_000);
    steps(&mut w, 120 * HZ);
    let vs = &w.game().unwrap().vehicles[&tv];
    let held: u32 = vs.cargo_g.iter().sum::<u32>() + vs.material_g;
    assert!(vs.cargo_g.iter().sum::<u32>() > 0, "it loaded some cargo");
    assert!(held <= cap, "{held} > {cap}");
    assert!(World::load(&w.save(), 1).is_ok());
}

#[test]
fn point_fortify_refuses_an_off_map_facing_point() {
    let mut w = world();
    let nest = fort(&w, "ww2:mg_nest");
    let eng = spawn(&mut w, 0, "ww2:ger_engineer_squad", 20, 500);
    fortify(&mut w, 0, eng, nest, vec![c(3, 500), c(-2, 500)]);
    assert_eq!(refused(&w), Some(FortRefusal::BadSpot));
    assert!(fs(&w).works.is_empty());
    // Facing inward: accepted, and the state saves and loads.
    fortify(&mut w, 0, eng, nest, vec![c(3, 500), c(8, 500)]);
    assert_eq!(fs(&w).works.len(), 1);
    steps(&mut w, 2);
    assert!(World::load(&w.save(), 1).is_ok());
}

#[test]
fn load_rejects_a_dig_step_without_a_pit_and_the_tick_never_panics() {
    let (w, eng) = busy();
    let bytes = w.save();
    // A sandbag wall (no pit) work at the Dig step; an over_hq-less roof.
    let bad: Vec<FortEdit> = vec![
        Box::new(|f| f.works.values_mut().next().unwrap().step = WorkStep::Dig),
        Box::new(|f| f.works.values_mut().next().unwrap().hq = Some(0)),
    ];
    for (k, e) in bad.into_iter().enumerate() {
        let mut x = World::load(&bytes, 1).unwrap();
        x.edit_fort(|f| e(f));
        assert!(World::load(&x.save(), 1).is_err(), "bad state {k} loaded");
    }
    let mut x = World::load(&bytes, 1).unwrap();
    let roof = fort(&x, "ww2:hq_dugout");
    x.edit_fort(|f| f.works.values_mut().next().unwrap().fort = roof);
    assert!(
        World::load(&x.save(), 1).is_err(),
        "a roof work with no HQ loaded"
    );
    // The tick on such a state (not loaded: edited) removes the work.
    let mut x = World::load(&bytes, 1).unwrap();
    x.edit_fort(|f| f.works.values_mut().next().unwrap().step = WorkStep::Dig);
    run(&mut x, 0, 9, CommandKind::Stop { squad: eng });
    assert!(fs(&x).works.is_empty());
}

#[test]
fn roof_work_whose_hq_goes_sets_its_builder_idle() {
    let mut w = world();
    let f = fort(&w, "ww2:hq_dugout");
    let hq = def(&w, 0, StockKind::Hq, None);
    let sid = add_built(&mut w, 0, hq, c(600, 600), [0, 0, 0]);
    let eng = spawn(&mut w, 0, "ww2:ger_engineer_squad", 620, 620);
    fortify(&mut w, 0, eng, f, vec![c(600, 600), c(610, 600)]);
    let id = *fs(&w).works.keys().next().expect("accepted");
    assert_eq!(
        w.game().unwrap().squads[&eng].order,
        Order::Fortify { work: id }
    );
    w.edit_supply6(|s| {
        s.structures.remove(&sid);
    });
    steps(&mut w, 1);
    assert!(fs(&w).works.is_empty());
    assert_eq!(w.game().unwrap().squads[&eng].order, Order::Idle);
}

#[test]
fn called_off_bangalore_carrier_rejoins_his_squad() {
    let mut w = world();
    let s = spawn(&mut w, 0, "ww2:ger_sturmpioniergruppe", 500, 440);
    place(&mut w, 0, s, c(500, 500));
    steps(&mut w, HZ);
    let k = frontline_content::ItemKind::Bangalore;
    let m = members(&w, s)
        .into_iter()
        .find(|m| {
            w.state.entities.kit[m.index()]
                .windup
                .is_some_and(|x| x.kind == k)
        })
        .expect("a carrier walks");
    assert!(w.state.entities.unit[m.index()].detached);
    // Pinned on the way: the placing is called off.
    w.edit_soldier(m, |_, c| c.pinned = true);
    steps(&mut w, 1);
    let u = &w.state.entities.unit[m.index()];
    assert!(w.state.entities.kit[m.index()].windup.is_none());
    assert!(!u.detached, "he rejoins his squad");
    assert_eq!(u.order, crate::unit::UnitOrder::Idle);
    assert_eq!(
        w.state.entities.kit[m.index()].items[k.index()],
        1,
        "item back"
    );
}
