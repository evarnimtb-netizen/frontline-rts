//! S10b-ai tests (docs/slices/S10b-ai.md 3, 6; Build A): the saved layout,
//! its load checks, the refusal memory (L3, L4) and the blueprints of the
//! new work kinds.

use frontline_content::setup::Capability;

use super::s6_tests::{flat_world_with, spawn, steps};
use super::*;
use crate::ai::fort::{WorkKind, fort_of};
use crate::ai::{
    AiDemolish, AiFortJob, AiRefusal, AiState, AiSweep, DemolishPhase, FortPhase, JobKind,
    MAX_BLOCKED_PAIRS, MAX_REFUSED, SweepPhase,
};
use crate::fort::{MAX_BLASTS, MineBlast};
use crate::game::SquadId;

fn c(x: i32, y: i32) -> FxVec2 {
    FxVec2::from_int(x, y)
}

fn fort(w: &World, id: &str) -> u16 {
    w.match_setup()
        .unwrap()
        .forts
        .iter()
        .position(|f| f.id == id)
        .unwrap() as u16
}

/// Player 1 is an AI; engineers can sweep and demolish.
fn ai_world() -> World {
    flat_world_with(1, |s| {
        s.players[1].is_ai = true;
        for t in s.squad_types.iter_mut() {
            if t.id.contains("engineer") {
                t.caps |= Capability::PlaceMines.bit()
                    | Capability::ClearMines.bit()
                    | Capability::Demolish.bit();
            }
        }
    })
}

fn ai_of(s: &mut SimState) -> &mut AiState {
    s.game.as_mut().unwrap().ai[1].as_mut().unwrap()
}

fn fort_of_state(s: &mut SimState) -> &mut crate::fort::FortState {
    &mut s.game.as_mut().unwrap().fort
}

/// The squads of [`ai_busy`]: sweeper, demolisher, builder, own tank and
/// its vehicle, an enemy squad.
struct Busy {
    sweep: SquadId,
    demo: SquadId,
    builder: SquadId,
    vehicle: SimId,
    enemy: SquadId,
    rifle: SquadId,
}

/// An AI world with every S10b-ai field set: a sweep, a demolition with one
/// piece done, two refusals, an AP field fort job with its start, a blast,
/// the new counters and account fields.
fn ai_busy() -> (World, Busy) {
    let mut w = ai_world();
    steps(&mut w, 3);
    let sweep = spawn(&mut w, 1, "ww2:usa_engineer_squad", 600, 600);
    let demo = spawn(&mut w, 1, "ww2:usa_engineer_squad", 620, 600);
    let builder = spawn(&mut w, 1, "ww2:usa_engineer_squad", 640, 600);
    let tank = spawn(&mut w, 1, "ww2:usa_m4a3_75_squad", 600, 650);
    let rifle = spawn(&mut w, 1, "ww2:usa_rifle_squad", 660, 600);
    let enemy = spawn(&mut w, 0, "ww2:ger_grenadiergruppe", 300, 300);
    let vehicle = w.game().unwrap().squads[&tank].members[0];
    let wall = fort(&w, "ww2:sandbag_wall");
    assert!(w.build_now(wall, &[c(400, 400), c(410, 400)], PlayerId(0)));
    let piece = *w.game().unwrap().fort().pieces.keys().next().unwrap();
    let ap = fort(&w, "ww2:ap_minefield");
    let tick = w.state.tick;
    assert!(w.with_ai_state(1, |ai| {
        ai.sweeps.insert(
            sweep,
            AiSweep {
                point: 0,
                a: c(600, 610),
                b: c(600, 640),
                start: tick - 2,
                since: tick - 1,
                phase: SweepPhase::Sweep,
                tries: 1,
            },
        );
        ai.demolitions.insert(
            demo,
            AiDemolish {
                vehicle,
                pieces: [Some(piece), Some(piece), None],
                done: 1,
                start: tick - 2,
                since: tick,
                phase: DemolishPhase::Charge,
            },
        );
        ai.refuse(JobKind::Sweep, c(500, 500), tick - 1);
        ai.refuse(JobKind::Fort, c(520, 500), tick);
        ai.forts.insert(
            builder,
            AiFortJob {
                fort: ap,
                point: 0,
                line: vec![c(640, 620), c(670, 620)],
                phase: FortPhase::Go,
                start: tick - 1,
                since: tick,
                work: None,
                tries: 0,
            },
        );
        ai.suspects_seen = 1;
        ai.counters.s10b_jobs = 1;
        ai.counters.refused_reorders = 2;
    }));
    w.edit_fort(|f| {
        f.blasts.push(MineBlast {
            pos: c(700, 700),
            owner: PlayerId(0),
            tick: tick - 1,
        });
        let acc = &mut f.accounts[1];
        acc.lost_on_known = 1;
        acc.at_exposure_ticks = 2;
        acc.own_mine_losses = 3;
        acc.own_pieces_demolished = 4;
    });
    let b = Busy {
        sweep,
        demo,
        builder,
        vehicle,
        enemy,
        rifle,
    };
    (w, b)
}

type StateEdit = Box<dyn Fn(&mut SimState)>;

/// Player 1's engineer squads, ascending.
fn engineers(s: &mut SimState) -> Vec<SquadId> {
    let g = s.game.as_ref().unwrap();
    g.squads
        .iter()
        .filter(|(_, q)| {
            q.owner == PlayerId(1)
                && g.setup.squad_types[q.type_idx as usize]
                    .id
                    .contains("engineer")
        })
        .map(|(&id, _)| id)
        .collect()
}

