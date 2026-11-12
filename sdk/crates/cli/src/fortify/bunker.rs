//! S10b Build E: the fortify-test `bunker` row (T107; docs/slices/
//! S10b-obstacles.md R22-R26, stage file `S10b-build-E.md` 3). Its own
//! module so `s10b.rs` keeps to the mine and obstacle rows.
//!
//! For each seed and each player in turn as the bunker's owner: a bunker
//! (pit dug and pieces standing at once) at (`TRENCH_X`, `LINE_Y`) facing
//! west (down the site's slope, about 8 %: facing up it, the undug lip in
//! the slit stands 14 cm above the slit's bottom and a satchel's blast
//! origin, 30 cm above it, lies in the roof), five men of the owner's rifle squad in the pit (0.4 m apart along
//! x around the centre; the rest 60 m east), held on their spots.
//! * `small_arms_hp_lost`: [`VOLLEY`] staged rounds of the other faction's
//!   rifle and MG (alternating) from 20 m west, level, at the front walls
//!   and the slit (y within 1.5 m of the centre, 5-35 cm above the walls'
//!   base); hp lost over the six pieces, all runs. `control.
//!   small_arms_hp_lost`: the same with armour 1 on the walls (review R6:
//!   the auto-fire run had no evidence a round reached a piece).
//! * `survived_81mm_x3_pct`, `sheltered_losses`: three 81 mm bombs of the
//!   other faction's mortar on the roof centre, 2 s apart.
//! * `roof_satchel_losses`: a satchel dropped 2 m above the roof's top.
//! * `slit_satchel_losses_pct`: a real Demolish order (the other side's
//!   field engineers, 15 m in front) clicked 20 cm outside the slit's
//!   centre; the charge lies where Demolish lays it (the target piece's
//!   outer face nearest the man, at terrain height; the roof and the front
//!   walls tie there and the roof, the lower id, is chosen). The five wait
//!   out of the fight until it is laid, the engineers are then moved off;
//!   share of the five dead after the fuse + 2 s (review R7). `slit_satchel_
//!   uphill_losses_pct`: the same with the bunker facing up the slope;
//!   `back_satchel_losses_pct`: the same at the back wall (no opening). The
//!   first fix run read 100/100/100: a charge at any wall kills the five,
//!   so T107d does not show the slit (owner question in the S10b log).
//! * `ap_shots_to_break`: the other side's medium tank gun AP round (Pz IV H
//!   KwK 40 PzGr 39, M4A3 M3 M61) fired across the roof 80 cm above the
//!   surface until it breaks (capped at [`AP_CAP`]); mean.
//! * `mg_losses_pct`, `control.mg_losses_pct`, `control.open_mg_losses_pct`:
//!   the MG team alone fires 60 s (auto-targeting, 150 m); share of the five
//!   dead with the bunker, in the bare pit, and standing on open ground (no
//!   pit: the open control shows the MG engages that spot).

use super::*;

/// Distance of the firing squads (m west of the bunker, in front).
const FIRE_M: i32 = 150;
/// Seconds of small-arms and MG fire.
const FIRE_S: u64 = 60;
/// Most AP shots fired at the roof.
pub const AP_CAP: u32 = 20;
/// Staged small-arms rounds per run (T107a).
pub const VOLLEY: u32 = 300;

/// What stands at the site: the bunker, its bare pit, or open ground.
#[derive(Clone, Copy, PartialEq)]
enum Site {
    Bunker,
    Pit,
    Open,
}

/// The bunker of `own` with its five men (pieces first the roof, then in
/// part order), only the pit, or open ground.
fn bunker(w: &mut World, own: u8, site: Site) -> Result<(Vec<u32>, Vec<SimId>)> {
    bunker_facing(w, own, site, WEST)
}

