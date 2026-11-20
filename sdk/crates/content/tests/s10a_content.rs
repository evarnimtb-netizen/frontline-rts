//! S10a content (docs/slices/S10a-fortifications.md 3.1, 3.4, 3.5, 7 Build
//! B): the three engineer cards resolve at `ai_weight` 0 at the end of the
//! rosters, the five promoted capabilities parse into their bits, the
//! existing engineers are field engineers, the Bangalore items lower, and
//! the five blueprints validate (with a broken copy rejected).

use std::path::{Path, PathBuf};

use frontline_content::setup::{Capability, Fixed, FortShape};
use frontline_content::{Difficulty, ItemKind, MatchRequest, MatchSetup, ModDefs};

fn live_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../mods/ww2")
}

fn setup() -> MatchSetup {
    let defs = ModDefs::load(&live_root()).unwrap_or_else(|e| panic!("{e}"));
    defs.resolve(&MatchRequest::skirmish(
        1,
        "ww2:germany",
        "ww2:usa",
        Difficulty::Normal,
    ))
    .unwrap_or_else(|e| panic!("{e}"))
}

fn ty<'a>(s: &'a MatchSetup, id: &str) -> &'a frontline_content::setup::SquadTypeSetup {
    &s.squad_types[s.squad_type_index(id).unwrap_or_else(|| panic!("{id}")) as usize]
}

#[test]
fn s10a_cards_resolve_with_their_ai_weights() {
    let s = setup();
    let roster = |f: &str| {
        let fac = s.factions.iter().find(|x| x.id == f).unwrap();
        fac.roster
            .iter()
            .map(|&t| s.squad_types[t as usize].id.clone())
            .collect::<Vec<_>>()
    };
    let ger = roster("ww2:germany");
    let usa = roster("ww2:usa");
    // Appended: existing indices unchanged (the S9 cards end before them;
    // S11a1 appends two cards a side after them, S11b two more, S12 one).
    let (ger, usa) = (&ger[..ger.len() - 5], &usa[..usa.len() - 5]);
    assert_eq!(
        &ger[ger.len() - 3..],
        [
            "ww2:ger_sdkfz11_squad",
            "ww2:ger_sturmpioniergruppe",
            "ww2:ger_baupioniergruppe"
        ]
    );
    assert_eq!(
        &usa[usa.len() - 2..],
        ["ww2:usa_57mm_m1_squad", "ww2:usa_assault_section"]
    );
    // S10a-ai (S10a-12 (A)): weights in the engineer slot; stage D (owner
    // 2026-11-11): Baupioniere 100 (Germany's default builder), Sturmpioniere
    // 60, the Pioniergruppe 40; the Assault Section keeps 35.
    assert_eq!(ty(&s, "ww2:ger_engineer_squad").ai_weight, 40);
    for (id, men, cost, tag, weight) in [
        ("ww2:ger_sturmpioniergruppe", 8, 264, "assault", 60),
        ("ww2:ger_baupioniergruppe", 8, 165, "construction", 100),
        ("ww2:usa_assault_section", 12, 380, "assault", 35),
    ] {
        let t = ty(&s, id);
        assert_eq!(t.ai_weight, weight, "{id}");
        assert_eq!(t.cost, cost, "{id}");
        assert_eq!(t.recall_cooldown_ticks, 60 * s.tick_hz, "{id}");
        assert_eq!(t.men as u32, men, "{id}");
        assert!(
            t.has_tag(s.tag("engineer")) && t.has_tag(s.tag(tag)),
            "{id}"
        );
    }
    // Bangalores: two with the Sturmpioniere and two with the Assault
    // Section; two satchels with the Sturmpioniere.
    let n = |id: &str, k: ItemKind| ty(&s, id).squad_items(k);
    assert_eq!(n("ww2:ger_sturmpioniergruppe", ItemKind::Bangalore), 2);
    assert_eq!(n("ww2:ger_sturmpioniergruppe", ItemKind::Satchel), 2);
    assert_eq!(n("ww2:usa_assault_section", ItemKind::Bangalore), 2);
    assert_eq!(n("ww2:ger_baupioniergruppe", ItemKind::Bangalore), 0);
    for id in ["ww2:gestreckte_ladung", "ww2:m1a1_bangalore"] {
        let it = s.items.iter().find(|x| x.id == id).unwrap();
        assert_eq!(it.kind, ItemKind::Bangalore);
        assert!(!it.kind.thrown());
        let b = it.bangalore.as_ref().unwrap();
        assert_eq!((b.bursts, b.place_ticks), (3, 5 * s.tick_hz));
        assert_eq!(b.length, Fixed::from_cm(300));
        assert_eq!(it.fuse_ticks, 8 * s.tick_hz);
        let he = it.he.as_ref().unwrap();
        assert_eq!(he.damage, 300);
        assert_eq!(he.blast.blast_radius, Fixed::from_cm(150));
        assert!(!he.cuts_trenches);
    }
}

