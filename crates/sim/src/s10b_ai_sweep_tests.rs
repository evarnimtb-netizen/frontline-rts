//! S10b-ai Build D tests (docs/slices/S10b-ai.md R9, R10, spec 5 "Sweep"):
//! the AI marks suspects from enemy mine blasts, sweeps the attack lane
//! before its attackers walk it, and its sweep jobs end on every path.

use super::s6_tests::{flat_world_with, spawn, steps};
use super::*;
use crate::ai::{AiHabit, AiState, DriveArgs, HabitEnd, SweepPhase};
use crate::fort::MineBlast;
use crate::game::SquadId;
use frontline_content::setup::Capability;

const ENG: &str = "ww2:usa_engineer_squad";
const RIFLE: &str = "ww2:usa_rifle_squad";

/// The attack geometry: the enemy point, the unit vector from AI player
/// 1's base toward it, and its perpendicular.
pub(crate) struct Attack {
    pub pos: FxVec2,
    pub dir: FxVec2,
    pub side: FxVec2,
}

impl Attack {
    /// `m` metres short of the point on the line from the AI base.
    pub fn short(&self, m: i32) -> FxVec2 {
        self.pos - self.dir * Fx::from_int(m)
    }
}

fn at(w: &mut World, card: &str, p: FxVec2) -> SquadId {
    spawn(w, 1, card, p.x.floor_int(), p.y.floor_int())
}

/// AI player 1 (no start squads, no buys, no match AI: the hook drives it) attacks
/// the map's only point, held by player 0; engineers can sweep. `edit`
/// runs last on the setup.
pub(crate) fn attack_world(
    edit: impl FnOnce(&mut frontline_content::MatchSetup),
) -> (World, Attack) {
    let mut w = flat_world_with(1, |s| {
        // The hook drives player 1 on a caller-kept state: no match AI.
        s.players[1].is_ai = false;
        s.map.points.truncate(1);
        for f in s.factions.iter_mut() {
            f.start_squads.clear();
        }
        for t in s.squad_types.iter_mut() {
            t.ai_weight = 0;
            if t.id.contains("engineer") {
                t.caps |= Capability::ClearMines.bit();
            }
        }
        edit(s);
    });
    let g = w.state.game.as_mut().unwrap();
    g.points[0].owner = Some(PlayerId(0));
    let pos: FxVec2 = g.setup.map.points[0].pos.into();
    let dir = (pos - g.base_of(PlayerId(1))).normalize_or_zero();
    let a = Attack {
        pos,
        dir,
        side: dir.perp(),
    };
    // 20 s in, so a blast can be older than `suspect_s`.
    steps(&mut w, 600);
    (w, a)
}

/// A blast of player 0's mine at `p`, `ago` ticks before now.
pub(crate) fn blast(w: &mut World, p: FxVec2, ago: u64) {
    let tick = w.state.tick;
    w.edit_fort(|f| {
        f.blasts.push(MineBlast {
            pos: p,
            owner: PlayerId(0),
            tick: tick.saturating_sub(ago),
        })
    });
}

/// The rows' squads: attacker 120 m short of the point, the engineers 130 m
/// short and 10 m aside; `a`'s line then runs from the attacker's leader
/// (the lane's line) so `short` is on it.
pub(crate) fn squads(w: &mut World, a: &mut Attack) -> (SquadId, SquadId) {
    let rifle = at(w, RIFLE, a.short(120));
    let eng = at(w, ENG, a.short(130) + a.side * Fx::from_int(10));
    a.dir = (a.pos - lead(w, rifle)).normalize_or_zero();
    a.side = a.dir.perp();
    (rifle, eng)
}

fn args(eng: SquadId, rifle: SquadId) -> DriveArgs {
    DriveArgs {
        squad: Some(eng),
        point: 0,
        others: vec![rifle],
        at: None,
    }
}

/// Drives the Sweep habit for up to `ticks`; the end, if any.
fn drive(w: &mut World, ai: &mut AiState, a: &DriveArgs, ticks: u64) -> Option<HabitEnd> {
    (0..ticks).find_map(|_| w.drive_ai_habit(ai, PlayerId(1), AiHabit::Sweep, a))
}