/// Control: hash `FortState` without `blasts` (`hash.rs`, the fort's
/// postcard bytes with the list cleared) -> the blast edit is not hashed.
#[test]
fn s10b_ai_layout_roundtrips_and_hashes() {
    let (w, _) = ai_busy();
    let bytes = w.save();
    let back = World::load(&bytes, 1).expect("the full S10b-ai state loads");
    assert_eq!(back.full_state_hash(), w.full_state_hash());
    assert_eq!(back.game().unwrap().ai[1], w.game().unwrap().ai[1]);
    assert_eq!(back.game().unwrap().fort(), w.game().unwrap().fort());
    assert_eq!(back.save(), bytes, "the loaded world saves the same bytes");
    let h = back.full_state_hash();
    let edits: Vec<StateEdit> = vec![
        Box::new(|s| ai_of(s).sweeps.values_mut().next().unwrap().tries += 1),
        Box::new(|s| ai_of(s).sweeps.values_mut().next().unwrap().phase = SweepPhase::Go),
        Box::new(|s| ai_of(s).sweeps.values_mut().next().unwrap().start -= 1),
        Box::new(|s| ai_of(s).demolitions.values_mut().next().unwrap().done = 0),
        Box::new(|s| {
            let j = ai_of(s).demolitions.values_mut().next().unwrap();
            j.pieces[2] = j.pieces[0];
        }),
        Box::new(|s| ai_of(s).refused[0].failed -= 1),
        Box::new(|s| ai_of(s).suspects_seen += 1),
        Box::new(|s| ai_of(s).forts.values_mut().next().unwrap().start -= 1),
        Box::new(|s| ai_of(s).counters.jobs_overdue += 1),
        Box::new(|s| ai_of(s).counters.own_route_rejects += 1),
        Box::new(|s| fort_of_state(s).blasts[0].pos = c(701, 700)),
        Box::new(|s| fort_of_state(s).accounts[1].at_exposure_ticks += 1),
        Box::new(|s| fort_of_state(s).accounts[1].own_pieces_demolished += 1),
        Box::new(|s| s.game.as_mut().unwrap().setup.rules.ai.obstacles.at_out_m += 1),
        Box::new(|s| s.game.as_mut().unwrap().setup.rules.ai.truck.refusal_memory = 0),
        Box::new(|s| {
            let f = s
                .game
                .as_mut()
                .unwrap()
                .setup
                .rules
                .fortify
                .as_mut()
                .unwrap();
            f.mine_band_foot_cm += 1;
        }),
    ];
    for (k, e) in edits.into_iter().enumerate() {
        let mut x = World::load(&bytes, 1).unwrap();
        e(&mut x.state);
        let y = World::load(&x.save(), 1).unwrap_or_else(|e| panic!("edit {k}: {e:?}"));
        assert_ne!(y.full_state_hash(), h, "edit {k} is not hashed");
    }
    // A fresh AI state holds none of it.
    let fresh = AiState::new(0);
    assert!(fresh.sweeps.is_empty() && fresh.demolitions.is_empty() && fresh.refused.is_empty());
    assert_eq!(fresh.suspects_seen, 0);
}

