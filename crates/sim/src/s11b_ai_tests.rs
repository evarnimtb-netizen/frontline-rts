//! S11b Build C tests (docs/slices/S11b.md 4 H1-H3, L1, 5): the AI's
//! field howitzer buy and use, the pending call-ins of a gun buy, light
//! mortars in the mix, no refill trip for rifle grenades. Each test names
//! its control: the edit that makes it fail.

use super::s6_tests::{flat_world_with, members, spawn, steps};
use super::*;
use crate::ai::ArtyPhase;
use crate::command::CommandKind;
use crate::vehicle::GunState;
use frontline_content::setup::member_trait;

const LEFH: &str = "ww2:ger_lefh18_squad";
const M2A1: &str = "ww2:usa_105mm_m2a1_squad";
const LEIG: &str = "ww2:ger_leig18_squad";
const VB: &str = "ww2:ger_vb_trupp";
const RIFLES: &str = "ww2:ger_grenadiergruppe";

fn c(x: i32, y: i32) -> FxVec2 {
    FxVec2::from_int(x, y)
}

/// A flat match whose player 0 (Germany) is an AI that buys no squads in
/// its mix (every other card `ai_weight` 0), `[ai.arty]` `buy` and `fo` 0, `how` as
/// given from minute 0, the gun cards unlocked at once and given
/// `ai_weight` 5 (the howitzers as Build C ships them), 4,000 MP of
/// starting income and `mp` manpower; `edit` changes the setup last.
fn how_world(
    threads: usize,
    how: u32,
    mp: u32,
    edit: impl FnOnce(&mut frontline_content::MatchSetup),
) -> World {
    let mut w = flat_world_with(threads, |s| {
        s.players[0].is_ai = true;
        let a = &mut s.rules.ai.arty;
        a.buy = 0;
        a.fo = 0;
        a.after_s = 0;
        a.how = how;
        a.how_after_s = 0;
        s.rules.start_manpower = 4000;
        for t in s.squad_types.iter_mut() {
            if t.id == LEFH || t.id == M2A1 || t.id == LEIG || t.id == VB {
                t.initial_cooldown_ticks = 0;
            }
            if t.id == LEFH || t.id == M2A1 {
                t.ai_weight = 5;
            } else if t.id != LEIG {
                t.ai_weight = 0;
            }
        }
        edit(s);
    });
    w.state.game.as_mut().unwrap().players[0].manpower = mp;
    w
}

/// `[ai.arty] buy` 1 (a howitzer needs it since fix C-2) with the
/// infantry gun out of reach (`after_s` 3600, its maximum): no leIG buy
/// and no pot held for it (the runs last at most 90 s).
fn no_inf_gun(s: &mut frontline_content::MatchSetup) {
    s.rules.ai.arty.buy = 1;
    s.rules.ai.arty.after_s = 3600;
}

fn counters(w: &World) -> crate::ai::AiCounters {
    w.game().unwrap().ai[0].as_ref().unwrap().counters.clone()
}

fn ai(w: &World) -> &crate::ai::AiState {
    w.game().unwrap().ai[0].as_ref().unwrap()
}

fn card(w: &World, id: &str) -> u16 {
    w.match_setup().unwrap().squad_type_index(id).unwrap()
}

/// Own squads of player 0 whose card is `id`.
fn own_of(w: &World, id: &str) -> Vec<SquadId> {
    let s = w.match_setup().unwrap();
    w.game()
        .unwrap()
        .squads
        .iter()
        .filter(|(_, q)| q.owner == PlayerId(0) && s.squad_types[q.type_idx as usize].id == id)
        .map(|(&id, _)| id)
        .collect()
}

/// Every call-in the AI has sent so far (each seq once) into `seen`.
fn note_call_ins(w: &World, seen: &mut std::collections::BTreeMap<u32, u16>) {
    for c in &ai(w).pending {
        if let CommandKind::CallIn { squad_type } = c.kind {
            seen.insert(c.seq, squad_type);
        }
    }
}

