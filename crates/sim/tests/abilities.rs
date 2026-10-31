//! Thrown items (S1b abilities): thrower choice, range and validation,
//! wind-up, flight and fuse, frags in a trench, satchels cutting a trench,
//! resupply in base, smoke blocking spotting and running out, the AI's
//! throws, and determinism with throws in the command log (thread counts,
//! save → load → continue with items in flight and smoke up).
//!
//! Squads are staged with `World::place_squad` / `World::dig_trench_now`
//! (setup tools, not commands) on the open ground east of the map centre
//! that `tests/combat.rs` uses.

use std::path::Path;

use frontline_content::{Difficulty, MatchRequest, MatchSetup, ModDefs};
use frontline_sim::{
    BlastKind, Command, CommandKind, Fx, FxVec2, ItemKind, NavPhase, PlayerId, SimId, SquadId,
    UnitOrder, World,
};

fn setup(seed: u64, ai: [bool; 2]) -> MatchSetup {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2");
    let defs = ModDefs::load(&root).unwrap_or_else(|e| panic!("{e}"));
    defs.resolve(&MatchRequest::skirmish(
        seed,
        "ww2:germany",
        "ww2:usa",
        Difficulty::Normal,
    ))
    .map(|mut s| {
        for (p, ai) in s.players.iter_mut().zip(ai) {
            p.is_ai = ai;
        }
        s
    })
    .unwrap_or_else(|e| panic!("{e}"))
}

/// Both sides driven by hand.
fn world(seed: u64) -> World {
    World::new_match(setup(seed, [false, false]), 2).unwrap()
}

fn cmd(player: u8, seq: u32, kind: CommandKind) -> Command {
    Command {
        player: PlayerId(player),
        seq,
        kind,
    }
}

const EAST: FxVec2 = FxVec2::new(Fx::ONE, Fx::ZERO);
const WEST: FxVec2 = FxVec2::new(Fx::NEG_ONE, Fx::ZERO);
const NORTH: FxVec2 = FxVec2::new(Fx::ZERO, Fx::ONE);

fn at(x: i32, y: i32) -> FxVec2 {
    FxVec2::from_int(x, y)
}

/// Player `p`'s squads of type `ty`.
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

fn members(w: &World, sq: SquadId) -> Vec<SimId> {
    w.game().unwrap().squad(sq).unwrap().members.clone()
}

fn pos(w: &World, id: SimId) -> FxVec2 {
    w.entities().transform()[id.index()].pos.xy()
}

fn items(w: &World, id: SimId, kind: ItemKind) -> u8 {
    w.entities().kit()[id.index()].items[kind.index()]
}

fn winding(w: &World, id: SimId) -> bool {
    w.entities().kit()[id.index()].windup.is_some()
}

/// Steps until `done(w)` or `max_ticks`; returns whether `done` held.
fn run_until(w: &mut World, max_ticks: u64, mut done: impl FnMut(&World) -> bool) -> bool {
    for _ in 0..max_ticks {
        if done(w) {
            return true;
        }
        w.step(&[]);
    }
    done(w)
}

/// Steps one tick with `kind` from player `p`; returns whether it applied.
fn order(w: &mut World, p: u8, kind: CommandKind) -> bool {
    let before = w.stats().commands_applied;
    let seq = w.tick() as u32;
    w.step(&[cmd(p, seq, kind)]);
    w.stats().commands_applied == before + 1
}

fn throw(units: Vec<SimId>, item: ItemKind, target: FxVec2) -> CommandKind {
    CommandKind::Throw {
        units,
        item,
        target,
    }
}

/// Buys a squad of type `ty` for player `p` and steps until it arrives.
fn call_in(w: &mut World, p: u8, ty: &str) -> SquadId {
    let t = w.match_setup().unwrap().squad_type_index(ty).unwrap();
    let known: Vec<SquadId> = w.game().unwrap().squads().keys().copied().collect();
    assert!(order(w, p, CommandKind::CallIn { squad_type: t }));
    for _ in 0..30 * 60 {
        if let Some(id) = w
            .game()
            .unwrap()
            .squads()
            .keys()
            .find(|id| !known.contains(id))
        {
            return *id;
        }
        w.step(&[]);
    }
    panic!("call-in never arrived");
}

