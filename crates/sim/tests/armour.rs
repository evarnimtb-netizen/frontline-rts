//! S3 armour tables in a match world: built from the setup at match start,
//! not saved, rebuilt identically after load; the state hash does not
//! depend on them.

use std::path::Path;

use frontline_content::{ArmourZone, Difficulty, MatchRequest, MatchSetup, ModDefs};
use frontline_sim::armour::{BoxPart, Face, PenResult, deg, impact_cos, penetrates, shot_dir};
use frontline_sim::{Fx, World};

/// The S1-S3c content frozen before the S4 content switch: these mechanics
/// tests were written for its squads (8-man rifle squads, 3-man MG teams)
/// and keep running on it; the S4 cards are covered by `s4_members.rs`,
/// `variants.rs` and the AI-vs-AI runs.
const S3C_MOD: &str = "../../sdk/crates/content/tests/fixtures/ww2_s3c";

fn setup() -> MatchSetup {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(S3C_MOD);
    let defs = ModDefs::load(&root).unwrap_or_else(|e| panic!("{e}"));
    defs.resolve(&MatchRequest::skirmish(
        5,
        "ww2:germany",
        "ww2:usa",
        Difficulty::Normal,
    ))
    .unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn tables_are_built_at_start_and_rebuilt_after_load() {
    let s = setup();

    let pz = s
        .vehicles
        .iter()
        .position(|v| v.id == "ww2:ger_pz4_h")
        .unwrap();
    let m61 = s.ammo.iter().position(|a| a.id == "ww2:75mm_m61").unwrap();
    let mut w = World::new_match(s, 2).unwrap();
    for _ in 0..30 {
        w.step(&[]);
    }
    let t = w.game().unwrap().armour().clone();
    assert_eq!(t.vehicles.len(), 2);
    assert_eq!(t.ammo.len(), 7);
    // The Panzer IV's hull front: 80 mm, normal tilted up 10°.
    let front = t.vehicles[pz].face(BoxPart::Hull, Face::Front);
    assert_eq!(front.zone, Some(ArmourZone::HullFront));
    assert_eq!(front.thickness, Fx::from_int(80));
    assert_eq!(front.normal.z, deg(10).sin());
    // M61 at 500 m head-on (S3b Fix: Bird & Livingston's 0° row, 81 mm)
    // against 81.2 mm effective, so the high end of the band penetrates
    // and the low end does not (golden row 16: 46 %).
    let a = &t.ammo[m61];
    let c = impact_cos(shot_dir(Face::Front, Fx::ZERO, Fx::ZERO), front.normal);
    let pen0 = a.pen0(Fx::from_int(500));
    let low = -frontline_sim::armour::PEN_BAND;
    assert_eq!(
        penetrates(pen0, c, front.thickness, Fx::ZERO, a.cos_ricochet, low),
        PenResult::NonPenetration
    );
    assert_eq!(
        penetrates(
            pen0,
            c,
            front.thickness,
            Fx::ZERO,
            a.cos_ricochet,
            frontline_sim::armour::PEN_BAND
        ),
        PenResult::Penetration
    );

    let hash = w.state_hash();
    let bytes = w.save();
    let mut l = World::load(&bytes, 3).unwrap();
    assert_eq!(l.game().unwrap().armour(), &t, "rebuilt after load");
    assert_eq!(l.state_hash(), hash);
    for _ in 0..30 {
        w.step(&[]);
        l.step(&[]);
    }
    assert_eq!(l.state_hash(), w.state_hash());
}
