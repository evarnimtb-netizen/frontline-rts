//! S10b-ai tests (docs/slices/S10b-ai.md R1-R4, R8, L1-L4; Build B chunk 2):
//! the AI lays AP and AT fields at its held, trenched points as fort jobs
//! within its caps and its own materials budget, and remembers the spots it
//! was refused.

use super::s6_tests::{flat_world_with, members, run, spawn, steps};
use super::*;
use crate::ai::{AiState, AiTrench, FortPhase, JobKind};
use crate::command::CommandKind;
use crate::game::SquadId;

const ENG: &str = "ww2:usa_engineer_squad";

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

fn play(w: &mut World, secs: u64) {
    steps(w, secs * 30);
}

fn ai_ro(w: &World) -> &AiState {
    w.game().unwrap().ai[1].as_ref().unwrap()
}

/// What a staged held point looks like: the point, its trench line, the
/// unit direction to the enemy and along the trench.
#[derive(Clone, Copy)]
pub(crate) struct Held {
    pub pos: FxVec2,
    pub line: [FxVec2; 2],
    pub enemy: FxVec2,
    pub along: FxVec2,
}

impl Held {
    pub fn mid(&self) -> FxVec2 {
        (self.line[0] + self.line[1]).div_int(2)
    }
}

/// AI player 1 (USA, no start squads, no buys) holds the map's only point
/// with a finished AI trench across the way to player 0's base (as
/// `s10a_ai_tests::held`), S10a `works` off. `edit` runs last.
pub(crate) fn held(edit: impl FnOnce(&mut frontline_content::MatchSetup)) -> (World, Held) {
    let mut w = flat_world_with(1, |s| {
        s.players[1].is_ai = true;
        s.map.points.truncate(1);
        let f = s.players[1].faction as usize;
        s.factions[f].start_squads.clear();
        s.squad_types.iter_mut().for_each(|t| t.ai_weight = 0);
        // The S10a works (nest, dugout) would take the builder first.
        s.rules.ai.fort.works = 0;
        edit(s);
    });
    let h = {
        let g = w.state.game.as_mut().unwrap();
        g.points[0].owner = Some(PlayerId(1));
        let s = &g.setup;
        let pos: FxVec2 = s.map.points[0].pos.into();
        let dir = (g.base_of(PlayerId(0)) - pos).normalize_or_zero();
        let mid = pos + dir * Fx::from(s.rules.ai.dig_offset);
        let half = dir.perp() * Fx::from(s.rules.ai.dig_length).div_int(2);
        let line = [mid - half, mid + half];
        g.ai[1]
            .as_mut()
            .unwrap()
            .trenches
            .push(AiTrench { point: 0, line });
        let along = (line[1] - line[0]).normalize_or_zero();
        let enemy = (mid - pos).normalize_or_zero();
        Held {
            pos,
            line,
            enemy,
            along,
        }
    };
    let depth = Fx::from(w.match_setup().unwrap().rules.dig.depth) + Fx::from_milli(300);
    let tick = w.state.tick;
    w.state
        .terrain
        .crater_cut(h.mid(), Fx::from_int(3), depth, tick);
    (w, h)
}

/// A player 1 squad at `at` carrying `kg`.
pub(crate) fn squad_at(w: &mut World, card: &str, at: FxVec2, kg: u32) -> SquadId {
    let id = spawn(w, 1, card, at.x.floor_int(), at.y.floor_int());
    if kg > 0 {
        w.edit_fort(|f| {
            f.carried_g.insert(id, kg * 1000);
        });
    }
    id
}

/// Player 1's mines of blueprint `f`.
fn mines_of(w: &World, f: u16) -> Vec<FxVec2> {
    w.game()
        .unwrap()
        .fort()
        .mines
        .values()
        .filter(|m| m.owner == PlayerId(1) && m.fort == f)
        .map(|m| m.pos)
        .collect()
}

/// Player 1's AI fort jobs on blueprint `f` (parked ones too).
fn jobs_of(w: &World, f: u16) -> usize {
    ai_ro(w).forts.values().filter(|j| j.fort == f).count()
}

/// The AP site of `h` (spec R1): `out` m out, 30 m along the trench.
fn ap_site(h: &Held, out: i32) -> [FxVec2; 2] {
    let c = h.mid() + h.enemy * Fx::from_int(out);
    [
        c - h.along * Fx::from_int(15),
        c + h.along * Fx::from_int(15),
    ]
}

