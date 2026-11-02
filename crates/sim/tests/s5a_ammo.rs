//! S5a Part A: counted rounds, crew feed, sharing, the sidearm, the priced
//! base refill and tank stowage (docs/slices/S5a-ammo.md targets T5, T6,
//! T7, T12, and the request-count rule for burst bundles).

use std::path::Path;

use frontline_content::{Difficulty, MatchRequest, MatchSetup, ModDefs};
use frontline_sim::supply::{expected_fire_ticks, fired_class};
use frontline_sim::{Command, CommandKind, Fx, FxVec2, PlayerId, SimId, SquadId, World};

/// Flat open ground with the test unlock, both sides human, plenty of
/// manpower; `edit` changes the setup further.
fn setup_with(edit: impl FnOnce(&mut MatchSetup)) -> MatchSetup {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2");
    let mut defs = ModDefs::load(&root).unwrap_or_else(|e| panic!("{e}"));
    let rules = defs
        .add_rules_override(&root.join("tests/rules_test_unlock.toml"))
        .unwrap_or_else(|e| panic!("{e}"));
    let mut req = MatchRequest::skirmish(1, "ww2:germany", "ww2:usa", Difficulty::Normal);
    req.rules = rules;
    let mut s = defs.resolve(&req).unwrap_or_else(|e| panic!("{e}"));
    s.players.iter_mut().for_each(|p| p.is_ai = false);
    s.rules.start_manpower = 100_000;
    s.map.terrain.octaves.clear();
    s.map.terrain.bands.clear();
    s.map.woods.clear();
    s.rules.vehicles.bail.crew_below = 0;
    // No income: manpower moves only by what ammunition costs.
    s.rules.income_per_min = 0;
    s.rules.point_income_per_min = 0;
    edit(&mut s);
    s
}

fn xy(x: i32, y: i32) -> FxVec2 {
    FxVec2::from_int(x, y)
}

const EAST: FxVec2 = FxVec2::new(Fx::ONE, Fx::ZERO);

fn spawn(w: &mut World, player: u8, id: &str, at: FxVec2, f: FxVec2) -> SquadId {
    let t = w.match_setup().unwrap().squad_type_index(id).unwrap();
    w.spawn_squad_at(PlayerId(player), t, at, f).unwrap()
}

fn members(w: &World, sq: SquadId) -> Vec<SimId> {
    w.game().unwrap().squad(sq).unwrap().members.clone()
}

fn fired(w: &World, p: usize, class: usize) -> u32 {
    w.game().unwrap().players()[p].ammo.fired[class]
}

fn base(w: &World, p: u8) -> FxVec2 {
    w.game().unwrap().base_of(PlayerId(p))
}

fn feed_slot(w: &World, sq: SquadId, cartridge: &str) -> usize {
    let s = w.match_setup().unwrap();
    let ty = &s.squad_types[w.game().unwrap().squad(sq).unwrap().type_idx as usize];
    let c = s.cartridges.iter().position(|c| c.id == cartridge).unwrap() as u16;
    ty.feeds
        .iter()
        .position(|f| *f == frontline_content::setup::Feed::Cartridge(c))
        .unwrap()
}

/// Rounds of feed slot `k` the squad holds (pouches, and magazines of
/// weapons that fire that feed).
fn squad_rounds(w: &World, sq: SquadId, k: usize) -> u32 {
    let s = w.match_setup().unwrap();
    let ty = &s.squad_types[w.game().unwrap().squad(sq).unwrap().type_idx as usize];
    members(w, sq)
        .iter()
        .map(|m| {
            let kit = &w.entities().kit()[m.index()];
            let wi = if kit.on_sidearm {
                ty.members[kit.slot as usize].sidearm.unwrap()
            } else {
                kit.weapon
            };
            let wd = &s.weapons[wi as usize];
            let in_gun = ty.feed_slot(wd) == Some(k) && wd.he.is_none();
            kit.rounds[k] as u32
                + if in_gun {
                    w.entities().combat()[m.index()].mag as u32
                } else {
                    0
                }
        })
        .sum()
}

