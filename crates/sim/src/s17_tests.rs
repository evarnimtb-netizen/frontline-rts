//! S17a Build A tests (docs/slices/S17-team-play.md 4, 9 "S17a"): the team
//! table, hostility and shared sight, team capture, tickets and victory,
//! the economy keys, the AI's foe and ally points, the save layout, and the
//! first runs with a third and fourth player. Each test names its control.

use super::s6_tests::{members, spawn, steps};
use super::*;
use crate::game::{Outcome, SquadId};
use crate::rules::Defeat;
use crate::teams::Teams;
use frontline_content::setup::FixedVec2;

const GER: &str = "ww2:ger_grenadiergruppe";
const USA: &str = "ww2:usa_rifle_squad";

/// The ww2 mod's Kessel Ridge setup with `teams.len()` players (slot `i`
/// on team `teams[i]`; factions alternate Germany, USA) and four bases:
/// the map's two plus the other two corners (slot 2 at (96, 928), slot 3
/// at (928, 96)). `flat`: flat open ground; `ai`: every player an AI.
pub(super) fn team_setup(
    teams: &[u8],
    flat: bool,
    ai: bool,
    edit: impl FnOnce(&mut frontline_content::MatchSetup),
) -> frontline_content::MatchSetup {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2");
    let defs = frontline_content::ModDefs::load(&root).unwrap();
    let req = frontline_content::MatchRequest::skirmish(
        1,
        "ww2:germany",
        "ww2:usa",
        frontline_content::Difficulty::Normal,
    );
    let mut setup = defs.resolve(&req).unwrap();
    let base = setup.map.bases[0].clone();
    for at in [[96, 928], [928, 96]] {
        let mut b = base.clone();
        b.center = FixedVec2::from_m(at);
        setup.map.bases.push(b);
    }
    let proto = setup.players.clone();
    setup.players = (0..teams.len())
        .map(|i| {
            let mut p = proto[i % 2].clone();
            p.base = i as u8;
            p.team = teams[i];
            p.is_ai = ai;
            p
        })
        .collect();
    if flat {
        setup.map.terrain.octaves.clear();
        setup.map.terrain.bands.clear();
        setup.map.woods.clear();
        setup.s11b_off();
        setup.s12_off();
        setup.s13_off();
    }
    edit(&mut setup);
    setup
}

fn world(teams: &[u8], edit: impl FnOnce(&mut frontline_content::MatchSetup)) -> World {
    World::new_match(team_setup(teams, true, false, edit), 1).unwrap()
}

fn sq(w: &mut World, p: u8, x: i32, y: i32) -> SquadId {
    spawn(w, p, if p.is_multiple_of(2) { GER } else { USA }, x, y)
}

/// Kills all but `keep` men of squad `s`.
fn thin(w: &mut World, s: SquadId, keep: usize) {
    for m in members(w, s).into_iter().skip(keep) {
        assert!(w.kill_soldier(m));
    }
}

fn game(w: &World) -> &MatchState {
    w.game().unwrap()
}

fn game_mut(w: &mut World) -> &mut MatchState {
    w.state.game.as_mut().unwrap()
}

// ------------------------------------------------------------ the table --

/// Plan 9: with every player alone `allies[p] == 1 << p`; a resolved 1v1
/// is two teams of one (the default when no `team` is asked for).
/// Control: a 2v2 table has two bits.
#[test]
fn every_player_alone_is_one_bit_and_a_1v1_resolves_alone() {
    let w = super::s6_tests::flat_world(1);
    let t = *game(&w).teams();
    assert_eq!(t, Teams::alone(2));
    for p in 0..2u8 {
        assert_eq!(t.allies_mask(p), 1 << p);
        assert_eq!(game(&w).setup().players[p as usize].team, p);
    }
    let w = world(&[0, 1, 0, 1], |_| {});
    assert_eq!(game(&w).teams().allies_mask(0), 0b0101);
    assert_eq!(game(&w).teams().allies_mask(3), 0b1010);
}