/// H1: with `how` 1 the AI calls in one leFH 18 inside its own 15 % share
/// (4,000 MP of income: 600 MP for a 480 MP card) and it arrives.
/// Controls: `how` 0 (none), and `how_income_pct` 5 (200 MP: the share
/// says no, `how_pct_holds`).
#[test]
fn ai_buys_a_howitzer_inside_its_own_share() {
    let run = |how: u32, pct: u32| {
        let mut w = how_world(1, how, 5000, |s| {
            no_inf_gun(s);
            s.rules.ai.arty.how_income_pct = pct;
        });
        steps(&mut w, 30 * 90);
        let k = counters(&w);
        (k.how_buys, own_of(&w, LEFH).len(), k.how_pct_holds > 0)
    };
    assert_eq!(run(1, 15), (1, 1, false), "one howitzer, how_max 1");
    assert_eq!(run(0, 15), (0, 0, false), "control: how 0");
    assert_eq!(run(1, 5), (0, 0, true), "control: the share");
}

/// H1: `how_reserve_mp` (300) stays: 480 + 299 MP buys nothing
/// (`how_mp_holds`), 480 + 300 buys it. Control: the second run.
#[test]
fn ai_howitzer_buy_keeps_the_reserve() {
    let run = |mp: u32| {
        let mut w = how_world(1, 1, mp, no_inf_gun);
        steps(&mut w, 30 * 5);
        let k = counters(&w);
        (k.how_buys, k.how_mp_holds > 0)
    };
    assert_eq!(run(480 + 299), (0, true), "the reserve says no");
    assert_eq!(run(480 + 300), (1, false), "control: enough");
}

/// S11b fix C-2: with `[ai.arty] buy` 0 no gun gets an AI record, so the
/// AI buys no howitzer it could not fire. Control: `buy` 1.
#[test]
fn ai_buys_no_howitzer_with_gun_buy_off() {
    let run = |buy: u32| {
        let mut w = how_world(1, 1, 5000, |s| {
            no_inf_gun(s);
            s.rules.ai.arty.buy = buy;
        });
        steps(&mut w, 30 * 90);
        (counters(&w).how_buys, own_of(&w, LEFH).len())
    };
    assert_eq!(run(0), (0, 0), "buy 0: no howitzer");
    assert_eq!(run(1), (1, 1), "control: buy 1");
}

/// S11b fix C-1: the howitzer buy leaves the infantry gun's pot alone.
/// `income_max_pct` 7 of 4,000 MP holds 280 MP for the leIG 18 (320 MP:
/// the share says no to the leIG itself), so 480 + 300 + 279 MP leaves
/// 779 for a howitzer needing 780 (`how_mp_holds`). Control:
/// `income_max_pct` 0 (no pot): the same manpower buys it.
#[test]
fn ai_howitzer_buy_leaves_the_gun_pots_alone() {
    let run = |pct: u32| {
        let mut w = how_world(1, 1, 480 + 300 + 279, |s| {
            s.rules.ai.arty.buy = 1;
            s.rules.ai.arty.income_max_pct = pct;
        });
        steps(&mut w, 30 * 5);
        let k = counters(&w);
        (k.how_buys, k.arty_buys, k.how_mp_holds > 0)
    };
    assert_eq!(run(7), (0, 0, true), "the leIG pot is held");
    assert_eq!(run(0), (1, 0, false), "control: no pot");
}

/// S11b fix: the howitzer's pot (`how_pot`, in `held_pots`) is kept from
/// the other buys: the Grenadiergruppe card is buyable (weight 50), the
/// howitzer buy holds on its reserve (`how_reserve_mp` 100,000), and of
/// its 750 MP the AI spends on rifles (232 MP) only what lies above the
/// pot, 480 MP (15 % of 4,000 capped at the card's price): one squad. Controls: `how` 0 (no pot: spent below
/// 480), and `how_after_s` past the run (no pot yet).
#[test]
fn ai_keeps_the_howitzer_pot_from_other_buys() {
    let run = |how: u32, after: u32| {
        let mut w = how_world(1, how, 750, |s| {
            no_inf_gun(s);
            s.rules.ai.arty.how_reserve_mp = 100_000;
            s.rules.ai.arty.how_after_s = after;
            for t in s.squad_types.iter_mut() {
                if t.id == RIFLES {
                    t.ai_weight = 50;
                    t.initial_cooldown_ticks = 0;
                }
            }
        });
        steps(&mut w, 30 * 60);
        let k = counters(&w);
        let mp = w.game().unwrap().players[0].manpower;
        (k.how_buys, own_of(&w, RIFLES).len() > 0, mp)
    };
    let (buys, rifles, mp) = run(1, 0);
    assert_eq!((buys, rifles), (0, true));
    assert!(mp >= 480, "the pot stays: {mp} MP");
    let (_, rifles, mp) = run(0, 0);
    assert!(rifles && mp < 480, "control how 0: spent to {mp} MP");
    let (_, rifles, mp) = run(1, 3600);
    assert!(
        rifles && mp < 480,
        "control before how_after_s: spent to {mp} MP"
    );
}

