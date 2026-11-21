//! S13a Build A tests (docs/slices/S13-heavy-wave.md 3, 5): the SAVE 31
//! layout and its load checks, rule O1 and `special_min_pct`. Each test
//! names its control: the edit that makes it fail.
#![allow(clippy::float_arithmetic)] // percentages for the targets only

use super::s6_tests::{flat_world_with, spawn, steps};
use super::*;
use frontline_content::{AmmoMechanism, SlopeClass};

/// A flat match whose player 1 is an AI, with the S13 AI fields staged.
fn staged() -> World {
    let mut w = flat_world_with(1, |s| s.players[1].is_ai = true);
    steps(&mut w, 3);
    spawn(&mut w, 1, "ww2:usa_rifle_squad", 300, 300);
    steps(&mut w, 30);
    let tick = w.tick();
    assert!(w.with_ai_state(1, |ai| {
        ai.armour_seen_at = tick - 5;
        ai.heavy_plan_since = tick - 2;
        let k = &mut ai.counters;
        (k.heavy_plans, k.heavy_buys, k.heavy_drops, k.push_turnbacks) = (2, 1, 1, 3);
    }));
    w
}

fn ai(s: &mut SimState) -> &mut crate::ai::AiState {
    s.game.as_mut().unwrap().ai[1].as_mut().unwrap()
}

/// Plan 5: the S13 AI fields save, load and hash. Control: an unedited
/// reload keeps the hash (each edit must change it).
#[test]
fn s13_layout_roundtrips_and_hashes() {
    let w = staged();
    let bytes = w.save();
    let back = World::load(&bytes, 1).expect("the S13 state loads");
    assert_eq!(back.game().unwrap().ai[1], w.game().unwrap().ai[1]);
    assert_eq!(back.full_state_hash(), w.full_state_hash());
    assert_eq!(back.save(), bytes);
    let h = back.full_state_hash();
    assert_eq!(World::load(&back.save(), 1).unwrap().full_state_hash(), h);
    type Edit = Box<dyn Fn(&mut SimState)>;
    let edits: Vec<Edit> = vec![
        Box::new(|s| ai(s).armour_seen_at -= 1),
        Box::new(|s| ai(s).heavy_plan_since = 0),
        Box::new(|s| ai(s).counters.heavy_plans += 1),
        Box::new(|s| ai(s).counters.heavy_buys += 1),
        Box::new(|s| ai(s).counters.heavy_drops += 1),
        Box::new(|s| ai(s).counters.push_turnbacks += 1),
    ];
    for (k, e) in edits.into_iter().enumerate() {
        let mut x = World::load(&bytes, 1).unwrap();
        e(&mut x.state);
        let y = World::load(&x.save(), 1).unwrap_or_else(|e| panic!("edit {k}: {e:?}"));
        assert_ne!(y.full_state_hash(), h, "edit {k} is not hashed");
    }
}

/// Plan 5 load validation, one case per check. Control: the staged save
/// itself loads (checked first), so each failure is the edit's.
#[test]
fn load_rejects_bad_s13_state() {
    let w = staged();
    let bytes = w.save();
    World::load(&bytes, 1).expect("control: the staged save loads");
    let tick = w.tick();
    type Edit = Box<dyn Fn(&mut SimState)>;
    let cases: Vec<(&str, Edit)> = vec![
        (
            "armour_seen_at in the future",
            Box::new(move |s| ai(s).armour_seen_at = tick + 1),
        ),
        (
            "heavy_plan_since in the future",
            Box::new(move |s| ai(s).heavy_plan_since = tick + 1),
        ),
    ];
    for (name, e) in cases {
        let mut x = World::load(&bytes, 1).unwrap();
        e(&mut x.state);
        assert!(World::load(&x.save(), 1).is_err(), "{name} loads");
    }
}

/// Plan 3.2, 5: the S13 setup keys are part of the setup (saved, hashed),
/// and a slope class on an HE or HEAT round, or a heavy flag on a soldier
/// card, is refused. Control: the unedited setup starts a match.
#[test]
fn s13_setup_keys_are_hashed_and_checked() {
    let w = flat_world_with(1, |_| {});
    let base = w.game().unwrap().setup.clone();
    let h0 = postcard::to_allocvec(&base).unwrap();
    let ap = base
        .ammo
        .iter()
        .position(|a| a.mechanism == AmmoMechanism::Aphe)
        .expect("an aphe round");
    let he = base
        .ammo
        .iter()
        .position(|a| a.mechanism == AmmoMechanism::He)
        .expect("an he round");
    let veh_card = base
        .squad_types
        .iter()
        .position(|t| t.vehicle.is_some())
        .unwrap();
    let man_card = base
        .squad_types
        .iter()
        .position(|t| t.vehicle.is_none())
        .unwrap();
    type Edit = Box<dyn Fn(&mut frontline_content::MatchSetup)>;
    let good: Vec<Edit> = vec![
        Box::new(move |s| s.ammo[ap].slope_class = Some(SlopeClass::Apcbc)),
        Box::new(move |s| s.ammo[ap].slope_class = Some(SlopeClass::Apcr)),
        Box::new(move |s| s.squad_types[veh_card].ai_heavy = true),
        Box::new(|s| s.vehicles[0].ai_overwatch = true),
        Box::new(|s| s.rules.armour.obliquity = 0),
        Box::new(|s| s.rules.armour.td_lo_pct = 30),
        Box::new(|s| s.rules.armour.td_hi_pct = 100),
        Box::new(|s| s.rules.armour.apcbc_w_pct = 50),
        Box::new(|s| s.rules.armour.apcr_w_pct = 50),
        Box::new(|s| s.rules.vehicles.gunnery.special_min_pct = 25),
        Box::new(|s| s.rules.ai.armour.heavy_buy = 1),
        Box::new(|s| s.rules.ai.armour.heavy_seen_s = 200),
        Box::new(|s| s.rules.ai.armour.heavy_save_max_s = 200),
        Box::new(|s| s.rules.ai.armour.heavy_overwatch = 1),
        Box::new(|s| s.rules.ai.armour.push_limit = 1),
        Box::new(|s| s.rules.ai.armour.push_without_supply_m = 200),
        Box::new(|s| s.rules.ai.armour.tank_reserve_rounds = 5),
    ];
    for (k, e) in good.into_iter().enumerate() {
        let mut s = base.clone();
        e(&mut s);
        assert_ne!(
            postcard::to_allocvec(&s).unwrap(),
            h0,
            "edit {k} is not in the setup bytes"
        );
        assert!(s.validate().is_empty(), "edit {k}: {:?}", s.validate());
    }
    let bad: Vec<(&str, Edit)> = vec![
        (
            "an he round with a class",
            Box::new(move |s| s.ammo[he].slope_class = Some(SlopeClass::Apcbc)),
        ),
        (
            "a soldier card marked heavy",
            Box::new(move |s| s.squad_types[man_card].ai_heavy = true),
        ),
        (
            "td_lo_pct not below td_hi_pct",
            Box::new(|s| s.rules.armour.td_lo_pct = 90),
        ),
        ("obliquity 2", Box::new(|s| s.rules.armour.obliquity = 2)),
        (
            "apcr_w_pct over 100",
            Box::new(|s| s.rules.armour.apcr_w_pct = 101),
        ),
        (
            "special_min_pct over 100",
            Box::new(|s| s.rules.vehicles.gunnery.special_min_pct = 101),
        ),
        ("heavy_buy 2", Box::new(|s| s.rules.ai.armour.heavy_buy = 2)),
    ];
    assert!(
        base.validate().is_empty(),
        "control: the unedited setup is valid"
    );
    for (name, e) in bad {
        let mut s = base.clone();
        e(&mut s);
        assert!(!s.validate().is_empty(), "{name} is accepted");
    }
}