/// Builds player 1's field of blueprint `id` on `line` at once.
fn prebuild(w: &mut World, id: &str, line: [FxVec2; 2]) {
    let f = fort(w, id);
    assert!(w.build_now(f, &line, PlayerId(1)), "{id} on {line:?}");
}

/// Plays up to `secs`, stopping once `done` holds; whether it did.
fn play_until(w: &mut World, secs: u64, mut done: impl FnMut(&World) -> bool) -> bool {
    for _ in 0..secs {
        if done(w) {
            return true;
        }
        play(w, 1);
    }
    done(w)
}

/// Distance (m, rounded down) of `p` from the line through `a` along `d`.
fn off_m(p: FxVec2, a: FxVec2, d: FxVec2) -> i32 {
    let v = p - a;
    (v.x * d.y - v.y * d.x).abs().floor_int()
}

/// Distance (m, rounded down) of `p` along `d` from `a`.
fn along_m(p: FxVec2, a: FxVec2, d: FxVec2) -> i32 {
    let v = p - a;
    (v.x * d.x + v.y * d.y).floor_int()
}

/// R1: with the point's wire standing, the AI lays a 30 m AP field
/// parallel to the trench `wire_out_m + ap_beyond_wire_m` (35 + 5) out.
/// Control: `[ai.obstacles] mines` 0 -> no AP job, no mine (seen failing
/// as "the AP field stands" when asserted with the switch off).
#[test]
fn ai_lays_ap_field_beyond_its_wire() {
    let lay = |mines: u32| {
        let (mut w, h) = held(|s| {
            s.rules.ai.obstacles.mines = mines;
            s.rules.ai.obstacles.at_mines = 0;
        });
        let o = w.match_setup().unwrap().rules.ai.fort.wire_out_m as i32;
        // The point's nest and wire stand (S10a), so the field comes next.
        let nest = h.line[1] + h.along * Fx::from_int(6);
        let nest_f = fort(&w, "ww2:mg_nest");
        assert!(w.build_now(
            nest_f,
            &[nest, nest + h.enemy * Fx::from_int(10)],
            PlayerId(1)
        ));
        let wc = h.mid() + h.enemy * Fx::from_int(o);
        prebuild(
            &mut w,
            "ww2:barbed_wire",
            [
                wc - h.along * Fx::from_int(30),
                wc + h.along * Fx::from_int(30),
            ],
        );
        squad_at(&mut w, ENG, h.pos, 120);
        let ap = fort(&w, "ww2:ap_minefield");
        let ok = play_until(&mut w, 600, |w| mines_of(w, ap).len() >= 15);
        (w, h, ok, o)
    };
    let (w, h, ok, o) = lay(1);
    let ap = fort(&w, "ww2:ap_minefield");
    assert!(ok, "the AP field stands: {} mines", mines_of(&w, ap).len());
    let beyond = w.match_setup().unwrap().rules.ai.obstacles.ap_beyond_wire_m as i32;
    for m in mines_of(&w, ap) {
        let out = along_m(m, h.mid(), h.enemy);
        assert!(
            (out - (o + beyond)).abs() <= 2,
            "{} m out: {out}",
            o + beyond
        );
        assert!(along_m(m, h.mid(), h.along).abs() <= 16, "within 30 m");
    }
    let c = &ai_ro(&w).counters;
    assert!(c.s10b_jobs >= 1 && c.lay_sites >= 1 && c.refused_reorders == 0);
    // Control: the switch off.
    let (w, _, ok, _) = lay(0);
    assert!(!ok);
    assert!(mines_of(&w, ap).is_empty() && jobs_of(&w, ap) == 0);
    assert_eq!(ai_ro(&w).counters.s10b_jobs, 0);
}

/// [`held`] with wire off and the AP field standing at its `ap_out_m`
/// site: only the AT field is left. `edit` runs last.
fn at_world(edit: impl FnOnce(&mut frontline_content::MatchSetup)) -> (World, Held) {
    let (mut w, h) = held(|s| {
        s.rules.ai.fort.wire = 0;
        edit(s);
    });
    let out = w.match_setup().unwrap().rules.ai.obstacles.ap_out_m as i32;
    prebuild(&mut w, "ww2:ap_minefield", ap_site(&h, out));
    (w, h)
}

/// The AT field the AI lays in [`at_world`] (`edit`), within 900 s: its
/// mines and the point frame.
fn lay_at(edit: fn(&mut frontline_content::MatchSetup)) -> (World, Held, Vec<FxVec2>) {
    let (mut w, h) = at_world(edit);
    squad_at(&mut w, ENG, h.pos, 120);
    let at = fort(&w, "ww2:at_minefield");
    play_until(&mut w, 900, |w| mines_of(w, at).len() >= 10);
    let ms = mines_of(&w, at);
    (w, h, ms)
}

