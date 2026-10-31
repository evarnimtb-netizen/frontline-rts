//! Grid A* on the heightfield's 4 m grid (§5.5, S1 "Pathfinding").
//!
//! * Nodes are heightfield grid points, 8-connected. A step costs its length
//!   (1000 straight, 1414 diagonal) times `1 + grade · slope_cost_pct`; steps
//!   steeper than `max_slope_pct` are impassable, and a diagonal step needs
//!   both straight steps beside it passable. The heuristic is the octile
//!   distance weighted by 1.3 (weighted A*): paths are within 30 % of the
//!   cheapest and searches expand far fewer nodes on hilly ground.
//! * The open list is a binary heap keyed by `(f, h, node)`: a total order,
//!   so expansion order never depends on anything but the inputs.
//! * A search runs to completion inside one tick but is capped at
//!   `max_expansions`. When the cap is hit (or the goal is unreachable) the
//!   path leads to the expanded node closest to the goal and is marked
//!   partial; the squad asks again when it gets there.
//! * The per-tick budget lives in [`crate::game::PathQueue`]: a search that
//!   overruns the budget leaves a debt that delays the next ones, so the
//!   amortised cost per tick stays at the budget.
//!
//! The scratch arrays here are reset per search and never carry state across
//! ticks, so they are neither saved nor hashed.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::fx::{Fx, FxVec2};
use crate::terrain::{Heightfield, SPACING_M};

const STRAIGHT: u32 = 1000;
const DIAGONAL: u32 = 1414;
/// Smoothing never skips more than this many grid nodes in one segment.
const SMOOTH_LOOKAHEAD: usize = 12;
/// Heuristic weight in percent (weighted A*): paths cost at most this much
/// more than optimal, for far fewer expansions on rolling terrain, where the
/// slope term makes the plain octile bound weak.
const H_WEIGHT_PCT: u32 = 130;
/// Grades up to this (percent) are always fine for a smoothed shortcut.
const SMOOTH_GENTLE_PCT: u32 = 10;

/// Slope rules for one search (from the match rules).
#[derive(Copy, Clone, Debug)]
pub struct PathCost {
    pub slope_cost_pct: u32,
    pub max_slope_pct: u32,
}

/// Result of [`find_path`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PathResult {
    /// Smoothed waypoints after the start; the last is the goal (or the
    /// closest reachable point when `reached` is false). Never empty.
    pub points: Vec<FxVec2>,
    /// Nodes expanded (charged against the tick budget).
    pub expansions: u32,
    pub reached: bool,
    /// The open list ran dry: no search from here will ever get closer.
    pub exhausted: bool,
}

/// Reusable search arrays, sized to the grid on first use.
#[derive(Clone, Debug, Default)]
pub struct PathScratch {
    g: Vec<u32>,
    parent: Vec<u32>,
    /// `seen[n] == stamp`: `g`/`parent` are valid for this search.
    seen: Vec<u32>,
    /// `closed[n] == stamp`: expanded in this search.
    closed: Vec<u32>,
    stamp: u32,
    heap: BinaryHeap<Reverse<(u32, u32, u32)>>,
    nodes: Vec<u32>,
}

impl PathScratch {
    fn begin(&mut self, n: usize) {
        if self.g.len() != n {
            self.g = vec![0; n];
            self.parent = vec![0; n];
            self.seen = vec![0; n];
            self.closed = vec![0; n];
            self.stamp = 0;
        }
        self.stamp = self.stamp.wrapping_add(1);
        if self.stamp == 0 {
            // Wrapped: old stamps could alias, so clear once.
            self.seen.fill(0);
            self.closed.fill(0);
            self.stamp = 1;
        }
        self.heap.clear();
        self.nodes.clear();
    }
}

/// Grade between two heights `dist_units` apart (1000 = 4 m), in percent.
#[inline]
fn grade_pct(h0: Fx, h1: Fx, dist_units: u32) -> u32 {
    let dh = (h1.raw() as i64 - h0.raw() as i64).unsigned_abs();
    // dh / (4 m · units/1000) · 100, with heights in Q16.16.
    let run = (SPACING_M as u64) << Fx::FRAC_BITS;
    (dh * 100 * 1000 / (run * dist_units as u64)).min(u32::MAX as u64) as u32
}

/// Step cost, or `None` if impassable.
#[inline]
fn step_cost(h0: Fx, h1: Fx, base: u32, c: &PathCost) -> Option<u32> {
    let g = grade_pct(h0, h1, base);
    if g > c.max_slope_pct {
        return None;
    }
    let mult = 100 + (g as u64 * c.slope_cost_pct as u64 / 100);
    Some((base as u64 * mult / 100).min(u32::MAX as u64 / 4) as u32)
}

/// Octile distance times [`H_WEIGHT_PCT`].
#[inline]
fn octile(ax: u32, ay: u32, bx: u32, by: u32) -> u32 {
    let dx = ax.abs_diff(bx);
    let dy = ay.abs_diff(by);
    let (lo, hi) = if dx < dy { (dx, dy) } else { (dy, dx) };
    (STRAIGHT * hi + (DIAGONAL - STRAIGHT) * lo) * H_WEIGHT_PCT / 100
}

