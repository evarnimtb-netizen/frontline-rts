//! S12 fortify-test rows (docs/slices/S12-wounds-medics.md 8, rows R7 and
//! R8; Build A): the `wounds` scene under `fortify10.s12`. The S12
//! switches are on in the row only (`[wounds] enabled` 1); each row's
//! control is the same scene with `enabled` 0, which plays as the base
//! (`<BASE>`, every S12 switch 0). Every row is driven by player commands;
//! no AI code runs. Rifle grenades are off (small arms only).
//!
//! A man "out" left his squad: dead, or (with wounds on) downed. Wounded
//! men still in the squad are not out.
//!
//! - `trench` (R7, hard): each faction's line squad in a dug trench under
//!   the other's line squad's rifles, 100 m, 300 s. `out_pct` (men out of
//!   men), `dead_pct`, `downed_pct`, `wounded_pct` (wounded at the end),
//!   control `out_pct` with `enabled` 0; `vs_base_pct` = `out_pct` as % of
//!   the control's; `pass` when `vs_base_pct` <= 110. `head_300`: the
//!   same with `head_damage_pct` 300 (a report).
//! - `prone` (R8): each faction's line squad prone in the open under the
//!   other's MG (its roster's `mg` card, called in), 100 m, 300 s. The same numbers; R8 holds when
//!   `vs_base_pct` <= 100 (men die no faster than at the base).
//!
//! Build B (W6-W11), switches on in the row only:
//! - `mg300` (R1, report): each faction's line squad in the open under two
//!   of the other's MG teams (`mg` card, spawned) at 100 m (the plan's
//!   300 m is past the cards' 150 m sight) for 30 s, then the MGs leave
//!   (the squad "ends with downed men"); 300 s in all, no
//!   medics (`auto_tend_m` 0). `downed`, `outright_pct` (killed outright
//!   of killed outright plus downed), `bled_out_pct` (of downed),
//!   `downed_end` (still down at the end). `pass` when men went down and
//!   `outright_pct` is 20-45.
//! - `medics` (R2): the same with an own medic team (`medic` card) 60 m
//!   behind the squad, out of the MGs' sight, that moves up to the squad
//!   when the fire lifts (auto-tend on, `auto_tend_m` 25). `drop_pts` = R1's `bled_out_pct`
//!   less R2's; `pass` when it is 30 or more.
//! - `post` (R3, R4): a built aid post with a medic team in it; a wounded
//!   man (40 HP) walks in. `treated_pct` (runs he was treated in
//!   `treat_s` + 60 s), `hp` (his HP after, mean), `stopped_pct` (runs in
//!   which, the medics moved out of the post as soon as he has a bed, he
//!   was not treated in the same time: the post stops), `second_pct` (runs
//!   in which, wounded again and back at the post for 120 s, he was
//!   treated twice); control `unstaffed_treated_pct` (100 less
//!   `stopped_pct`). `pass` when 100, 75 (`return_hp_pct`), 100, 0.
//! - `refund` (R5): in the base, `Evacuate` of a paid man (of a squad
//!   called in) refunds `evac_refund_pct` of his per-man price once
//!   (`paid_ok_pct`: runs with exactly that); of an unpaid man (a start
//!   squad's) and a second `Evacuate` of the gone man nothing
//!   (`unpaid_mp`, `repeat_mp`). `pass` when 100, 0, 0.
//! - `reinforce` (R6, `reinforce` 1): a line squad in base loses three
//!   men to `Evacuate`; `Reinforce` brings it back to card size
//!   (`filled_pct`) with the card's plain rifle kit (`rifle_kit_pct`:
//!   runs in which every new man carries it); control `control_filled_pct`
//!   with the switch 0. `pass` when both are 100.
use anyhow::Result;
use frontline_content::MatchSetup;
use frontline_sim::command::CommandKind;
use frontline_sim::{PlayerId, SquadId, World};
use serde::Serialize;

use super::s10b::{fields, pct};
use super::s11a1::{par_seeds, ratio};
use super::{EAST, FortifyArgs, WEST, at, members, step_with, world_with};

