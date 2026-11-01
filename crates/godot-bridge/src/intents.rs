//! Player intents (godot/docs/match_state.md) → sim [`CommandKind`]s.
//! Plain Rust: Godot-space floats in, fixed-point commands out.
//!
//! This is the one place presenter floats become sim input. Positions are
//! quantised to millimetres once, here, and the resulting commands are what
//! the sim (and a replay or a lockstep peer) sees, so the conversion never
//! has to be bit-identical across machines.
//!
//! S1b soldier-level intents carry `units` (soldier ids). They are filtered
//! to the local player's living men, sorted, deduplicated and capped at
//! [`MAX_UNITS`] here ([`unit_set`]); the sim validates them again.

use frontline_content::{ItemKind, MatchSetup, SquadRole};
use frontline_sim::game::MAX_DIG_POINTS;
use frontline_sim::{CommandKind, Fx, FxVec2, MAX_UNITS, SimId, SquadId, StanceOrder};

/// Lateral spacing (m) between squads sent to one point together.
pub const GROUP_SPACING_M: f32 = 18.0;
/// Shortest piece of a dig line handed to one squad.
pub const MIN_DIG_PIECE_M: f32 = 4.0;

/// Godot `(x, z)` → sim position (millimetre precision).
pub fn godot_to_fx(x: f32, z: f32, half: f32) -> FxVec2 {
    sim_to_fx([x + half, -z + half])
}

pub fn sim_to_fx(p: [f32; 2]) -> FxVec2 {
    let q = |v: f32| Fx::from_milli((v * 1000.0).round().clamp(-2.0e9, 2.0e9) as i32);
    FxVec2::new(q(p[0]), q(p[1]))
}

/// Role name used by the presenter (`rifle`, `mg`, `engineer`, `mortar`;
/// S3: `at`, `vehicle`).
pub fn role_name(r: SquadRole) -> &'static str {
    match r {
        SquadRole::Rifle => "rifle",
        SquadRole::Mg => "mg",
        SquadRole::Engineer => "engineer",
        SquadRole::Mortar => "mortar",
        SquadRole::AntiTank => "at",
        SquadRole::Vehicle => "vehicle",
    }
}

/// The squad type of `role` in player `player`'s faction roster.
pub fn call_in_type(setup: &MatchSetup, player: u8, role: &str) -> Option<u16> {
    let f = setup.players.get(player as usize)?.faction as usize;
    setup.factions.get(f)?.roster.iter().copied().find(|&t| {
        setup
            .squad_types
            .get(t as usize)
            .is_some_and(|ty| role_name(ty.role) == role)
    })
}

/// Destinations for `n` squads at `anchors` (sim xy) ordered to `target`:
/// a line across the group's approach direction, `spacing` apart, in the
/// order that keeps their paths from crossing.
pub fn spread_targets(anchors: &[[f32; 2]], target: [f32; 2], spacing: f32) -> Vec<[f32; 2]> {
    let n = anchors.len();
    if n <= 1 {
        return vec![target; n];
    }
    let c = anchors.iter().fold([0.0f32; 2], |a, p| {
        [a[0] + p[0] / n as f32, a[1] + p[1] / n as f32]
    });
    let (dx, dy) = (target[0] - c[0], target[1] - c[1]);
    let len = (dx * dx + dy * dy).sqrt();
    let dir = if len > 1e-3 {
        [dx / len, dy / len]
    } else {
        [0.0, 1.0]
    };
    let side = [-dir[1], dir[0]];
    // Rank squads by where they stand across the approach line.
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| {
        let pa = anchors[a][0] * side[0] + anchors[a][1] * side[1];
        let pb = anchors[b][0] * side[0] + anchors[b][1] * side[1];
        pa.total_cmp(&pb).then(a.cmp(&b))
    });
    let mut out = vec![target; n];
    for (rank, &i) in order.iter().enumerate() {
        let k = rank as f32 - (n - 1) as f32 * 0.5;
        out[i] = [
            target[0] + side[0] * k * spacing,
            target[1] + side[1] * k * spacing,
        ];
    }
    out
}

/// Splits `line` into `parts` consecutive pieces of equal length (fewer if
/// a piece would be shorter than `min_len`). Each piece keeps the original
/// corners it spans, so the trenches join up.
pub fn split_polyline(line: &[[f32; 2]], parts: usize, min_len: f32) -> Vec<Vec<[f32; 2]>> {
    let total = crate::match_frame::polyline_len(line);
    if line.len() < 2 || parts <= 1 || total < 2.0 * min_len {
        return vec![line.to_vec()];
    }
    let parts = parts.min((total / min_len) as usize).max(1);
    let piece = total / parts as f32;
    let mut cuts = (1..parts).map(|k| k as f32 * piece).peekable();
    let mut out = Vec::with_capacity(parts);
    let mut cur = vec![line[0]];
    let mut acc = 0.0;
    for w in line.windows(2) {
        let (a, b) = (w[0], w[1]);
        let seg = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
        while let Some(&c) = cuts.peek() {
            if seg <= 0.0 || c > acc + seg {
                break;
            }
            let t = ((c - acc) / seg).clamp(0.0, 1.0);
            let p = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
            if cur.last() != Some(&p) {
                cur.push(p);
            }
            out.push(std::mem::replace(&mut cur, vec![p]));
            cuts.next();
        }
        acc += seg;
        if cur.last() != Some(&b) {
            cur.push(b);
        }
    }
    out.push(cur);
    out
}