/// Moves player 1's starting squads far away from the test ground.
fn park_enemy(w: &mut World) {
    for (k, sq) in squads(w, 1, "ww2:usa_rifle_squad")
        .into_iter()
        .chain(squads(w, 1, "ww2:usa_engineer_squad"))
        .enumerate()
    {
        w.place_squad(sq, at(880 - 30 * k as i32, 700), WEST);
    }
}

#[test]
fn throw_picks_the_nearest_eligible_man_and_checks_the_rest() {
    let mut w = world(1);
    park_enemy(&mut w);
    let sq = squads(&w, 0, "ww2:ger_rifle_squad")[0];
    w.place_squad(sq, at(430, 420), EAST);
    let men = members(&w, sq);
    let leader = men[0];
    assert!(w.entities().kit()[leader.index()].leader);
    assert_eq!(items(&w, leader, ItemKind::Frag), 2);
    assert_eq!(items(&w, leader, ItemKind::Smoke), 1);
    assert_eq!(items(&w, men[1], ItemKind::Frag), 1);
    assert_eq!(items(&w, men[1], ItemKind::Smoke), 0);
    // Squad totals for the ability buttons: 9 frags, 1 smoke, no satchel.
    let card = |w: &World| {
        w.snapshot()
            .squads
            .iter()
            .find(|s| s.id == sq.0)
            .unwrap()
            .items
    };
    assert_eq!(card(&w), [9, 1, 0]);

    // Nearest to the target, ties by lowest id: recomputed here.
    let target = at(455, 423);
    let nearest = |w: &World, men: &[SimId]| {
        *men.iter()
            .min_by_key(|&&m| ((target - pos(w, m)).length_sq_raw(), m))
            .unwrap()
    };
    let first = nearest(&w, &men);
    assert!(order(&mut w, 0, throw(men.clone(), ItemKind::Frag, target)));
    for &m in &men {
        assert_eq!(winding(&w, m), m == first, "{m:?}");
    }
    assert_eq!(items(&w, first, ItemKind::Frag), (first == leader) as u8);
    let u = &w.entities().unit()[first.index()];
    assert!(u.detached && u.order == UnitOrder::Idle && u.nav.phase == NavPhase::Holding);
    assert_eq!(card(&w), [8, 1, 0]);
    assert!(w.snapshot().soldier_throwing.iter().filter(|t| **t).count() == 1);
    // He is busy: the next throw goes to the next nearest man.
    let rest: Vec<SimId> = men.iter().copied().filter(|&m| m != first).collect();
    let second = nearest(&w, &rest);
    assert!(order(&mut w, 0, throw(men.clone(), ItemKind::Frag, target)));
    assert!(winding(&w, second));

    // Range: nobody within 30 m, or the target inside the 5 m minimum.
    assert!(!order(
        &mut w,
        0,
        throw(men.clone(), ItemKind::Frag, at(480, 420))
    ));
    let m = men[3];
    let near = pos(&w, m) + FxVec2::from_int(2, 0);
    assert!(!order(&mut w, 0, throw(vec![m], ItemKind::Frag, near)));
    // Off the map, empty, too many, foreign or wrong-kind sets.
    assert!(!order(
        &mut w,
        0,
        throw(men.clone(), ItemKind::Frag, at(-5, 420))
    ));
    assert!(!order(&mut w, 0, throw(vec![], ItemKind::Frag, target)));
    assert!(!order(
        &mut w,
        0,
        throw(vec![men[4]; 121], ItemKind::Frag, target)
    ));
    let enemy = members(&w, squads(&w, 1, "ww2:usa_rifle_squad")[0]);
    assert!(!order(&mut w, 0, throw(enemy, ItemKind::Frag, target)));
    // Only the leader carries smoke; rifle squads have no satchel.
    assert!(!order(
        &mut w,
        0,
        throw(vec![men[5]], ItemKind::Smoke, target)
    ));
    assert!(!order(
        &mut w,
        0,
        throw(men.clone(), ItemKind::Satchel, target)
    ));
    assert!(order(
        &mut w,
        0,
        throw(men.clone(), ItemKind::Smoke, target)
    ));
    assert!(winding(&w, leader) || first == leader || second == leader);
    // MG teams carry nothing.
    let mg_sq = call_in(&mut w, 0, "ww2:ger_mg_team");
    w.place_squad(mg_sq, at(430, 440), EAST);
    let mg = members(&w, mg_sq);
    assert!(!order(&mut w, 0, throw(mg, ItemKind::Frag, at(450, 440))));

    // ThrowEach: every eligible man throws, each with his own scatter.
    let fresh: Vec<SimId> = men
        .iter()
        .copied()
        .filter(|&m| !winding(&w, m) && items(&w, m, ItemKind::Frag) > 0)
        .collect();
    assert!(fresh.len() >= 5);
    let before = w.stats().throws;
    assert!(order(
        &mut w,
        0,
        CommandKind::ThrowEach {
            units: men.clone(),
            item: ItemKind::Frag,
            target,
        }
    ));
    assert!(fresh.iter().all(|&m| winding(&w, m)));
    assert!(run_until(&mut w, 90, |w| w.stats().throws
        == before + fresh.len() as u64));
    assert_eq!(w.game().unwrap().thrown().len(), fresh.len());
    assert!(run_until(&mut w, 300, |w| w
        .game()
        .unwrap()
        .thrown()
        .iter()
        .all(|t| t.landed)));
    let mut spots: Vec<(i32, i32)> = w
        .game()
        .unwrap()
        .thrown()
        .iter()
        .map(|t| (t.pos.x.raw(), t.pos.y.raw()))
        .collect();
    let n = spots.len();
    spots.sort_unstable();
    spots.dedup();
    assert_eq!(spots.len(), n, "scatter differs per thrower");
}

