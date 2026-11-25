//! Menu choice → [`MatchSetup`] (S1 skirmish). The bridge is the presenter
//! side, so it may read files: it finds `mods/ww2`, loads the defs with the
//! open content crate and resolves a fixed-point setup, which the sim then
//! saves and hashes like any other state (S1 "Hard rules").
//!
//! In a skirmish the human takes player slot 0 (base 0, south-west) with
//! the chosen faction and the AI takes slot 1 with the other faction. A
//! net match (S2) names both slots' factions with [`MenuChoice::slots`],
//! and both slots are human.

use std::path::{Path, PathBuf};

use frontline_content::{Difficulty, MatchRequest, MatchSetup, ModDefs, PlayerRequest};

/// What the main menu (`Session.setup`) hands the match.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuChoice {
    /// Faction short id without the mod prefix: `germany` or `usa`.
    pub faction: String,
    /// `easy`, `normal` or `hard`.
    pub difficulty: String,
    /// Map short id: `kessel_ridge`.
    pub map: String,
    pub seed: u64,
    /// Both slots AI (spectating, checks); the local view stays slot 0.
    pub ai_vs_ai: bool,
    /// Optional overrides for short or scripted matches.
    pub tickets: Option<u32>,
    pub start_manpower: Option<u32>,
    /// Net match (S2): each slot's faction and whether a human plays it.
    /// Overrides `faction` and `ai_vs_ai`. Both clients resolve the same
    /// choice into the same setup (its hash is the `Ready` check).
    pub slots: Option<[SlotChoice; 2]>,
    /// S17 (Presenter P): `1v1` (default) or `2v2`: the human (slot 0)
    /// and an AI ally (slot 1, same faction) against two AIs of the other
    /// faction (slots 2 and 3) on a map with four bases (Harrow Cross).
    /// Offline only: a `2v2` with `slots` is refused (online teams: S18).
    pub mode: String,
    /// S3: play the test rules override (`mods/ww2/tests/
    /// rules_test_unlock.toml`: vehicles unlocked at once), for scripted
    /// runs. The defs must hold it ([`load_defs`] with `test_unlock`).
    pub test_unlock: bool,
}

/// One player slot of a net match.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlotChoice {
    /// Short faction id: `germany` or `usa`.
    pub faction: String,
    pub human: bool,
}

impl Default for MenuChoice {
    fn default() -> MenuChoice {
        MenuChoice {
            faction: "germany".into(),
            difficulty: "normal".into(),
            map: "kessel_ridge".into(),
            seed: 1,
            ai_vs_ai: false,
            tickets: None,
            start_manpower: None,
            slots: None,
            mode: "1v1".into(),
            test_unlock: false,
        }
    }
}

/// S17: the menu's team modes.
pub const MODES: [&str; 2] = ["1v1", "2v2"];

pub const MOD_ID: &str = "ww2";
/// Rules id of the S3 test override (vehicles unlocked at once).
pub const TEST_UNLOCK_RULES: &str = "ww2:skirmish_test_unlock";
/// Its file, relative to the mod directory.
pub const TEST_UNLOCK_FILE: &str = "tests/rules_test_unlock.toml";
pub const FACTIONS: [&str; 2] = ["germany", "usa"];

/// The first candidate that holds a `mod.toml`.
pub fn find_mods_dir(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates
        .iter()
        .find(|p| p.join("mod.toml").is_file())
        .cloned()
}

/// Where `mods/ww2` may be, most specific first: an explicit directory,
/// next to the Godot project (`<project>/../mods/ww2`, the source tree), next
/// to the executable (exports), in a macOS app's `Contents/Resources`
/// (`tools/export_macos.sh` puts it there, so the bundle stays signed), and
/// the source tree this crate was built from.
pub fn mods_candidates(explicit: Option<&Path>, project_dir: Option<&Path>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(p) = explicit {
        out.push(p.to_owned());
    }
    if let Some(p) = project_dir {
        out.push(p.join("../mods").join(MOD_ID));
        out.push(p.join("mods").join(MOD_ID));
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        out.push(dir.join("mods").join(MOD_ID));
        out.push(dir.join("../Resources/mods").join(MOD_ID));
    }
    out.push(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../mods")
            .join(MOD_ID),
    );
    out
}

fn difficulty(s: &str) -> Result<Difficulty, String> {
    match s {
        "easy" => Ok(Difficulty::Easy),
        "normal" => Ok(Difficulty::Normal),
        "hard" => Ok(Difficulty::Hard),
        other => Err(format!("unknown difficulty {other:?}")),
    }
}

