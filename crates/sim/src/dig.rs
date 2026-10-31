//! Trench digging (S1 "Digging", §5.10.2 `Dig` + `Spoil`, §5.10.9 pacing).
//!
//! A Dig order carries a [`DigJob`]: the line split by arc length into one
//! [`DigSegment`] per man at order time. Each tick, serially in ascending
//! squad id (Intent stage, before steering):
//!
//! 1. diggers that left the squad, or whose segment is finished, are let go;
//! 2. men without a segment take the nearest open unfinished one (to their
//!    slot; ties by segment index), so survivors and early finishers pick
//!    up the rest of the line;
//! 3. each digger's slot is his segment's midpoint; once he stands within
//!    [`DIG_REACH_MM`] of it he adds his squad type's `dig_rate_pct` work
//!    points per tick (one man at 100 % does 100 work points per second;
//!    work is counted in points × tick rate so it stays integral);
//! 4. when a segment's work crosses the next of `passes` equal steps, the
//!    trench profile is stamped over that segment at the matching fraction
//!    of full depth, so the trench visibly deepens pass by pass.
//!
//! The profile (rules: depth, floor width, parapet height): a flat floor,
//! walls sloping out over [`WALL_RUN_MM`] to the lip, and a spoil parapet
//! just beyond the lip on the front side only (toward the nearest enemy when
//! the order was given). The ends are rounded. Cutting takes the minimum of
//! the current surface and the base minus the profile depth, so crossing
//! trenches merge; the parapet only raises undug ground. A new Dig order on
//! a partly dug line credits the depth already there.

use frontline_content::setup::DigSetup;
use serde::{Deserialize, Serialize};

use crate::ecs::{Entities, PlayerId, SimId};
use crate::fx::{Fx, FxVec2};
use crate::game::{MatchState, NavPhase, Order};
use crate::squad::{point_along, polyline_length};
use crate::terrain::{DETAIL_SHIFT, Heightfield};

/// Horizontal run of a trench wall, floor edge to lip (mm).
pub const WALL_RUN_MM: i32 = 400;
/// Parapet: rises over this run from the lip (mm) ...
const PARAPET_RISE_MM: i32 = 300;
/// ... stays at full height over this run (mm) ...
const PARAPET_TOP_MM: i32 = 400;
/// ... and falls back to the ground over this run (mm).
const PARAPET_FALL_MM: i32 = 600;
/// The parapet only raises ground at most this far below the base (mm), so
/// it never fills another trench or a crater.
const PARAPET_UNDUG_MM: i32 = 50;
/// A digger works when this close to his spot (mm).
pub const DIG_REACH_MM: i32 = 1500;

/// One man's share of a trench line.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct DigSegment {
    /// Arc-length range along the line (m).
    pub from: Fx,
    pub to: Fx,
    /// Work done, in work points × tick rate.
    pub work: u64,
    /// Passes stamped so far (`passes` = finished).
    pub passes: u8,
    /// The man digging it.
    pub digger: Option<SimId>,
}

/// Digging state of a Dig order.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct DigJob {
    /// Parapet side: `1` on the left of the line direction (`perp()`),
    /// `-1` on the right.
    pub front: i8,
    pub segments: Vec<DigSegment>,
}

impl DigJob {
    /// Work points × tick rate needed for `seg`.
    pub fn required(seg: &DigSegment, dig: &DigSetup, hz: u32) -> u64 {
        let len = (seg.to - seg.from).raw().max(0) as u64;
        ((dig.work_per_m as u64 * hz as u64 * len) >> Fx::FRAC_BITS).max(1)
    }

    /// Whether every segment has all its passes.
    pub fn finished(&self, dig: &DigSetup) -> bool {
        self.segments.iter().all(|s| s.passes as u32 >= dig.passes)
    }

