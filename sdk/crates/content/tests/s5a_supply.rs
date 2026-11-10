//! S5a content: cartridges, supply weights, feeds, member rounds, the rack
//! factor and `rare_cap` (docs/slices/S5a-ammo.md "Data model", targets T6
//! and T17).

use std::path::{Path, PathBuf};

use frontline_content::setup::Feed;
use frontline_content::{ContentError, Difficulty, MatchRequest, MatchSetup, ModDefs, SupplyKind};

fn ww2_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../mods/ww2")
}

fn setup() -> MatchSetup {
    let defs = ModDefs::load(&ww2_root()).unwrap_or_else(|e| panic!("{e}"));
    let req = MatchRequest::skirmish(1, "ww2:germany", "ww2:usa", Difficulty::Normal);
    defs.resolve(&req).unwrap_or_else(|e| panic!("{e}"))
}

/// Copies the live mod into a temp dir and applies `edit(root)`.
fn broken_copy(name: &str, edit: impl FnOnce(&Path)) -> PathBuf {
    let dst = std::env::temp_dir().join(format!("frontline-s5a-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dst);
    copy_dir(&ww2_root(), &dst);
    edit(&dst);
    dst
}

fn copy_dir(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for e in std::fs::read_dir(src).unwrap() {
        let e = e.unwrap();
        let to = dst.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &to);
        } else {
            std::fs::copy(e.path(), to).unwrap();
        }
    }
}

fn replace_in(path: &Path, from: &str, to: &str) {
    let s = std::fs::read_to_string(path).unwrap();
    assert!(s.contains(from), "{} lacks {from:?}", path.display());
    std::fs::write(path, s.replace(from, to)).unwrap();
}

fn issues(e: ContentError) -> String {
    match e {
        ContentError::Invalid { issues, .. } => issues
            .into_iter()
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join("\n"),
        other => panic!("expected validation issues, got {other}"),
    }
}

/// µMP of one round of `f` at the rules' prices.
fn price(s: &MatchSetup, f: Feed) -> u64 {
    let sp = s.rules.supply.as_ref().unwrap();
    let (kind, g) = match f {
        Feed::Cartridge(c) => (s.cartridges[c as usize].kind, s.cartridges[c as usize].g),
        Feed::Ammo(a) => {
            let x = s.ammo[a as usize].supply.unwrap();
            (x.kind, x.g)
        }
    };
    g as u64
        * match kind {
            SupplyKind::SmallArms => sp.small_arms_mp_per_t,
            SupplyKind::Explosives => sp.explosives_mp_per_t,
            SupplyKind::Shells => sp.shell_mp_per_t,
            // S10a: no round is construction material.
            SupplyKind::Construction => unreachable!("a feed is never construction"),
        } as u64
}

/// Manpower (MP) a full small-arms refill of squad type `id` costs.
fn small_arms_refill_mp(s: &MatchSetup, id: &str) -> f64 {
    let ty = &s.squad_types[s.squad_type_index(id).unwrap() as usize];
    let mut micro = 0u64;
    for (k, &f) in ty.feeds.iter().enumerate() {
        let Feed::Cartridge(c) = f else { continue };
        if s.cartridges[c as usize].kind != SupplyKind::SmallArms {
            continue;
        }
        for m in &ty.members {
            micro += m.rounds[k] as u64 * m.count as u64 * price(s, f);
        }
    }
    micro as f64 / 1e6
}