/// The table is derived: a save does not carry it, the load rebuilds it,
/// and the setup's team field is hashed. Control: an edited team number
/// changes the hash.
#[test]
fn the_team_table_is_rebuilt_at_load_and_the_team_is_hashed() {
    let mut w = world(&[0, 1, 0, 1], |_| {});
    steps(&mut w, 5);
    let bytes = w.save();
    let back = World::load(&bytes, 1).unwrap();
    assert_eq!(game(&back).teams(), game(&w).teams());
    assert_eq!(back.full_state_hash(), w.full_state_hash());
    let other = World::new_match(team_setup(&[0, 1, 1, 0], true, false, |_| {}), 1).unwrap();
    let w0 = World::new_match(team_setup(&[0, 1, 0, 1], true, false, |_| {}), 1).unwrap();
    assert_ne!(other.full_state_hash(), w0.full_state_hash());
}

/// Plan 9: a one-team setup is refused. Control: two teams load.
#[test]
fn a_one_team_setup_is_refused() {
    let s = team_setup(&[0, 0, 0, 0], true, false, |_| {});
    assert!(World::new_match(s, 1).is_err());
    let s = team_setup(&[0, 0, 1, 1], true, false, |_| {});
    assert!(World::new_match(s, 1).is_ok());
}

// ------------------------------------------------------- T1 to T3: fire --

/// Plan 9: an ally is never a target (rifles and MGs of both allies stand
/// next to each other for a minute); the control is an enemy squad put in
/// view afterwards, which is targeted.
#[test]
fn an_ally_is_never_a_target() {
    let mut w = world(&[0, 1, 0, 1], |_| {});
    let a = sq(&mut w, 0, 500, 500);
    let b = sq(&mut w, 2, 520, 500);
    steps(&mut w, 60 * 30);
    let targets = |w: &World| -> Vec<(SimId, SimId)> {
        let e = w.entities();
        let mut out = Vec::new();
        for s in [a, b] {
            for m in members(w, s) {
                if let Some(t) = e.combat[m.index()].target {
                    out.push((m, t));
                }
            }
        }
        out
    };
    assert!(targets(&w).is_empty(), "allies aim at each other");
    assert_eq!(game(&w).players[0].kills + game(&w).players[2].kills, 0);
    // Control: an enemy squad near them is engaged.
    sq(&mut w, 1, 560, 500);
    steps(&mut w, 10 * 30);
    let t = targets(&w);
    assert!(!t.is_empty(), "control: the enemy is targeted");
    for (_, tgt) in t {
        let o = w.entities().owner[tgt.index()].0;
        assert_eq!(o, 1, "only the enemy is a target");
    }
}

/// Plan 9 T2: a man seen by my ally is a target for me with
/// `shared_sight` 1 and not with 0. Control: the sight masks.
#[test]
fn shared_sight_makes_my_allys_spot_mine() {
    for shared in [1, 0] {
        let w = world(&[0, 1, 0, 1], |s| s.rules.teams.shared_sight = shared);
        let t = game(&w).teams();
        let seen_by_ally = 1u16 << 2;
        assert_eq!(t.sees(0, seen_by_ally), shared == 1);
        assert_eq!(t.enemy_seen(0, 1, seen_by_ally), shared == 1);
        assert!(
            !t.enemy_seen(0, 2, seen_by_ally),
            "an ally is never a target"
        );
        assert!(t.sees(0, 1), "my own spots always count");
    }
}

/// T3: an ally's men are spared by direct fire as own men are. A squad of
/// player 0 fires on an enemy squad with an ally's squad in the line for a
/// minute; every kill of team 0 is a loss of team 1 (a man of player 2
/// hit by player 0 would break it). Control: the enemy took losses.
#[test]
fn direct_fire_passes_an_ally() {
    let mut w = world(&[0, 1, 0, 1], |_| {});
    sq(&mut w, 0, 400, 500);
    sq(&mut w, 2, 430, 500);
    sq(&mut w, 1, 520, 500);
    steps(&mut w, 60 * 30);
    let p = &game(&w).players;
    assert!(p[1].losses > 0, "control: the enemy took losses");
    assert_eq!(p[0].kills + p[2].kills, p[1].losses + p[3].losses);
    assert_eq!(p[1].kills + p[3].kills, p[0].losses + p[2].losses);
}

