//! S15-ai Build D tests (docs/slices/S15-ussr.md 7, A1, A2 and liveness):
//! the rider job (`ai::riders`) and the AI's card table on the USSR roster.
//! Player 1 is a USSR AI that buys nothing (every `ai_weight` 0); player 0
//! (Germany) holds the one point and has no men there. `[riders] enabled`
//! and `[ai.riders] jobs` are 1 in the test only. Each test names its
//! control.

use frontline_content::MatchSetup;
use frontline_content::setup::{Capability, member_trait};
use frontline_content::vehicle::VehicleRole;

use super::s6_tests::{members, run, spawn, steps};
use super::*;
use crate::command::CommandKind;
use crate::s15::{RIDER_BOARD, RIDER_DROP, RIDER_END_NAMES, RIDER_RIDE, RiderJob};

const T34: &str = "ww2:ussr_t34_85_squad";
/// The six-man rifle section (tag `line`; the SMG section, tag
/// `vehicle_riders`, has nine men and does not fit the T-34-85's eight
/// deck slots at full strength).
const SMG: &str = "ww2:ussr_strelkovoe_otdelenie_6";

fn end(name: &str) -> usize {
    RIDER_END_NAMES.iter().position(|n| *n == name).unwrap()
}

/// The real defs with the USSR enabled against Germany (the shipped
/// switches of S15-ai) and every S15 switch on; `edit` changes the setup.
pub(super) fn ussr_setup(edit: impl FnOnce(&mut MatchSetup)) -> MatchSetup {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2");
    let mut defs = frontline_content::ModDefs::load(&root).unwrap();
    for r in defs.rules.values_mut() {
        r.matchups = Some(frontline_content::MatchupsDef {
            factions: ["ww2:germany", "ww2:usa", "ww2:ussr"]
                .map(String::from)
                .into(),
            pairs: vec![
                ["ww2:germany", "ww2:usa"].map(String::from),
                ["ww2:germany", "ww2:ussr"].map(String::from),
            ],
            free: 0,
            mirror: 0,
        });
        r.riders.get_or_insert_with(Default::default).enabled = 1;
        r.body_armour.get_or_insert_with(Default::default).enabled = 1;
        r.melee.get_or_insert_with(Default::default).bayonets = 1;
        if let Some(x) = r.supply.as_mut() {
            x.foreign_ammo = 1;
        }
    }
    let req = frontline_content::MatchRequest::skirmish(
        1,
        "ww2:germany",
        "ww2:ussr",
        frontline_content::Difficulty::Normal,
    );
    let mut s = defs.resolve(&req).unwrap();
    edit(&mut s);
    s
}

/// The rider scene: one point held by player 0, a T-34-85 and the
/// six-man section of player 1 (the AI) in its base, the squad 20 m beside the
/// tank. Returns (world, tank, squad).
fn rider_world(threads: usize, edit: impl FnOnce(&mut MatchSetup)) -> (World, SimId, SquadId) {
    let s = ussr_setup(|s| {
        s.players.iter_mut().for_each(|p| p.is_ai = false);
        s.players[1].is_ai = true;
        s.map.terrain.octaves.clear();
        s.map.terrain.bands.clear();
        s.map.woods.clear();
        s.s11b_off();
        s.s12_off();
        s.s13_off();
        s.s14_off();
        super::s10a_ai_tests::s10b_ai_off(s);
        s.map.points.truncate(1);
        s.factions.iter_mut().for_each(|f| f.start_squads.clear());
        s.squad_types.iter_mut().for_each(|t| t.ai_weight = 0);
        s.rules.ai.riders.jobs = 1;
        edit(s);
    });
    let mut w = World::new_match(s, threads).unwrap();
    w.state.game.as_mut().unwrap().points[0].owner = Some(PlayerId(0));
    let base = w.game().unwrap().base_of(PlayerId(1));
    let (x, y) = (base.x.floor_int(), base.y.floor_int());
    let t = spawn(&mut w, 1, T34, x, y);
    let tank = members(&w, t)[0];
    let sq = spawn(&mut w, 1, SMG, x + 20, y);
    (w, tank, sq)
}

fn ai_of(w: &World) -> &crate::ai::AiState {
    w.game().unwrap().ai[1].as_ref().unwrap()
}