/// µMP the squad's missing small-arms rounds cost at the rules' price.
fn missing_small_arms_micro(w: &World, sq: SquadId) -> u64 {
    let s = w.match_setup().unwrap();
    let sp = s.rules.supply.as_ref().unwrap();
    let ty = &s.squad_types[w.game().unwrap().squad(sq).unwrap().type_idx as usize];
    let mut out = 0;
    for k in 0..ty.feeds.len() {
        let frontline_content::setup::Feed::Cartridge(c) = ty.feeds[k] else {
            continue;
        };
        let c = &s.cartridges[c as usize];
        if c.kind != frontline_content::SupplyKind::SmallArms {
            continue;
        }
        let load: u32 = members(w, sq)
            .iter()
            .map(|m| ty.members[w.entities().kit()[m.index()].slot as usize].rounds[k] as u32)
            .sum();
        let have = squad_rounds(w, sq, k);
        out += (load - have) as u64 * c.g as u64 * sp.small_arms_mp_per_t as u64;
    }
    out
}

fn suppress(w: &mut World, seq: u32, units: Vec<SimId>, at: FxVec2) {
    w.step(&[Command {
        player: PlayerId(0),
        seq,
        kind: CommandKind::SuppressUnits {
            units,
            line: vec![at],
        },
    }]);
}

// ------------------------------------------------------------------ T5 --

/// T5: an MG team firing steadily goes dry in the time its rounds, rate
/// and drum changes give (±3 %), the assistant and bearer feeding the gun;
/// in base it refills paying each round's weight at the small-arms price;
/// with no manpower the refill pauses and resumes when manpower comes.
#[test]
fn mg_team_goes_dry_on_time_and_refills_in_base_for_manpower() {
    // Crew feed reaches the bearer too (the formation puts him 6 m from
    // the gun), so no sharing pass delays a drum change.
    let mut w = World::new_match(setup_with(|s| s.rules.combat.share.crew_feed_m = 10), 1).unwrap();
    let sq = spawn(&mut w, 0, "ww2:ger_grenadiergruppe", xy(600, 500), EAST);
    let men = members(&w, sq);
    let gunner = men[1];
    let belt = feed_slot(&w, sq, "ww2:792x57_belt");
    let total = squad_rounds(&w, sq, belt);
    assert_eq!(total, 50 + 500 + 600, "the team's belts");
    for _ in 0..40 {
        w.step(&[]);
    }
    suppress(&mut w, 1, vec![gunner], xy(800, 500));
    let mut first = None;
    let mut last = None;
    for _ in 0..8000 {
        w.step(&[]);
        let n = fired(&w, 0, fired_class::MG);
        if n > 0 && first.is_none() {
            first = Some(w.tick());
        }
        if n == total && last.is_none() {
            last = Some(w.tick());
        }
    }
    assert_eq!(
        fired(&w, 0, fired_class::MG),
        total,
        "every belt round fired, no more"
    );
    assert_eq!(squad_rounds(&w, sq, belt), 0, "the team is dry");
    let s = w.match_setup().unwrap();
    let mg = s.weapons.iter().find(|x| x.id == "ww2:mg42_bipod").unwrap();
    let want = expected_fire_ticks(mg, total) as f64;
    let got = (last.unwrap() - first.unwrap()) as f64;
    assert!(
        (got - want).abs() <= want * 0.03,
        "dry after {got} ticks, expected {want}"
    );
    // The gunner draws his P 38 when the team is dry.
    assert!(w.entities().kit()[gunner.index()].on_sidearm);

    // Refill in base: paid round by round at the small-arms price.
    let owed = missing_small_arms_micro(&w, sq);
    let home = base(&w, 0);
    assert!(w.place_squad(sq, home, EAST));
    w.edit_player(PlayerId(0), |p| p.manpower = 0);
    let paid0 = w.game().unwrap().players()[0].ammo.paid[0];
    let have0 = squad_rounds(&w, sq, belt);
    for _ in 0..120 {
        w.step(&[]);
    }
    let pl = &w.game().unwrap().players()[0];
    assert!(pl.ammo.paused_ticks > 0, "no manpower: the refill pauses");
    assert!(
        squad_rounds(&w, sq, belt) <= have0 + 26,
        "at most the prepaid credit's rounds go in without manpower"
    );
    w.edit_player(PlayerId(0), |p| p.manpower = 1_000);
    let (c0, p0) = {
        let a = &w.game().unwrap().players()[0].ammo;
        (a.credit as u64, a.paid.iter().sum::<u64>())
    };
    for _ in 0..1500 {
        w.step(&[]);
    }
    assert_eq!(squad_rounds(&w, sq, belt), total, "refilled");
    let pl = &w.game().unwrap().players()[0];
    let paid = pl.ammo.paid[0] - paid0;
    assert_eq!(
        paid, owed,
        "each round paid at its weight x small_arms_mp_per_t"
    );
    // 1,150 belt rounds at 38 g and 0.2 MP per kg: 8.74 MP.
    assert!(paid >= 8_740_000, "{paid}");
    // Manpower went only into ammunition: manpower x 1e6 + credit + paid
    // stays constant (prepaid credit, no debt).
    let (c1, p1) = (pl.ammo.credit as u64, pl.ammo.paid.iter().sum::<u64>());
    assert_eq!(
        (1_000 - pl.manpower) as u64 * 1_000_000 + c0,
        c1 + (p1 - p0)
    );
}