/// The run length (s).
const RUN_S: u64 = 300;
/// `trench`: the squads' distance (m).
const TRENCH_GAP_M: i32 = 100;
/// `prone`: the MG's distance (m).
const PRONE_GAP_M: i32 = 100;
/// `mg300` (R1, R2): the MGs' distance (m). The plan's 300 m is beyond
/// the cards' `sight_m` 150: nobody would see anybody.
const MG_GAP_M: i32 = 100;
/// `mg300`: the MGs fire this long (s), then leave; the rest of `RUN_S`
/// is the lull in which medics work and downed men bleed out.
const MG_FIRE_S: u64 = 30;
/// `medics` (R2): the medic team waits this far behind the squad (m; out
/// of the MGs' sight) and comes up when the fire lifts.
const MEDIC_BACK_M: i32 = 60;
/// R7: the trench's losses with wounds on, at most this % of the base's.
pub const R7_MAX_PCT: f64 = 110.0;

/// One variant's numbers.
#[derive(Clone, Debug, Default, Serialize)]
pub struct WoundsRun {
    pub out_pct: Option<f64>,
    pub dead_pct: Option<f64>,
    pub downed_pct: Option<f64>,
    pub wounded_pct: Option<f64>,
}

/// `fortify10.s12.trench` (R7) and `.prone` (R8).
#[derive(Clone, Debug, Default, Serialize)]
pub struct WoundsRow {
    pub out_pct: Option<f64>,
    pub dead_pct: Option<f64>,
    pub downed_pct: Option<f64>,
    pub wounded_pct: Option<f64>,
    /// `out_pct` as % of the control's.
    pub vs_base_pct: Option<f64>,
    /// Exposure: men under fire (both factions, every seed).
    pub men: Option<f64>,
    pub pass: Option<bool>,
    /// `enabled` 0: the base.
    pub control: WoundsRun,
    /// `trench` only: `head_damage_pct` 300 (a report).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head_300: Option<WoundsRun>,
}

/// `fortify10.s12.mg300` (R1) and `.medics` (R2).
#[derive(Clone, Debug, Default, Serialize)]
pub struct MgRow {
    /// Men under fire (both factions, every seed).
    pub men: Option<f64>,
    pub downed: Option<f64>,
    pub outright_pct: Option<f64>,
    pub bled_out_pct: Option<f64>,
    pub stabilised_pct: Option<f64>,
    pub downed_end: Option<f64>,
    /// R2: R1's `bled_out_pct` less this row's (points).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drop_pts: Option<f64>,
    pub pass: Option<bool>,
}

/// `fortify10.s12.post` (R3, R4).
#[derive(Clone, Debug, Default, Serialize)]
pub struct PostRow {
    pub runs: Option<f64>,
    pub treated_pct: Option<f64>,
    pub hp: Option<f64>,
    pub stopped_pct: Option<f64>,
    /// Control: runs treated with the medics gone (100 - `stopped_pct`).
    pub unstaffed_treated_pct: Option<f64>,
    pub second_pct: Option<f64>,
    pub pass: Option<bool>,
}

/// `fortify10.s12.refund` (R5).
#[derive(Clone, Debug, Default, Serialize)]
pub struct RefundRow {
    pub runs: Option<f64>,
    pub paid_ok_pct: Option<f64>,
    pub unpaid_mp: Option<f64>,
    pub repeat_mp: Option<f64>,
    pub pass: Option<bool>,
}

/// `fortify10.s12.reinforce` (R6).
#[derive(Clone, Debug, Default, Serialize)]
pub struct ReinforceRow {
    pub runs: Option<f64>,
    pub filled_pct: Option<f64>,
    pub rifle_kit_pct: Option<f64>,
    pub men: Option<f64>,
    /// Control: `filled_pct` with the `reinforce` switch 0.
    pub control_filled_pct: Option<f64>,
    pub pass: Option<bool>,
}

/// The S12 rows (`fortify10.s12`).
#[derive(Clone, Debug, Default, Serialize)]
pub struct S12Report {
    pub trench: WoundsRow,
    pub prone: WoundsRow,
    pub mg300: MgRow,
    pub medics: MgRow,
    pub post: PostRow,
    pub refund: RefundRow,
    pub reinforce: ReinforceRow,
}

/// Summary lines, one per row.
pub fn summary_text(r: &S12Report) -> String {
    let v = serde_json::to_value(r).unwrap_or_default();
    [
        "trench",
        "prone",
        "mg300",
        "medics",
        "post",
        "refund",
        "reinforce",
    ]
    .iter()
    .map(|k| format!("  S12 {k}: {}\n", fields(&v[*k])))
    .collect()
}