/// One case per line of spec 6. Control: drop the `refused` sort check in
/// `AiState::validate_s10b` -> the unsorted case (`refused` reversed) loads.
#[test]
fn load_rejects_bad_s10b_ai_state() {
    let (w, b) = ai_busy();
    let bytes = w.save();
    let Busy {
        sweep,
        demo,
        builder,
        vehicle,
        enemy,
        rifle,
    } = b;
    let enemy_soldier = w.game().unwrap().squads[&enemy].members[0];
    let blast = |s: &mut SimState| fort_of_state(s).blasts[0].clone();
    let bad: Vec<(&str, StateEdit)> = vec![
        // `sweeps`, `demolitions`: caps.
        (
            "sweeps over the cap",
            Box::new(move |s| {
                let j = ai_of(s).sweeps[&sweep].clone();
                for k in engineers(s) {
                    ai_of(s).sweeps.insert(k, j.clone());
                }
            }),
        ),
        (
            "demolitions over the cap",
            Box::new(move |s| {
                let j = ai_of(s).demolitions[&demo].clone();
                for k in engineers(s) {
                    ai_of(s).demolitions.insert(k, j.clone());
                }
            }),
        ),
        // Keys: squads made so far, own with `clear_mines` / `demolish`
        // while they stand (a squad gone since the plan loads: finding 1).
        (
            "sweep of an enemy squad",
            Box::new(move |s| {
                let j = ai_of(s).sweeps.remove(&sweep).unwrap();
                ai_of(s).sweeps.insert(enemy, j);
            }),
        ),
        (
            "sweep of a squad without clear_mines",
            Box::new(move |s| {
                let j = ai_of(s).sweeps.remove(&sweep).unwrap();
                ai_of(s).sweeps.insert(rifle, j);
            }),
        ),
        (
            "sweep of a squad never made",
            Box::new(move |s| {
                let j = ai_of(s).sweeps.remove(&sweep).unwrap();
                ai_of(s).sweeps.insert(SquadId(100_000), j);
            }),
        ),
        (
            "demolition of a squad without demolish",
            Box::new(move |s| {
                let j = ai_of(s).demolitions.remove(&demo).unwrap();
                ai_of(s).demolitions.insert(rifle, j);
            }),
        ),
        // Points, lane on the map and not longer than `lane_max_m`.
        (
            "sweep point out of range",
            Box::new(|s| ai_of(s).sweeps.values_mut().next().unwrap().point = 9),
        ),
        (
            "sweep lane off the map",
            Box::new(|s| ai_of(s).sweeps.values_mut().next().unwrap().a = c(-1, 610)),
        ),
        (
            "sweep lane end off the map",
            Box::new(|s| ai_of(s).sweeps.values_mut().next().unwrap().b = c(600, 5000)),
        ),
        (
            "sweep lane too long",
            Box::new(|s| ai_of(s).sweeps.values_mut().next().unwrap().b = c(600, 700)),
        ),
        // `vehicle` an own vehicle.
        (
            "demolition for an enemy soldier",
            Box::new(move |s| {
                ai_of(s).demolitions.values_mut().next().unwrap().vehicle = enemy_soldier
            }),
        ),
        (
            "demolition for an own soldier",
            Box::new(move |s| {
                let man = s.game.as_ref().unwrap().squads[&builder].members[0];
                assert_ne!(man, vehicle);
                ai_of(s).demolitions.values_mut().next().unwrap().vehicle = man
            }),
        ),
        (
            "demolition for a vehicle never made",
            Box::new(|s| {
                ai_of(s).demolitions.values_mut().next().unwrap().vehicle = SimId(u32::MAX)
            }),
        ),
        // Pieces < `next_piece`; `done <= 3`.
        (
            "demolition piece out of range",
            Box::new(|s| {
                ai_of(s).demolitions.values_mut().next().unwrap().pieces[2] = Some(99_999)
            }),
        ),
        (
            "demolition done over 3",
            Box::new(|s| ai_of(s).demolitions.values_mut().next().unwrap().done = 4),
        ),
        // Charge: a piece at `done` (review finding 2: `run_demolish`
        // indexed `pieces[done]`).
        (
            "demolition in Charge with done 3",
            Box::new(|s| {
                let j = ai_of(s).demolitions.values_mut().next().unwrap();
                j.pieces = [j.pieces[0]; 3];
                j.done = 3;
            }),
        ),
        (
            "demolition in Charge with no piece at done",
            Box::new(|s| ai_of(s).demolitions.values_mut().next().unwrap().done = 2),
        ),
        // `start <= since <= tick`.
        (
            "sweep start after since",
            Box::new(|s| {
                let j = ai_of(s).sweeps.values_mut().next().unwrap();
                j.start = j.since + 1;
            }),
        ),
        (
            "sweep since in the future",
            Box::new(|s| {
                let t = s.tick;
                ai_of(s).sweeps.values_mut().next().unwrap().since = t + 1;
            }),
        ),
        (
            "demolition start after since",
            Box::new(|s| {
                let j = ai_of(s).demolitions.values_mut().next().unwrap();
                j.start = j.since + 1;
            }),
        ),
        (
            "demolition since in the future",
            Box::new(|s| {
                let t = s.tick;
                let j = ai_of(s).demolitions.values_mut().next().unwrap();
                j.since = t + 1;
            }),
        ),
        // `refused`: <= 32, sorted, `failed <= tick`.
        (
            "refused over the cap",
            Box::new(|s| {
                let r = ai_of(s).refused[0].clone();
                ai_of(s).refused = vec![r; MAX_REFUSED + 1];
            }),
        ),
        (
            "refused not sorted",
            Box::new(|s| ai_of(s).refused.reverse()),
        ),
        (
            "refusal in the future",
            Box::new(|s| {
                let t = s.tick;
                ai_of(s).refused.last_mut().unwrap().failed = t + 1;
            }),
        ),
        // `blocked_pairs` (Build D): <= 32, strictly ascending, pieces made.
        (
            "blocked_pairs over the cap",
            Box::new(|s| {
                ai_of(s).blocked_pairs = (0..=MAX_BLOCKED_PAIRS as u32)
                    .map(|k| (SquadId(k), 0))
                    .collect();
            }),
        ),
        (
            "blocked_pairs not sorted",
            Box::new(|s| ai_of(s).blocked_pairs = vec![(SquadId(2), 0), (SquadId(1), 0)]),
        ),
        (
            "blocked pair of a piece never made",
            Box::new(|s| ai_of(s).blocked_pairs = vec![(SquadId(1), u32::MAX)]),
        ),
        // `blasts`: <= 32, owner a player, `tick <= now`, on the map.
        (
            "blasts over the cap",
            Box::new(move |s| {
                let x = blast(s);
                fort_of_state(s).blasts = vec![x; MAX_BLASTS + 1];
            }),
        ),
        (
            "blast owner not a player",
            Box::new(|s| fort_of_state(s).blasts[0].owner = PlayerId(9)),
        ),
        (
            "blast in the future",
            Box::new(|s| {
                let t = s.tick;
                fort_of_state(s).blasts[0].tick = t + 1;
            }),
        ),
        (
            "blast off the map",
            Box::new(|s| fort_of_state(s).blasts[0].pos = c(5000, 5)),
        ),
        // `AiFortJob::start <= since`.
        (
            "fort job start after since",
            Box::new(|s| {
                let j = ai_of(s).forts.values_mut().next().unwrap();
                j.start = j.since + 1;
            }),
        ),
    ];
    for (what, e) in bad {
        let mut x = World::load(&bytes, 1).unwrap();
        if what.ends_with("over the cap") {
            // Enough own engineer squads that only the cap is broken.
            for n in 0..5 {
                spawn(&mut x, 1, "ww2:usa_engineer_squad", 700 + 4 * n, 700);
            }
            assert!(engineers(&mut x.state).len() > 5);
        }
        e(&mut x.state);
        assert!(
            World::load(&x.save(), 1).is_err(),
            "bad state loaded: {what}"
        );
    }
}