/// [`bunker`] with the bunker facing `face` (WEST is down the slope).
fn bunker_facing(
    w: &mut World,
    own: u8,
    site: Site,
    face: FxVec2,
) -> Result<(Vec<u32>, Vec<SimId>)> {
    let built = site == Site::Bunker;
    let f = s10a::fort_of(w, "ww2:bunker")?;
    let half = Fx::from(w.match_setup().unwrap().rules.bunker_pit.pit_line).div_int(2);
    let c = at(TRENCH_X, LINE_Y);
    let e = FxVec2::new(half, Fx::ZERO);
    if site != Site::Open
        && !w.dig_pit_now(&[c - e, c + e], frontline_content::DigProfile::BunkerPit)
    {
        bail!("the bunker pit was not dug");
    }
    let before: Vec<u32> = w.game().unwrap().fort().pieces.keys().copied().collect();
    if built && !w.build_now(f, &[c, c + face], PlayerId(own)) {
        bail!("the bunker was not staged");
    }
    let fort = w.game().unwrap().fort();
    let mut pieces: Vec<(u8, u32)> = fort
        .pieces
        .iter()
        .filter(|(k, _)| !before.contains(k))
        .map(|(&k, q)| (q.part, k))
        .collect();
    pieces.sort();
    let sq = squad(w, own, "line")?;
    let all = members(w, sq);
    for (k, &m) in all.iter().enumerate() {
        let p = if k < 5 {
            c + FxVec2::new(Fx::from_milli(400 * (k as i32 - 2)), Fx::ZERO)
        } else {
            at(TRENCH_X + 60 + k as i32, LINE_Y + 60)
        };
        w.place_soldier(m, p);
    }
    w.step(&[]);
    Ok((
        pieces.into_iter().map(|x| x.1).collect(),
        all.into_iter().take(5).collect(),
    ))
}

/// Steps `secs` holding `men` (living ones) on their spots.
fn hold(w: &mut World, men: &[SimId], secs: u64) {
    let hz = w.match_setup().unwrap().tick_hz as u64;
    hold_ticks(w, men, secs * hz);
}

/// [`hold`] for `ticks` ticks.
fn hold_ticks(w: &mut World, men: &[SimId], ticks: u64) {
    let spots: Vec<FxVec2> = men.iter().map(|&m| pos(w, m)).collect();
    for _ in 0..ticks {
        for (&m, &p) in men.iter().zip(&spots) {
            if s10a::alive(w, m) {
                w.place_soldier(m, p);
            }
        }
        w.step(&[]);
    }
}

fn dead(w: &World, men: &[SimId]) -> u32 {
    men.iter().filter(|&&m| !s10a::alive(w, m)).count() as u32
}

fn hp_sum(w: &World, pieces: &[u32]) -> u32 {
    let f = w.game().unwrap().fort();
    pieces
        .iter()
        .map(|k| f.pieces.get(k).map_or(0, |q| q.hp))
        .sum()
}

fn satchel(w: &World) -> Result<u16> {
    w.match_setup()
        .unwrap()
        .items
        .iter()
        .position(|x| x.kind == ItemKind::Satchel)
        .map(|i| i as u16)
        .ok_or_else(|| anyhow::anyhow!("no satchel item"))
}

/// A man of `p`'s rifle squad, moved 40 m off to throw (out of the fight).
fn thrower(w: &mut World, p: u8) -> Result<SimId> {
    let sq = squad(w, p, "line")?;
    w.place_squad(sq, at(TRENCH_X + 40, LINE_Y + 40), WEST);
    Ok(members(w, sq)[0])
}

fn weapon(w: &World, id: &str) -> Result<u16> {
    w.match_setup()
        .unwrap()
        .weapons
        .iter()
        .position(|x| x.id == id)
        .map(|i| i as u16)
        .ok_or_else(|| anyhow::anyhow!("no weapon {id}"))
}

/// `att`'s MG team fires for [`FIRE_S`] s from [`FIRE_M`] m west at
/// `own`'s five at `site`: the five dead.
fn fire_run(args: &FortifyArgs, seed: u64, own: u8, site: Site) -> Result<u32> {
    let att = 1 - own;
    let mut w = world(args, seed)?;
    let (_, men) = bunker(&mut w, own, site)?;
    let sq = match squad(&w, att, "mg") {
        Ok(s) => s,
        Err(_) => call_in(&mut w, att, "mg")?,
    };
    w.place_squad(sq, at(TRENCH_X - FIRE_M, LINE_Y), EAST);
    hold(&mut w, &men, FIRE_S);
    Ok(dead(&w, &men))
}

