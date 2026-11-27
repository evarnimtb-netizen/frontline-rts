//! S10b Build A tests (docs/slices/S10b-obstacles.md 3, 6; stage file
//! `S10b-build-A.md` 5): the saved layout, the empty fast path and the AI
//! guard. Behaviour tests come with Builds B-E.

use frontline_content::setup::Capability;

use super::s6_tests::{flat_world_with, spawn};
use super::*;
use crate::ai::fort::{WorkKind, fort_of};
use crate::fort::{Charge, FortState};
use crate::game::{Order, SquadId};

fn c(x: i32, y: i32) -> FxVec2 {
    FxVec2::from_int(x, y)
}

pub(super) fn fort(s: &frontline_content::MatchSetup, id: &str) -> u16 {
    s.forts.iter().position(|f| f.id == id).unwrap() as u16
}

pub(super) fn fs(w: &World) -> &FortState {
    w.game().unwrap().fort()
}

fn ai_setup(seed: u64) -> frontline_content::MatchSetup {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2");
    let defs = frontline_content::ModDefs::load(&root).unwrap();
    let req = frontline_content::MatchRequest::skirmish(
        seed,
        "ww2:germany",
        "ww2:usa",
        frontline_content::Difficulty::Normal,
    );
    let mut s = defs.resolve(&req).unwrap();
    s.players.iter_mut().for_each(|p| p.is_ai = true);
    s
}

/// A flat world holding every S10b saved field: a mine known to both
/// players and set off for the next tick, a demolition charge, a
/// `ClearMines` and a `Demolish` order (engineer squads get bits 12-14; no
/// card has them before Builds B-D).
fn s10b_world() -> (World, SquadId, SquadId) {
    let mut w = flat_world_with(1, |s| {
        for t in s.squad_types.iter_mut() {
            if t.id.contains("engineer") {
                t.caps |= Capability::PlaceMines.bit()
                    | Capability::ClearMines.bit()
                    | Capability::Demolish.bit();
            }
        }
    });
    let s = w.match_setup().unwrap();
    let (wall, field) = (fort(s, "ww2:sandbag_wall"), fort(s, "ww2:ap_minefield"));
    let satchel = s
        .items
        .iter()
        .position(|i| i.kind == frontline_content::ItemKind::Satchel)
        .unwrap() as u16;
    assert!(w.build_now(wall, &[c(400, 400), c(410, 400)], PlayerId(1)));
    let piece = *fs(&w).pieces.keys().next().unwrap();
    let mine = w.lay_mine_now(field, c(300, 300), PlayerId(1)).unwrap();
    let sweep = spawn(&mut w, 0, "ww2:ger_engineer_squad", 280, 300);
    let demo = spawn(&mut w, 0, "ww2:ger_engineer_squad", 390, 390);
    let tick = w.tick();
    w.edit_fort(|f| {
        let m = f.mines.get_mut(&mine).unwrap();
        m.known = 0b11;
        m.boom_tick = Some(tick + 1);
        f.charges.push(Charge {
            id: f.next_charge,
            owner: PlayerId(0),
            item: satchel,
            from: c(399, 400),
            to: c(400, 400),
            fuse_tick: tick + 90,
            piece: Some(piece),
        });
        f.next_charge += 1;
    });
    assert!(w.edit_squad(sweep, |q| {
        q.order = Order::ClearMines {
            a: c(280, 300),
            b: c(320, 300),
            along: Fx::from_int(12),
            lift: Some(mine),
            work: 100,
        }
    }));
    assert!(w.edit_squad(demo, |q| {
        q.order = Order::Demolish {
            piece,
            man: None,
            work: 200,
        }
    }));
    (w, sweep, demo)
}

#[test]
fn s10b_layout_roundtrips_and_hashes() {
    // Control: `#[serde(skip)]` on `FortState::mines` -> the loaded world
    // has no mine (the lift order's mine is unknown, the load fails).
    let (w, sweep, demo) = s10b_world();
    let bytes = w.save();
    let b = World::load(&bytes, 1).unwrap_or_else(|e| panic!("load: {e}"));
    assert_eq!(fs(&b), fs(&w));
    assert_eq!(fs(&b).mines.len(), 1);
    assert!(fs(&b).charges.iter().any(|ch| ch.piece.is_some()));
    let sq = |x: &World, s: SquadId| x.game().unwrap().squads()[&s].order.clone();
    assert_eq!(sq(&b, sweep), sq(&w, sweep));
    assert_eq!(sq(&b, demo), sq(&w, demo));
    assert!(matches!(sq(&b, sweep), Order::ClearMines { .. }));
    assert!(matches!(sq(&b, demo), Order::Demolish { .. }));
    assert_eq!(b.state_hash(), w.state_hash());
    assert_eq!(b.full_state_hash(), w.full_state_hash());
    assert_eq!(b.save(), bytes);
    // `Mine::known` is hashed.
    let mut k = World::load(&bytes, 1).unwrap();
    k.edit_fort(|f| f.mines.values_mut().for_each(|m| m.known = 0b01));
    assert_ne!(k.state_hash(), w.state_hash());
    assert_ne!(k.full_state_hash(), w.full_state_hash());
}

#[test]
fn empty_s10b_state_changes_nothing() {
    // 600 AI ticks with the S10b blueprints in the setup and without them
    // (mine items stay: removing them would shift every item index):
    // entities and squads equal every second, and no mine ever laid. Then
    // one far mine makes the fort state non-empty (the fast path is
    // skipped). Control: `FortState::is_empty` ignoring `mines` -> the last
    // assert fails.
    let run_ai = |strip: bool, mine: bool| {
        let mut s = ai_setup(3);
        if strip {
            s.forts.retain(|d| {
                d.mine_item.is_none()
                    && !d.blocks_vehicles
                    && d.pit != Some(frontline_content::setup::DigProfile::BunkerPit)
            });
        }
        let field = s.forts.iter().position(|d| d.mine_item.is_some());
        let mut w = World::new_match(s, 1).unwrap();
        let mut laid = false;
        if mine {
            w.lay_mine_now(field.unwrap() as u16, c(4, 1020), PlayerId(1))
                .unwrap();
            laid = !fs(&w).is_empty();
        }
        let mut seen = Vec::new();
        for t in 0..600u64 {
            w.step(&[]);
            if t % 30 == 0 {
                let g = w.game().unwrap();
                seen.push((w.entities().clone(), g.squads().clone()));
            }
        }
        assert_eq!(fs(&w).mines.len(), usize::from(mine));
        (seen, laid)
    };
    let (with, _) = run_ai(false, false);
    let (without, _) = run_ai(true, false);
    assert!(with == without, "the S10b blueprints changed AI play");
    let (far, laid) = run_ai(false, true);
    assert!(far == with, "a far mine changed AI play");
    assert!(
        laid,
        "a laid mine left the fort state empty (fast path taken)"
    );
}

#[test]
fn fort_of_ignores_s10b_blueprints() {
    // Control: the R29 filter (`mine_item`, `blocks_vehicles`) removed from
    // `ai::fort::fort_of` -> `Wall` is `ww2:ap_minefield`.
    let s = ai_setup(1);
    let id = |k| s.forts[fort_of(&s, k).unwrap() as usize].id.as_str();
    assert_eq!(id(WorkKind::Wall), "ww2:sandbag_wall");
    assert_eq!(id(WorkKind::Dugout), "ww2:dugout");
    assert_eq!(id(WorkKind::Nest), "ww2:mg_nest");
}

// ---- Build B: mines laid and set off (S10b-build-B.md 3).

const HZ: u64 = 30;

fn flat(edit: impl FnOnce(&mut frontline_content::MatchSetup)) -> World {
    flat_world_with(1, edit)
}

