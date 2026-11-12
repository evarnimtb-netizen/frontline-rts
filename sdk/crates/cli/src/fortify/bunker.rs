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
//! * `small_arms_hp_lost`: the other player's rifle squad and MG team 150 m
//!   west fire for 60 s; hp lost over the six pieces, all runs.
//! * `survived_81mm_x3_pct`, `sheltered_losses`: three 81 mm bombs of the
//!   other faction's mortar on the roof centre, 2 s apart.
//! * `roof_satchel_losses`: a satchel dropped 2 m above the roof's top.
//! * `slit_satchel_losses_pct`: a satchel at the slit's inner lip (slit
//!   centre, 5 cm above its bottom, 10 cm inside the front walls' centre
//!   line): share of the five dead after 12 s.
//! * `ap_shots_to_break`: the other side's medium tank gun AP round (Pz IV H
//!   KwK 40 PzGr 39, M4A3 M3 M61) fired across the roof 80 cm above the
//!   surface until it breaks (capped at [`AP_CAP`]); mean.
//! * `mg_losses_pct` and `control.mg_losses_pct`: the MG team alone fires
//!   60 s; share of the five dead, with the bunker and in the bare pit.

use super::*;

/// Distance of the firing squads (m west of the bunker, in front).
const FIRE_M: i32 = 150;
/// Seconds of small-arms and MG fire.
const FIRE_S: u64 = 60;
/// Most AP shots fired at the roof.
pub const AP_CAP: u32 = 20;

/// The bunker of `own` with its five men (pieces first the roof, then in
/// part order), or only the pit when `built` is false.
fn bunker(w: &mut World, own: u8, built: bool) -> Result<(Vec<u32>, Vec<SimId>)> {
    let f = s10a::fort_of(w, "ww2:bunker")?;
    let half = Fx::from(w.match_setup().unwrap().rules.bunker_pit.pit_line).div_int(2);
    let c = at(TRENCH_X, LINE_Y);
    let e = FxVec2::new(half, Fx::ZERO);
    if !w.dig_pit_now(&[c - e, c + e], frontline_content::DigProfile::BunkerPit) {
        bail!("the bunker pit was not dug");
    }
    let before: Vec<u32> = w.game().unwrap().fort().pieces.keys().copied().collect();
    if built && !w.build_now(f, &[c, c + WEST], PlayerId(own)) {
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
    let spots: Vec<FxVec2> = men.iter().map(|&m| pos(w, m)).collect();
    for _ in 0..secs * hz {
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

/// `att`'s squads with tags `roles` fire for [`FIRE_S`] s from
/// [`FIRE_M`] m east at `own`'s bunker (built or not): (piece hp lost, the
/// five dead).
fn fire_run(
    args: &FortifyArgs,
    seed: u64,
    own: u8,
    built: bool,
    roles: &[&str],
) -> Result<(u32, u32)> {
    let att = 1 - own;
    let mut w = world(args, seed)?;
    let (pieces, men) = bunker(&mut w, own, built)?;
    let full = hp_sum(&w, &pieces);
    for (k, role) in roles.iter().enumerate() {
        let sq = match squad(&w, att, role) {
            Ok(s) => s,
            Err(_) => call_in(&mut w, att, role)?,
        };
        w.place_squad(sq, at(TRENCH_X - FIRE_M, LINE_Y - 5 + 10 * k as i32), EAST);
    }
    hold(&mut w, &men, FIRE_S);
    Ok((full - hp_sum(&w, &pieces), dead(&w, &men)))
}

/// Three bombs on the roof: (roof stood, the five dead).
fn bombs_run(args: &FortifyArgs, seed: u64, own: u8) -> Result<(bool, u32)> {
    let mut w = world(args, seed)?;
    let (pieces, men) = bunker(&mut w, own, true)?;
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

/// A satchel of the other player on the roof (`slit` false) or at the
/// slit's inner lip: the five dead.
fn satchel_run(args: &FortifyArgs, seed: u64, own: u8, slit: bool) -> Result<u32> {
    let mut w = world(args, seed)?;
    let (pieces, men) = bunker(&mut w, own, true)?;
    let t = thrower(&mut w, 1 - own)?;
    let item = satchel(&w)?;
    let c = at(TRENCH_X, LINE_Y);
    let (p, z) = if slit {
        // 5 cm above the slit's bottom (the front walls' base).
        let p = c - FxVec2::new(Fx::from_milli(1800), Fx::ZERO);
        (
            p,
            w.game().unwrap().fort().pieces[&pieces[4]].z + Fx::from_milli(50),
        )
    } else {
        let q = w.game().unwrap().fort().pieces[&pieces[0]].clone();
        let b = &w.match_setup().unwrap().forts[q.fort as usize].pieces[q.part as usize];
        (c, q.z + Fx::from(b.half[2]).mul_int(2) + Fx::from_int(2))
    };
    let above = z - w.terrain().height_at(p);
    if !w.drop_item_now(t, item, p, above) {
        bail!("seed {seed}: the satchel was not dropped");
    }
    hold(&mut w, &men, if slit { 12 } else { 5 });
    Ok(dead(&w, &men))
}

/// AP rounds of the other side's medium tank gun across the roof until it
/// breaks: shots fired (at most [`AP_CAP`]).
fn ap_run(args: &FortifyArgs, seed: u64, own: u8) -> Result<u32> {
    let mut w = world(args, seed)?;
    let (pieces, _) = bunker(&mut w, own, true)?;
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
    for seed in seeds {
        for own in [0u8, 1] {
            hp_lost += fire_run(args, seed, own, true, &["line", "mg"])?.0;
            let (s, d) = bombs_run(args, seed, own)?;
            stood += s as u32;
            sheltered += d;
            roof_dead += satchel_run(args, seed, own, false)?;
            slit_dead += satchel_run(args, seed, own, true)?;
            shots += ap_run(args, seed, own)?;
            mg_dead += fire_run(args, seed, own, true, &["mg"])?.1;
            bare_dead += fire_run(args, seed, own, false, &["mg"])?.1;
            n += 1;
        }
    }
    Ok(s10b::BunkerRow {
        small_arms_hp_lost: Some(hp_lost as f64),
        survived_81mm_x3_pct: pct(stood, n),
        sheltered_losses: Some(sheltered as f64),
        slit_satchel_losses_pct: pct(slit_dead, 5 * n),
        roof_satchel_losses: Some(roof_dead as f64),
        ap_shots_to_break: (n > 0).then(|| (shots as f64 * 10.0 / n as f64).round() / 10.0),
        mg_losses_pct: pct(mg_dead, 5 * n),
        control: s10b::BunkerControl {
            mg_losses_pct: pct(bare_dead, 5 * n),
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
        // The bunker's own claims (T107a-c, e) hold on one seed.
        assert_eq!(r.small_arms_hp_lost, Some(0.0));
        assert_eq!(r.survived_81mm_x3_pct, Some(100.0));
        assert_eq!(r.sheltered_losses, Some(0.0));
        assert_eq!(r.roof_satchel_losses, Some(0.0));
    }
}