/// L3: a full memory replaces its oldest failure; L4 reads kind, age and
/// distance; stale entries drop. Control: replace the newest instead (the
/// `max_by_key` in `AiState::refuse`) -> the oldest stays.
#[test]
fn refusal_memory_replaces_oldest_failure() {
    let mut ai = AiState::new(0);
    // 32 failures, ticks 100..132 in a scrambled order, two kinds.
    for k in 0..MAX_REFUSED as u64 {
        let t = 100 + (k * 13) % MAX_REFUSED as u64;
        let kind = if t.is_multiple_of(2) {
            JobKind::Fort
        } else {
            JobKind::Sweep
        };
        ai.refuse(kind, c(10 * t as i32, 0), t);
    }
    assert_eq!(ai.refused.len(), MAX_REFUSED);
    let sorted = |ai: &AiState| ai.refused.windows(2).all(|w| w[0].key() <= w[1].key());
    assert!(sorted(&ai));
    ai.refuse(JobKind::Demolish, c(5, 5), 200);
    assert_eq!(ai.refused.len(), MAX_REFUSED);
    assert!(sorted(&ai));
    assert!(
        !ai.refused.iter().any(|r| r.failed == 100),
        "the oldest went"
    );
    assert!(
        ai.refused.iter().any(|r| r.failed == 131),
        "the newest stays"
    );
    assert!(ai.refused.iter().any(|r| r.failed == 200));
    // L4: same kind, young, near.
    let hz = 30;
    assert!(ai.refused_near(JobKind::Demolish, c(15, 20), 200, 300 * hz, 20));
    assert!(
        !ai.refused_near(JobKind::Demolish, c(5, 30), 200, 300 * hz, 20),
        "too far"
    );
    assert!(
        !ai.refused_near(JobKind::Sweep, c(5, 5), 200, 300 * hz, 20),
        "another kind"
    );
    assert!(
        !ai.refused_near(JobKind::Demolish, c(5, 5), 200 + 300 * hz, 300 * hz, 20),
        "aged out"
    );
    assert!(
        !ai.refused_near(JobKind::Demolish, c(5, 5), 200, 0, 20),
        "refused_s 0"
    );
    // Stale entries drop.
    ai.drop_stale_refusals(150, 20);
    assert!(ai.refused.iter().all(|r| r.failed > 130));
    assert_eq!(ai.refused.len(), 2);
    let r = AiRefusal {
        kind: JobKind::TruckGoal,
        at: c(1, 1),
        failed: 1,
    };
    assert_eq!(
        r.key(),
        (JobKind::TruckGoal, 1, Fx::ONE.raw(), Fx::ONE.raw())
    );
}

/// Control: the R29 guard (`mine_item`, `blocks_vehicles`) on every kind in
/// `ai::fort::fort_of` -> the minefields and obstacles are `None`.
#[test]
fn fort_of_returns_s10b_blueprints_for_new_kinds() {
    let w = ai_world();
    let s = w.match_setup().unwrap();
    let id = |k| s.forts[fort_of(s, k).unwrap() as usize].id.as_str();
    assert_eq!(id(WorkKind::ApField), "ww2:ap_minefield");
    assert_eq!(id(WorkKind::AtField), "ww2:at_minefield");
    assert_eq!(id(WorkKind::Hedgehog), "ww2:hedgehog");
    assert_eq!(id(WorkKind::LogBarrier), "ww2:log_barrier");
    assert_eq!(id(WorkKind::Bunker), "ww2:bunker");
    // The S10a kinds keep R29.
    assert_eq!(id(WorkKind::Wall), "ww2:sandbag_wall");
    assert_eq!(id(WorkKind::Nest), "ww2:mg_nest");
}

/// The hook skeleton: every habit is built (Fort: Build B; Sweep and
/// Demolish: Build D); without a squad none ever ends and the world steps.
/// Control: an arm returning `Some(NotBuilt)` -> the loop ends early (seen
/// with Demolish before Build D filled it).
#[test]
fn drive_ai_habit_skeleton_ends_not_built() {
    use crate::ai::{AiHabit, DriveArgs};
    let mut w = ai_world();
    for h in [
        AiHabit::Fort,
        AiHabit::Sweep,
        AiHabit::Demolish,
        AiHabit::Truck,
    ] {
        let mut ai = AiState::new(w.state.tick);
        let t0 = w.state.tick;
        let none =
            (0..10).find_map(|_| w.drive_ai_habit(&mut ai, PlayerId(1), h, &DriveArgs::default()));
        assert_eq!(none, None, "{h:?} without a squad");
        assert!(w.state.tick > t0);
    }
}

// ---------------------------------------------------------------------------
// Liveness harness (L1-L4; spec 5 "Generic"). Build A: the harness and its
// check on dummy traces; B, C and D add one `LIVENESS` row per job kind.

/// What a liveness row drives: the habit, its args and the goal it can never
/// reach (the refusal is looked for within `refused_m` of it).
pub(crate) struct Stage {
    pub habit: crate::ai::AiHabit,
    pub args: crate::ai::DriveArgs,
    pub goal: FxVec2,
    /// The caller-kept AI state the row starts from (a fresh one if None).
    pub ai: Option<AiState>,
}

/// One row per job kind: the setup builds a world whose goal is
/// unreachable for that kind (B: Fort; D: Sweep, Demolish, TruckGoal).
type StageSetup = fn() -> (World, Stage);
/// (kind, setup, the row's L1 key is `barrier_job_max_s`: Build E, C1).
const LIVENESS: &[(JobKind, StageSetup, bool)] = &[
    (JobKind::Fort, fort_row, false),
    (JobKind::Fort, barrier_row, true),
    (JobKind::Fort, bunker_row, false),
    (JobKind::Sweep, sweep_row, false),
    (JobKind::Demolish, demolish_row, false),
    (JobKind::TruckGoal, truck_row, false),
    (JobKind::Fort, front_ap_row, false),
];

/// The TruckGoal row (Build D, L5): the AI's loaded truck in a trench ring
/// and its own HQ out of stock outside it, so the HQ drop can never be
/// reached (the truck's 180 s drop limit ends it); the refusal is at the
/// HQ. Control: `refusal_memory` 0 -> no refusal (the check fails: "no
/// refusal", seen).
pub(crate) fn truck_row() -> (World, Stage) {
    use frontline_content::setup::StockKind;
    let mut w = ai_world();
    let hq_at = c(700, 700);
    let hq = super::s6_tests::def(
        &w,
        w.match_setup().unwrap().players[1].faction,
        StockKind::Hq,
        None,
    );
    w.build_structure_now(PlayerId(1), hq, hq_at, Fx::ZERO, [0, 0, 0])
        .unwrap();
    let line: Vec<FxVec2> = [(-24, -24), (24, -24), (24, 24), (-24, 24), (-24, -24)]
        .iter()
        .map(|&(a, b)| c(620 + a, 620 + b))
        .collect();
    assert!(w.dig_trench_now(&line, 1));
    let truck = spawn(&mut w, 1, "ww2:usa_cckw_squad", 620, 620);
    let v = w.game().unwrap().squads[&truck].members[0];
    w.edit_vehicle(v, |vs| vs.cargo_g = [100_000, 100_000, 300_000]);
    let stage = Stage {
        habit: crate::ai::AiHabit::Truck,
        args: crate::ai::DriveArgs {
            squad: Some(truck),
            ..Default::default()
        },
        goal: hq_at,
        ai: None,
    };
    (w, stage)
}