fn job(w: &World) -> Option<RiderJob> {
    ai_of(w).riders.jobs.first().copied()
}

fn ends(w: &World) -> [u32; 5] {
    ai_of(w).counters.rider_job_ends
}

fn hz(w: &World) -> u64 {
    w.match_setup().unwrap().tick_hz as u64
}

/// Steps until `f` holds, at most `secs`; whether it did.
fn until(w: &mut World, secs: u64, mut f: impl FnMut(&World) -> bool) -> bool {
    for _ in 0..secs * hz(w) {
        if f(w) {
            return true;
        }
        steps(w, 1);
    }
    f(w)
}

fn point_dist_m(w: &World, id: SimId) -> i32 {
    let g = w.game().unwrap();
    let p: FxVec2 = g.setup.map.points[0].pos.into();
    (w.state.entities.transform[id.index()].pos.xy() - p)
        .length()
        .floor_int()
}

/// A1: the AI puts the six-man section on the T-34-85's deck in its base, the
/// tank drives to the attack and the deck is told off short of the point
/// (within `drop_m` and a plan's drive beyond it, never at the point); the
/// job ends as dropped and the squad is free again. Control: with `[ai.riders]
/// jobs` 0 nobody boards.
#[test]
fn rider_job_drops_short() {
    let (mut w, tank, sq) = rider_world(1, |_| {});
    let n = members(&w, sq).len();
    assert!(until(&mut w, 60, |w| job(w).is_some()), "a job starts");
    assert_eq!(job(&w).unwrap().squad, sq);
    assert!(
        until(&mut w, 60, |w| w.game().unwrap().riders_of(tank).len() == n),
        "every man aboard"
    );
    assert!(until(&mut w, 30, |w| job(w)
        .is_some_and(|j| j.phase == RIDER_RIDE)));
    assert!(
        until(&mut w, 300, |w| job(w)
            .is_none_or(|j| j.phase == RIDER_DROP)),
        "the deck is told off"
    );
    let at = point_dist_m(&w, tank);
    let drop_m = w.match_setup().unwrap().rules.ai.riders.drop_m as i32;
    assert!(until(&mut w, 30, |w| job(w).is_none()), "the job ends");
    assert_eq!(ends(&w)[end("dropped")], 1, "{:?}", ends(&w));
    assert_eq!(ai_of(&w).counters.rider_jobs, 1);
    assert!(
        at <= drop_m + 40 && at >= 40,
        "told off {at} m from the point"
    );
    let g = w.game().unwrap();
    assert!(g.riders_of(tank).is_empty());
    assert_eq!(g.players[1].s15.rider_boards, n as u32);
    assert!(g.players[1].s15.rider_exits[0] >= n as u32, "ordered off");
    assert_ne!(g.squads[&sq].order, (Order::Embark { vehicle: tank }));

    let (mut w, tank, _) = rider_world(1, |s| s.rules.ai.riders.jobs = 0);
    let n = 60 * hz(&w);
    steps(&mut w, n);
    assert!(job(&w).is_none() && w.game().unwrap().riders_of(tank).is_empty());
    assert_eq!(ai_of(&w).counters.rider_jobs, 0, "control: jobs 0");
}

/// Liveness: the tank wrecked while it rides ends the job as tank lost.
/// Control: the same scene unwrecked ends it as dropped (above).
#[test]
fn rider_job_ends_when_tank_dies() {
    let (mut w, tank, _) = rider_world(1, |_| {});
    assert!(until(&mut w, 120, |w| job(w)
        .is_some_and(|j| j.phase == RIDER_RIDE)));
    assert!(w.wreck_vehicle(tank));
    assert!(until(&mut w, 10, |w| job(w).is_none()), "the job ends");
    assert_eq!(ends(&w)[end("tank_lost")], 1, "{:?}", ends(&w));
}