    /// Overall progress in percent (work done over work needed).
    pub fn progress_pct(&self, dig: &DigSetup, hz: u32) -> u8 {
        let (mut done, mut need) = (0u64, 0u64);
        for s in &self.segments {
            let r = Self::required(s, dig, hz);
            done += s.work.min(r);
            need += r;
        }
        (done * 100 / need.max(1)) as u8
    }

    /// Unit vector toward the front (parapet side) of `line` as a whole.
    pub fn facing(&self, line: &[FxVec2]) -> FxVec2 {
        let along = (line[line.len() - 1] - line[0]).normalize_or_zero();
        let left = along.perp();
        if self.front >= 0 { left } else { -left }
    }

    /// A job for `line` split among `men`, with the parapet facing the
    /// nearest soldier not owned by `owner` (or away from `base` if there is
    /// none). Depth already dug at each segment's midpoint is credited.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        line: &[FxVec2],
        men: usize,
        owner: PlayerId,
        base: FxVec2,
        ents: &Entities,
        terrain: &Heightfield,
        dig: &DigSetup,
        hz: u32,
    ) -> DigJob {
        let len = polyline_length(line);
        let mid = point_along(line, len.div_int(2));
        let enemy = ents
            .soldiers()
            .filter(|id| ents.owner[id.index()] != owner)
            .map(|id| {
                let p = ents.transform[id.index()].pos.xy();
                ((p - mid).length_sq_raw(), id, p)
            })
            .min()
            .map(|(_, _, p)| p);
        let toward = match enemy {
            Some(p) => p - mid,
            None => mid - base,
        };
        let left = (line[line.len() - 1] - line[0]).perp();
        let front = if left.dot_raw(toward) >= 0 { 1 } else { -1 };
        let n = men.max(1) as i32;
        let depth = Fx::from(dig.depth);
        let segments = (0..n)
            .map(|i| {
                let from = len.mul_int(i).div_int(n);
                let to = if i == n - 1 {
                    len
                } else {
                    len.mul_int(i + 1).div_int(n)
                };
                let mut seg = DigSegment {
                    from,
                    to,
                    work: 0,
                    passes: 0,
                    digger: None,
                };
                let c = point_along(line, (from + to).div_int(2));
                let dug = terrain.base_height_at(c) - terrain.height_at(c);
                if dug > Fx::ZERO {
                    let done = (dug.raw() as i64 * dig.passes as i64 / depth.raw().max(1) as i64)
                        .clamp(0, dig.passes as i64) as u64;
                    seg.passes = done as u8;
                    seg.work = Self::required(&seg, dig, hz) * done / dig.passes as u64;
                }
                seg
            })
            .collect();
        DigJob { front, segments }
    }
}

/// Closest point of `line` to `p`: `(distance², arc length there, on the
/// left side)`; ties go to the earlier segment.
fn nearest(line: &[FxVec2], p: FxVec2) -> (u64, Fx, bool) {
    let mut best = (u64::MAX, Fx::ZERO, false);
    let mut acc = Fx::ZERO;
    for w in line.windows(2) {
        let (a, ab) = (w[0], w[1] - w[0]);
        let ap = p - a;
        let len_sq = ab.length_sq_raw() as i128;
        let t = if len_sq == 0 {
            Fx::ZERO
        } else {
            let num = (ap.dot_raw(ab) as i128).clamp(0, len_sq);
            Fx::from_raw(((num << Fx::FRAC_BITS) / len_sq) as i32)
        };
        let q = a + ab * t;
        let d_sq = (p - q).length_sq_raw();
        let len = ab.length();
        if d_sq < best.0 {
            best = (d_sq, acc + len.mul(t), ab.det(ap) > Fx::ZERO);
        }
        acc += len;
    }
    best
}

/// Profile depth (positive, m) at lateral distance `r` from the centre line,
/// at pass fraction `k / n`.
fn cut_depth(r: Fx, prof: &DigSetup, k: i32, n: i32) -> Fx {
    let depth = Fx::from(prof.depth);
    let half_floor = Fx::from(prof.floor_width).div_int(2);
    let wall = Fx::from_milli(WALL_RUN_MM);
    let full = if r <= half_floor {
        depth
    } else if r < half_floor + wall {
        depth.mul(half_floor + wall - r).div(wall)
    } else {
        Fx::ZERO
    };
    full.min(depth.mul_int(k).div_int(n))
}