// ------------------------------------------------------------ rule O1 --

use crate::armour::{
    AmmoTable, Face, Obliquity, PPM, aimed_descent, deg, impact_cos, pen_chance_ppm,
    pen_chance_ppm_at, penetrates, penetrates_at, plate_normal, shot_dir,
};
use frontline_content::tuning::ArmourTuning;

/// A test round: B&L 0° rows at 100, 500, 1000, 1500, 2000 m (plan 4.1),
/// its calibre and class, under `[armour]` `t`.
fn round(rows: [u32; 5], cal: u32, class: Option<SlopeClass>, t: &ArmourTuning) -> AmmoTable {
    let o1 = class.and_then(|k| {
        let mut a = frontline_content::vehicle::AmmoSetup {
            id: "t".into(),
            name: "t".into(),
            mechanism: AmmoMechanism::Aphe,
            caliber_mm: cal,
            speed: frontline_content::Fixed::from_int(800),
            penetration: Vec::new(),
            pen_ref_deg: 0,
            ricochet_deg: 70,
            nonpen_damage: 0,
            track_damage: 0,
            core: None,
            spall: None,
            filler: None,
            blast: None,
            supply: None,
            rare: false,
            slope_class: None,
        };
        a.slope_class = Some(k);
        Obliquity::of(&a, t)
    });
    AmmoTable {
        mechanism: Some(AmmoMechanism::Aphe),
        cos_ricochet: deg(70).cos(),
        cos_ref: Fx::ONE,
        rows: [100, 500, 1000, 1500, 2000]
            .iter()
            .zip(rows)
            .map(|(&r, mm)| (Fx::from_int(r), Fx::from_int(mm as i32)))
            .collect(),
        o1,
    }
}

const M62: ([u32; 5], u32) = ([125, 116, 106, 97, 89], 76);
const M93: ([u32; 5], u32) = ([239, 208, 175, 147, 124], 76);
const M82: ([u32; 5], u32) = ([164, 152, 137, 126, 114], 90);
const PZGR3942: ([u32; 5], u32) = ([185, 168, 149, 132, 116], 75);
const PZGR39_88: ([u32; 5], u32) = ([162, 151, 138, 126, 116], 88);

/// Chance (%) of round `r` against a front plate `t_tenths` / 10 mm at
/// `slope` deg, at `range` m and horizontal `yaw` deg, a flat shot (the
/// plan's calculation, `S13-research-obliquity.py`; [`DESCENT`] adds the
/// aimed descent of an 800 m/s shell).
fn pct(r: &AmmoTable, t_tenths: i32, slope: i32, range: i32, yaw: i32) -> f64 {
    let t = Fx::from_ratio(t_tenths as i64, 10);
    let rg = Fx::from_int(range);
    let d = if DESCENT.with(|x| x.get()) {
        aimed_descent(Fx::from_int(800), rg)
    } else {
        Fx::ZERO
    };
    let c = impact_cos(
        shot_dir(Face::Front, deg(yaw), d),
        plate_normal(Face::Front, slope),
    );
    let ppm = pen_chance_ppm_at(r.pen0(rg), c, r.c_pen(c, t), t, Fx::ZERO, r.cos_ricochet);
    f64::from(ppm) * 100.0 / f64::from(PPM)
}

thread_local! {
    /// Whether [`pct`] adds the shell's aimed descent (default: flat).
    static DESCENT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// T151a-g (plan 3.4) under `[armour]` `t`: the failed targets by name.
fn t151(t: &ArmourTuning) -> Vec<&'static str> {
    let ap = Some(SlopeClass::Apcbc);
    let cr = Some(SlopeClass::Apcr);
    let m62 = round(M62.0, M62.1, ap, t);
    let m93 = round(M93.0, M93.1, cr, t);
    let m82 = round(M82.0, M82.1, ap, t);
    let p75 = round(PZGR3942.0, PZGR3942.1, ap, t);
    let p88 = round(PZGR39_88.0, PZGR39_88.1, ap, t);
    // Panther glacis 80 @ 55, nose 60 @ 55; Sherman glacis 63.5 @ 47;
    // Jumbo glacis 101.6 @ 47 (plan 4.2).
    let checks = [
        ("T151a", pct(&m62, 800, 55, 100, 0) == 0.0),
        ("T151b 100 m", pct(&m93, 800, 55, 100, 0) >= 25.0),
        ("T151b 400 m", pct(&m93, 800, 55, 400, 0) <= 15.0),
        ("T151c", pct(&m82, 800, 55, 100, 0) <= 20.0),
        ("T151d", pct(&m82, 600, 55, 350, 0) >= 80.0),
        ("T151e", pct(&m62, 600, 55, 350, 0) <= 30.0),
        ("T151f 0 deg", pct(&p75, 635, 47, 1000, 0) >= 90.0),
        ("T151f 30 deg", pct(&p75, 635, 47, 1000, 30) >= 90.0),
        ("T151g", pct(&p88, 1016, 47, 300, 0) <= 20.0),
    ];
    checks
        .iter()
        .filter(|(_, ok)| !ok)
        .map(|(n, _)| *n)
        .collect()
}