/// The Demolish row (Build D): a tank blocked inside a closed hedgehog
/// ring and AI engineers with no satchel left, so every `Demolish` is
/// refused (`NotAllowed`); the refusal is at the waiting tank. Control:
/// `refused_s` 0 -> a restart each plan (the check fails, seen).
fn demolish_row() -> (World, Stage) {
    use super::s10b_ai_demolish_tests::blocked;
    let b = blocked(|s| {
        let k = frontline_content::ItemKind::Satchel.index();
        for t in s.squad_types.iter_mut() {
            t.members.iter_mut().for_each(|m| m.items[k] = 0);
        }
    });
    let g = b.w.game().unwrap();
    let v = g.squads[&b.tank].members[0];
    let goal = b.w.state.entities.transform[v.index()].pos.xy();
    let stage = Stage {
        habit: crate::ai::AiHabit::Demolish,
        args: crate::ai::DriveArgs {
            squad: Some(b.eng),
            point: 0,
            others: vec![b.tank],
            at: None,
        },
        goal,
        ai: None,
    };
    (b.w, stage)
}

/// The Sweep row (Build D): an attacker, AI engineers and an enemy blast
/// on the attack line; the caller fixes the lane end off the map, so every
/// `ClearMines` is refused (BadSpot). Control: `refused_s` 0 -> a restart
/// each plan (the check fails, seen).
fn sweep_row() -> (World, Stage) {
    use super::s10b_ai_sweep_tests::{attack_world, blast, squads};
    let (mut w, mut a) = attack_world(|_| {});
    let (rifle, eng) = squads(&mut w, &mut a);
    blast(&mut w, a.short(70), 0);
    let off = c(-5, -5);
    let stage = Stage {
        habit: crate::ai::AiHabit::Sweep,
        args: crate::ai::DriveArgs {
            squad: Some(eng),
            point: 0,
            others: vec![rifle],
            at: Some(off),
        },
        goal: (a.short(80) + off).div_int(2),
        ai: None,
    };
    (w, stage)
}

/// The Fort row (Build B): an AI engineer with materials at its held,
/// trenched point; its AP field site lies under an own tank (inside
/// `vehicle_clear_m`), so every `Fortify` is refused (BadSpot). The match
/// AI is taken out; the row's state keeps its trench. Control: `refused_s`
/// 0 -> each refusal is followed by a restart the same plan (one job alive
/// to the end: the check fails, seen).
pub(crate) fn fort_row() -> (World, Stage) {
    use super::s10b_ai_lay_tests::{held, squad_at};
    let (mut w, h) = held(|s| {
        s.rules.ai.fort.wire = 0;
        s.rules.ai.obstacles.at_mines = 0;
    });
    let out = w.match_setup().unwrap().rules.ai.obstacles.ap_out_m as i32;
    let goal = h.mid() + h.enemy * Fx::from_int(out);
    spawn(
        &mut w,
        1,
        "ww2:usa_m4a3_75_squad",
        goal.x.floor_int(),
        goal.y.floor_int(),
    );
    let eng = squad_at(&mut w, "ww2:usa_engineer_squad", h.pos, 120);
    let ai = w.state.game.as_mut().unwrap().ai[1].take();
    let stage = Stage {
        habit: crate::ai::AiHabit::Fort,
        args: crate::ai::DriveArgs {
            squad: Some(eng),
            point: 0,
            ..Default::default()
        },
        goal,
        ai,
    };
    (w, stage)
}

/// The FrontAp row (S10c-ai Build B chunk 2, spec 5 "Generic"): the
/// held point with `front` on and its saved lane (found by a few seconds of
/// the match AI, then taken out); an own tank stands on the AP line's
/// middle (the spec's slope over 25 % needs relief the flat test map
/// lacks; any refusal drives L1-L4 the same, as the LogBarrier row), so
/// every `Fortify` is refused (BadSpot). The refusal is at the line's
/// middle. Control: `refused_s` 0 -> a restart each plan (the check fails,
/// seen).
fn front_ap_row() -> (World, Stage) {
    use super::s10b_ai_lay_tests::{held, squad_at};
    let (mut w, h) = held(|s| {
        s.rules.ai.fort.wire = 0;
        s.rules.ai.obstacles.at_mines = 0;
        s.rules.ai.obstacles.front = 1;
        // S10c-ai Build C (F1): the lone held point is the home point;
        // plain F1 keeps it a front point.
        s.rules.ai.obstacles.front_home = 1;
    });
    steps(&mut w, 3 * 30);
    let lane = w.game().unwrap().ai[1].as_ref().unwrap().front_lanes[0];
    let goal = (lane.ap[0] + lane.ap[1]).div_int(2);
    spawn(
        &mut w,
        1,
        "ww2:usa_m4a3_75_squad",
        goal.x.floor_int(),
        goal.y.floor_int(),
    );
    let eng = squad_at(&mut w, "ww2:usa_engineer_squad", h.pos, 120);
    let ai = w.state.game.as_mut().unwrap().ai[1].take();
    let stage = Stage {
        habit: crate::ai::AiHabit::Fort,
        args: crate::ai::DriveArgs {
            squad: Some(eng),
            point: 0,
            ..Default::default()
        },
        goal,
        ai,
    };
    (w, stage)
}