/// R2 (Build C fix): an enemy tank that drives the approach the AI laid
/// its AT field across sets a mine off. Vehicles drive the 4 m path grid
/// and the field's middle sits on that route, so a row centred on it puts a
/// mine under the hull's middle, 1.25 m from each track (trigger 40 cm),
/// and every mine 4 m out lies 2.75 m from the other track: the tank never
/// set one off (fortify `s10b_ai.mines` 0/10, matches 0). The row is now
/// moved along itself by the route vehicle's track half-gauge. Control:
/// drop that shift in `at_site_search` -> the tank drives through (seen
/// failing as "the route tank sets a mine off").
#[test]
fn ai_at_field_catches_the_route_tank() {
    let (mut w, h, ms) = lay_at(|_| {});
    assert!(ms.len() >= 10, "the AT field stands: {} mines", ms.len());
    let base = w.game().unwrap().base_of(PlayerId(0));
    let dir = (base - h.pos).normalize_or_zero();
    let from = h.pos + dir * Fx::from_int(150);
    let tank = spawn(
        &mut w,
        0,
        "ww2:ger_pz4_h_squad",
        from.x.floor_int(),
        from.y.floor_int(),
    );
    let v = members(&w, tank)[0];
    run(
        &mut w,
        0,
        1,
        CommandKind::Move {
            squad: tank,
            target: h.pos,
        },
    );
    let stopped = play_until(&mut w, 120, |w| {
        let g = w.game().unwrap();
        g.vehicle(v).is_none_or(|vs| {
            vs.immobilised(&g.setup().vehicles[vs.type_idx as usize]) || vs.wreck_tick.is_some()
        })
    });
    assert!(stopped, "the route tank sets a mine off");
}

/// R2: the AT field lies across the enemy vehicle approach `at_out_m` (60)
/// from the point: on the flat map the vehicle path from the enemy base runs
/// along the straight line, so the field's middle lies within 10 m of that
/// line, 60 m (+-10) out, its ends on both sides of it. Control: `at_out_m`
/// 1 (the spec's 0 is out of the key's range 1..=1000) -> the field lies at
/// the point, not 60 m out (seen failing as "60 m out").
#[test]
fn ai_lays_at_field_on_the_vehicle_approach() {
    let at_60 = |w: &World, h: &Held, ms: &[FxVec2]| {
        assert!(ms.len() >= 10, "the AT field stands: {} mines", ms.len());
        let base = w.game().unwrap().base_of(PlayerId(0));
        let dir = (base - h.pos).normalize_or_zero();
        let mid = ms
            .iter()
            .fold(FxVec2::ZERO, |a, &m| a + m)
            .div_int(ms.len() as i32);
        // Across it: the line is perpendicular to the path's own leg there
        // (grid-aligned), so its ends lie on both sides of the straight line.
        let side = |p: FxVec2| {
            let v = p - h.pos;
            v.x * dir.y - v.y * dir.x
        };
        let lo = ms.iter().map(|m| side(*m)).min().unwrap();
        let hi = ms.iter().map(|m| side(*m)).max().unwrap();
        off_m(mid, h.pos, dir) <= 10
            && lo < Fx::ZERO
            && hi > Fx::ZERO
            && hi - lo >= Fx::from_int(30)
            && (along_m(mid, h.pos, dir) - 60).abs() <= 10
    };
    let (w, h, ms) = lay_at(|_| {});
    assert!(at_60(&w, &h, &ms), "across the approach, 60 m out: {ms:?}");
    assert_eq!(ai_ro(&w).counters.s10b_refused, 0);
    let (w, h, ms) = lay_at(|s| s.rules.ai.obstacles.at_out_m = 1);
    assert!(!at_60(&w, &h, &ms), "control: at_out_m 1 is not 60 m out");
}

