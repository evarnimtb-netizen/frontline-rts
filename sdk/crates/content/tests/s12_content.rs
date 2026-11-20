//! S12 (docs/slices/S12-wounds-medics.md 2): the `[wounds]`, `[ai.medic]`
//! and `[supply] aid_max` keys parse from the ww2 rules with every switch 0
//! (S12a), the ablation files apply, every key is range-checked (overrides
//! and untrusted saves).

use std::path::{Path, PathBuf};

use frontline_content::tuning::{AiMedicTuning, WoundsTuning};
use frontline_content::{Difficulty, MatchRequest, ModDefs};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn write(name: &str, text: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("frontline-s12-{name}-{}.toml", std::process::id()));
    std::fs::write(&p, text).unwrap();
    p
}

/// One out-of-range case per key bound (spec 2 ranges: switches 0..=1,
/// seconds 1..=3600, percents 1..=400). Control: widen one range (seen with
/// `bandage_s: 1..=3601`) and that case is accepted, so the test fails.
#[test]
fn load_rejects_bad_s12_keys() {
    let defs0 = ModDefs::load(&root().join("mods/ww2")).unwrap_or_else(|e| panic!("{e}"));
    let mut defs = defs0.clone();
    let req = MatchRequest::skirmish(1, "ww2:germany", "ww2:usa", Difficulty::Normal);
    let s = defs.resolve(&req).unwrap();
    // The ww2 rules state the defaults, every switch 0.
    assert_eq!(s.rules.wounds, WoundsTuning::default());
    assert_eq!(s.rules.ai.medic, AiMedicTuning::default());
    assert_eq!(s.rules.supply.as_ref().unwrap().aid_max, 2);
    let w = &s.rules.wounds;
    assert_eq!(
        (w.enabled, w.reinforce, w.head_damage_pct, w.legs_damage_pct),
        (0, 0, 100, 60)
    );
    assert_eq!(
        (
            w.down_bleed_min_s,
            w.down_bleed_max_s,
            w.wounded_hp,
            w.overkill_kill
        ),
        (60, 180, 50, 60)
    );
    let m = &s.rules.ai.medic;
    assert_eq!((m.buy, m.field, m.post, m.reinforce), (0, 0, 0, 0));
    assert_eq!(WoundsTuning::KEYS.len(), 27);
    assert_eq!(AiMedicTuning::KEYS.len(), 9);
    // The ablation files apply (their keys exist and are in range).
    for f in [
        "s12_off",
        "wounds_only",
        "head_300",
        "no_medics",
        "no_post",
        "no_reinforce",
    ] {
        let mut d = defs0.clone();
        let p = root().join(format!("docs/slices/targets/S12-ablations/{f}.toml"));
        d.apply_override(&p, "ww2:skirmish")
            .unwrap_or_else(|e| panic!("{f}: {e}"));
        let s = d.resolve(&req).unwrap();
        assert_eq!(s.rules.wounds.enabled, (f != "s12_off") as u32, "{f}");
    }
    // An override reaches the setup.
    let on = write(
        "on",
        "[wounds]\nenabled = 1\nbandage_s = 3600\n[ai.medic]\nfield = 1\n[supply]\naid_max = 8\n",
    );
    defs.apply_override(&on, "ww2:skirmish")
        .unwrap_or_else(|e| panic!("{e}"));
    let s = defs.resolve(&req).unwrap();
    assert_eq!(
        (s.rules.wounds.enabled, s.rules.wounds.bandage_s),
        (1, 3600)
    );
    assert_eq!(s.rules.ai.medic.field, 1);
    assert_eq!(s.rules.supply.as_ref().unwrap().aid_max, 8);
    // Out of range: one case per bound; the issue names the key.
    let mut cases: Vec<(&str, &str, u32)> = Vec::new();
    for &(key, lo, hi) in WoundsTuning::KEYS {
        if lo > 0 {
            cases.push(("wounds", key, lo - 1));
        }
        cases.push(("wounds", key, hi + 1));
    }
    for &(key, lo, hi) in AiMedicTuning::KEYS {
        if lo > 0 {
            cases.push(("ai.medic", key, lo - 1));
        }
        cases.push(("ai.medic", key, hi + 1));
    }
    cases.push(("supply", "aid_max", 9));
    for (table, key, bad) in cases {
        let f = write(key, &format!("[{table}]\n{key} = {bad}\n"));
        let e = defs
            .apply_override(&f, "ww2:skirmish")
            .err()
            .unwrap_or_else(|| panic!("{table}.{key} = {bad} was accepted"))
            .to_string();
        assert!(e.contains(&format!("{table}.{key}")), "{e}");
    }
    // The spec's ranges, spot-checked against the key list.
    let range = |k: &str| {
        WoundsTuning::KEYS
            .iter()
            .find(|e| e.0 == k)
            .map(|e| (e.1, e.2))
            .unwrap()
    };
    assert_eq!(range("enabled"), (0, 1));
    assert_eq!(range("bandage_s"), (1, 3600));
    assert_eq!(range("head_damage_pct"), (1, 400));
    // The bleed-out window keeps its order, the head and leg bands do not
    // overlap.
    for text in [
        "[wounds]\ndown_bleed_min_s = 200\ndown_bleed_max_s = 100\n",
        "[wounds]\nhead_top_pct = 60\nlegs_bottom_pct = 50\n",
        // Checker G7: a refund above the replacement price is a gain loop.
        "[wounds]\nevac_refund_pct = 120\nreinforce_cost_pct = 100\n",
    ] {
        let f = write("order", text);
        assert!(defs.apply_override(&f, "ww2:skirmish").is_err(), "{text}");
    }
    // The setup is checked too (saves are untrusted).
    let mut s = defs.resolve(&req).unwrap();
    assert!(s.validate().is_empty());
    s.rules.wounds.bleed_ms_per_hp = 0;
    assert!(!s.validate().is_empty());
    let mut s = defs.resolve(&req).unwrap();
    s.rules.wounds.evac_refund_pct = s.rules.wounds.reinforce_cost_pct + 1;
    assert!(
        !s.validate().is_empty(),
        "checker G7: refund over the price"
    );
    s.rules.wounds.evac_refund_pct = s.rules.wounds.reinforce_cost_pct;
    assert!(
        s.validate().is_empty(),
        "checker G7: refund equal to the price"
    );
    let mut s = defs.resolve(&req).unwrap();
    s.rules.ai.medic.post = 2;
    assert!(!s.validate().is_empty());
    let mut s = defs.resolve(&req).unwrap();
    s.rules.supply.as_mut().unwrap().aid_max = 9;
    assert!(!s.validate().is_empty());
}