#[inline]
fn node_pos(dim: u32, n: u32) -> FxVec2 {
    FxVec2::from_int((n % dim) as i32 * SPACING_M, (n / dim) as i32 * SPACING_M)
}

/// Finds a path from `from` to `to` (both clamped to the map).
pub fn find_path(
    terrain: &Heightfield,
    from: FxVec2,
    to: FxVec2,
    cost: &PathCost,
    max_expansions: u32,
    s: &mut PathScratch,
) -> PathResult {
    let dim = terrain.dim();
    let to = terrain.clamp(to);
    let (sx, sy) = terrain.nearest_grid(from);
    let (tx, ty) = terrain.nearest_grid(to);
    let start = sy * dim + sx;
    let goal = ty * dim + tx;
    s.begin((dim * dim) as usize);

    let stamp = s.stamp;
    s.g[start as usize] = 0;
    s.parent[start as usize] = start;
    s.seen[start as usize] = stamp;
    let h0 = octile(sx, sy, tx, ty);
    s.heap.push(Reverse((h0, h0, start)));
    // Closest expanded node to the goal: (h, g, node).
    let mut best = (h0, 0u32, start);
    let mut expansions = 0u32;
    let mut reached = false;
    let mut exhausted = true;

    while let Some(Reverse((f, h, n))) = s.heap.pop() {
        let ni = n as usize;
        if s.closed[ni] == stamp || f != s.g[ni].saturating_add(h) {
            continue; // stale heap entry
        }
        s.closed[ni] = stamp;
        if (h, s.g[ni], n) < best {
            best = (h, s.g[ni], n);
        }
        if n == goal {
            reached = true;
            exhausted = false;
            break;
        }
        if expansions >= max_expansions {
            exhausted = false;
            break;
        }
        expansions += 1;
        let (nx, ny) = (n % dim, n / dim);
        let hn = terrain.grid_height(nx, ny);
        let g_n = s.g[ni];
        // Fixed neighbour order (irrelevant for the result, which the heap
        // key fully orders, but kept stable anyway).
        for (dx, dy) in [
            (1i32, 0i32),
            (-1, 0),
            (0, 1),
            (0, -1),
            (1, 1),
            (-1, 1),
            (1, -1),
            (-1, -1),
        ] {
            let (mx, my) = (nx as i32 + dx, ny as i32 + dy);
            if mx < 0 || my < 0 || mx >= dim as i32 || my >= dim as i32 {
                continue;
            }
            let (mx, my) = (mx as u32, my as u32);
            let m = my * dim + mx;
            let mi = m as usize;
            if s.closed[mi] == stamp {
                continue;
            }
            let diagonal = dx != 0 && dy != 0;
            // No corner cutting: a diagonal step needs both straight steps
            // around it passable (a wall's corner is not a gap).
            if diagonal
                && (step_cost(hn, terrain.grid_height(mx, ny), STRAIGHT, cost).is_none()
                    || step_cost(hn, terrain.grid_height(nx, my), STRAIGHT, cost).is_none())
            {
                continue;
            }
            let base = if diagonal { DIAGONAL } else { STRAIGHT };
            let Some(c) = step_cost(hn, terrain.grid_height(mx, my), base, cost) else {
                continue;
            };
            // Saturating: extreme slope rules on a large map could exceed
            // u32 (the sim panics on overflow).
            let g = g_n.saturating_add(c);
            if s.seen[mi] == stamp && s.g[mi] <= g {
                continue;
            }
            s.seen[mi] = stamp;
            s.g[mi] = g;
            s.parent[mi] = n;
            let h = octile(mx, my, tx, ty);
            s.heap.push(Reverse((g.saturating_add(h), h, m)));
        }
    }

    // Walk parents back from the end node.
    let end = if reached { goal } else { best.2 };
    let mut n = end;
    while n != start {
        s.nodes.push(n);
        n = s.parent[n as usize];
    }
    s.nodes.reverse();
    let mut grid: Vec<FxVec2> = s.nodes.iter().map(|&n| node_pos(dim, n)).collect();
    if reached {
        // End exactly on the requested point, not its grid node.
        match grid.last_mut() {
            Some(last) => *last = to,
            None => grid.push(to),
        }
    } else if grid.is_empty() {
        grid.push(terrain.clamp(from));
    }
    let points = smooth(terrain, terrain.clamp(from), &grid, cost);
    PathResult {
        points,
        expansions,
        reached,
        exhausted,
    }
}

/// Greatest grade along the straight segment `a → b`, sampled every 2 m.
fn segment_grade(terrain: &Heightfield, a: FxVec2, b: FxVec2) -> u32 {
    let d = b - a;
    let len = d.length();
    let steps = (len.raw() >> (Fx::FRAC_BITS + 1)).max(1);
    // Units per sample step: len/steps metres, 1000 units per 4 m.
    let units = ((len.raw() as i64 * 250 / steps as i64) >> Fx::FRAC_BITS).max(1) as u32;
    let mut prev = terrain.height_at(a);
    let mut worst = 0;
    for k in 1..=steps {
        let p = a + FxVec2::new(d.x.mul_int(k).div_int(steps), d.y.mul_int(k).div_int(steps));
        let h = terrain.height_at(p);
        worst = worst.max(grade_pct(prev, h, units));
        prev = h;
    }
    worst
}