/// Moves player `p`'s squads but `keep` to a corner.
fn park_others(w: &mut World, p: u8, keep: SquadId) {
    let corner = if p == 0 { at(60, 950) } else { at(964, 950) };
    for (id, q) in w.game().unwrap().squads().clone() {
        if id != keep && q.owner == PlayerId(p) {
            w.place_squad(id, corner, if p == 0 { EAST } else { WEST });
        }
    }
}

/// The wounds variants: base (`enabled` 0), on, on with head 300 %.
#[derive(Clone, Copy, PartialEq)]
pub enum Variant {
    Base,
    On,
    Head300,
}

fn edit(v: Variant) -> impl FnOnce(&mut MatchSetup) {
    move |s: &mut MatchSetup| {
        super::s11a1::flat(s);
        s.rifle_grenades_off();
        let w = &mut s.rules.wounds;
        w.enabled = (v != Variant::Base) as u32;
        if v == Variant::Head300 {
            w.head_damage_pct = 300;
        }
    }
}

/// The scene of one run, target player `tp` (the other fires), set up
/// and not yet run: (world, target squad). `prone`: R8's scene, else R7's.
pub fn scene(
    args: &FortifyArgs,
    seed: u64,
    tp: u8,
    prone: bool,
    v: Variant,
) -> Result<(World, SquadId)> {
    let mut w = world_with(args, seed, edit(v))?;
    let fp = 1 - tp;
    let y = 500;
    let tx = 400;
    let target = super::squad(&w, tp, "line")?;
    park_others(&mut w, tp, target);
    if prone {
        let q = super::call_in(&mut w, fp, "mg")?;
        park_others(&mut w, fp, q);
        w.place_squad(q, at(tx + PRONE_GAP_M, y), WEST);
    } else {
        let q = super::squad(&w, fp, "line")?;
        park_others(&mut w, fp, q);
        w.dig_trench_now(&[at(tx, y + 16), at(tx, y - 16)], -1);
        w.place_squad(q, at(tx + TRENCH_GAP_M, y), WEST);
    }
    w.place_squad(target, at(tx, y), EAST);
    if prone {
        let men = members(&w, target);
        step_with(
            &mut w,
            tp,
            vec![CommandKind::SetStance {
                units: men,
                stance: frontline_sim::StanceOrder::Prone,
            }],
        );
    }
    Ok((w, target))
}

/// One run: [out, dead, downed, wounded, men] of the target squad.
fn once(args: &FortifyArgs, seed: u64, tp: u8, prone: bool, v: Variant) -> Result<[u32; 5]> {
    let (mut w, target) = scene(args, seed, tp, prone, v)?;
    let men = members(&w, target).len() as u32;
    let g = w.game().unwrap();
    let pl = &g.players()[tp as usize];
    let (l0, d0) = (pl.losses, pl.s12.downed);
    let hz = w.match_setup().unwrap().tick_hz as u64;
    for _ in 0..RUN_S * hz {
        w.step(&[]);
    }
    let left = members(&w, target);
    let wounded = left
        .iter()
        .filter(|m| w.entities().combat()[m.index()].wound.state == frontline_sim::wounds::WOUNDED)
        .count() as u32;
    let pl = &w.game().unwrap().players()[tp as usize];
    Ok([
        men - left.len() as u32,
        pl.losses - l0,
        pl.s12.downed - d0,
        wounded,
        men,
    ])
}

fn run_of(t: &[u32; 5]) -> WoundsRun {
    WoundsRun {
        out_pct: pct(t[0], t[4]),
        dead_pct: pct(t[1], t[4]),
        downed_pct: pct(t[2], t[4]),
        wounded_pct: pct(t[3], t[4]),
    }
}