/// Loads the mod at `mods`, with the S3 test rules override when
/// `test_unlock` is set.
pub fn load_defs(mods: &Path, test_unlock: bool) -> Result<ModDefs, String> {
    let mut defs = ModDefs::load(mods).map_err(|e| e.to_string())?;
    if test_unlock {
        defs.add_rules_override(&mods.join(TEST_UNLOCK_FILE))
            .map_err(|e| e.to_string())?;
    }
    Ok(defs)
}

/// Loads the mod at `mods` and resolves `choice` into a setup.
pub fn resolve(mods: &Path, choice: &MenuChoice) -> Result<MatchSetup, String> {
    let defs = load_defs(mods, choice.test_unlock)?;
    resolve_with(&defs, choice)
}

pub fn resolve_with(defs: &ModDefs, choice: &MenuChoice) -> Result<MatchSetup, String> {
    let d = difficulty(&choice.difficulty)?;
    if !MODES.contains(&choice.mode.as_str()) {
        return Err(format!("unknown mode {:?}", choice.mode));
    }
    // S17 review (O4): net slots name two players; a networked 2v2 is
    // S18, so a 2v2 with slots is refused rather than played as a 1v1.
    if choice.mode == "2v2" && choice.slots.is_some() {
        return Err("a 2v2 with net slots is not supported (S18)".into());
    }
    let slots = match &choice.slots {
        Some(s) => s.clone(),
        None => {
            let other = if choice.faction == FACTIONS[0] {
                FACTIONS[1]
            } else {
                FACTIONS[0]
            };
            [
                SlotChoice {
                    faction: choice.faction.clone(),
                    human: !choice.ai_vs_ai,
                },
                SlotChoice {
                    faction: other.into(),
                    human: false,
                },
            ]
        }
    };
    for s in &slots {
        if !FACTIONS.contains(&s.faction.as_str()) {
            return Err(format!("unknown faction {:?}", s.faction));
        }
    }
    if slots[0].faction == slots[1].faction {
        return Err(format!("both slots are {:?}", slots[0].faction));
    }
    // S17: in a 2v2 slot 0 (human, unless ai_vs_ai) and slot 1 (AI ally)
    // play the chosen faction, slots 2 and 3 (AI) the other; teams 0 and 1.
    let team_slots: Vec<(SlotChoice, Option<u8>)> = if choice.mode == "2v2" {
        let ally = SlotChoice {
            faction: slots[0].faction.clone(),
            human: false,
        };
        vec![
            (slots[0].clone(), Some(0)),
            (ally, Some(0)),
            (slots[1].clone(), Some(1)),
            (slots[1].clone(), Some(1)),
        ]
    } else {
        slots.iter().map(|s| (s.clone(), None)).collect()
    };
    let req = MatchRequest {
        rules: if choice.test_unlock {
            TEST_UNLOCK_RULES.to_owned()
        } else {
            format!("{MOD_ID}:skirmish")
        },
        map: format!("{MOD_ID}:{}", choice.map),
        seed: choice.seed,
        players: team_slots
            .iter()
            .map(|(s, team)| PlayerRequest {
                faction: format!("{MOD_ID}:{}", s.faction),
                ai: !s.human,
                difficulty: d,
                team: *team,
            })
            .collect(),
    };
    let mut setup = defs.resolve(&req).map_err(|e| e.to_string())?;
    if let Some(t) = choice.tickets {
        setup.rules.tickets = t.max(1);
    }
    if let Some(mp) = choice.start_manpower {
        setup.rules.start_manpower = mp;
    }
    Ok(setup)
}