/// [`VOLLEY`] staged rounds of `1 - own`'s rifle and MG (alternating) from
/// 20 m west at `own`'s bunker front, the walls at `armour` mm when given:
/// piece hp lost.
fn volley_run(args: &FortifyArgs, seed: u64, own: u8, armour: Option<u32>) -> Result<u32> {
    let mut w = world_with(args, seed, |s| {
        if let (Some(a), Some(f)) = (armour, s.forts.iter_mut().find(|f| f.id == "ww2:bunker")) {
            f.pieces.iter_mut().skip(1).for_each(|p| p.armour_mm = a);
        }
    })?;
    let (pieces, men) = bunker(&mut w, own, Site::Bunker)?;
    let full = hp_sum(&w, &pieces);
    let germany = faction(w.match_setup().unwrap(), 1 - own).ends_with("germany");
    let guns = if germany {
        [weapon(&w, "ww2:kar98k")?, weapon(&w, "ww2:mg42")?]
    } else {
        [weapon(&w, "ww2:m1_garand")?, weapon(&w, "ww2:m1919")?]
    };
    let c = at(TRENCH_X, LINE_Y);
    // The front walls' base (the site's undug surface).
    let base = w.game().unwrap().fort().pieces[&pieces[4]].z;
    for k in 0..VOLLEY as i32 {
        let y = Fx::from_milli((k * 37) % 3000 - 1500);
        let z = base + Fx::from_milli(50 + (k * 13) % 300);
        let to = FxVec2::new(c.x, c.y + y);
        let from = to - FxVec2::new(Fx::from_int(20), Fx::ZERO);
        if !w.fire_round_now(1 - own, guns[k as usize % 2], from.extend(z), to.extend(z)) {
            bail!("seed {seed}: round {k} was not fired");
        }
        hold_ticks(&mut w, &men, 2);
    }
    hold(&mut w, &men, 1);
    Ok(full - hp_sum(&w, &pieces))
}

/// Three bombs on the roof: (roof stood, the five dead).
fn bombs_run(args: &FortifyArgs, seed: u64, own: u8) -> Result<(bool, u32)> {
    let mut w = world(args, seed)?;
    let (pieces, men) = bunker(&mut w, own, Site::Bunker)?;
    let mortar = s10a_mortar(&w, 1 - own)?;
    for _ in 0..3 {
        if !w.drop_shell_now(1 - own, mortar, at(TRENCH_X, LINE_Y)) {
            bail!("seed {seed}: the bomb was not dropped");
        }
        hold(&mut w, &men, 2);
    }
    Ok((hp_sum(&w, &pieces[..1]) > 0, dead(&w, &men)))
}

/// The other faction's 81 mm mortar (as the S10a roof row).
fn s10a_mortar(w: &World, p: u8) -> Result<u16> {
    let s = w.match_setup().unwrap();
    let germany = faction(s, p).ends_with("germany");
    weapon(
        w,
        if germany {
            "ww2:grw34"
        } else {
            "ww2:m1_mortar"
        },
    )
}

/// A satchel of the other player dropped on the roof: the five dead.
fn satchel_run(args: &FortifyArgs, seed: u64, own: u8) -> Result<u32> {
    let mut w = world(args, seed)?;
    let (pieces, men) = bunker(&mut w, own, Site::Bunker)?;
    let t = thrower(&mut w, 1 - own)?;
    let item = satchel(&w)?;
    let c = at(TRENCH_X, LINE_Y);
    let q = w.game().unwrap().fort().pieces[&pieces[0]].clone();
    let b = &w.match_setup().unwrap().forts[q.fort as usize].pieces[q.part as usize];
    let z = q.z + Fx::from(b.half[2]).mul_int(2) + Fx::from_int(2);
    let above = z - w.terrain().height_at(c);
    if !w.drop_item_now(t, item, c, above) {
        bail!("seed {seed}: the satchel was not dropped");
    }
    hold(&mut w, &men, 5);
    Ok(dead(&w, &men))
}

