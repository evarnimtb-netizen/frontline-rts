//! S13a Build A tests (docs/slices/S13-heavy-wave.md 3, 5): the SAVE 31
//! layout and its load checks, rule O1 and `special_min_pct`. Each test
//! names its control: the edit that makes it fail.

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
    assert_eq!(on, &off);
    assert!(on.ammo.iter().all(|t| t.o1.is_none()));
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