/// T151a-g (plan 3.4) at the shipped `[armour]`. Control: with
/// `obliquity` 0 (the cosine law) T151b-c fail (the 90 mm and the HVAP
/// pierce the glacis), so the rule is what passes them.
#[test]
fn t151_calibration_targets() {
    let t = ArmourTuning::default();
    assert_eq!(t151(&t), Vec::<&str>::new());
    // Plan 3.3's rows, at the defaults: M93 at 100 m 28 %, M82 at 100 m 0.
    let m93 = round(M93.0, M93.1, Some(SlopeClass::Apcr), &t);
    let p = pct(&m93, 800, 55, 100, 0);
    assert!((20.0..=35.0).contains(&p), "M93 vs glacis at 100 m: {p}");
    let off = ArmourTuning { obliquity: 0, ..t };
    let failed = t151(&off);
    assert!(failed.contains(&"T151c"), "{failed:?}");
    assert!(failed.contains(&"T151b 400 m"), "{failed:?}");
    // With the shell's aimed descent (as a match shot) the defaults pass
    // too.
    DESCENT.with(|x| x.set(true));
    assert_eq!(t151(&t), Vec::<&str>::new(), "with descent");
    DESCENT.with(|x| x.set(false));
}

/// Plan 3.2: every (lo, hi) in {25,30,40,50} x {75,80,90,100} passes
/// T151a-g on flat shots (the calibration is not fragile; with the aimed
/// descent the corner (50, 100) misses T151e by a point: 31 % > 30 %).
/// Control: (lo, hi) = (0, 300) fails (the rule almost off).
#[test]
fn t151_window_of_ramps() {
    for lo in [25, 30, 40, 50] {
        for hi in [75, 80, 90, 100] {
            let t = ArmourTuning {
                td_lo_pct: lo,
                td_hi_pct: hi,
                ..ArmourTuning::default()
            };
            assert_eq!(t151(&t), Vec::<&str>::new(), "({lo}, {hi})");
        }
    }
    let wide = ArmourTuning {
        td_lo_pct: 0,
        td_hi_pct: 300,
        ..ArmourTuning::default()
    };
    assert!(!t151(&wide).is_empty());
}

/// Plan 3.5: a round without a class is the cosine law, bit for bit, at
/// every plate, angle and range; also with the switch off. Control: an
/// `apcbc` round on a T/d 1 plate at 55° is lower.
#[test]
fn no_class_is_the_cosine_law() {
    let t = ArmourTuning::default();
    let plain = round(M82.0, M82.1, None, &t);
    let ap = round(M82.0, M82.1, Some(SlopeClass::Apcbc), &t);
    let off = round(
        M82.0,
        M82.1,
        Some(SlopeClass::Apcbc),
        &ArmourTuning {
            obliquity: 0,
            ..t.clone()
        },
    );
    assert!(plain.o1.is_none() && off.o1.is_none() && ap.o1.is_some());
    for th in [10, 30, 63, 90, 150] {
        for s in [0, 20, 47, 55, 70] {
            for r in [100, 500, 1500] {
                let x = Fx::from_int(th);
                let rg = Fx::from_int(r);
                let c = impact_cos(
                    shot_dir(Face::Front, Fx::ZERO, Fx::ZERO),
                    plate_normal(Face::Front, s),
                );
                assert_eq!(plain.c_pen(c, x), c);
                assert_eq!(off.c_pen(c, x), c);
                let old = pen_chance_ppm(plain.pen0(rg), c, x, Fx::ZERO, plain.cos_ricochet);
                let new = pen_chance_ppm_at(
                    plain.pen0(rg),
                    c,
                    plain.c_pen(c, x),
                    x,
                    Fx::ZERO,
                    plain.cos_ricochet,
                );
                assert_eq!(old, new);
                for v in [-crate::armour::PEN_BAND, Fx::ZERO, crate::armour::PEN_BAND] {
                    assert_eq!(
                        penetrates(plain.pen0(rg), c, x, Fx::ONE, plain.cos_ricochet, v),
                        penetrates_at(plain.pen0(rg), c, c, x, Fx::ONE, plain.cos_ricochet, v)
                    );
                }
            }
        }
    }
    // Control: T/d 1 (90 mm) at 55°.
    let c = deg(55).cos();
    let x = Fx::from_int(90);
    assert!(ap.c_pen(c, x) < c);
    let rg = Fx::from_int(100);
    assert!(
        pen_chance_ppm_at(ap.pen0(rg), c, ap.c_pen(c, x), x, Fx::ZERO, ap.cos_ricochet)
            < pen_chance_ppm(ap.pen0(rg), c, x, Fx::ZERO, ap.cos_ricochet)
    );
}

/// Plan 3.5: a plate thin against the calibre (T/d 0.3) keeps the cosine
/// law. Control: T/d 1.2 is lower.
#[test]
fn overmatch_keeps_the_cosine() {
    let t = ArmourTuning::default();
    let r = round(M82.0, M82.1, Some(SlopeClass::Apcbc), &t);
    let c = deg(60).cos();
    assert_eq!(r.c_pen(c, Fx::from_int(27)), c, "T/d 0.3");
    assert!(r.c_pen(c, Fx::from_int(108)) < c, "T/d 1.2");
    // The ramp is continuous: halfway (T/d 0.65) lies between.
    let mid = r.c_pen(c, Fx::from_ratio(585, 10));
    assert!(mid < c && mid > r.c_pen(c, Fx::from_int(108)));
}

/// Plan 3.5: on a slope an `apcr` round loses more than an `apcbc` one.
/// Control: at 0° both are the cosine law (`c` 1).
#[test]
fn apcr_loses_more_than_apcbc_on_a_slope() {
    let t = ArmourTuning::default();
    let ap = round(M93.0, M93.1, Some(SlopeClass::Apcbc), &t);
    let cr = round(M93.0, M93.1, Some(SlopeClass::Apcr), &t);
    let x = Fx::from_int(80);
    let c = deg(55).cos();
    assert!(cr.c_pen(c, x) < ap.c_pen(c, x));
    assert!(ap.c_pen(c, x) < c);
    // Full weight: c^2 and c^1.5 (fixed point, within 1e-3).
    let f = |v: Fx| f64::from(v.to_f32());
    assert!((f(cr.c_pen(c, x)) - f(c).powi(2)).abs() < 1e-3);
    assert!((f(ap.c_pen(c, x)) - f(c).powf(1.5)).abs() < 1e-3);
    assert_eq!(cr.c_pen(Fx::ONE, x), Fx::ONE);
    assert_eq!(ap.c_pen(Fx::ONE, x), Fx::ONE);
    // The class weights: 0 % is the cosine law.
    let w0 = ArmourTuning { apcr_w_pct: 0, ..t };
    assert_eq!(
        round(M93.0, M93.1, Some(SlopeClass::Apcr), &w0).c_pen(c, x),
        c
    );
}