/// `ww2:germany` → `germany`.
pub fn short_id(id: &str) -> &str {
    id.rsplit(':').next().unwrap_or(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mods() -> PathBuf {
        find_mods_dir(&mods_candidates(None, None)).expect("mods/ww2 in the source tree")
    }

    #[test]
    fn resolves_the_menu_choice() {
        let choice = MenuChoice {
            faction: "usa".into(),
            difficulty: "hard".into(),
            seed: 7,
            tickets: Some(120),
            ..MenuChoice::default()
        };
        let s = resolve(&mods(), &choice).unwrap();
        assert_eq!(s.seed, 7);
        assert_eq!(s.rules.tickets, 120);
        assert_eq!(s.players.len(), 2);
        assert!(!s.players[0].is_ai && s.players[1].is_ai);
        assert_eq!(
            short_id(&s.factions[s.players[0].faction as usize].id),
            "usa"
        );
        assert_eq!(
            short_id(&s.factions[s.players[1].faction as usize].id),
            "germany"
        );
        assert_eq!(s.players[1].difficulty, Difficulty::Hard);
        assert_eq!(s.map.points.len(), 5);
    }

    #[test]
    fn rejects_bad_choices() {
        let m = mods();
        let bad = MenuChoice {
            faction: "ussr".into(),
            ..MenuChoice::default()
        };
        assert!(resolve(&m, &bad).is_err());
        let bad = MenuChoice {
            difficulty: "insane".into(),
            ..MenuChoice::default()
        };
        assert!(resolve(&m, &bad).is_err());
        let bad = MenuChoice {
            map: "nowhere".into(),
            ..MenuChoice::default()
        };
        assert!(resolve(&m, &bad).is_err());
    }

    #[test]
    fn a_2v2_with_net_slots_is_refused() {
        let slot = |f: &str| SlotChoice {
            faction: f.into(),
            human: true,
        };
        let choice = MenuChoice {
            map: "harrow_cross".into(),
            mode: "2v2".into(),
            slots: Some([slot("usa"), slot("germany")]),
            ..MenuChoice::default()
        };
        let e = resolve(&mods(), &choice).unwrap_err();
        assert!(e.contains("2v2"), "{e}");
    }

    #[test]
    fn net_slots_are_both_human() {
        let choice = MenuChoice {
            faction: "usa".into(),
            seed: 9,
            slots: Some([
                SlotChoice {
                    faction: "usa".into(),
                    human: true,
                },
                SlotChoice {
                    faction: "germany".into(),
                    human: true,
                },
            ]),
            ..MenuChoice::default()
        };
        let s = resolve(&mods(), &choice).unwrap();
        assert!(s.players.iter().all(|p| !p.is_ai));
        assert_eq!(
            short_id(&s.factions[s.players[0].faction as usize].id),
            "usa"
        );
        let mut same = choice.clone();
        same.slots.as_mut().unwrap()[1].faction = "usa".into();
        assert!(resolve(&mods(), &same).is_err());
    }

    /// S17: the 2v2 menu choice on Harrow Cross: the human and an AI ally
    /// of his faction (team 0) against two AIs of the other (team 1). A 2v2
    /// on a two-base map and an unknown mode are refused.
    #[test]
    fn a_2v2_choice_gives_an_ally_and_two_opponents() {
        let choice = MenuChoice {
            mode: "2v2".into(),
            map: "harrow_cross".into(),
            ..MenuChoice::default()
        };
        let s = resolve(&mods(), &choice).unwrap();
        let team: Vec<u8> = s.players.iter().map(|p| p.team).collect();
        assert_eq!(team, [0, 0, 1, 1]);
        let human: Vec<bool> = s.players.iter().map(|p| !p.is_ai).collect();
        assert_eq!(human, [true, false, false, false]);
        let f = |p: usize| short_id(&s.factions[s.players[p].faction as usize].id).to_owned();
        assert_eq!(
            [f(0), f(1), f(2), f(3)],
            ["germany", "germany", "usa", "usa"]
        );
        let two_bases = MenuChoice {
            mode: "2v2".into(),
            ..MenuChoice::default()
        };
        assert!(resolve(&mods(), &two_bases).is_err());
        let bad = MenuChoice {
            mode: "3v3".into(),
            ..choice
        };
        assert!(resolve(&mods(), &bad).is_err());
    }

    #[test]
    fn test_unlock_rules_unlock_vehicles_at_once() {
        let normal = resolve(&mods(), &MenuChoice::default()).unwrap();
        assert!(normal.rules.vehicles.unlock_ticks > 0);
        let choice = MenuChoice {
            test_unlock: true,
            ..MenuChoice::default()
        };
        let s = resolve(&mods(), &choice).unwrap();
        assert_eq!(s.rules.vehicles.unlock_ticks, 0);
        // Everything else is the base rules.
        assert_eq!(s.rules.tickets, normal.rules.tickets);
        // Without the override loaded the rules id is unknown.
        let defs = ModDefs::load(&mods()).unwrap();
        assert!(resolve_with(&defs, &choice).is_err());
    }

    #[test]
    fn candidates_prefer_explicit_and_project() {
        let c = mods_candidates(Some(Path::new("/x")), Some(Path::new("/p/godot")));
        assert_eq!(c[0], PathBuf::from("/x"));
        assert_eq!(c[1], PathBuf::from("/p/godot/../mods/ww2"));
        assert!(find_mods_dir(&[PathBuf::from("/definitely/not/here")]).is_none());
    }
}