#[test]
fn a_frag_winds_up_lobs_lands_and_bursts_on_its_fuse() {
    let mut w = world(2);
    park_enemy(&mut w);
    let sq = squads(&w, 0, "ww2:ger_rifle_squad")[0];
    w.place_squad(sq, at(430, 420), EAST);
    let m = members(&w, sq)[2];
    let target = pos(&w, m) + FxVec2::from_int(25, 0);
    let ground = w.terrain().height_at(target);
    let t0 = w.tick();
    assert!(order(&mut w, 0, throw(vec![m], ItemKind::Frag, target)));
    // Wind-up: about a second in hand, not firing, facing the target.
    assert!(run_until(&mut w, 60, |w| !w
        .game()
        .unwrap()
        .thrown()
        .is_empty()));
    let released = w.tick() - t0;
    assert!((29..=31).contains(&released), "released after {released}");
    assert!(!winding(&w, m) && w.stats().throws == 1);
    let yaw = w.entities().transform()[m.index()].yaw;
    assert!(yaw.abs() < Fx::from_ratio(1, 10), "{yaw:?}");
    let t = &w.game().unwrap().thrown()[0];
    assert!(!t.landed && t.thrower == m && t.owner == PlayerId(0));
    assert_eq!(w.snapshot().thrown.len(), 1);
    assert_eq!(w.snapshot().thrown[0].kind, ItemKind::Frag);
    // A lob: well above the straight line half way.
    let mut apex = Fx::ZERO;
    assert!(run_until(&mut w, 200, |w| {
        let t = &w.game().unwrap().thrown()[0];
        apex = apex.max(t.pos.z - ground);
        t.landed
    }));
    assert!(apex > Fx::from_int(5), "apex {apex:?}");
    let flight = w.tick() - t0 - released;
    assert!((60..=80).contains(&flight), "flight {flight} ticks");
    let land = w.game().unwrap().thrown()[0].pos;
    // 6 % scatter per axis at 25 m.
    assert!((land.xy() - target).length() < Fx::from_int(3), "{land:?}");
    // It lies there until 4 s after the throw began, then bursts.
    let mut burst = None;
    assert!(run_until(&mut w, 120, |w| {
        let e = w.snapshot().explosions.first().cloned();
        burst = e;
        burst.is_some()
    }));
    let fuse = w.tick() - t0;
    assert!((118..=122).contains(&fuse), "burst after {fuse}");
    let e = burst.unwrap();
    assert_eq!(e.kind, BlastKind::Frag);
    assert!(w.game().unwrap().thrown().is_empty());
    assert!(w.terrain().height_at(land.xy()) < land.z, "small crater");
    // The thrower stays detached where he stood.
    let u = &w.entities().unit()[m.index()];
    assert!(u.detached && u.order == UnitOrder::Idle);
}