fn lead(w: &World, q: SquadId) -> FxVec2 {
    let g = w.game().unwrap();
    w.state.entities.transform[g.squads[&q].lead(&w.state.entities).index()]
        .pos
        .xy()
}

/// R9: an enemy blast 70 m short of the point, on the attack line, wants a
/// sweep: the job starts with its lane 10 m short of the blast. Control:
/// `suspect_s` 1 with the blast 10 s old -> no want, no job (seen).
#[test]
fn ai_marks_suspect_from_blast() {
    for control in [false, true] {
        let (mut w, mut a) = attack_world(|s| {
            if control {
                s.rules.ai.obstacles.suspect_s = 1;
            }
        });
        let (rifle, eng) = squads(&mut w, &mut a);
        blast(&mut w, a.short(70), 300);
        let mut ai = AiState::new(w.state.tick);
        drive(&mut w, &mut ai, &args(eng, rifle), 30);
        if control {
            assert_eq!(ai.counters.sweep_wanted, 0);
            assert!(ai.sweeps.is_empty());
            continue;
        }
        assert!(ai.counters.sweep_wanted >= 1);
        assert_eq!(ai.counters.sweeps_started, 1);
        let j = &ai.sweeps[&eng];
        // `a` 10 m short of the suspect, never behind the attacker.
        let ahead = (a.pos - j.a).dot(a.dir);
        assert!(
            (ahead - Fx::from_int(80)).abs() < Fx::from_int(3),
            "{:?} {:?}",
            j.a,
            a.short(80)
        );
        assert!(((j.b - j.a).length() - Fx::from_int(50)).abs() < Fx::from_int(1));
        assert_eq!(ai.suspects_seen, 1);
    }
}

/// R10: the sweeper sweeps the lane while the attacker holds short of it;
/// the lane ends clear (`sweeps_done`), the lane's mine lifted if found; the
/// attacker never stood past the lane start before. Control: `sweep` 0 ->
/// no job, the mine stays (seen).
#[test]
fn ai_sweeps_suspect_lane_before_attack() {
    for control in [false, true] {
        let (mut w, mut a) = attack_world(|s| {
            if control {
                s.rules.ai.obstacles.sweep = 0;
            }
        });
        let (rifle, eng) = squads(&mut w, &mut a);
        blast(&mut w, a.short(70), 30);
        let ap = fort_id(&w, "ww2:ap_minefield");
        let mine = w.lay_mine_now(ap, a.short(60), PlayerId(0)).unwrap();
        let mut ai = AiState::new(w.state.tick);
        let args = args(eng, rifle);
        let hz = w.match_setup().unwrap().tick_hz as u64;
        let mut past = false;
        let mut end = None;
        for _ in 0..600 * hz {
            if let Some(e) = w.drive_ai_habit(&mut ai, PlayerId(1), AiHabit::Sweep, &args) {
                end = Some(e);
                break;
            }
            if let Some(j) = ai.sweeps.get(&eng).filter(|j| j.phase != SweepPhase::Done) {
                past |= (lead(&w, rifle) - j.a).dot(a.dir) > Fx::ZERO;
            }
        }
        let lifted = !w.game().unwrap().fort().mines.contains_key(&mine);
        if control {
            assert!(ai.sweeps.is_empty() && !lifted && end.is_none());
            continue;
        }
        assert_eq!(end, Some(HabitEnd::Done), "{:?}", ai.sweeps);
        assert_eq!(ai.counters.sweeps_done, 1);
        // Swept: the mine lifted, or never found (detection is a roll).
        let found = w.game().unwrap().fort().accounts[1].mines_found;
        assert_eq!(lifted, found > 0);
        assert_eq!(w.game().unwrap().fort().accounts[1].sweeps, 1);
        assert!(!past, "the attacker went past the lane start");
    }
}