/// S11b fix: the howitzer is bought once its pot covers it. 4,000 MP of
/// income at `how_income_pct` 10 makes a 400 MP pot, short of the 480 MP
/// card (`how_pct_holds`, nothing bought); at 15 the pot is 480 and the
/// card is bought from it with the reserve on top. Control: the 10 % run.
#[test]
fn ai_buys_the_howitzer_once_its_pot_covers_it() {
    let run = |pct: u32| {
        let mut w = how_world(1, 1, 480 + 300, |s| {
            no_inf_gun(s);
            s.rules.ai.arty.how_income_pct = pct;
        });
        steps(&mut w, 30 * 90);
        let k = counters(&w);
        (k.how_buys, own_of(&w, LEFH).len(), k.how_pct_holds > 0)
    };
    assert_eq!(run(15), (1, 1, false), "pot full: bought");
    assert_eq!(run(10), (0, 0, true), "control: pot short of the card");
}

/// H1 (review 3) and spec 5 "the infantry gun and a squad are bought in
/// the same plan": a rifle squad's call-in is pending (232 MP) and the
/// player holds 436 MP: with `buy_pending` 1 the infantry-gun buy (320
/// MP) sees 204 MP and waits. Control `buy_pending` 0: both are ordered
/// on the same manpower.
#[test]
fn ai_gun_buy_counts_pending_call_ins() {
    let run = |pending: u32| {
        let mut w = how_world(1, 0, 436, |s| {
            s.rules.ai.arty.buy = 1;
            s.rules.ai.arty.buy_pending = pending;
        });
        let rifles = card(&w, RIFLES);
        let g = w.state.game.as_mut().unwrap();
        let a = g.ai[0].as_mut().unwrap();
        a.pending.push(crate::ai::AiCommand {
            due: u64::MAX / 2,
            seq: 9999,
            kind: CommandKind::CallIn { squad_type: rifles },
        });
        steps(&mut w, 30 * 5);
        counters(&w).arty_buys
    };
    assert_eq!(run(1), 0, "pending call-ins counted");
    assert_eq!(run(0), 1, "control: buy_pending 0 orders both");
}

/// H1: the infantry-gun buy and `arty_pot` skip field howitzer cards: with
/// the leFH 18 first in the roster and a living own leFH 18 (`max_guns`
/// 1), the AI still buys the leIG 18. Control: the leFH 18 card without
/// its `field_howitzer` tag is picked (and counts as the one gun).
#[test]
fn ai_infantry_gun_buy_ignores_howitzer_cards() {
    let run = |tagged: bool, alive: bool| {
        let mut w = how_world(1, 0, 5000, |s| {
            s.rules.ai.arty.buy = 1;
            let lefh = s.squad_type_index(LEFH).unwrap();
            let tag = s.tag("field_howitzer").unwrap();
            if !tagged {
                s.squad_types[lefh as usize].tags.retain(|&t| t != tag);
            }
            let f = s.players[0].faction as usize;
            let roster = &mut s.factions[f].roster;
            roster.retain(|&t| t != lefh);
            roster.insert(0, lefh);
        });
        // Before the first plan.
        if alive {
            spawn(&mut w, 0, LEFH, 100, 100);
        }
        steps(&mut w, 30 * 60);
        (own_of(&w, LEIG).len(), counters(&w).arty_buys)
    };
    assert_eq!(run(true, false), (1, 1), "the leIG 18, not the howitzer");
    assert_eq!(
        run(true, true),
        (1, 1),
        "a living howitzer is no infantry gun"
    );
    assert_eq!(
        run(false, false),
        (0, 1),
        "control: untagged, the leFH 18 is picked"
    );
    assert_eq!(
        run(false, true),
        (0, 0),
        "control: untagged, it fills max_guns"
    );
}