#[test]
fn a_wind_up_is_called_off_by_another_order() {
    let mut w = world(3);
    park_enemy(&mut w);
    let sq = squads(&w, 0, "ww2:ger_rifle_squad")[0];
    w.place_squad(sq, at(430, 420), EAST);
    let m = members(&w, sq)[3];
    let aim = pos(&w, m) + at(20, 0);
    assert!(order(&mut w, 0, throw(vec![m], ItemKind::Frag, aim)));
    assert_eq!(items(&w, m, ItemKind::Frag), 0);
    w.step(&[]);
    assert!(order(
        &mut w,
        0,
        CommandKind::MoveUnits {
            units: vec![m],
            target: at(430, 380),
        }
    ));
    assert!(!winding(&w, m));
    assert_eq!(items(&w, m, ItemKind::Frag), 1, "item kept");
    // A squad order regroups him: same.
    let aim = pos(&w, m) + at(20, 0);
    assert!(order(&mut w, 0, throw(vec![m], ItemKind::Frag, aim)));
    assert!(order(&mut w, 0, CommandKind::Stop { squad: sq }));
    assert!(!winding(&w, m) && items(&w, m, ItemKind::Frag) == 1);
    // StopUnits on him alone (he stays detached and idle): same.
    let aim = pos(&w, m) + at(20, 0);
    assert!(order(&mut w, 0, throw(vec![m], ItemKind::Frag, aim)));
    assert!(order(&mut w, 0, CommandKind::StopUnits { units: vec![m] }));
    assert!(!winding(&w, m) && items(&w, m, ItemKind::Frag) == 1);
    w.step(&[]);
    assert!(w.game().unwrap().thrown().is_empty() && w.stats().throws == 0);
}

/// A finished trench along x = 500 from y = 485 to 515, parapet east.
fn trench(w: &mut World) {
    w.dig_trench_now(&[at(500, 485), at(500, 515)], -1);
}

#[test]
fn frags_lobbed_into_a_trench_hit_the_men_in_it() {
    let mut w = world(4);
    trench(&mut w);
    let d = squads(&w, 1, "ww2:usa_rifle_squad")[0];
    w.place_squad(d, at(500, 500), EAST);
    let others = squads(&w, 1, "ww2:usa_rifle_squad")[1];
    w.place_squad(others, at(880, 700), WEST);
    let sq = squads(&w, 0, "ww2:ger_rifle_squad")[0];
    w.place_squad(sq, at(525, 500), WEST);
    let men = members(&w, sq);
    let base = w.terrain().base_height_at(at(500, 500));
    // Every man throws at the trench line from 25 m.
    assert!(order(
        &mut w,
        0,
        CommandKind::ThrowEach {
            units: men.clone(),
            item: ItemKind::Frag,
            target: at(500, 500),
        }
    ));
    let thrown = w.stats().throws;
    assert!(run_until(&mut w, 60, |w| w.stats().throws > thrown));
    assert!(run_until(&mut w, 200, |w| {
        let g = w.game().unwrap();
        !g.thrown().is_empty() && g.thrown().iter().all(|t| t.landed)
    }));
    // Below the base surface: on the floor or a wall, under the lip.
    let in_trench = w
        .game()
        .unwrap()
        .thrown()
        .iter()
        .filter(|t| t.pos.z < base)
        .count();
    let n = w.game().unwrap().thrown().len();
    assert!(n >= 6, "only {n} men threw");
    assert!(
        in_trench * 3 >= n,
        "{in_trench} of {n} landed in the trench"
    );
    // Damage only from the bursts: hit points lost in burst ticks.
    let defenders = members(&w, d);
    let hp = |w: &World, m: SimId| {
        let k = w.snapshot().soldier_ids.iter().position(|&s| s == m);
        k.map_or(0, |k| w.snapshot().soldier_hp[k] as u32)
    };
    let mut blast_dmg = 0u32;
    let mut frag_bursts = 0;
    for _ in 0..30 * 6 {
        let before: Vec<u32> = defenders.iter().map(|&m| hp(&w, m)).collect();
        w.step(&[]);
        let bursts = w
            .snapshot()
            .explosions
            .iter()
            .filter(|e| e.kind == BlastKind::Frag)
            .count();
        if bursts > 0 {
            frag_bursts += bursts;
            blast_dmg += defenders
                .iter()
                .zip(&before)
                .map(|(&m, &b)| b.saturating_sub(hp(&w, m)))
                .sum::<u32>();
        }
    }
    assert_eq!(frag_bursts, n);
    assert!(
        blast_dmg >= 200,
        "only {blast_dmg} hp lost to frags in the trench"
    );
}