/// Spec 5 "Sweep / target gone": with a known mine in the lane and no
/// suspect, the job ends when the lane is clear and the swept entry keeps
/// a second job off the lane. Control: `sweep` 0 -> no job, no
/// `sweeps_done` (seen; `lift_s` 600 was no control: the lift still ends
/// within the time).
#[test]
fn ai_sweep_ends_when_lane_clear() {
    for control in [false, true] {
        let (mut w, mut a) = attack_world(|s| {
            if control {
                s.rules.ai.obstacles.sweep = 0;
            }
        });
        let (rifle, eng) = squads(&mut w, &mut a);
        let ap = fort_id(&w, "ww2:ap_minefield");
        let mine = w.lay_mine_now(ap, a.short(70), PlayerId(0)).unwrap();
        w.edit_fort(|f| f.mines.get_mut(&mine).unwrap().known |= 0b10);
        let mut ai = AiState::new(w.state.tick);
        let hz = w.match_setup().unwrap().tick_hz as u64;
        // The swept lane takes about 100 s.
        let end = drive(&mut w, &mut ai, &args(eng, rifle), 150 * hz);
        if control {
            assert_ne!(end, Some(HabitEnd::Done));
            assert_eq!(ai.counters.sweeps_done, 0);
            continue;
        }
        assert_eq!(end, Some(HabitEnd::Done));
        assert!(
            !w.game().unwrap().fort().mines.contains_key(&mine),
            "lifted"
        );
        assert_eq!(ai.counters.sweeps_done, 1);
        assert!(ai.sweeps[&eng].phase == SweepPhase::Done);
        drive(&mut w, &mut ai, &args(eng, rifle), 60 * hz);
        assert_eq!(ai.counters.sweeps_started, 1, "no second sweep");
    }
}

/// Spec 5 "Sweep / another squad on it": two engineer squads, one lane: one
/// job, never two on the lane, also after the first is swept. Control:
/// `MAX_SWEEPS`-style cap off is not a key; the control is the plan with
/// the swept entry dropped at once (`suspect_s` 1 with the blast fresh
/// each minute): a second sweep of the same lane then starts (seen).
/// Review finding 19: a blast later than the sweep reopens the lane, so
/// the run's blast is fresh only until the first sweep ends; the control
/// keeps it fresh every 30 s.
#[test]
fn ai_two_sweepers_never_share_lane() {
    for control in [false, true] {
        let (mut w, mut a) = attack_world(|s| {
            if control {
                s.rules.ai.obstacles.suspect_s = 1;
            }
        });
        let (rifle, eng) = squads(&mut w, &mut a);
        let eng2 = at(&mut w, ENG, a.short(130) - a.side * Fx::from_int(10));
        let hz = w.match_setup().unwrap().tick_hz as u64;
        let mut ai = AiState::new(w.state.tick);
        let mut args = args(eng, rifle);
        args.others.push(eng2);
        let mut most = 0;
        for k in 0..240 * hz {
            if k % (30 * hz) == 0 && (control || ai.counters.sweeps_done == 0) {
                blast(&mut w, a.short(70), 0);
            }
            w.drive_ai_habit(&mut ai, PlayerId(1), AiHabit::Sweep, &args);
            let live = ai
                .sweeps
                .values()
                .filter(|j| j.phase != SweepPhase::Done)
                .count();
            most = most.max(live);
        }
        assert!(most <= 1, "two sweeps on one lane");
        if control {
            assert!(
                ai.counters.sweeps_started >= 2,
                "{}",
                ai.counters.sweeps_started
            );
        } else {
            assert_eq!(ai.counters.sweeps_started, 1);
        }
    }
}

/// Spec 5 "Sweep / squad dead": the sweeper killed mid-job ends the job
/// (no refusal; the attacker no longer holds). Control: one man killed
/// (the squad stays over 60 %) -> the job lives (seen).
#[test]
fn ai_sweep_ends_when_sweeper_dies() {
    for control in [false, true] {
        let (mut w, mut a) = attack_world(|_| {});
        let (rifle, eng) = squads(&mut w, &mut a);
        blast(&mut w, a.short(70), 30);
        let mut ai = AiState::new(w.state.tick);
        let args = args(eng, rifle);
        drive(&mut w, &mut ai, &args, 60);
        assert!(ai.sweeps.contains_key(&eng));
        let men = w.game().unwrap().squads[&eng].members.clone();
        let n = if control { 1 } else { men.len() };
        for m in men.into_iter().take(n) {
            assert!(w.kill_soldier(m));
        }
        drive(&mut w, &mut ai, &args, 60);
        if control {
            assert!(
                ai.sweeps
                    .get(&eng)
                    .is_some_and(|j| j.phase != SweepPhase::Done)
            );
            continue;
        }
        assert!(!ai.sweeps.contains_key(&eng));
        assert!(ai.refused.is_empty());
        assert_eq!(ai.counters.s10b_refused, 0);
    }
}