/// The other side's field engineers demolish at `own`'s bunker slit (a
/// real Demolish order, the bunker facing `face`): the five dead, or None
/// when no charge was laid in 120 s.
fn slit_demolish_run(
    args: &FortifyArgs,
    seed: u64,
    own: u8,
    face: FxVec2,
    back: bool,
) -> Result<Option<u32>> {
    let mut w = world(args, seed)?;
    let (_, men) = bunker_facing(&mut w, own, Site::Bunker, face)?;
    // The owner's squad waits far off while the engineers work (else the
    // five and the engineers fight and the losses are not the charge's).
    let spots: Vec<FxVec2> = men.iter().map(|&m| pos(&w, m)).collect();
    let sq = squad(&w, own, "line")?;
    w.place_squad(sq, at(TRENCH_X + 300, LINE_Y + 300), WEST);
    let att = 1 - own;
    let eng = s10a::call_in_card(&mut w, att, &s10a::FIELD_ENGINEERS)?;
    let c = at(TRENCH_X, LINE_Y);
    // `back`: the same at the back wall's centre (no opening there).
    let side = if back { face.mul_int(-1) } else { face };
    w.place_squad(eng, c + side.mul_int(15), side.mul_int(-1));
    // 20 cm outside the slit's centre (the front's outer face is 2.1 m out).
    let click = c + side.mul_int(23).div_int(10);
    step_with(
        &mut w,
        att,
        vec![CommandKind::Demolish {
            squad: eng,
            at: click,
        }],
    );
    let hz = w.match_setup().unwrap().tick_hz as u64;
    let mut laid = false;
    for _ in 0..120 * hz {
        if !w.game().unwrap().fort().charges.is_empty() {
            laid = true;
            break;
        }
        w.step(&[]);
    }
    if !laid {
        return Ok(None);
    }
    w.place_squad(eng, at(TRENCH_X - 300, LINE_Y - 300), EAST);
    for (&m, &p) in men.iter().zip(&spots) {
        w.place_soldier(m, p);
    }
    let fuse = w
        .match_setup()
        .unwrap()
        .rules
        .fortify
        .as_ref()
        .map_or(10, |t| t.demolish_fuse_s as u64);
    hold(&mut w, &men, fuse + 2);
    Ok(Some(dead(&w, &men)))
}

/// AP rounds of the other side's medium tank gun across the roof until it
/// breaks: shots fired (at most [`AP_CAP`]).
fn ap_run(args: &FortifyArgs, seed: u64, own: u8) -> Result<u32> {
    let mut w = world(args, seed)?;
    let (pieces, _) = bunker(&mut w, own, Site::Bunker)?;
    let s = w.match_setup().unwrap();
    let (gun, round) = if faction(s, 1 - own).ends_with("germany") {
        ("ww2:kwk40_l48", "ww2:75mm_pzgr39")
    } else {
        ("ww2:m3_75mm", "ww2:75mm_m61")
    };
    let ai = s
        .ammo
        .iter()
        .position(|a| a.id == round)
        .ok_or_else(|| anyhow::anyhow!("no ammo {round}"))? as u16;
    let wi = weapon(&w, gun)?;
    let hz = w.match_setup().unwrap().tick_hz as u64;
    let c = at(TRENCH_X, LINE_Y);
    let z = w.terrain().base_height_at(c) + Fx::from_milli(800);
    for n in 1..=AP_CAP {
        let from = c.extend(z) + frontline_sim::FxVec3::new(Fx::ZERO, Fx::from_int(-30), Fx::ZERO);
        let to = c.extend(z) + frontline_sim::FxVec3::new(Fx::ZERO, Fx::from_int(30), Fx::ZERO);
        if !w.fire_shell_now(1 - own, wi, ai, from, to) {
            bail!("seed {seed}: the shot was not fired");
        }
        for _ in 0..hz {
            w.step(&[]);
        }
        if hp_sum(&w, &pieces[..1]) == 0 {
            return Ok(n);
        }
    }
    Ok(AP_CAP)
}

fn pct(a: u32, b: u32) -> Option<f64> {
    (b > 0).then(|| (a as f64 * 1000.0 / b as f64).round() / 10.0)
}

