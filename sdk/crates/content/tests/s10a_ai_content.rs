//! S10a-ai (docs/slices/S10a-ai.md 3.7): the `[ai.fort]` table parses from
//! the ww2 rules, reaches the setup, works with overrides (`--set`) and is
//! range-checked.

use std::path::{Path, PathBuf};

use frontline_content::tuning::AiFortTuning;
use frontline_content::{Difficulty, MatchRequest, ModDefs};

fn ww2_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../mods/ww2")
}

fn write(name: &str, text: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "frontline-s10a-ai-{name}-{}.toml",
        std::process::id()
    ));
    std::fs::write(&p, text).unwrap();
    p
}

#[test]
fn ai_fort_tuning_parses_and_validates() {
    let mut defs = ModDefs::load(&ww2_root()).unwrap_or_else(|e| panic!("{e}"));
    let req = MatchRequest::skirmish(1, "ww2:germany", "ww2:usa", Difficulty::Normal);
    let s = defs.resolve(&req).unwrap();
    // The mod restates the starting values, every switch on.
    assert_eq!(s.rules.ai.fort, AiFortTuning::default());
    let f = &s.rules.ai.fort;
    assert_eq!((f.works, f.wire, f.breach, f.cards), (1, 1, 1, 1));
    // Stage D: the per-work switches, all on; 60 m wire lines.
    assert_eq!((f.nest, f.dugout, f.roof, f.sandbags), (1, 1, 1, 1));
    assert_eq!(
        (
            f.wire_out_m,
            f.wire_len_m,
            f.materials_max_pct,
            f.breach_tries
        ),
        (35, 60, 3, 3)
    );
    // B1-4: `hold_max` (2, today's cap with builders_max 2).
    assert_eq!(f.hold_max, 2);
    assert_eq!(AiFortTuning::KEYS.len(), 40);
    assert!(
        frontline_content::tuning::TABLES
            .iter()
            .any(|(t, k)| *t == "ai.fort" && k.len() == 40)
    );
    // An override reaches the setup (the ablation switches).
    let off = write(
        "off",
        "[ai.fort]\nworks = 0\nwire = 0\nbreach = 0\ncards = 0\nnest = 0\nsandbags = 0\n",
    );
    defs.apply_override(&off, "ww2:skirmish")
        .unwrap_or_else(|e| panic!("{e}"));
    let s = defs.resolve(&req).unwrap();
    let f = &s.rules.ai.fort;
    assert_eq!((f.works, f.wire, f.breach, f.cards), (0, 0, 0, 0));
    assert_eq!((f.nest, f.dugout, f.roof, f.sandbags), (0, 1, 1, 0));
    assert_eq!(f.wire_out_m, 35, "the other keys stay");
    // Out of range: the issue names the key and a fix.
    let bad = write("range", "[ai.fort]\nwire = 2\n");
    let e = defs
        .apply_override(&bad, "ww2:skirmish")
        .unwrap_err()
        .to_string();
    assert!(
        e.contains("ai.fort.wire") && e.contains("[ai.fort] wire = 1"),
        "{e}"
    );
    let bad = write("tries", "[ai.fort]\nbreach_tries = 0\n");
    let e = defs
        .apply_override(&bad, "ww2:skirmish")
        .unwrap_err()
        .to_string();
    assert!(e.contains("ai.fort.breach_tries"), "{e}");
    // An unknown key is refused.
    let typo = write("typo", "[ai.fort]\nwire_out = 30\n");
    let e = defs
        .apply_override(&typo, "ww2:skirmish")
        .unwrap_err()
        .to_string();
    assert!(e.contains("wire_out"), "{e}");
    // The setup is checked too (saves are untrusted).
    let mut s = defs.resolve(&req).unwrap();
    assert!(s.validate().is_empty());
    s.rules.ai.fort.smoke_frac_pct = 101;
    assert!(
        s.validate()
            .iter()
            .any(|i| i.path.contains("ai.fort.smoke_frac_pct")
                || i.message.contains("ai.fort.smoke_frac_pct")),
        "{:?}",
        s.validate()
    );
}
