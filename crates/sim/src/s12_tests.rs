//! S12a Build A tests (docs/slices/S12-wounds-medics.md 3, 4, 6): the
//! SAVE 30 layout and its load checks, then the wound rules W1-W5,
//! W12-W14 (every test turns `[wounds] enabled` on in the test only). Each
//! test names its control: the edit that makes it fail.

use super::s6_tests::{flat_world_with, members, spawn};
use super::*;
use crate::ecs::EntityKind;
use crate::wounds::{Casualty, DOWNED, WOUNDED, Wound};

const RIFLES: &str = "ww2:ger_grenadiergruppe";
const US_RIFLES: &str = "ww2:usa_rifle_squad";

fn c(x: i32, y: i32) -> FxVec2 {
    FxVec2::from_int(x, y)
}

fn game_mut(s: &mut SimState) -> &mut MatchState {
    s.game.as_mut().unwrap()
}

fn wd(s: &mut SimState, id: SimId) -> &mut Wound {
    &mut s.entities.combat[id.index()].wound
}

fn cas(s: &mut SimState, id: SimId) -> &mut Casualty {
    s.game.as_mut().unwrap().casualties.get_mut(&id).unwrap()
}

/// A flat match with `[wounds] enabled` = `on`.
pub(super) fn wounds_world(on: u32) -> World {
    flat_world_with(1, |s| s.rules.wounds.enabled = on)
}

/// Turns soldier `id` into a downed man with a casualty bleeding out at
/// `bleed_out_at` (the layout staging; the W5 rule does it in play).
pub(super) fn stage_downed(w: &mut World, id: SimId, bleed_out_at: u64) {
    let tick = w.tick();
    let sq = w.state.entities.squad[id.index()].unwrap();
    let card = w.game().unwrap().squads[&sq].type_idx;
    {
        let g = w.state.game.as_mut().unwrap();
        g.kill(&mut w.state.entities, &mut w.hash, id, 30);
        g.players[w.state.entities.owner[id.index()].0 as usize].losses -= 1;
        g.casualties.insert(
            id,
            Casualty {
                from_squad: sq,
                card,
                bleed_out_at: bleed_out_at.max(tick + 1),
                ..Casualty::default()
            },
        );
    }
    let e = &mut w.state.entities;
    e.kind[id.index()] = EntityKind::Downed;
    let cb = &mut e.combat[id.index()];
    cb.corpse = 0;
    cb.hp = 1;
    cb.wound = Wound {
        state: DOWNED,
        by: (1, crate::game::kill_class::RIFLE),
        ..Wound::default()
    };
    w.hash.mark_entity_chunk(id.chunk());
    w.refresh_hashes();
}

/// The S12 staged save: a German rifle squad with a wounded man
/// mid-bandage and a downed man, a few S12 counters and AI counters.
/// Returns `(world, squad, bandaging man, downed man)`.
fn staged() -> (World, SquadId, SimId, SimId) {
    let mut w = wounds_world(1);
    let sq = spawn(&mut w, 0, RIFLES, 300, 300);
    spawn(&mut w, 1, US_RIFLES, 700, 700);
    super::s6_tests::steps(&mut w, 3);
    let m = members(&w, sq);
    let (band, down) = (m[1], m[2]);
    assert!(w.edit_soldier(band, |_, c| {
        c.hp = 30;
        c.wound = Wound {
            state: WOUNDED,
            legs: true,
            paid: true,
            bleed_acc_ms: 1200,
            bandaging: 150,
            by: (1, crate::game::kill_class::MG),
            ..Wound::default()
        };
    }));
    let t = w.tick();
    stage_downed(&mut w, down, t + 90 * 30);
    assert!(w.edit_player(PlayerId(0), |p| {
        p.s12.downed = 1;
        p.s12.head_hits = [2, 1];
        p.s12.leg_hits[1] = 3;
    }));
    assert!(w.edit_player(PlayerId(1), |p| {
        p.s12.downed_by_class[crate::game::kill_class::MG as usize] = 1;
    }));
    (w, sq, band, down)
}

/// Spec 3: the S12 state saves, loads and hashes. Control: an unedited
/// reload keeps the hash (each edit must change it).
#[test]
fn s12_layout_roundtrips_and_hashes() {
    let (w, _sq, band, down) = staged();
    let bytes = w.save();
    let back = World::load(&bytes, 1).expect("the S12 state loads");
    assert_eq!(back.state(), w.state());
    assert_eq!(back.full_state_hash(), w.full_state_hash());
    assert_eq!(back.save(), bytes);
    let h = back.full_state_hash();
    type Edit = Box<dyn Fn(&mut SimState)>;
    let edits: Vec<Edit> = vec![
        Box::new(move |s| wd(s, band).bandaging -= 1),
        Box::new(move |s| wd(s, band).bleed_acc_ms += 1),
        Box::new(move |s| wd(s, band).legs = false),
        Box::new(move |s| wd(s, band).paid = false),
        Box::new(move |s| wd(s, band).treated = true),
        Box::new(move |s| wd(s, band).by.1 = 0),
        Box::new(move |s| wd(s, down).by.0 = 0),
        Box::new(move |s| game_mut(s).casualties.get_mut(&down).unwrap().bleed_out_at += 1),
        Box::new(move |s| game_mut(s).casualties.get_mut(&down).unwrap().card += 1),
        Box::new(|s| game_mut(s).players[0].s12.downed += 1),
        Box::new(|s| game_mut(s).players[0].s12.torso_hits[0] += 1),
        Box::new(|s| game_mut(s).players[1].s12.downed_by_class[0] += 1),
        Box::new(|s| game_mut(s).players[1].s12.refund_mp += 1),
        Box::new(move |s| cas(s, down).tend = Some((SimId(0), u64::MAX))),
    ];
    assert_eq!(World::load(&back.save(), 1).unwrap().full_state_hash(), h);
    for (k, e) in edits.into_iter().enumerate() {
        let mut x = World::load(&bytes, 1).unwrap();
        e(&mut x.state);
        // Hashed in memory: some edits do not load (the last one).
        assert_ne!(x.full_state_hash(), h, "edit {k} is not hashed");
    }
}

