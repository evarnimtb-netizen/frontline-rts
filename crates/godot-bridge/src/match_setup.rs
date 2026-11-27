//! Menu choice → [`MatchSetup`] (S1 skirmish). The bridge is the presenter
//! side, so it may read files: it finds `mods/ww2`, loads the defs with the
//! open content crate and resolves a fixed-point setup, which the sim then
//! saves and hashes like any other state (S1 "Hard rules").
//!
//! In a skirmish the human takes player slot 0 (base 0, south-west) with
//! the chosen faction and the AI takes slot 1 with the other faction. A
//! net match (S2) names every slot's faction with [`MenuChoice::slots`]:
//! two human slots in a 1v1; S18: two to four slots, each with its team,
//! some of them AI fill.

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
    /// Net match (S2): each slot's faction, team and whether a human
    /// plays it (2 to 4 slots, S18). Overrides `faction`, `ai_vs_ai` and
    /// `mode`. Both clients resolve the same choice into the same setup
    /// (its hash is the `Ready` check).
    pub slots: Option<Vec<SlotChoice>>,
    /// S17 (Presenter P): `1v1` (default) or `2v2`: the human (slot 0)
    /// and an AI ally (slot 1, same faction) against two AIs of the other
    /// faction (slots 2 and 3) on a map with four bases (Harrow Cross).
    /// With `slots` (a net match) the slots decide; `2v2` then needs four.
    pub mode: String,
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
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct SlotChoice {
    /// Short faction id: `germany` or `usa`.
    pub faction: String,
    pub human: bool,
    /// S18: the slot's team (`None`: alone, as in a 1v1).
    pub team: Option<u8>,
    /// S18: an AI slot's difficulty (`easy`, `normal`, `hard`); `None`:
    /// the choice's.
    pub difficulty: Option<String>,
}