/// Checker after A (finding 1; T3, T8): an ally's HE burst beside my crate
/// spares it as my own would (overpressure and fragments). Control: the same
/// shell from a hostile player breaks it and it cooks off.
#[test]
fn an_allys_he_burst_spares_my_crate() {
    use super::s6_tests::{add_crate_at, fire};
    use frontline_content::setup::{StockKind, SupplyKind};
    let mut w = world(&[0, 1, 0, 1], |_| {});
    let d = super::s6_tests::def(&w, 0, StockKind::Crate, Some(SupplyKind::Shells));
    let c = add_crate_at(&mut w, 0, d, FxVec2::from_int(500, 500));
    w.step(&[]);
    let h = w.state.terrain.height_at(FxVec2::from_int(500, 500));
    let from = crate::fx::FxVec3::new(
        Fx::from_ratio(4992, 10),
        Fx::from_int(500),
        h + Fx::from_int(3),
    );
    let down = crate::fx::FxVec3::new(Fx::ZERO, Fx::ZERO, Fx::from_int(-300));
    for (shooter, broken) in [(2u8, false), (1, true)] {
        fire(
            &mut w,
            shooter,
            "ww2:m3_75mm",
            Some("ww2:75mm_m48"),
            from,
            down,
        );
        steps(&mut w, 3);
        let g = game(&w).supply6();
        assert_eq!(
            !g.crates.contains_key(&c),
            broken,
            "a burst of player {shooter}"
        );
    }
}

// ---------------------------------------------------------- T4: capture --

fn point_xy(w: &World, k: usize) -> (i32, i32) {
    let p: FxVec2 = game(w).setup().map.points[k].pos.into();
    (p.x.floor_int(), p.y.floor_int())
}

/// Ticks until point `k` gets an owner (at most `max`).
fn ticks_to_capture(w: &mut World, k: usize, max: u64) -> Option<u64> {
    for t in 1..=max {
        w.step(&[]);
        if game(w).points[k].owner.is_some() {
            return Some(t);
        }
    }
    None
}

/// Plan 9 T4: two allies on a neutral point capture it at the summed rate
/// (one man each = two men of one player) and the one with more men gets
/// it. Control: the same men of one player.
#[test]
fn two_allies_capture_at_the_summed_rate_and_the_larger_gets_it() {
    let k = 2;
    let mut w = world(&[0, 1, 0, 1], |_| {});
    let (x, y) = point_xy(&w, k);
    let a = sq(&mut w, 0, x, y);
    let b = sq(&mut w, 2, x + 2, y);
    thin(&mut w, a, 1);
    thin(&mut w, b, 1);
    let allied = ticks_to_capture(&mut w, k, 20_000).expect("captured");
    let mut c = world(&[0, 1, 0, 1], |_| {});
    let s = sq(&mut c, 0, x, y);
    thin(&mut c, s, 2);
    let alone = ticks_to_capture(&mut c, k, 20_000).expect("captured");
    assert!(allied.abs_diff(alone) <= 1, "{allied} vs {alone}");
    // The larger share takes it.
    let mut w = world(&[0, 1, 0, 1], |_| {});
    let a = sq(&mut w, 0, x, y);
    let b = sq(&mut w, 2, x + 2, y);
    thin(&mut w, a, 1);
    thin(&mut w, b, 3);
    ticks_to_capture(&mut w, k, 20_000).expect("captured");
    assert_eq!(game(&w).points[k].owner, Some(PlayerId(2)));
}