/// The H2 scene (S11a1's A4 scene with the leFH 18): the AI's howitzer at
/// (100, 500), its VB-Trupp at (600, 500) (radio only), a US rifle squad
/// with no rounds at (700, 500) that the team sees, the point marked;
/// `crest` puts a 25 m band across x = 620, between the gun and the
/// point, and the team beyond it at (760, 500). `edit` changes the setup.
fn how_scene(
    threads: usize,
    kill_radio: bool,
    crest: bool,
    edit: impl FnOnce(&mut frontline_content::MatchSetup),
) -> (World, SquadId) {
    let mut w = how_world(threads, 0, 5000, |s| {
        s.rules.ai.arty.buy = 1;
        s.rules.ai.arty.max_guns = 0;
        if crest {
            s.map
                .terrain
                .bands
                .push(frontline_content::setup::BandSetup {
                    from: frontline_content::setup::FixedVec2::from_m([620, 0]),
                    to: frontline_content::setup::FixedVec2::from_m([620, 1024]),
                    height: frontline_content::Fixed::from_int(25),
                    half_width: frontline_content::Fixed::from_int(40),
                });
        }
        edit(s);
    });
    steps(&mut w, 2);
    let q = spawn(&mut w, 0, LEFH, 100, 500);
    let fo = spawn(&mut w, 0, VB, if crest { 760 } else { 600 }, 500);
    let e = spawn(&mut w, 1, "ww2:usa_rifle_squad", 700, 500);
    for m in members(&w, e) {
        w.edit_soldier(m, |k, c| {
            k.rounds = [0; frontline_content::setup::MAX_FEEDS];
            c.mag = 0;
        });
    }
    assert!(w.mark_intel(PlayerId(0), c(700, 500)));
    if kill_radio {
        let s = w.match_setup().unwrap().clone();
        let ty = &s.squad_types[w.game().unwrap().squads[&fo].type_idx as usize];
        for m in members(&w, fo) {
            if ty.members[w.entities().kit[m.index()].slot as usize].traits & member_trait::RADIO
                != 0
            {
                assert!(w.kill_soldier(m));
            }
        }
    }
    (w, q)
}

/// H2: a limbered AI howitzer is ordered to emplace (90 s), is Ready, and
/// fires an observed mission through its observer team (radio); the AI
/// never orders it pushed or hitched (stage B review B-1) and the sim
/// refuses none of its orders. Control: the team's radio man dead: no
/// mission.
#[test]
fn ai_howitzer_emplaces_and_takes_an_observed_mission() {
    let run = |kill_radio: bool| {
        let (mut w, q) = how_scene(1, kill_radio, false, |_| {});
        let gun = members(&w, q)[0];
        w.state
            .game
            .as_mut()
            .unwrap()
            .vehicles
            .get_mut(&gun)
            .unwrap()
            .served
            .as_mut()
            .unwrap()
            .state = GunState::Limbered;
        let mut moved = 0u32;
        for _ in 0..30 * 200 {
            w.step(&[]);
            for c in &ai(&w).pending {
                if matches!(&c.kind, CommandKind::PushGun { guns, .. }
                    | CommandKind::Hitch { guns, .. } if guns.contains(&gun))
                {
                    moved += 1;
                }
            }
        }
        let g = w.game().unwrap();
        let p = &g.players[0];
        (
            g.vehicles[&gun].served.as_ref().unwrap().state,
            ai(&w).arty.get(&q).map(|a| a.phase),
            p.s11.gun_missions[crate::fire_mission::mode::OBSERVED as usize],
            p.s11.masked + p.s11.not_ready,
            p.s11b.how_rounds.iter().sum::<u32>(),
            moved,
            ai(&w).rejected,
        )
    };
    let (state, phase, observed, refused, rounds, moved, rejected) = run(false);
    assert_eq!(state, GunState::Emplaced);
    assert_eq!(phase, Some(ArtyPhase::Ready));
    assert!(observed >= 1, "observed missions {observed}");
    assert!(rounds >= 1, "howitzer rounds {rounds}");
    assert_eq!((refused, moved, rejected), (0, 0, 0));
    let (_, _, observed, _, rounds, _, _) = run(true);
    assert_eq!((observed, rounds), (0, 0), "control: no radio, no mission");
}