/// Plan 3.5: HE and HEAT rounds refuse a class (setup check; the def check
/// is `heat_and_he_reject_a_class` in the content tests). Control: an
/// `aphe` round takes one.
#[test]
fn heat_and_he_reject_a_class() {
    let w = flat_world_with(1, |_| {});
    let base = w.game().unwrap().setup.clone();
    for (k, a) in base.ammo.iter().enumerate() {
        let mut s = base.clone();
        s.ammo[k].slope_class = Some(SlopeClass::Apcbc);
        let ok = matches!(a.mechanism, AmmoMechanism::Ap | AmmoMechanism::Aphe);
        assert_eq!(s.validate().is_empty(), ok, "{}", a.id);
    }
}

/// T151h (plan 3.4): no pre-S13 round has a class, so every armour table
/// of the ww2 match (and every armour-test case and `armour.rs` test built
/// on them) is the same with `obliquity` 1 and 0. Control: classing one
/// round changes its table.
#[test]
fn t151h_old_rounds_are_unchanged() {
    let w = flat_world_with(1, |_| {});
    let s = &w.game().unwrap().setup;
    let on = &w.game().unwrap().armour;
    let off = crate::armour::ArmourTables::new(
        &s.ammo,
        &s.vehicles,
        &s.weapons,
        &s.rules.vehicles.gunnery,
        &ArmourTuning {
            obliquity: 0,
            ..s.rules.armour.clone()
        },
    );
    assert_eq!(s.rules.armour.obliquity, 1);
    // S13a Build B: the seven new AP rounds are classed; every other table
    // (all the old rounds, the HE rounds) is equal with the rule on and off,
    // and a classed round is the only difference.
    let mut on_old = on.clone();
    for (i, a) in s.ammo.iter().enumerate() {
        if a.slope_class.is_some() {
            assert!(on.ammo[i].o1.is_some(), "{}", a.id);
            assert!(off.ammo[i].o1.is_none(), "{}", a.id);
            on_old.ammo[i] = off.ammo[i].clone();
        } else {
            assert!(on.ammo[i].o1.is_none(), "{}", a.id);
        }
    }
    assert_eq!(on_old, off);
    assert_eq!(s.ammo.iter().filter(|a| a.slope_class.is_some()).count(), 7);
    let mut a = s.ammo[0].clone();
    a.mechanism = AmmoMechanism::Aphe;
    a.slope_class = Some(SlopeClass::Apcbc);
    assert!(AmmoTable::with_o1(&a, &s.rules.armour).o1.is_some());
}

/// Plan 3.5: every logged hit equals the rule with its recorded `c_pen`
/// (the armour-test invariant). A Panzer IV whose PzGr 39 is classed
/// `apcbc` duels a Sherman head-on at 150 m. Control: with `obliquity` 0
/// every record has `c_pen == c`; with it on, a hit on the sloped glacis
/// has `c_pen < c`.
#[test]
fn hit_record_matches_the_rule() {
    let duel = |on: u32| {
        let mut w = flat_world_with(1, |s| {
            s.rules.armour.obliquity = on;
            let k = s
                .ammo
                .iter()
                .position(|a| a.id == "ww2:75mm_pzgr39")
                .unwrap();
            s.ammo[k].slope_class = Some(SlopeClass::Apcbc);
        });
        let ty = |w: &World, id: &str| {
            w.game()
                .unwrap()
                .setup
                .squad_types
                .iter()
                .position(|t| t.id == id)
                .unwrap() as u16
        };
        let (pz, sh) = (
            ty(&w, "ww2:ger_pz4_h_squad"),
            ty(&w, "ww2:usa_m4a3_75_squad"),
        );
        w.spawn_squad_at(
            PlayerId(0),
            pz,
            FxVec2::from_int(300, 300),
            FxVec2::new(Fx::ONE, Fx::ZERO),
        )
        .unwrap();
        w.spawn_squad_at(
            PlayerId(1),
            sh,
            FxVec2::from_int(450, 300),
            FxVec2::new(-Fx::ONE, Fx::ZERO),
        )
        .unwrap();
        w.enable_hit_log();
        let mut log = Vec::new();
        for _ in 0..(90 * 30) {
            w.step(&[]);
            log.extend(w.take_hit_log());
            if log.iter().filter(|h| h.player == 0).count() >= 4 {
                break;
            }
        }
        log
    };
    for on in [0, 1] {
        let log = duel(on);
        let ger: Vec<_> = log.iter().filter(|h| h.player == 0).collect();
        assert!(!ger.is_empty(), "obliquity {on}: the Panzer IV hit nothing");
        for h in &log {
            let again = penetrates_at(
                h.pen0,
                h.c,
                h.c_pen,
                h.thickness,
                h.extra,
                h.cos_ricochet,
                h.v,
            );
            assert_eq!(again, h.result, "obliquity {on}: {h:?}");
            if on == 0 || h.player == 1 {
                assert_eq!(h.c_pen, h.c, "obliquity {on}: {h:?}");
            }
        }
        if on == 1 {
            assert!(
                ger.iter().any(|h| h.c > Fx::ZERO && h.c_pen < h.c),
                "no O1 hit: {ger:?}"
            );
        }
    }
}

// ------------------------------------------------ armour-test helper --