/// Plan 9 T4: an ally on my point does not contest it (and the point stays
/// mine); an enemy does. Control: the enemy.
#[test]
fn an_ally_on_my_point_does_not_contest_it() {
    let k = 2;
    let mut w = world(&[0, 1, 0, 1], |_| {});
    game_mut(&mut w).points[k].owner = Some(PlayerId(0));
    let (x, y) = point_xy(&w, k);
    sq(&mut w, 0, x, y);
    sq(&mut w, 2, x + 3, y);
    steps(&mut w, 300);
    let c = game(&w).points[k].clone();
    assert!(!c.contested);
    assert_eq!(c.owner, Some(PlayerId(0)));
    assert_eq!(c.capturer, None);
    sq(&mut w, 3, x - 3, y);
    steps(&mut w, 5);
    assert!(game(&w).points[k].contested, "control: an enemy contests");
}

// ---------------------------------------------------- T6, T7: the match --

/// Plan 9 T6: team drain equals the 1v1 drain for the same point gap.
/// Control: per-player drain with `team_tickets` 0.
#[test]
fn team_drain_equals_the_1v1_drain_for_the_same_gap() {
    let mut one = super::s6_tests::flat_world(1);
    let g = game_mut(&mut one);
    g.points[0].owner = Some(PlayerId(1));
    g.points[1].owner = Some(PlayerId(1));
    let d1 = g.drain_per_min(PlayerId(0));
    assert!(d1 > 0);
    for tt in [1, 0] {
        let mut w = world(&[0, 1, 0, 1], |s| s.rules.teams.team_tickets = tt);
        let g = game_mut(&mut w);
        // Team 1 holds two points split over its members.
        g.points[0].owner = Some(PlayerId(1));
        g.points[1].owner = Some(PlayerId(3));
        for p in [0u8, 2] {
            let d = g.drain_per_min(PlayerId(p));
            if tt == 1 {
                assert_eq!(d, d1, "player {p}");
            } else {
                assert!(d < d1, "control: per player the gap is one point");
            }
        }
        assert_eq!(g.drain_per_min(PlayerId(1)), 0);
    }
}

/// Plan 9 T7: one member wiped with an ally standing is not out; both
/// wiped is a loss; a team wins when the other team's members are all out.
/// Control: `wiped_waits` 0 puts the wiped player out at once.
#[test]
fn a_wiped_player_waits_for_his_ally_and_the_last_team_wins() {
    for waits in [1, 0] {
        let mut w = world(&[0, 1, 0, 1], |s| {
            s.rules.teams.wiped_waits = waits;
            s.factions.iter_mut().for_each(|f| f.start_squads.clear());
        });
        // Every player has a squad except player 2 (no men, no manpower).
        for p in [0u8, 1, 3] {
            sq(&mut w, p, 200 + 100 * p as i32, 500);
        }
        game_mut(&mut w).players[2].manpower = 0;
        steps(&mut w, 2);
        let d = game(&w).players[2].defeat;
        if waits == 1 {
            assert_eq!(d, None, "his ally stands");
        } else {
            assert_eq!(d, Some(Defeat::Wiped), "control");
        }
        assert_eq!(game(&w).outcome(), Outcome::Running);
    }
    // Both wiped: team 0 is out and team 1 wins (lowest standing slot).
    let mut w = world(&[0, 1, 0, 1], |s| {
        s.factions.iter_mut().for_each(|f| f.start_squads.clear())
    });
    for p in [1u8, 3] {
        sq(&mut w, p, 200 + 100 * p as i32, 500);
    }
    game_mut(&mut w).players[0].manpower = 0;
    game_mut(&mut w).players[2].manpower = 0;
    steps(&mut w, 2);
    assert_eq!(game(&w).players[0].defeat, Some(Defeat::Wiped));
    assert_eq!(game(&w).players[2].defeat, Some(Defeat::Wiped));
    assert_eq!(
        game(&w).outcome(),
        Outcome::Won {
            winner: PlayerId(1)
        }
    );
    assert!(game(&w).teams().won_by(3, 1));
}