/// R4: at most `at_fields_max` AT fields per player (three short fields
/// stand elsewhere: no AT job), the blueprint's per-player limit (no job
/// that would pass it) and `fields_per_point` (1: the AP field fills the
/// point). Control: `at_fields_max` 9 -> the AT job starts (seen failing as
/// "no AT job" with the cap check removed).
#[test]
fn ai_mine_caps_hold() {
    let at_jobs = |edit: fn(&mut frontline_content::MatchSetup)| {
        let (mut w, h) = at_world(edit);
        let base = w.game().unwrap().base_of(PlayerId(1));
        for k in 0..3 {
            let a = base + c(-150 + 100 * k, 120);
            prebuild(&mut w, "ww2:at_minefield", [a, a + c(8, 0)]);
        }
        squad_at(&mut w, ENG, h.pos, 120);
        let at = fort(&w, "ww2:at_minefield");
        let mut most = 0;
        for _ in 0..120 {
            play(&mut w, 1);
            most = most.max(jobs_of(&w, at));
        }
        most
    };
    assert_eq!(at_jobs(|_| {}), 0, "at_fields_max 3");
    assert!(
        at_jobs(|s| s.rules.ai.obstacles.at_fields_max = 9) > 0,
        "control"
    );
    // The blueprint limit: 6 segments used + 10 > 15.
    let limit = |s: &mut frontline_content::MatchSetup| {
        s.rules.ai.obstacles.at_fields_max = 9;
        let f = s
            .forts
            .iter()
            .position(|f| f.id == "ww2:at_minefield")
            .unwrap();
        s.forts[f].limit_per_player = 15;
    };
    assert_eq!(at_jobs(limit), 0, "blueprint limit");
    let per_point = |s: &mut frontline_content::MatchSetup| {
        s.rules.ai.obstacles.at_fields_max = 9;
        s.rules.ai.obstacles.fields_per_point = 1;
    };
    assert_eq!(at_jobs(per_point), 0, "fields_per_point 1");
}

/// R8: S10b loads stop at `[ai.obstacles] materials_max_pct` of the income
/// (1 % of 300 MP = 3 MP = 30 kg at 100 MP/t; the squad carries 56), the
/// S10a budget at 0. Control:
/// `materials_max_pct` 100 -> the builder loads its full carry (seen
/// failing as "loads stop at the budget").
#[test]
fn ai_s10b_budget_stops_loading() {
    let loaded = |pct: u32| {
        let (mut w, _) = held(|s| {
            s.rules.ai.fort.materials_max_pct = 0;
            s.rules.ai.fort.wire = 0;
            s.rules.ai.obstacles.at_mines = 0;
            s.rules.ai.obstacles.materials_max_pct = pct;
            s.rules.start_manpower = 300;
        });
        let base = w.game().unwrap().base_of(PlayerId(1));
        let a = squad_at(&mut w, ENG, base, 0);
        play(&mut w, 30);
        let fs = w.game().unwrap().fort();
        let got = fs.carried_g.get(&a).copied().unwrap_or(0);
        let _ = got;
        (
            fs.accounts[1].materials_loaded_g as u32,
            fs.accounts[1].materials_micro,
        )
    };
    let (got, micro) = loaded(1);
    assert_eq!(got, 30_000, "loads stop at the budget");
    assert!(micro <= 3 * crate::supply::MICRO);
    let (full, _) = loaded(100);
    assert!(full > 30_000, "control: full carry {full}");
}

/// R3 (S10b R7): an own tank drives through the own AT field unhurt.
/// Control: the same field owned by the enemy (unknown to player 1) -> the
/// tank is immobilised (seen failing).
#[test]
fn ai_tank_drives_through_own_field_unhurt() {
    let drive = |owner: u8| {
        let mut w = flat_world_with(1, |_| {});
        steps(&mut w, 3);
        let at = fort(&w, "ww2:at_minefield");
        // Every 0.5 m across the tank's way (a track passes within 40 cm).
        for k in 0..=20 {
            let p = FxVec2::new(
                Fx::from_int(395) + Fx::from_milli(500 * k),
                Fx::from_int(400),
            );
            w.lay_mine_now(at, p, PlayerId(owner)).unwrap();
        }
        let tank = spawn(&mut w, 1, "ww2:usa_m4a3_75_squad", 400, 340);
        let v = members(&w, tank)[0];
        run(
            &mut w,
            1,
            1,
            CommandKind::Move {
                squad: tank,
                target: c(400, 460),
            },
        );
        play(&mut w, 60);
        let g = w.game().unwrap();
        let y = w.state.entities.transform[v.index()].pos.y.floor_int();
        let hit = g.vehicle(v).is_none_or(|vs| {
            vs.immobilised(&g.setup().vehicles[vs.type_idx as usize]) || vs.wreck_tick.is_some()
        });
        (hit, y)
    };
    let (stuck, y) = drive(1);
    assert!(!stuck && y > 440, "own field: through unhurt (y {y})");
    let (stuck, _) = drive(0);
    assert!(stuck, "control: an enemy field immobilises it");
}

