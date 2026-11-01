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
        }
    }
}

pub const MOD_ID: &str = "ww2";
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

/// Loads the mod at `mods` and resolves `choice` into a setup.
pub fn resolve(mods: &Path, choice: &MenuChoice) -> Result<MatchSetup, String> {
    let defs = ModDefs::load(mods).map_err(|e| e.to_string())?;
    resolve_with(&defs, choice)
}

pub fn resolve_with(defs: &ModDefs, choice: &MenuChoice) -> Result<MatchSetup, String> {
    let d = difficulty(&choice.difficulty)?;
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
    let req = MatchRequest {
        rules: format!("{MOD_ID}:skirmish"),
        map: format!("{MOD_ID}:{}", choice.map),
        seed: choice.seed,
        players: slots
            .iter()
            .map(|s| PlayerRequest {
                faction: format!("{MOD_ID}:{}", s.faction),
                ai: !s.human,
                difficulty: d,
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

    #[test]
    fn candidates_prefer_explicit_and_project() {
        let c = mods_candidates(Some(Path::new("/x")), Some(Path::new("/p/godot")));
        assert_eq!(c[0], PathBuf::from("/x"));
        assert_eq!(c[1], PathBuf::from("/p/godot/../mods/ww2"));
        assert!(find_mods_dir(&[PathBuf::from("/definitely/not/here")]).is_none());
    }
}