/// Spec 5 "Sweep / vehicle in footprint" and "save/load mid-job": an own
/// tank parked in the lane; the sweep goes on and ends swept, the tank
/// unhurt, no mine goes off; a save and load mid-job continues exactly
/// (same state hash after 60 s). Control: `sweep` 0 -> the lane's mine
/// is not lifted (seen).
#[test]
fn ai_sweep_beside_own_tank() {
    for control in [false, true] {
        let (mut w, mut a) = attack_world(|s| {
            if control {
                s.rules.ai.obstacles.sweep = 0;
            }
        });
        let (rifle, eng) = squads(&mut w, &mut a);
        let tank_at = a.short(55) + a.side * Fx::from_int(6);
        let tank = at(&mut w, "ww2:usa_m4a3_75_squad", tank_at);
        let v = w.game().unwrap().squads[&tank].members[0];
        let ap = fort_id(&w, "ww2:ap_minefield");
        let mine = w.lay_mine_now(ap, a.short(65), PlayerId(0)).unwrap();
        blast(&mut w, a.short(70), 30);
        let hz = w.match_setup().unwrap().tick_hz as u64;
        let mut ai = AiState::new(w.state.tick);
        let args = args(eng, rifle);
        // Mid-job: a save, loaded, runs on the same.
        let ran = drive(&mut w, &mut ai, &args, 40 * hz);
        if !control {
            assert!(ran.is_none() && ai.sweeps.contains_key(&eng));
            let mut w2 = World::load(&w.save(), 1).unwrap();
            let mut ai2 = ai.clone();
            drive(&mut w, &mut ai, &args, 60 * hz);
            drive(&mut w2, &mut ai2, &args, 60 * hz);
            assert_eq!(w.state_hash(), w2.state_hash());
            assert_eq!(ai, ai2);
        }
        let end = drive(&mut w, &mut ai, &args, 400 * hz);
        let g = w.game().unwrap();
        let lifted = !g.fort().mines.contains_key(&mine);
        let blasts = g.fort().blasts.len();
        if control {
            assert!(!lifted);
            continue;
        }
        assert_eq!(end, Some(HabitEnd::Done));
        assert!(lifted);
        assert_eq!(blasts, 1, "only the staged blast");
        let hp = &g.vehicles[&v];
        assert!(!hp.immobile(&g.setup.vehicles[hp.type_idx as usize]));
    }
}

/// Blueprint `id`'s index.
fn fort_id(w: &World, id: &str) -> u16 {
    w.match_setup()
        .unwrap()
        .forts
        .iter()
        .position(|f| f.id == id)
        .unwrap() as u16
}