/// A wreck over `[vehicles] wreck_cap` is removed between two plans: a
/// save then holds a job whose tank is gone. It loads (the load check
/// takes a missing tank as the next plan's to end), and that plan ends the
/// job as tank lost. Control: before this fix the load refused the save.
#[test]
fn rider_job_with_a_removed_wreck_loads() {
    let (mut w, tank, _) = rider_world(1, |s| s.rules.vehicles.wreck_cap = 1);
    assert!(until(&mut w, 120, |w| job(w)
        .is_some_and(|j| j.phase == RIDER_RIDE)));
    // Just after a plan: the riders get off the wreck (Status stage) and a
    // second wreck removes the oldest (the tank), all before the next plan.
    let other = spawn(&mut w, 0, T34, 100, 100);
    let other = members(&w, other)[0];
    assert!(until(&mut w, 5, |w| ai_of(w).next_plan >= w.state.tick + 10));
    assert!(w.wreck_vehicle(tank));
    steps(&mut w, 3);
    assert!(w.game().unwrap().riders_of(tank).is_empty());
    assert!(w.wreck_vehicle(other));
    assert!(!w.game().unwrap().vehicles.contains_key(&tank), "removed");
    assert!(job(&w).is_some());
    let mut l = World::load(&w.save(), 1).unwrap();
    assert!(until(&mut l, 10, |w| job(w).is_none()), "the job ends");
    assert_eq!(ends(&l)[end("tank_lost")], 1, "{:?}", ends(&l));
}

/// Liveness L1: `job_max_s` 8 ends the job as a deadline and the deck is
/// told off; no job lives past `job_max_s` and one plan. Control: the
/// default 240 s drops the deck (above).
#[test]
fn rider_job_ends_on_deadline() {
    let (mut w, tank, _) = rider_world(1, |s| s.rules.ai.riders.job_max_s = 8);
    assert!(until(&mut w, 60, |w| job(w).is_some()));
    let start = w.state.tick;
    let lim = 8 * hz(&w) + 5 * hz(&w);
    assert!(until(&mut w, 20, |w| job(w).is_none()), "the job ends");
    assert!(
        w.state.tick - start <= lim,
        "{} ticks",
        w.state.tick - start
    );
    assert_eq!(ends(&w)[end("deadline")], 1, "{:?}", ends(&w));
    assert!(
        until(&mut w, 10, |w| w.game().unwrap().riders_of(tank).is_empty()),
        "the deck is empty"
    );
}

/// Liveness L2, L3: the squad ordered away while it walks to the tank
/// (nobody aboard, no longer boarding) ends the job as a stall; the pair
/// is refused for `recheck_s` (no new job with it), then tried again.
/// Control: left alone it boards (above).
#[test]
fn rider_job_ends_on_stall() {
    let (mut w, _, sq) = rider_world(1, |s| s.rules.ai.riders.recheck_s = 40);
    assert!(until(&mut w, 60, |w| job(w)
        .is_some_and(|j| j.phase == RIDER_BOARD)));
    let tank = job(&w).unwrap().tank;
    assert!(until(&mut w, 5, |w| w.game().unwrap().squads[&sq].order
        == (Order::Embark { vehicle: tank })));
    run(&mut w, 1, 9000, CommandKind::Stop { squad: sq });
    assert!(until(&mut w, 20, |w| job(w).is_none()), "the job ends");
    assert_eq!(ends(&w)[end("stall")], 1, "{:?}", ends(&w));
    assert_eq!(ai_of(&w).riders.refused.len(), 1);
    // Keep both in the base (the AI would send them to the point): the
    // pair is free again `recheck_s` after the stall.
    let stalled = w.state.tick;
    let base = w.game().unwrap().base_of(PlayerId(1));
    let tank_sq = w.state.entities.squad[tank.index()].unwrap();
    let mut seq = 9001;
    let mut again = None;
    for _ in 0..16 {
        for (q, dx) in [(sq, 20), (tank_sq, 0)] {
            run(
                &mut w,
                1,
                seq,
                CommandKind::Move {
                    squad: q,
                    target: base + FxVec2::from_int(dx, 0),
                },
            );
            seq += 1;
        }
        let n = 5 * hz(&w);
        steps(&mut w, n);
        if ai_of(&w).counters.rider_jobs == 2 {
            again = Some(w.state.tick - stalled);
            break;
        }
    }
    let recheck = 40 * hz(&w);
    let again = again.expect("tried again");
    assert!(again >= recheck, "L3: not before recheck_s ({again} ticks)");
}