// ------------------------------------------------------------------ T6 --

/// T6: a Panzer IV rearming in base pays each main-gun round's weight x
/// shell_mp_per_t x its rack factor (53 %: the full rack within 75 % of
/// its card) and each MG round at the small-arms price; standard rounds
/// come one per 2 s, the rare PzGr 40 one per 10 s and never above
/// rare_cap.
#[test]
fn tank_rearms_in_base_round_by_round_and_rare_rounds_stay_under_the_cap() {
    let mut w = World::new_match(setup_with(|_| {}), 1).unwrap();
    let home = base(&w, 0);
    let sq = spawn(&mut w, 0, "ww2:ger_pz4_h_squad", home, EAST);
    let v = members(&w, sq)[0];
    let s = w.match_setup().unwrap().clone();
    let vt = &s.vehicles[w.game().unwrap().vehicle(v).unwrap().type_idx as usize];
    assert_eq!(vt.rack_price_pct, 53);
    assert_eq!(vt.rare_cap, 3);
    let gun = vt.main_gun().clone();
    let counts: Vec<u32> = gun.ammo.iter().map(|a| a.count as u32).collect();
    assert_eq!(counts, vec![42, 42, 3], "Panzer IV stowage 87");
    let full_rack: u64 = gun
        .ammo
        .iter()
        .map(|a| a.count as u64 * s.ammo[a.ammo as usize].supply.unwrap().g as u64 * 1000)
        .sum();
    assert!(
        full_rack * 53 / 100 <= 465_000_000,
        "rack within 75 % of the 620 card"
    );
    w.edit_vehicle(v, |vs| {
        vs.ammo.iter_mut().for_each(|n| *n = 0);
        vs.loaded = None;
        vs.mgs.iter_mut().for_each(|m| m.spent = 300);
    });
    let mp0 = w.game().unwrap().players()[0].manpower;
    let rare_k = gun.ammo.iter().position(|a| a.rare).unwrap();
    let held = |w: &World| {
        let vs = w.game().unwrap().vehicle(v).unwrap();
        (0..vs.ammo.len())
            .map(|k| vs.ammo[k] as u32 + (vs.loaded == Some(k as u8)) as u32)
            .collect::<Vec<u32>>()
    };
    let hz = s.tick_hz as u64;
    let mut max_rare = 0;
    for t in 1..=(200 * hz) {
        w.step(&[]);
        let h = held(&w);
        max_rare = max_rare.max(h[rare_k]);
        if t == 31 * hz {
            // One rare round per 10 s: 3 after 30 s; standard one per 2 s.
            assert_eq!(h[rare_k], 3);
            let std: u32 = h
                .iter()
                .enumerate()
                .filter(|(k, _)| *k != rare_k)
                .map(|(_, n)| n)
                .sum();
            assert!((14..=16).contains(&std), "{std} standard rounds after 31 s");
        }
    }
    assert!(max_rare <= vt.rare_cap);
    assert_eq!(held(&w), counts, "rack full");
    let pl = &w.game().unwrap().players()[0];
    let per_round =
        |k: usize| s.ammo[gun.ammo[k].ammo as usize].supply.unwrap().g as u64 * 1000 * 53 / 100;
    let shells: u64 = (0..counts.len())
        .map(|k| counts[k] as u64 * per_round(k))
        .sum();
    assert_eq!(
        pl.ammo.paid[2], shells,
        "main-gun rounds at shell price x 53 %"
    );
    assert_eq!(
        pl.ammo.paid[0],
        600 * 38 * 200,
        "MG belts at the small-arms price"
    );
    let spent = (mp0 - pl.manpower) as u64 * 1_000_000 - pl.ammo.credit as u64;
    assert_eq!(spent, pl.ammo.paid.iter().sum::<u64>());
}

