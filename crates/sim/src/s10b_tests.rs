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