/// H2: the AI orders no mission its howitzer would be refused `masked`
/// (behind a 25 m crest the flat arc does not clear): no mission, no
/// refusal. Control: flat ground: a mission.
#[test]
fn ai_does_not_order_a_masked_howitzer_mission() {
    let run = |crest: bool| {
        let (mut w, _) = how_scene(1, false, crest, |_| {});
        steps(&mut w, 30 * 30);
        let p = &w.game().unwrap().players[0];
        (counters(&w).arty_missions, p.s11.masked, ai(&w).rejected)
    };
    let (missions, masked, rejected) = run(true);
    assert_eq!((missions, masked, rejected), (0, 0, 0), "behind the crest");
    assert!(run(false).0 >= 1, "control: flat ground");
}

/// Liveness and neutrality of the switches (spec 4 L1): with every S11b
/// switch 0 (and the howitzer and light mortar cards weighted as shipped)
/// 40 plans call in no howitzer and no light mortar and every S11b AI
/// counter stays 0. Control: `how` 1 calls in a howitzer.
#[test]
fn ai_all_s11b_switches_off_emit_nothing_new() {
    let run = |how: u32| {
        let mut w = how_world(1, how, 5000, |s| {
            // The mix buys again (the shipped weights), light mortars at 50.
            let rifles = s.squad_type_index(RIFLES).unwrap();
            let heavy = s.squad_type_index("ww2:ger_mortar_team").unwrap();
            for t in [rifles, heavy] {
                s.squad_types[t as usize].ai_weight = 100;
            }
            s.rules.ai.arty.buy = 1;
            s.rules.ai.arty.fo = 1;
            s.rules.ai.arty.cb_alternate = 0;
            s.rules.ai.arty.buy_pending = 0;
            s.rules.ai.arty.light = 0;
            s.rules.items.rifle_grenades = 0;
            let light = s.tag("light_mortar");
            for t in s.squad_types.iter_mut() {
                if t.has_tag(light) {
                    t.ai_weight = 50;
                    t.initial_cooldown_ticks = 0;
                }
            }
        });
        let s = w.match_setup().unwrap().clone();
        let mut seen = std::collections::BTreeMap::new();
        let plans0 = counters(&w).plans;
        while counters(&w).plans < plans0 + 40 {
            w.step(&[]);
            note_call_ins(&w, &mut seen);
        }
        let new = seen
            .values()
            .filter(|&&t| s.type_has_tag(t, "field_howitzer") || s.type_has_tag(t, "light_mortar"))
            .count();
        let k = counters(&w);
        (
            new,
            k.cb_alt_holds + k.how_buys + k.how_pct_holds + k.how_mp_holds,
            seen.len(),
        )
    };
    let (new, s11b, sent) = run(0);
    assert_eq!((new, s11b), (0, 0), "nothing new");
    assert!(sent > 0, "the AI did call in squads");
    assert!(run(1).0 > 0, "control: how 1 calls in a howitzer");
}

/// Determinism (spec 5 "Howitzer buy"): the buy scene runs the same on 1
/// and 4 threads, and saves taken before and after the howitzer call-in
/// play on to the same hash. Control: `how` 0 differs.
#[test]
fn ai_s11b_deterministic_threads_and_save_load() {
    let go = |threads: usize, how: u32| {
        let mut w = how_world(threads, how, 5000, |s| {
            s.rules.ai.arty.buy = 1;
            s.rules.ai.arty.buy_pending = 1;
            s.rules.ai.arty.cb_alternate = 1;
        });
        let mut saves = Vec::new();
        for t in 0..30 * 120u64 {
            if t == 30 || t == 30 * 30 || t == 30 * 100 {
                saves.push((w.tick(), w.save()));
            }
            w.step(&[]);
        }
        (w.full_state_hash(), w.tick(), saves, counters(&w).how_buys)
    };
    let (h1, end, saves, buys) = go(1, 1);
    assert_eq!(buys, 1);
    let (h4, _, _, _) = go(4, 1);
    assert_eq!(h1, h4, "1 and 4 threads");
    for (t, bytes) in saves {
        let mut w = World::load(&bytes, 1).unwrap_or_else(|e| panic!("save at {t}: {e:?}"));
        while w.tick() < end {
            w.step(&[]);
        }
        assert_eq!(w.full_state_hash(), h1, "the save at tick {t}");
    }
    let (h0, _, _, _) = go(1, 0);
    assert_ne!(h0, h1, "control: how 0");
}