/// Plan 1: armour-test spawns a duel side by card id. Control: naming the
/// first tanks (Panzer IV, Sherman) is exactly the role-based duel
/// (same state hash after a few seconds); a StuG named for player 0 is
/// spawned; a card of the other faction or a soldier card is refused.
#[test]
fn duel_by_card_spawns_the_named_vehicles() {
    use crate::scenarios::{Duel, duel, duel_cards, type_by_card};
    let setup = || flat_world_with(1, |_| {}).game().unwrap().setup.clone();
    let mut a = duel(setup(), 1, Duel::Frontal300, false).unwrap();
    let mut b = duel_cards(
        setup(),
        1,
        Duel::Frontal300,
        false,
        Some(["ww2:ger_pz4_h_squad", "ww2:usa_m4a3_75_squad"]),
    )
    .unwrap();
    for _ in 0..60 {
        a.world.step(&[]);
        b.world.step(&[]);
    }
    assert_eq!(a.world.full_state_hash(), b.world.full_state_hash());
    let s = setup();
    let stug = s
        .squad_types
        .iter()
        .find(|t| {
            t.vehicle.is_some_and(|v| {
                s.vehicles[v as usize].role == frontline_content::VehicleRole::AssaultGun
            }) && type_by_card(&s, 0, &t.id).is_some()
        })
        .map(|t| t.id.clone())
        .expect("a German assault gun card");
    let c = duel_cards(
        setup(),
        1,
        Duel::Frontal300,
        false,
        Some([&stug, "ww2:usa_m4a3_75_squad"]),
    )
    .unwrap();
    let v = c.tanks[0].1;
    let g = c.world.game().unwrap();
    let ty = g.setup.vehicles[g.vehicle(v).unwrap().type_idx as usize].role;
    assert_eq!(ty, frontline_content::VehicleRole::AssaultGun);
    assert!(
        type_by_card(&s, 0, "ww2:usa_m4a3_75_squad").is_none(),
        "other faction"
    );
    assert!(
        duel_cards(
            setup(),
            1,
            Duel::Frontal300,
            false,
            Some(["ww2:ger_grenadiergruppe", "ww2:usa_m4a3_75_squad"])
        )
        .is_err(),
        "a soldier card"
    );
}

// ------------------------------------------------- special_min_pct --

/// Plan 3.6: `Auto` fires the special round only when its chance reaches
/// `special_min_pct`. Control: `special_min_pct` 0 is the S3b rule (any
/// higher chance), so an HVAP at 5 % against 0 % still goes.
#[test]
fn special_min_pct_keeps_rare_rounds_for_real_chances() {
    use crate::gunnery::special_pick;
    let p = |pct: u32| pct * (PPM / 100);
    // The S3b rule (0): any higher chance under `special_below_pct`.
    assert!(special_pick(0, p(5), 50, 0));
    assert!(
        !special_pick(p(50), p(90), 50, 0),
        "standard round good enough"
    );
    assert!(!special_pick(p(10), p(10), 50, 0), "not higher");
    // 25: a 5 % special waits, a 25 % one goes.
    assert!(!special_pick(0, p(5), 50, 25));
    assert!(special_pick(0, p(25), 50, 25));
    assert!(special_pick(p(20), p(60), 50, 25));
    // The shipped rules keep the S3b rule.
    let w = flat_world_with(1, |_| {});
    assert_eq!(
        w.game()
            .unwrap()
            .setup
            .rules
            .vehicles
            .gunnery
            .special_min_pct,
        0
    );
}

// ------------------------------------------------- S13-ai H1-H3 --

const PANTHER: &str = "ww2:ger_panther_g_squad";
const JUMBO: &str = "ww2:usa_m4a3e2_squad";

fn card_of(w: &World, id: &str) -> u16 {
    w.match_setup().unwrap().squad_type_index(id).unwrap()
}

fn ai_of(w: &World, p: usize) -> &crate::ai::AiState {
    w.game().unwrap().ai[p].as_ref().unwrap()
}

fn buys(w: &World, p: usize, id: &str) -> u32 {
    let t = card_of(w, id) as usize;
    ai_of(w, p).counters.bought.get(t).copied().unwrap_or(0)
}

/// Germany (player 0) an AI with no start squads, vehicles unlocked, the
/// Panther ready, H1 as `heavy_buy`; a German rifle squad 150 m from a US
/// Sherman (`sherman`) at the map's middle, so the AI sees armour.
fn heavy_world(
    threads: usize,
    heavy_buy: u32,
    sherman: bool,
    edit: impl FnOnce(&mut frontline_content::MatchSetup),
) -> World {
    let mut w = flat_world_with(threads, |s| {
        s.players[0].is_ai = true;
        let f = s.players[0].faction as usize;
        s.factions[f].start_squads.clear();
        s.rules.start_manpower = 0;
        s.rules.vehicles.unlock_ticks = 0;
        s.rules.ai.armour.heavy_buy = heavy_buy;
        for t in s.squad_types.iter_mut() {
            if t.id == PANTHER {
                t.initial_cooldown_ticks = 0;
            }
        }
        edit(s);
    });
    let mid = w.game().unwrap().setup.map.size_m / 2;
    spawn(&mut w, 0, "ww2:ger_grenadiergruppe", mid - 150, mid);
    if sherman {
        spawn(&mut w, 1, "ww2:usa_m4a3_75_squad", mid, mid);
    }
    w
}

fn set_mp(w: &mut World, p: usize, mp: u32) {
    w.state.game.as_mut().unwrap().players[p].manpower = mp;
}

/// Plays until the AI has seen armour (at most 60 s), then gives it
/// `mp` manpower.
fn after_sighting(w: &mut World, mp: u32) -> bool {
    for _ in 0..60 * 30 {
        if ai_of(w, 0).armour_seen_at != 0 {
            set_mp(w, 0, mp);
            return true;
        }
        w.step(&[]);
    }
    false
}

/// H1: armour seen and the Panther ready: the AI saves for it and buys it
/// within 5 min (income given as 300 a minute); a save and reload in the
/// middle of the plan plays on to the same state; 1 and 4 threads agree.
/// Control: `heavy_buy` 0 never buys it (weight 0) and never dates a
/// sighting.
#[test]
fn heavy_bought_after_armour_seen() {
    let run = |threads: usize, heavy: u32| {
        let mut w = heavy_world(threads, heavy, true, |_| {});
        let seen = after_sighting(&mut w, 300);
        let mut mid = None;
        for s in 0..300 {
            if buys(&w, 0, PANTHER) > 0 {
                break;
            }
            let mp = w.game().unwrap().players[0].manpower;
            set_mp(&mut w, 0, mp + 5);
            if s == 60 && mid.is_none() {
                mid = Some(w.save());
            }
            steps(&mut w, 30);
        }
        (w, seen, mid)
    };
    let (w, seen, mid) = run(1, 1);
    assert!(seen, "the AI saw the Sherman");
    let c = &ai_of(&w, 0).counters;
    assert_eq!(buys(&w, 0, PANTHER), 1, "the Panther is bought");
    assert!(c.heavy_plans >= 1 && c.heavy_buys == 1, "{c:?}");
    assert_eq!(
        ai_of(&w, 0).heavy_plan_since,
        0,
        "the plan ends with the buy"
    );
    // Save fuzz with the new fields in play: mid-plan save reloads.
    let mid = mid.expect("a mid-plan save");
    let back = World::load(&mid, 1).expect("loads");
    assert_eq!(
        World::load(&back.save(), 1).unwrap().full_state_hash(),
        back.full_state_hash()
    );
    assert!(ai_of(&back, 0).heavy_plan_since != 0, "saved mid-plan");
    // Threads 1 vs 4.
    let (w4, _, _) = run(4, 1);
    assert_eq!(w4.full_state_hash(), w.full_state_hash());
    // Control.
    let (off, _, _) = run(1, 0);
    assert_eq!(buys(&off, 0, PANTHER), 0);
    assert_eq!(ai_of(&off, 0).armour_seen_at, 0);
    assert_eq!(ai_of(&off, 0).counters.heavy_plans, 0);
}