/// The `bunker` row (T107) over `seeds`, each player owning the bunker in
/// turn.
pub fn bunker_row(args: &FortifyArgs, seeds: std::ops::Range<u64>) -> Result<s10b::BunkerRow> {
    let (mut hp_lost, mut stood, mut sheltered, mut roof_dead) = (0u32, 0u32, 0u32, 0u32);
    let (mut slit_dead, mut mg_dead, mut bare_dead, mut shots, mut n) = (0u32, 0, 0, 0, 0u32);
    let (mut thin_lost, mut open_dead) = (0u32, 0u32);
    let (mut slit_n, mut up_dead, mut up_n) = (0u32, 0u32, 0u32);
    let (mut back_dead, mut back_n) = (0u32, 0u32);
    for seed in seeds {
        for own in [0u8, 1] {
            hp_lost += volley_run(args, seed, own, None)?;
            thin_lost += volley_run(args, seed, own, Some(1))?;
            let (s, d) = bombs_run(args, seed, own)?;
            stood += s as u32;
            sheltered += d;
            roof_dead += satchel_run(args, seed, own)?;
            if let Some(d) = slit_demolish_run(args, seed, own, WEST, true)? {
                back_dead += d;
                back_n += 1;
            }
            if let Some(d) = slit_demolish_run(args, seed, own, WEST, false)? {
                slit_dead += d;
                slit_n += 1;
            }
            if let Some(d) = slit_demolish_run(args, seed, own, EAST, false)? {
                up_dead += d;
                up_n += 1;
            }
            shots += ap_run(args, seed, own)?;
            mg_dead += fire_run(args, seed, own, Site::Bunker)?;
            bare_dead += fire_run(args, seed, own, Site::Pit)?;
            open_dead += fire_run(args, seed, own, Site::Open)?;
            n += 1;
        }
    }
    Ok(s10b::BunkerRow {
        small_arms_hp_lost: Some(hp_lost as f64),
        survived_81mm_x3_pct: pct(stood, n),
        sheltered_losses: Some(sheltered as f64),
        slit_satchel_losses_pct: pct(slit_dead, 5 * slit_n),
        slit_satchel_uphill_losses_pct: pct(up_dead, 5 * up_n),
        slit_charge_laid_pct: pct(slit_n + up_n, 2 * n),
        back_satchel_losses_pct: pct(back_dead, 5 * back_n),
        roof_satchel_losses: Some(roof_dead as f64),
        ap_shots_to_break: (n > 0).then(|| (shots as f64 * 10.0 / n as f64).round() / 10.0),
        mg_losses_pct: pct(mg_dead, 5 * n),
        control: s10b::BunkerControl {
            mg_losses_pct: pct(bare_dead, 5 * n),
            small_arms_hp_lost: Some(thin_lost as f64),
            open_mg_losses_pct: pct(open_dead, 5 * n),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bunker_row_fills_every_field() {
        // Control: `bunker_row` returning `BunkerRow::default()` (every
        // field null, as before Build E) -> the `is_some` asserts fail.
        let r = bunker_row(&FortifyArgs::default(), 1..2).unwrap();
        eprintln!("{r:?}");
        let v = serde_json::to_value(&r).unwrap();
        for k in [
            "small_arms_hp_lost",
            "survived_81mm_x3_pct",
            "sheltered_losses",
            "slit_satchel_losses_pct",
            "roof_satchel_losses",
            "ap_shots_to_break",
            "mg_losses_pct",
        ] {
            assert!(v[k].is_number(), "{k} is filled");
        }
        assert!(r.control.mg_losses_pct.is_some());
        // Review R7. Control: drop the Demolish command in
        // `slit_demolish_run` -> no charge is laid and this fails.
        assert_eq!(r.slit_charge_laid_pct, Some(100.0), "every Demolish laid");
        // Review R6. Control: aim the volleys 30 m off the bunker (or never
        // place the MG team) -> the controls read 0 and these fail, while
        // the bunker's own 0s above would still pass.
        assert!(
            r.control.small_arms_hp_lost.unwrap() > 0.0,
            "thin walls lose hp to the same volleys"
        );
        assert!(
            r.control.open_mg_losses_pct.unwrap() > 0.0,
            "the MG kills men on open ground at that spot"
        );
        // The bunker's own claims (T107a-c, e) hold on one seed.
        assert_eq!(r.small_arms_hp_lost, Some(0.0));
        assert_eq!(r.survived_81mm_x3_pct, Some(100.0));
        assert_eq!(r.sheltered_losses, Some(0.0));
        assert_eq!(r.roof_satchel_losses, Some(0.0));
    }
}
