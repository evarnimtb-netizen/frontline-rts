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
