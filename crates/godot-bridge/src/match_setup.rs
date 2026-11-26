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
    /// S3: play the test rules override (`mods/ww2/tests/
    /// rules_test_unlock.toml`: vehicles unlocked at once), for scripted
    /// runs. The defs must hold it ([`load_defs`] with `test_unlock`).
    pub test_unlock: bool,
    /// S15 M3: the AI's faction (short id) in a skirmish; `None`: the
    /// default opponent of `faction` by the rules' `[matchups]` (the other
    /// member of the first pair holding it).
    pub opponent: Option<String>,
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
            test_unlock: false,
            opponent: None,
        }
    }
}

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

/// S15 M: the `[matchups]` of the rules `choice` plays (None: no table).
fn matchups<'a>(
    defs: &'a ModDefs,
    choice: &MenuChoice,
) -> Option<&'a frontline_content::MatchupsDef> {
    let rules = if choice.test_unlock {
        TEST_UNLOCK_RULES.to_owned()
    } else {
        format!("{MOD_ID}:skirmish")
    };
    defs.rules.get(&rules).and_then(|r| r.matchups.as_ref())
}

/// S15 M1: the factions the menus offer (short ids, the rules' `[matchups]
/// factions` order; without the table, [`FACTIONS`]).
pub fn menu_factions(defs: &ModDefs, choice: &MenuChoice) -> Vec<String> {
    match matchups(defs, choice) {
        Some(m) if !m.factions.is_empty() => {
            m.factions.iter().map(|f| short_id(f).to_owned()).collect()
        }
        _ => FACTIONS.iter().map(|f| (*f).to_owned()).collect(),
    }
}

/// S15 M2-M3: the opponents faction `f` (short id) may face in the menus,
/// the default first (empty: no table, the other of [`FACTIONS`]).
pub fn menu_opponents(defs: &ModDefs, choice: &MenuChoice, f: &str) -> Vec<String> {
    let all: Vec<String> = defs.factions.keys().cloned().collect();
    match matchups(defs, choice) {
        Some(m) => m
            .opponents(&format!("{MOD_ID}:{f}"), &all)
            .iter()
            .map(|o| short_id(o).to_owned())
            .collect(),
        None => FACTIONS
            .iter()
            .filter(|x| **x != f)
            .map(|x| (*x).to_owned())
            .collect(),
    }
}

/// S15 P (M3): what the menus and the lobby offer, read from the loaded
/// rules: every faction the rules enable (short id, display name, colour
/// `#rrggbb`, in the `[matchups] factions` order), the opponents each may
/// face (the default first), and whether a faction may fight itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuOptions {
    pub factions: Vec<(String, String, String)>,
    pub opponents: Vec<(String, Vec<String>)>,
    pub mirror: bool,
}

/// [`MenuOptions`] of the rules `choice` plays. With no `[matchups]` table
/// it is today's two factions facing each other.
pub fn menu_options(defs: &ModDefs, choice: &MenuChoice) -> MenuOptions {
    let ids = menu_factions(defs, choice);
    let factions = ids
        .iter()
        .map(|id| {
            let d = defs.factions.get(&format!("{MOD_ID}:{id}"));
            (
                id.clone(),
                d.map_or_else(|| id.clone(), |d| d.name.clone()),
                d.map_or_else(String::new, |d| d.color.clone()),
            )
        })
        .collect();
    let opponents = ids
        .iter()
        .map(|id| (id.clone(), menu_opponents(defs, choice, id)))
        .collect();
    MenuOptions {
        factions,
        opponents,
        mirror: matchups(defs, choice).is_some_and(|m| m.mirror == 1),
    }
}

/// The lobby's words for a refused pair (`a` and `b` short ids), or `None`
/// when the rules let them fight (the same faction only with `[matchups]
/// mirror`).
pub fn pair_refusal(defs: &ModDefs, choice: &MenuChoice, a: &str, b: &str) -> Option<String> {
    let o = menu_options(defs, choice);
    let name = |id: &str| {
        o.factions
            .iter()
            .find(|f| f.0 == id)
            .map_or_else(|| id.to_owned(), |f| f.1.clone())
    };
    if a == b {
        return (!o.mirror)
            .then(|| format!("Both players picked {}; factions must differ.", name(a)));
    }
    let ok = o
        .opponents
        .iter()
        .find(|(f, _)| f == a)
        .is_some_and(|(_, v)| v.iter().any(|x| x == b));
    (!ok).then(|| {
        format!(
            "{} cannot fight {} in this ruleset; pick another faction.",
            name(a),
            name(b)
        )
    })
}