/// T7: a surrendered member leaves his ally playing; the match ends when
/// the whole team is out. Control: the 1v1 rule (one out, the other wins).
#[test]
fn one_member_out_keeps_the_match_running() {
    let mut w = world(&[0, 1, 0, 1], |_| {});
    game_mut(&mut w).players[1].defeat = Some(Defeat::Tickets);
    game_mut(&mut w).update_outcome();
    assert_eq!(game(&w).outcome(), Outcome::Running);
    game_mut(&mut w).players[3].defeat = Some(Defeat::Tickets);
    game_mut(&mut w).update_outcome();
    assert_eq!(
        game(&w).outcome(),
        Outcome::Won {
            winner: PlayerId(0)
        }
    );
}

// --------------------------------------------------------- T8, T9 -------

/// Plan 9 T8: an ally's abandoned gun cannot be captured; an enemy's can
/// (the control, the S9 rule).
#[test]
fn an_allys_abandoned_gun_cannot_be_captured() {
    let mut w = world(&[0, 1, 0, 1], |_| {});
    let gsq = spawn(&mut w, 0, "ww2:ger_pak40_squad", 500, 500);
    let gun = members(&w, gsq)[0];
    steps(&mut w, 2);
    let crew: Vec<SimId> = game(&w).vehicles[&gun]
        .served
        .as_ref()
        .unwrap()
        .slots
        .iter()
        .filter_map(|s| s.man)
        .collect();
    for m in crew {
        assert!(w.kill_soldier(m));
    }
    let hz = w.match_setup().unwrap().tick_hz as u64;
    steps(&mut w, 15 * hz);
    assert_eq!(
        w.entities().kind[gun.index()],
        crate::ecs::EntityKind::Abandoned
    );
    let ally = sq(&mut w, 2, 505, 500);
    let enemy = sq(&mut w, 1, 505, 505);
    let g = game(&w);
    assert_eq!(
        g.recrew_check(w.entities(), PlayerId(2), ally, gun),
        Err(crate::bail::RecrewRefusal::NoCapture)
    );
    assert_ne!(
        g.recrew_check(w.entities(), PlayerId(1), enemy, gun),
        Err(crate::bail::RecrewRefusal::NoCapture),
        "control: an enemy may capture it"
    );
}

/// Review (T8): an ally's set-down crate cannot be taken (crates serve
/// their owner only). Control: a hostile player may take it (the S9 rule).
#[test]
fn an_allys_crate_cannot_be_taken() {
    use super::s6_tests::{add_crate_at, run};
    use frontline_content::setup::{StockKind, SupplyKind};
    let mut w = world(&[0, 1, 0, 1], |_| {});
    let d = super::s6_tests::def(&w, 0, StockKind::Crate, Some(SupplyKind::Shells));
    let c = add_crate_at(&mut w, 0, d, FxVec2::from_int(500, 500));
    let ally = sq(&mut w, 2, 520, 500);
    let enemy = sq(&mut w, 1, 480, 500);
    w.step(&[]);
    let claim = |w: &World| game(w).supply6().crates[&c].claim;
    let take = |squad| CommandKind::PickUpCrate { squad, crate_id: c };
    run(&mut w, 2, 1, take(ally));
    assert_eq!(claim(&w), None, "an ally may not take my crate");
    run(&mut w, 1, 1, take(enemy));
    assert_eq!(claim(&w), Some(enemy), "control: an enemy may");
}

/// Plan 9 T9: `pop_pct` caps a team player's pop, not a lone player's;
/// the start keys apply only on a team. Control: the 1v1 values.
#[test]
fn pop_pct_and_start_keys_apply_only_on_a_team() {
    let w = super::s6_tests::flat_world_with(1, |s| {
        s.rules.teams.pop_pct = 50;
        s.rules.teams.start_manpower_pct = 200;
    });
    let r = &game(&w).setup().rules;
    assert_eq!(game(&w).pop_cap_of(PlayerId(0)), r.pop_cap);
    assert_eq!(game(&w).players[0].manpower, r.start_manpower);
    let w = world(&[0, 1, 0, 1], |s| {
        s.rules.teams.pop_pct = 50;
        s.rules.teams.start_manpower_pct = 200;
        s.rules.teams.tickets_pct = 50;
    });
    let r = &game(&w).setup().rules;
    for p in 0..4u8 {
        assert_eq!(game(&w).pop_cap_of(PlayerId(p)), r.pop_cap / 2);
        assert_eq!(game(&w).players[p as usize].manpower, r.start_manpower * 2);
        assert_eq!(game(&w).players[p as usize].tickets, r.tickets / 2);
    }
    // A free-for-all of three: nobody is on a team.
    let w = world(&[0, 1, 2], |s| s.rules.teams.pop_pct = 50);
    let r = &game(&w).setup().rules;
    assert_eq!(game(&w).pop_cap_of(PlayerId(2)), r.pop_cap);
}

