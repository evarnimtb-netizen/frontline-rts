//! S10b content (docs/slices/S10b-obstacles.md 3, Build A): the five
//! blueprints and two mine items validate and lower, the bad shapes are
//! refused, and the three promoted capabilities parse into bits 12-14.

use std::path::{Path, PathBuf};

use frontline_content::setup::{Capability, Fixed, FortShape};
use frontline_content::{Difficulty, ItemKind, MatchRequest, ModDefs};

fn live_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../mods/ww2")
}

fn defs() -> ModDefs {
    ModDefs::load(&live_root()).unwrap_or_else(|e| panic!("{e}"))
}

fn req() -> MatchRequest {
    MatchRequest::skirmish(1, "ww2:germany", "ww2:usa", Difficulty::Normal)
}

/// Control: each bad copy below alone (drop its check from
/// `FortDef::validate` / `ModDefs::cross_check`) and its assert fails.
#[test]
fn s10b_defs_validate() {
    let d = defs();
    for id in [
        "ww2:ap_minefield",
        "ww2:at_minefield",
        "ww2:hedgehog",
        "ww2:log_barrier",
        "ww2:bunker",
    ] {
        let f = &d.forts[id];
        assert!(f.validate().is_empty(), "{id}: {:?}", f.validate());
    }
    for id in ["ww2:ap_mine", "ww2:at_mine"] {
        let it = &d.items[id];
        assert_eq!(it.kind, ItemKind::Mine);
        assert!(it.validate().is_empty(), "{id}: {:?}", it.validate());
    }
    assert!(d.cross_check().is_empty(), "{:?}", d.cross_check());
    // Lowered: mine_item is the item's index, obstacles block vehicles.
    let s = d.resolve(&req()).unwrap_or_else(|e| panic!("{e}"));
    let f = |id: &str| s.forts.iter().find(|f| f.id == id).unwrap();
    let item = |id: &str| s.items.iter().position(|i| i.id == id).map(|i| i as u16);
    assert_eq!(f("ww2:ap_minefield").mine_item, item("ww2:ap_mine"));
    assert_eq!(f("ww2:at_minefield").mine_item, item("ww2:at_mine"));
    assert_eq!(
        f("ww2:ap_minefield").capability,
        Capability::PlaceMines as u8
    );
    assert!(f("ww2:hedgehog").blocks_vehicles && f("ww2:log_barrier").blocks_vehicles);
    assert!(!f("ww2:bunker").blocks_vehicles && f("ww2:bunker").mine_item.is_none());
    assert_eq!(f("ww2:bunker").pieces.len(), 6);
    assert_eq!(f("ww2:hedgehog").shape, FortShape::Point);
    assert_eq!(f("ww2:hedgehog").pieces[0].part, 3, "steel");
    assert_eq!(f("ww2:at_minefield").segment, Fixed::from_cm(400));
    let ap = &s.items[item("ww2:ap_mine").unwrap() as usize];
    assert_eq!(
        ap.mine.as_ref().map(|m| (m.trigger, m.trigger_cm)),
        Some((0, 30))
    );
    let at = &s.items[item("ww2:at_mine").unwrap() as usize];
    assert_eq!(
        at.mine.as_ref().map(|m| (m.trigger, m.trigger_cm)),
        Some((1, 40))
    );
    assert!(s.rules.bunker_pit.depth > Fixed::ZERO);

    // A mine item in a kit.
    let mut bad = defs();
    let eng = bad.squads.get_mut("ww2:ger_engineer_squad").unwrap();
    eng.members[0].items.insert("ww2:ap_mine".into(), 1);
    assert!(
        bad.cross_check()
            .iter()
            .any(|i| i.message.contains("never carried in a kit")),
        "a mine in a kit"
    );
    // A minefield with pieces.
    let mut m = d.forts["ww2:ap_minefield"].clone();
    m.pieces = d.forts["ww2:sandbag_wall"].pieces.clone();
    assert!(!m.validate().is_empty(), "a minefield with pieces");
    // Seven pieces.
    let mut b = d.forts["ww2:bunker"].clone();
    b.pieces.push(b.pieces[1].clone());
    assert!(!b.validate().is_empty(), "seven pieces");
    // mine_item naming a frag.
    let mut bad = defs();
    bad.forts.get_mut("ww2:ap_minefield").unwrap().mine_item = Some("ww2:mk2_grenade".into());
    assert!(
        bad.cross_check()
            .iter()
            .any(|i| i.message.contains("is not a mine item")),
        "mine_item naming a frag"
    );
    // A mine item without [mine].
    let mut i = d.items["ww2:ap_mine"].clone();
    i.mine = None;
    assert!(!i.validate().is_empty(), "a mine without [mine]");
    // An obstacle without pieces, build_heavy over an HQ.
    let mut o = d.forts["ww2:barbed_wire"].clone();
    o.blocks_vehicles = true;
    assert!(!o.validate().is_empty(), "blocks_vehicles without pieces");
    let mut h = d.forts["ww2:hq_dugout"].clone();
    h.capability = "build_heavy".into();
    assert!(!h.validate().is_empty(), "build_heavy over an HQ");
}

/// Control: put any of the three back into `RESERVED_CAPABILITIES` and the
/// squad def's validation reports it reserved (and the bit is not set).
#[test]
fn promoted_capabilities_parse() {
    assert_eq!(
        [
            Capability::PlaceMines,
            Capability::ClearMines,
            Capability::Demolish
        ]
        .map(|c| (c.bit(), c.name())),
        [
            (1 << 12, "place_mines"),
            (1 << 13, "clear_mines"),
            (1 << 14, "demolish")
        ]
    );
    let mut d = defs();
    let eng = d.squads.get_mut("ww2:ger_engineer_squad").unwrap();
    for c in ["place_mines", "clear_mines", "demolish"] {
        eng.capabilities.push(c.into());
    }
    let issues = eng.validate();
    assert!(
        !issues.iter().any(|i| i.message.contains("reserved")),
        "{issues:?}"
    );
    let s = d.resolve(&req()).unwrap_or_else(|e| panic!("{e}"));
    let t = &s.squad_types[s.squad_type_index("ww2:ger_engineer_squad").unwrap() as usize];
    assert_eq!(t.caps >> 12 & 0b111, 0b111);
}
