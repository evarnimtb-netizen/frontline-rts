//! Grid A* on the heightfield's 4 m grid (§5.5, S1 "Pathfinding").
//!
//! * Nodes are heightfield grid points, 8-connected. A step costs its length
//!   (1000 straight, 1414 diagonal) times `1 + grade · slope_cost_pct`; steps
//!   steeper than `max_slope_pct` are impassable, and a diagonal step needs
//!   both straight steps beside it passable. A step between two trench
//!   nodes (`Heightfield::is_trench`) costs `trench_cost_pct` of that.
//!   Grades are measured on the base tier: trench walls are sub-grid detail
//!   and never block a path (the Movement stage slows men on steep detail
//!   ground instead).
//! * Trench lines: a search that starts on a trench node with its goal
//!   within [`TRENCH_SEARCH_NODES`] scales its heuristic by
//!   `trench_cost_pct` too, so it really finds routes along the trench
//!   network ("cheaper for infantry already in a trench line"). Other
//!   searches keep the fast heuristic and the discount only nudges them
//!   locally. Smoothing never cuts a corner off an all-trench stretch.
//!   The heuristic is the octile
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
//! * Vehicles (S3, [`find_vehicle_path`]) search the same grid with their own
//!   costs: slope as infantry but impassable above the vehicle's
//!   `max_slope_pct`; woods ×2; ground dug 0.4–1.0 m ×1.5 and 1.0 m or more
//!   ×4 (crossing rules apply when driving); vehicle-blocked nodes
//!   impassable (the dilated block layer, one bit per neighbour check); no
//!   trench discount; the block layer's clearance band ×4. A stuck re-path
//!   makes one `avoid` node 8× dearer.
//!   Blocked nodes next to the start are passable, so a vehicle standing at
//!   the edge of a blocked area can drive out. Smoothing never keeps a
//!   segment that enters the clearance band.
//!
//! * Infantry in a match with AT ditches (S3, [`find_infantry_path`]): a
//!   step into an AT-ditch node costs ×3 and gets no trench discount, so
//!   men cross a ditch but never use it as a covered road.
//!
//! The scratch arrays here are reset per search and never carry state across
//! ticks, so they are neither saved nor hashed.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::fx::{Fx, FxVec2};
use crate::terrain::{FLAG_WOODS, Heightfield, SPACING_M};
use crate::vehicle::BlockLayer;
use frontline_content::tuning::VehiclePathTuning;

const STRAIGHT: u32 = 1000;
const DIAGONAL: u32 = 1414;
/// Smoothing never skips more than this many grid nodes in one segment.
const SMOOTH_LOOKAHEAD: usize = 12;
/// Heuristic weight in percent (weighted A*): paths cost at most this much
/// more than optimal, for far fewer expansions on rolling terrain, where the
/// slope term makes the plain octile bound weak.
const H_WEIGHT_PCT: u32 = 130;
/// A search from a trench node uses the trench-scaled heuristic only when
/// its goal is at most this many grid steps away (100 m): local moves along
/// a trench network, not cross-map searches, which would expand far more.
const TRENCH_SEARCH_NODES: u32 = 25;
/// Grades up to this (percent) are always fine for a smoothed shortcut.
const SMOOTH_GENTLE_PCT: u32 = 10;

/// Slope rules for one search (from the match rules).
#[derive(Copy, Clone, Debug)]
pub struct PathCost {
    pub slope_cost_pct: u32,
    pub max_slope_pct: u32,
    /// Step cost between two trench nodes, in percent.
    pub trench_cost_pct: u32,
    /// Infantry step cost into an AT-ditch node, in percent (S3).
    pub ditch_cost_pct: u32,
}

/// Vehicle costs on top of [`PathCost`] (S3).
#[derive(Copy, Clone, Debug)]
pub struct VehiclePathCost<'a> {
    pub block: &'a BlockLayer,
    /// Node index made `avoid_pct` dearer.
    pub avoid: Option<u32>,
    /// Woods, dug-ground, avoid and margin costs (`[vehicles.path]`).
    pub costs: &'a VehiclePathTuning,
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