/// Parapet height (m) at distance `u` beyond the lip, at pass `k / n`.
fn parapet_height(u: Fx, prof: &DigSetup, k: i32, n: i32) -> Fx {
    let top = Fx::from(prof.parapet_height).mul_int(k).div_int(n);
    let rise = Fx::from_milli(PARAPET_RISE_MM);
    let flat = rise + Fx::from_milli(PARAPET_TOP_MM);
    let fall = Fx::from_milli(PARAPET_FALL_MM);
    if u <= Fx::ZERO || u >= flat + fall {
        Fx::ZERO
    } else if u < rise {
        top.mul(u).div(rise)
    } else if u < flat {
        top
    } else {
        top.mul(flat + fall - u).div(fall)
    }
}

/// Stamps the trench profile at pass `k` of `n` over the part of `line`
/// between arc lengths `from` and `to` (the line's last segment also takes
/// the end cap at `to`). `front` as in [`DigJob::front`].
#[allow(clippy::too_many_arguments)]
pub(crate) fn stamp(
    terrain: &mut Heightfield,
    line: &[FxVec2],
    front: i8,
    from: Fx,
    to: Fx,
    k: i32,
    n: i32,
    prof: &DigSetup,
    tick: u64,
) {
    let len = polyline_length(line);
    let closed_end = to >= len;
    let half_floor = Fx::from(prof.floor_width).div_int(2);
    let lip = half_floor + Fx::from_milli(WALL_RUN_MM);
    let reach = lip + Fx::from_milli(PARAPET_RISE_MM + PARAPET_TOP_MM + PARAPET_FALL_MM + 250);
    // Box around the sub-line: its end points and the vertices inside it.
    let mut lo = point_along(line, from);
    let mut hi = lo;
    let mut grow = |q: FxVec2| {
        lo = FxVec2::new(lo.x.min(q.x), lo.y.min(q.y));
        hi = FxVec2::new(hi.x.max(q.x), hi.y.max(q.y));
    };
    grow(point_along(line, to));
    let mut acc = Fx::ZERO;
    for w in line.windows(2) {
        acc += (w[1] - w[0]).length();
        if acc > from && acc < to {
            grow(w[1]);
        }
    }
    let idx = |v: Fx| v.raw() >> DETAIL_SHIFT;
    let undug = Fx::from_milli(PARAPET_UNDUG_MM);
    terrain.edit(
        (idx(lo.x - reach), idx(lo.y - reach)),
        (idx(hi.x + reach) + 1, idx(hi.y + reach) + 1),
        tick,
        |px, py, base, cur| {
            let p = FxVec2::new(
                Fx::from_raw(px << DETAIL_SHIFT),
                Fx::from_raw(py << DETAIL_SHIFT),
            );
            let (d_sq, s, left) = nearest(line, p);
            if s < from || s > to || (s == to && !closed_end) {
                return cur;
            }
            let r = Fx::sqrt_q32(d_sq);
            let depth = cut_depth(r, prof, k, n);
            if depth > Fx::ZERO {
                return cur.min(base - depth);
            }
            if left == (front >= 0) && cur >= base - undug {
                let h = parapet_height(r - lip, prof, k, n);
                if h > Fx::ZERO {
                    return cur.max(base + h);
                }
            }
            cur
        },
    );
}