/// Caps a polyline to at most [`MAX_DIG_POINTS`] points (keeps the ends).
pub fn cap_points(line: &[[f32; 2]]) -> Vec<[f32; 2]> {
    if line.len() <= MAX_DIG_POINTS {
        return line.to_vec();
    }
    let n = MAX_DIG_POINTS;
    (0..n)
        .map(|k| line[k * (line.len() - 1) / (n - 1)])
        .collect()
}

/// A Dig command for `squad` along `line` (sim xy).
pub fn dig_command(squad: u32, line: &[[f32; 2]]) -> CommandKind {
    CommandKind::Dig {
        squad: SquadId(squad),
        line: cap_points(line).into_iter().map(sim_to_fx).collect(),
        profile: Default::default(),
    }
}

/// The soldier set of a `*_units`, `set_stance` or `throw` intent: ids that
/// pass `own` (the local player's living men), ascending, without
/// duplicates, at most [`MAX_UNITS`]. Empty means the intent is dropped.
pub fn unit_set(raw: &[i32], own: impl Fn(u32) -> bool) -> Vec<SimId> {
    let mut ids: Vec<u32> = raw
        .iter()
        .filter(|&&i| i >= 0)
        .map(|&i| i as u32)
        .filter(|&i| own(i))
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids.truncate(MAX_UNITS);
    ids.into_iter().map(SimId).collect()
}

/// `set_stance` names (`match_state.md`).
pub fn stance_order(name: &str) -> Option<StanceOrder> {
    Some(match name {
        "auto" => StanceOrder::Auto,
        "stand" => StanceOrder::Stand,
        "crouch" => StanceOrder::Crouch,
        "prone" => StanceOrder::Prone,
        _ => return None,
    })
}

pub fn stance_order_name(s: u8) -> &'static str {
    match s {
        1 => "stand",
        2 => "crouch",
        3 => "prone",
        _ => "auto",
    }
}

/// Throw intent item names.
pub fn item_kind(name: &str) -> Option<ItemKind> {
    Some(match name {
        "frag" => ItemKind::Frag,
        "smoke" => ItemKind::Smoke,
        "satchel" => ItemKind::Satchel,
        _ => return None,
    })
}

pub fn item_name(k: ItemKind) -> &'static str {
    match k {
        ItemKind::Frag => "frag",
        ItemKind::Smoke => "smoke",
        ItemKind::Satchel => "satchel",
    }
}

