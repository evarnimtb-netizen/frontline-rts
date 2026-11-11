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

fn fort(s: &frontline_content::MatchSetup, id: &str) -> u16 {
    s.forts.iter().position(|f| f.id == id).unwrap() as u16
}

fn fs(w: &World) -> &FortState {
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
    // Control: iterate the mines through a `HashMap` in `take_mines` (not
    // committed) -> hashes differ. Two runs of a 300-tick crossing with 40
    // mines, and the second through save/load at tick 150.
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