/// The LogBarrier row (Build C): an AI gun in Hold, an engineer with
/// materials beside it; an own tank stands on the barrier's site (40 m out
/// in the arc), so every `Fortify` is refused (BadSpot; the spec's "steep
/// slope" needs relief the flat test map lacks, and any refusal drives
/// L1-L4 the same). Control: `refused_s` 0 -> a restart each plan (the
/// check fails, seen).
fn barrier_row() -> (World, Stage) {
    use super::s10b_ai_lay_tests::squad_at;
    use super::s10b_ai_obst_tests::{gun_in_hold, obst_world};
    let (mut w, h) = obst_world(|_| {});
    let at = h.pos + c(20, 0);
    let gun = gun_in_hold(&mut w, at, at + h.enemy * Fx::from_int(100));
    let ai = w.state.game.as_mut().unwrap().ai[1].take();
    let j = ai.as_ref().unwrap().guns[&gun];
    let out = w.match_setup().unwrap().rules.ai.obstacles.obst_gun_m as i32;
    let goal = j.site + (j.facing - j.site).normalize_or_zero() * Fx::from_int(out);
    spawn(
        &mut w,
        1,
        "ww2:usa_m4a3_75_squad",
        goal.x.floor_int(),
        goal.y.floor_int(),
    );
    let eng = squad_at(&mut w, "ww2:usa_engineer_squad", at + c(-15, 0), 120);
    let stage = Stage {
        habit: crate::ai::AiHabit::Fort,
        args: crate::ai::DriveArgs {
            squad: Some(eng),
            point: 0,
            ..Default::default()
        },
        goal,
        ai,
    };
    (w, stage)
}

/// The Bunker row (Build C): Baupioniere with the kg at hand at the held
/// point; an own tank stands on the bunker site, so every `Fortify` is
/// refused (BadSpot, S10b R18; the spec's "slope over 15 %" needs relief the
/// flat test map lacks). Control: `refused_s` 0 -> a restart each plan (the
/// check fails, seen).
fn bunker_row() -> (World, Stage) {
    use super::s10b_ai_bunker_tests::{builder, bunker_site, bunker_world};
    let (mut w, h) = bunker_world(1, |_| {});
    let goal = bunker_site(&w, &h);
    spawn(
        &mut w,
        1,
        "ww2:usa_m4a3_75_squad",
        goal.x.floor_int(),
        goal.y.floor_int(),
    );
    let eng = builder(&mut w, &h, "ww2:ger_baupioniergruppe", h.pos);
    let ai = w.state.game.as_mut().unwrap().ai[1].take();
    let stage = Stage {
        habit: crate::ai::AiHabit::Fort,
        args: crate::ai::DriveArgs {
            squad: Some(eng),
            point: 0,
            ..Default::default()
        },
        goal,
        ai,
    };
    (w, stage)
}

/// Minutes each row runs (spec 5: 20 min).
const LIVENESS_MIN: u64 = 20;

/// One tick of a driven job, as the harness sees it.
#[derive(Clone, Copy, Debug, Default)]
struct Seen {
    tick: u64,
    /// A job of the row's kind is alive for the row's squad.
    alive: bool,
    /// What the hook returned this tick.
    end: Option<crate::ai::HabitEnd>,
    /// The newest `failed` tick of a refusal of the kind near the goal.
    refused_at: Option<u64>,
    refused_reorders: u32,
}

/// Limits of one check, in ticks.
struct Limits {
    job_max: u64,
    refused: u64,
    /// Plan cadence and hook latency allowed around each event.
    slack: u64,
}

/// Whether a job of `kind` is alive for `squad` (Done jobs wait for the
/// next plan to be dropped and do not count).
fn job_alive(ai: &AiState, kind: JobKind, squad: Option<SquadId>) -> bool {
    let Some(q) = squad else { return false };
    match kind {
        JobKind::Fort => ai.forts.get(&q).is_some_and(|j| j.phase != FortPhase::Done),
        JobKind::Sweep => ai
            .sweeps
            .get(&q)
            .is_some_and(|j| j.phase != SweepPhase::Done),
        JobKind::Demolish => ai
            .demolitions
            .get(&q)
            .is_some_and(|j| j.phase != DemolishPhase::Done),
        // A truck goal is alive while the truck's drop runs (F8, L5).
        JobKind::TruckGoal => ai
            .trucks
            .get(&q)
            .is_some_and(|j| j.phase == crate::ai::TruckPhase::Drop),
    }
}

/// Drives `stage` for `ticks` ticks on a caller-kept AI state and records
/// what the harness checks.
fn drive_row(w: &mut World, kind: JobKind, stage: &Stage, ticks: u64) -> Vec<Seen> {
    let near = w.match_setup().unwrap().rules.ai.obstacles.refused_m;
    let r2 = FxVec2::new(Fx::from_int(near as i32), Fx::ZERO).length_sq_raw();
    let mut ai = stage
        .ai
        .clone()
        .unwrap_or_else(|| AiState::new(w.state.tick));
    let mut out = Vec::with_capacity(ticks as usize);
    for _ in 0..ticks {
        let end = w.drive_ai_habit(&mut ai, PlayerId(1), stage.habit, &stage.args);
        let refused_at = ai
            .refused
            .iter()
            .filter(|r| r.kind == kind && (r.at - stage.goal).length_sq_raw() <= r2)
            .map(|r| r.failed)
            .max();
        out.push(Seen {
            tick: w.state.tick,
            alive: job_alive(&ai, kind, stage.args.squad),
            end,
            refused_at,
            refused_reorders: ai.counters.refused_reorders,
        });
    }
    out
}