/// H1: a plan that cannot be paid within `heavy_save_max_s` is dropped
/// (`heavy_drops`). Control: a longer save keeps it.
#[test]
fn heavy_plan_drops_after_save_max() {
    let run = |save_s: u32| {
        let mut w = heavy_world(1, 1, true, |s| s.rules.ai.armour.heavy_save_max_s = save_s);
        assert!(after_sighting(&mut w, 0));
        for _ in 0..90 {
            set_mp(&mut w, 0, 300);
            steps(&mut w, 30);
        }
        w
    };
    let w = run(60);
    let c = &ai_of(&w, 0).counters;
    assert!(c.heavy_drops >= 1 && c.heavy_plans >= 1, "{c:?}");
    assert_eq!(buys(&w, 0, PANTHER), 0);
    let w = run(600);
    let c = &ai_of(&w, 0).counters;
    assert_eq!((c.heavy_drops, c.heavy_plans), (0, 1), "control: {c:?}");
    assert!(ai_of(&w, 0).heavy_plan_since != 0);
}

/// H1: no enemy armour, no heavy plan however rich the AI. Control:
/// `heavy_bought_after_armour_seen` (a Sherman in sight).
#[test]
fn no_armour_no_heavy() {
    let mut w = heavy_world(1, 1, false, |_| {});
    for _ in 0..120 {
        set_mp(&mut w, 0, 5000);
        steps(&mut w, 30);
    }
    let a = ai_of(&w, 0);
    assert_eq!((a.armour_seen_at, a.counters.heavy_plans), (0, 0));
    assert_eq!(buys(&w, 0, PANTHER), 0);
    assert!(a.counters.bought.iter().sum::<u32>() > 0, "it does buy");
}

/// H1 (S13-11 (A)): with the vehicle limit full no plan starts and no
/// heavy is bought. Control: one more place -> bought.
#[test]
fn heavy_waits_for_a_limit_place() {
    let run = |limit: u32| {
        let mut w = heavy_world(1, 1, true, |s| s.rules.vehicles.limit = limit);
        let mid = w.game().unwrap().setup.map.size_m / 2;
        spawn(&mut w, 0, "ww2:ger_pz4_h_squad", mid - 400, mid - 100);
        assert!(after_sighting(&mut w, 2000));
        for _ in 0..30 {
            steps(&mut w, 30);
        }
        w
    };
    let w = run(1);
    assert_eq!(buys(&w, 0, PANTHER), 0);
    assert_eq!(ai_of(&w, 0).counters.heavy_plans, 0);
    let w = run(2);
    assert_eq!(buys(&w, 0, PANTHER), 1, "control");
}

/// One AI tank of `card` for player `p` (an AI), an enemy rifle squad in
/// sight; true when its squad held an overwatch entry within 60 s.
fn takes_overwatch(
    p: u8,
    card: &str,
    edit: impl FnOnce(&mut frontline_content::MatchSetup),
) -> bool {
    let mut w = flat_world_with(1, |s| {
        s.players[p as usize].is_ai = true;
        s.rules.ai.armour.heavy_overwatch = 1;
        edit(s);
    });
    let mid = w.game().unwrap().setup.map.size_m / 2;
    let q = spawn(&mut w, p, card, mid - 200, mid);
    spawn(
        &mut w,
        1 - p,
        if p == 0 {
            "ww2:usa_rifle_squad"
        } else {
            "ww2:ger_grenadiergruppe"
        },
        mid + 200,
        mid,
    );
    for _ in 0..60 {
        steps(&mut w, 30);
        if ai_of(&w, p as usize).overwatch.contains_key(&q) {
            return true;
        }
    }
    false
}

/// H2: a Panther (`ai_overwatch`) holds overwatch with `heavy_overwatch`
/// 1. Control: 0 -> it takes a tank's tasks.
#[test]
fn panther_takes_overwatch() {
    assert!(takes_overwatch(0, PANTHER, |_| {}));
    assert!(!takes_overwatch(0, PANTHER, |s| s
        .rules
        .ai
        .armour
        .heavy_overwatch = 0));
}

/// H2: the Jumbo (no `ai_overwatch`) leads like a tank. Control: its def
/// flagged -> overwatch.
#[test]
fn jumbo_leads() {
    assert!(!takes_overwatch(1, JUMBO, |_| {}));
    assert!(takes_overwatch(1, JUMBO, |s| {
        let v = s
            .vehicles
            .iter()
            .position(|v| v.id == "ww2:usa_m4a3e2")
            .unwrap();
        s.vehicles[v].ai_overwatch = true;
    }));
}

/// H3: a Panzer IV 600 m out from its base with 9 main rounds (AP and HE
/// above the S3c rearm marks) turns back on the rearm trip
/// (`push_turnbacks`). The default `tank_reserve_rounds` 10 is under the
/// S3c marks (AP 3 + HE 6 + 1), so the test raises it to 20 with 15
/// rounds. Control: `push_limit` 0 -> no trip.
#[test]
fn tank_turns_back_at_push_limit() {
    let run = |push: u32| {
        let mut w = flat_world_with(1, |s| {
            s.players[0].is_ai = true;
            s.rules.ai.armour.push_limit = push;
            s.rules.ai.armour.tank_reserve_rounds = 20;
        });
        let base = w.game().unwrap().base_of(PlayerId(0));
        let mid = w.game().unwrap().setup.map.size_m / 2;
        let at = FxVec2::from_int(mid, mid);
        assert!((at - base).length() > Fx::from_int(500), "far from base");
        let q = spawn(&mut w, 0, "ww2:ger_pz4_h_squad", mid, mid);
        let v = w.game().unwrap().squads[&q].members[0];
        {
            let vs = w.state.game.as_mut().unwrap().vehicles.get_mut(&v).unwrap();
            for (k, n) in vs.ammo.iter_mut().enumerate() {
                *n = [6, 8, 0][k.min(2)];
            }
            vs.loaded = Some(0);
        }
        steps(&mut w, 30 * 20);
        let a = ai_of(&w, 0);
        (a.counters.push_turnbacks, a.tasks.get(&q).map(|t| t.kind))
    };
    let (n, task) = run(1);
    assert_eq!(n, 1);
    assert_eq!(task, Some(crate::ai::TaskKind::Rearm));
    let (n, task) = run(0);
    assert_eq!(n, 0, "control");
    assert_ne!(task, Some(crate::ai::TaskKind::Rearm), "control");
}