// ------------------------------------------------------------------ T7 --

/// T7: a mortar with no resupply stops after the 15 bombs its team
/// carries, the barrage order ending; in base the bombs come back one per
/// `base_bomb_s` per man at the shells price.
#[test]
fn mortar_stops_after_its_carried_bombs_and_refills_at_the_shells_price() {
    let mut w = World::new_match(setup_with(|s| s.rules.combat.share.crew_feed_m = 10), 1).unwrap();
    let sq = spawn(&mut w, 0, "ww2:ger_mortar_team", xy(600, 500), EAST);
    let bombs = feed_slot(&w, sq, "ww2:8cm_wgr34");
    assert_eq!(squad_rounds(&w, sq, bombs), 15);
    let carried: u32 = members(&w, sq)
        .iter()
        .map(|m| w.entities().kit()[m.index()].rounds[bombs] as u32)
        .sum();
    assert_eq!(carried, 15);
    w.step(&[Command {
        player: PlayerId(0),
        seq: 1,
        kind: CommandKind::Barrage {
            squad: sq,
            target: xy(1000, 500),
            rounds: 50,
        },
    }]);
    for _ in 0..4000 {
        w.step(&[]);
    }
    assert_eq!(fired(&w, 0, fired_class::MORTAR), 15);
    assert!(matches!(
        w.game().unwrap().squad(sq).unwrap().order,
        frontline_sim::Order::Idle
    ));
    let home = base(&w, 0);
    assert!(w.place_squad(sq, home, EAST));
    let paid0 = w.game().unwrap().players()[0].ammo.paid[2];
    for _ in 0..(30 * 30) {
        w.step(&[]);
    }
    let back: u32 = members(&w, sq)
        .iter()
        .map(|m| w.entities().kit()[m.index()].rounds[bombs] as u32)
        .sum();
    assert_eq!(back, 15, "3 men x one per 4 s: all back within 30 s");
    let paid = w.game().unwrap().players()[0].ammo.paid[2] - paid0;
    assert_eq!(paid, 15 * 3500 * 1000);
}

// ----------------------------------------------------------------- T12 --