// ------------------------------------------------------------------ AI --

/// Plan 9 A1: the foe sites pick the standing hostile player whose base is
/// nearest; `foe_first` 1 the lowest hostile slot. Control: the 1v1 foe is
/// the other player.
#[test]
fn the_main_foe_is_the_nearest_hostile_base() {
    let w = super::s6_tests::flat_world(1);
    assert_eq!(game(&w).main_foe(PlayerId(0)), Some(PlayerId(1)));
    assert_eq!(game(&w).main_foe(PlayerId(1)), Some(PlayerId(0)));
    // Bases: 0 (96, 96), 1 (928, 928), 2 (96, 928), 3 (928, 96).
    // Teams {0, 2} west, {1, 3} east.
    let mut w = world(&[0, 1, 0, 1], |_| {});
    let g = game(&w);
    assert_eq!(g.main_foe(PlayerId(0)), Some(PlayerId(3)));
    assert_eq!(g.main_foe(PlayerId(2)), Some(PlayerId(1)));
    assert_eq!(g.main_foe(PlayerId(1)), Some(PlayerId(2)));
    // The nearest foe out: the other one.
    game_mut(&mut w).players[3].defeat = Some(Defeat::Tickets);
    assert_eq!(game(&w).main_foe(PlayerId(0)), Some(PlayerId(1)));
    let w = world(&[0, 1, 0, 1], |s| s.rules.ai.team.foe_first = 1);
    assert_eq!(game(&w).main_foe(PlayerId(0)), Some(PlayerId(1)));
}

/// Plan 9 A2: the AI gives an ally's point no squad. Player 0 is an AI
/// whose ally holds every point but one; no task of player 0 ever names an
/// ally's point. Control: the one free point gets tasks.
#[test]
fn the_ai_gives_an_allys_point_no_squad() {
    let mut w = world(&[0, 1, 0, 1], |s| s.players[0].is_ai = true);
    let n = game(&w).points.len();
    let free = n - 1;
    for k in 0..free {
        game_mut(&mut w).points[k].owner = Some(PlayerId(2));
    }
    for _ in 0..4 {
        sq(&mut w, 0, 150, 150);
    }
    let hz = w.match_setup().unwrap().tick_hz as u64;
    let mut seen_free = false;
    for _ in 0..60 {
        steps(&mut w, hz);
        let tasks = &game(&w).ai[0].as_ref().unwrap().tasks;
        for t in tasks.values() {
            assert!(
                t.point as usize >= free || t.point == crate::ai::NO_POINT,
                "a task at the ally's point {}",
                t.point
            );
            seen_free |= t.point as usize == free;
        }
    }
    assert!(seen_free, "control: the free point gets squads");
}

/// S17b A4 (`help_ally`): an ally's point with a known enemy squad beside
/// it gets a defence task from me. Player 2 (my ally) holds point 0 with a
/// squad on it; an enemy squad (player 1) stands 40 m off, seen by my ally
/// (shared sight). Control: with `help_ally` 0 no task of mine names an
/// ally's point (A2).
#[test]
fn the_ai_helps_an_allys_threatened_point() {
    let helped = |help: u32| -> bool {
        let mut w = world(&[0, 1, 0, 1], |s| {
            s.players[0].is_ai = true;
            s.rules.ai.team.help_ally = help;
        });
        let n = game(&w).points.len();
        for k in 0..n - 1 {
            game_mut(&mut w).points[k].owner = Some(PlayerId(2));
        }
        let (x, y) = point_xy(&w, 0);
        sq(&mut w, 2, x, y);
        sq(&mut w, 1, x + 40, y);
        for _ in 0..4 {
            sq(&mut w, 0, 150, 150);
        }
        let hz = w.match_setup().unwrap().tick_hz as u64;
        for _ in 0..40 {
            steps(&mut w, hz);
            let tasks = &game(&w).ai[0].as_ref().unwrap().tasks;
            if tasks
                .values()
                .any(|t| t.point == 0 && t.kind == crate::ai::TaskKind::Defend)
            {
                return true;
            }
        }
        false
    };
    assert!(helped(1), "A4: a squad defends the ally's point");
    assert!(!helped(0), "control: no help with the switch off");
}

