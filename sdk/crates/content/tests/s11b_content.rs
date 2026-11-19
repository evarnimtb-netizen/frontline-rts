//! S11b Build A (docs/slices/S11b.md 2, 4 G1): the new `[ai.arty]` and
//! `[items]` keys parse from the ww2 rules with every switch 0, the
//! `s11b_off` control file applies, every key is range-checked (overrides
//! and untrusted saves), and the rifle-grenade item def needs an HE
//! launcher.

use std::path::{Path, PathBuf};

use frontline_content::tuning::AiArtyTuning;
use frontline_content::{Difficulty, ItemKind, MatchRequest, ModDefs};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn write(name: &str, text: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "frontline-s11b-{name}-{}.toml",
        std::process::id()
    ));
    std::fs::write(&p, text).unwrap();
    p
}

/// One out-of-range case per new key (spec 2 ranges). Control: widen one
/// range (seen with `how_max: 0..=9`) and that case is accepted, so the
/// test fails.
#[test]
fn load_rejects_bad_s11b_keys() {
    let mut defs = ModDefs::load(&root().join("mods/ww2")).unwrap_or_else(|e| panic!("{e}"));
    let req = MatchRequest::skirmish(1, "ww2:germany", "ww2:usa", Difficulty::Normal);
    let s = defs.resolve(&req).unwrap();
    let a = &s.rules.ai.arty;
    // Build A: every switch 0, the other keys at the spec values.
    assert_eq!(
        (a.cb_alternate, a.buy_pending, a.how, a.light),
        (0, 0, 0, 0)
    );
    assert_eq!(
        (a.how_after_s, a.how_max, a.how_income_pct, a.how_reserve_mp),
        (480, 1, 15, 300)
    );
    assert_eq!(
        (s.rules.items.rifle_grenades, s.rules.items.rifle_grenade_gap_ticks),
        (0, 15 * 30)
    );
    // Absent keys read the same values.
    let d = AiArtyTuning::default();
    assert_eq!(
        (d.cb_alternate, d.buy_pending, d.how, d.light),
        (0, 0, 0, 0)
    );
    assert_eq!(
        (d.how_after_s, d.how_max, d.how_income_pct, d.how_reserve_mp),
        (480, 1, 15, 300)
    );
    // The control file applies (its keys exist and are in range).
    let off = root().join("docs/slices/targets/S11b-ablations/s11b_off.toml");
    defs.apply_override(&off, "ww2:skirmish")
        .unwrap_or_else(|e| panic!("{e}"));
    // An override reaches the setup.
    let on = write(
        "on",
        "[ai.arty]\ncb_alternate = 1\nhow_reserve_mp = 100000\n[items]\nrifle_grenades = 1\nrifle_grenade_gap_s = 600\n",
    );
    defs.apply_override(&on, "ww2:skirmish")
        .unwrap_or_else(|e| panic!("{e}"));
    let s = defs.resolve(&req).unwrap();
    assert_eq!(
        (s.rules.ai.arty.cb_alternate, s.rules.ai.arty.how_reserve_mp),
        (1, 100_000)
    );
    assert_eq!(
        (s.rules.items.rifle_grenades, s.rules.items.rifle_grenade_gap_ticks),
        (1, 600 * 30)
    );
    // Out of range: one case per key; the issue names the key.
    let cases: Vec<(&str, &str, u32)> = vec![
        ("ai.arty", "cb_alternate", 2),
        ("ai.arty", "buy_pending", 2),
        ("ai.arty", "how", 2),
        ("ai.arty", "how_after_s", 3601),
        ("ai.arty", "how_max", 9),
        ("ai.arty", "how_income_pct", 101),
        ("ai.arty", "how_reserve_mp", 100_001),
        ("ai.arty", "light", 2),
        ("items", "rifle_grenades", 2),
        ("items", "rifle_grenade_gap_s", 601),
    ];
    for (table, key, bad) in cases {
        let f = write(key, &format!("[{table}]\n{key} = {bad}\n"));
        let e = defs
            .apply_override(&f, "ww2:skirmish")
            .err()
            .unwrap_or_else(|| panic!("{table}.{key} = {bad} was accepted"))
            .to_string();
        assert!(e.contains(&format!("{table}.{key}")), "{e}");
    }
    // The setup is checked too (saves are untrusted).
    let mut s = defs.resolve(&req).unwrap();
    assert!(s.validate().is_empty());
    s.rules.ai.arty.how_max = 9;
    assert!(!s.validate().is_empty());
    let mut s = defs.resolve(&req).unwrap();
    s.rules.items.rifle_grenades = 2;
    assert!(!s.validate().is_empty());
    let mut s = defs.resolve(&req).unwrap();
    s.rules.items.rifle_grenade_gap_ticks = 600 * 30 + 1;
    assert!(!s.validate().is_empty());
}