/// Job table [B] "another squad on it": two builders, `jobs_max` 2, AP only:
/// never two jobs on the one AP site. Control: `fort_near` blind to jobs
/// and works (`field_has` always false) -> both take the AP site (seen
/// failing).
#[test]
fn ai_two_builders_never_share_site() {
    let (mut w, h) = held(|s| {
        s.rules.ai.fort.wire = 0;
        s.rules.ai.obstacles.at_mines = 0;
        s.rules.ai.obstacles.jobs_max = 2;
        s.rules.ai.fort.builders_max = 4;
    });
    squad_at(&mut w, ENG, h.pos, 120);
    squad_at(&mut w, ENG, h.pos + c(4, 0), 120);
    let s = w.match_setup().unwrap().clone();
    let mut one = 0;
    for _ in 0..240 {
        play(&mut w, 1);
        let n = ai_ro(&w)
            .forts
            .values()
            .filter(|j| j.phase != FortPhase::Done && s.forts[j.fort as usize].mine_item.is_some())
            .count();
        assert!(n <= 1, "two jobs on the one AP site");
        one += n;
    }
    assert!(one > 0, "the AP job ran");
}

/// Job table [B] "squad dead": the builder dies mid work -> the job ends,
/// the work is removed and its kg are counted lost. Control: the builder
/// lives -> the job and work go on (asserted first).
#[test]
fn ai_s10b_builder_dies_mid_work() {
    let (mut w, h) = held(|s| {
        s.rules.ai.fort.wire = 0;
        s.rules.ai.obstacles.at_mines = 0;
    });
    let eng = squad_at(&mut w, ENG, h.pos, 120);
    let ap = fort(&w, "ww2:ap_minefield");
    let working = |w: &World| {
        w.game()
            .unwrap()
            .fort()
            .works
            .values()
            .any(|k| k.builder == eng && k.fort == ap && k.got_g > 0)
    };
    assert!(play_until(&mut w, 300, working), "the work began");
    assert!(ai_ro(&w).forts.contains_key(&eng));
    let lost0 = w.game().unwrap().fort().accounts[1].materials_lost_g;
    for m in members(&w, eng) {
        w.kill_soldier(m);
    }
    play(&mut w, 10);
    let fs = w.game().unwrap().fort();
    assert!(!fs.works.values().any(|k| k.builder == eng), "work removed");
    assert!(!ai_ro(&w).forts.contains_key(&eng), "job ended");
    assert!(fs.accounts[1].materials_lost_g > lost0, "kg counted");
    assert_eq!(ai_ro(&w).counters.refused_reorders, 0);
}

/// Job table [B] "vehicle in footprint" (L3, L4): an own tank on the AP
/// site -> BadSpot; the refusal is remembered and no job starts near it
/// for `refused_s`. Control: `refused_s` 0 -> the AI orders the same spot
/// again within 300 s (`refused_reorders` > 0; seen failing).
#[test]
fn ai_s10b_refused_spot_is_remembered() {
    let go = |refused_s: u32| {
        let (mut w, h) = held(|s| {
            s.rules.ai.fort.wire = 0;
            s.rules.ai.obstacles.at_mines = 0;
            s.rules.ai.obstacles.refused_s = refused_s;
        });
        let out = w.match_setup().unwrap().rules.ai.obstacles.ap_out_m as i32;
        let site = ap_site(&h, out);
        let mid = (site[0] + site[1]).div_int(2);
        spawn(
            &mut w,
            1,
            "ww2:usa_m4a3_75_squad",
            mid.x.floor_int(),
            mid.y.floor_int(),
        );
        squad_at(&mut w, ENG, h.pos, 120);
        play(&mut w, 240);
        let ai = ai_ro(&w);
        (
            ai.refused
                .iter()
                .filter(|r| r.kind == JobKind::Fort)
                .count(),
            ai.counters.s10b_jobs,
            ai.counters.refused_reorders,
        )
    };
    let (refused, jobs, reorders) = go(300);
    assert!(refused >= 1, "remembered");
    assert_eq!(jobs, 1, "one start in 240 s");
    assert_eq!(reorders, 0);
    let (_, jobs, reorders) = go(0);
    assert!(
        jobs > 1 && reorders > 0,
        "control: {jobs} jobs, {reorders} reorders"
    );
}