/// L1-L4 on a trace: every job ends by its deadline (L1/L2); each end
/// without success adds exactly one refusal (L3); no job starts within
/// `refused` ticks of a refusal (L4) and at most one retry follows each;
/// `refused_reorders` stays 0; at least one refusal (the goal is
/// unreachable).
fn check_liveness(trace: &[Seen], l: &Limits) -> Result<(), String> {
    // Episodes: [start, end) of each alive run.
    let mut eps: Vec<(u64, u64)> = Vec::new();
    let mut open: Option<u64> = None;
    for s in trace {
        match (s.alive, open) {
            (true, None) => open = Some(s.tick),
            (false, Some(a)) => {
                eps.push((a, s.tick));
                open = None;
            }
            _ => {}
        }
    }
    if let Some(a) = open {
        let last = trace.last().map_or(a, |s| s.tick);
        if last - a > l.job_max + l.slack {
            return Err(format!(
                "job alive from {a} to the end {last}: past its deadline (L1)"
            ));
        }
    }
    // Refusal events: each rise of the newest refusal's tick.
    let mut refusals: Vec<u64> = Vec::new();
    let mut last: Option<u64> = None;
    for s in trace {
        if s.refused_at > last {
            refusals.extend(s.refused_at);
            last = s.refused_at;
        }
    }
    if refusals.is_empty() {
        return Err("no refusal: the unreachable goal was never given up (L3)".into());
    }
    let done = |e: u64| {
        trace.iter().any(|s| {
            s.end == Some(crate::ai::HabitEnd::Done)
                && s.tick + l.slack >= e
                && s.tick <= e + l.slack
        })
    };
    let mut failures = 0usize;
    for &(a, e) in &eps {
        if e - a > l.job_max + l.slack {
            return Err(format!(
                "job {a}..{e} outlived its deadline {} (L1/L2)",
                l.job_max
            ));
        }
        if done(e) {
            continue;
        }
        failures += 1;
        if !refusals
            .iter()
            .any(|&f| f + l.slack >= e && f <= e + l.slack)
        {
            return Err(format!(
                "job {a}..{e} ended without success and no refusal (L3)"
            ));
        }
    }
    if refusals.len() != failures {
        return Err(format!(
            "{} refusals for {failures} failed jobs (L3: one each)",
            refusals.len()
        ));
    }
    for (k, &f) in refusals.iter().enumerate() {
        let next = refusals.get(k + 1).copied().unwrap_or(u64::MAX);
        let starts: Vec<u64> = eps
            .iter()
            .map(|e| e.0)
            .chain(open)
            .filter(|&a| a > f && a < next)
            .collect();
        if let Some(&a) = starts.first()
            && a < f + l.refused
        {
            return Err(format!(
                "job restarted at {a}, {} ticks after the refusal at {f} (L4)",
                a - f
            ));
        }
        if starts.len() > 1 {
            return Err(format!(
                "{} retries after the refusal at {f} (at most one)",
                starts.len()
            ));
        }
    }
    match trace.last() {
        Some(s) if s.refused_reorders > 0 => {
            Err(format!("refused_reorders {}", s.refused_reorders))
        }
        _ => Ok(()),
    }
}

/// A dummy job kind's trace: a job alive `run` ticks, failing, then waiting
/// `wait` ticks before its retry, over `total` ticks; `refuses` writes the
/// refusal at each failure.
fn dummy_trace(run: u64, wait: u64, total: u64, refuses: bool) -> Vec<Seen> {
    let mut out = Vec::new();
    let mut refused_at = None;
    let mut start = 1;
    for t in 1..=total {
        let in_job = t >= start && t < start + run;
        if t == start + run {
            if refuses {
                refused_at = Some(t);
            }
            start = t + wait;
        }
        out.push(Seen {
            tick: t,
            alive: in_job,
            end: (t == start.saturating_sub(wait) && t > 1).then_some(crate::ai::HabitEnd::Failed),
            refused_at,
            refused_reorders: 0,
        });
    }
    out
}

/// L1-L4 for every job kind (spec 5): each row drives its kind at an
/// unreachable goal for 20 min. With the table empty (Build A) it checks
/// the harness: a dummy kind that fails, refuses and waits passes; the
/// control, a dummy kind that never refuses, fails it (seen); a job past
/// its deadline and a retry inside `refused_s` fail it too; and the drive
/// loop runs on a world (each habit `NotBuilt`, no job).
#[test]
fn ai_jobs_end_and_never_reorder_a_refused_goal() {
    use crate::ai::{AiHabit, DriveArgs, HabitEnd};
    let hz = 30;
    let l = Limits {
        job_max: 900 * hz,
        refused: 300 * hz,
        slack: 5 * hz,
    };
    let total = LIVENESS_MIN * 60 * hz;
    // A good dummy: fails after 600 s, waits refused_s, retries once.
    let good = dummy_trace(600 * hz, 300 * hz, total, true);
    check_liveness(&good, &l).unwrap();
    // Control: never refuses.
    let silent = dummy_trace(600 * hz, 300 * hz, total, false);
    assert!(
        check_liveness(&silent, &l).is_err(),
        "a kind that never refuses passed"
    );
    // Past its deadline; and a retry 60 s after a refusal.
    assert!(check_liveness(&dummy_trace(1000 * hz, 300 * hz, total, true), &l).is_err());
    assert!(check_liveness(&dummy_trace(600 * hz, 60 * hz, total, true), &l).is_err());

    // The drive loop on a world: no habit is built in A.
    let mut w = ai_world();
    let stage = Stage {
        habit: AiHabit::Fort,
        args: DriveArgs::default(),
        goal: c(0, 0),
        ai: None,
    };
    let tr = drive_row(&mut w, JobKind::Fort, &stage, 60);
    assert!(tr.iter().all(|s| !s.alive && s.refused_at.is_none()));
    assert!(
        tr.iter()
            .all(|s| matches!(s.end, None | Some(HabitEnd::NotBuilt)))
    );

    for &(kind, setup, barrier) in LIVENESS {
        let (mut w, stage) = setup();
        let s = w.match_setup().unwrap();
        let (hz, o) = (s.tick_hz as u64, s.rules.ai.obstacles.clone());
        let max_s = if barrier {
            o.barrier_job_max_s
        } else {
            o.job_max_s
        };
        let l = Limits {
            job_max: max_s as u64 * hz,
            refused: o.refused_s as u64 * hz,
            slack: 2 * o.recheck_s.max(5) as u64 * hz,
        };
        let tr = drive_row(&mut w, kind, &stage, LIVENESS_MIN * 60 * hz);
        if let Err(e) = check_liveness(&tr, &l) {
            panic!("{kind:?}: {e}");
        }
    }
}