/// The Command for a soldier-level intent `kind` on `units` (already
/// filtered by [`unit_set`], non-empty). `target` and `line` are sim xy;
/// they are quantised here, once. `None` when the intent lacks what it
/// needs (a target, a line of two points, a known stance or item).
pub fn unit_command(
    kind: &str,
    units: Vec<SimId>,
    target: Option<[f32; 2]>,
    line: &[[f32; 2]],
    word: &str,
) -> Option<CommandKind> {
    let t = || target.map(sim_to_fx);
    Some(match kind {
        "move_units" => CommandKind::MoveUnits {
            units,
            target: t()?,
        },
        "attack_move_units" => CommandKind::AttackMoveUnits {
            units,
            target: t()?,
        },
        "stop_units" => CommandKind::StopUnits { units },
        "retreat_units" => CommandKind::RetreatUnits { units },
        "dig_units" if line.len() >= 2 => CommandKind::DigUnits {
            units,
            line: cap_points(line).into_iter().map(sim_to_fx).collect(),
            profile: Default::default(),
        },
        "set_stance" => CommandKind::SetStance {
            units,
            stance: stance_order(word)?,
        },
        "throw" => CommandKind::Throw {
            units,
            item: item_kind(word)?,
            target: t()?,
        },
        "throw_each" => CommandKind::ThrowEach {
            units,
            item: item_kind(word)?,
            target: t()?,
        },
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::match_frame::polyline_len;

    #[test]
    fn godot_points_become_sim_fixed_point() {
        let p = godot_to_fx(-412.0, 384.25, 512.0);
        assert_eq!(p.to_f32(), [100.0, 127.75]);
        assert_eq!(sim_to_fx([1.0006, -2.0]).x, Fx::from_milli(1001));
    }

    #[test]
    fn group_targets_spread_across_the_approach() {
        let anchors = [[0.0, 0.0], [10.0, 0.0], [5.0, -3.0]];
        let t = spread_targets(&anchors, [5.0, 100.0], 18.0);
        assert_eq!(t.len(), 3);
        // Moving north: spread along x, left-most squad keeps the left slot.
        let mut xs: Vec<f32> = t.iter().map(|p| p[0]).collect();
        xs.sort_by(f32::total_cmp);
        assert!((xs[2] - xs[0] - 36.0).abs() < 0.5, "{xs:?}");
        assert!(t[0][0] < t[1][0]);
        assert_eq!(
            spread_targets(&anchors[..1], [5.0, 100.0], 18.0),
            vec![[5.0, 100.0]]
        );
    }

    #[test]
    fn dig_lines_split_evenly() {
        let line = [[0.0, 0.0], [10.0, 0.0], [10.0, 20.0]];
        let parts = split_polyline(&line, 3, 4.0);
        assert_eq!(parts.len(), 3);
        for p in &parts {
            assert!((polyline_len(p) - 10.0).abs() < 0.01, "{p:?}");
        }
        assert_eq!(parts[0].first(), Some(&[0.0, 0.0]));
        assert_eq!(parts[2].last(), Some(&[10.0, 20.0]));
        // Consecutive pieces share their joint.
        assert_eq!(parts[0].last(), parts[1].first());
        // Too short to split.
        assert_eq!(split_polyline(&[[0.0, 0.0], [6.0, 0.0]], 3, 4.0).len(), 1);
        let two = split_polyline(&[[0.0, 0.0], [10.0, 0.0]], 4, 4.0);
        assert_eq!(two.len(), 2);
        assert!((polyline_len(&two[1]) - 5.0).abs() < 0.01);
    }

    #[test]
    fn unit_sets_are_own_sorted_unique_and_capped() {
        let own = |i: u32| i.is_multiple_of(2);
        let ids = unit_set(&[8, 2, -1, 3, 2, 6], own);
        assert_eq!(ids, vec![SimId(2), SimId(6), SimId(8)]);
        assert!(unit_set(&[1, 3], own).is_empty());
        let many: Vec<i32> = (0..400).collect();
        let capped = unit_set(&many, |_| true);
        assert_eq!(capped.len(), MAX_UNITS);
        assert_eq!(capped.last(), Some(&SimId(MAX_UNITS as u32 - 1)));
    }

    #[test]
    fn unit_intents_become_commands() {
        let u = || vec![SimId(4), SimId(9)];
        let t = Some([100.0006, 20.0]);
        match unit_command("move_units", u(), t, &[], "") {
            Some(CommandKind::MoveUnits { units, target }) => {
                assert_eq!(units, u());
                assert_eq!(target, sim_to_fx([100.0006, 20.0]));
                assert_eq!(target.x, Fx::from_milli(100_001));
            }
            c => panic!("{c:?}"),
        }
        assert!(matches!(
            unit_command("attack_move_units", u(), t, &[], ""),
            Some(CommandKind::AttackMoveUnits { .. })
        ));
        assert!(unit_command("move_units", u(), None, &[], "").is_none());
        assert!(matches!(
            unit_command("stop_units", u(), None, &[], ""),
            Some(CommandKind::StopUnits { .. })
        ));
        assert!(matches!(
            unit_command("retreat_units", u(), None, &[], ""),
            Some(CommandKind::RetreatUnits { .. })
        ));
        let line: Vec<[f32; 2]> = (0..40).map(|i| [i as f32, 0.0]).collect();
        match unit_command("dig_units", u(), None, &line, "") {
            Some(CommandKind::DigUnits { line, .. }) => assert_eq!(line.len(), MAX_DIG_POINTS),
            c => panic!("{c:?}"),
        }
        assert!(unit_command("dig_units", u(), None, &line[..1], "").is_none());
        assert!(matches!(
            unit_command("set_stance", u(), None, &[], "prone"),
            Some(CommandKind::SetStance {
                stance: StanceOrder::Prone,
                ..
            })
        ));
        assert!(unit_command("set_stance", u(), None, &[], "lie").is_none());
        assert!(matches!(
            unit_command("throw", u(), t, &[], "smoke"),
            Some(CommandKind::Throw {
                item: ItemKind::Smoke,
                ..
            })
        ));
        assert!(matches!(
            unit_command("throw_each", u(), t, &[], "frag"),
            Some(CommandKind::ThrowEach {
                item: ItemKind::Frag,
                ..
            })
        ));
        assert!(unit_command("throw", u(), t, &[], "rock").is_none());
        for s in ["auto", "stand", "crouch", "prone"] {
            assert_eq!(stance_order_name(stance_order(s).unwrap() as u8), s);
        }
        for k in ItemKind::ALL {
            assert_eq!(item_kind(item_name(k)), Some(k));
        }
    }

    #[test]
    fn long_polylines_are_capped() {
        let line: Vec<[f32; 2]> = (0..40).map(|i| [i as f32, 0.0]).collect();
        let c = cap_points(&line);
        assert_eq!(c.len(), MAX_DIG_POINTS);
        assert_eq!(c[0], [0.0, 0.0]);
        assert_eq!(c[MAX_DIG_POINTS - 1], [39.0, 0.0]);
    }
}