/// R13 (Build C, used by D): the AI knows an enemy obstacle by distance
/// alone. Player 0's hedgehog 100 m from player 1's squad (within
/// `obst_seen_m` 150) is known; one 300 m out is not; player 1's own
/// hedgehog never is. Control: `obst_seen_m` 1 -> none known (seen failing
/// as "the near hedgehog is known").
#[test]
fn ai_knows_obstacles_by_distance() {
    let known = |seen_m: u32| {
        let (mut w, _) = held(|s| s.rules.ai.obstacles.obst_seen_m = seen_m);
        let hh = fort(&w, "ww2:hedgehog");
        squad_at(&mut w, ENG, c(400, 400), 0);
        let put = |w: &mut World, x: i32, owner: u8| {
            let before: Vec<u32> = w.game().unwrap().fort().pieces.keys().copied().collect();
            assert!(
                w.build_now(hh, &[c(x, 400), c(x + 1, 400)], PlayerId(owner)),
                "hedgehog at {x}"
            );
            let after = w.game().unwrap().fort().pieces.keys().copied();
            after.filter(|k| !before.contains(k)).collect::<Vec<u32>>()
        };
        let near = put(&mut w, 500, 0);
        let far = put(&mut w, 700, 0);
        let own = put(&mut w, 450, 1);
        assert!(!near.is_empty() && !far.is_empty() && !own.is_empty());
        let got = w
            .game()
            .unwrap()
            .known_obstacles(&w.state.entities, PlayerId(1));
        (got, near, far, own)
    };
    let (got, near, far, own) = known(150);
    assert!(
        near.iter().all(|p| got.contains(p)),
        "the near hedgehog is known: {got:?}"
    );
    assert!(!far.iter().any(|p| got.contains(p)), "the far one is not");
    assert!(!own.iter().any(|p| got.contains(p)), "own pieces never");
    assert!(got.windows(2).all(|x| x[0] < x[1]), "ascending");
    let (got, near, _, _) = known(1);
    assert!(
        !near.iter().any(|p| got.contains(p)),
        "control: obst_seen_m 1 knows none: {got:?}"
    );
}

/// The second-row mines of an AT field `ms` (laid in id order, the first
/// row's `n` first): each lies one step (4 m, +-0.75) behind the first
/// row's line on the point's side and half a step (2 m, +-0.75) along it
/// from the nearest first-row mine.
fn row2_mines(h: &Held, ms: &[FxVec2], n: usize) -> usize {
    if ms.len() <= n {
        return 0;
    }
    let (a, b) = (ms[0], ms[n - 1]);
    let along = (b - a).normalize_or_zero();
    let mut back = along.perp();
    if back.dot(h.pos - a) < Fx::ZERO {
        back = -back;
    }
    let tol = Fx::from_milli(750);
    let near = |x: Fx, v: i32| (x - Fx::from_int(v)).abs() <= tol;
    ms[n..]
        .iter()
        .filter(|&&m| {
            let behind = (m - a).dot(back);
            let t = (m - a).dot(along);
            let gap = ms[..n]
                .iter()
                .map(|&r| (t - (r - a).dot(along)).abs())
                .min()
                .unwrap();
            near(behind, 4) && near(gap, 2)
        })
        .count()
}

/// S10b-ai Build E item 5 (S10b-ai-B2a, `at_rows` 2): once the AT field's
/// first row (10 mines on 40 m) is laid, the AI lays a second row as its
/// own job, one step behind it toward the point and half a step along it,
/// so its mines lie between the first row's. Control: `at_rows` 1 -> no
/// second row (seen failing as "a staggered second row").
#[test]
fn ai_at_field_lays_a_staggered_second_row() {
    let lay = |rows: u32| {
        let (mut w, h) = at_world(|s| s.rules.ai.obstacles.at_rows = rows);
        squad_at(&mut w, ENG, h.pos, 200);
        let at = fort(&w, "ww2:at_minefield");
        play_until(&mut w, 1500, |w| mines_of(w, at).len() >= 20);
        let ms = mines_of(&w, at);
        (h, ms)
    };
    let (h, ms) = lay(2);
    let n2 = row2_mines(&h, &ms, 10);
    assert!(
        n2 >= 8,
        "a staggered second row: {n2} of {} mines: {ms:?}",
        ms.len()
    );
    let (h, ms) = lay(1);
    assert!(ms.len() >= 10, "control: row 1 stands ({} mines)", ms.len());
    assert_eq!(
        row2_mines(&h, &ms, 10),
        0,
        "control: at_rows 1 keeps one row"
    );
}