/// G1: a `rifle_grenade` item names a launcher whose round is HE (with an
/// `[he]` table) and is refilled as shells; the item itself is never
/// thrown. Control: the ww2 def (M7, M17 HE) passes; pointed at the
/// Panzerfaust (a HEAT round) it is refused, and with no `weapon` too.
#[test]
fn rifle_grenade_item_needs_an_he_launcher() {
    const IT: &str = "ww2:m7_rifle_grenade";
    let defs = ModDefs::load(&root().join("mods/ww2")).unwrap_or_else(|e| panic!("{e}"));
    let it = &defs.items[IT];
    assert_eq!(it.kind, ItemKind::RifleGrenade);
    assert!(!ItemKind::RifleGrenade.thrown());
    assert_eq!(
        (it.range_m, it.min_range_m, it.windup_ms, it.stow_ms),
        (180, 30, 3000, 3000)
    );
    let mine = |d: &ModDefs| {
        d.cross_check()
            .into_iter()
            .filter(|i| i.path.contains("m7_rifle_grenade"))
            .count()
    };
    assert_eq!(mine(&defs), 0, "control: the M7 with the M17 HE round");
    // A HEAT launcher is refused.
    let mut heat = defs.clone();
    heat.items.get_mut(IT).unwrap().weapon = Some("ww2:pzf60".into());
    assert_eq!(mine(&heat), 1, "a HEAT launcher");
    // A round refilled as anything but shells is refused.
    let mut kind = defs.clone();
    kind.items.get_mut(IT).unwrap().supply.as_mut().unwrap().kind = "explosives".into();
    assert_eq!(mine(&kind), 1, "explosives");
    // No weapon at all: the def check refuses it.
    let mut none = defs.items[IT].clone();
    none.weapon = None;
    assert!(none.validate().iter().any(|i| i.path == "weapon"));
    assert!(defs.items[IT].validate().is_empty());
    // A range beyond 300 m or a stow time on another kind are refused.
    let mut far = defs.items[IT].clone();
    far.range_m = 301;
    assert!(far.validate().iter().any(|i| i.path == "range_m"));
    let mut frag = defs.items["ww2:mk2_grenade"].clone();
    frag.stow_ms = 1000;
    assert!(frag.validate().iter().any(|i| i.path == "stow_ms"));
}

/// G1 issue rule: with `[items] rifle_grenades` 0 the setup build drops the
/// `rifle_grenade` kind from every squad type (no item index, member
/// counts 0). Control 1: the slot's kit holds 3. (No card carries the
/// grenade before Build B: the test gives the US riflemen three.)
#[test]
fn switch_off_issues_no_rifle_grenades() {
    let mut defs = ModDefs::load(&root().join("mods/ww2")).unwrap_or_else(|e| panic!("{e}"));
    let sq = defs.squads.get_mut("ww2:usa_rifle_squad").unwrap();
    let slot = sq
        .members
        .iter_mut()
        .find(|m| m.slot == "rifleman")
        .expect("a rifleman slot");
    slot.items.insert("ww2:m7_rifle_grenade".into(), 3);
    let req = MatchRequest::skirmish(1, "ww2:germany", "ww2:usa", Difficulty::Normal);
    let kit = |rg: u32| {
        let mut d = defs.clone();
        d.rules.get_mut("ww2:skirmish").unwrap().items.rifle_grenades = rg;
        let s = d.resolve(&req).unwrap_or_else(|e| panic!("{e}"));
        let t = &s.squad_types[s.squad_type_index("ww2:usa_rifle_squad").unwrap() as usize];
        let k = t.members.iter().position(|m| m.slot == "rifleman").unwrap() as u8;
        (
            t.items[ItemKind::RifleGrenade.index()].is_some(),
            t.full_kit(k)[ItemKind::RifleGrenade.index()],
            t.squad_items(ItemKind::RifleGrenade),
        )
    };
    assert_eq!(kit(0), (false, 0, 0), "switch 0: no grenade issued");
    assert_eq!(kit(1), (true, 3, 15), "control: five riflemen, three each");
}