impl SlotChoice {
    /// A human slot of `faction`, no team (the 1v1 slots).
    pub fn human(faction: &str) -> SlotChoice {
        SlotChoice {
            faction: faction.into(),
            human: true,
            ..SlotChoice::default()
        }
    }
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
            opponent: None,
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
    if !MODES.contains(&choice.mode.as_str()) {
        return Err(format!("unknown mode {:?}", choice.mode));
    }
    // (slot, team, difficulty) per player.
    let team_slots: Vec<(SlotChoice, Option<u8>, Difficulty)> = match &choice.slots {
        // S18: net slots name every player (S17 review O4 refused a 2v2
        // with them; now they are the match).
        Some(slots) => {
            if !(2..=4).contains(&slots.len()) {
                return Err(format!("{} net slots (2 to 4)", slots.len()));
            }
            if choice.mode == "2v2" && slots.len() != 4 {
                return Err("a 2v2 needs four slots".into());
            }
            let mut out = Vec::new();
            for s in slots {
                let sd = match &s.difficulty {
                    Some(x) if !s.human => difficulty(x)?,
                    _ => d,
                };
                out.push((s.clone(), s.team, sd));
            }
            out
        }
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
            let me = SlotChoice {
                faction: choice.faction.clone(),
                human: !choice.ai_vs_ai,
                ..SlotChoice::default()
            };
            let them = SlotChoice {
                faction: other,
                human: false,
                ..SlotChoice::default()
            };
            // S17: in a 2v2 slot 0 (human, unless ai_vs_ai) and slot 1 (AI
            // ally) play the chosen faction, slots 2 and 3 (AI) the other;
            // teams 0 and 1.
            if choice.mode == "2v2" {
                let ally = SlotChoice {
                    human: false,
                    ..me.clone()
                };
                vec![
                    (me, Some(0), d),
                    (ally, Some(0), d),
                    (them.clone(), Some(1), d),
                    (them, Some(1), d),
                ]
            } else {
                vec![(me, None, d), (them, None, d)]
            }
        }
    };
    // S15 M1: every slot's faction is one the rules offer.
    let offered = menu_factions(defs, choice);
    for (s, ..) in &team_slots {
        if !offered.contains(&s.faction) {
            return Err(format!("unknown faction {:?}", s.faction));
        }
    }
    // M4: in a 1v1 (two slots, no teams) the same faction on both sides
    // only with `[matchups] mirror` 1. A team match leaves it to the
    // content's M2 check of every hostile pair (allies may share a
    // faction or not).
    let mirror = matchups(defs, choice).is_some_and(|m| m.mirror == 1);
    if team_slots.len() == 2
        && team_slots.iter().all(|t| t.1.is_none())
        && !mirror
        && team_slots[0].0.faction == team_slots[1].0.faction
    {
        return Err(format!("both slots are {:?}", team_slots[0].0.faction));
    }
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
            .map(|(s, team, d)| PlayerRequest {
                faction: format!("{MOD_ID}:{}", s.faction),
                ai: !s.human,
                difficulty: *d,
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

    /// S15 M3: a choice without an opponent resolves as before (Germany
    /// against the USA, the same setup bytes as naming the USA); the menu
    /// offers the shipped three factions (S15-ai enables the USSR, review
    /// 6). Control: the USA against the USSR is refused (the pair is off
    /// without `free`).
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
        assert_eq!(menu_factions(&defs, &c), ["germany", "usa", "ussr"]);
        assert_eq!(menu_opponents(&defs, &c, "germany"), ["usa", "ussr"]);
        assert_eq!(menu_opponents(&defs, &c, "usa"), ["germany"]);
        assert_eq!(menu_opponents(&defs, &c, "ussr"), ["germany"]);
        let bad = MenuChoice {
            faction: "usa".into(),
            opponent: Some("ussr".into()),
            ..MenuChoice::default()
        };
        assert!(resolve(&m, &bad).is_err(), "control");
    }

    /// S15 P (M3, M6): the shipped rules' `[matchups]` (Germany-USA,
    /// Germany-USSR) give the menu three factions with the partners each may
    /// face, the default first; USA against USSR gets the lobby's refusal
    /// words and the setup refuses it too; Germany against the USSR and the
    /// USSR's default opponent resolve. Control: with `ww2:ussr` and its pair
    /// taken out of the table the menu has two factions and one opponent
    /// each (the picker is hidden).
    #[test]
    fn menu_options_follow_the_matchups_table() {
        let mut defs = load_defs(&mods(), false).unwrap();
        let c = MenuChoice::default();
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
        let play = |defs: &ModDefs, f: &str, o: Option<&str>| {
            resolve_with(
                defs,
                &MenuChoice {
                    faction: f.into(),
                    opponent: o.map(str::to_owned),
                    ..MenuChoice::default()
                },
            )
        };
        let s = play(&defs, "ussr", None).unwrap();
        assert_eq!(
            short_id(&s.factions[s.players[1].faction as usize].id),
            "germany"
        );
        assert!(play(&defs, "germany", Some("ussr")).is_ok());
        assert!(
            play(&defs, "usa", Some("ussr")).is_err(),
            "the refused pair"
        );
        let r = defs.rules.get_mut("ww2:skirmish").unwrap();
        let m = r.matchups.as_mut().unwrap();
        m.factions.retain(|f| f != "ww2:ussr");
        m.pairs.retain(|p| !p.iter().any(|f| f == "ww2:ussr"));
        let two = menu_options(&defs, &c);
        assert_eq!(two.factions.len(), 2, "control: two factions");
        assert!(two.factions.iter().all(|f| f.0 != "ussr"));
        assert!(
            two.opponents.iter().all(|x| x.1.len() == 1),
            "control: one opponent each"
        );
    }

    #[test]
    fn rejects_bad_choices() {
        let m = mods();
        // S15 Fix (review 6): the USSR resolves now; no rules enable Japan.
        let bad = MenuChoice {
            faction: "japan".into(),
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

    /// S18: a 2v2 with two net slots is refused (S17 review O4 kept); with
    /// four, the slots are the match: teams, humans and AI fill as named.
    #[test]
    fn a_2v2_with_net_slots_needs_four() {
        let choice = MenuChoice {
            map: "harrow_cross".into(),
            mode: "2v2".into(),
            slots: Some(vec![SlotChoice::human("usa"), SlotChoice::human("germany")]),
            ..MenuChoice::default()
        };
        let e = resolve(&mods(), &choice).unwrap_err();
        assert!(e.contains("2v2"), "{e}");
        let slot = |f: &str, human: bool, team: u8| SlotChoice {
            faction: f.into(),
            human,
            team: Some(team),
            difficulty: (!human).then(|| "hard".into()),
        };
        let four = MenuChoice {
            slots: Some(vec![
                slot("usa", true, 0),
                slot("usa", false, 0),
                slot("germany", true, 1),
                slot("usa", false, 1),
            ]),
            ..choice
        };
        let s = resolve(&mods(), &four).unwrap();
        let team: Vec<u8> = s.players.iter().map(|p| p.team).collect();
        assert_eq!(team, [0, 0, 1, 1]);
        let ai: Vec<bool> = s.players.iter().map(|p| p.is_ai).collect();
        assert_eq!(ai, [false, true, false, true]);
        assert_eq!(s.players[1].difficulty, Difficulty::Hard);
        assert_eq!(s.players[0].difficulty, Difficulty::Normal);
    }

    #[test]
    fn net_slots_are_both_human() {
        let choice = MenuChoice {
            faction: "usa".into(),
            seed: 9,
            slots: Some(vec![SlotChoice::human("usa"), SlotChoice::human("germany")]),
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

    /// Merge of S15 with S17/S18: the USSR on a team. The 2v2 menu choice
    /// for the USSR plays two Soviet players against its default opponent
    /// (Germany) twice; net slots put the USA and the USSR on one team
    /// against two Germans (allies may be any two factions). Control: the
    /// USA against the USSR across teams is refused (`[matchups]` M2), and
    /// an unknown faction in a slot is refused.
    #[test]
    fn the_ussr_plays_on_a_team() {
        let m = mods();
        let f = |s: &MatchSetup, p: usize| {
            short_id(&s.factions[s.players[p].faction as usize].id).to_owned()
        };
        let menu = MenuChoice {
            faction: "ussr".into(),
            mode: "2v2".into(),
            map: "harrow_cross".into(),
            ..MenuChoice::default()
        };
        let s = resolve(&m, &menu).unwrap();
        assert_eq!(
            [f(&s, 0), f(&s, 1), f(&s, 2), f(&s, 3)],
            ["ussr", "ussr", "germany", "germany"]
        );
        let slot = |fa: &str, team: u8| SlotChoice {
            faction: fa.into(),
            human: true,
            team: Some(team),
            difficulty: None,
        };
        let net = |slots: Vec<SlotChoice>| MenuChoice {
            map: "harrow_cross".into(),
            mode: "2v2".into(),
            slots: Some(slots),
            ..MenuChoice::default()
        };
        let s = resolve(
            &m,
            &net(vec![
                slot("usa", 0),
                slot("ussr", 0),
                slot("germany", 1),
                slot("germany", 1),
            ]),
        )
        .unwrap();
        assert_eq!(
            [f(&s, 0), f(&s, 1), f(&s, 2), f(&s, 3)],
            ["usa", "ussr", "germany", "germany"]
        );
        let hostile = resolve(
            &m,
            &net(vec![
                slot("usa", 0),
                slot("germany", 0),
                slot("ussr", 1),
                slot("germany", 1),
            ]),
        );
        assert!(hostile.is_err(), "control: USA against USSR");
        let unknown = resolve(
            &m,
            &net(vec![
                slot("usa", 0),
                slot("france", 0),
                slot("germany", 1),
                slot("germany", 1),
            ]),
        );
        assert!(unknown.is_err(), "control: an unknown faction");
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