/// T17: the prices read right against hand figures (±10 %).
#[test]
fn refill_prices_match_the_hand_figures() {
    let s = setup();
    let near = |got: f64, want: f64| (got - want).abs() <= want * 0.10;
    // Grenadiergruppe: 58.8 kg of small arms, 1,150 MG rounds of it.
    let g = small_arms_refill_mp(&s, "ww2:ger_grenadiergruppe");
    assert!(near(g, 11.76), "Grenadiergruppe {g:.2} MP");
    // US Rifle Squad: 11 x 136 Garand rounds and 700 BAR rounds, 74.5 kg.
    let u = small_arms_refill_mp(&s, "ww2:usa_rifle_squad");
    assert!(near(u, 14.9), "Rifle Squad {u:.2} MP");
    // Tanks: a full rack within 75 % of the card (D4).
    for (id, card, pct, max) in [
        ("ww2:ger_pz4_h", 620u64, 53u32, 465.0),
        ("ww2:usa_m4a3_75", 590, 51, 443.0),
    ] {
        let v = s.vehicles.iter().find(|v| v.id == id).unwrap();
        assert_eq!(v.rack_price_pct, pct, "{id}");
        let rack: u64 = v
            .main_ammo()
            .iter()
            .map(|a| a.count as u64 * price(&s, Feed::Ammo(a.ammo)) * pct as u64 / 100)
            .sum();
        let mp = rack as f64 / 1e6;
        assert!(
            mp <= max && mp <= card as f64 * 0.75,
            "{id}: full rack {mp:.1} MP"
        );
        assert!(mp >= max * 0.9, "{id}: full rack {mp:.1} MP");
    }
}

/// The shipped loads: stowage restored, rare_cap set, feeds and member
/// rounds as the plan's tables.
#[test]
fn shipped_loads_and_stowage() {
    let s = setup();
    let pz = s.vehicles.iter().find(|v| v.id == "ww2:ger_pz4_h").unwrap();
    let n: Vec<u16> = pz.main_ammo().iter().map(|a| a.count).collect();
    assert_eq!(n, vec![42, 42, 3]);
    assert_eq!(pz.rare_cap, 3);
    assert!(pz.main_ammo()[2].rare);
    let sh = s
        .vehicles
        .iter()
        .find(|v| v.id == "ww2:usa_m4a3_75")
        .unwrap();
    let n: Vec<u16> = sh.main_ammo().iter().map(|a| a.count).collect();
    assert_eq!(n, vec![35, 62]);
    assert_eq!(sh.rare_cap, 0);
    // The Grenadiergruppe's MG team: 50 + 500 + 600 belt rounds.
    let ty = &s.squad_types[s.squad_type_index("ww2:ger_grenadiergruppe").unwrap() as usize];
    let belt = s
        .cartridges
        .iter()
        .position(|c| c.id == "ww2:792x57_belt")
        .unwrap() as u16;
    let k = ty
        .feeds
        .iter()
        .position(|f| *f == Feed::Cartridge(belt))
        .unwrap();
    let team: Vec<u16> = ty.members[1..4].iter().map(|m| m.rounds[k]).collect();
    assert_eq!(team, vec![50, 500, 600]);
    assert!(ty.feeds.len() <= frontline_content::setup::MAX_FEEDS);
    // Mortar squads: 20 bombs (S5b 5-man squads, S5b-5 (A); S5a's 3-man
    // teams carried 15).
    for id in ["ww2:ger_mortar_team", "ww2:usa_mortar_team"] {
        let ty = &s.squad_types[s.squad_type_index(id).unwrap() as usize];
        let bombs: u32 = ty
            .feeds
            .iter()
            .enumerate()
            .filter(|(_, f)| matches!(f, Feed::Cartridge(c) if s.cartridges[*c as usize].kind == SupplyKind::Shells))
            .map(|(k, _)| ty.members.iter().map(|m| m.rounds[k] as u32 * m.count as u32).sum::<u32>())
            .sum();
        assert_eq!(bombs, 20, "{id}");
    }
}