/// Build B step 1: the medic cards resolve with the `medic` trait on every
/// man, `ai_weight` 0, the plan's prices and cooldowns, appended last in
/// each roster; the trait name is known to content validation and an
/// unknown bit is refused. Control: drop `medic` from `member_trait::ALL`
/// and the setup check fails.
#[test]
fn s12_medic_cards_resolve() {
    use frontline_content::setup::member_trait;
    let defs = ModDefs::load(&root().join("mods/ww2")).unwrap_or_else(|e| panic!("{e}"));
    let req = MatchRequest::skirmish(1, "ww2:germany", "ww2:usa", Difficulty::Normal);
    let s = defs.resolve(&req).unwrap();
    assert!(s.validate().is_empty());
    assert!(member_trait::NAMES.contains(&("medic", member_trait::MEDIC)));
    assert_eq!(member_trait::MEDIC, 64);
    assert!(frontline_content::variant::TRAITS.contains(&"medic"));
    for (id, cost, w, armed) in [
        ("ww2:ger_sanitaetstrupp", 66, "ww2:p38", true),
        ("ww2:usa_aid_men", 80, "ww2:m1911a1", false),
    ] {
        let k = s.squad_type_index(id).unwrap_or_else(|| panic!("{id}"));
        let t = &s.squad_types[k as usize];
        assert_eq!((t.men, t.cost, t.ai_weight), (2, cost, 0), "{id}");
        assert_eq!(t.recall_cooldown_ticks, 30 * s.tick_hz, "{id}");
        assert!(t.tags.contains(&s.tag("medic").unwrap()), "{id}");
        for m in &t.members {
            assert!(m.traits & member_trait::MEDIC != 0, "{id}");
            assert_eq!(s.weapons[m.weapon as usize].id, w, "{id}");
            assert_eq!(m.rounds.iter().any(|&r| r > 0), armed, "{id}");
            // Checker G8: the card counts rounds (the P 38's `load`, 16,
            // with no `rounds` key), so its pistols run dry.
            if armed {
                assert_eq!(m.rounds.iter().sum::<u16>(), 16, "{id}");
            }
        }
        assert!(!t.feeds.is_empty(), "{id}: counted");
        let f = &s.factions[t.faction as usize];
        assert_eq!(*f.roster.last().unwrap(), k, "{id} is appended last");
    }
    let mut bad = s.clone();
    let k = bad.squad_type_index("ww2:usa_aid_men").unwrap() as usize;
    bad.squad_types[k].members[0].traits |= 128;
    assert!(!bad.validate().is_empty());
}