/// One row (module docs): `prone` R8, else R7.
pub fn row(args: &FortifyArgs, seeds: std::ops::Range<u64>, prone: bool) -> Result<WoundsRow> {
    let variants: &[Variant] = if prone {
        &[Variant::On, Variant::Base]
    } else {
        &[Variant::On, Variant::Base, Variant::Head300]
    };
    let mut t = [[0u32; 5]; 3];
    let all = par_seeds(seeds, |seed| {
        let mut v = [[0u32; 5]; 3];
        for (k, &var) in variants.iter().enumerate() {
            for tp in 0..2u8 {
                let r = once(args, seed, tp, prone, var)?;
                for (x, y) in v[k].iter_mut().zip(r) {
                    *x += y;
                }
            }
        }
        Ok(v)
    })?;
    for v in all {
        for (a, b) in t.iter_mut().zip(v) {
            for (x, y) in a.iter_mut().zip(b) {
                *x += y;
            }
        }
    }
    let on = run_of(&t[0]);
    let control = run_of(&t[1]);
    let vs = ratio(on.out_pct, control.out_pct).or(
        // No base losses: on passes only with none either.
        (t[1][4] > 0 && t[1][0] == 0).then_some(if t[0][0] == 0 { 100.0 } else { f64::INFINITY }),
    );
    let max = if prone { 100.0 } else { R7_MAX_PCT };
    Ok(WoundsRow {
        out_pct: on.out_pct,
        dead_pct: on.dead_pct,
        downed_pct: on.downed_pct,
        wounded_pct: on.wounded_pct,
        vs_base_pct: vs,
        men: (t[0][4] > 0).then_some(t[0][4] as f64),
        pass: vs.map(|v| v <= max),
        control,
        head_300: (!prone).then(|| run_of(&t[2])),
    })
}

// ------------------------------------------------- Build B: R1-R6 --

/// The S12 switches of the Build B rows (`reinforce` for R6 only).
fn edit_b(auto_tend_m: u32, reinforce: bool) -> impl FnOnce(&mut MatchSetup) {
    move |s: &mut MatchSetup| {
        super::s11a1::flat(s);
        s.rifle_grenades_off();
        let w = &mut s.rules.wounds;
        w.enabled = 1;
        w.auto_tend_m = auto_tend_m;
        w.reinforce = reinforce as u32;
    }
}

/// Player `p`'s first roster card with tag `tag`, spawned at once (no
/// cost or cooldown) at `pos` facing `facing`.
fn spawn_tag(
    w: &mut World,
    p: u8,
    tag: &str,
    pos: frontline_sim::FxVec2,
    facing: frontline_sim::FxVec2,
) -> Result<SquadId> {
    let s = w.match_setup().unwrap();
    let f = &s.factions[s.players[p as usize].faction as usize];
    let Some(&ty) = f.roster.iter().find(|&&t| s.type_has_tag(t, tag)) else {
        anyhow::bail!("{} has no {tag} card", f.id);
    };
    w.spawn_squad_at(PlayerId(p), ty, pos, facing)
        .ok_or_else(|| anyhow::anyhow!("cannot spawn the {tag} card"))
}

fn s12_of(w: &World, p: u8) -> frontline_sim::wounds::S12Counters {
    w.game().unwrap().players()[p as usize].s12
}

fn downed_of(w: &World, p: u8) -> u32 {
    let e = w.entities();
    e.kind()
        .iter()
        .zip(e.owner())
        .filter(|(k, o)| **k == frontline_sim::EntityKind::Downed && o.0 == p)
        .count() as u32
}

/// R1/R2 run: [men, downed, outright, bled out, stabilised, down at end].
fn mg_once(args: &FortifyArgs, seed: u64, tp: u8, medics: bool) -> Result<[u32; 6]> {
    let mut w = world_with(args, seed, edit_b(if medics { 25 } else { 0 }, false))?;
    let fp = 1 - tp;
    let (tx, y) = (350, 500);
    let target = super::squad(&w, tp, "line")?;
    park_others(&mut w, tp, target);
    park_others(&mut w, fp, SquadId(u32::MAX));
    w.place_squad(target, at(tx, y), EAST);
    let mut mgs = Vec::new();
    for dy in [-10, 10] {
        mgs.push(spawn_tag(
            &mut w,
            fp,
            "mg",
            at(tx + MG_GAP_M, y + dy),
            WEST,
        )?);
    }
    let team = if medics {
        Some(spawn_tag(
            &mut w,
            tp,
            "medic",
            at(tx - MEDIC_BACK_M, y),
            EAST,
        )?)
    } else {
        None
    };
    let men = members(&w, target).len() as u32;
    let s0 = s12_of(&w, tp);
    let hz = w.match_setup().unwrap().tick_hz as u64;
    for k in 0..RUN_S * hz {
        if k == MG_FIRE_S * hz {
            // The fire lifts: the MGs leave (to their own corner) and the
            // medics come up to the squad (auto-tend from there).
            park_others(&mut w, fp, SquadId(u32::MAX));
            if let Some(q) = team {
                step_with(
                    &mut w,
                    tp,
                    vec![CommandKind::Move {
                        squad: q,
                        target: at(tx - 4, y),
                    }],
                );
                continue;
            }
        }
        w.step(&[]);
    }
    let s1 = s12_of(&w, tp);
    Ok([
        men,
        s1.downed - s0.downed,
        s1.killed_outright - s0.killed_outright,
        s1.bled_out - s0.bled_out,
        s1.stabilised - s0.stabilised,
        downed_of(&w, tp),
    ])
}

