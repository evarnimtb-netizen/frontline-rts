//! The base `ww2` mod loads, validates and resolves into a match setup; broken
//! copies of it report every problem with a file-qualified path.

use std::path::{Path, PathBuf};

use frontline_content::setup::Fixed;
use frontline_content::{
    ContentError, Difficulty, MatchRequest, ModDefs, PlayerRequest, SquadRole, WeaponClass,
};

fn ww2_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../mods/ww2")
}

fn load() -> ModDefs {
    ModDefs::load(&ww2_root()).unwrap_or_else(|e| panic!("{e}"))
}

/// Copies the ww2 mod into a fresh temp dir and applies `edit(root)`.
fn broken_copy(name: &str, edit: impl FnOnce(&Path)) -> PathBuf {
    let dst = std::env::temp_dir().join(format!("frontline-content-{name}-{}", std::process::id()));
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

fn issue_paths(e: ContentError) -> Vec<String> {
    match e {
        ContentError::Invalid { issues, .. } => issues.into_iter().map(|i| i.path).collect(),
        other => panic!("expected validation issues, got {other}"),
    }
}

#[test]
fn ww2_mod_loads_clean() {
    let d = load();
    assert_eq!(d.manifest.as_ref().unwrap().id, "ww2");
    // S1 squads, plus (S3) two AT teams and two vehicle squads.
    assert_eq!(d.squads.len(), 12);
    // S1 weapons, plus (S3) two cannons, the MG 34 and two launchers.
    assert_eq!(d.weapons.len(), 13);
    assert_eq!(d.items.len(), 6);
    assert_eq!(
        d.vehicles.keys().collect::<Vec<_>>(),
        ["ww2:ger_pz4_h", "ww2:usa_m4a3_75"]
    );
    // S3: six rounds; S3b: the PzGr 40 APCR.
    assert_eq!(d.ammo.len(), 7);
    assert_eq!(
        d.factions.keys().collect::<Vec<_>>(),
        ["ww2:germany", "ww2:usa"]
    );
    assert!(d.maps.contains_key("ww2:kessel_ridge"));
    assert!(d.rules.contains_key("ww2:skirmish"));
    let map = &d.maps["ww2:kessel_ridge"];
    assert_eq!(map.size_m, 1024);
    assert_eq!(map.capture_points.len(), 5);
    assert_eq!(map.bases.len(), 2);
    // Every faction fields one squad of each role (S3: its AT team and its
    // tank too).
    for f in d.factions.values() {
        let mut roles: Vec<SquadRole> = f.roster.iter().map(|s| d.squads[s].role).collect();
        roles.sort();
        let mut want = vec![
            SquadRole::Rifle,
            SquadRole::Mg,
            SquadRole::Engineer,
            SquadRole::Mortar,
            SquadRole::AntiTank,
            SquadRole::Vehicle,
        ];
        want.sort();
        assert_eq!(roles, want);
    }
}

#[test]
fn skirmish_resolves_to_fixed_point() {
    let d = load();
    let req = MatchRequest::skirmish(7, "ww2:germany", "ww2:usa", Difficulty::Hard);
    let s = d.resolve(&req).unwrap();
    assert!(s.validate().is_empty());
    assert_eq!((s.seed, s.tick_hz, s.map.size_m), (7, 30, 1024));
    assert_eq!(s.players.len(), 2);
    assert_eq!(s.factions[s.players[0].faction as usize].id, "ww2:germany");
    assert_eq!(s.factions[s.players[1].faction as usize].id, "ww2:usa");
    assert!(s.players[1].is_ai && !s.players[0].is_ai);
    assert_eq!(s.players[1].base, 1);
    assert_eq!(s.rules.capture_ticks, 20 * 30);
    assert_eq!(s.rules.capture_radius, Fixed::from_int(15));
    assert_eq!(s.rules.difficulty.get(Difficulty::Hard).reaction_ticks, 30);
    let c = &s.rules.combat;
    assert_eq!(c.spot_period_ticks, 6);
    assert_eq!(c.reveal_ticks, 60);
    assert_eq!(c.near_miss, Fixed::from_int(2));
    assert_eq!(c.corpse_ticks, 30 * 30);
    assert_eq!(c.suppression_decay_milli, 8 * 1000 / 30);
    assert_eq!(s.rules.ticket_drain_per_min_per_point, 20);
    let ai = &s.rules.ai;
    assert_eq!(ai.retreat_strength_pct, 40);
    assert_eq!(ai.threat_radius, Fixed::from_int(80));
    assert_eq!(ai.mix_pct, [50, 20, 15, 15]);
    assert_eq!(ai.dig_length, Fixed::from_int(20));
    assert!(ai.dig_length <= s.rules.dig.max_length);

    let rifle = &s.squad_types[s.squad_type_index("ww2:ger_rifle_squad").unwrap() as usize];
    assert_eq!(rifle.men, 8);
    assert_eq!(rifle.speed, Fixed(3 << 16));
    assert_eq!(rifle.spacing, Fixed(3 << 16));
    assert_eq!(rifle.recall_cooldown_ticks, 24 * 30);
    assert_eq!(s.weapons[rifle.weapon as usize].id, "ww2:kar98k");
    // 15 rpm at 30 Hz: 120 ticks between rounds.
    assert_eq!(s.weapons[rifle.weapon as usize].interval_milli, 120_000);
    let mg42 = s.weapons.iter().find(|w| w.id == "ww2:mg42").unwrap();
    // 1200 rpm is 1.5 ticks at 30 Hz, kept exact in milli-ticks.
    assert_eq!(mg42.interval_milli, 1500);
    assert_eq!(mg42.setup_ticks, 90);
    let mortar = s.weapons.iter().find(|w| w.id == "ww2:grw34").unwrap();
    assert_eq!(mortar.class, WeaponClass::Mortar);
    let mhe = mortar.he.as_ref().unwrap();
    assert_eq!(mhe.blast_radius, Fixed::from_ratio(3, 2));
    // S3b Tune: 96 -> 120 fragments (owner question S3b-23 (A)).
    assert_eq!((mhe.frag_count, mhe.frag_reach), (120, Fixed::from_int(15)));
    // Ridge plus valley.
    assert_eq!(s.map.terrain.bands.len(), 2);
    assert!(s.map.terrain.bands[1].height < Fixed::ZERO);
    // Roster indices point at the faction's own squads.
    for (fi, f) in s.factions.iter().enumerate() {
        for &t in f.roster.iter().chain(&f.start_squads) {
            assert_eq!(s.squad_types[t as usize].faction as usize, fi);
        }
    }
    // Same defs and request, same setup.
    assert_eq!(load().resolve(&req).unwrap(), s);
}

#[test]
fn bad_requests_list_every_problem() {
    let d = load();
    let req = MatchRequest {
        rules: "ww2:nope".into(),
        map: "ww2:kessel_ridge".into(),
        seed: 1,
        players: vec![PlayerRequest {
            faction: "ww2:france".into(),
            ai: false,
            difficulty: Difficulty::Normal,
        }],
    };
    let paths = issue_paths(d.resolve(&req).unwrap_err());
    assert_eq!(paths, ["rules", "players", "players[0].faction"]);
}

#[test]
fn broken_mod_reports_file_qualified_issues() {
    let root = broken_copy("refs", |r| {
        replace_in(
            &r.join("defs/units/ger_rifle_squad.toml"),
            "weapon = \"ww2:kar98k\"",
            "weapon = \"ww2:kar99\"",
        );
        replace_in(&r.join("defs/units/usa_mg_team.toml"), "men = 3", "men = 0");
        replace_in(
            &r.join("defs/weapons/mp40.toml"),
            "id = \"ww2:mp40\"",
            "id = \"other:mp40\"",
        );
        replace_in(
            &r.join("defs/maps/kessel_ridge.toml"),
            "size_m = 1024",
            "size_m = 1000",
        );
        replace_in(
            &r.join("defs/rules/skirmish.toml"),
            "dig_length_m = 20",
            "dig_length_m = 90",
        );
    });
    let paths = issue_paths(ModDefs::load(&root).unwrap_err());
    for want in [
        "defs/units/usa_mg_team.toml: men",
        "defs/weapons/mp40.toml: id",
        "defs/maps/kessel_ridge.toml: size_m",
        "defs/rules/skirmish.toml: ai.dig_length_m",
        "squad `ww2:ger_rifle_squad`: weapon",
        // The engineer squad still names the renamed SMG.
        "squad `ww2:ger_engineer_squad`: weapon",
    ] {
        assert!(
            paths.iter().any(|p| p == want),
            "missing {want:?} in {paths:?}"
        );
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn unknown_keys_are_reported_with_location() {
    let root = broken_copy("keys", |r| {
        replace_in(
            &r.join("defs/rules/skirmish.toml"),
            "pop_cap = 120",
            "pop_cap = 120\npopcap = 1",
        );
    });
    let e = ModDefs::load(&root).unwrap_err().to_string();
    assert!(
        e.contains("rules/skirmish.toml") && e.contains("popcap"),
        "{e}"
    );
    assert!(e.contains("line"), "{e}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn items_and_loadouts_resolve() {
    use frontline_content::ItemKind;
    let d = load();
    assert_eq!(d.items.len(), 6);
    let s = d
        .resolve(&MatchRequest::skirmish(
            1,
            "ww2:germany",
            "ww2:usa",
            Difficulty::Normal,
        ))
        .unwrap();
    assert_eq!(s.rules.items.resupply_ticks, 20 * 30);
    assert_eq!(s.rules.ai.throw_cooldown_ticks, 10 * 30);
    let frag = s
        .items
        .iter()
        .find(|i| i.id == "ww2:stielhandgranate")
        .unwrap();
    assert_eq!(frag.kind, ItemKind::Frag);
    assert_eq!(frag.range_max, Fixed::from_int(30));
    assert_eq!((frag.windup_ticks, frag.fuse_ticks), (30, 120));
    let he = frag.he.as_ref().unwrap();
    assert_eq!(he.blast.blast_radius, Fixed::from_int(2));
    assert_eq!(he.blast.frag_count, 96);
    assert!(!he.cuts_trenches && frag.smoke.is_none());
    let satchel = s
        .items
        .iter()
        .find(|i| i.kind == ItemKind::Satchel)
        .unwrap();
    assert!(satchel.he.as_ref().unwrap().cuts_trenches);
    let smoke = s.items.iter().find(|i| i.kind == ItemKind::Smoke).unwrap();
    assert_eq!(smoke.smoke.as_ref().unwrap().life_ticks(), 33 * 30);
    let ty = |id: &str| &s.squad_types[s.squad_type_index(id).unwrap() as usize];
    // Rifles: a frag each, two and a smoke for the leader; engineers add
    // the satchel; MG and mortar teams carry nothing.
    assert_eq!(
        ty("ww2:ger_rifle_squad").full_kit(&s.items, false),
        [1, 0, 0]
    );
    assert_eq!(
        ty("ww2:ger_rifle_squad").full_kit(&s.items, true),
        [2, 1, 0]
    );
    assert_eq!(
        ty("ww2:usa_engineer_squad").full_kit(&s.items, true),
        [2, 1, 1]
    );
    assert_eq!(ty("ww2:usa_mg_team").full_kit(&s.items, true), [0, 0, 0]);
    assert_eq!(
        ty("ww2:ger_mortar_team").full_kit(&s.items, true),
        [0, 0, 0]
    );
    let (_, it) = ty("ww2:usa_rifle_squad")
        .kit(&s.items, ItemKind::Frag)
        .unwrap();
    assert_eq!(it.id, "ww2:mk2_grenade");
}

#[test]
fn broken_items_and_loadouts_are_reported() {
    // Smoke without its cloud, a second frag in a loadout, an unknown item,
    // and a minimum range beyond the range.
    let root = broken_copy("items", |r| {
        replace_in(
            &r.join("defs/units/usa_rifle_squad.toml"),
            "item = \"ww2:m15_smoke\"",
            "item = \"ww2:stielhandgranate\"",
        );
        replace_in(
            &r.join("defs/units/usa_engineer_squad.toml"),
            "item = \"ww2:m37_satchel\"",
            "item = \"ww2:tnt\"",
        );
        replace_in(
            &r.join("defs/items/mk2_grenade.toml"),
            "min_range_m = 5",
            "min_range_m = 50",
        );
        replace_in(&r.join("defs/items/nbhgr39.toml"), "[smoke]\n", "");
        replace_in(&r.join("defs/items/nbhgr39.toml"), "radius_m = 8\n", "");
        replace_in(&r.join("defs/items/nbhgr39.toml"), "grow_ms = 3000\n", "");
        replace_in(&r.join("defs/items/nbhgr39.toml"), "hold_ms = 25000\n", "");
        replace_in(&r.join("defs/items/nbhgr39.toml"), "fade_ms = 5000\n", "");
    });
    let paths = issue_paths(ModDefs::load(&root).unwrap_err());
    for want in [
        "defs/items/nbhgr39.toml: smoke",
        "defs/items/mk2_grenade.toml: min_range_m",
        "squad `ww2:usa_rifle_squad`: loadout[1].item",
        "squad `ww2:usa_engineer_squad`: loadout[2].item",
    ] {
        assert!(
            paths.iter().any(|p| p == want),
            "missing {want:?} in {paths:?}"
        );
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn content_hash_is_stable_and_sees_def_changes() {
    let a = load();
    // Loading twice gives the same hash (no listing-order or address
    // dependence), and a clone hashes the same.
    assert_eq!(a.content_hash(), load().content_hash());
    assert_eq!(a.content_hash(), a.clone().content_hash());
    // One changed def value changes it.
    let dir = broken_copy("content_hash", |root| {
        replace_in(
            &root.join("defs/rules/skirmish.toml"),
            "tickets = 500",
            "tickets = 501",
        );
    });
    let b = ModDefs::load(&dir).expect("edited copy still loads");
    assert_ne!(a.content_hash(), b.content_hash());
}

#[test]
fn call_in_cooldowns_are_the_owner_defaults() {
    // S3b, owner 2026-11-01: no queue, no delivery time; re-call cooldowns
    // 20 % under the first proposal. No start cooldowns, no groups.
    let req = MatchRequest::skirmish(1, "ww2:germany", "ww2:usa", Difficulty::Normal);
    let s = load().resolve(&req).unwrap();
    let want = [
        ("ww2:ger_rifle_squad", 24),
        ("ww2:usa_rifle_squad", 24),
        ("ww2:ger_engineer_squad", 36),
        ("ww2:usa_engineer_squad", 36),
        ("ww2:ger_mg_team", 36),
        ("ww2:usa_mg_team", 36),
        ("ww2:ger_panzerschreck_team", 48),
        ("ww2:usa_bazooka_team", 48),
        ("ww2:ger_mortar_team", 60),
        ("ww2:usa_mortar_team", 60),
        ("ww2:ger_pz4_h_squad", 120),
        ("ww2:usa_m4a3_75_squad", 120),
    ];
    assert_eq!(s.squad_types.len(), want.len());
    for (id, secs) in want {
        let t = &s.squad_types[s.squad_type_index(id).unwrap() as usize];
        assert_eq!(t.recall_cooldown_ticks, secs * 30, "{id}");
        assert_eq!(t.initial_cooldown_ticks, 0, "{id}");
        assert_eq!(t.cooldown_group, None, "{id}");
    }
    let c = &s.rules.call_in;
    assert_eq!((c.spawn_ring_pct, c.spawn_clear_m), (50, 10));
    assert_eq!(s.rules.ai.armour.tank_reserve_mp, 200);
}

#[test]
fn cooldown_groups_share_an_index_and_bad_values_are_refused() {
    let root = ww2_root();
    let mut defs = load();
    let path =
        std::env::temp_dir().join(format!("frontline-s3b-group-{}.toml", std::process::id()));
    std::fs::write(
        &path,
        "[defs.units.\"ww2:ger_mg_team\"]\ncooldown_group = \"ww2:support\"\n\
         [defs.units.\"ww2:ger_mortar_team\"]\ncooldown_group = \"ww2:support\"\n\
         initial_cooldown_s = 90\n",
    )
    .unwrap();
    defs.apply_override(&path, "ww2:skirmish").unwrap();
    let req = MatchRequest::skirmish(1, "ww2:germany", "ww2:usa", Difficulty::Normal);
    let s = defs.resolve(&req).unwrap();
    let ty = |id: &str| &s.squad_types[s.squad_type_index(id).unwrap() as usize];
    assert_eq!(ty("ww2:ger_mg_team").cooldown_group, Some(0));
    assert_eq!(ty("ww2:ger_mortar_team").cooldown_group, Some(0));
    assert_eq!(ty("ww2:ger_mortar_team").initial_cooldown_ticks, 90 * 30);
    assert_eq!(ty("ww2:ger_rifle_squad").cooldown_group, None);
    assert!(s.validate().is_empty());
    // Out of range and a group that is not a namespaced id.
    for (text, key) in [
        ("recall_cooldown_s = 601", "recall_cooldown_s"),
        ("initial_cooldown_s = 1801", "initial_cooldown_s"),
        ("cooldown_group = \"Support\"", "cooldown_group"),
    ] {
        std::fs::write(&path, format!("[defs.units.\"ww2:ger_mg_team\"]\n{text}\n")).unwrap();
        let e = ModDefs::load(&root)
            .unwrap()
            .apply_override(&path, "ww2:skirmish")
            .unwrap_err()
            .to_string();
        assert!(e.contains(key), "{key}: {e}");
    }
    // A vehicle squad takes its cooldowns from the vehicle def only.
    std::fs::write(
        &path,
        "[defs.units.\"ww2:ger_pz4_h_squad\"]\nrecall_cooldown_s = 10\n",
    )
    .unwrap();
    let e = ModDefs::load(&root)
        .unwrap()
        .apply_override(&path, "ww2:skirmish")
        .unwrap_err()
        .to_string();
    assert!(e.contains("recall_cooldown_s"), "{e}");
    // The setup re-checks (saves are untrusted).
    let mut bad = s.clone();
    bad.squad_types[0].recall_cooldown_ticks = 601 * 30;
    assert!(bad.validate().iter().any(|i| i.path.contains("cooldown")));
    let mut bad = s.clone();
    bad.squad_types[0].cooldown_group = Some(500);
    assert!(bad.validate().iter().any(|i| i.path.contains("cooldown")));
    let _ = std::fs::remove_file(&path);
}
