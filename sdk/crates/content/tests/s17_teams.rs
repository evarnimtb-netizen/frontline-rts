//! S17a Build A (`docs/slices/S17-team-play.md` 2-4): the request's `team`,
//! the setup's `PlayerSetup::team`, `[teams]`, `[ai.team]`, the bases'
//! `side` and the checks on them. A request without teams resolves as
//! today: every player alone on his team (his slot).

use std::path::Path;

use frontline_content::tuning::{AiTeamTuning, TeamsTuning};
use frontline_content::{Difficulty, MapFileDef, MatchRequest, ModDefs};

fn ww2() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../mods/ww2")
}

fn defs() -> ModDefs {
    ModDefs::load(&ww2()).unwrap_or_else(|e| panic!("{e}"))
}

fn req(teams: [Option<u8>; 2]) -> MatchRequest {
    let mut r = MatchRequest::skirmish(1, "ww2:germany", "ww2:usa", Difficulty::Normal);
    for (p, t) in r.players.iter_mut().zip(teams) {
        p.team = t;
    }
    r
}

/// No `team`: each player on his slot's team. Control: explicit teams.
#[test]
fn a_request_without_teams_puts_each_player_alone() {
    let s = defs().resolve(&req([None, None])).unwrap();
    assert_eq!(s.players[0].team, 0);
    assert_eq!(s.players[1].team, 1);
    let s = defs().resolve(&req([Some(5), Some(2)])).unwrap();
    assert_eq!((s.players[0].team, s.players[1].team), (5, 2));
}

/// Plan 4: a request with one team is refused, and a team number must stay
/// under the player limit. Control: two teams resolve.
#[test]
fn a_one_team_request_is_refused() {
    let e = defs().resolve(&req([Some(3), Some(3)])).unwrap_err();
    assert!(format!("{e}").contains("one team"), "{e}");
    let e = defs().resolve(&req([Some(0), Some(10)])).unwrap_err();
    assert!(format!("{e}").contains("players[1].team"), "{e}");
    // A slot number equal to the other's explicit team is one team too.
    assert!(defs().resolve(&req([None, Some(0)])).is_err());
    assert!(defs().resolve(&req([Some(1), Some(0)])).is_ok());
}

/// Plan 2: the ww2 rules set every `[teams]` and `[ai.team]` key, at the
/// plan's values (the habit switches A3, A4 off until S17b).
#[test]
fn ww2_ships_the_s17a_team_keys() {
    let s = defs().resolve(&req([None, None])).unwrap();
    assert_eq!(s.rules.teams, TeamsTuning::default());
    assert_eq!(s.rules.ai.team, AiTeamTuning::default());
    assert_eq!(TeamsTuning::KEYS.len(), 7);
    assert_eq!(AiTeamTuning::KEYS.len(), 6);
    for &(k, lo, hi) in TeamsTuning::KEYS {
        if k.ends_with("_pct") {
            assert_eq!((lo, hi), (10, 300), "{k}");
        }
    }
}

/// Plan 4: `side` on bases warns unless it gives two sides of equal size.
/// Control: Kessel Ridge names no side (no warning).
#[test]
fn base_sides_warn_when_unequal() {
    let src = std::fs::read_to_string(ww2().join("defs/maps/kessel_ridge.toml")).unwrap();
    let mut m: MapFileDef = toml::from_str(&src).unwrap();
    assert!(m.side_warnings().is_empty());
    m.bases[0].side = Some(0);
    m.bases[1].side = Some(1);
    assert!(m.side_warnings().is_empty());
    m.bases[1].side = Some(0);
    assert_eq!(m.side_warnings().len(), 1);
    m.bases[1].side = None;
    assert_eq!(m.side_warnings().len(), 2);
}

/// Checker after A (finding 3; Build A's D3): a mine's `known` mask holds 8
/// players (`fort.rs` folds higher slots), so a request or a setup for 9 is
/// refused. Control: 2 players pass both checks.
#[test]
fn more_than_eight_players_are_refused() {
    let mut r = req([None, None]);
    let p = r.players[0].clone();
    r.players = vec![p; 9];
    let e = defs().resolve(&r).unwrap_err();
    assert!(format!("{e}").contains("at most 8"), "{e}");
    let mut s = defs().resolve(&req([None, None])).unwrap();
    assert!(s.validate().is_empty());
    let mut bases = s.map.bases.clone();
    while bases.len() < 9 {
        bases.push(s.map.bases[0].clone());
    }
    s.map.bases = bases;
    s.players = (0..9u8)
        .map(|i| {
            let mut p = s.players[0].clone();
            p.base = i;
            p.team = i;
            p
        })
        .collect();
    let issues = s.validate();
    assert!(
        issues.iter().any(|i| format!("{i}").contains("at most 8")),
        "{issues:?}"
    );
}