fn mg_row(args: &FortifyArgs, seeds: std::ops::Range<u64>, medics: bool) -> Result<MgRow> {
    let all = par_seeds(seeds, |seed| {
        let mut v = [0u32; 6];
        for tp in 0..2u8 {
            for (x, y) in v.iter_mut().zip(mg_once(args, seed, tp, medics)?) {
                *x += y;
            }
        }
        Ok(v)
    })?;
    let mut t = [0u32; 6];
    for v in all {
        for (x, y) in t.iter_mut().zip(v) {
            *x += y;
        }
    }
    let outright = pct(t[2], t[2] + t[1]);
    Ok(MgRow {
        men: (t[0] > 0).then_some(t[0] as f64),
        downed: Some(t[1] as f64),
        outright_pct: outright,
        bled_out_pct: pct(t[3], t[1]),
        stabilised_pct: pct(t[4], t[1]),
        downed_end: Some(t[5] as f64),
        drop_pts: None,
        pass: (!medics).then(|| t[1] > 0 && outright.is_some_and(|o| (20.0..=45.0).contains(&o))),
    })
}

/// R3/R4 run, player `tp`: (treated in time, his HP after, stopped when
/// unstaffed, treated twice).
fn post_once(args: &FortifyArgs, seed: u64, tp: u8) -> Result<(bool, u16, bool, bool)> {
    use frontline_sim::wounds::WOUNDED;
    let run = |staffed: bool| -> Result<(bool, u16, bool)> {
        let mut w = world_with(args, seed, edit_b(0, false))?;
        let s = w.match_setup().unwrap();
        let f = s.players[tp as usize].faction;
        let def = s
            .stock_of(f, frontline_content::setup::StockKind::Aid, None)
            .ok_or_else(|| anyhow::anyhow!("no aid post def"))?;
        let treat = s.rules.wounds.treat_s as u64;
        let hz = s.tick_hz as u64;
        let (px, py) = (400, 500);
        let pid = w
            .build_structure_now(
                PlayerId(tp),
                def,
                at(px, py),
                frontline_sim::Fx::ZERO,
                [0; 3],
            )
            .ok_or_else(|| anyhow::anyhow!("cannot build the post"))?;
        let line = super::squad(&w, tp, "line")?;
        park_others(&mut w, tp, line);
        park_others(&mut w, 1 - tp, SquadId(u32::MAX));
        let med = spawn_tag(&mut w, tp, "medic", at(px, py), EAST)?;
        w.place_squad(line, at(px + 30, py), EAST);
        let m = *members(&w, line).last().unwrap();
        let hurt = |w: &mut World| {
            w.edit_soldier(m, |_, c| {
                c.hp = 40;
                c.wound.state = WOUNDED;
                c.wound.bandaged = true;
            });
            step_with(
                w,
                tp,
                vec![CommandKind::MoveUnits {
                    units: vec![m],
                    target: at(px + 3, py + 3),
                }],
            );
        };
        hurt(&mut w);
        let bed = |w: &World| {
            w.game()
                .unwrap()
                .supply6()
                .structures
                .get(&pid)
                .is_some_and(|st| st.beds.iter().any(|b| b.man == m))
        };
        let treated = |w: &World| w.entities().combat()[m.index()].wound.treated;
        let mut k = 0;
        while !bed(&w) && k < 60 * hz {
            w.step(&[]);
            k += 1;
        }
        if !staffed {
            w.place_squad(med, at(px - 200, py), EAST);
        }
        for _ in 0..(treat + 60) * hz {
            if treated(&w) {
                break;
            }
            w.step(&[]);
        }
        let ok = treated(&w);
        let hp = w.entities().combat()[m.index()].hp;
        let mut twice = false;
        if staffed && ok {
            let n0 = s12_of(&w, tp).treated;
            for _ in 0..3 * hz {
                w.step(&[]);
            }
            hurt(&mut w);
            for _ in 0..120 * hz {
                w.step(&[]);
            }
            twice = s12_of(&w, tp).treated > n0 || w.entities().combat()[m.index()].hp > 40;
        }
        Ok((ok, hp, twice))
    };
    let (ok, hp, twice) = run(true)?;
    let (stopped_treated, _, _) = run(false)?;
    Ok((ok, hp, !stopped_treated, twice))
}