#[test]
fn a_frag_aimed_at_the_trench_clears_the_parapet() {
    // No scatter: only the arc decides. Aimed at the floor itself, a 45°
    // lob used to come down on the near parapet.
    let mut s = setup(4, [false, false]);
    for it in &mut s.items {
        it.scatter_pct = 0;
    }
    let mut w = World::new_match(s, 1).unwrap();
    trench(&mut w);
    park_enemy(&mut w);
    let base = w.terrain().base_height_at(at(500, 500));
    let sq = squads(&w, 0, "ww2:ger_rifle_squad")[0];
    w.place_squad(sq, at(525, 500), WEST);
    let m = members(&w, sq)[3];
    let aim = FxVec2::new(Fx::from_int(500), pos(&w, m).y);
    assert!(order(&mut w, 0, throw(vec![m], ItemKind::Frag, aim)));
    assert!(run_until(&mut w, 300, |w| {
        let g = w.game().unwrap();
        !g.thrown().is_empty() && g.thrown().iter().all(|t| t.landed)
    }));
    let t = &w.game().unwrap().thrown()[0];
    assert!(
        t.pos.z < base - Fx::ONE && (t.pos.x - Fx::from_int(500)).abs() < Fx::ONE,
        "landed at {:?}, trench floor at {:?}",
        t.pos,
        base - Fx::from_milli(1600)
    );
}

#[test]
fn a_satchel_charge_cuts_the_trench() {
    let mut w = world(5);
    park_enemy(&mut w);
    trench(&mut w);
    let sq = squads(&w, 0, "ww2:ger_engineer_squad")[0];
    w.place_squad(sq, at(506, 500), WEST);
    let men = members(&w, sq);
    let y = pos(&w, men[0]).y.round_int();
    assert_eq!(items(&w, men[0], ItemKind::Satchel), 1);
    assert_eq!(items(&w, men[1], ItemKind::Satchel), 0);
    let h = |w: &World, x_mm: i32, y: i32| {
        w.terrain()
            .height_at(FxVec2::new(Fx::from_milli(x_mm), Fx::from_int(y)))
    };
    let floor = h(&w, 500_000, y);
    let parapet = (500_000..503_000)
        .step_by(250)
        .map(|x| h(&w, x, y))
        .max()
        .unwrap();
    let far_y = if y > 500 { 487 } else { 513 };
    let far = h(&w, 500_000, far_y);
    let base = w.terrain().base_height_at(at(500, y));
    assert!(floor < base - Fx::from_milli(1400) && parapet > base + Fx::from_milli(200));
    // Out of reach first (8 m), then placed from the trench edge.
    assert!(!order(
        &mut w,
        0,
        throw(men.clone(), ItemKind::Satchel, at(495, y))
    ));
    assert!(order(
        &mut w,
        0,
        throw(men.clone(), ItemKind::Satchel, at(500, y))
    ));
    assert!(winding(&w, men[0]));
    let mut kind = None;
    assert!(run_until(&mut w, 30 * 6, |w| {
        kind = w.snapshot().explosions.first().map(|e| e.kind);
        kind.is_some()
    }));
    assert_eq!(kind, Some(BlastKind::Satchel));
    let land = w.snapshot().explosions[0].pos;
    let cx = (land[0] * 1000.0) as i32;
    // The trench is filled in at the blast and the parapet blown down; the
    // trench away from the crater is untouched.
    let floor_after = h(&w, cx, y);
    assert!(
        floor_after > floor + Fx::from_milli(500),
        "{floor:?} → {floor_after:?}"
    );
    let parapet_after = (500_000..503_000)
        .step_by(250)
        .map(|x| h(&w, x, y))
        .max()
        .unwrap();
    assert!(parapet_after < parapet, "{parapet:?} → {parapet_after:?}");
    assert_eq!(h(&w, 500_000, far_y), far);
}