const KZ: &str = "ww2:ger_kz_grw42_team";
const HEAVY: &str = "ww2:ger_mortar_team";

/// L1: with `light` 1 and the shipped weights (light 50, the heavy card's
/// default 100) the mix buys the kz. GrW 42 among its mortar buys (every
/// other card weight 0, the cards off cooldown), and an own light team
/// fires on an enemy squad its leader sees 200 m away. Control `light` 0:
/// no light card is bought.
#[test]
fn ai_buys_a_light_mortar_and_fires_it() {
    let buys = |light: u32| {
        let mut w = how_world(1, 0, 5000, |s| {
            s.rules.ai.arty.light = light;
            for t in s.squad_types.iter_mut() {
                if t.id == KZ || t.id == HEAVY {
                    t.ai_weight = if t.id == KZ { 50 } else { 100 };
                    t.initial_cooldown_ticks = 0;
                    t.recall_cooldown_ticks = 0;
                }
            }
        });
        steps(&mut w, 30 * 120);
        let s = w.match_setup().unwrap();
        let k = counters(&w);
        let n = |id: &str| {
            k.bought
                .get(s.squad_type_index(id).unwrap() as usize)
                .copied()
        };
        (n(KZ).unwrap_or(0), n(HEAVY).unwrap_or(0))
    };
    let (light, heavy) = buys(1);
    assert!(light >= 1 && heavy >= light, "light {light}, heavy {heavy}");
    assert_eq!(buys(0).0, 0, "control: light 0");
    // Fires: the mortar habit drives the light team like a heavy one.
    let mut w = how_world(1, 0, 0, |s| s.rules.ai.arty.light = 1);
    steps(&mut w, 2);
    let q = spawn(&mut w, 0, KZ, 300, 500);
    let e = spawn(&mut w, 1, "ww2:usa_rifle_squad", 500, 500);
    for m in members(&w, e) {
        w.edit_soldier(m, |k, c| {
            k.rounds = [0; frontline_content::setup::MAX_FEEDS];
            c.mag = 0;
        });
    }
    steps(&mut w, 30 * 60);
    let fired = w.game().unwrap().players[0].ammo.fired[crate::supply::fired_class::MORTAR];
    assert!(fired > 0, "the light team fired ({q:?})");
}

/// H3 (G4: the base does not refill rifle grenades): a rifle squad whose
/// grenadier has spent his rifle grenades, with full rounds, makes no
/// refill trip (the AI's ammunition share reads rounds, not items: no code
/// was needed). Control: the same squad with its rounds spent goes.
#[test]
fn ai_does_not_send_a_squad_to_refill_rifle_grenades() {
    let run = |spend_rounds: bool| {
        let mut w = how_world(1, 0, 5000, |s| {
            super::s11b_tests::arm(s, RIFLES, "rifleman", 3)
        });
        steps(&mut w, 2);
        let q = spawn(&mut w, 0, RIFLES, 500, 500);
        let k = frontline_content::ItemKind::RifleGrenade.index();
        let mut had = 0u32;
        for m in members(&w, q) {
            w.edit_soldier(m, |kit, c| {
                had += kit.items[k] as u32;
                kit.items[k] = 0;
                if spend_rounds {
                    kit.rounds = [0; frontline_content::setup::MAX_FEEDS];
                    c.mag = 0;
                }
            });
        }
        assert!(had > 0, "the grenadier carried rifle grenades");
        steps(&mut w, 30 * 30);
        let refill = ai(&w)
            .tasks
            .get(&q)
            .is_some_and(|t| t.kind == crate::ai::TaskKind::Refill);
        (refill, counters(&w).refill_trips)
    };
    assert_eq!(run(false), (false, 0), "no trip for rifle grenades");
    assert!(run(true).1 > 0, "control: rounds spent: a trip");
}