fn fortify(w: &mut World, p: u8, squad: SquadId, f: u16, line: Vec<FxVec2>) {
    super::s6_tests::run(
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

fn refused(w: &World) -> Option<crate::fort::FortRefusal> {
    w.fort_refusals().last().map(|e| e.refusal)
}

fn mine_at(w: &mut World, field: &str, x_dm: i32, y_dm: i32, owner: u8) -> u32 {
    let f = fort(w.match_setup().unwrap(), field);
    let p = FxVec2::new(Fx::from_milli(x_dm * 100), Fx::from_milli(y_dm * 100));
    w.lay_mine_now(f, p, PlayerId(owner)).unwrap()
}

fn pos(w: &World, m: SimId) -> FxVec2 {
    w.state.entities.transform[m.index()].pos.xy()
}

/// A rifleman of a fresh `p` squad at (x, y) (m), the rest of his squad
/// standing; returns him.
fn rifleman(w: &mut World, p: u8, card: &str, x: i32, y: i32) -> SimId {
    let s = spawn(w, p, card, x, y);
    super::s6_tests::members(w, s)[1]
}

/// Walks `m` (detached) to `to` for at most `max` ticks.
fn walk_to(w: &mut World, m: SimId, to: FxVec2, max: u64) {
    let p = w.state.entities.owner[m.index()].0;
    super::s6_tests::run(
        w,
        p,
        7,
        CommandKind::MoveUnits {
            units: vec![m],
            target: to,
        },
    );
    for _ in 0..max {
        if !w.state.entities.is_alive(m) || (pos(w, m) - to).length() < Fx::from_milli(300) {
            break;
        }
        w.step(&[]);
    }
}

fn set_off(w: &World, owner: usize) -> u32 {
    fs(w).accounts[owner].mines_set_off
}

#[test]
fn minefield_lays_one_mine_per_segment() {
    // Control: AP `segment_cm` 200 -> 300 -> 10 mines, not 15.
    let mut w = flat(|_| {});
    let f = fort(w.match_setup().unwrap(), "ww2:ap_minefield");
    let eng = spawn(&mut w, 0, "ww2:ger_engineer_squad", 500, 500);
    w.edit_fort(|s| {
        s.carried_g.insert(eng, 60_000);
    });
    fortify(&mut w, 0, eng, f, vec![c(500, 506), c(530, 506)]);
    assert_eq!(fs(&w).works.len(), 1, "{:?}", refused(&w));
    for _ in 0..400 * HZ {
        w.step(&[]);
        if fs(&w).works.is_empty() {
            break;
        }
    }
    let m = &fs(&w).mines;
    assert_eq!(m.len(), 15);
    for (k, m) in m.values().enumerate() {
        let want = FxVec2::from_int(502 + 2 * k as i32, 506);
        assert!(
            (m.pos - want).length() < Fx::from_milli(50),
            "mine {k} at {:?}",
            m.pos
        );
    }
    assert!(
        m.values()
            .all(|m| m.owner == PlayerId(0) && m.known == 0b01)
    );
    let a = &fs(&w).accounts[0];
    assert_eq!(a.mines_laid, 15);
    assert_eq!(a.works_done[f as usize], 1);
    assert_eq!(w.game().unwrap().squads()[&eng].order, Order::Idle);
}

#[test]
fn mine_cap_per_player() {
    // Control: AP `limit_per_player` 60 -> 100: the order is accepted and
    // the work lays its mine.
    let mut w = flat(|_| {});
    let f = fort(w.match_setup().unwrap(), "ww2:ap_minefield");
    for k in 0..59 {
        mine_at(&mut w, "ww2:ap_minefield", 1000 + 20 * k, 1000, 0);
    }
    let eng = spawn(&mut w, 0, "ww2:ger_engineer_squad", 500, 500);
    w.edit_fort(|s| {
        s.carried_g.insert(eng, 60_000);
    });
    // 59 + 2 > 60: refused at the command.
    fortify(&mut w, 0, eng, f, vec![c(500, 506), c(504, 506)]);
    assert_eq!(refused(&w), Some(crate::fort::FortRefusal::Cap));
    assert!(fs(&w).works.is_empty());
    // 59 + 1 fits; a 60th mine laid meanwhile ends the work at its mine.
    fortify(&mut w, 0, eng, f, vec![c(500, 506), c(502, 506)]);
    assert_eq!(fs(&w).works.len(), 1);
    mine_at(&mut w, "ww2:ap_minefield", 1000, 1100, 0);
    for _ in 0..120 * HZ {
        w.step(&[]);
        if fs(&w).works.is_empty() {
            break;
        }
    }
    assert!(fs(&w).works.is_empty());
    assert_eq!(fs(&w).mines.len(), 60, "the 61st mine is never laid");
    assert_eq!(refused(&w), Some(crate::fort::FortRefusal::Cap));
    assert_eq!(w.game().unwrap().squads()[&eng].order, Order::Idle);
}

#[test]
fn mine_laying_ends_when_line_done_or_interrupted() {
    // Control: keep a minefield work whose builder took another order
    // (`fort_tick` `keep`) -> the work is still there after the move.
    let mut w = flat(|_| {});
    let f = fort(w.match_setup().unwrap(), "ww2:at_minefield");
    let eng = spawn(&mut w, 0, "ww2:ger_engineer_squad", 500, 500);
    w.edit_fort(|s| {
        s.carried_g.insert(eng, 200_000);
    });
    // Done: two segments of 4 m.
    fortify(&mut w, 0, eng, f, vec![c(500, 506), c(508, 506)]);
    for _ in 0..300 * HZ {
        w.step(&[]);
        if fs(&w).works.is_empty() {
            break;
        }
    }
    assert_eq!(fs(&w).mines.len(), 2);
    assert_eq!(fs(&w).accounts[0].works_done[f as usize], 1);
    // Interrupted after 3 mines of a 40 m line.
    fortify(&mut w, 0, eng, f, vec![c(500, 520), c(540, 520)]);
    assert_eq!(fs(&w).works.len(), 1);
    for _ in 0..600 * HZ {
        w.step(&[]);
        if fs(&w).mines.len() == 5 {
            break;
        }
    }
    assert_eq!(fs(&w).mines.len(), 5);
    super::s6_tests::run(
        &mut w,
        0,
        9,
        CommandKind::Move {
            squad: eng,
            target: c(450, 450),
        },
    );
    w.step(&[]);
    assert!(fs(&w).works.is_empty(), "the work goes with the order");
    assert_eq!(fs(&w).mines.len(), 5, "laid mines stay");
    assert_eq!(fs(&w).accounts[0].works_done[f as usize], 1);
}

#[test]
fn ap_mine_goes_off_on_contact_only() {
    // Control: AP `trigger_cm` 30 -> 60: the 50 cm pass sets it off.
    for (off_cm, boom) in [(50, false), (20, true)] {
        let mut w = flat(|_| {});
        let m = rifleman(&mut w, 0, "ww2:ger_grenadiergruppe", 500, 500);
        let p = pos(&w, m);
        let at = p + FxVec2::new(Fx::from_int(10), Fx::from_milli(off_cm * 10));
        let f = fort(w.match_setup().unwrap(), "ww2:ap_minefield");
        w.lay_mine_now(f, at, PlayerId(1)).unwrap();
        walk_to(&mut w, m, p + FxVec2::from_int(20, 0), 40 * HZ);
        assert_eq!(fs(&w).mines.is_empty(), boom, "pass at {off_cm} cm");
        assert_eq!(set_off(&w, 1), u32::from(boom));
    }
}

#[test]
fn ap_mine_kills_the_man_who_steps_on_it() {
    // Control: AP `[he] damage` 150 -> 10: he lives in some seeds.
    for seed in 0..10u64 {
        let mut w = flat(|s| s.seed = seed);
        let m = rifleman(&mut w, 0, "ww2:ger_grenadiergruppe", 500, 500);
        let p = pos(&w, m);
        let f = fort(w.match_setup().unwrap(), "ww2:ap_minefield");
        w.lay_mine_now(f, p + FxVec2::from_int(6, 0), PlayerId(1))
            .unwrap();
        walk_to(&mut w, m, p + FxVec2::from_int(12, 0), 30 * HZ);
        w.step(&[]);
        assert_eq!(set_off(&w, 1), 1, "seed {seed}");
        let down = !w.state.entities.is_alive(m) || w.state.entities.combat[m.index()].hp == 0;
        assert!(down, "seed {seed}: he lives");
        assert_eq!(fs(&w).accounts[0].men_lost_to_mines, 1, "seed {seed}");
    }
}

/// Drives `p`'s tank `card` from (480, 500) to (530, 500) facing +x; the
/// tank's vehicle id.
fn drive_tank(w: &mut World, p: u8, card: &str) -> SimId {
    let tank = spawn(w, p, card, 480, 500);
    let v = super::s6_tests::members(w, tank)[0];
    super::s6_tests::run(
        w,
        p,
        3,
        CommandKind::Move {
            squad: tank,
            target: c(530, 500),
        },
    );
    for _ in 0..60 * HZ {
        w.step(&[]);
        if pos(w, v).x > Fx::from_int(522) {
            break;
        }
    }
    v
}

/// The left track's lateral offset (m, hull +y) of tank card `card`.
fn left_track_y(w: &World, card: &str) -> Fx {
    use frontline_content::vehicle::ComponentKind;
    let s = w.match_setup().unwrap();
    let ty = &s.squad_types[s.squad_type_index(card).unwrap() as usize];
    let v = &s.vehicles[ty.vehicle.unwrap() as usize];
    let t = v
        .components
        .iter()
        .find(|c| c.kind == ComponentKind::TrackLeft)
        .unwrap();
    crate::vehicle::v3(t.anchors[0].pos).y
}

#[test]
fn ap_mine_ignores_vehicles() {
    // Control: swap `trigger` in ap_mine/at_mine defs (foot <-> vehicle).
    let mut w = flat(|_| {});
    let y = left_track_y(&w, "ww2:ger_pz4_h_squad");
    let f = fort(w.match_setup().unwrap(), "ww2:ap_minefield");
    for x in [495, 500, 505, 510] {
        w.lay_mine_now(
            f,
            FxVec2::new(Fx::from_int(x), Fx::from_int(500) + y),
            PlayerId(1),
        );
        w.lay_mine_now(f, FxVec2::from_int(x, 500), PlayerId(1));
    }
    let v = drive_tank(&mut w, 0, "ww2:ger_pz4_h_squad");
    assert!(pos(&w, v).x > Fx::from_int(515), "the tank crossed");
    assert_eq!(fs(&w).mines.len(), 8);
    assert_eq!(set_off(&w, 1), 0);
}

/// S10b review R18: an AP mine that goes off right under a tank's track
/// (set off by a burst) leaves the track whole. Control: `vs_vehicle_pct`
/// 100 in ap_mine.toml -> 150 on a 100 hp track: broken.
#[test]
fn ap_mine_burst_does_not_break_a_track() {
    use frontline_content::vehicle::ComponentKind;
    let mut w = flat(|_| {});
    let y = left_track_y(&w, "ww2:ger_pz4_h_squad");
    let (_, v) = tank_at(&mut w, 0, "ww2:ger_pz4_h_squad", 500, 500);
    w.step(&[]);
    let f = fort(w.match_setup().unwrap(), "ww2:ap_minefield");
    let id = w
        .lay_mine_now(
            f,
            FxVec2::new(Fx::from_int(500), Fx::from_int(500) + y),
            PlayerId(1),
        )
        .unwrap();
    let t = w.tick();
    w.edit_fort(|s| s.mines.get_mut(&id).unwrap().boom_tick = Some(t));
    let before = w.game().unwrap().vehicles[&v].component(ComponentKind::TrackLeft);
    super::s6_tests::steps(&mut w, 3);
    assert!(!fs(&w).mines.contains_key(&id), "it went off");
    let vs = &w.game().unwrap().vehicles[&v];
    let after = vs.component(ComponentKind::TrackLeft);
    let setup = w.match_setup().unwrap();
    assert!(
        after > 0 && !vs.immobilised(&setup.vehicles[vs.type_idx as usize]),
        "{before} -> {after}"
    );
}

#[test]
fn at_mine_ignores_infantry() {
    // Control: as `ap_mine_ignores_vehicles`.
    let mut w = flat(|_| {});
    let m = rifleman(&mut w, 0, "ww2:ger_grenadiergruppe", 500, 500);
    let p = pos(&w, m);
    let f = fort(w.match_setup().unwrap(), "ww2:at_minefield");
    w.lay_mine_now(f, p + FxVec2::from_int(6, 0), PlayerId(1))
        .unwrap();
    walk_to(&mut w, m, p + FxVec2::from_int(12, 0), 30 * HZ);
    assert!((pos(&w, m) - (p + FxVec2::from_int(12, 0))).length() < Fx::ONE);
    assert_eq!(fs(&w).mines.len(), 1);
    assert_eq!(set_off(&w, 1), 0);
}

#[test]
fn at_mine_immobilises_a_tank_over_it() {
    // Control: AT `[he] vs_vehicle_pct` 100 -> 10: 40 hp, the track holds.
    use frontline_content::vehicle::ComponentKind;
    for card in ["ww2:ger_pz4_h_squad", "ww2:usa_m4a3_75_squad"] {
        let mut w = flat(|_| {});
        let y = left_track_y(&w, card);
        let f = fort(w.match_setup().unwrap(), "ww2:at_minefield");
        w.lay_mine_now(
            f,
            FxVec2::new(Fx::from_int(500), Fx::from_int(500) + y),
            PlayerId(1),
        )
        .unwrap();
        let v = drive_tank(&mut w, 0, card);
        let g = w.game().unwrap();
        let vs = g.vehicle(v).unwrap();
        let def = &w.match_setup().unwrap().vehicles[vs.type_idx as usize];
        assert!(fs(&w).mines.is_empty(), "{card}: not set off");
        assert_eq!(
            vs.components[ComponentKind::TrackLeft as usize],
            0,
            "{card}"
        );
        assert!(vs.immobile(def), "{card}");
        assert!(
            pos(&w, v).x < Fx::from_int(510),
            "{card} stopped near the mine"
        );
        assert_eq!(fs(&w).accounts[1].vehicles_mined, 1, "{card}");
        assert_eq!(set_off(&w, 1), 1);
    }
}

#[test]
fn own_side_never_sets_off_own_mines() {
    // Control: drop the `owner != m.owner` filters in `take_mines` /
    // `vehicle_over` -> own men and the own tank set them off.
    let mut w = flat(|_| {});
    let y = left_track_y(&w, "ww2:ger_pz4_h_squad");
    let ap = fort(w.match_setup().unwrap(), "ww2:ap_minefield");
    let at = fort(w.match_setup().unwrap(), "ww2:at_minefield");
    w.lay_mine_now(
        at,
        FxVec2::new(Fx::from_int(500), Fx::from_int(500) + y),
        PlayerId(0),
    )
    .unwrap();
    let v = drive_tank(&mut w, 0, "ww2:ger_pz4_h_squad");
    assert!(pos(&w, v).x > Fx::from_int(515));
    let m = rifleman(&mut w, 0, "ww2:ger_grenadiergruppe", 500, 600);
    let p = pos(&w, m);
    w.lay_mine_now(ap, p + FxVec2::from_int(6, 0), PlayerId(0))
        .unwrap();
    w.lay_mine_now(at, p + FxVec2::from_int(8, 0), PlayerId(0))
        .unwrap();
    walk_to(&mut w, m, p + FxVec2::from_int(12, 0), 30 * HZ);
    assert!((pos(&w, m) - (p + FxVec2::from_int(12, 0))).length() < Fx::ONE);
    assert_eq!(fs(&w).mines.len(), 3);
    assert_eq!(set_off(&w, 0), 0);
}

#[test]
fn mine_trigger_is_deterministic() {
    // Control: a mine burst's key from a per-run `RandomState` hash of its
    // id (not committed) -> hashes differ. (The spec's control, iterating
    // the mines through a `HashMap`, passes: this tick's bursts commute.)
    // Two runs of a 300-tick crossing with 40 mines, and the second through
    // save/load at tick 150.
    let run = |reload: bool| {
        let mut w = flat(|_| {});
        let ap = fort(w.match_setup().unwrap(), "ww2:ap_minefield");
        let at = fort(w.match_setup().unwrap(), "ww2:at_minefield");
        for k in 0..20 {
            w.lay_mine_now(ap, FxVec2::from_int(495 + k, 520), PlayerId(1))
                .unwrap();
            w.lay_mine_now(at, FxVec2::from_int(495 + k, 501), PlayerId(1))
                .unwrap();
        }
        let sq = spawn(&mut w, 0, "ww2:ger_grenadiergruppe", 505, 510);
        let tank = spawn(&mut w, 0, "ww2:ger_pz4_h_squad", 480, 500);
        let cmds = |seq| {
            vec![
                Command {
                    player: PlayerId(0),
                    seq,
                    kind: CommandKind::Move {
                        squad: sq,
                        target: c(505, 535),
                    },
                },
                Command {
                    player: PlayerId(0),
                    seq: seq + 1,
                    kind: CommandKind::Move {
                        squad: tank,
                        target: c(530, 500),
                    },
                },
            ]
        };
        w.step(&cmds(0));
        for t in 1..300 {
            if reload && t == 150 {
                w = World::load(&w.save(), 1).unwrap();
            }
            w.step(&[]);
        }
        (w.full_state_hash(), set_off(&w, 1), fs(&w).mines.len())
    };
    let a = run(false);
    assert!(a.1 > 1, "the crossing set mines off: {a:?}");
    assert_eq!(a, run(false));
    assert_eq!(a, run(true));
}

#[test]
fn site_refused_near_a_vehicle() {
    // Control: `vehicle_clear_m` 6 -> 0: the line 7 m from the hull centre
    // is accepted.
    let mut w = flat(|_| {});
    let f = fort(w.match_setup().unwrap(), "ww2:ap_minefield");
    spawn(&mut w, 1, "ww2:usa_m4a3_75_squad", 500, 500);
    let eng = spawn(&mut w, 0, "ww2:ger_engineer_squad", 520, 520);
    w.step(&[]);
    fortify(&mut w, 0, eng, f, vec![c(490, 507), c(510, 507)]);
    assert_eq!(refused(&w), Some(crate::fort::FortRefusal::BadSpot));
    assert!(fs(&w).works.is_empty());
    fortify(&mut w, 0, eng, f, vec![c(490, 515), c(510, 515)]);
    assert_eq!(fs(&w).works.len(), 1, "{:?}", refused(&w));
}

// ---- Build C: sweeping, lifting, mines set off by HE (S10b-build-C.md 3).

fn clear_mines(w: &mut World, p: u8, squad: SquadId, a: FxVec2, b: FxVec2) {
    super::s6_tests::run(w, p, 0, CommandKind::ClearMines { squad, a, b });
}

fn order_of(w: &World, s: SquadId) -> Order {
    w.game().unwrap().squads[&s].order.clone()
}

fn dm(x_dm: i32, y_dm: i32) -> FxVec2 {
    FxVec2::new(Fx::from_milli(x_dm * 100), Fx::from_milli(y_dm * 100))
}

/// One mine's sweep record: found (its finder's bit set while it stood),
/// `n` the (man, tick) pairs a living member of a sweeping squad stood
/// within `detect_cm` of it while it was unknown, and whether it went off.
#[derive(Clone, Copy, Debug, Default)]
struct Pass {
    found: bool,
    n: u32,
    off: bool,
    /// The closest a member came after it was known and before it went
    /// (m, f64 for the report only).
    closest_known: f64,
}

/// Steps `w` up to `max` ticks while `squads` sweep, recording `mines`
/// (owned by player 1; player 0 sweeps). Stops when every squad is idle.
fn sweep_run(w: &mut World, squads: &[SquadId], mines: &[u32], max: u64) -> Vec<Pass> {
    let detect = Fx::from_milli(
        w.match_setup()
            .unwrap()
            .rules
            .fortify
            .as_ref()
            .unwrap()
            .detect_cm as i32
            * 10,
    );
    let mut out = vec![
        Pass {
            closest_known: f64::MAX,
            ..Pass::default()
        };
        mines.len()
    ];
    for _ in 0..max {
        let before: Vec<Option<u8>> = mines
            .iter()
            .map(|id| fs(w).mines.get(id).map(|m| m.known))
            .collect();
        w.step(&[]);
        let men: Vec<FxVec2> = squads
            .iter()
            .flat_map(|&s| super::s6_tests::members(w, s))
            .filter(|&m| w.state.entities.is_alive(m))
            .map(|m| pos(w, m))
            .collect();
        for (k, id) in mines.iter().enumerate() {
            let Some(known) = before[k] else { continue };
            let now = fs(w).mines.get(id).map(|m| (m.pos, m.known));
            if known & 1 == 0 {
                let p = now.map(|x| x.0).unwrap_or_default();
                if now.is_some() {
                    out[k].n += men.iter().filter(|&&q| (q - p).length() <= detect).count() as u32;
                }
                match now {
                    Some((_, kn)) if kn & 1 != 0 => out[k].found = true,
                    None => out[k].off = true,
                    _ => {}
                }
            } else if let Some((p, _)) = now {
                for q in &men {
                    let d = (*q - p).length().to_f32() as f64;
                    out[k].closest_known = out[k].closest_known.min(d);
                }
            }
        }
        if squads
            .iter()
            .all(|&s| !matches!(order_of(w, s), Order::ClearMines { .. }))
        {
            break;
        }
    }
    out
}

#[test]
fn sweep_finds_at_expected_rate() {
    // Control: `detect_pct_per_s` 50 -> 25 in the sim only (the expected
    // share still uses 50) -> found falls far under expected.
    // 1600 single-mine passes: 80 seeds x 20 lanes, offsets 0..140 cm (the
    // spec's 400 leave a 1.9-point sd, so 3 points failed on a fair sim
    // in about one seed set in ten; 1600 make it 0.95).
    // Expected per pass `1 - (1 - p)^n`, n the sweeper-ticks the mine would
    // be in range on a full pass: each sweeper walks his +-1 m line at the
    // pace measured in the run (`along` per tick), so a sweeper at lateral
    // distance l is in range over the chord `2 sqrt(r^2 - l^2)`, or up to
    // the trigger circle when l < `trigger_cm` (he steps on it). The n
    // counted in the run itself stops at the find, which would bias the
    // expectation low; it is used only to check the geometry on misses.
    let (mut found, mut expected, mut passes) = (0u32, 0f64, 0u32);
    for seed in 0..80u64 {
        let mut w = flat(|s| s.seed = 100 + seed);
        let t = w.match_setup().unwrap().rules.fortify.clone().unwrap();
        let p = t.detect_pct_per_s as f64 / (100.0 * HZ as f64);
        let mut squads = Vec::new();
        let mut mines = Vec::new();
        for k in 0..20 {
            let y = 200 + 40 * k;
            let s = spawn(&mut w, 0, "ww2:ger_engineer_squad", 300, y);
            let off = ((k as u64 + seed) % 8) as i32 * 2; // dm: 0..140 cm
            mines.push(mine_at(&mut w, "ww2:ap_minefield", 3060, y * 10 + off, 1));
            squads.push(s);
        }
        w.step(&[]);
        for (k, &s) in squads.iter().enumerate() {
            let y = 200 + 40 * k as i32;
            clear_mines(&mut w, 0, s, c(300, y), c(312, y));
        }
        let step = Fx::from(
            w.match_setup().unwrap().squad_types
                [w.game().unwrap().squads[&squads[0]].type_idx as usize]
                .speed,
        )
        .mul_int(t.sweep_speed_pct as i32)
        .div_int(100)
        .div_int(HZ as i32)
        .to_f32() as f64;
        let r = t.detect_cm as f64 / 100.0;
        for (k, run) in sweep_run(&mut w, &squads, &mines, 120 * HZ)
            .into_iter()
            .enumerate()
        {
            let o = (((k as u64 + seed) % 8) * 2) as f64 / 10.0;
            let n: f64 = [o - 1.0, o + 1.0]
                .iter()
                .map(|l: &f64| {
                    let l = l.abs();
                    if l >= r {
                        0.0
                    } else if l < 0.3 {
                        ((r * r - l * l).sqrt() - (0.09 - l * l).sqrt()) / step
                    } else {
                        2.0 * (r * r - l * l).sqrt() / step
                    }
                })
                .sum();
            if !run.found && !run.off {
                assert!(
                    (run.n as f64 - n).abs() <= 3.0,
                    "lane {k}: {} vs {n:.1}",
                    run.n
                );
            }
            passes += 1;
            found += u32::from(run.found);
            expected += 1.0 - (1.0 - p).powf(n.floor());
        }
    }
    let found_pct = 100.0 * found as f64 / passes as f64;
    let exp_pct = 100.0 * expected / passes as f64;
    assert_eq!(passes, 1600);
    assert!(
        (found_pct - exp_pct).abs() <= 3.0,
        "found {found_pct:.1} % vs expected {exp_pct:.1} %"
    );
}

/// `n` lanes 40 m apart, each with a `player` 0 `card` squad at its start
/// (300, y) and a player-1 AP mine at `mine_dm` (x, lateral) from (300, y),
/// known to player 0 when `known`; the sweep ordered. (World, squads,
/// mines.)
fn lanes(
    seed: u64,
    card: &str,
    n: i32,
    mine_dm: (i32, i32),
    known: bool,
    edit: impl FnOnce(&mut frontline_content::MatchSetup),
) -> (World, Vec<SquadId>, Vec<u32>) {
    let mut w = flat(|s| {
        s.seed = seed;
        edit(s);
    });
    let mut squads = Vec::new();
    let mut mines = Vec::new();
    for k in 0..n {
        let y = 200 + 40 * k;
        squads.push(spawn(&mut w, 0, card, 300, y));
        mines.push(mine_at(
            &mut w,
            "ww2:ap_minefield",
            3000 + mine_dm.0,
            y * 10 + mine_dm.1,
            1,
        ));
    }
    if known {
        w.edit_fort(|f| f.mines.values_mut().for_each(|m| m.known |= 1));
    }
    w.step(&[]);
    for (k, &s) in squads.iter().enumerate() {
        let y = 200 + 40 * k as i32;
        clear_mines(&mut w, 0, s, c(300, y), c(312, y));
    }
    (w, squads, mines)
}

#[test]
fn sweeper_stops_short_of_a_found_mine() {
    // Control: the lifter's spot on the mine itself (`LIFT_STAND` 0, no
    // clamp) -> he steps on it: it goes off.
    for seed in 0..5u64 {
        let (mut w, squads, mines) =
            lanes(seed, "ww2:ger_engineer_squad", 10, (60, 0), true, |_| {});
        let runs = sweep_run(&mut w, &squads, &mines, 90 * HZ);
        let a = &fs(&w).accounts;
        assert_eq!(a[1].mines_set_off, 0, "seed {seed}");
        assert_eq!(a[0].mines_lifted, 10, "seed {seed}");
        for (k, r) in runs.iter().enumerate() {
            assert!(!r.off, "seed {seed} lane {k}");
            // trigger_cm 30 + 50 cm.
            assert!(r.closest_known >= 0.8, "seed {seed} lane {k}: {r:?}");
        }
    }
}

#[test]
fn lift_time_follows_tier() {
    // Control: `tier_of` always field (100 %) -> 20 s for all three.
    for (card, secs) in [
        ("ww2:ger_engineer_squad", 20),
        ("ww2:ger_sturmpioniergruppe", 10),
        ("ww2:ger_baupioniergruppe", 40),
    ] {
        let (mut w, squads, mines) = lanes(1, card, 1, (60, 0), true, |_| {});
        let s = squads[0];
        let (mut prev, mut ticks) = (0u64, 0u64);
        for _ in 0..90 * HZ {
            w.step(&[]);
            let work = match order_of(&w, s) {
                Order::ClearMines { work, .. } => work,
                _ => 0,
            };
            let gone = !fs(&w).mines.contains_key(&mines[0]);
            if work > prev || (gone && prev > 0) {
                ticks += 1;
            }
            prev = work;
            if gone {
                break;
            }
        }
        assert_eq!(fs(&w).accounts[0].mines_lifted, 1, "{card}");
        let want = secs * HZ;
        assert!(
            ticks.abs_diff(want) <= 1,
            "{card}: {ticks} ticks, want {want}"
        );
    }
}

/// S10b review R16: squad B (the higher id) lifts a mine that squad A (the
/// lower id, handled first that tick) also has as its lift; the save made
/// on that tick must load. Control: drop the stale-lift clean-up after
/// `mines.remove` in `sweep_tick` -> "ClearMines lifts a mine its player
/// does not know".
#[test]
fn second_sweeper_lift_leaves_no_stale_lift() {
    let mut w = flat(|_| {});
    let a = spawn(&mut w, 0, "ww2:ger_engineer_squad", 260, 200);
    let mine = mine_at(&mut w, "ww2:ap_minefield", 3060, 2000, 1);
    w.edit_fort(|f| f.mines.values_mut().for_each(|m| m.known |= 1));
    let b = spawn(&mut w, 0, "ww2:ger_engineer_squad", 300, 200);
    assert!(a < b);
    w.step(&[]);
    clear_mines(&mut w, 0, b, c(300, 200), c(312, 200));
    clear_mines(&mut w, 0, a, c(280, 200), c(312, 200));
    w.edit_squad(a, |q| {
        q.order = Order::ClearMines {
            a: c(280, 200),
            b: c(312, 200),
            along: Fx::from_int(24),
            lift: Some(mine),
            work: 0,
        }
    });
    let mut lifted = false;
    for _ in 0..90 * HZ {
        w.step(&[]);
        if !fs(&w).mines.contains_key(&mine) {
            lifted = true;
            break;
        }
    }
    assert!(lifted, "B lifted the mine");
    assert_eq!(fs(&w).accounts[0].mines_lifted, 1);
    if let Err(e) = World::load(&w.save(), 1) {
        panic!("the save on the lift tick: {e}");
    }
}

/// S10b review R2: a loaded `ClearMines` order runs even when every
/// player's `sweeps` report counter is 0. Control: gate `sweep_tick` on
/// `sweeps > 0` again -> the order never advances.
#[test]
fn sweep_runs_with_zero_sweeps_counter() {
    let (mut w, squads, _) = lanes(1, "ww2:ger_engineer_squad", 1, (60, 0), true, |_| {});
    w.edit_fort(|f| f.accounts.iter_mut().for_each(|a| a.sweeps = 0));
    let w = World::load(&w.save(), 1).unwrap();
    let mut w = w;
    for _ in 0..90 * HZ {
        w.step(&[]);
        if !matches!(order_of(&w, squads[0]), Order::ClearMines { .. }) {
            break;
        }
    }
    assert_eq!(fs(&w).accounts[0].mines_lifted, 1);
    assert_eq!(order_of(&w, squads[0]), Order::Idle);
}

#[test]
fn clear_mines_needs_capability() {
    // Control: give the grenadiers `clear_mines` -> accepted.
    let mut w = flat(|_| {});
    let s = spawn(&mut w, 0, "ww2:ger_grenadiergruppe", 300, 300);
    w.step(&[]);
    clear_mines(&mut w, 0, s, c(300, 300), c(312, 300));
    assert_eq!(refused(&w), Some(crate::fort::FortRefusal::CannotBuild));
    assert!(!matches!(order_of(&w, s), Order::ClearMines { .. }));
    assert_eq!(fs(&w).accounts[0].sweeps, 0);
    // An engineer: a lane longer than `lane_max_m` (50 m) is `BadSpot`.
    let e = spawn(&mut w, 0, "ww2:ger_engineer_squad", 300, 340);
    w.step(&[]);
    clear_mines(&mut w, 0, e, c(300, 340), c(351, 340));
    assert_eq!(refused(&w), Some(crate::fort::FortRefusal::BadSpot));
    clear_mines(&mut w, 0, e, c(300, 340), c(350, 340));
    assert!(matches!(order_of(&w, e), Order::ClearMines { .. }));
    assert_eq!(fs(&w).accounts[0].sweeps, 1);
}

#[test]
fn clear_ends_at_lane_end_with_no_known_mine_left() {
    // Control: end at the lane end ignoring known mines (`done` without
    // the known-mine check) -> a found mine is left standing.
    for seed in 0..6u64 {
        let mut w = flat(|s| s.seed = 200 + seed);
        let s = spawn(&mut w, 0, "ww2:usa_engineer_squad", 300, 300);
        let mut mines = Vec::new();
        for (x, y) in [
            (40, 0),
            (70, 12),
            (100, -15),
            (130, 5),
            (160, -8),
            (190, 18),
        ] {
            mines.push(mine_at(&mut w, "ww2:ap_minefield", 3000 + x, 3000 + y, 1));
        }
        w.step(&[]);
        clear_mines(&mut w, 0, s, c(300, 300), c(320, 300));
        let mut last = Fx::ZERO;
        for _ in 0..400 * HZ {
            w.step(&[]);
            match order_of(&w, s) {
                Order::ClearMines { along, .. } => last = along,
                _ => break,
            }
        }
        assert_eq!(order_of(&w, s), Order::Idle, "seed {seed}");
        // Idle in the tick `along` reached 20 m: the last value seen is
        // within one step (2.9 m/s x 25 % / 30 Hz = 2.4 cm) of it.
        assert!(
            last < Fx::from_int(20) && last + Fx::from_milli(25) >= Fx::from_int(20),
            "seed {seed}: {last}"
        );
        let f = fs(&w);
        for id in &mines {
            assert!(
                f.mines.get(id).is_none_or(|m| m.known & 1 == 0),
                "seed {seed}: mine {id} known and left"
            );
        }
        assert!(f.accounts[0].mines_found > 0, "seed {seed}");
        // Every found mine was lifted, unless it went off first.
        let (a0, a1) = (&f.accounts[0], &f.accounts[1]);
        assert!(
            a0.mines_lifted >= 1 && a0.mines_lifted <= a0.mines_found,
            "seed {seed}"
        );
        assert!(
            a0.mines_lifted + a1.mines_set_off + a1.mines_blasted >= a0.mines_found,
            "seed {seed}"
        );
    }
}

fn weapon_id(w: &World, id: &str) -> u16 {
    w.match_setup()
        .unwrap()
        .weapons
        .iter()
        .position(|x| x.id == id)
        .unwrap() as u16
}

#[test]
fn found_mine_known_to_finder_only() {
    // Control: set every player's bit on a find -> `known` 0b11 after it.
    let (mut w, squads, mines) = lanes(3, "ww2:ger_engineer_squad", 1, (60, 0), false, |_| {});
    let m = mines[0];
    w.edit_fort(|f| f.mines.get_mut(&m).unwrap().known = 0);
    let mut seen = None;
    for _ in 0..60 * HZ {
        w.step(&[]);
        let k = fs(&w).mines[&m].known;
        if k != 0 {
            seen = Some(k);
            break;
        }
    }
    assert_eq!(seen, Some(0b01), "the finder's bit only");
    assert_eq!(fs(&w).accounts[0].mines_found, 1);
    // A mortar bomb beside it: known to all when set off, gone a tick on.
    let _ = squads;
    let mortar = weapon_id(&w, "ww2:m1_mortar");
    let at = fs(&w).mines[&m].pos;
    assert!(w.drop_shell_now(1, mortar, at + FxVec2::from_int(0, 1)));
    let mut all = false;
    for _ in 0..2 * HZ {
        w.step(&[]);
        match fs(&w).mines.get(&m) {
            Some(x) if x.boom_tick.is_some() => {
                all = x.known == 0b11;
            }
            Some(_) => {}
            None => break,
        }
    }
    assert!(all, "known to all once set off");
    assert!(!fs(&w).mines.contains_key(&m), "gone");
}

#[test]
fn burst_sets_off_mines_next_tick() {
    // Control: `blast_mines` radius x 0.5 -> the mine at 1.2 m stays.
    let mut w = flat(|_| {});
    let row: Vec<u32> = [5, 12, 25]
        .iter()
        .map(|&x| mine_at(&mut w, "ww2:ap_minefield", 5000 + x, 5000, 0))
        .collect();
    let mortar = weapon_id(&w, "ww2:m1_mortar");
    assert!(w.drop_shell_now(1, mortar, c(500, 500)));
    let mut set = None;
    for _ in 0..2 * HZ {
        w.step(&[]);
        if fs(&w).mines.values().any(|m| m.boom_tick.is_some()) {
            set = Some(w.tick());
            break;
        }
    }
    let t = set.expect("the burst set mines off");
    let f = fs(&w);
    assert_eq!(f.mines[&row[0]].boom_tick, Some(t));
    assert_eq!(f.mines[&row[1]].boom_tick, Some(t));
    assert_eq!(f.mines[&row[2]].boom_tick, None);
    assert_eq!(f.accounts[0].mines_blasted, 2);
    // A save between the burst and the mines' tick loads (boom = tick).
    let back = World::load(&w.save(), 1).unwrap_or_else(|e| panic!("load: {e}"));
    assert_eq!(back.full_state_hash(), w.full_state_hash());
    w.step(&[]);
    let f = fs(&w);
    assert!(!f.mines.contains_key(&row[0]) && !f.mines.contains_key(&row[1]));
    assert_eq!(f.accounts[0].mines_set_off, 0, "a blast is not a contact");
    super::s6_tests::steps(&mut w, 3 * HZ);
    // The two mine bursts (100 cm) do not reach the one at 2.5 m.
    assert!(fs(&w).mines.contains_key(&row[2]));
    assert_eq!(fs(&w).accounts[0].mines_blasted, 2);
}

/// A 2-row AP field (rows 1 m apart, mines every 2 m, the second row
/// staggered by 1 m) and a German Bangalore laid from `from` to `to` (m,
/// x 10), bursting next tick. Returns the mines standing after 3 s and the
/// ones within 150 cm of a burst.
fn bangalore_case(from: FxVec2, to: FxVec2) -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let mut w = flat(|_| {});
    let mut all = Vec::new();
    for k in 0..10 {
        all.push(mine_at(&mut w, "ww2:ap_minefield", 4900 + 20 * k, 5000, 1));
        all.push(mine_at(&mut w, "ww2:ap_minefield", 4910 + 20 * k, 5010, 1));
    }
    let item = w
        .match_setup()
        .unwrap()
        .items
        .iter()
        .position(|i| i.id == "ww2:gestreckte_ladung")
        .unwrap() as u16;
    let tick = w.tick();
    w.edit_fort(|f| {
        f.charges.push(Charge {
            id: f.next_charge,
            owner: PlayerId(0),
            item,
            from,
            to,
            fuse_tick: tick + 1,
            piece: None,
        });
        f.next_charge += 1;
    });
    let near: Vec<u32> = all
        .iter()
        .copied()
        .filter(|id| {
            let p = fs(&w).mines[id].pos;
            (0..3).any(|k| {
                let b = from + (to - from).mul_int(2 * k + 1).div_int(6);
                (p - b).length() <= Fx::from_milli(1500)
            })
        })
        .collect();
    super::s6_tests::steps(&mut w, 3 * HZ);
    let left: Vec<u32> = all
        .iter()
        .copied()
        .filter(|id| fs(&w).mines.contains_key(id))
        .collect();
    (all, left, near)
}

#[test]
fn bangalore_clears_mines_along_its_length() {
    // Control: the Bangalore laid 3 m short (y 495.5..498.5) -> no mine
    // within 150 cm of its bursts: none removed.
    let (all, left, near) = bangalore_case(dm(5000, 4985), dm(5000, 5015));
    assert!(near.len() >= 3, "{near:?}");
    for id in &all {
        assert_eq!(left.contains(id), !near.contains(id), "mine {id}");
    }
}

#[test]
fn sweep_is_deterministic() {
    // Control: the detection roll keyed by a process-wide counter instead
    // of the tick -> the second run finds other mines: hashes differ.
    // (The spec's control, the roll keyed by tick only, stays
    // deterministic and passes.)
    let run = |reload: bool| {
        let (mut w, squads, _) = lanes(9, "ww2:ger_engineer_squad", 4, (60, 4), false, |_| {});
        for k in 0..4 {
            let y = 200 + 40 * k;
            for x in [90, 120, 150] {
                mine_at(&mut w, "ww2:ap_minefield", 3000 + x, y * 10 - 6, 1);
            }
        }
        for t in 0..900 {
            if reload && t == 450 {
                w = World::load(&w.save(), 1).unwrap();
            }
            w.step(&[]);
        }
        let a = &fs(&w).accounts[0];
        let _ = &squads;
        (
            w.state_hash(),
            w.full_state_hash(),
            a.mines_found,
            a.mines_lifted,
        )
    };
    let a = run(false);
    assert!(a.2 > 0, "{a:?}");
    assert_eq!(a, run(false));
    assert_eq!(a, run(true));
}

// ---- Build D: AT obstacles and demolition (S10b-build-D.md 3).

/// `n` hedgehogs of `owner` every 3 m up from (x, y0), facing +x; their
/// piece ids.
fn hedgehogs(w: &mut World, x: i32, y0: i32, n: i32, owner: u8) -> Vec<u32> {
    let f = fort(w.match_setup().unwrap(), "ww2:hedgehog");
    let before: Vec<u32> = fs(w).pieces.keys().copied().collect();
    for k in 0..n {
        let y = y0 + 3 * k;
        assert!(w.build_now(f, &[c(x, y), c(x + 1, y)], PlayerId(owner)));
    }
    fs(w)
        .pieces
        .keys()
        .copied()
        .filter(|id| !before.contains(id))
        .collect()
}

/// Every 0.5 m point of `a..b` is on a blocked node of the tracked layer
/// (and of the wheeled one, if any).
fn all_blocked(w: &World, a: FxVec2, b: FxVec2) -> bool {
    let d = w.game().unwrap().vehicle_derived();
    let n = ((b - a).length().mul_int(2)).floor_int().max(1);
    (0..=n).all(|k| {
        let p = a + (b - a).mul_int(k).div_int(n);
        d.block.blocked_at(w.terrain(), p)
            && d.wheel_block
                .as_ref()
                .is_none_or(|l| l.blocked_at(w.terrain(), p))
    })
}

/// Drives a fresh `p` M4 from `from` to `to` (60 s at most); its hull
/// positions every tick.
fn tank_track(w: &mut World, p: u8, from: FxVec2, to: FxVec2, secs: u64) -> Vec<FxVec2> {
    let x = from.x.floor_int();
    let y = from.y.floor_int();
    let tank = spawn(w, p, "ww2:usa_m4a3_75_squad", x, y);
    let v = super::s6_tests::members(w, tank)[0];
    super::s6_tests::run(
        w,
        p,
        3,
        CommandKind::Move {
            squad: tank,
            target: to,
        },
    );
    let mut out = vec![pos(w, v)];
    for _ in 0..secs * HZ {
        w.step(&[]);
        out.push(pos(w, v));
        if (pos(w, v) - to).length() < Fx::from_int(3) {
            break;
        }
    }
    out
}

/// Whether the track crosses x = `x` between y0 and y1.
fn crosses_x(track: &[FxVec2], x: i32, y0: i32, y1: i32) -> bool {
    let x = Fx::from_int(x);
    track.windows(2).any(|s| {
        let (a, b) = (s[0], s[1]);
        if (a.x < x) == (b.x < x) {
            return false;
        }
        let t = (x - a.x) / (b.x - a.x);
        let y = a.y + (b.y - a.y) * t;
        y >= Fx::from_int(y0) && y <= Fx::from_int(y1)
    })
}

#[test]
fn obstacle_blocks_vehicle_nodes() {
    // Control: `blocks_vehicles = false` on both blueprints -> the nodes
    // under the lines are free and the tank drives straight through.
    for log in [false, true] {
        let mut w = flat(|_| {});
        if log {
            let f = fort(w.match_setup().unwrap(), "ww2:log_barrier");
            assert!(w.build_now(f, &[c(500, 490), c(500, 510)], PlayerId(0)));
            assert_eq!(fs(&w).pieces.len(), 5);
            // The line's ends sit half way between two nodes: 1 m in.
            assert!(all_blocked(&w, c(500, 491), c(500, 509)));
        } else {
            assert_eq!(hedgehogs(&mut w, 500, 485, 11, 0).len(), 11);
            assert!(all_blocked(&w, c(500, 485), c(500, 515)));
        }
        let (y0, y1) = if log { (490, 510) } else { (485, 515) };
        let track = tank_track(&mut w, 1, c(470, 500), c(530, 500), 90);
        assert!(
            !crosses_x(&track, 500, y0, y1),
            "log {log}: crossed the line"
        );
        let end = *track.last().unwrap();
        assert!(end.x > Fx::from_int(470), "log {log}: never moved");
    }
}

#[test]
fn removed_obstacle_reopens_the_path() {
    // Control: no `mark_piece_block` for removed pieces -> the gap's nodes
    // stay blocked and the tank goes round (or stops).
    let mut w = flat(|_| {});
    let ids = hedgehogs(&mut w, 500, 455, 31, 0);
    assert!(all_blocked(&w, c(500, 455), c(500, 545)));
    // The middle nine (476 .. 524 m apart once gone: a 28 m gap between
    // standing centres) are destroyed.
    let gone: Vec<u32> = ids[11..20].to_vec();
    w.edit_fort(|s| {
        for id in &gone {
            s.pieces.remove(id);
        }
    });
    let d = w.game().unwrap().vehicle_derived();
    assert!(!d.block.blocked_at(w.terrain(), c(500, 500)), "same tick");
    let track = tank_track(&mut w, 1, c(470, 500), c(530, 500), 60);
    assert!(
        crosses_x(&track, 500, 491, 509),
        "the tank goes through the gap"
    );
}

#[test]
fn infantry_walks_through_obstacles() {
    // Control: pieces slowing men like wire (`wire_pace` with every
    // hedgehog a band) -> the crossing is slower.
    let mut w = flat(|_| {});
    hedgehogs(&mut w, 500, 485, 11, 0);
    // Ticks from x = 494 to x = 506 (the band 6 m either side of the
    // line), walking from 485 toward 520.
    let time = |w: &mut World, y: i32| {
        let m = rifleman(w, 1, "ww2:usa_rifle_squad", 485, y);
        super::s6_tests::run(
            w,
            1,
            7,
            CommandKind::MoveUnits {
                units: vec![m],
                target: c(520, y),
            },
        );
        let (mut t0, mut t1) = (None, None);
        for _ in 0..60 * HZ {
            w.step(&[]);
            let x = pos(w, m).x;
            if t0.is_none() && x >= Fx::from_int(494) {
                t0 = Some(w.tick());
            }
            if x >= Fx::from_int(506) {
                t1 = Some(w.tick());
                break;
            }
        }
        t1.expect("crossed") - t0.expect("started")
    };
    let on = time(&mut w, 500);
    let off = time(&mut w, 560);
    assert!(
        on.abs_diff(off) * 100 <= off,
        "across {on} ticks, off {off}"
    );
}

/// The vehicle id of `p`'s squad `card` at (x, y) facing +x.
fn tank_at(w: &mut World, p: u8, card: &str, x: i32, y: i32) -> (SquadId, SimId) {
    let sq = spawn(w, p, card, x, y);
    (sq, super::s6_tests::members(w, sq)[0])
}

fn move_squad(w: &mut World, p: u8, squad: SquadId, to: FxVec2) {
    super::s6_tests::run(w, p, 3, CommandKind::Move { squad, target: to });
}

/// S10b review R15: a hedgehog ordered 5.3 m from a tank's hull centre (R18
/// passes with `vehicle_clear_m` 0; S10b-ai E3: a piece's nodes reach only
/// half a grid step past it) does not stand while its nodes cover a track
/// sample; once the tank has driven off it stands. Control: drop the
/// `obstacle_on_vehicle` wait in `fort_tick` -> the hedgehog stands at
/// once and the tank is `on_blocked`.
#[test]
fn obstacle_waits_until_no_vehicle_is_on_its_nodes() {
    let mut w = flat(|s| {
        if let Some(f) = s.rules.fortify.as_mut() {
            f.vehicle_clear_m = 0;
        }
    });
    let f = fort(w.match_setup().unwrap(), "ww2:hedgehog");
    let (tank, v) = tank_at(&mut w, 0, "ww2:ger_pz4_h_squad", 500, 500);
    let eng = spawn(&mut w, 0, "ww2:ger_baupioniergruppe", 520, 520);
    w.edit_fort(|s| {
        s.carried_g.insert(eng, 100_000);
    });
    w.step(&[]);
    fortify(&mut w, 0, eng, f, vec![dm(5050, 5016), dm(5060, 5016)]);
    assert_eq!(fs(&w).works.len(), 1, "{:?}", refused(&w));
    for _ in 0..400 * HZ {
        w.step(&[]);
        assert!(
            fs(&w).pieces.is_empty(),
            "the hedgehog stood on the tank's nodes"
        );
        assert!(!w.game().unwrap().vehicles[&v].on_blocked);
        let full = fs(&w).works.values().any(|x| {
            let d = &w.match_setup().unwrap().forts[x.fort as usize];
            x.step == crate::fort::WorkStep::Build
                && x.work >= d.work_ticks as u64 * d.work_men as u64 * 100
        });
        if full {
            break;
        }
    }
    super::s6_tests::steps(&mut w, 2 * HZ);
    assert!(fs(&w).pieces.is_empty());
    assert_eq!(fs(&w).works.len(), 1, "the work waits");
    move_squad(&mut w, 0, tank, c(440, 500));
    for _ in 0..60 * HZ {
        w.step(&[]);
        if !fs(&w).pieces.is_empty() {
            break;
        }
    }
    assert!(!fs(&w).pieces.is_empty(), "stands once the tank is gone");
    assert!(!w.game().unwrap().vehicles[&v].on_blocked);
}

/// S10b review R15: a tank whose track samples lie on nodes a hedgehog
/// blocks (stood beside it by the test tool) can drive out, but not on
/// through the obstacle. Control: `escape` always false -> it never moves.
/// S10b-ai E3: off the piece's node it may roll straight on away from the
/// piece over the node's edge. Control: `piece_near` None -> wedged at yaw
/// -2.48, 1.2 m in 60 s (seen).
#[test]
fn vehicle_on_obstacle_nodes_drives_out() {
    let mut w = flat(|_| {});
    let f = fort(w.match_setup().unwrap(), "ww2:hedgehog");
    let (tank, v) = tank_at(&mut w, 0, "ww2:ger_pz4_h_squad", 500, 500);
    w.step(&[]);
    assert!(w.build_now(f, &[dm(5050, 5016), dm(5060, 5016)], PlayerId(0)));
    super::s6_tests::steps(&mut w, 2);
    assert!(w.game().unwrap().vehicles[&v].on_blocked, "the stand-in");
    let start = pos(&w, v);
    move_squad(&mut w, 0, tank, c(440, 500));
    for _ in 0..60 * HZ {
        w.step(&[]);
    }
    let end = pos(&w, v);
    assert!(
        (start - end).length() > Fx::from_int(20),
        "drove out: {start:?} -> {end:?}"
    );
    assert!(!w.game().unwrap().vehicles[&v].on_blocked);
    // Not on through: ordered past the hedgehog, it never gets nearer it.
    let mut w = flat(|_| {});
    let (tank, v) = tank_at(&mut w, 0, "ww2:ger_pz4_h_squad", 500, 500);
    w.step(&[]);
    assert!(w.build_now(f, &[dm(5050, 5016), dm(5060, 5016)], PlayerId(0)));
    super::s6_tests::steps(&mut w, 2);
    let hh = dm(5050, 5016);
    let d0 = (pos(&w, v) - hh).length();
    move_squad(&mut w, 0, tank, c(530, 530));
    for _ in 0..30 * HZ {
        w.step(&[]);
        let d = (pos(&w, v) - hh).length();
        assert!(
            d >= d0 - Fx::from_milli(100),
            "nearer the hedgehog: {d:?} < {d0:?}"
        );
    }
}

/// S10b review R19: a point site is checked at the site only, not along
/// the line to its facing point. Control: `near_a_vehicle(line)` again ->
/// BadSpot for the tank 70 m from the site.
#[test]
fn point_site_ignores_a_vehicle_near_its_facing_point() {
    let mut w = flat(|_| {});
    let f = fort(w.match_setup().unwrap(), "ww2:hedgehog");
    tank_at(&mut w, 0, "ww2:ger_pz4_h_squad", 500, 570);
    let eng = spawn(&mut w, 0, "ww2:ger_baupioniergruppe", 520, 500);
    w.step(&[]);
    fortify(&mut w, 0, eng, f, vec![c(500, 500), c(500, 580)]);
    assert_eq!(fs(&w).works.len(), 1, "{:?}", refused(&w));
}

#[test]
fn obstacle_refused_near_a_vehicle() {
    // Control: `vehicle_clear_m` 0 -> the hedgehog 4 m from the hull is
    // accepted.
    let mut w = flat(|_| {});
    let f = fort(w.match_setup().unwrap(), "ww2:hedgehog");
    spawn(&mut w, 1, "ww2:usa_m4a3_75_squad", 500, 500);
    let eng = spawn(&mut w, 0, "ww2:ger_baupioniergruppe", 520, 520);
    w.step(&[]);
    fortify(&mut w, 0, eng, f, vec![c(504, 500), c(505, 500)]);
    assert_eq!(refused(&w), Some(crate::fort::FortRefusal::BadSpot));
    assert!(fs(&w).works.is_empty());
    fortify(&mut w, 0, eng, f, vec![c(504, 530), c(505, 530)]);
    assert_eq!(fs(&w).works.len(), 1, "{:?}", refused(&w));
}

/// Ticks `card` (of player 0) spends in a hedgehog's build step (kg
/// carried, so the wait is one tick), with `heavy` as `heavy_by_field_pct`.
fn hedgehog_build_ticks(card: &str, heavy: u32) -> u64 {
    let mut w = flat(|s| s.rules.fortify.as_mut().unwrap().heavy_by_field_pct = heavy);
    let f = fort(w.match_setup().unwrap(), "ww2:hedgehog");
    let eng = spawn(&mut w, 0, card, 500, 500);
    w.edit_fort(|s| {
        s.carried_g.insert(eng, 100_000);
    });
    fortify(&mut w, 0, eng, f, vec![c(500, 504), c(501, 504)]);
    assert_eq!(fs(&w).works.len(), 1, "{card}: {:?}", refused(&w));
    let mut ticks = 0;
    for _ in 0..1200 * HZ {
        w.step(&[]);
        let building = fs(&w)
            .works
            .values()
            .any(|x| x.step == crate::fort::WorkStep::Build && x.work > 0);
        if building {
            ticks += 1;
        }
        if fs(&w).works.is_empty() {
            break;
        }
    }
    assert_eq!(fs(&w).accounts[0].obstacles_built, 1, "{card}");
    ticks
}

#[test]
fn heavy_blueprint_at_half_rate_for_field_engineers() {
    // Control: `heavy_by_field_pct` 100 (the setup default 50 left alone
    // in the first run) -> US Engineers take as long at 50 as at 100.
    let s = flat(|_| {});
    let st = s.match_setup().unwrap();
    let t = st.rules.fortify.as_ref().unwrap();
    let d = &st.forts[fort(st, "ww2:hedgehog") as usize];
    let ty = |card: &str| &st.squad_types[st.squad_type_index(card).unwrap() as usize];
    let rate = |card| crate::game::MatchState::build_rate(ty(card), d, t);
    assert_eq!(t.heavy_by_field_pct, 50);
    assert_eq!(rate("ww2:usa_engineer_squad"), 50);
    assert_eq!(rate("ww2:ger_baupioniergruppe"), 150);
    let half = hedgehog_build_ticks("ww2:usa_engineer_squad", 50);
    let full = hedgehog_build_ticks("ww2:usa_engineer_squad", 100);
    let bau = hedgehog_build_ticks("ww2:ger_baupioniergruppe", 50);
    assert!(
        (half * 100).abs_diff(full * 200) <= full * 200 / 20,
        "half {half}, full {full}"
    );
    // Baupioniere at 150 % against a 100 % squad of their size.
    let n = |card| w_members(card);
    let want = full * n("ww2:usa_engineer_squad") * 100 / (n("ww2:ger_baupioniergruppe") * 150);
    assert!(
        bau.abs_diff(want) * 20 <= want,
        "bau {bau}, want {want} (full {full})"
    );
}

fn w_members(card: &str) -> u64 {
    let mut w = flat(|_| {});
    let s = spawn(&mut w, 0, card, 500, 500);
    super::s6_tests::members(&w, s).len() as u64
}

fn demolish(w: &mut World, p: u8, squad: SquadId, at: FxVec2) {
    super::s6_tests::run(w, p, 0, CommandKind::Demolish { squad, at });
}

/// Satchels squad `s` carries.
fn satchels(w: &World, s: SquadId) -> u32 {
    let k = frontline_content::ItemKind::Satchel.index();
    super::s6_tests::members(w, s)
        .iter()
        .map(|m| w.state.entities.kit[m.index()].items[k] as u32)
        .sum()
}

/// Steps until a charge is laid (at most `secs`); the tick it was laid on.
fn until_laid(w: &mut World, secs: u64) -> Option<u64> {
    for _ in 0..secs * HZ {
        w.step(&[]);
        if !fs(w).charges.is_empty() {
            return Some(w.tick() - 1);
        }
    }
    None
}

#[test]
fn demolish_lays_a_satchel_and_destroys_a_hedgehog() {
    // Control: `demolish_pct` 100 -> the hedgehog stands at 300 hp.
    let mut w = flat(|_| {});
    let hh = hedgehogs(&mut w, 500, 500, 1, 1)[0];
    let eng = spawn(&mut w, 0, "ww2:ger_engineer_squad", 470, 500);
    let before = satchels(&w, eng);
    assert!(before > 0);
    demolish(&mut w, 0, eng, c(501, 501));
    assert!(
        matches!(order_of(&w, eng), Order::Demolish { piece, .. } if piece == hh),
        "{:?}",
        refused(&w)
    );
    let laid = until_laid(&mut w, 120).expect("charge laid");
    let ch = fs(&w).charges[0].clone();
    assert_eq!(ch.piece, Some(hh));
    assert_eq!(ch.from, ch.to);
    assert_eq!(ch.fuse_tick, laid + 10 * HZ);
    assert_eq!(order_of(&w, eng), Order::Idle);
    assert_eq!(satchels(&w, eng), before - 1);
    assert_eq!(fs(&w).accounts[0].demolitions, 1);
    super::s6_tests::steps(&mut w, 10 * HZ + 2);
    assert!(fs(&w).charges.is_empty());
    assert!(
        !fs(&w).pieces.contains_key(&hh),
        "stands at {:?}",
        fs(&w).pieces.get(&hh).map(|q| q.hp)
    );
    let a = &fs(&w).accounts[0];
    assert_eq!((a.demolitions, a.pieces_demolished), (1, 1));
    assert_eq!(fs(&w).accounts[1].pieces_lost, 1);
}

/// Review finding 22 (S10b-ai spec 3): a demolition that takes down the
/// demolisher's own piece counts `own_pieces_demolished`; an enemy piece
/// does not. Control: the own-piece check in `demolish_piece` removed ->
/// the own hedgehog reads 0 ("own piece counted").
#[test]
fn demolishing_an_own_piece_counts_own_pieces_demolished() {
    for owner in [0u8, 1] {
        let mut w = flat(|_| {});
        let hh = hedgehogs(&mut w, 500, 500, 1, owner)[0];
        let eng = spawn(&mut w, 0, "ww2:ger_engineer_squad", 470, 500);
        demolish(&mut w, 0, eng, c(501, 501));
        until_laid(&mut w, 120).expect("charge laid");
        super::s6_tests::steps(&mut w, 10 * HZ + 2);
        assert!(!fs(&w).pieces.contains_key(&hh), "owner {owner}: stands");
        let a = &fs(&w).accounts[0];
        assert_eq!(a.pieces_demolished, 1, "owner {owner}");
        let own = u32::from(owner == 0);
        assert_eq!(
            a.own_pieces_demolished, own,
            "owner {owner}: own piece counted"
        );
        assert_eq!(fs(&w).accounts[1].own_pieces_demolished, 0);
    }
}

#[test]
fn demolish_needs_capability_and_satchel() {
    // Control: give the grenadiers `demolish` -> `NotAllowed` (no satchel)
    // where `CannotBuild` is expected.
    use crate::fort::FortRefusal;
    let mut w = flat(|_| {});
    hedgehogs(&mut w, 500, 500, 1, 1);
    let rifles = spawn(&mut w, 0, "ww2:ger_grenadiergruppe", 480, 500);
    demolish(&mut w, 0, rifles, c(500, 500));
    assert_eq!(refused(&w), Some(FortRefusal::CannotBuild));
    let eng = spawn(&mut w, 0, "ww2:ger_engineer_squad", 480, 520);
    demolish(&mut w, 0, eng, c(500, 560));
    assert_eq!(
        refused(&w),
        Some(FortRefusal::NoTarget),
        "nothing within 25 m"
    );
    let k = frontline_content::ItemKind::Satchel.index();
    for m in super::s6_tests::members(&w, eng) {
        w.state.entities.kit[m.index()].items[k] = 0;
    }
    demolish(&mut w, 0, eng, c(500, 500));
    assert_eq!(refused(&w), Some(FortRefusal::NotAllowed));
    assert_eq!(order_of(&w, eng), Order::Idle);
    assert_eq!(order_of(&w, rifles), Order::Idle);
}

#[test]
fn demolish_ends_when_charge_laid_or_target_gone() {
    // Control: keep the order when the piece is gone -> still `Demolish`.
    let mut w = flat(|_| {});
    let hh = hedgehogs(&mut w, 500, 500, 1, 1)[0];
    let eng = spawn(&mut w, 0, "ww2:ger_engineer_squad", 478, 500);
    let before = satchels(&w, eng);
    demolish(&mut w, 0, eng, c(500, 500));
    let man = match order_of(&w, eng) {
        Order::Demolish { man, .. } => man.unwrap(),
        o => panic!("{o:?} {:?}", refused(&w)),
    };
    super::s6_tests::steps(&mut w, HZ);
    assert!(w.state.entities.unit[man.index()].detached, "walking");
    w.edit_fort(|f| f.pieces.get_mut(&hh).unwrap().hp = 1);
    let mortar = weapon_id(&w, "ww2:m1_mortar");
    assert!(w.drop_shell_now(0, mortar, c(501, 502)));
    for _ in 0..2 * HZ {
        w.step(&[]);
        if !fs(&w).pieces.contains_key(&hh) {
            break;
        }
    }
    // Broken in the damage stage, seen in the same tick's status stage.
    assert!(!fs(&w).pieces.contains_key(&hh), "the test HE broke it");
    assert_eq!(order_of(&w, eng), Order::Idle);
    assert_eq!(refused(&w), Some(crate::fort::FortRefusal::NoTarget));
    assert!(
        !w.state.entities.unit[man.index()].detached,
        "back in formation"
    );
    assert_eq!(satchels(&w, eng), before, "satchel kept");
    assert!(fs(&w).charges.is_empty());
    assert_eq!(fs(&w).accounts[0].demolitions, 0);
}

#[test]
fn demolition_is_deterministic() {
    // Control: `demolish_fuse_s` 10 -> 11 in the reloaded run -> hashes
    // differ (the fuse tick is saved and hashed).
    let run = |reload: bool| {
        let mut w = flat(|_| {});
        hedgehogs(&mut w, 500, 497, 3, 1);
        let eng = spawn(&mut w, 0, "ww2:usa_engineer_squad", 470, 500);
        demolish(&mut w, 0, eng, c(500, 500));
        assert!(matches!(order_of(&w, eng), Order::Demolish { .. }));
        for t in 0..60 * HZ {
            if reload && t == 2 * HZ {
                w = World::load(&w.save(), 1).unwrap();
            }
            w.step(&[]);
        }
        let a = &fs(&w).accounts[0];
        (
            w.state_hash(),
            w.full_state_hash(),
            a.demolitions,
            a.pieces_demolished,
            fs(&w).pieces.len(),
        )
    };
    let a = run(false);
    assert_eq!((a.2, a.3, a.4), (1, 1, 2), "{a:?}");
    assert_eq!(a, run(false));
    assert_eq!(a, run(true));
}

// --- Build E: the bunker (R21-R24, R26) ---

use super::s6_tests::{members, steps};

/// A flat world (seed `seed`) with the bunker blueprint edited.
pub(super) fn bunker_world(
    seed: u64,
    edit: impl FnOnce(&mut frontline_content::setup::FortSetup),
) -> World {
    flat_world_with(1, |s| {
        s.seed = seed;
        // 250 kg carried by one squad (the cap is 20 kg a man).
        s.rules.fortify.as_mut().unwrap().carry_kg_per_man = 50;
        let i = fort(s, "ww2:bunker") as usize;
        edit(&mut s.forts[i]);
    })
}

/// A bunker of player 0 at (500, 500) facing +x: its pit dug, its pieces
/// standing when `built`; five men of a grenadier squad in the pit along x
/// (0.4 m apart around the centre), the rest 60 m off. (Pieces, the five.)
pub(super) fn bunker_at(w: &mut World, built: bool) -> (Vec<u32>, Vec<SimId>) {
    let s = w.match_setup().unwrap();
    let f = fort(s, "ww2:bunker");
    let half = Fx::from(s.rules.bunker_pit.pit_line).div_int(2);
    let (x, y) = (Fx::from_int(500), Fx::from_int(500));
    assert!(w.dig_pit_now(
        &[FxVec2::new(x - half, y), FxVec2::new(x + half, y)],
        frontline_content::DigProfile::BunkerPit
    ));
    if built {
        assert!(w.build_now(f, &[c(500, 500), c(510, 500)], PlayerId(0)));
    }
    let pieces: Vec<u32> = fs(w)
        .pieces
        .iter()
        .filter(|(_, q)| q.fort == f)
        .map(|(&id, _)| id)
        .collect();
    let sq = spawn(w, 0, "ww2:ger_grenadiergruppe", 500, 500);
    let all = members(w, sq);
    for (k, &m) in all.iter().enumerate() {
        let p = if k < 5 {
            FxVec2::new(x + Fx::from_milli(400 * (k as i32 - 2)), y)
        } else {
            c(560 + k as i32, 560)
        };
        w.place_soldier(m, p);
    }
    steps(w, 1);
    (pieces, all.into_iter().take(5).collect())
}

fn piece_hp(w: &World, id: u32) -> Option<u32> {
    fs(w).pieces.get(&id).map(|q| q.hp)
}

/// Men of `men` dead (hp 0 or no longer a soldier).
pub(super) fn lost(w: &World, men: &[SimId]) -> usize {
    men.iter()
        .filter(|m| {
            w.state.entities.kind[m.index()] != crate::ecs::EntityKind::Soldier
                || w.state.entities.combat[m.index()].hp == 0
        })
        .count()
}

/// Steps `ticks` holding `men` on their spots (they would run).
pub(super) fn hold(w: &mut World, men: &[SimId], ticks: u64) {
    let spots: Vec<FxVec2> = men.iter().map(|&m| pos(w, m)).collect();
    for _ in 0..ticks {
        for (m, p) in men.iter().zip(&spots) {
            if w.state.entities.kind[m.index()] == crate::ecs::EntityKind::Soldier {
                w.place_soldier(*m, *p);
            }
        }
        w.step(&[]);
    }
}

fn weapon_of(w: &World, id: &str) -> u16 {
    w.match_setup()
        .unwrap()
        .weapons
        .iter()
        .position(|x| x.id == id)
        .unwrap() as u16
}

fn satchel_item(w: &World) -> u16 {
    w.match_setup()
        .unwrap()
        .items
        .iter()
        .position(|i| i.kind == frontline_content::ItemKind::Satchel)
        .unwrap() as u16
}

/// A player-1 thrower far off.
fn thrower(w: &mut World) -> SimId {
    let sq = spawn(w, 1, "ww2:usa_engineer_squad", 900, 100);
    members(w, sq)[0]
}

/// Piece `id`'s top (m).
fn piece_top(w: &World, id: u32) -> Fx {
    let q = &fs(w).pieces[&id];
    let b = &w.match_setup().unwrap().forts[q.fort as usize].pieces[q.part as usize];
    q.z + Fx::from(b.half[2]).mul_int(2)
}

#[test]
fn bunker_digs_pit_then_builds_six_pieces() {
    // Control: a five-piece def (drop the last front wall) -> the count
    // assert fails (seen: "left: 5, right: 6").
    let mut w = bunker_world(1, |_| {});
    let f = fort(w.match_setup().unwrap(), "ww2:bunker");
    let eng = spawn(&mut w, 0, "ww2:ger_baupioniergruppe", 495, 510);
    w.edit_fort(|s| {
        s.carried_g.insert(eng, 250_000);
    });
    fortify(&mut w, 0, eng, f, vec![c(500, 510), c(510, 510)]);
    assert_eq!(fs(&w).works.len(), 1, "{:?}", refused(&w));
    for _ in 0..(3600 * HZ / 30) {
        steps(&mut w, 30);
        if fs(&w).works.is_empty() {
            break;
        }
    }
    assert!(fs(&w).works.is_empty(), "the bunker is done");
    let t = &w.state.terrain;
    let dug = t.base_height_at(c(500, 510)) - t.height_at(c(500, 510));
    assert!(dug > Fx::from_ratio(12, 10), "the pit is dug ({dug:?})");
    let d = &w.match_setup().unwrap().forts[f as usize];
    let pieces: Vec<_> = fs(&w).pieces.values().filter(|q| q.fort == f).collect();
    assert_eq!(pieces.len(), 6);
    // R23 positions (cm, facing +x): roof, rear, sides, front walls.
    let want = [
        (0, 0, 40),
        (-190, 0, 0),
        (0, -165, 0),
        (0, 165, 0),
        (190, -112, 0),
        (190, 112, 0),
    ];
    for q in &pieces {
        let (x, y, z) = want[q.part as usize];
        let at = FxVec2::new(
            Fx::from_int(500) + Fx::from_ratio(x, 100),
            Fx::from_int(510) + Fx::from_ratio(y, 100),
        );
        assert!(
            (q.pos - at).length() < Fx::from_milli(20),
            "part {}",
            q.part
        );
        let z_want = w.state.terrain.base_height_at(c(500, 510)) + Fx::from_ratio(z, 100);
        assert!(
            (q.z - z_want).abs() < Fx::from_milli(20),
            "part {} z",
            q.part
        );
        assert_eq!(q.hp, d.pieces[q.part as usize].hp);
    }
    let a = &fs(&w).accounts[0];
    assert_eq!(a.works_done[f as usize], 1);
    assert_eq!(a.bunkers_built, 1);
    assert_eq!(a.obstacles_built, 0);
}

/// Ticks `card` (player 0) spends in the bunker's build step with
/// `heavy_by_field_pct` `heavy` (kg carried; the pit is dug first).
fn bunker_build_ticks(card: &str, heavy: u32) -> u64 {
    let mut w = flat_world_with(1, |s| {
        let t = s.rules.fortify.as_mut().unwrap();
        t.heavy_by_field_pct = heavy;
        t.carry_kg_per_man = 50;
    });
    let f = fort(w.match_setup().unwrap(), "ww2:bunker");
    let eng = spawn(&mut w, 0, card, 495, 510);
    w.edit_fort(|s| {
        s.carried_g.insert(eng, 250_000);
    });
    fortify(&mut w, 0, eng, f, vec![c(500, 510), c(510, 510)]);
    assert_eq!(fs(&w).works.len(), 1, "{card}: {:?}", refused(&w));
    let mut ticks = 0;
    for _ in 0..4800 * HZ {
        w.step(&[]);
        if fs(&w)
            .works
            .values()
            .any(|x| x.step == crate::fort::WorkStep::Build && x.work > 0)
        {
            ticks += 1;
        }
        if fs(&w).works.is_empty() {
            break;
        }
    }
    assert_eq!(fs(&w).accounts[0].bunkers_built, 1, "{card}");
    ticks
}

#[test]
fn bunker_needs_build_heavy_or_half_rate_field() {
    // Control: `heavy_by_field_pct` 100 in the "half" run -> half == full,
    // the ratio assert fails.
    let mut w = bunker_world(1, |_| {});
    let f = fort(w.match_setup().unwrap(), "ww2:bunker");
    let rifles = spawn(&mut w, 0, "ww2:ger_grenadiergruppe", 495, 510);
    fortify(&mut w, 0, rifles, f, vec![c(500, 510), c(510, 510)]);
    assert_eq!(refused(&w), Some(crate::fort::FortRefusal::CannotBuild));
    assert!(fs(&w).works.is_empty());
    let half = bunker_build_ticks("ww2:usa_engineer_squad", 50);
    let full = bunker_build_ticks("ww2:usa_engineer_squad", 100);
    assert!(
        (half * 100).abs_diff(full * 200) <= full * 200 / 20,
        "half {half}, full {full}"
    );
}

#[test]
fn small_arms_do_not_damage_bunker() {
    // 150 Kar98k and 150 MG 42 rounds of player 1 from 20 m in front at
    // the slit and the front walls: no piece loses hp. Control: armour 1 on
    // the walls -> hp lost (checked below with the same volley).
    let volley = |armour: u32| {
        let mut w = bunker_world(1, |d| {
            for p in d.pieces.iter_mut().skip(1) {
                p.armour_mm = armour;
            }
        });
        let (pieces, men) = bunker_at(&mut w, true);
        let full: u32 = pieces.iter().map(|&p| piece_hp(&w, p).unwrap()).sum();
        let guns = [weapon_of(&w, "ww2:kar98k"), weapon_of(&w, "ww2:mg42")];
        for k in 0..300 {
            let g = guns[k % 2];
            let ground = w.state.terrain.base_height_at(c(502, 500));
            let y = Fx::from_milli(((k as i32 * 37) % 300) - 150);
            let z = ground + Fx::from_milli(50 + (k as i32 * 13) % 300);
            let from = crate::fx::FxVec3::new(Fx::from_int(522), y + Fx::from_int(500), z);
            let to = crate::fx::FxVec3::new(Fx::from_int(500), y + Fx::from_int(500), z);
            assert!(w.fire_round_now(1, g, from, to));
            hold(&mut w, &men, 2);
        }
        hold(&mut w, &men, HZ);
        let left: u32 = pieces.iter().map(|&p| piece_hp(&w, p).unwrap_or(0)).sum();
        full - left
    };
    assert_eq!(volley(100), 0, "armour 100 takes no small-arms damage");
    assert!(volley(1) > 0, "control: thin walls take the rounds");
}

#[test]
fn bunker_roof_survives_three_81mm() {
    // Three GrW 34 bombs of player 1 on the roof centre: the roof takes
    // 3 x 150 and stands at 550; nobody of the five inside is lost.
    // Control: roof hp 400 -> broken after three (checked below).
    let bombs = |roof_hp: u32| {
        let mut w = bunker_world(1, |d| d.pieces[0].hp = roof_hp);
        let (pieces, men) = bunker_at(&mut w, true);
        let roof = pieces[0];
        let mortar = weapon_of(&w, "ww2:grw34");
        for _ in 0..3 {
            assert!(w.drop_shell_now(1, mortar, c(500, 500)));
            hold(&mut w, &men, 2 * HZ);
        }
        (piece_hp(&w, roof), lost(&w, &men))
    };
    let dmg = {
        let w = bunker_world(1, |_| {});
        w.match_setup().unwrap().weapons[weapon_of(&w, "ww2:grw34") as usize].damage
    };
    assert_eq!(dmg, 150);
    assert_eq!(bombs(1000), (Some(550), 0));
    assert_eq!(bombs(400).0, None, "control: a 400 hp roof breaks");
}

#[test]
fn roof_satchel_kills_nobody_inside() {
    // A satchel of player 1 dropped onto the roof centre rests there and
    // bursts: the roof takes 300 (700 left), nobody inside is lost.
    // Control: roof hp 250 -> the roof breaks (checked below).
    let satchel = |roof_hp: u32| {
        let mut w = bunker_world(1, |d| d.pieces[0].hp = roof_hp);
        let (pieces, men) = bunker_at(&mut w, true);
        let roof = pieces[0];
        let before: Vec<u16> = men
            .iter()
            .map(|m| w.state.entities.combat[m.index()].hp)
            .collect();
        let t = thrower(&mut w);
        let item = satchel_item(&w);
        let p = c(500, 500);
        let above = piece_top(&w, roof) + Fx::from_int(2) - w.state.terrain.height_at(p);
        assert!(w.drop_item_now(t, item, p, above));
        hold(&mut w, &men, 5 * HZ);
        let after: Vec<u16> = men
            .iter()
            .map(|m| w.state.entities.combat[m.index()].hp)
            .collect();
        (piece_hp(&w, roof), lost(&w, &men), before == after)
    };
    assert_eq!(satchel(1000), (Some(700), 0, true));
    assert_eq!(satchel(250).0, None, "control: a 250 hp roof breaks");
}

/// A satchel of player 1 at the slit's inner lip (slit centre, 5 cm above
/// its bottom, 10 cm inside the front walls' centre line), seed `seed`: men
/// of the five lost or hurt to it, (lost, hurt).
fn slit_satchel(seed: u64, closed: bool) -> (usize, usize) {
    let mut w = bunker_world(seed, |d| {
        if closed {
            // The two front walls meet at y 0 (no slit).
            for k in [4, 5] {
                d.pieces[k].half[1] = frontline_content::setup::Fixed::from_ratio(185, 200);
                d.pieces[k].offset[1] = frontline_content::setup::Fixed::from_ratio(
                    if k == 4 { -925 } else { 925 },
                    1000,
                );
            }
        }
    });
    let (_, men) = bunker_at(&mut w, true);
    let before: Vec<u16> = men
        .iter()
        .map(|m| w.state.entities.combat[m.index()].hp)
        .collect();
    let t = thrower(&mut w);
    let item = satchel_item(&w);
    let p = FxVec2::new(Fx::from_int(500) + Fx::from_milli(1800), Fx::from_int(500));
    // The slit's bottom: the front walls' base (the site's undug surface).
    let bottom = fs(&w).pieces.values().find(|q| q.part == 4).unwrap().z;
    let above = bottom + Fx::from_milli(50) - w.state.terrain.height_at(p);
    assert!(w.drop_item_now(t, item, p, above));
    hold(&mut w, &men, 12 * HZ);
    let hurt = men
        .iter()
        .zip(&before)
        .filter(|(m, b)| w.state.entities.combat[m.index()].hp < **b)
        .count();
    (lost(&w, &men), hurt)
}

#[test]
fn satchel_at_slit_reaches_men_inside() {
    // Control: close the slit (the front walls meet) -> none lost
    // (checked below on seed 1).
    for seed in 1..=10 {
        let (dead, _) = slit_satchel(seed, false);
        assert!(dead >= 2, "seed {seed}: {dead} of 5 lost");
    }
    assert_eq!(slit_satchel(1, true), (0, 0), "control: no slit, no losses");
}

/// Review 17 (S10b-ai): a Demolish clicked just outside a bunker's slit,
/// the engineers off to one side, lays its charge in the slit (|y| under
/// the slit's 40 cm half-width), not at the box corner nearest the man.
/// Control: the man's stand spot taken from the box point nearest him (the
/// old `order_demolish`) -> the charge lies at the roof corner (y 1.85 m
/// off the slit's centre; seen).
#[test]
fn demolish_clicked_at_slit_lays_charge_in_slit() {
    let mut w = bunker_world(1, |_| {});
    let (_, men) = bunker_at(&mut w, true);
    // The five wait far off (else they and the engineers fight).
    for (k, &m) in men.iter().enumerate() {
        w.place_soldier(m, c(900 + k as i32, 900));
    }
    let eng = spawn(&mut w, 1, "ww2:usa_engineer_squad", 515, 507);
    let click = FxVec2::new(Fx::from_int(502) + Fx::from_milli(300), Fx::from_int(500));
    demolish(&mut w, 1, eng, click);
    assert!(
        matches!(order_of(&w, eng), Order::Demolish { .. }),
        "{:?}",
        refused(&w)
    );
    until_laid(&mut w, 120).expect("charge laid");
    let ch = fs(&w).charges[0].clone();
    let off = (ch.from.y - Fx::from_int(500)).abs();
    assert!(
        off < Fx::from_milli(400),
        "charge {:?}: {off:?} off the slit",
        ch.from
    );
    let out = ch.from.x - Fx::from_int(500);
    assert!(
        out > Fx::from_milli(1700),
        "charge {:?} on the front",
        ch.from
    );
}

#[test]
fn ap_shots_break_the_roof_after_seven() {
    // PzGr 39 shots of player 1 into the roof's side (80 cm above the
    // surface, under it only the pit): `shot_piece_damage` 150 each.
    // Control: `shot_piece_damage` 100 -> seven leave it standing.
    let shots = |n: u32, dmg: u32| {
        let mut w = flat_world_with(1, |s| {
            s.rules.fortify.as_mut().unwrap().shot_piece_damage = dmg
        });
        let (pieces, _) = bunker_at(&mut w, true);
        let roof = pieces[0];
        let s = w.match_setup().unwrap();
        let wi = weapon_of(&w, "ww2:kwk40_l48");
        let ai = s
            .ammo
            .iter()
            .position(|a| a.id == "ww2:75mm_pzgr39")
            .unwrap() as u16;
        let z = w.state.terrain.base_height_at(c(500, 500)) + Fx::from_milli(800);
        for _ in 0..n {
            let from = crate::fx::FxVec3::new(Fx::from_int(500), Fx::from_int(470), z);
            let to = crate::fx::FxVec3::new(Fx::from_int(500), Fx::from_int(530), z);
            assert!(w.fire_shell_now(1, wi, ai, from, to));
            steps(&mut w, HZ);
        }
        piece_hp(&w, roof)
    };
    assert_eq!(shots(6, 150), Some(100), "six leave it at 100");
    assert_eq!(shots(7, 150), None, "seven break it");
    assert!(shots(7, 100).is_some(), "control: at 100 seven do not");
}

#[test]
fn bunker_saves_loads_and_hashes() {
    // Control: one piece dropped after load -> the hash differs (below).
    let mut w = bunker_world(1, |_| {});
    let f = fort(w.match_setup().unwrap(), "ww2:bunker");
    let eng = spawn(&mut w, 0, "ww2:ger_baupioniergruppe", 495, 510);
    w.edit_fort(|s| {
        s.carried_g.insert(eng, 250_000);
    });
    fortify(&mut w, 0, eng, f, vec![c(500, 510), c(510, 510)]);
    // Mid-build: step until the build step has begun.
    for _ in 0..3600 * HZ {
        w.step(&[]);
        if fs(&w)
            .works
            .values()
            .any(|x| x.step == crate::fort::WorkStep::Build && x.work > 0)
        {
            break;
        }
    }
    let mut built = bunker_world(1, |_| {});
    bunker_at(&mut built, true);
    for orig in [&w, &built] {
        let bytes = orig.save();
        let mut a = World::load(&bytes, 1).unwrap();
        assert_eq!(a.state_hash(), orig.state_hash());
        assert_eq!(a.game().unwrap().fort(), fs(orig));
        let back = World::load(&a.save(), 1).unwrap();
        assert_eq!(back.state_hash(), a.state_hash());
        if !fs(&a).pieces.is_empty() {
            let mut x = World::load(&bytes, 1).unwrap();
            x.edit_fort(|s| {
                let k = *s.pieces.keys().next().unwrap();
                s.pieces.remove(&k);
            });
            assert_ne!(
                x.state_hash(),
                a.state_hash(),
                "control: a dropped piece changes the hash"
            );
        } else {
            assert!(!fs(&a).works.is_empty(), "mid-build state has its work");
        }
        steps(&mut a, HZ);
    }
}

#[test]
fn bunker_pieces_share_one_base_on_a_slope() {
    // On the skirmish map at (420, 420) (the fortify rows' site; the
    // undug surface rises to the east) every piece stands on the site's
    // undug surface, so the roof meets the walls and the slit is its full
    // 40 cm. Control: each piece on the surface at its own centre (the
    // S10a rule) -> the walls' z differ from the site's (seen).
    let mut s = ai_setup(1);
    s.players.iter_mut().for_each(|p| p.is_ai = false);
    let mut w = World::new_match(s, 1).unwrap();
    let f = fort(w.match_setup().unwrap(), "ww2:bunker");
    let t = &w.state.terrain;
    let (east, west) = (c(422, 420), c(418, 420));
    assert!(
        (t.base_height_at(east) - t.base_height_at(west)).abs() > Fx::from_milli(100),
        "the site slopes"
    );
    assert!(w.build_now(f, &[c(420, 420), c(430, 420)], PlayerId(0)));
    let base = w.state.terrain.base_height_at(c(420, 420));
    let d = &w.match_setup().unwrap().forts[f as usize];
    for q in fs(&w).pieces.values() {
        let off = Fx::from(d.pieces[q.part as usize].offset[2]);
        assert_eq!(q.z, base + off, "part {}", q.part);
    }
}

// ---- Build F: snapshot.

#[test]
fn fort_snapshot_lists_mines_with_known_bits() {
    // Control: fill `known` with all bits in `fill_fort` -> the enemy mine
    // shows player 0's bit before anyone found it.
    let mut w = flat(|_| {});
    let m = mine_at(&mut w, "ww2:at_minefield", 5000, 5000, 1);
    let eng = spawn(&mut w, 0, "ww2:ger_engineer_squad", 470, 500);
    w.step(&[]);
    let snap = &w.snapshot().fort.mines;
    assert_eq!(snap.len(), 1, "every mine is listed");
    assert_eq!(snap[0].id, m);
    assert_eq!(snap[0].owner, 1);
    assert_eq!(snap[0].known & 0b01, 0, "unknown to the enemy until found");
    assert_ne!(snap[0].known & 0b10, 0, "known to its owner");
    assert!((snap[0].pos[0] - 500.0).abs() < 0.01 && (snap[0].pos[1] - 500.0).abs() < 0.01);
    w.edit_fort(|f| f.mines.get_mut(&m).unwrap().known |= 0b01);
    w.step(&[]);
    assert_eq!(
        w.snapshot().fort.mines[0].known,
        0b11,
        "found: player 0's bit set"
    );
    // A `clear_mines` order shows its lane.
    clear_mines(&mut w, 0, eng, c(470, 510), c(470, 540));
    w.step(&[]);
    let sq = w.snapshot().squads.iter().find(|s| s.id == eng.0).unwrap();
    let lane = sq.clear_lane.expect("the lane");
    assert_eq!((lane.a, lane.b), ([470.0, 510.0], [470.0, 540.0]));
    assert_eq!(sq.demolish, None);
}

#[test]
fn charge_ids_stay_below_the_key_bound() {
    // Review R3: charge ids stay below `MAX_CHARGE_ID` (2^28) so the burst
    // keys `id * 8` cannot wrap into the mine keys. Control: drop the
    // `next_charge < MAX_CHARGE_ID` guard in `demolish_tick` -> the charge
    // is laid, the counter passes the bound and the save no longer loads.
    use crate::fort::MAX_CHARGE_ID;
    let mut w = flat(|_| {});
    hedgehogs(&mut w, 500, 500, 1, 1);
    let eng = spawn(&mut w, 0, "ww2:ger_engineer_squad", 470, 500);
    let before = satchels(&w, eng);
    w.edit_fort(|s| s.next_charge = MAX_CHARGE_ID);
    demolish(&mut w, 0, eng, c(501, 501));
    assert_eq!(until_laid(&mut w, 120), None, "no charge past the bound");
    assert_eq!(satchels(&w, eng), before, "the satchel is kept");
    assert_eq!(fs(&w).next_charge, MAX_CHARGE_ID);
    assert!(World::load(&w.save(), 1).is_ok(), "at the bound it loads");
    w.edit_fort(|s| s.next_charge = MAX_CHARGE_ID + 1);
    assert!(
        World::load(&w.save(), 1).is_err(),
        "past the bound it does not"
    );
}

/// Review R17: a mine found behind and beside a sweeper. Control: the
/// lifter's stand-off always `-d` (the lane's back side) -> his straight
/// walk to it passes within 30 cm of the mine and sets it off.
#[test]
fn lifter_does_not_cross_a_mine_behind_him() {
    let f = |w: &World| fort(w.match_setup().unwrap(), "ww2:ap_minefield");
    for side in [1i32, -1] {
        let mut w = flat(|_| {});
        let eng = spawn(&mut w, 0, "ww2:ger_engineer_squad", 470, 500);
        w.step(&[]);
        clear_mines(&mut w, 0, eng, c(480, 500), c(510, 500));
        assert!(
            matches!(order_of(&w, eng), Order::ClearMines { .. }),
            "{:?}",
            refused(&w)
        );
        let mut lead = None;
        for _ in 0..120 * HZ {
            w.step(&[]);
            let along = match order_of(&w, eng) {
                Order::ClearMines { along, .. } => along,
                o => panic!("order {o:?}"),
            };
            if along >= Fx::from_int(10) {
                // The front sweeper on the `side` half of the lane.
                lead = super::s6_tests::members(&w, eng)
                    .into_iter()
                    .filter(|&m| (pos(&w, m).y - Fx::from_int(500)) * Fx::from_int(side) > Fx::ZERO)
                    .max_by_key(|&m| pos(&w, m).x);
                break;
            }
        }
        let m = lead.expect("a sweeper on that side");
        let p = pos(&w, m);
        let at = p + FxVec2::new(Fx::from_milli(-500), Fx::from_milli(100 * side));
        let fid = f(&w);
        let id = w.lay_mine_now(fid, at, PlayerId(1)).unwrap();
        w.edit_fort(|s| s.mines.get_mut(&id).unwrap().known |= 1);
        for _ in 0..60 * HZ {
            w.step(&[]);
            if !fs(&w).mines.contains_key(&id) {
                break;
            }
        }
        let a = &fs(&w).accounts;
        assert_eq!(
            a[1].mines_set_off, 0,
            "side {side}: the lifter stepped on it"
        );
        assert!(!fs(&w).mines.contains_key(&id), "side {side}: lifted");
        assert_eq!(a[0].mines_lifted, 1, "side {side}");
    }
}

/// Review R12: Demolish does not find enemy pieces the player cannot see;
/// it refuses them as "nothing there". Control: drop the sight filter in
/// `order_demolish` -> the order is accepted with no own man in sight.
#[test]
fn demolish_ignores_unseen_enemy_pieces() {
    use crate::fort::FortRefusal;
    let mut w = flat(|_| {});
    // The frame's sight: the faction's longest squad sight.
    let s = w.match_setup().unwrap();
    let fac = s.players[0].faction;
    let sight = s
        .squad_types
        .iter()
        .filter(|t| t.faction == fac)
        .map(|t| Fx::from(t.sight).round_int())
        .max()
        .unwrap()
        .max(150);
    let x = (sight + 60).min(900);
    assert!(x - sight >= 40, "sight {sight} fits the map");
    let hh = hedgehogs(&mut w, x, 500, 1, 1)[0];
    let eng = spawn(&mut w, 0, "ww2:ger_engineer_squad", x - sight - 20, 500);
    w.step(&[]);
    demolish(&mut w, 0, eng, c(x + 1, 501));
    assert_eq!(
        refused(&w),
        Some(FortRefusal::NoTarget),
        "unseen (sight {sight}): nothing there; order {:?}",
        order_of(&w, eng)
    );
    assert_eq!(order_of(&w, eng), Order::Idle);
    // A rifle squad of the same player 100 m off sees it: accepted.
    spawn(&mut w, 0, "ww2:ger_grenadiergruppe", x - 100, 500);
    w.step(&[]);
    demolish(&mut w, 0, eng, c(x + 1, 501));
    assert!(
        matches!(order_of(&w, eng), Order::Demolish { piece, .. } if piece == hh),
        "{:?}",
        refused(&w)
    );
}