/// Determinism: the rider scene hashes the same at 1 and 4 threads, and
/// through a save and load in each phase.
#[test]
fn rider_job_is_deterministic_across_threads_and_save_load() {
    let n = 90 * 20;
    let mut hashes = Vec::new();
    for threads in [1, 4] {
        let (mut w, _, _) = rider_world(threads, |_| {});
        steps(&mut w, n);
        hashes.push(w.state_hash());
    }
    assert_eq!(hashes[0], hashes[1]);
    for phase in [RIDER_BOARD, RIDER_RIDE, RIDER_DROP] {
        let (mut w, _, _) = rider_world(2, |_| {});
        assert!(
            until(&mut w, 300, |w| job(w).is_some_and(|j| j.phase == phase)),
            "{phase}"
        );
        let mut l = World::load(&w.save(), 3).unwrap();
        assert_eq!(l.state(), w.state());
        steps(&mut w, 300);
        steps(&mut l, 300);
        assert_eq!(l.state_hash(), w.state_hash(), "phase {phase}");
    }
}

/// A2 (plan 7): every AI habit finds its USSR card by the tag, capability
/// or role it already uses (the habits' own pick rules, read on the
/// resolved setup): mix slots, builder, HQ, bearers, medic, observer,
/// infantry gun then howitzer, heavy, flame, recon, truck and tow,
/// transport, AT gun. Control: the German roster finds its own cards by
/// the same rules (none of them a USSR card).
#[test]
fn ai_habits_find_ussr_cards() {
    let s = ussr_setup(|_| {});
    let fac = |id: &str| s.factions.iter().position(|f| f.id == id).unwrap();
    let id = |t: u16| s.squad_types[t as usize].id.as_str();
    for (f, prefix) in [("ww2:ussr", "ww2:ussr_"), ("ww2:germany", "ww2:ger_")] {
        let roster = &s.factions[fac(f)].roster;
        let bought = |t: u16| s.squad_types[t as usize].ai_weight > 0;
        let role = |t: u16, r: VehicleRole| {
            s.squad_types[t as usize]
                .vehicle
                .is_some_and(|v| s.vehicles[v as usize].role == r)
        };
        let first = |f: &dyn Fn(u16) -> bool| roster.iter().copied().find(|&t| f(t));
        let mut table: Vec<(&str, Option<u16>)> = Vec::new();
        // Mix slots (`ai::mix_slot`): every slot has a card the AI buys.
        for (k, m) in s.rules.ai.mix.iter().enumerate() {
            let name = s.tags[m.tag as usize].as_str();
            let card = first(&|t| bought(t) && crate::ai::mix_slot(&s, t) == Some(k));
            table.push((name, card));
        }
        // Builder floor (`builder_floor_card`): the cheapest builder.
        let builder = roster
            .iter()
            .copied()
            .filter(|&t| {
                let ty = &s.squad_types[t as usize];
                bought(t)
                    && ty.has(Capability::PlaceMines)
                    && ty.has(Capability::BuildStructure)
                    && !ty.has(Capability::Breach)
            })
            .min_by_key(|&t| s.squad_types[t as usize].cost);
        table.push(("builder", builder));
        table.push(("hq", first(&|t| s.type_has_tag(t, "hq"))));
        table.push(("bearers", first(&|t| s.type_has_tag(t, "bearers"))));
        table.push(("medic", first(&|t| bought(t) && s.type_has_tag(t, "medic"))));
        let fo = |t: u16| {
            let ty = &s.squad_types[t as usize];
            ty.vehicle.is_none()
                && ty
                    .members
                    .iter()
                    .any(|m| m.traits & member_trait::OBSERVER != 0)
                && ty
                    .members
                    .iter()
                    .any(|m| m.traits & member_trait::RADIO != 0)
        };
        table.push(("observer", first(&fo)));
        let how = |t: u16| s.type_has_tag(t, "field_howitzer");
        table.push((
            "inf_gun",
            first(&|t| bought(t) && !how(t) && role(t, VehicleRole::InfGun)),
        ));
        table.push((
            "howitzer",
            first(&|t| bought(t) && how(t) && role(t, VehicleRole::InfGun)),
        ));
        table.push(("heavy", first(&|t| s.squad_types[t as usize].ai_heavy)));
        table.push((
            "flame",
            first(&|t| bought(t) && s.squad_types[t as usize].has(Capability::Flame)),
        ));
        table.push((
            "recon",
            first(&|t| bought(t) && role(t, VehicleRole::Recon)),
        ));
        table.push((
            "truck",
            first(&|t| bought(t) && role(t, VehicleRole::Truck)),
        ));
        table.push((
            "transport",
            first(&|t| bought(t) && role(t, VehicleRole::Transport)),
        ));
        table.push((
            "at_gun",
            first(&|t| bought(t) && role(t, VehicleRole::AtGun)),
        ));
        for (habit, card) in &table {
            let card = card.unwrap_or_else(|| panic!("{f}: no card for {habit}"));
            assert!(id(card).starts_with(prefix), "{f} {habit}: {}", id(card));
        }
        // Every USSR gun card is towed by the faction's truck (S15 Build C;
        // some German guns are towed by a tractor or not at all).
        let truck = table.iter().find(|h| h.0 == "truck").unwrap().1;
        if f != "ww2:ussr" {
            continue;
        }
        for &t in roster
            .iter()
            .filter(|&&t| role(t, VehicleRole::AtGun) || role(t, VehicleRole::InfGun))
        {
            let tow = s.squad_types[t as usize].tow_card;
            assert_eq!(tow, truck, "{}", id(t));
        }
        {
            let named: Vec<(&str, &str)> =
                table.iter().map(|(h, c)| (*h, id(c.unwrap()))).collect();
            let want = [
                ("builder", "ww2:ussr_sapyornoe_otdelenie"),
                ("hq", "ww2:ussr_upravlenie_vzvoda"),
                ("bearers", "ww2:ussr_podnoschiki"),
                ("medic", "ww2:ussr_sanitary"),
                ("observer", "ww2:ussr_razvedchiki"),
                ("inf_gun", "ww2:ussr_76mm_m1943_squad"),
                ("howitzer", "ww2:ussr_122mm_m30_squad"),
                ("heavy", "ww2:ussr_is2_squad"),
                ("flame", "ww2:ussr_shturmovaya_gruppa"),
                ("recon", "ww2:ussr_ba64b_squad"),
                ("truck", "ww2:ussr_us6_squad"),
                ("transport", "ww2:ussr_m3a1_scout_squad"),
                ("at_gun", "ww2:ussr_45mm_m42_squad"),
            ];
            for (h, c) in want {
                assert!(named.contains(&(h, c)), "{h}: {named:?}");
            }
        }
    }
}