/// String-pulls a grid path: from each anchor, jump to the furthest of the
/// next [`SMOOTH_LOOKAHEAD`] nodes whose straight segment is no steeper than
/// the grid path it replaces (or gentle anyway).
fn smooth(terrain: &Heightfield, from: FxVec2, grid: &[FxVec2], cost: &PathCost) -> Vec<FxVec2> {
    let mut out = Vec::new();
    let mut anchor = from;
    let mut i = 0usize; // next grid node not yet passed
    while i < grid.len() {
        let mut best = i;
        let mut path_worst = 0u32;
        let mut prev = anchor;
        let hi = (i + SMOOTH_LOOKAHEAD).min(grid.len() - 1);
        for (j, &node) in grid.iter().enumerate().take(hi + 1).skip(i) {
            path_worst = path_worst.max(segment_grade(terrain, prev, node));
            prev = node;
            if j == i {
                continue;
            }
            let allowed = path_worst.max(SMOOTH_GENTLE_PCT).min(cost.max_slope_pct);
            if segment_grade(terrain, anchor, node) <= allowed {
                best = j;
            }
        }
        out.push(grid[best]);
        anchor = grid[best];
        i = best + 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::TerrainKind;

    const COST: PathCost = PathCost {
        slope_cost_pct: 300,
        max_slope_pct: 80,
    };

    fn flat(size: i32) -> Heightfield {
        Heightfield::generate(TerrainKind::Flat, size, 1)
    }

    #[test]
    fn straight_line_on_flat_ground_smooths_to_one_segment() {
        let t = flat(256);
        let mut s = PathScratch::default();
        let r = find_path(
            &t,
            FxVec2::from_int(10, 10),
            FxVec2::from_int(200, 30),
            &COST,
            100_000,
            &mut s,
        );
        assert!(r.reached && !r.exhausted);
        assert_eq!(r.points.last(), Some(&FxVec2::from_int(200, 30)));
        // Grid path has ~48 nodes; on flat ground smoothing keeps every
        // 12th or so.
        assert!(r.points.len() <= 5, "{:?}", r.points);
    }

    #[test]
    fn wall_is_walked_around() {
        // A steep wall across x = 100 with a gap near y = 200.
        let mut t = flat(256);
        t.edit_heights_for_test(|gx, gy, h| {
            if gx == 25 && gy < 48 {
                *h = Fx::from_int(30);
            }
        });
        let mut s = PathScratch::default();
        let r = find_path(
            &t,
            FxVec2::from_int(60, 40),
            FxVec2::from_int(140, 40),
            &COST,
            100_000,
            &mut s,
        );
        assert!(r.reached);
        let max_y = r.points.iter().map(|p| p.y).max().unwrap();
        assert!(
            max_y >= Fx::from_int(190),
            "went through the wall: {:?}",
            r.points
        );
        // No waypoint pair straddles the wall below its top.
        let mut prev = FxVec2::from_int(60, 40);
        for p in &r.points {
            assert!(segment_grade(&t, prev, *p) <= COST.max_slope_pct);
            prev = *p;
        }
    }

    #[test]
    fn capped_search_returns_partial_path_toward_goal() {
        let t = flat(1024);
        let mut s = PathScratch::default();
        let from = FxVec2::from_int(8, 8);
        let to = FxVec2::from_int(1000, 1000);
        let r = find_path(&t, from, to, &COST, 50, &mut s);
        assert!(!r.reached && !r.exhausted);
        assert_eq!(r.expansions, 50);
        let end = *r.points.last().unwrap();
        assert!((to - end).length_sq_raw() < (to - from).length_sq_raw());
    }

    #[test]
    fn unreachable_goal_exhausts() {
        // Goal on a pillar: no passable step leads up.
        let mut t = flat(128);
        t.edit_heights_for_test(|gx, gy, h| {
            if (14..=18).contains(&gx) && (14..=18).contains(&gy) {
                *h = Fx::from_int(40);
            }
        });
        let mut s = PathScratch::default();
        let r = find_path(
            &t,
            FxVec2::from_int(8, 8),
            FxVec2::from_int(64, 64),
            &COST,
            100_000,
            &mut s,
        );
        assert!(!r.reached && r.exhausted);
    }

    #[test]
    fn repeat_searches_are_identical() {
        let t = Heightfield::generate(TerrainKind::Noise, 512, 9);
        let mut s = PathScratch::default();
        let run = |s: &mut PathScratch| {
            find_path(
                &t,
                FxVec2::from_int(20, 30),
                FxVec2::from_int(480, 470),
                &COST,
                100_000,
                s,
            )
        };
        let a = run(&mut s);
        let b = run(&mut s);
        let c = run(&mut PathScratch::default());
        assert!(a.reached);
        assert_eq!(a, b);
        assert_eq!(a, c);
    }
}
