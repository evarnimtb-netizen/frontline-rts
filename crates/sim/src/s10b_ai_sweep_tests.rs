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
            if k % (30 * hz) == 0 {
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