/// T6: a vehicle def above the rules' rare_cap_max fails to load with an
/// actionable error; so do rare rounds above a vehicle's rare_cap.
#[test]
fn rare_cap_above_the_rules_limit_fails_to_load() {
    let root = broken_copy("rare-cap", |r| {
        replace_in(
            &r.join("defs/vehicles/ger_pz4_h.toml"),
            "rare_cap = 3",
            "rare_cap = 12",
        );
    });
    let e = issues(ModDefs::load(&root).unwrap_err());
    assert!(
        e.contains("rare_cap 12 is above the rules' supply.rare_cap_max (10")
            && e.contains("lower rare_cap to at most 10"),
        "{e}"
    );
    let root = broken_copy("rare-over", |r| {
        replace_in(
            &r.join("defs/vehicles/ger_pz4_h.toml"),
            "rare_cap = 3",
            "rare_cap = 2",
        );
    });
    let e = issues(ModDefs::load(&root).unwrap_err());
    assert!(e.contains("3 rare rounds stowed, above rare_cap 2"), "{e}");
}

/// S5a Fix (review L8, M1): an MG mount without `rounds` in a priced mod
/// and an ambush release inside the watch are refused with fixes.
#[test]
fn mg_mount_without_rounds_and_short_ambush_release_are_refused() {
    let root = broken_copy("mg-rounds", |r| {
        replace_in(&r.join("defs/vehicles/ger_pz4_h.toml"), "rounds = 1575", "");
        replace_in(
            &r.join("defs/rules/skirmish.toml"),
            "ambush_release_m = 300",
            "ambush_release_m = 150",
        );
    });
    let e = issues(ModDefs::load(&root).unwrap_err());
    assert!(
        e.contains("vehicle `ww2:ger_pz4_h`: weapons[1].rounds")
            && e.contains("required on coax and hull MG mounts")
            && e.contains("rounds = 3000"),
        "{e}"
    );
    assert!(
        e.contains("ai.assault.ambush_watch_m") && e.contains("must not exceed ambush_release_m"),
        "{e}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// Feeds and supply tables are checked with fixes.
#[test]
fn missing_feeds_and_supply_tables_are_refused() {
    let root = broken_copy("no-feed", |r| {
        replace_in(
            &r.join("defs/weapons/kar98k.toml"),
            "feed = \"ww2:792x57_clip\"",
            "",
        );
        replace_in(
            &r.join("defs/items/stielhandgranate.toml"),
            "supply = { kind = \"explosives\", g = 595 }",
            "",
        );
        replace_in(
            &r.join("defs/cartridges/9x19.toml"),
            "kind = \"small_arms\"",
            "kind = \"construction\"",
        );
        replace_in(
            &r.join("defs/weapons/mg34.toml"),
            "feed = \"ww2:792x57_belt\"",
            "feed = \"ww2:75mm_pzgr40\"",
        );
    });
    let e = issues(ModDefs::load(&root).unwrap_err());
    assert!(
        e.contains("ww2:kar98k fires counted rounds since S5a"),
        "{e}"
    );
    assert!(e.contains("item `ww2:stielhandgranate`: supply"), "{e}");
    assert!(e.contains("not before S6"), "{e}");
    assert!(e.contains("rare round"), "{e}");
    // A slot setting both the S4 `ammo` and `rounds`.
    let root = broken_copy("both", |r| {
        replace_in(
            &r.join("defs/units/ger_grenadiergruppe.toml"),
            "rounds = { \"ww2:792x57_belt\" = 600 }",
            "rounds = { \"ww2:792x57_belt\" = 600 }\nammo = 60",
        );
    });
    let e = issues(ModDefs::load(&root).unwrap_err());
    assert!(e.contains("not both"), "{e}");
}

/// A mod without `[supply]` (the frozen S3c content) has no feeds: rounds
/// stay unlimited there (S5a-22).
#[test]
fn legacy_content_has_no_feeds() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ww2_s3c");
    let defs = ModDefs::load(&root).unwrap_or_else(|e| panic!("{e}"));
    let s = defs
        .resolve(&MatchRequest::skirmish(
            1,
            "ww2:germany",
            "ww2:usa",
            Difficulty::Normal,
        ))
        .unwrap();
    assert!(s.rules.supply.is_none());
    assert!(s.squad_types.iter().all(|t| t.feeds.is_empty()));
    assert!(s.weapons.iter().all(|w| w.feed.is_none()));
}