// -------------------------------------------------- 3 and 4 players: smoke --

/// Steps `w` `ticks` ticks; every 30 ticks checks that no soldier, vehicle
/// gun or vehicle MG aims at an ally of its owner. Returns the hashes every
/// 300 ticks.
fn run_checked(w: &mut World, ticks: u64) -> Vec<u64> {
    let mut hashes = Vec::new();
    for t in 1..=ticks {
        w.step(&[]);
        if t % 30 == 0 {
            let e = w.entities();
            let g = game(w);
            let teams = g.teams();
            let ally = |o: PlayerId, t: SimId| {
                let to = e.owner[t.index()];
                to != o && !teams.hostile_id(o, to)
            };
            for i in 0..e.len() {
                if e.kind[i] == crate::ecs::EntityKind::Soldier
                    && let Some(tg) = e.combat[i].target
                {
                    assert!(!ally(e.owner[i], tg), "tick {t}: a man aims at an ally");
                }
            }
            for (&id, vs) in &g.vehicles {
                let o = e.owner[id.index()];
                if e.kind[id.index()] != crate::ecs::EntityKind::Vehicle {
                    continue;
                }
                if let Some(tg) = vs.target {
                    assert!(!ally(o, tg), "tick {t}: a gun aims at an ally");
                }
                for m in &vs.mgs {
                    if let Some(tg) = m.target {
                        assert!(!ally(o, tg), "tick {t}: an MG aims at an ally");
                    }
                }
            }
        }
        if t % 300 == 0 {
            hashes.push(w.state_hash());
        }
        if game(w).outcome() != Outcome::Running {
            break;
        }
    }
    hashes
}

fn smoke(teams: &[u8], ticks: u64) {
    let s = team_setup(teams, false, true, |_| {});
    let mut a = World::new_match(s.clone(), 1).unwrap();
    let mut b = World::new_match(s, 3).unwrap();
    let ha = run_checked(&mut a, ticks);
    let hb = run_checked(&mut b, ticks);
    assert_eq!(ha, hb, "hashes differ between 1 and 3 threads");
    // Through save and load.
    let back = World::load(&a.save(), 2).unwrap();
    assert_eq!(back.full_state_hash(), a.full_state_hash());
    let g = game(&a);
    assert!(
        g.squads
            .values()
            .map(|q| q.owner.0)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            >= teams.len().min(3),
        "the AIs buy"
    );
}

/// Plan 9 smoke (short): a 4-player 2v2 of AIs on Kessel Ridge with two
/// more bases runs 4 minutes with no panic, no ally aimed at, and equal
/// hashes at 1 and 3 threads and through save/load.
#[test]
fn a_2v2_of_ais_runs_with_no_ally_targeted() {
    smoke(&[0, 1, 0, 1], 4 * 60 * 30);
}

/// Plan 9 smoke (short): a 3-player free-for-all (the first runs with a
/// third player).
#[test]
fn a_3_player_free_for_all_runs() {
    smoke(&[0, 1, 2], 4 * 60 * 30);
}

/// Plan 9 smoke (long): 45 minutes of each, one outcome or the cap.
/// `cargo test --release -- --ignored s17_smoke_45`.
#[test]
#[ignore]
fn s17_smoke_45_min() {
    smoke(&[0, 1, 0, 1], 45 * 60 * 30);
    smoke(&[0, 1, 2], 45 * 60 * 30);
}