pub fn resolve_with(defs: &ModDefs, choice: &MenuChoice) -> Result<MatchSetup, String> {
    let d = difficulty(&choice.difficulty)?;
    let slots = match &choice.slots {
        Some(s) => s.clone(),
        None => {
            // S15 M3: the chosen opponent, else the first pair's partner
            // (Germany -> USA, USA -> Germany as before).
            let other = match &choice.opponent {
                Some(o) => o.clone(),
                None => menu_opponents(defs, choice, &choice.faction)
                    .into_iter()
                    .next()
                    .ok_or_else(|| format!("no opponent for {:?}", choice.faction))?,
            };
            [
                SlotChoice {
                    faction: choice.faction.clone(),
                    human: !choice.ai_vs_ai,
                },
                SlotChoice {
                    faction: other,
                    human: false,
                },
            ]
        }
    };
    let offered = menu_factions(defs, choice);
    for s in &slots {
        if !offered.contains(&s.faction) {
            return Err(format!("unknown faction {:?}", s.faction));
        }
    }
    // M4: the same faction on both sides only with `[matchups] mirror` 1.
    let mirror = matchups(defs, choice).is_some_and(|m| m.mirror == 1);
    if !mirror && slots[0].faction == slots[1].faction {
        return Err(format!("both slots are {:?}", slots[0].faction));
    }
    let req = MatchRequest {
        rules: if choice.test_unlock {
            TEST_UNLOCK_RULES.to_owned()
        } else {
            format!("{MOD_ID}:skirmish")
        },
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

    /// S15 M3: a choice without an opponent resolves as before (Germany
    /// against the USA, the same setup bytes as naming the USA); the menu
    /// offers today's two factions. Control: an opponent not enabled is
    /// refused.
    #[test]
    fn menu_choice_without_opponent_is_base() {
        let m = mods();
        let base = resolve(&m, &MenuChoice::default()).unwrap();
        assert_eq!(
            short_id(&base.factions[base.players[1].faction as usize].id),
            "usa"
        );
        let named = MenuChoice {
            opponent: Some("usa".into()),
            ..MenuChoice::default()
        };
        let named = resolve(&m, &named).unwrap();
        assert_eq!(named, base);
        let defs = load_defs(&m, false).unwrap();
        let c = MenuChoice::default();
        assert_eq!(menu_factions(&defs, &c), ["germany", "usa"]);
        assert_eq!(menu_opponents(&defs, &c, "germany"), ["usa"]);
        assert_eq!(menu_opponents(&defs, &c, "usa"), ["germany"]);
        let bad = MenuChoice {
            opponent: Some("ussr".into()),
            ..MenuChoice::default()
        };
        assert!(resolve(&m, &bad).is_err(), "control");
    }

    /// S15 P (M3, M6): with the USSR switched on in the rules' `[matchups]`
    /// (Germany-USA, Germany-USSR) the menu lists three factions with the
    /// partners each may face, the default first; USA against USSR gets the
    /// lobby's refusal words and the setup refuses it too; Germany against
    /// the USSR and the USSR's default opponent resolve. Control: the
    /// shipped rules offer two factions and no USSR.
    #[test]
    fn menu_options_follow_the_matchups_table() {
        let mut defs = load_defs(&mods(), false).unwrap();
        let c = MenuChoice::default();
        let shipped = menu_options(&defs, &c);
        assert_eq!(shipped.factions.len(), 2, "control: today's menu");
        assert!(shipped.factions.iter().all(|f| f.0 != "ussr"));
        let r = defs.rules.get_mut("ww2:skirmish").unwrap();
        let m = r.matchups.as_mut().unwrap();
        m.factions.push("ww2:ussr".into());
        m.pairs.push(["ww2:germany".into(), "ww2:ussr".into()]);
        let o = menu_options(&defs, &c);
        let ids: Vec<&str> = o.factions.iter().map(|f| f.0.as_str()).collect();
        assert_eq!(ids, ["germany", "usa", "ussr"]);
        assert_eq!(o.factions[2].1, "USSR");
        assert_eq!(o.factions[2].2, "#9a5a4a");
        let opp = |f: &str| o.opponents.iter().find(|x| x.0 == f).unwrap().1.clone();
        assert_eq!(opp("germany"), ["usa", "ussr"]);
        assert_eq!(opp("usa"), ["germany"]);
        assert_eq!(opp("ussr"), ["germany"]);
        assert!(pair_refusal(&defs, &c, "germany", "ussr").is_none());
        let why = pair_refusal(&defs, &c, "usa", "ussr").unwrap();
        assert!(why.contains("USA cannot fight USSR"), "{why}");
        assert!(
            pair_refusal(&defs, &c, "ussr", "ussr")
                .unwrap()
                .contains("must differ")
        );
        let play = |f: &str, o: Option<&str>| {
            resolve_with(
                &defs,
                &MenuChoice {
                    faction: f.into(),
                    opponent: o.map(str::to_owned),
                    ..MenuChoice::default()
                },
            )
        };
        let s = play("ussr", None).unwrap();
        assert_eq!(
            short_id(&s.factions[s.players[1].faction as usize].id),
            "germany"
        );
        assert!(play("germany", Some("ussr")).is_ok());
        assert!(play("usa", Some("ussr")).is_err(), "the refused pair");
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