fn post_row(args: &FortifyArgs, seeds: std::ops::Range<u64>) -> Result<PostRow> {
    let all = par_seeds(seeds, |seed| {
        (0..2u8)
            .map(|tp| post_once(args, seed, tp))
            .collect::<Result<Vec<_>>>()
    })?;
    let v: Vec<_> = all.into_iter().flatten().collect();
    let n = v.len() as u32;
    type Run = (bool, u16, bool, bool);
    let count = |f: fn(&Run) -> bool| v.iter().filter(|r| f(r)).count() as u32;
    let treated = count(|r| r.0);
    let hp_sum: u32 = v.iter().filter(|r| r.0).map(|r| r.1 as u32).sum();
    let hp = (treated > 0).then(|| (hp_sum as f64 * 10.0 / treated as f64).round() / 10.0);
    let row = PostRow {
        runs: Some(n as f64),
        treated_pct: pct(treated, n),
        hp,
        stopped_pct: pct(count(|r| r.2), n),
        unstaffed_treated_pct: pct(n - count(|r| r.2), n),
        second_pct: pct(count(|r| r.3), n),
        pass: None,
    };
    Ok(PostRow {
        pass: Some(
            row.treated_pct == Some(100.0)
                && row.hp == Some(75.0)
                && row.stopped_pct == Some(100.0)
                && row.second_pct == Some(0.0),
        ),
        ..row
    })
}

/// A man of squad `sq` made wounded (40 HP, bandaged), his `paid` kept.
fn wound_man(w: &mut World, m: frontline_sim::SimId) {
    w.edit_soldier(m, |_, c| {
        c.hp = 40;
        c.wound.state = frontline_sim::wounds::WOUNDED;
        c.wound.bandaged = true;
    });
}

/// R5 run, player `tp`: (paid refund as expected, unpaid refund, repeat
/// refund) in MP.
fn refund_once(args: &FortifyArgs, seed: u64, tp: u8) -> Result<(bool, u32, u32)> {
    let mut w = world_with(args, seed, edit_b(0, false))?;
    park_others(&mut w, 1 - tp, SquadId(u32::MAX));
    let bought = super::call_in(&mut w, tp, "line")?;
    let s = w.match_setup().unwrap();
    let ty = &s.squad_types[w.game().unwrap().squad(bought).unwrap().type_idx as usize];
    let want = ty.cost / (ty.men as u32).max(1) * s.rules.wounds.evac_refund_pct / 100;
    let base = w.game().unwrap().base_of(PlayerId(tp));
    let paid = *members(&w, bought).last().unwrap();
    w.place_soldier(paid, base);
    wound_man(&mut w, paid);
    let r0 = s12_of(&w, tp).refund_mp;
    step_with(
        &mut w,
        tp,
        vec![CommandKind::Evacuate { units: vec![paid] }],
    );
    let r1 = s12_of(&w, tp).refund_mp;
    let start = super::squad(&w, tp, "line")?;
    let unpaid = *members(&w, start).last().unwrap();
    w.place_soldier(unpaid, base);
    wound_man(&mut w, unpaid);
    step_with(
        &mut w,
        tp,
        vec![CommandKind::Evacuate {
            units: vec![unpaid],
        }],
    );
    let r2 = s12_of(&w, tp).refund_mp;
    step_with(
        &mut w,
        tp,
        vec![CommandKind::Evacuate { units: vec![paid] }],
    );
    let r3 = s12_of(&w, tp).refund_mp;
    Ok((r1 - r0 == want && want > 0, r2 - r1, r3 - r2))
}