impl MatchState {
    /// Advances every digging squad by one tick (see the module docs).
    pub(crate) fn dig_work(&mut self, ents: &Entities, terrain: &mut Heightfield, tick: u64) {
        let hz = self.setup.tick_hz;
        let dig = &self.setup.rules.dig;
        let n = dig.passes as i32;
        let reach_sq = Fx::from_milli(DIG_REACH_MM).raw() as u64;
        let reach_sq = reach_sq * reach_sq;
        for sq in self.squads.values_mut() {
            if sq.members.is_empty()
                || !matches!(sq.nav.phase, NavPhase::Arriving | NavPhase::Holding)
                || sq.nav.slots.len() != sq.members.len()
            {
                continue;
            }
            let rate = self.setup.squad_types[sq.type_idx as usize].dig_rate_pct as u64;
            let Order::Dig { line, job } = &mut sq.order else {
                continue;
            };
            // 1. Let go of men who left or finished.
            for seg in job.segments.iter_mut() {
                if seg
                    .digger
                    .is_some_and(|d| seg.passes as i32 >= n || !sq.members.contains(&d))
                {
                    seg.digger = None;
                }
            }
            // 2. Idle men take the nearest open segment.
            for (k, &m) in sq.members.iter().enumerate() {
                if job.segments.iter().any(|g| g.digger == Some(m)) {
                    continue;
                }
                let slot = sq.nav.slots[k];
                let pick = job
                    .segments
                    .iter()
                    .enumerate()
                    .filter(|(_, g)| g.digger.is_none() && (g.passes as i32) < n)
                    .map(|(i, g)| {
                        let c = point_along(line, (g.from + g.to).div_int(2));
                        ((c - slot).length_sq_raw(), i)
                    })
                    .min();
                if let Some((_, i)) = pick {
                    job.segments[i].digger = Some(m);
                }
            }
            // 3–4. Walk to the spot, work there, stamp finished passes.
            for seg in job.segments.iter_mut() {
                let Some(d) = seg.digger else { continue };
                let Some(k) = sq.members.iter().position(|&m| m == d) else {
                    continue;
                };
                let spot = terrain.clamp(point_along(line, (seg.from + seg.to).div_int(2)));
                sq.nav.slots[k] = spot;
                let at = ents.transform[d.index()].pos.xy();
                if (spot - at).length_sq_raw() > reach_sq {
                    continue;
                }
                let need = DigJob::required(seg, dig, hz);
                seg.work = (seg.work + rate).min(need);
                let passes = (seg.work * n as u64 / need) as i32;
                if passes > seg.passes as i32 {
                    seg.passes = passes as u8;
                    stamp(
                        terrain, line, job.front, seg.from, seg.to, passes, n, dig, tick,
                    );
                }
            }
            if job.finished(dig) {
                sq.order = Order::Idle;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::FxVec3;
    use crate::terrain::TerrainKind;
    use frontline_content::Fixed;

    fn prof() -> DigSetup {
        DigSetup {
            depth: Fixed::from_cm(160),
            floor_width: Fixed::from_cm(80),
            parapet_height: Fixed::from_cm(30),
            max_length: Fixed::from_int(60),
            work_per_m: 8400,
            passes: 8,
        }
    }

    /// Equal within 1 mm (fixed-point rounding).
    #[track_caller]
    fn assert_near(a: Fx, b: Fx) {
        assert!((a - b).abs() <= Fx::from_milli(1), "{a:?} != {b:?}");
    }

    #[test]
    fn profile_shape() {
        let p = prof();
        let m = Fx::from_milli;
        assert_near(cut_depth(m(0), &p, 8, 8), m(1600));
        assert_near(cut_depth(m(400), &p, 8, 8), m(1600));
        assert_near(cut_depth(m(600), &p, 8, 8), m(800));
        assert_near(cut_depth(m(800), &p, 8, 8), Fx::ZERO);
        // Half the passes: half depth, same walls.
        assert_near(cut_depth(m(0), &p, 4, 8), m(800));
        assert_near(cut_depth(m(700), &p, 4, 8), m(400));
        assert_near(parapet_height(m(0), &p, 8, 8), Fx::ZERO);
        assert_near(parapet_height(m(500), &p, 8, 8), m(300));
        assert_near(parapet_height(m(1300), &p, 8, 8), Fx::ZERO);
    }

    #[test]
    fn stamped_trench_has_floor_walls_and_front_parapet() {
        let mut t = Heightfield::generate(TerrainKind::Flat, 128, 1);
        // Line west → east; front = 1 puts the parapet on the north side.
        let line = [FxVec2::from_int(40, 64), FxVec2::from_int(60, 64)];
        stamp(
            &mut t,
            &line,
            1,
            Fx::ZERO,
            Fx::from_int(20),
            8,
            8,
            &prof(),
            7,
        );
        let h = |x: i32, ymm: i32| t.height_at(FxVec2::new(Fx::from_int(x), Fx::from_milli(ymm)));
        assert_near(h(50, 64_000), Fx::from_milli(-1600));
        assert_near(h(50, 64_250), Fx::from_milli(-1600));
        assert!(h(50, 64_600) > Fx::from_milli(-1600) && h(50, 64_600) < Fx::ZERO);
        // North (front): parapet; south (rear): untouched ground.
        assert_near(h(50, 65_500), Fx::from_milli(300));
        assert_near(h(50, 62_500), Fx::ZERO);
        // Rounded end: dug at the end point, nothing 1 m beyond.
        assert_near(h(60, 64_000), Fx::from_milli(-1600));
        assert_near(h(61, 64_000), Fx::ZERO);
        assert!(t.is_trench(12, 16), "trench nav flag");
        assert!(!t.is_trench(12, 18));
    }

    #[test]
    fn parapet_stops_flat_fire_from_the_front_only() {
        let mut t = Heightfield::generate(TerrainKind::Flat, 512, 1);
        let line = [FxVec2::from_int(240, 256), FxVec2::from_int(260, 256)];
        stamp(
            &mut t,
            &line,
            1,
            Fx::ZERO,
            Fx::from_int(20),
            8,
            8,
            &prof(),
            1,
        );
        let at =
            |z_mm: i32| FxVec3::new(Fx::from_int(250), Fx::from_int(256), Fx::from_milli(z_mm));
        // A standing rifleman 150 m north (the front), muzzle 1.5 m up, and
        // one 150 m south (the rear).
        let front = FxVec3::new(Fx::from_int(250), Fx::from_int(406), Fx::from_milli(1500));
        let rear = FxVec3::new(Fx::from_int(250), Fx::from_int(106), Fx::from_milli(1500));
        // The top of a man standing on the floor (1.75 m): 0.15 m above
        // ground, under the 0.3 m parapet line.
        let hit = t
            .segment_hit(front, at(150))
            .expect("parapet stops the shot");
        let short = hit.y - Fx::from_int(256);
        assert!(
            short > Fx::from_milli(800) && short < Fx::from_milli(2100),
            "{hit:?}"
        );
        assert!(!t.line_of_sight(front, at(150)));
        // From the rear there is no parapet.
        assert!(t.segment_hit(rear, at(150)).is_none());
        assert!(t.line_of_sight(rear, at(150)));
        // Well above the parapet: in view from the front too.
        assert!(t.segment_hit(front, at(450)).is_none());
    }

    #[test]
    fn crossing_trenches_merge_and_parapet_does_not_fill() {
        let mut t = Heightfield::generate(TerrainKind::Flat, 128, 1);
        let a = [FxVec2::from_int(40, 64), FxVec2::from_int(60, 64)];
        let b = [FxVec2::from_int(50, 54), FxVec2::from_int(50, 74)];
        stamp(&mut t, &a, 1, Fx::ZERO, Fx::from_int(20), 8, 8, &prof(), 1);
        stamp(&mut t, &b, 1, Fx::ZERO, Fx::from_int(20), 8, 8, &prof(), 2);
        let c = FxVec2::from_int(50, 64);
        assert_near(t.height_at(c), Fx::from_milli(-1600));
        // b's parapet (west side) does not refill a's trench.
        let p = FxVec2::new(Fx::from_milli(48_500), Fx::from_int(64));
        assert_near(t.height_at(p), Fx::from_milli(-1600));
    }
}