/// Spec 3 load validation, one case per check. Control: the staged save
/// itself loads (checked first), so each failure is the edit's.
#[test]
fn load_rejects_bad_s12_state() {
    let (mut w, sq, band, down) = staged();
    let hq_def = super::s6_tests::def(&w, 0, frontline_content::setup::StockKind::Hq, None);
    let hq = super::s6_tests::add_built(&mut w, 0, hq_def, c(250, 250), [0; 3]);
    let bytes = w.save();
    World::load(&bytes, 1).expect("control: the staged save loads");
    let tick = w.tick();
    let other = members(&w, sq)[0];
    type Edit = Box<dyn Fn(&mut SimState)>;
    let cases: Vec<(&str, Edit)> = vec![
        (
            "wound state past Downed",
            Box::new(move |s| wd(s, band).state = 3),
        ),
        (
            "a soldier in state Downed",
            Box::new(move |s| wd(s, band).state = DOWNED),
        ),
        (
            "a downed man not in state Downed",
            Box::new(move |s| wd(s, down).state = WOUNDED),
        ),
        (
            "a fit man bandaged",
            Box::new(move |s| wd(s, other).bandaged = true),
        ),
        (
            "a fit man bandaging",
            Box::new(move |s| wd(s, other).bandaging = 5),
        ),
        (
            "bandaging past bandage_s",
            Box::new(move |s| wd(s, band).bandaging = 10 * 30 + 1),
        ),
        (
            "bandaged and bandaging",
            Box::new(move |s| wd(s, band).bandaged = true),
        ),
        (
            "hitter names no player",
            Box::new(move |s| wd(s, band).by.0 = 2),
        ),
        (
            "hitter class unknown",
            Box::new(move |s| wd(s, band).by.1 = 99),
        ),
        (
            "a wound on a corpse",
            Box::new(move |s| {
                s.entities.kind[down.index()] = EntityKind::Corpse;
                s.entities.combat[down.index()].corpse = 5;
                s.game.as_mut().unwrap().casualties.clear();
            }),
        ),
        (
            "a downed man at 0 HP",
            Box::new(move |s| s.entities.combat[down.index()].hp = 0),
        ),
        (
            "a downed man with no casualty",
            Box::new(|s| s.game.as_mut().unwrap().casualties.clear()),
        ),
        (
            "a casualty that is a soldier",
            Box::new(move |s| {
                let c0 = cas(s, down).clone();
                s.game.as_mut().unwrap().casualties.insert(other, c0);
            }),
        ),
        ("card unknown", Box::new(move |s| cas(s, down).card = 9999)),
        (
            "bleed-out in the past",
            Box::new(move |s| cas(s, down).bleed_out_at = tick),
        ),
        (
            "bleed-out far ahead",
            Box::new(move |s| cas(s, down).bleed_out_at = u64::MAX),
        ),
        (
            "a medic that is an enemy or gone",
            Box::new(move |s| cas(s, down).tend = Some((SimId(9999), tick + 30))),
        ),
        (
            "a tend that ended",
            Box::new(move |s| cas(s, down).tend = Some((other, tick))),
        ),
        (
            "the same carrier twice",
            Box::new(move |s| cas(s, down).carriers = [Some(other), Some(other)]),
        ),
        (
            "a carrier that is downed",
            Box::new(move |s| cas(s, down).carriers[0] = Some(down)),
        ),
        (
            "destination off the map",
            Box::new(move |s| cas(s, down).dest = Some(c(-5, 10))),
        ),
        (
            "evacuation in the past",
            Box::new(move |s| cas(s, down).evac_at = tick),
        ),
        (
            "beds on a structure that is not an aid post",
            Box::new(move |s| {
                let g = s.game.as_mut().unwrap();
                let st = g.supply6.structures.get_mut(&hq).unwrap();
                st.beds.push(crate::wounds::Bed {
                    man: down,
                    done_at: 0,
                    evac: false,
                });
            }),
        ),
        (
            "s12 counter out of range",
            Box::new(|s| game_mut(s).players[0].s12.bled_out = u32::MAX),
        ),
        (
            "s12 class counter out of range",
            Box::new(|s| game_mut(s).players[0].s12.downed_by_class[3] = u32::MAX),
        ),
    ];
    for (name, e) in cases {
        let mut x = World::load(&bytes, 1).unwrap();
        e(&mut x.state);
        assert!(World::load(&x.save(), 1).is_err(), "{name}: loaded");
    }
}