/// T12: a dry rifleman takes rounds from a squadmate in the sharing pass;
/// a dry gunner draws his pistol and goes back to the MG at his next
/// reload once a belt reached him.
#[test]
fn sharing_feeds_a_dry_rifleman_and_the_sidearm_switches_back() {
    let mut w = World::new_match(setup_with(|_| {}), 1).unwrap();
    let sq = spawn(&mut w, 0, "ww2:ger_grenadiergruppe", xy(600, 500), EAST);
    let men = members(&w, sq);
    let clip = feed_slot(&w, sq, "ww2:792x57_clip");
    let belt = feed_slot(&w, sq, "ww2:792x57_belt");
    let rifleman = men[9];
    w.edit_soldier(rifleman, |k, c| {
        k.rounds[clip] = 0;
        c.mag = 0;
    });
    // Exactly one sharing pass in any 5 s.
    for _ in 0..(5 * 30) {
        w.step(&[]);
    }
    let k = &w.entities().kit()[rifleman.index()];
    let c = &w.entities().combat()[rifleman.index()];
    // 20 % of a 60-round load: 12 rounds, a clip of them in the rifle.
    assert_eq!(k.rounds[clip] as u32 + c.mag as u32, 12, "shared");

    // The MG team dry: the gunner fires his pistol, then the MG again.
    let gunner = men[1];
    for &m in &men[1..4] {
        w.edit_soldier(m, |k, c| {
            k.rounds[belt] = 0;
            if m == gunner {
                c.mag = 0;
            }
        });
    }
    for _ in 0..(5 * 30) {
        w.step(&[]);
    }
    assert!(w.entities().kit()[gunner.index()].on_sidearm, "dry: pistol");
    let p0 = fired(&w, 0, fired_class::PISTOL);
    suppress(&mut w, 1, vec![gunner], xy(620, 500));
    for _ in 0..(10 * 30) {
        w.step(&[]);
    }
    assert!(fired(&w, 0, fired_class::PISTOL) > p0, "the pistol fires");
    w.edit_soldier(gunner, |k, _| k.rounds[belt] = 50);
    let mg0 = fired(&w, 0, fired_class::MG);
    let mut back = false;
    for _ in 0..(40 * 30) {
        w.step(&[]);
        if !w.entities().kit()[gunner.index()].on_sidearm {
            back = true;
            break;
        }
    }
    assert!(back, "back on the MG at the pistol's next reload");
    for _ in 0..(10 * 30) {
        w.step(&[]);
    }
    assert!(fired(&w, 0, fired_class::MG) > mg0, "the MG fires again");
}

/// Every requested round is spent and counted, also when the projectile
/// pool drops it (the rule any future burst bundle must keep: it
/// subtracts every round it stands for).
#[test]
fn rounds_spent_equal_requests_made_including_dropped_ones() {
    let mut w = World::new_match(
        setup_with(|s| {
            s.rules.projectile_cap = 8;
        }),
        1,
    )
    .unwrap();
    let a = spawn(&mut w, 0, "ww2:ger_grenadiergruppe", xy(600, 500), EAST);
    let b = spawn(&mut w, 1, "ww2:usa_rifle_squad", xy(700, 500), EAST);
    let clip = feed_slot(&w, a, "ww2:792x57_clip");
    let before = squad_rounds(&w, a, clip);
    for _ in 0..(20 * 30) {
        w.step(&[]);
    }
    let st = w.stats();
    assert!(st.shots_dropped > 0, "the small pool drops requests");
    let g = w.game().unwrap();
    let all: u64 = g
        .players()
        .iter()
        .map(|p| p.ammo.fired.iter().map(|&n| n as u64).sum::<u64>())
        .sum();
    // `shots_fired` counts every request, the dropped ones included.
    assert_eq!(all, st.shots_fired);
    let rifle = fired(&w, 0, fired_class::RIFLE);
    assert!(rifle > 0);
    let _ = b;
    // Kar98k rounds left: the load less every requested rifle round (the
    // squad's dead take theirs with them, so only while all live).
    if members(&w, a).len() == 10 {
        assert_eq!(squad_rounds(&w, a, clip), before - rifle);
    }
}