fn refund_row(args: &FortifyArgs, seeds: std::ops::Range<u64>) -> Result<RefundRow> {
    let all = par_seeds(seeds, |seed| {
        (0..2u8)
            .map(|tp| refund_once(args, seed, tp))
            .collect::<Result<Vec<_>>>()
    })?;
    let v: Vec<_> = all.into_iter().flatten().collect();
    let n = v.len() as u32;
    let ok = pct(v.iter().filter(|r| r.0).count() as u32, n);
    let unpaid: u32 = v.iter().map(|r| r.1).sum();
    let repeat: u32 = v.iter().map(|r| r.2).sum();
    Ok(RefundRow {
        runs: Some(n as f64),
        paid_ok_pct: ok,
        unpaid_mp: Some(unpaid as f64),
        repeat_mp: Some(repeat as f64),
        pass: Some(ok == Some(100.0) && unpaid == 0 && repeat == 0),
    })
}

/// R6 run, player `tp`: (back to card size, every new man with the plain
/// rifle kit, men reinforced).
fn reinforce_once(args: &FortifyArgs, seed: u64, tp: u8, on: bool) -> Result<(bool, bool, u32)> {
    let mut w = world_with(args, seed, edit_b(0, on))?;
    park_others(&mut w, 1 - tp, SquadId(u32::MAX));
    let sq = super::squad(&w, tp, "line")?;
    let base = w.game().unwrap().base_of(PlayerId(tp));
    w.place_squad(sq, base, EAST);
    let s = w.match_setup().unwrap();
    let ty = &s.squad_types[w.game().unwrap().squad(sq).unwrap().type_idx as usize];
    let size = ty.men as usize;
    let rifle = ty
        .members
        .iter()
        .find(|m| !s.weapons[m.weapon as usize].crew && m.traits == 0)
        .map(|m| m.weapon);
    let before = members(&w, sq);
    let gone: Vec<_> = before.iter().rev().take(3).copied().collect();
    for &m in &gone {
        wound_man(&mut w, m);
    }
    step_with(
        &mut w,
        tp,
        vec![CommandKind::Evacuate {
            units: gone.clone(),
        }],
    );
    // The men who stay (a new man may reuse a gone man's id).
    let before: Vec<_> = before.into_iter().filter(|m| !gone.contains(m)).collect();
    let r0 = s12_of(&w, tp).reinforced_men;
    step_with(&mut w, tp, vec![CommandKind::Reinforce { squad: sq }]);
    let hz = w.match_setup().unwrap().tick_hz as u64;
    for _ in 0..5 * hz {
        w.step(&[]);
    }
    let now = members(&w, sq);
    let new: Vec<_> = now.iter().filter(|m| !before.contains(m)).collect();
    let kit = !new.is_empty()
        && new
            .iter()
            .all(|m| Some(w.entities().kit()[m.index()].weapon) == rifle);
    Ok((now.len() == size, kit, s12_of(&w, tp).reinforced_men - r0))
}

fn reinforce_row(args: &FortifyArgs, seeds: std::ops::Range<u64>) -> Result<ReinforceRow> {
    let all = par_seeds(seeds, |seed| {
        let mut v = Vec::new();
        for tp in 0..2u8 {
            v.push((
                reinforce_once(args, seed, tp, true)?,
                reinforce_once(args, seed, tp, false)?.0,
            ));
        }
        Ok(v)
    })?;
    let both: Vec<_> = all.into_iter().flatten().collect();
    let v: Vec<_> = both.iter().map(|b| b.0).collect();
    let off = both.iter().filter(|b| b.1).count() as u32;
    let n = v.len() as u32;
    let filled = pct(v.iter().filter(|r| r.0).count() as u32, n);
    let kit = pct(v.iter().filter(|r| r.1).count() as u32, n);
    Ok(ReinforceRow {
        runs: Some(n as f64),
        filled_pct: filled,
        rifle_kit_pct: kit,
        men: Some(v.iter().map(|r| r.2).sum::<u32>() as f64),
        control_filled_pct: pct(off, n),
        pass: Some(filled == Some(100.0) && kit == Some(100.0)),
    })
}