// ------------------------------------------- S13a review V1, V3, V5 --

const S13_VEHICLES: [&str; 5] = [
    "ww2:ger_panther_g",
    "ww2:ger_tiger_1",
    "ww2:usa_m4a3_76w",
    "ww2:usa_m36",
    "ww2:usa_m4a3e2",
];

/// Review V3: `hull_body` is box 0 exactly for every pre-S13 def (casemates
/// included: their upper box stays out); for the S13 defs it spans box 0
/// and the superstructure (its top is the superstructure's). Control: the
/// S13 bodies differ from box 0.
#[test]
fn hull_body_is_box_0_before_s13() {
    let w = flat_world_with(1, |_| {});
    let s = &w.game().unwrap().setup;
    for v in &s.vehicles {
        let body = v.hull_body();
        if S13_VEHICLES.contains(&v.id.as_str()) {
            assert_ne!(body, v.hull, "{}", v.id);
            let sup = &v.boxes[1].bounds;
            assert_eq!(v.boxes[1].id, "superstructure");
            // Within one raw unit (the half extent rounds up).
            let top = (body.center.z.0 + body.half.z.0) - (sup.center.z.0 + sup.half.z.0);
            let bottom = (body.center.z.0 - body.half.z.0) - (v.hull.center.z.0 - v.hull.half.z.0);
            assert!(
                top.abs() <= 1 && bottom.abs() <= 1,
                "{}: {top} {bottom}",
                v.id
            );
        } else {
            assert_eq!(body, v.hull, "{}", v.id);
            assert_eq!(v.hull_body_boxes(), vec![0], "{}", v.id);
        }
    }
}

/// Review V2 (what the presenter draws, `match_node::vehicle_types`): the
/// glacis box is box 0 for every pre-S13 def and the superstructure for the
/// S13 defs, so the Tiger's drawn glacis is its 8 deg driver's plate, not
/// the 24 deg nose, and the drawn hull reaches the turret ring (no floating
/// turret). Control: box 0's front (the old `plate(0)`) and top differ.
#[test]
fn presenter_hull_is_the_body_and_its_glacis() {
    let w = flat_world_with(1, |_| {});
    let s = &w.game().unwrap().setup;
    let glacis = |v: &frontline_content::vehicle::VehicleSetup| {
        v.plate((v.glacis_box() * frontline_content::vehicle::PLATE_GROUPS) as u8)
            .map(|p| p.slope_deg)
    };
    for v in &s.vehicles {
        if S13_VEHICLES.contains(&v.id.as_str()) {
            assert_eq!(v.glacis_box(), 1, "{}", v.id);
            let top = v.hull_body().center.z.0 + v.hull_body().half.z.0;
            assert!(
                top > v.hull.center.z.0 + v.hull.half.z.0,
                "control {}",
                v.id
            );
        } else {
            assert_eq!(v.glacis_box(), 0, "{}", v.id);
            assert_eq!(glacis(v), v.plate(0).map(|p| p.slope_deg), "{}", v.id);
        }
    }
    let tiger = s
        .vehicles
        .iter()
        .find(|v| v.id == "ww2:ger_tiger_1")
        .unwrap();
    assert_eq!(glacis(tiger), Some(8));
    assert_eq!(tiger.plate(0).map(|p| p.slope_deg), Some(24), "control");
    let panther = s
        .vehicles
        .iter()
        .find(|v| v.id == "ww2:ger_panther_g")
        .unwrap();
    assert_eq!(glacis(panther), Some(55));
}

/// Review V3a: the spotting hull point is `spot_drop_mm` under the hull
/// body's top (the superstructure's for the S13 defs, box 0's before), so
/// a Tiger's hull is seen over a crest at its real 1.9 m, not box 0's
/// 1.3 m. Control: box 0's top is lower on every S13 def.
#[test]
fn spot_point_is_on_the_hull_body() {
    use crate::ecs::Transform;
    use crate::vehicle::{Geo, VehicleState, frame_of};
    let w = flat_world_with(1, |_| {});
    let s = &w.game().unwrap().setup;
    let drop = s.rules.vehicles.driving.spot_drop_mm;
    for v in &s.vehicles {
        let geo = Geo::of(v, &s.rules.vehicles.driving);
        let vs = VehicleState::new(0, v, &s.rules.vehicles);
        let f = frame_of(SimId(1), PlayerId(0), &Transform::default(), &vs, &geo);
        let [_, hull] = f.spot_points(drop);
        let body = v.hull_body();
        let want = Fx::from(body.center.z) + Fx::from(body.half.z) - Fx::from_milli(drop as i32);
        assert!(
            (hull.z - want).abs() <= Fx::from_milli(2),
            "{}: {:?} {:?}",
            v.id,
            hull.z,
            want
        );
        if S13_VEHICLES.contains(&v.id.as_str()) {
            let box0 = Fx::from(v.hull.center.z) + Fx::from(v.hull.half.z);
            assert!(hull.z > box0, "control {}", v.id);
        } else {
            assert_eq!(geo.body_top, 0, "{}", v.id);
        }
    }
}

