//! Player intents (godot/docs/match_state.md) → sim [`CommandKind`]s.
//! Plain Rust: Godot-space floats in, fixed-point commands out.
//!
//! This is the one place presenter floats become sim input. Positions are
//! quantised to millimetres once, here, and the resulting commands are what
//! the sim (and a replay or a lockstep peer) sees, so the conversion never
//! has to be bit-identical across machines.

use frontline_content::{MatchSetup, SquadRole};
use frontline_sim::game::MAX_DIG_POINTS;
use frontline_sim::{CommandKind, Fx, FxVec2, SquadId};

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

/// Role name used by the presenter (`rifle`, `mg`, `engineer`, `mortar`).
pub fn role_name(r: SquadRole) -> &'static str {
    match r {
        SquadRole::Rifle => "rifle",
        SquadRole::Mg => "mg",
        SquadRole::Engineer => "engineer",
        SquadRole::Mortar => "mortar",
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
    }
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
    fn long_polylines_are_capped() {
        let line: Vec<[f32; 2]> = (0..40).map(|i| [i as f32, 0.0]).collect();
        let c = cap_points(&line);
        assert_eq!(c.len(), MAX_DIG_POINTS);
        assert_eq!(c[0], [0.0, 0.0]);
        assert_eq!(c[MAX_DIG_POINTS - 1], [39.0, 0.0]);
    }
}