/// Build E (S10b-ai-D2): after the sweep, the attackers walk the swept lane
/// in single file on its centre line (`AiHabit::LaneFile` ends Done once
/// all are past the lane end); no man strays more than 1.5 m from the
/// centre line while along the lane from 5 m in (the lane starts 10 m short
/// of the mine; men of the squad's hold line may still step a little past
/// the start before the first file order lands). Control: `lane_file` 0 -> the habit
/// ends Failed at once and the squad's own walk (a `Move` onto the point,
/// as the fortify row's advance) spreads its men wider than that (seen).
#[test]
fn ai_attackers_file_through_swept_lane() {
    use crate::command::{Command, CommandKind};
    for control in [false, true] {
        let (mut w, mut a) = attack_world(|s| {
            if control {
                s.rules.ai.obstacles.lane_file = 0;
            }
        });
        let (rifle, eng) = squads(&mut w, &mut a);
        let ap = fort_id(&w, "ww2:ap_minefield");
        let mine = w.lay_mine_now(ap, a.short(70), PlayerId(0)).unwrap();
        w.edit_fort(|f| f.mines.get_mut(&mine).unwrap().known |= 0b10);
        let mut ai = AiState::new(w.state.tick);
        let hz = w.match_setup().unwrap().tick_hz as u64;
        let args = args(eng, rifle);
        assert_eq!(
            drive(&mut w, &mut ai, &args, 150 * hz),
            Some(HabitEnd::Done)
        );
        let (la, lb) = (ai.sweeps[&eng].a, ai.sweeps[&eng].b);
        let d = (lb - la).normalize_or_zero();
        let len = (lb - la).length();
        let mut worst = Fx::ZERO;
        let mut look = |w: &World| {
            let g = w.game().unwrap();
            for m in &g.squads[&rifle].members {
                let r = w.state.entities.transform[m.index()].pos.xy() - la;
                let along = r.dot(d);
                if along >= Fx::from_int(5) && along <= len {
                    worst = worst.max(r.dot(d.perp()).abs());
                }
            }
        };
        let mut end = None;
        for _ in 0..120 * hz {
            end = w.drive_ai_habit(&mut ai, PlayerId(1), AiHabit::LaneFile, &args);
            look(&w);
            if end.is_some() {
                break;
            }
        }
        if control {
            assert_eq!(end, Some(HabitEnd::Failed));
            w.step(&[Command {
                player: PlayerId(1),
                seq: 1,
                kind: CommandKind::Move {
                    squad: rifle,
                    target: a.pos,
                },
            }]);
            for _ in 0..120 * hz {
                w.step(&[]);
                look(&w);
            }
            assert!(worst > Fx::from_milli(1500), "control in file: {worst:?}");
            continue;
        }
        assert_eq!(end, Some(HabitEnd::Done));
        assert!(
            worst <= Fx::from_milli(1500),
            "a man left the centre line: {worst:?}"
        );
        // Every man is past the lane end.
        let g = w.game().unwrap();
        for m in &g.squads[&rifle].members {
            let r = w.state.entities.transform[m.index()].pos.xy() - la;
            assert!(r.dot(d) >= len, "{m:?} not past");
        }
    }
}

/// Sweeps the rows' lane (a known AP mine 70 m short of the point) with the
/// Sweep habit; the world, the hook's state, the args and the lane.
fn swept_lane(
    edit: impl FnOnce(&mut frontline_content::MatchSetup),
) -> (World, AiState, DriveArgs, SquadId, FxVec2, FxVec2) {
    let (mut w, mut a) = attack_world(edit);
    let (rifle, eng) = squads(&mut w, &mut a);
    let ap = fort_id(&w, "ww2:ap_minefield");
    let mine = w.lay_mine_now(ap, a.short(70), PlayerId(0)).unwrap();
    w.edit_fort(|f| f.mines.get_mut(&mine).unwrap().known |= 0b10);
    let mut ai = AiState::new(w.state.tick);
    let hz = w.match_setup().unwrap().tick_hz as u64;
    let args = args(eng, rifle);
    assert_eq!(
        drive(&mut w, &mut ai, &args, 150 * hz),
        Some(HabitEnd::Done)
    );
    let (la, lb) = (ai.sweeps[&eng].a, ai.sweeps[&eng].b);
    (w, ai, args, eng, la, lb)
}

/// Review finding 18: a man who reaches the spot beyond the lane end and
/// takes arrival cover (a trench across the lane 6 m short of that spot,
/// so his cover slot lies back inside the lane) counts as through; he is
/// not sent to the spot again every plan; the LaneFile habit ends Done
/// within 240 s (under the filing bound) with every man past the lane end
/// or in cover by the spot. Failing control: the pre-fix `lane_file` (past
/// only at `along >= len + 2`, `going` read from the cover slot): men walk
/// to the spot, back to the trench and again; the habit ended with a man
/// back at 35 m of the 50 m lane (seen).
#[test]
fn ai_lane_file_ends_when_the_last_spot_takes_cover() {
    let (mut w, mut ai, args, _, la, lb) = swept_lane(|_| {});
    let d = (lb - la).normalize_or_zero();
    let len = (lb - la).length();
    let mid = la + d * (len - Fx::ONE);
    let side = d.perp() * Fx::from_int(6);
    assert!(w.dig_trench_now(&[mid - side, mid + side], 1));
    let hz = w.match_setup().unwrap().tick_hz as u64;
    let mut end = None;
    for _ in 0..240 * hz {
        end = w.drive_ai_habit(&mut ai, PlayerId(1), AiHabit::LaneFile, &args);
        if end.is_some() {
            break;
        }
    }
    assert_eq!(end, Some(HabitEnd::Done), "still filing after 240 s");
    // Every man stands past the lane end or in cover near the spot.
    // The spot beyond the lane end (`FILE_PAST_M` 5 m).
    let spot = lb + d * Fx::from_int(5);
    let g = w.game().unwrap();
    for m in &g.squads[&args.others[0]].members {
        let p = w.state.entities.transform[m.index()].pos.xy();
        let r = (p - la).dot(d);
        assert!(
            r >= len || (p - spot).length() <= Fx::from_int(11),
            "{m:?} at {r:?} of {len:?}"
        );
    }
}