/// Build C's finding, fixed (`spotting::pct_of`): a vehicle's sight times
/// its commander's percent saturated Q16.16 above about 327 m. A T-34-85
/// given a 500 m sight sees a still Panzer IV (size 100 %) on open flat
/// ground 450 m off; at 550 m it does not. Control: the old product caps
/// the 500 m sight at 327 m, so the 450 m tank was never seen. Shipped
/// sights are 150-220 m: below the cap nothing changes (T211).
#[test]
fn vehicle_sight_over_327_m_reaches_its_range() {
    use frontline_content::setup::Fixed;
    let sight_of = |d: i32| {
        let (mut w, tank, sq) = rider_world(1, |s| {
            s.rules.ai.riders.jobs = 0;
            for v in s.vehicles.iter_mut() {
                v.sight = Fixed::from_ratio(500, 1);
                v.spot_size_pct = 100;
                v.optic_sight_pct = 100;
                v.stationary_sight_pct = 100;
            }
            s.players[1].is_ai = false;
        });
        for m in members(&w, sq) {
            assert!(w.kill_soldier(m));
        }
        let at = w.state.entities.transform[tank.index()].pos.xy();
        let p = spawn(
            &mut w,
            0,
            "ww2:ger_pz4_h_squad",
            at.x.floor_int() - d,
            at.y.floor_int(),
        );
        let pz = members(&w, p)[0];
        until(&mut w, 10, |w| {
            w.state.entities.combat[pz.index()].seen_by & 0b10 != 0
        })
    };
    assert!(sight_of(450), "seen at 450 m");
    assert!(!sight_of(550), "not seen at 550 m");
    let x = Fx::from_int(500);
    assert_eq!(crate::spotting::pct_of(x, 100), x);
    assert_eq!(
        crate::spotting::pct_of(Fx::from_int(300), 70),
        Fx::from_int(210)
    );
    assert_eq!(
        Fx::from_int(500).mul_int(100).div_int(100).floor_int(),
        327,
        "the old cap"
    );
}