/// Every S12 row.
pub fn s12_rows(args: &FortifyArgs, seeds: std::ops::Range<u64>) -> Result<S12Report> {
    let mg300 = mg_row(args, seeds.clone(), false)?;
    let mut medics = mg_row(args, seeds.clone(), true)?;
    medics.drop_pts = match (mg300.bled_out_pct, medics.bled_out_pct) {
        (Some(a), Some(b)) => Some(((a - b) * 10.0).round() / 10.0),
        _ => None,
    };
    medics.pass = medics.drop_pts.map(|d| d >= 30.0);
    Ok(S12Report {
        trench: row(args, seeds.clone(), false)?,
        prone: row(args, seeds.clone(), true)?,
        mg300,
        medics,
        post: post_row(args, seeds.clone())?,
        refund: refund_row(args, seeds.clone())?,
        reinforce: reinforce_row(args, seeds)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// R7/R8 (Build A): each row runs on one seed, men are hit, and with
    /// wounds on some are wounded or down where the control (the base) has
    /// none. (The R7 and R8 verdicts are read off the fortify10 set, not
    /// one seed.)
    #[test]
    fn fortify_s12_rows_run() {
        let r = s12_rows(&FortifyArgs::default(), 1..2).unwrap();
        eprintln!("{}", serde_json::to_string(&r).unwrap());
        for (k, row) in [("trench", &r.trench), ("prone", &r.prone)] {
            assert!(row.men.unwrap() > 0.0, "{k}");
            assert_eq!(
                row.control.downed_pct,
                Some(0.0),
                "{k}: none down at the base"
            );
            assert_eq!(
                row.control.wounded_pct,
                Some(0.0),
                "{k}: none wounded at the base"
            );
            assert!(
                row.downed_pct.unwrap() + row.wounded_pct.unwrap() > 0.0,
                "{k}: wounds on"
            );
        }
        assert!(
            r.trench.control.out_pct.unwrap() > 0.0,
            "the base loses men"
        );
        assert!(r.trench.head_300.is_some() && r.prone.head_300.is_none());
        // R1-R6 (Build B).
        assert!(r.mg300.downed.unwrap() > 0.0, "R1: men went down");
        assert_eq!(r.mg300.stabilised_pct, Some(0.0), "R1: no medics");
        assert!(
            r.medics.stabilised_pct.unwrap() > 0.0,
            "R2: medics stabilise"
        );
        assert_eq!(r.post.pass, Some(true), "R3, R4: {:?}", r.post);
        assert_eq!(r.refund.pass, Some(true), "R5: {:?}", r.refund);
        assert_eq!(r.reinforce.pass, Some(true), "R6: {:?}", r.reinforce);
    }

    /// The scene with wounds on plays the same at 1, 2, 3 and 8 threads
    /// (state hashes equal every 30 ticks for 90 s). Control: another seed differs.
    #[test]
    fn s12_wounds_scene_hash_is_thread_count_free() {
        let hashes = |threads: usize, seed: u64| {
            let args = FortifyArgs {
                threads,
                ..FortifyArgs::default()
            };
            let (mut w, _) = scene(&args, seed, 1, false, Variant::On).unwrap();
            let mut v = Vec::new();
            for _ in 0..90 {
                for _ in 0..30 {
                    w.step(&[]);
                }
                v.push(w.state_hash());
            }
            let hits: u32 = w
                .game()
                .unwrap()
                .players()
                .iter()
                .map(|p| {
                    let s = &p.s12;
                    s.torso_hits
                        .iter()
                        .chain(&s.head_hits)
                        .chain(&s.leg_hits)
                        .sum::<u32>()
                })
                .sum();
            (v, hits)
        };
        let (one, hits) = hashes(1, 1);
        assert!(hits > 0, "wounds came into play");
        for n in [2, 3, 8] {
            assert_eq!(hashes(n, 1).0, one, "{n} threads");
        }
        assert_ne!(hashes(1, 2).0, one, "control: another seed");
    }

    /// One-off: the S12 rows on seeds 1..=`S12_SEEDS` (default 1; prints
    /// the values); run by hand in release.
    #[test]
    #[ignore]
    fn s12_rows_one_off() {
        let t = std::time::Instant::now();
        let n: u64 = std::env::var("S12_SEEDS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1);
        let r = s12_rows(&FortifyArgs::default(), 1..n + 1).unwrap();
        eprintln!("{}", serde_json::to_string(&r).unwrap());
        eprintln!("s12 rows wall s: {:.1}", t.elapsed().as_secs_f64());
    }
}