#[test]
fn men_in_their_base_get_items_back_up_to_their_loadout() {
    let mut w = world(6);
    park_enemy(&mut w);
    let hz = w.match_setup().unwrap().tick_hz as u64;
    let every = w.match_setup().unwrap().rules.items.resupply_ticks as u64;
    assert_eq!(every, 20 * hz);
    // Starting squads stand in the base at (96, 96).
    let home = squads(&w, 0, "ww2:ger_rifle_squad")[0];
    let m = members(&w, home)[1];
    let away_sq = squads(&w, 0, "ww2:ger_rifle_squad")[1];
    w.place_squad(away_sq, at(300, 300), EAST);
    let away = members(&w, away_sq)[1];
    assert!((pos(&w, m) - at(96, 96)).length() < Fx::from_int(40));
    let aim = pos(&w, m) + at(15, 0);
    assert!(order(&mut w, 0, throw(vec![m], ItemKind::Frag, aim)));
    let aim = pos(&w, away) + at(15, 0);
    assert!(order(&mut w, 0, throw(vec![away], ItemKind::Frag, aim)));
    let t0 = w.tick();
    assert!(run_until(&mut w, every + 10, |w| items(
        w,
        m,
        ItemKind::Frag
    ) == 1));
    let took = w.tick() - t0;
    assert!(took + 2 >= every && took <= every + 2, "{took} ticks");
    // Full again: no more.
    for _ in 0..every + 30 {
        w.step(&[]);
    }
    assert_eq!(items(&w, m, ItemKind::Frag), 1);
    assert_eq!(
        items(&w, away, ItemKind::Frag),
        0,
        "no resupply outside base"
    );
}

/// Player 1's rifle squad as a file pointing west at (540..561, 420), so a
/// cloud in front of it hides every man from the west.
fn smoke_rig(seed: u64) -> (World, SquadId) {
    let mut w = world(seed);
    let others = squads(&w, 0, "ww2:ger_rifle_squad");
    for (k, sq) in others.into_iter().enumerate() {
        w.place_squad(sq, at(150 + 30 * k as i32, 900), EAST);
    }
    let b = squads(&w, 1, "ww2:usa_rifle_squad")[0];
    w.place_squad(b, at(550, 420), NORTH);
    (w, b)
}

/// Player 0's MG team, called in and parked out of sight.
fn mg_team(w: &mut World) -> SquadId {
    let mg = call_in(w, 0, "ww2:ger_mg_team");
    w.place_squad(mg, at(150, 800), EAST);
    mg
}

/// Over `secs` seconds: ticks in which player 0 sees one of `sq`'s men,
/// player 0's shots, and `sq`'s losses.
fn watch(w: &mut World, sq: SquadId, secs: u64) -> (u32, u32, u32) {
    let men = members(w, sq);
    let (mut seen, mut shots) = (0, 0);
    let lost = w.game().unwrap().players()[1].losses;
    for _ in 0..secs * 30 {
        w.step(&[]);
        let s = w.snapshot();
        let vis = s
            .soldier_ids
            .iter()
            .zip(&s.soldier_visible)
            .any(|(id, v)| men.contains(id) && v & 1 != 0);
        seen += vis as u32;
        shots += s
            .shots
            .iter()
            .filter(|f| w.entities().owner()[f.shooter.index()] == PlayerId(0))
            .count() as u32;
    }
    (seen, shots, w.game().unwrap().players()[1].losses - lost)
}