/// Octile distance times `weight_pct` / 100.
#[inline]
fn octile(ax: u32, ay: u32, bx: u32, by: u32, weight_pct: u32) -> u32 {
    let dx = ax.abs_diff(bx);
    let dy = ay.abs_diff(by);
    let (lo, hi) = if dx < dy { (dx, dy) } else { (dy, dx) };
    ((STRAIGHT * hi + (DIAGONAL - STRAIGHT) * lo) as u64 * weight_pct as u64 / 100) as u32
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
    search(terrain, from, to, cost, None, None, max_expansions, s)
}

/// [`find_path`] for infantry in a match with AT ditches (S3): a step into
/// an AT-ditch node ([`BlockLayer::is_ditch`]) costs `cost.ditch_cost_pct`
/// and never gets the trench discount.
pub fn find_infantry_path(
    terrain: &Heightfield,
    from: FxVec2,
    to: FxVec2,
    cost: &PathCost,
    ditches: &BlockLayer,
    max_expansions: u32,
    s: &mut PathScratch,
) -> PathResult {
    search(
        terrain,
        from,
        to,
        cost,
        None,
        Some(ditches),
        max_expansions,
        s,
    )
}

/// Finds a vehicle path (module docs): `cost.max_slope_pct` is the
/// vehicle's, `cost.trench_cost_pct` is ignored.
pub fn find_vehicle_path(
    terrain: &Heightfield,
    from: FxVec2,
    to: FxVec2,
    cost: &PathCost,
    vehicle: &VehiclePathCost,
    max_expansions: u32,
    s: &mut PathScratch,
) -> PathResult {
    search(
        terrain,
        from,
        to,
        cost,
        Some(vehicle),
        None,
        max_expansions,
        s,
    )
}

/// Vehicle step-cost multiplier (percent) into node `(mx, my)`, or `None`
/// if it is blocked.
#[inline]
fn vehicle_pct(
    terrain: &Heightfield,
    v: &VehiclePathCost,
    mx: u32,
    my: u32,
    near_start: bool,
) -> Option<u64> {
    let dim = terrain.dim();
    let m = my * dim + mx;
    if v.block.is_blocked(m) && !near_start {
        return None;
    }
    let k = v.costs;
    let mut pct = 100u64;
    let flags = terrain.flags();
    if !flags.is_empty() && flags[m as usize] & FLAG_WOODS != 0 {
        pct = pct * k.woods_pct as u64 / 100;
    }
    let dug = terrain.dug_dm(mx, my) as u32;
    if dug >= k.deep_dm {
        pct = pct * k.deep_pct as u64 / 100;
    } else if dug >= k.dug_dm {
        pct = pct * k.dug_pct as u64 / 100;
    }
    if v.avoid == Some(m) {
        pct = pct * k.avoid_pct as u64 / 100;
    }
    if v.block.is_margin(m) {
        pct = pct * k.margin_pct as u64 / 100;
    }
    Some(pct)
}