#[test]
fn promoted_capabilities_parse() {
    let s = setup();
    let caps = |id: &str| {
        let t = ty(&s, id);
        Capability::ALL
            .iter()
            .filter(|&&c| t.has(c))
            .copied()
            .collect::<Vec<_>>()
    };
    use Capability::*;
    // The bits of spec 3.4.
    assert_eq!(
        [BuildStructure, BuildHeavy, PlaceWire, ClearWire, Breach].map(|c| c.bit()),
        [1 << 7, 1 << 8, 1 << 9, 1 << 10, 1 << 11]
    );
    for id in ["ww2:ger_engineer_squad", "ww2:usa_engineer_squad"] {
        assert_eq!(
            caps(id),
            [
                DigBasic,
                DigFull,
                Recrew,
                BuildStructure,
                PlaceWire,
                ClearWire,
                // S10b Build B, C, D (R27).
                PlaceMines,
                ClearMines,
                Demolish
            ],
            "{id}"
        );
    }
    assert_eq!(
        caps("ww2:ger_sturmpioniergruppe"),
        [DigBasic, Recrew, ClearWire, Breach, ClearMines, Demolish]
    );
    assert_eq!(
        caps("ww2:ger_baupioniergruppe"),
        [
            DigBasic,
            DigFull,
            BuildStructure,
            BuildHeavy,
            PlaceWire,
            ClearWire,
            PlaceMines,
            ClearMines
        ]
    );
    // The bazooka derives `at_hunt` from its weapon.
    let us = caps("ww2:usa_assault_section");
    for c in [DigBasic, Recrew, ClearWire, Breach, ClearMines] {
        assert!(us.contains(&c), "{c:?}");
    }
    // S10b R27: `demolish` only on the cards with a satchel.
    assert!(!us.contains(&Demolish));
    assert!(!caps("ww2:ger_baupioniergruppe").contains(&Demolish));
    assert!(!us.contains(&BuildStructure) && !us.contains(&PlaceWire));
    // Line squads gained nothing.
    let rifles = caps("ww2:ger_grenadiergruppe");
    assert!(rifles.iter().all(|c| !matches!(
        c,
        BuildStructure | BuildHeavy | PlaceWire | ClearWire | Breach
    )));
}

#[test]
fn fort_defs_validate() {
    let defs = ModDefs::load(&live_root()).unwrap_or_else(|e| panic!("{e}"));
    let ids: Vec<&str> = defs.forts.keys().map(|k| k.as_str()).collect();
    assert_eq!(
        ids,
        [
            // S10b: minefields, the bunker, hedgehog and log barrier.
            "ww2:ap_minefield",
            "ww2:at_minefield",
            "ww2:barbed_wire",
            "ww2:bunker",
            "ww2:dugout",
            "ww2:hedgehog",
            "ww2:hq_dugout",
            "ww2:log_barrier",
            "ww2:mg_nest",
            "ww2:sandbag_wall"
        ]
    );
    for f in defs.forts.values() {
        assert!(f.validate().is_empty(), "{}: {:?}", f.id, f.validate());
    }
    let s = setup();
    let wire = s.forts.iter().find(|f| f.id == "ww2:barbed_wire").unwrap();
    assert_eq!(wire.shape, FortShape::Line);
    assert_eq!(wire.capability, 9);
    let w = wire.wire.as_ref().unwrap();
    assert_eq!(
        (w.band, w.height, w.hp),
        (Fixed::from_cm(150), Fixed::from_cm(90), 200)
    );
    assert!(wire.pieces.is_empty());
    // S10a-ai stage D (owner 2026-11-11): 60 m lines, 100 sections a player.
    assert_eq!(
        (wire.max_length, wire.limit_per_player),
        (Fixed::from_int(60), 100)
    );
    // A broken copy: a wire def with a piece is refused.
    let mut bad = defs.forts["ww2:barbed_wire"].clone();
    bad.pieces = defs.forts["ww2:sandbag_wall"].pieces.clone();
    assert!(!bad.validate().is_empty());
    let mut bad = defs.forts["ww2:mg_nest"].clone();
    bad.pieces.clear();
    assert!(!bad.validate().is_empty());
}