/// Review finding 18 (bound): a squad files through a lane for at most
/// three nominal file walks plus 60 s after the sweep ended; then the habit
/// ends (the squad gets its normal order). With the Done entry 400 s old
/// (`suspect_s` 3600 keeps it) the habit ends Done within 5 s; the
/// pre-fix code (no bound) was still filing (seen). Control: the sweep just
/// ended -> still filing after 5 s.
#[test]
fn ai_lane_file_has_a_time_bound() {
    for control in [false, true] {
        let (mut w, mut ai, args, eng, _, _) = swept_lane(|s| {
            s.rules.ai.obstacles.suspect_s = 3600;
        });
        let hz = w.match_setup().unwrap().tick_hz as u64;
        // 400 s pass without plans; the control's sweep ended just now.
        for _ in 0..400 * hz {
            w.step(&[]);
        }
        if control {
            ai.sweeps.get_mut(&eng).unwrap().since = w.state.tick;
        }
        let hz5 = 5 * hz;
        let end =
            (0..hz5).find_map(|_| w.drive_ai_habit(&mut ai, PlayerId(1), AiHabit::LaneFile, &args));
        if control {
            assert_eq!(end, None, "control: filing");
        } else {
            assert_eq!(end, Some(HabitEnd::Done), "bound: the squad stops filing");
        }
    }
}

/// Review finding 19: a swept (Done) lane is checked again. After the
/// sweep, (a) a new enemy blast inside the lane, (b) an enemy mine laid
/// and known inside it, (c) a new blast 10 m beside it each drop the Done
/// entry and start a second sweep (the attackers hold again, no single
/// file through the lane). Control (d): a blast inside the lane older than
/// the sweep's end -> the Done entry stays, no second sweep. Failing
/// control: the pre-fix code keeps the Done entry in (a)-(c) (seen).
#[test]
fn ai_swept_lane_is_checked_again() {
    for case in 0..4 {
        let (mut w, mut ai, args, eng, la, lb) = swept_lane(|_| {});
        let hz = w.match_setup().unwrap().tick_hz as u64;
        let d = (lb - la).normalize_or_zero();
        let mid = la + d * Fx::from_int(20);
        let since = ai.sweeps[&eng].since;
        let started = ai.counters.sweeps_started;
        match case {
            0 => blast(&mut w, mid, 0),
            1 => {
                let ap = fort_id(&w, "ww2:ap_minefield");
                let mine = w.lay_mine_now(ap, mid, PlayerId(0)).unwrap();
                w.edit_fort(|f| f.mines.get_mut(&mine).unwrap().known |= 0b10);
            }
            2 => blast(&mut w, mid + d.perp() * Fx::from_int(10), 0),
            _ => {
                let ago = w.state.tick - since + 1;
                blast(&mut w, mid, ago)
            }
        }
        for _ in 0..10 * hz {
            w.drive_ai_habit(&mut ai, PlayerId(1), AiHabit::Sweep, &args);
            if ai.counters.sweeps_started > started {
                break;
            }
        }
        let again = ai.counters.sweeps_started > started;
        if case == 3 {
            assert!(!again, "control: no second sweep");
            assert_eq!(ai.sweeps[&eng].phase, SweepPhase::Done);
        } else {
            assert!(again, "case {case}: no second sweep: {:?}", ai.sweeps);
            assert!(ai.sweeps.values().all(|j| j.phase != SweepPhase::Done));
        }
    }
}