#[allow(clippy::too_many_arguments)]
fn search(
    terrain: &Heightfield,
    from: FxVec2,
    to: FxVec2,
    cost: &PathCost,
    vehicle: Option<&VehiclePathCost>,
    ditches: Option<&BlockLayer>,
    max_expansions: u32,
    s: &mut PathScratch,
) -> PathResult {
    let ditch = |n: u32| ditches.is_some_and(|d| d.is_ditch(n));
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
    let near = sx.abs_diff(tx).max(sy.abs_diff(ty)) <= TRENCH_SEARCH_NODES;
    let weight = if vehicle.is_none() && near && terrain.is_trench(sx, sy) && !ditch(start) {
        H_WEIGHT_PCT * cost.trench_cost_pct / 100
    } else {
        H_WEIGHT_PCT
    };
    let h0 = octile(sx, sy, tx, ty, weight);
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
        let n_trench = terrain.is_trench(nx, ny);
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
            // Vehicles: blocked nodes are walls, except right at the start.
            let near_start = |x: u32, y: u32| x.abs_diff(sx) <= 1 && y.abs_diff(sy) <= 1;
            let vpct = match vehicle {
                None => None,
                Some(v) => match vehicle_pct(terrain, v, mx, my, near_start(mx, my)) {
                    None => continue,
                    some => some,
                },
            };
            // No corner cutting: a diagonal step needs both straight steps
            // around it passable (a wall's corner is not a gap).
            if diagonal
                && (step_cost(hn, terrain.grid_height(mx, ny), STRAIGHT, cost).is_none()
                    || step_cost(hn, terrain.grid_height(nx, my), STRAIGHT, cost).is_none()
                    || vehicle.is_some_and(|v| {
                        (v.block.is_blocked(ny * dim + mx) && !near_start(mx, ny))
                            || (v.block.is_blocked(my * dim + nx) && !near_start(nx, my))
                    }))
            {
                continue;
            }
            let base = if diagonal { DIAGONAL } else { STRAIGHT };
            let Some(mut c) = step_cost(hn, terrain.grid_height(mx, my), base, cost) else {
                continue;
            };
            if let Some(pct) = vpct {
                c = (c as u64 * pct / 100).min(u32::MAX as u64 / 4) as u32;
            } else if ditch(m) {
                // An AT ditch: crossable, dear, never a covered road.
                c = (c as u64 * cost.ditch_cost_pct as u64 / 100).min(u32::MAX as u64 / 4) as u32;
            } else if n_trench && terrain.is_trench(mx, my) && !ditch(n) {
                c = (c as u64 * cost.trench_cost_pct as u64 / 100) as u32;
            }
            // Saturating: extreme slope rules on a large map could exceed
            // u32 (the sim panics on overflow).
            let g = g_n.saturating_add(c);
            if s.seen[mi] == stamp && s.g[mi] <= g {
                continue;
            }
            s.seen[mi] = stamp;
            s.g[mi] = g;
            s.parent[mi] = n;
            let h = octile(mx, my, tx, ty, weight);
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
    let points = smooth(terrain, terrain.clamp(from), &grid, cost, vehicle);
    PathResult {
        points,
        expansions,
        reached,
        exhausted,
    }
}

/// Greatest grade along the straight segment `a → b`, sampled every 2 m on
/// the base tier (like the grid search).
fn segment_grade(terrain: &Heightfield, a: FxVec2, b: FxVec2) -> u32 {
    let d = b - a;
    let len = d.length();
    let steps = (len.raw() >> (Fx::FRAC_BITS + 1)).max(1);
    // Units per sample step: len/steps metres, 1000 units per 4 m.
    let units = ((len.raw() as i64 * 250 / steps as i64) >> Fx::FRAC_BITS).max(1) as u32;
    let mut prev = terrain.base_height_at(a);
    let mut worst = 0;
    for k in 1..=steps {
        let p = a + FxVec2::new(d.x.mul_int(k).div_int(steps), d.y.mul_int(k).div_int(steps));
        let h = terrain.base_height_at(p);
        worst = worst.max(grade_pct(prev, h, units));
        prev = h;
    }
    worst
}

/// Whether every 2 m sample of the straight segment `a → b` (end points
/// included) is nearest to a trench node.
fn segment_in_trench(terrain: &Heightfield, a: FxVec2, b: FxVec2) -> bool {
    let d = b - a;
    let steps = (d.length().raw() >> (Fx::FRAC_BITS + 1)).max(1);
    (0..=steps).all(|k| {
        let p = a + FxVec2::new(d.x.mul_int(k).div_int(steps), d.y.mul_int(k).div_int(steps));
        let (gx, gy) = terrain.nearest_grid(p);
        terrain.is_trench(gx, gy)
    })
}