/// Review V6 (data): the Panther and Tiger keep no Pz IV turret side doors
/// (no turret hatch on a turret side face); every S13 hull hatch is on the
/// hull body's roof; every S13 track anchor is within 20 cm of box 0's
/// side and every hull-frame ammunition bin inside the hull body; the
/// 76 mm M1A1 reloads as the 3-inch M7 (the same M62 projectile).
/// Control: the Panzer IV (the copied def) has its two side doors.
#[test]
fn s13_data_has_its_own_hatches_bins_and_anchors() {
    use frontline_content::vehicle::Frame;
    let w = flat_world_with(1, |_| {});
    let s = &w.game().unwrap().setup;
    let side_doors = |v: &frontline_content::vehicle::VehicleSetup| {
        let half_y = v.turret.half.y.0;
        v.hatches
            .iter()
            .filter(|h| h.frame == Frame::Turret && (h.pos.y.0.abs() - half_y).abs() <= 6554)
            .count()
    };
    let pz4 = s.vehicles.iter().find(|v| v.id == "ww2:ger_pz4_h").unwrap();
    assert_eq!(side_doors(pz4), 2, "control");
    for id in S13_VEHICLES {
        let v = s.vehicles.iter().find(|v| v.id == id).unwrap();
        assert_eq!(side_doors(v), 0, "{id}");
        let body = v.hull_body();
        let roof = body.center.z.0 + body.half.z.0;
        for h in v.hatches.iter().filter(|h| h.frame == Frame::Hull) {
            assert!(
                (h.pos.z.0 - roof).abs() <= 656,
                "{id}: hatch at {:?}",
                h.pos
            );
        }
        for c in &v.components {
            for a in &c.anchors {
                let gap = v.hull.half.y.0 - a.pos.y.0.abs();
                assert!((0..=13200).contains(&gap), "{id}: anchor {:?}", a.pos);
            }
            for b in c.parts.iter().filter(|b| b.frame == Frame::Hull) {
                for k in 0..3 {
                    let (c0, h0) = match k {
                        0 => (b.bounds.center.x.0, b.bounds.half.x.0),
                        1 => (b.bounds.center.y.0, b.bounds.half.y.0),
                        _ => (b.bounds.center.z.0, b.bounds.half.z.0),
                    };
                    let (bc, bh) = match k {
                        0 => (body.center.x.0, body.half.x.0),
                        1 => (body.center.y.0, body.half.y.0),
                        _ => (body.center.z.0, body.half.z.0),
                    };
                    assert!(
                        c0 - h0 >= bc - bh - 1 && c0 + h0 <= bc + bh + 1,
                        "{id}: {:?} part outside the hull body",
                        c.kind
                    );
                }
            }
        }
    }
    let reload = |id: &str| s.weapons.iter().find(|w| w.id == id).unwrap().reload_ticks;
    assert_eq!(reload("ww2:m1a1_76mm"), reload("ww2:m7_3in"));
}

/// Review V1: the S13 defs' track band (a share of box 0, the lower hull)
/// is within 10 cm of the S3 band (45 % of the whole hull side). Control:
/// at the old 45 % of box 0 it is 25 cm or more short.
#[test]
fn s13_track_band_keeps_the_s3_height() {
    let w = flat_world_with(1, |_| {});
    let s = &w.game().unwrap().setup;
    for id in S13_VEHICLES {
        let v = s.vehicles.iter().find(|v| v.id == id).unwrap();
        let cm = |f: frontline_content::Fixed| f.0 as f64 * 100.0 / 65536.0;
        let box0 = 2.0 * cm(v.hull.half.z);
        // The whole hull side: box 0's bottom to the superstructure's top.
        let sup = &v.boxes[1].bounds;
        let body = cm(sup.center.z) + cm(sup.half.z) - cm(v.hull.center.z) + cm(v.hull.half.z);
        let want = body * 0.45;
        let band = box0 * f64::from(v.track_band_pct) / 100.0;
        assert!((band - want).abs() <= 10.0, "{id}: {band} cm vs {want} cm");
        assert!(want - box0 * 0.45 >= 25.0, "control {id}");
    }
}

/// Review V5: each S13 superstructure runs the hull's full length (its
/// front face at the nose's), so no strip of box 0's roof shows in front
/// of the glacis. Control: box 0's length is the reference.
#[test]
fn s13_superstructure_spans_the_hull() {
    let w = flat_world_with(1, |_| {});
    let s = &w.game().unwrap().setup;
    for id in S13_VEHICLES {
        let v = s.vehicles.iter().find(|v| v.id == id).unwrap();
        let sup = &v.boxes[1].bounds;
        assert!(v.hull.half.x.0 > 0);
        assert_eq!(
            sup.center.x.0 + sup.half.x.0,
            v.hull.center.x.0 + v.hull.half.x.0,
            "{id} front"
        );
        assert_eq!(
            sup.center.x.0 - sup.half.x.0,
            v.hull.center.x.0 - v.hull.half.x.0,
            "{id} rear"
        );
    }
}

/// Review V3c: `Auto` judges the special round on the hull body's worst
/// plate. A Panther facing a Jumbo at 300 m (PzGr 39/42 under 50 % on the
/// glacis, while the nose would pass it) loads PzGr 40/42 first. Control:
/// facing a 76 W (glacis 63.5 mm: the standard round) it fires PzGr 39/42.
#[test]
fn special_round_judged_on_the_glacis() {
    let first_rare = |target: &str| {
        let mut w = flat_world_with(1, |s| {
            for v in &mut s.vehicles {
                v.sight = frontline_content::Fixed::from_int(1000);
            }
        });
        let ty = |w: &World, id: &str| w.match_setup().unwrap().squad_type_index(id).unwrap();
        let (pz, tg) = (ty(&w, PANTHER), ty(&w, target));
        w.spawn_squad_at(
            PlayerId(0),
            pz,
            FxVec2::from_int(300, 300),
            FxVec2::new(Fx::ONE, Fx::ZERO),
        )
        .unwrap();
        let t = w
            .spawn_squad_at(
                PlayerId(1),
                tg,
                FxVec2::from_int(600, 300),
                FxVec2::new(-Fx::ONE, Fx::ZERO),
            )
            .unwrap();
        // The target holds fire (no rounds): only the Panther shoots.
        let v = w.game().unwrap().squads[&t].members[0];
        w.edit_vehicle(v, |vs| {
            vs.ammo.iter_mut().for_each(|n| *n = 0);
            vs.loaded = None;
        });
        w.enable_hit_log();
        for _ in 0..(60 * 30) {
            w.step(&[]);
            if let Some(h) = w.take_hit_log().into_iter().find(|h| h.player == 0) {
                return w.match_setup().unwrap().ammo[h.ammo as usize].rare;
            }
        }
        panic!("the Panther hit nothing at {target}");
    };
    assert!(first_rare(JUMBO), "PzGr 40/42 at the Jumbo");
    assert!(
        !first_rare("ww2:usa_m4a3_76w_squad"),
        "control: PzGr 39/42 at the 76 W"
    );
}