#[test]
fn smoke_hides_a_squad_from_an_mg_at_100_m_then_runs_out() {
    // Without smoke the MG sees and fires on the squad.
    let (mut w, b) = smoke_rig(7);
    let mg = mg_team(&mut w);
    w.place_squad(mg, at(450, 420), EAST);
    let (seen, shots, _) = watch(&mut w, b, 15);
    assert!(
        seen > 300 && shots > 20,
        "control: seen {seen}, shots {shots}"
    );

    // The squad's leader throws smoke 28 m toward where the MG will be.
    let (mut w, b) = smoke_rig(7);
    let mg = mg_team(&mut w);
    let leader = members(&w, b)[0];
    assert!(order(
        &mut w,
        1,
        throw(vec![leader], ItemKind::Smoke, at(522, 420))
    ));
    let full = |w: &World| {
        w.snapshot()
            .smoke
            .first()
            .is_some_and(|c| c.radius >= 8.0 && c.density >= 1.0)
    };
    assert!(run_until(&mut w, 30 * 8, full));
    let cloud = w.game().unwrap().smoke()[0];
    assert!((cloud.pos.xy() - at(522, 420)).length() < Fx::from_int(3));
    w.place_squad(mg, at(450, 420), EAST);
    let (seen, shots, lost) = watch(&mut w, b, 20);
    assert_eq!((seen, shots, lost), (0, 0, 0), "seen ticks, shots, losses");
    // It thins out and is gone 33 s after it started; then he is seen.
    assert!(run_until(&mut w, 30 * 15, |w| w
        .game()
        .unwrap()
        .smoke()
        .is_empty()));
    let (seen, _, _) = watch(&mut w, b, 5);
    assert!(seen > 0);
}

#[test]
fn the_ai_throws_frags_into_trenches_and_satchels_from_close() {
    // Player 1 is the AI. Player 0 holds a trench; the AI's rifle squad and
    // engineers stand within throwing range.
    let mut w = World::new_match(setup(8, [false, true]), 2).unwrap();
    trench(&mut w);
    let d = squads(&w, 0, "ww2:ger_rifle_squad")[0];
    w.place_squad(d, at(500, 500), EAST);
    let rifle = squads(&w, 1, "ww2:usa_rifle_squad")[0];
    w.place_squad(rifle, at(522, 500), NORTH);
    let eng = squads(&w, 1, "ww2:usa_engineer_squad")[0];
    w.place_squad(eng, at(506, 490), NORTH);
    let mut kinds = Vec::new();
    run_until(&mut w, 30 * 40, |w| {
        kinds.extend(w.snapshot().explosions.iter().map(|e| e.kind));
        kinds.contains(&BlastKind::Frag) && kinds.contains(&BlastKind::Satchel)
    });
    assert!(kinds.contains(&BlastKind::Frag), "{kinds:?}");
    assert!(kinds.contains(&BlastKind::Satchel), "{kinds:?}");
    let ai = w.game().unwrap().ai(PlayerId(1)).unwrap();
    assert!(ai.issued > 0);
}

// ------------------------------------------------------------ determinism --