/// Build B step 4 (checker F8): the aid post defs load (StockKind::Aid is
/// accepted now) with the plan's values: 40 MP, aid_pit, radius 12, 2 beds,
/// staff 2, built by the medic tag, one tent box of 60 hp, no stock.
/// Control: a def with 0 beds, or beds on an HQ, is refused.
#[test]
fn s12_aid_post_defs_resolve() {
    use frontline_content::setup::{DigProfile, StockKind};
    let defs = ModDefs::load(&root().join("mods/ww2")).unwrap_or_else(|e| panic!("{e}"));
    let req = MatchRequest::skirmish(1, "ww2:germany", "ww2:usa", Difficulty::Normal);
    let s = defs.resolve(&req).unwrap();
    assert!(s.validate().is_empty());
    for f in 0..s.factions.len() as u16 {
        let k = s
            .stock_of(f, StockKind::Aid, None)
            .expect("an aid def per faction");
        let d = &s.stocks[k as usize];
        assert_eq!(
            (d.cost, d.pit, d.beds, d.staff_min),
            (40, DigProfile::AidPit, 2, 2)
        );
        assert_eq!(d.radius, frontline_content::Fixed::from_int(12));
        assert_eq!(d.cap_g, [0; 3]);
        assert_eq!(d.built_by, vec![s.tag("medic").unwrap()]);
        assert_eq!(d.boxes.len(), 1);
        assert_eq!((d.boxes[0].part, d.boxes[0].hp), (3, 60));
        assert!(d.cookoff.is_none());
    }
    assert!(s.rules.aid_pit.depth > frontline_content::Fixed::ZERO);
    let k = s.stock_of(0, StockKind::Aid, None).unwrap() as usize;
    let mut bad = s.clone();
    bad.stocks[k].beds = 0;
    assert!(!bad.validate().is_empty(), "0 beds");
    let mut bad = s.clone();
    let h = s.stock_of(0, StockKind::Hq, None).unwrap() as usize;
    bad.stocks[h].beds = 2;
    assert!(!bad.validate().is_empty(), "beds on an HQ");
    let mut d = defs
        .supply
        .values()
        .find(|d| d.kind == StockKind::Aid)
        .unwrap()
        .clone();
    assert!(d.validate().is_empty());
    d.beds = 9;
    assert!(!d.validate().is_empty(), "def beds 9");
}