// ------------------------------------------------------- Fix (review) --

/// Review finding 1: a save made between a sweeper's or demolisher's death
/// (or the demolition tank's loss) and the next AI plan loads, like the
/// S10a fort jobs (`validate_fort`), and the next plan drops the job.
/// Control: `validate_s10b` keys back to `own_with` (an own living squad)
/// and the vehicle back to `own_vehicle` -> each of the three saves fails
/// with "AI state out of range".
#[test]
fn saves_load_between_a_job_squads_loss_and_the_next_plan() {
    type Kill = fn(&mut World, &Busy);
    let cases: [(&str, Kill); 3] = [
        ("sweeper wiped out", |w, b| {
            for m in w.game().unwrap().squads[&b.sweep].members.clone() {
                assert!(w.kill_soldier(m));
            }
            assert!(!w.game().unwrap().squads.contains_key(&b.sweep));
        }),
        ("demolisher wiped out", |w, b| {
            for m in w.game().unwrap().squads[&b.demo].members.clone() {
                assert!(w.kill_soldier(m));
            }
            assert!(!w.game().unwrap().squads.contains_key(&b.demo));
        }),
        ("demolition tank wrecked", |w, b| {
            assert!(w.wreck_vehicle(b.vehicle));
            let g = w.game().unwrap();
            assert!(!g.squads.values().any(|q| q.members.contains(&b.vehicle)));
        }),
    ];
    for (what, kill) in cases {
        let (mut w, b) = ai_busy();
        kill(&mut w, &b);
        let ai = w.game().unwrap().ai[1].as_ref().unwrap();
        assert!(ai.sweeps.contains_key(&b.sweep) && ai.demolitions.contains_key(&b.demo));
        let mut back = World::load(&w.save(), 1).unwrap_or_else(|e| panic!("{what}: {e:?}"));
        // The next plans drop the job of the lost squad or tank.
        let s = back.match_setup().unwrap();
        let d = s.rules.difficulty.get(s.players[1].difficulty);
        let plan = d.replan_ticks.max(1) as u64;
        steps(&mut back, 2 * plan + 2);
        let ai = back.game().unwrap().ai[1].as_ref().unwrap();
        match what {
            "sweeper wiped out" => assert!(!ai.sweeps.contains_key(&b.sweep), "{what}"),
            _ => assert!(!ai.demolitions.contains_key(&b.demo), "{what}"),
        }
    }
}

/// Review finding 2: a demolition in Charge without a piece at `done`
/// (none listed there, or `done` 3) ends at the next plan instead of
/// idling until L1 or panicking on `pieces[3]`. Control: `run_demolish`'s
/// Charge back to `job.pieces[job.done as usize]?` -> the first case keeps
/// its job, the second panics (index out of bounds).
#[test]
fn demolition_in_charge_without_a_piece_ends() {
    for done in [2u8, 3] {
        let (mut w, b) = ai_busy();
        assert!(w.with_ai_state(1, |ai| {
            let j = ai.demolitions.get_mut(&b.demo).unwrap();
            if done == 3 {
                j.pieces = [j.pieces[0]; 3];
            }
            j.done = done;
        }));
        let s = w.match_setup().unwrap();
        let d = s.rules.difficulty.get(s.players[1].difficulty);
        let plan = d.replan_ticks.max(1) as u64;
        steps(&mut w, 2 * plan + 2);
        let ai = w.game().unwrap().ai[1].as_ref().unwrap();
        assert!(
            !ai.demolitions.contains_key(&b.demo),
            "done {done}: job kept"
        );
    }
}

/// Fix 21: a lone truck that once gave up its own base (its Return failed)
/// is sent back again after `refused_s` (300 s): the base refusal ages, the
/// job's own memory of the base included. Before the fix it stayed in
/// Follow, empty, for the match (seen failing as "never sent back").
#[test]
fn a_base_give_up_ages_so_a_lone_truck_returns() {
    use crate::ai::{AiTruck, TruckPhase};
    let mut w = ai_world();
    let base = w.game().unwrap().base_of(PlayerId(1));
    let truck = spawn(&mut w, 1, "ww2:usa_cckw_squad", 500, 500);
    let v = w.game().unwrap().squads[&truck].members[0];
    w.edit_vehicle(v, |vs| vs.cargo_g = [0, 0, 0]);
    let tick = w.state.tick;
    let ai = ai_of(&mut w.state);
    let mut gave_up = [None; crate::ai::GAVE_UP_GOALS];
    gave_up[0] = Some(base);
    ai.trucks.insert(
        truck,
        AiTruck {
            phase: TruckPhase::Follow,
            since: tick,
            gave_up,
            ..Default::default()
        },
    );
    ai.refuse(JobKind::TruckGoal, base, tick);
    let phase = |w: &mut World| ai_of(&mut w.state).trucks.get(&truck).map(|j| j.phase);
    steps(&mut w, 60 * 30);
    assert_eq!(
        phase(&mut w),
        Some(TruckPhase::Follow),
        "young refusal: kept off"
    );
    let mut back = false;
    for _ in 0..300 {
        steps(&mut w, 30);
        back |= matches!(phase(&mut w), Some(TruckPhase::Return | TruckPhase::Load));
        if back {
            break;
        }
    }
    assert!(back, "never sent back");
}