/// Scripted throws by player 0 next to two AIs, derived from the state so
/// every run sends the same ones: every few seconds one of its men throws
/// a frag (or each of three men does), or its first leader smoke, at the
/// nearest enemy (true positions: this is a test, not a player).
fn script(w: &World) -> Vec<Command> {
    let t = w.tick();
    if t < 600 || !t.is_multiple_of(61) {
        return Vec::new();
    }
    let ents = w.entities();
    let mine: Vec<SimId> = ents
        .soldiers()
        .filter(|id| ents.owner()[id.index()] == PlayerId(0))
        .collect();
    let theirs: Vec<SimId> = ents
        .soldiers()
        .filter(|id| ents.owner()[id.index()] == PlayerId(1))
        .collect();
    if mine.is_empty() || theirs.is_empty() {
        return Vec::new();
    }
    let k = (t / 61) as usize;
    let m = mine[k % mine.len()];
    let near = |from: FxVec2| {
        theirs
            .iter()
            .map(|&e| pos(w, e))
            .min_by_key(|e| ((*e - from).length_sq_raw(), e.x, e.y))
            .unwrap()
    };
    let from = pos(w, m);
    let enemy = near(from);
    // Within reach: at the enemy; else 20 m toward him.
    let target = if (enemy - from).length() <= Fx::from_int(30) {
        enemy
    } else {
        w.terrain()
            .clamp(from + (enemy - from).normalize_or_zero() * Fx::from_int(20))
    };
    let pick =
        |n: usize| -> Vec<SimId> { (0..n).map(|j| mine[(k + j * 3) % mine.len()]).collect() };
    let kind = match k % 4 {
        0 | 1 => throw(vec![m], ItemKind::Frag, target),
        2 => CommandKind::ThrowEach {
            units: pick(3),
            item: ItemKind::Frag,
            target,
        },
        _ => throw(mine.clone(), ItemKind::Smoke, target),
    };
    vec![cmd(0, 3_000_000 + k as u32, kind)]
}

fn ai_match(seed: u64, threads: usize) -> World {
    World::new_match(setup(seed, [true, true]), threads).unwrap()
}

/// Runs to `until` with the script, collecting published hashes; checks
/// the incremental hash against a full recompute every `full_every` ticks.
fn run(w: &mut World, until: u64, full_every: u64) -> Vec<(u64, u64)> {
    let mut out = Vec::new();
    while w.tick() < until {
        let cmds = script(w);
        if let Some(h) = w.step(&cmds) {
            out.push((w.tick(), h));
        }
        if full_every > 0 && w.tick().is_multiple_of(full_every) {
            assert_eq!(w.state_hash(), w.full_state_hash(), "tick {}", w.tick());
        }
    }
    out
}

#[test]
fn throws_in_an_ai_match_are_identical_across_thread_counts() {
    let end = 5 * 60 * 30;
    let mut reference_world = ai_match(21, 1);
    let reference = run(&mut reference_world, end, 83);
    let st = reference_world.stats();
    assert!(st.throws > 10 && st.explosions > 5, "{st:?}");
    for threads in [2, 3, 8] {
        let mut w = ai_match(21, threads);
        assert_eq!(run(&mut w, end, 0), reference, "{threads} threads");
    }
}

#[test]
fn throws_survive_save_load_continue() {
    let end = 5 * 60 * 30;
    let mut straight = ai_match(22, 2);
    let mut reference = Vec::new();
    let (mut seen_flight, mut seen_smoke) = (false, false);
    while straight.tick() < end {
        let cmds = script(&straight);
        if let Some(h) = straight.step(&cmds) {
            reference.push((straight.tick(), h));
        }
        let g = straight.game().unwrap();
        seen_flight |= g.thrown().iter().any(|t| !t.landed);
        seen_smoke |= !g.smoke().is_empty();
    }
    assert!(seen_flight && seen_smoke);

    // Save with an item in the air, one winding up and smoke standing.
    let mut first = ai_match(22, 2);
    let mut mid = 2 * 60 * 30;
    run(&mut first, mid, 0);
    let busy = |w: &World| {
        let g = w.game().unwrap();
        g.thrown().iter().any(|t| !t.landed)
            && !g.smoke().is_empty()
            && w.entities().kit().iter().any(|k| k.windup.is_some())
    };
    while !busy(&first) {
        mid += 1;
        run(&mut first, mid, 0);
        assert!(mid < end - 600, "no busy moment to save in");
    }
    let bytes = first.save();
    let mut resumed = World::load(&bytes, 3).unwrap();
    assert_eq!(resumed.state(), first.state());
    assert_eq!(resumed.state_hash(), first.state_hash());
    drop(first);
    let tail: Vec<_> = reference.into_iter().filter(|(t, _)| *t > mid).collect();
    assert_eq!(run(&mut resumed, end, 31), tail);
    assert_eq!(resumed.save(), straight.save(), "final save bytes differ");
    assert_eq!(resumed.snapshot(), straight.snapshot());
}
