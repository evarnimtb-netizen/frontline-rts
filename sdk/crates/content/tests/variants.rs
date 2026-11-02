//! S4 variant system in the content crate (docs/slices/S4-variants.md,
//! Part A): `extends` on every def kind, `[[members]]`, the card block,
//! capabilities, tags and training, with actionable errors; overrides on a
//! parent reach its variants; what today's sim cannot run is refused at
//! resolve with "needs S4 Part B".

use std::path::{Path, PathBuf};

use frontline_content::{ContentError, Difficulty, MatchRequest, ModDefs, SquadRole};

/// S4: the loader tests extend and patch the S1-S3c legacy cards, frozen
/// before the content switch (`fixtures/ww2_s3c`).
fn ww2_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ww2_s3c")
}

/// The live `mods/ww2` (the S4 content and its example variant).
fn live_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../mods/ww2")
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

/// A copy of the ww2 mod with `files` (path under the mod, text) added or
/// replaced.
fn mod_with(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dst = std::env::temp_dir().join(format!("frontline-s4-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dst);
    copy_dir(&ww2_root(), &dst);
    for (rel, text) in files {
        let p = dst.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
    dst
}

fn load_with(name: &str, files: &[(&str, &str)]) -> Result<ModDefs, ContentError> {
    let root = mod_with(name, files);
    let r = ModDefs::load(&root);
    let _ = std::fs::remove_dir_all(&root);
    r
}

fn errors(r: Result<ModDefs, ContentError>) -> String {
    match r {
        Ok(_) => panic!("expected errors"),
        Err(e) => e.to_string(),
    }
}

fn skirmish() -> MatchRequest {
    MatchRequest::skirmish(1, "ww2:germany", "ww2:usa", Difficulty::Normal)
}

fn write_tmp(name: &str, text: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("frontline-s4-{name}-{}.toml", std::process::id()));
    std::fs::write(&p, text).unwrap();
    p
}

const UNIT: &str = "defs/units/zz_variant.toml";

#[test]
fn the_example_variant_resolves_from_its_parent() {
    let root = live_root();
    let mut d = ModDefs::load(&root).unwrap();
    d.add_defs_dir(&root.join("tests/variants")).unwrap();
    let v = &d.squads["ww2:ger_grenadiergruppe_example"];
    let p = &d.squads["ww2:ger_grenadiergruppe"];
    assert_eq!(v.role, SquadRole::Rifle);
    assert_eq!(v.tags, p.tags);
    // Its own members carry no Panzerfaust: `at_hunt` is derived, so it
    // drops out.
    assert_eq!(v.capabilities, ["dig_basic", "recrew"]);
    assert!(p.has_capability("at_hunt"));
    assert_eq!(
        (v.cost, v.cooldown_s, v.speed_cm_s),
        (p.cost, p.cooldown_s, p.speed_cm_s)
    );
    assert_eq!(v.members.iter().map(|m| m.count).sum::<u8>(), 6);
    // The variant's own file is kept raw (the SDK view), with its parent.
    let raw = d.raw.get("units", &v.id).unwrap();
    assert_eq!(raw.extends(), Some("ww2:ger_grenadiergruppe"));
    // It is not in a roster without the roster patch, but it resolves.
    let s = d.resolve(&skirmish()).unwrap();
    let t = s.squad_type_index(&v.id).unwrap();
    assert_eq!(s.squad_types[t as usize].men, 6);
    // Its id cannot be added twice.
    let e = d
        .clone()
        .add_defs_dir(&root.join("tests/variants"))
        .unwrap_err()
        .to_string();
    assert!(e.contains("already a units def"), "{e}");
}

/// S4 content (B6): the shipped cards resolve with their men, costs,
/// tags and the rules' pop cap and tag mix.
#[test]
fn the_s4_content_resolves_with_historical_squads() {
    let d = ModDefs::load(&live_root()).unwrap();
    let s = d.resolve(&skirmish()).unwrap();
    let ty = |id: &str| &s.squad_types[s.squad_type_index(id).unwrap() as usize];
    for (id, men, cost) in [
        ("ww2:ger_grenadiergruppe", 10, 280),
        ("ww2:ger_grenadiergruppe_7", 7, 200),
        ("ww2:ger_smg_gruppe", 6, 195),
        ("ww2:ger_engineer_squad", 8, 250),
        ("ww2:usa_rifle_squad", 12, 360),
        ("ww2:usa_rifle_squad_9", 9, 275),
        ("ww2:usa_rifle_squad_2bar", 12, 380),
        ("ww2:usa_lmg_squad", 5, 280),
        ("ww2:usa_engineer_squad", 8, 280),
        ("ww2:ger_mortar_team", 3, 280),
        ("ww2:usa_mortar_team", 3, 280),
    ] {
        assert_eq!((ty(id).men, ty(id).cost), (men, cost), "{id}");
    }
    // One gun per MG and mortar card (S4-2).
    let guns = |id: &str| {
        ty(id)
            .members
            .iter()
            .filter(|m| s.weapons[m.weapon as usize].crew)
            .map(|m| m.count as u32)
            .sum::<u32>()
    };
    for id in [
        "ww2:ger_smg_gruppe",
        "ww2:usa_lmg_squad",
        "ww2:ger_mortar_team",
        "ww2:usa_mortar_team",
        "ww2:ger_grenadiergruppe",
    ] {
        assert_eq!(guns(id), 1, "{id}");
    }
    // Two Panzerfausts per Grenadiergruppe, one per Feldstärke squad.
    let pzf = |id: &str| ty(id).squad_items(frontline_content::ItemKind::SingleShotAt);
    assert_eq!(
        (
            pzf("ww2:ger_grenadiergruppe"),
            pzf("ww2:ger_grenadiergruppe_7")
        ),
        (2, 1)
    );
    assert!(ty("ww2:ger_grenadiergruppe_7").has_tag(s.tag("understrength")));
    assert_eq!(s.rules.pop_cap, 150);
    let mix: Vec<(&str, u32)> = s
        .rules
        .ai
        .mix
        .iter()
        .map(|m| (s.tags[m.tag as usize].as_str(), m.pct))
        .collect();
    assert_eq!(
        mix,
        [
            ("line", 50),
            ("mg", 20),
            ("engineer", 15),
            ("mortar", 15),
            ("at", 20),
            ("vehicle", 20)
        ]
    );
    // Start squads: two full line squads and the engineers (S4-14 (A)).
    let men = |f: &str| {
        let f = s.factions.iter().find(|x| x.id == f).unwrap();
        f.start_squads
            .iter()
            .map(|&t| s.squad_types[t as usize].men as u32)
            .sum::<u32>()
    };
    assert_eq!((men("ww2:germany"), men("ww2:usa")), (28, 32));
    // Every S4 card is regular (S4-10 (A)).
    assert!(
        s.squad_types
            .iter()
            .all(|t| s.training[t.training as usize].is_regular())
    );
    // The two-BAR card is dated; skirmish has no date filter.
    assert!(s.factions.iter().any(|f| {
        f.roster
            .contains(&s.squad_type_index("ww2:usa_rifle_squad_2bar").unwrap())
    }));
}

#[test]
fn extends_errors_are_actionable() {
    // A cycle names its defs.
    let e = errors(load_with(
        "cycle",
        &[
            (
                "defs/units/zz_a.toml",
                "id = \"ww2:zz_a\"\nextends = \"ww2:zz_b\"\n",
            ),
            (
                "defs/units/zz_b.toml",
                "id = \"ww2:zz_b\"\nextends = \"ww2:zz_a\"\n",
            ),
        ],
    ));
    assert!(
        e.contains("defs/units/zz_a.toml: extends")
            && e.contains("ww2:zz_a -> ww2:zz_b -> ww2:zz_a"),
        "{e}"
    );
    // An unknown parent suggests the closest id.
    let e = errors(load_with(
        "unknown",
        &[(
            UNIT,
            "id = \"ww2:zz_variant\"\nextends = \"ww2:usa_rifle_sqad\"\n",
        )],
    ));
    assert!(e.contains("did you mean `ww2:usa_rifle_squad`"), "{e}");
    // A parent of another kind is named as such.
    let e = errors(load_with(
        "kind",
        &[(UNIT, "id = \"ww2:zz_variant\"\nextends = \"ww2:kar98k\"\n")],
    ));
    assert!(e.contains("`ww2:kar98k` is a weapons def"), "{e}");
    // Five parents are one too many.
    let mut files = Vec::new();
    let texts: Vec<(String, String)> = (0..6)
        .map(|k| {
            let parent = if k == 0 {
                "ww2:usa_rifle_squad".to_owned()
            } else {
                format!("ww2:zz_{}", k - 1)
            };
            (
                format!("defs/units/zz_{k}.toml"),
                format!("id = \"ww2:zz_{k}\"\nextends = \"{parent}\"\n"),
            )
        })
        .collect();
    for (p, t) in &texts {
        files.push((p.as_str(), t.as_str()));
    }
    let e = errors(load_with("deep", &files));
    assert!(
        e.contains("defs/units/zz_4.toml: extends") && e.contains("5 parents deep"),
        "{e}"
    );
    assert!(!e.contains("zz_3.toml: extends"), "{e}");
}

#[test]
fn unit_rules_of_the_merge_are_enforced() {
    // Legacy soldier keys over a parent with [[members]].
    let e = errors(load_with(
        "legacy_over_members",
        &[
            (
                "defs/units/zz_a.toml",
                "id = \"ww2:zz_a\"\nextends = \"ww2:usa_rifle_squad\"\n\
                 [[members]]\nslot = \"r\"\ncount = 4\nweapon = \"ww2:m1_garand\"\n",
            ),
            (
                "defs/units/zz_b.toml",
                "id = \"ww2:zz_b\"\nextends = \"ww2:zz_a\"\nmen = 6\n",
            ),
        ],
    ));
    assert!(
        e.contains("zz_b.toml: men") && e.contains("parent `ww2:zz_a` uses [[members]]"),
        "{e}"
    );
    // An alias and its S4 name in one file.
    let e = errors(load_with(
        "alias",
        &[(
            UNIT,
            "id = \"ww2:zz_variant\"\nextends = \"ww2:usa_rifle_squad\"\n\
             cooldown_s = 10\nrecall_cooldown_s = 12\n",
        )],
    ));
    assert!(e.contains("are one key"), "{e}");
    // The S4 name over a parent's alias replaces it.
    let d = load_with(
        "alias_ok",
        &[(
            UNIT,
            "id = \"ww2:zz_variant\"\nextends = \"ww2:usa_rifle_squad\"\ncooldown_s = 10\n",
        )],
    )
    .unwrap();
    let v = &d.squads["ww2:zz_variant"];
    assert_eq!((v.cooldown_s, v.recall_cooldown_s), (Some(10), 0));
    let s = d.resolve(&skirmish()).unwrap();
    let t = s.squad_type_index("ww2:zz_variant").unwrap();
    assert_eq!(s.squad_types[t as usize].recall_cooldown_ticks, 10 * 30);
    // Members and legacy keys in one file.
    let e = errors(load_with(
        "both",
        &[(
            UNIT,
            "id = \"ww2:zz_variant\"\nname = \"X\"\nfaction = \"ww2:usa\"\ntags = [\"line\"]\n\
             men = 3\nspeed_cm_s = 300\nsight_m = 150\nspacing_dm = 30\n\
             [[members]]\nslot = \"r\"\nweapon = \"ww2:m1_garand\"\n",
        )],
    ));
    assert!(e.contains("set either [[members]] or the legacy"), "{e}");
}

#[test]
fn members_tags_capabilities_and_dates_are_checked() {
    let e = errors(load_with(
        "labels",
        &[(
            UNIT,
            r#"
id = "ww2:zz_variant"
extends = "ww2:usa_rifle_squad"
tags = ["line", "Bad-Tag", "line"]
capabilities = ["dig_basic", "breach", "dig_ful", "barrage"]
available_from = "1944-13"
available_to = "1943-01"
short_name = "A name far too long"
[[members]]
slot = "lead"
count = 2
weapon = "ww2:m1_garand"
traits = ["leader", "officer", "crew_gunr"]
items = { "ww2:mk2_grenade" = 11, "ww2:stielhandgranate" = 1, "ww2:nope" = 1 }
[[members]]
slot = "r"
count = 15
weapon = "ww2:m1_garnd"
"#,
        )],
    ));
    for want in [
        "tags[1]",
        "tags[2]: `line` is listed twice",
        "capability `breach` is reserved",
        "did you mean `dig_full`",
        "`barrage` comes from a mortar",
        "available_from: `1944-13` is not a month",
        "short_name",
        "a `leader` slot comes first and has count = 1",
        "trait `officer` is reserved",
        "did you mean `crew_gunner`",
        "items.\"ww2:mk2_grenade\": must be in 0..=10",
        "a second Frag item",
        "unknown item `ww2:nope`",
        "the slots hold 17 men",
        "did you mean `ww2:m1_garand`",
    ] {
        assert!(e.contains(want), "missing {want:?} in\n{e}");
    }
    // A unit without role or tags says what to add.
    let e = errors(load_with(
        "notags",
        &[(
            UNIT,
            "id = \"ww2:zz_variant\"\nname = \"X\"\nfaction = \"ww2:usa\"\n\
             speed_cm_s = 300\nsight_m = 150\nspacing_dm = 30\n\
             [[members]]\nslot = \"r\"\nweapon = \"ww2:m1_garand\"\n",
        )],
    ));
    assert!(e.contains("zz_variant.toml: tags: required"), "{e}");
    // The reserved vehicle and crew tags must match the card.
    let e = errors(load_with(
        "reserved",
        &[(
            UNIT,
            "id = \"ww2:zz_variant\"\nextends = \"ww2:usa_rifle_squad\"\n\
             tags = [\"line\", \"vehicle\", \"crew\"]\n",
        )],
    ));
    assert!(
        e.contains("`vehicle` marks exactly") && e.contains("`crew` marks exactly"),
        "{e}"
    );
    // An issue in an inherited value names the parent.
    let e = errors(load_with(
        "from_parent",
        &[
            (
                "defs/units/zz_a.toml",
                "id = \"ww2:zz_a\"\nextends = \"ww2:usa_rifle_squad\"\nabstract = true\n\
                 sight_m = 0\n",
            ),
            (UNIT, "id = \"ww2:zz_variant\"\nextends = \"ww2:zz_a\"\n"),
        ],
    ));
    assert!(
        e.contains("zz_variant.toml: sight_m: must be in 1..=2000, got 0 (from `ww2:zz_a`)"),
        "{e}"
    );
}

/// S4 review M1-M3: cards the sim would run wrongly (idle launcher men,
/// crews whose saves cannot load) are refused with actionable errors.
#[test]
fn launcher_and_crew_cards_the_sim_cannot_run_are_refused() {
    const HEAD: &str = "id = \"ww2:zz_variant\"\nname = \"X\"\nfaction = \"ww2:germany\"\n\
                        speed_cm_s = 300\nsight_m = 150\nspacing_dm = 30\n";
    let card = |tags: &str, caps: &str, members: &str| {
        format!("{HEAD}tags = [{tags}]\ncapabilities = [{caps}]\n{members}")
    };
    let slot = |name: &str, n: u8, w: &str| {
        format!("[[members]]\nslot = \"{name}\"\ncount = {n}\nweapon = \"{w}\"\n")
    };
    let lead = "[[members]]\nslot = \"leader\"\nweapon = \"ww2:kar98k\"\ntraits = [\"leader\"]\n";
    // A launcher man behind the MG: never the main weapon.
    let text = card(
        "\"line\"",
        "\"recrew\"",
        &format!(
            "{lead}{}{}",
            slot("mg", 1, "ww2:mg42"),
            slot("at", 1, "ww2:rpzb54")
        ),
    );
    let e = errors(load_with("m3_main", &[(UNIT, &text)]));
    assert!(
        e.contains("a launcher must be the squad's main weapon"),
        "{e}"
    );
    // Two launcher men: one carrier only.
    let text = card(
        "\"at\"",
        "",
        &format!(
            "{}{}",
            slot("at", 2, "ww2:rpzb54"),
            slot("r", 1, "ww2:kar98k")
        ),
    );
    let e = errors(load_with("m3_two", &[(UNIT, &text)]));
    assert!(e.contains("2 men carry a launcher"), "{e}");
    // A crew without `vehicle_crew`, and a crew with a launcher.
    let text = card("\"crew\"", "\"recrew\"", &slot("c", 3, "ww2:p38"));
    let e = errors(load_with("m2_crew", &[(UNIT, &text)]));
    assert!(
        e.contains("the tag `crew` and the capability `vehicle_crew` go together"),
        "{e}"
    );
    let text = card("\"line\"", "\"vehicle_crew\"", &slot("c", 3, "ww2:kar98k"));
    let e = errors(load_with("m2_cap", &[(UNIT, &text)]));
    assert!(e.contains("go together"), "{e}");
    let text = card(
        "\"crew\"",
        "\"vehicle_crew\"",
        &format!("{}{}", slot("c", 1, "ww2:rpzb54"), slot("d", 2, "ww2:p38")),
    );
    let e = errors(load_with("m1_crew", &[(UNIT, &text)]));
    assert!(e.contains("a crew squad cannot carry a launcher"), "{e}");
}

#[test]
fn training_profiles_and_rules_keys_are_checked() {
    let e = errors(load_with(
        "training",
        &[
            (
                "defs/training/zz.toml",
                "id = \"ww2:training_zz\"\nname = \"Z\"\nspread_pct = 500\n",
            ),
            (
                UNIT,
                "id = \"ww2:zz_variant\"\nextends = \"ww2:usa_rifle_squad\"\n\
                 training = \"ww2:training_regualr\"\n",
            ),
        ],
    ));
    assert!(
        e.contains("spread_pct: must be in 25..=400, got 500"),
        "{e}"
    );
    assert!(e.contains("did you mean `ww2:training_regular`"), "{e}");
    // The rules' date and default training are checked too.
    let mut d = ModDefs::load(&ww2_root()).unwrap();
    let p = write_tmp(
        "rules_keys",
        "[call_in]\ndate = \"June 44\"\n[training]\ndefault = \"ww2:training_x\"\n",
    );
    let e = d
        .apply_override(&p, "ww2:skirmish")
        .unwrap_err()
        .to_string();
    assert!(
        e.contains("call_in.date") && e.contains("unknown training profile"),
        "{e}"
    );
}

#[test]
fn an_override_on_a_parent_reaches_its_variants() {
    // The live content: the US rifle squad and its two shipped variants
    // (the understrength and two-BAR squads set their own cost).
    let mut d = ModDefs::load(&live_root()).unwrap();
    let p = write_tmp(
        "parent",
        "[defs.units.\"ww2:usa_rifle_squad\"]\ncost = 300\nsight_m = 160\n",
    );
    let changes = d.apply_override(&p, "ww2:skirmish").unwrap();
    let shown: Vec<String> = changes.iter().map(|c| c.to_string()).collect();
    assert_eq!(
        shown,
        [
            "defs.units.\"ww2:usa_rifle_squad\": cost: 360 -> 300",
            "defs.units.\"ww2:usa_rifle_squad\": sight_m: 150 -> 160",
            "defs.units.\"ww2:usa_rifle_squad_2bar\": sight_m: 150 -> 160",
            "defs.units.\"ww2:usa_rifle_squad_9\": sight_m: 150 -> 160",
        ]
    );
    assert_eq!(d.squads["ww2:usa_rifle_squad_9"].cost, 275);
    assert_eq!(d.squads["ww2:usa_rifle_squad_9"].sight_m, 160);
    // A [[members]] patch on a legacy squad (the frozen S3c content)
    // replaces its soldier keys.
    let mut d = ModDefs::load(&ww2_root()).unwrap();
    let p = write_tmp(
        "members_patch",
        "[defs.units.\"ww2:ger_mg_team\"]\n\
         [[defs.units.\"ww2:ger_mg_team\".members]]\nslot = \"gunner\"\nweapon = \"ww2:mg42\"\n\
         [[defs.units.\"ww2:ger_mg_team\".members]]\nslot = \"rifleman\"\ncount = 3\n\
         weapon = \"ww2:kar98k\"\n",
    );
    d.apply_override(&p, "ww2:skirmish").unwrap();
    let s = d.resolve(&skirmish()).unwrap();
    let ty = &s.squad_types[s.squad_type_index("ww2:ger_mg_team").unwrap() as usize];
    assert_eq!(ty.men, 4);
    assert_eq!(
        ty.members.iter().map(|m| m.count).collect::<Vec<_>>(),
        [1, 3]
    );
    assert_ne!(ty.members[0].weapon, ty.members[1].weapon);
}

#[test]
fn what_part_a_refused_resolves_in_part_b() {
    // S4 Part B (B1): three weapons, member sidearms, other training,
    // card limits and capabilities that differ from the legacy role's all
    // reach the sim now.
    for (name, extra) in [
        (
            "three_weapons",
            "[[members]]\nslot = \"a\"\nweapon = \"ww2:m1_garand\"\n\
             [[members]]\nslot = \"b\"\nweapon = \"ww2:m3_smg\"\n\
             [[members]]\nslot = \"c\"\nweapon = \"ww2:m1919\"\n",
        ),
        (
            "sidearm",
            "[[members]]\nslot = \"a\"\nweapon = \"ww2:m1919\"\nsidearm = \"ww2:m1911a1\"\n",
        ),
        ("training", "training = \"ww2:training_conscript\"\n"),
        ("max_alive", "max_alive = 2\n"),
        (
            "caps",
            "capabilities = [\"dig_basic\", \"dig_full\", \"recrew\"]\n",
        ),
    ] {
        let text = format!("id = \"ww2:zz_variant\"\nextends = \"ww2:usa_rifle_squad\"\n{extra}");
        let d = load_with(name, &[(UNIT, &text)]).unwrap_or_else(|e| panic!("{name}: {e}"));
        let s = d
            .resolve(&skirmish())
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(s.squad_type_index("ww2:zz_variant").is_some(), "{name}");
    }
    // An unknown limit group is still refused (at load: no rules def has
    // it), with a hint.
    let text =
        "id = \"ww2:zz_variant\"\nextends = \"ww2:usa_rifle_squad\"\nlimit_group = \"ww2:zz\"\n";
    let e = load_with("limit", &[(UNIT, text)]).unwrap_err().to_string();
    assert!(e.contains("limit group `ww2:zz`"), "{e}");
}

#[test]
fn abstract_templates_resolve_but_never_reach_a_match() {
    let files = [
        (
            "defs/units/zz_template.toml",
            "id = \"ww2:zz_template\"\nextends = \"ww2:usa_rifle_squad\"\nabstract = true\n\
             name = \"Template\"\n",
        ),
        (
            UNIT,
            "id = \"ww2:zz_variant\"\nextends = \"ww2:zz_template\"\nname = \"From template\"\n",
        ),
    ];
    let d = load_with("abstract", &files).unwrap();
    assert!(d.squads["ww2:zz_template"].is_abstract);
    assert!(!d.squads["ww2:zz_variant"].is_abstract);
    let s = d.resolve(&skirmish()).unwrap();
    assert!(s.squad_type_index("ww2:zz_template").is_none());
    assert!(s.squad_type_index("ww2:zz_variant").is_some());
    // Squad indices stay dense: the crew squads still point at crews.
    for v in &s.vehicles {
        let c = v.crew_squad.unwrap();
        assert!(s.type_has_tag(c, "crew"));
    }
    let usa = std::fs::read_to_string(ww2_root().join("defs/factions/usa.toml"))
        .unwrap()
        .replace(
            "\"ww2:usa_rifle_squad\",\n    \"ww2:usa_mg_team\"",
            "\"ww2:usa_rifle_squad\",\n    \"ww2:zz_template\",\n    \"ww2:usa_mg_team\"",
        );
    let mut with_roster = files.to_vec();
    with_roster.push(("defs/factions/usa.toml", &usa));
    let e = errors(load_with("abstract_roster", &with_roster));
    assert!(e.contains("squad `ww2:zz_template` is abstract"), "{e}");
}

#[test]
fn the_match_date_filters_rosters() {
    let files = [(
        UNIT,
        "id = \"ww2:zz_variant\"\nextends = \"ww2:usa_rifle_squad\"\n\
         available_from = \"1944-08\"\n",
    )];
    let usa = std::fs::read_to_string(ww2_root().join("defs/factions/usa.toml"))
        .unwrap()
        .replace(
            "\"ww2:usa_mg_team\",",
            "\"ww2:usa_mg_team\",\n    \"ww2:zz_variant\",",
        );
    let mut all = files.to_vec();
    all.push(("defs/factions/usa.toml", &usa));
    let mut d = load_with("date", &all).unwrap();
    let in_roster = |d: &ModDefs| {
        let s = d.resolve(&skirmish()).unwrap();
        let t = s.squad_type_index("ww2:zz_variant").unwrap();
        s.factions.iter().any(|f| f.roster.contains(&t))
    };
    // Skirmish has no date: every card is fielded.
    assert!(in_roster(&d));
    let june = write_tmp("june", "[call_in]\ndate = \"1944-06\"\n");
    let mut early = d.clone();
    early.apply_override(&june, "ww2:skirmish").unwrap();
    assert!(!in_roster(&early));
    let sept = write_tmp("sept", "[call_in]\ndate = \"1944-09\"\n");
    d.apply_override(&sept, "ww2:skirmish").unwrap();
    assert!(in_roster(&d));
    // A start squad outside the date is an error at resolve.
    let start = write_tmp(
        "start",
        "[defs.factions.\"ww2:usa\"]\nstart_squads = [\"ww2:zz_variant\"]\n",
    );
    early.apply_override(&start, "ww2:skirmish").unwrap();
    let e = early.resolve(&skirmish()).unwrap_err().to_string();
    assert!(e.contains("not fielded on the match date 1944-06"), "{e}");
}

#[test]
fn the_ai_mix_by_tag_maps_one_to_one_onto_the_old_keys() {
    let rules = std::fs::read_to_string(ww2_root().join("defs/rules/skirmish.toml")).unwrap();
    let mut out = String::new();
    for line in rules.lines() {
        if line.starts_with("mix_") {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out = out.replace(
        "[ai.infantry]",
        "[ai.mix]\nline = 50\nmg = 20\nengineer = 15\nmortar = 15\nvehicle = 20\nat = 20\n\n\
         [ai.infantry]",
    );
    let d = load_with("mix", &[("defs/rules/skirmish.toml", &out)]).unwrap();
    let base = ModDefs::load(&ww2_root()).unwrap();
    let bytes = |d: &ModDefs| postcard::to_allocvec(&d.resolve(&skirmish()).unwrap()).unwrap();
    assert_eq!(bytes(&d), bytes(&base));
    // Both forms at once is an error naming the keys to move.
    let mut both = d.clone();
    let p = write_tmp("mix_both", "[ai]\nmix_mg_pct = 20\n");
    let e = both
        .apply_override(&p, "ww2:skirmish")
        .unwrap_err()
        .to_string();
    assert!(
        e.contains("ai.mix_mg_pct") && e.contains("[ai.mix] mg"),
        "{e}"
    );
    // S4 Part B: a new tag resolves; it follows the legacy six in the mix.
    let p = write_tmp("mix_new_tag", "[ai.mix]\nassault = 10\n");
    both.apply_override(&p, "ww2:skirmish").unwrap();
    let s = both.resolve(&skirmish()).unwrap();
    let last = s.rules.ai.mix.last().unwrap();
    assert_eq!(
        (s.tags[last.tag as usize].as_str(), last.pct),
        ("assault", 10)
    );
}

#[test]
fn extends_works_for_every_def_kind() {
    let d = load_with(
        "kinds",
        &[
            (
                "defs/weapons/zz.toml",
                "id = \"ww2:zz_kar\"\nextends = \"ww2:kar98k\"\nname = \"Kar (long)\"\nrange_m = 400\n",
            ),
            (
                "defs/items/zz.toml",
                "id = \"ww2:zz_frag\"\nextends = \"ww2:mk2_grenade\"\n[he]\nfrag_count = 10\n",
            ),
            (
                "defs/vehicles/zz.toml",
                "id = \"ww2:zz_pz4\"\nextends = \"ww2:ger_pz4_h\"\nname = \"Pz IV (test)\"\ncost = 700\n",
            ),
            (
                "defs/ammo/zz.toml",
                "id = \"ww2:zz_ap\"\nextends = \"ww2:75mm_pzgr39\"\nname = \"AP (test)\"\n",
            ),
            (
                "defs/maps/zz.toml",
                "id = \"ww2:zz_map\"\nextends = \"ww2:kessel_ridge\"\nname = \"Ridge (test)\"\n",
            ),
            (
                "defs/rules/zz.toml",
                "id = \"ww2:zz_rules\"\nextends = \"ww2:skirmish\"\nname = \"R\"\n[economy]\n\
                 start_manpower = 900\n",
            ),
            (
                "defs/training/zz.toml",
                "id = \"ww2:zz_green\"\nextends = \"ww2:training_conscript\"\nname = \"Green\"\n\
                 spread_pct = 140\n",
            ),
        ],
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let (k, base) = (&d.weapons["ww2:zz_kar"], &d.weapons["ww2:kar98k"]);
    assert_eq!((k.range_m, k.damage), (400, base.damage));
    let f = d.items["ww2:zz_frag"].he.as_ref().unwrap();
    let fb = d.items["ww2:mk2_grenade"].he.as_ref().unwrap();
    assert_eq!((f.frag_count, f.damage), (10, fb.damage));
    assert_eq!(d.vehicles["ww2:zz_pz4"].cost, 700);
    assert_eq!(
        d.vehicles["ww2:zz_pz4"].hull,
        d.vehicles["ww2:ger_pz4_h"].hull
    );
    assert_eq!(
        d.ammo["ww2:zz_ap"].caliber_mm,
        d.ammo["ww2:75mm_pzgr39"].caliber_mm
    );
    assert_eq!(
        d.maps["ww2:zz_map"].capture_points,
        d.maps["ww2:kessel_ridge"].capture_points
    );
    assert_eq!(d.rules["ww2:zz_rules"].economy.start_manpower, 900);
    assert_eq!(
        d.rules["ww2:zz_rules"].tickets,
        d.rules["ww2:skirmish"].tickets
    );
    let g = &d.training["ww2:zz_green"];
    assert_eq!((g.spread_pct, g.reload_pct), (140, 115));
}