/// String-pulls a grid path: from each anchor, jump to the furthest of the
/// next [`SMOOTH_LOOKAHEAD`] nodes whose straight segment is no steeper than
/// the grid path it replaces (or gentle anyway), and stays in the trench
/// network if the grid path it replaces does.
fn smooth(
    terrain: &Heightfield,
    from: FxVec2,
    grid: &[FxVec2],
    cost: &PathCost,
    vehicle: Option<&VehiclePathCost>,
) -> Vec<FxVec2> {
    // Vehicles: the dearest node a segment touches (2 m samples), `None`
    // for a blocked one. A shortcut may not touch dearer ground than the
    // grid nodes it replaces, nor the clearance band.
    let dear = |v: &VehiclePathCost, a: FxVec2, b: FxVec2| -> Option<u64> {
        let d = b - a;
        let steps = (d.length().raw() >> (Fx::FRAC_BITS + 1)).max(1);
        let mut worst = 0u64;
        for k in 0..=steps {
            let p = a + FxVec2::new(d.x.mul_int(k).div_int(steps), d.y.mul_int(k).div_int(steps));
            let (gx, gy) = terrain.nearest_grid(p);
            worst = worst.max(vehicle_pct(terrain, v, gx, gy, false)?);
        }
        Some(worst)
    };
    let mut out = Vec::new();
    let mut anchor = from;
    let mut i = 0usize; // next grid node not yet passed
    while i < grid.len() {
        let mut best = i;
        let mut path_worst = 0u32;
        let mut prev = anchor;
        let at = |p: FxVec2| {
            let (gx, gy) = terrain.nearest_grid(p);
            terrain.is_trench(gx, gy)
        };
        let mut all_trench = vehicle.is_none() && at(anchor);
        let mut path_dear = 0u64;
        let hi = (i + SMOOTH_LOOKAHEAD).min(grid.len() - 1);
        for (j, &node) in grid.iter().enumerate().take(hi + 1).skip(i) {
            path_worst = path_worst.max(segment_grade(terrain, prev, node));
            all_trench &= at(node);
            if let Some(v) = vehicle {
                let (gx, gy) = terrain.nearest_grid(node);
                path_dear = path_dear.max(vehicle_pct(terrain, v, gx, gy, true).unwrap_or(0));
            }
            prev = node;
            if j == i {
                continue;
            }
            let allowed = path_worst.max(SMOOTH_GENTLE_PCT).min(cost.max_slope_pct);
            if segment_grade(terrain, anchor, node) <= allowed
                && (!all_trench || segment_in_trench(terrain, anchor, node))
                && vehicle.is_none_or(|v| {
                    !v.block.segment_in_margin(terrain, anchor, node)
                        && dear(v, anchor, node).is_some_and(|d| d <= path_dear.max(100))
                })
            {
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
        trench_cost_pct: 60,
        ditch_cost_pct: 300,
    };

    /// The `[vehicles.path]` values of mods/ww2.
    const VCOSTS: VehiclePathTuning = VehiclePathTuning {
        woods_pct: 200,
        dug_pct: 150,
        dug_dm: 4,
        deep_pct: 400,
        deep_dm: 10,
        avoid_pct: 800,
        margin_pct: 400,
        ditch_reach_m: 2,
        wreck_pad_m: 2,
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
    fn trench_lines_are_followed_when_cheaper() {
        // An L-shaped trench (20, 20) → (100, 20) → (100, 100); the direct
        // diagonal is shorter, but at 20 % step cost the trench wins.
        let mut t = flat(128);
        let dig = |t: &mut Heightfield, (x0, y0): (i32, i32), (x1, y1): (i32, i32)| {
            t.edit(
                (x0 * 4 - 2, y0 * 4 - 2),
                (x1 * 4 + 2, y1 * 4 + 2),
                1,
                |_, _, b, _| b - Fx::from_milli(1500),
            );
        };
        dig(&mut t, (20, 20), (100, 20));
        dig(&mut t, (100, 20), (100, 100));
        assert!(t.is_trench(15, 5) && t.is_trench(25, 15) && !t.is_trench(15, 15));
        let mut s = PathScratch::default();
        let (from, to) = (FxVec2::from_int(20, 20), FxVec2::from_int(100, 100));
        let cheap = PathCost {
            trench_cost_pct: 20,
            ..COST
        };
        let r = find_path(&t, from, to, &cheap, 100_000, &mut s);
        assert!(r.reached);
        let corner = FxVec2::from_int(100, 20);
        assert!(
            r.points
                .iter()
                .any(|p| (*p - corner).length() < Fx::from_int(6)),
            "{:?}",
            r.points
        );
        // Without a discount the diagonal is the better route.
        let full = PathCost {
            trench_cost_pct: 100,
            ..COST
        };
        let r = find_path(&t, from, to, &full, 100_000, &mut s);
        assert!(
            r.points
                .iter()
                .all(|p| (*p - corner).length() > Fx::from_int(20))
        );
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

    // ---------------------------------------------------------- vehicles --

    fn layer(t: &Heightfield, walls: &[crate::vehicle::Footprint]) -> BlockLayer {
        let mut b = BlockLayer::new(t.dim(), 19);
        b.mark_all();
        b.update(t, &[], walls, &VCOSTS);
        b
    }

    fn passes_near(points: &[FxVec2], p: FxVec2, m: i32) -> bool {
        let mut prev = points[0];
        for &q in points {
            // Distance from p to the segment prev → q (coarse: 1 m steps).
            let d = q - prev;
            let n = (d.length().raw() >> Fx::FRAC_BITS).max(1);
            for k in 0..=n {
                let s = prev + FxVec2::new(d.x.mul_int(k).div_int(n), d.y.mul_int(k).div_int(n));
                if (s - p).length() < Fx::from_int(m) {
                    return true;
                }
            }
            prev = q;
        }
        false
    }

    #[test]
    fn vehicle_paths_go_round_blocked_nodes_with_clearance() {
        let t = flat(256);
        // A wall across x = 128 from y = 0 to 180 (a long wreck row).
        let wall = crate::vehicle::Footprint {
            center: FxVec2::from_int(128, 90),
            fwd: FxVec2::new(Fx::ZERO, Fx::ONE),
            half_x: Fx::from_int(88),
            half_y: Fx::ONE,
        };
        let b = layer(&t, &[wall]);
        let v = VehiclePathCost {
            block: &b,
            avoid: None,
            costs: &VCOSTS,
        };
        let mut s = PathScratch::default();
        let (from, to) = (FxVec2::from_int(60, 60), FxVec2::from_int(200, 60));
        let r = find_vehicle_path(&t, from, to, &COST, &v, 100_000, &mut s);
        assert!(r.reached);
        let mut prev = from;
        for p in &r.points {
            assert!(!b.segment_blocked(&t, prev, *p), "{prev:?} → {p:?}");
            prev = *p;
        }
        // Round the wall's end (y ≈ 182 + padding), not through it.
        assert!(
            r.points.iter().any(|p| p.y > Fx::from_int(186)),
            "{:?}",
            r.points
        );
        // Infantry walk straight through (they climb over wrecks... and
        // know nothing of the layer).
        let r = find_path(&t, from, to, &COST, 100_000, &mut s);
        assert!(r.points.iter().all(|p| p.y < Fx::from_int(100)));
    }

    #[test]
    fn vehicle_paths_avoid_dug_ground_and_the_avoided_node() {
        // A 1.5 m deep cut across the straight line, 24 m long.
        let mut t = flat(256);
        t.edit((100 * 4, 60 * 4), (104 * 4, 84 * 4), 1, |_, _, b, _| {
            b - Fx::from_milli(1500)
        });
        let b = layer(&t, &[]);
        let v = VehiclePathCost {
            block: &b,
            avoid: None,
            costs: &VCOSTS,
        };
        let mut s = PathScratch::default();
        let (from, to) = (FxVec2::from_int(60, 72), FxVec2::from_int(150, 72));
        let r = find_vehicle_path(&t, from, to, &COST, &v, 100_000, &mut s);
        assert!(r.reached);
        assert!(
            !passes_near(
                &[&[from][..], &r.points].concat(),
                FxVec2::from_int(102, 72),
                6
            ),
            "round the ×4 ground: {:?}",
            r.points
        );
        // Infantry cross it.
        let r = find_path(&t, from, to, &COST, 100_000, &mut s);
        assert!(passes_near(
            &[&[from][..], &r.points].concat(),
            FxVec2::from_int(102, 72),
            3
        ));
        // Flat: the straight line, unless its middle node is avoided.
        let t = flat(256);
        let b = layer(&t, &[]);
        let mid = FxVec2::from_int(100, 72);
        let (gx, gy) = t.nearest_grid(mid);
        let avoided = VehiclePathCost {
            block: &b,
            avoid: Some(gy * t.dim() + gx),
            costs: &VCOSTS,
        };
        let r = find_vehicle_path(&t, from, to, &COST, &avoided, 100_000, &mut s);
        let path = [&[from][..], &r.points].concat();
        assert!(!passes_near(&path, mid, 1), "{:?}", r.points);
        let r = find_vehicle_path(&t, from, to, &COST, &v, 100_000, &mut s);
        assert!(passes_near(&[&[from][..], &r.points].concat(), mid, 1));
    }

    #[test]
    fn infantry_cross_an_at_ditch_but_never_walk_along_it() {
        use frontline_content::Fixed;
        use frontline_content::setup::DigSetup;
        let ditch = DigSetup {
            depth: Fixed::from_cm(230),
            floor_width: Fixed::from_cm(100),
            wall_run: Fixed::from_cm(120),
            parapet_height: Fixed::from_cm(60),
            max_length: Fixed::from_int(30),
            work_per_m: 22_000,
            passes: 10,
            engineers_only: true,
            common: frontline_content::tuning::DigCommonTuning {
                parapet_rise_mm: 300,
                parapet_top_mm: 400,
                parapet_fall_mm: 600,
                parapet_undug_mm: 50,
                reach_mm: 1500,
            },
        };
        // The same 60 m line along y = 128, as a ditch and as a trench.
        let line = [FxVec2::from_int(100, 128), FxVec2::from_int(160, 128)];
        let mut t = flat(256);
        crate::dig::stamp(
            &mut t,
            &line,
            1,
            Fx::ZERO,
            Fx::from_int(60),
            10,
            10,
            &ditch,
            0,
        );
        let mut b = BlockLayer::new(t.dim(), 19);
        b.mark_all();
        b.update(&t, &[line.to_vec()], &[], &VCOSTS);
        let mid = crate::vehicle::node_at(&t, FxVec2::from_int(130, 128));
        assert!(b.is_ditch(mid) && b.is_blocked(mid));
        let mut s = PathScratch::default();
        // Along the line, from one end of it to the other: the trench
        // discount does not apply in a ditch, so the path costs at least
        // the open ground's.
        let (a, z) = (FxVec2::from_int(104, 128), FxVec2::from_int(156, 128));
        let cheap = PathCost {
            trench_cost_pct: 20,
            ..COST
        };
        let open = find_path(&flat(256), a, z, &cheap, 100_000, &mut s);
        let along = find_infantry_path(&t, a, z, &cheap, &b, 100_000, &mut s);
        let in_ditch = |r: &PathResult| {
            r.points
                .iter()
                .filter(|p| b.is_ditch(crate::vehicle::node_at(&t, **p)))
                .count()
        };
        assert!(along.reached);
        assert!(in_ditch(&along) <= 1, "{:?}", along.points);
        assert!(open.reached);
        // Across it: reached, through it.
        let r = find_infantry_path(
            &t,
            FxVec2::from_int(130, 110),
            FxVec2::from_int(130, 146),
            &cheap,
            &b,
            100_000,
            &mut s,
        );
        assert!(r.reached);
        // The plain search (no ditch layer) treats it as a trench.
        let plain = find_path(&t, a, z, &cheap, 100_000, &mut s);
        // The plain search (no ditch layer) takes it as a trench: straight
        // along it; the infantry search walks beside it and steps in only
        // at the goal.
        assert_eq!(plain.points.len(), 1, "{:?}", plain.points);
        assert!(along.points.len() >= 2, "{:?}", along.points);
        assert!(!b.is_ditch(crate::vehicle::node_at(&t, along.points[0])));
    }
}
