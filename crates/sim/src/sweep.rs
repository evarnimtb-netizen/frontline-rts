//! S10b Build C: sweeping and lifting mines, and mines set off by HE
//! (docs/slices/S10b-obstacles.md R8-R13, 5 "Clearing").
//!
//! * **`ClearMines { squad, a, b }`** gives a `clear_mines` squad
//!   `Order::ClearMines`. Its sweepers are the two able members nearest the
//!   sweep point `a + dir * along` (ascending `SimId` on ties; at the order
//!   that is "nearest `a`"; a dead sweeper is replaced the same way). They
//!   walk abreast at +-1 m from the centreline at `sweep_speed_pct` of the
//!   squad's pace (`steer_squads`); the others hold 5 m behind `a`.
//!   `along` advances only while every sweeper is within 1 m of his spot
//!   and no mine is being lifted.
//! * **Detection (R8)**, every tick, per sweeper, mines in ascending id
//!   within `detect_cm` that his player does not know: a u32 roll keyed
//!   `(tick, sweeper, MINE_DETECT)` mixed with the mine id; found when
//!   `roll * 100 * hz < detect_pct_per_s * 2^32` (per-tick p =
//!   `detect_pct_per_s / (100 * hz)`). A find sets the finder's player bit
//!   only (R13) and `mines_found`.
//! * **Lifting (R10, R11)**: the sweeper nearest the mine walks to 1 m
//!   short of it along the lane; his spot (and every sweeper spot) is
//!   clamped back along the lane until no mine his player knows lies within
//!   its `trigger_cm + 50 cm`. Within 1.5 m of the mine he adds his tier's
//!   `cut_pct` per tick (x0.3 suppressed, 0 pinned) toward `lift_s x 100 x
//!   hz`; done: the mine is removed (`mines_lifted`).
//! * **HE (R12)**: every burst with a clear line from its lift point to
//!   10 cm above a mine within its `blast_radius` sets that mine's
//!   `boom_tick` to the next tick (`mines_blasted` on the owner; known to
//!   all). `take_mines` bursts it then: a chain moves one hop per tick.
//!
//! Nothing here runs without a `ClearMines` order (`sweeps` counted) or
//! without mines (the HE hook), so AI-vs-AI play is unchanged.

use frontline_content::setup::Capability;
use frontline_content::tuning::FortTuning;

use crate::ecs::{Entities, PlayerId, SimId};
use crate::fort::{FortRefusal, near_segment};
use crate::fx::{Fx, FxVec2, FxVec3};
use crate::game::{Ctx, MatchState, NavPhase, Order, SquadId};
use crate::rng::{Purpose, mix64, rng};
use crate::terrain::Heightfield;

/// A sweeper's lateral offset from the lane centreline.
const ABREAST: Fx = Fx::ONE;
/// How close a sweeper must be to his spot for `along` to advance.
const ON_SPOT: Fx = Fx::ONE;
/// The lifter's stand-off from the mine he lifts (R10: beyond any
/// `trigger_cm + 50 cm`).
const LIFT_STAND: Fx = Fx::ONE;
/// The lifter works while this close to the mine.
const LIFT_REACH: Fx = Fx::from_raw(Fx::ONE.raw() * 3 / 2);
/// The R10 clearance beyond a mine's `trigger_cm`.
const CLEAR_SLACK_CM: i32 = 50;
/// Clamp step back along the lane, and at most this many steps.
const CLAMP_STEP: Fx = Fx::from_raw(Fx::ONE.raw() / 10);
const CLAMP_STEPS: u32 = 60;
/// Where the rest of the squad holds: this far behind `a`.
const HOLD_BACK: i32 = 5;

/// Whether `roll` (u32) finds a mine at `pct` % per second and `hz` ticks
/// per second (u64, no float).
#[inline]
pub(crate) fn detects(roll: u32, pct: u32, hz: u64) -> bool {
    (roll as u64) * 100 * hz < (pct as u64) << 32
}

/// The detection roll of `sweeper` for mine `mine` on `tick`.
#[inline]
pub(crate) fn detect_roll(seed: u64, tick: u64, sweeper: SimId, mine: u32) -> u32 {
    (mix64(rng(seed, tick, sweeper, Purpose::MINE_DETECT) ^ mine as u64) >> 32) as u32
}

impl MatchState {
    /// `ClearMines { squad, a, b }` (S10b-build-C 1).
    pub(crate) fn order_clear_mines(
        &mut self,
        ctx: &mut Ctx,
        p: PlayerId,
        squad: SquadId,
        a: FxVec2,
        b: FxVec2,
    ) -> bool {
        let Some(t) = self.setup.rules.fortify.clone() else {
            return false;
        };
        if self.fort_off || !self.owned(p, squad) || self.is_vehicle_squad(squad) {
            return self.refuse_fort(p, Some(squad), FortRefusal::NotAllowed);
        }
        let ty = &self.setup.squad_types[self.squads[&squad].type_idx as usize];
        if !ty.has(Capability::ClearMines) {
            return self.refuse_fort(p, Some(squad), FortRefusal::CannotBuild);
        }
        if !self.squads[&squad]
            .members
            .iter()
            .any(|&m| ctx.ents.is_alive(m))
        {
            return self.refuse_fort(p, Some(squad), FortRefusal::NotAllowed);
        }
        let size = Fx::from_int(self.setup.map.size_m);
        let on_map = |v: FxVec2| [v.x, v.y].iter().all(|c| *c >= Fx::ZERO && *c <= size);
        if !on_map(a) || !on_map(b) || (b - a).length() > Fx::from_int(t.lane_max_m as i32) {
            return self.refuse_fort(p, Some(squad), FortRefusal::BadSpot);
        }
        self.clear_detached(ctx, squad);
        self.squads.get_mut(&squad).expect("owned").order = Order::ClearMines {
            a,
            b,
            along: Fx::ZERO,
            lift: None,
            work: 0,
        };
        if let Some(acc) = self.fort.accounts.get_mut(p.0 as usize) {
            acc.sweeps = acc.sweeps.saturating_add(1);
        }
        self.request_path(squad, a);
        true
    }

    /// `spot` moved back along `-d` until no mine `bit` knows lies within
    /// its `trigger_cm + 50 cm` (at most `CLAMP_STEPS` steps).
    fn clear_of_known(&self, mut spot: FxVec2, d: FxVec2, bit: u8) -> FxVec2 {
        for _ in 0..CLAMP_STEPS {
            let near = self.fort.mines.values().any(|m| {
                if m.known & bit == 0 {
                    return false;
                }
                let trig = self
                    .setup
                    .items
                    .get(m.item as usize)
                    .and_then(|it| it.mine.as_ref())
                    .map_or(0, |x| x.trigger_cm as i32);
                let r = Fx::from_milli((trig + CLEAR_SLACK_CM) * 10);
                (spot - m.pos).length_sq_raw() < (r.raw() as u64).pow(2)
            });
            if !near || d == FxVec2::ZERO {
                break;
            }
            spot -= d * CLAMP_STEP;
        }
        spot
    }

    /// Spec 5 "Clearing" per tick (`fort_tick` calls it only while a
    /// `ClearMines` order stands).
    pub(crate) fn sweep_tick(
        &mut self,
        ents: &Entities,
        terrain: &Heightfield,
        t: &FortTuning,
        tick: u64,
    ) {
        let ids: Vec<SquadId> = self
            .squads
            .iter()
            .filter(|(_, q)| matches!(q.order, Order::ClearMines { .. }))
            .map(|(&id, _)| id)
            .collect();
        let hz = self.setup.tick_hz as u64;
        let need = t.lift_s as u64 * hz * 100;
        let crouch = self.setup.rules.combat.crouch_suppression;
        let detect_r2 = (Fx::from_milli(t.detect_cm as i32 * 10).raw() as u64).pow(2);
        let half = Fx::from_milli(t.lane_half_cm as i32 * 10);
        for id in ids {
            let sq = &self.squads[&id];
            let Order::ClearMines {
                a,
                b,
                mut along,
                lift,
                mut work,
            } = sq.order.clone()
            else {
                continue;
            };
            let owner = sq.owner;
            let bit = 1u8 << owner.0.min(7);
            let len = (b - a).length();
            let d = (b - a).normalize_or_zero();
            let n = d.perp();
            let able = |m: SimId| {
                let u = &ents.unit[m.index()];
                ents.is_alive(m) && !u.detached && u.riding.is_none() && u.serving.is_none()
            };
            let at = a + d * along;
            let mut cand: Vec<(u64, SimId)> = sq
                .members
                .iter()
                .copied()
                .filter(|&m| able(m))
                .map(|m| ((ents.transform[m.index()].pos.xy() - at).length_sq_raw(), m))
                .collect();
            cand.sort_unstable();
            cand.truncate(2);
            if cand.is_empty() {
                self.squads.get_mut(&id).expect("listed").order = Order::Idle;
                continue;
            }
            let mut sweepers: Vec<SimId> = cand.iter().map(|&(_, m)| m).collect();
            sweepers.sort_unstable();
            let type_idx = sq.type_idx as usize;
            // A lifted mine that went off or was lifted by others: drop it.
            let mut lift =
                lift.filter(|l| self.fort.mines.get(l).is_some_and(|m| m.known & bit != 0));
            if lift.is_none() {
                work = 0;
            }
            // R8: detection.
            if !self.fort.mines.is_empty() {
                for &s in &sweepers {
                    let p = ents.transform[s.index()].pos.xy();
                    let mut found: Vec<u32> = Vec::new();
                    for (&mid, m) in &self.fort.mines {
                        if m.known & bit != 0 || (m.pos - p).length_sq_raw() > detect_r2 {
                            continue;
                        }
                        let roll = detect_roll(self.setup.seed, tick, s, mid);
                        if detects(roll, t.detect_pct_per_s, hz) {
                            found.push(mid);
                        }
                    }
                    for mid in found {
                        self.fort.mines.get_mut(&mid).expect("listed").known |= bit;
                        let acc = &mut self.fort.accounts[owner.0 as usize];
                        acc.mines_found = acc.mines_found.saturating_add(1);
                        lift = lift.or(Some(mid));
                    }
                }
            }
            // The next known mine in the lane: ahead within detection
            // reach, or any at the lane end (nearest `a` first).
            if lift.is_none() {
                let reach = along + Fx::from_milli(t.detect_cm as i32 * 10);
                lift = self
                    .fort
                    .mines
                    .iter()
                    .filter(|(_, m)| m.known & bit != 0 && near_segment(m.pos, a, b, half))
                    .map(|(&mid, m)| ((m.pos - a).dot(d), mid))
                    .filter(|&(proj, _)| along >= len || proj <= reach)
                    .min()
                    .map(|(_, mid)| mid);
                work = 0;
            }
            let pos = |m: SimId| ents.transform[m.index()].pos.xy();
            let mut spots: Vec<(SimId, FxVec2)> = Vec::with_capacity(2);
            for (k, &s) in sweepers.iter().enumerate() {
                let side = if sweepers.len() == 1 {
                    Fx::ZERO
                } else if k == 0 {
                    ABREAST
                } else {
                    -ABREAST
                };
                spots.push((s, at + n * side));
            }
            if let Some(l) = lift {
                let mp = self.fort.mines[&l].pos;
                let lifter = sweepers
                    .iter()
                    .copied()
                    .min_by_key(|&s| ((pos(s) - mp).length_sq_raw(), s))
                    .expect("a sweeper");
                // From the mine toward the lifter: his straight walk to the
                // stand-off stays on his side of it, never across (R10;
                // review R17). The lane's back side when he is on it.
                let mine_to_him = (pos(lifter) - mp).normalize_or_zero();
                let toward = if mine_to_him != FxVec2::ZERO {
                    mine_to_him
                } else if d == FxVec2::ZERO {
                    FxVec2::ZERO
                } else {
                    FxVec2::ZERO - d
                };
                for sp in spots.iter_mut() {
                    if sp.0 == lifter {
                        sp.1 = mp + toward * LIFT_STAND;
                    }
                }
                let c = &ents.combat[lifter.index()];
                let near = (pos(lifter) - mp).length_sq_raw() <= (LIFT_REACH.raw() as u64).pow(2);
                let rate = Self::tier_of(&self.setup.squad_types[type_idx], t).cut_pct as u64;
                if near && !c.pinned {
                    work += if c.suppression / 1000 >= crouch {
                        rate * 3 / 10
                    } else {
                        rate
                    };
                }
                if work >= need {
                    self.fort.mines.remove(&l);
                    // Other squads lifting the same mine drop it now, so no
                    // order points at a gone mine between ticks (review R16).
                    for (oid, o) in self.squads.iter_mut() {
                        if *oid != id
                            && let Order::ClearMines { lift, work, .. } = &mut o.order
                            && *lift == Some(l)
                        {
                            *lift = None;
                            *work = 0;
                        }
                    }
                    let acc = &mut self.fort.accounts[owner.0 as usize];
                    acc.mines_lifted = acc.mines_lifted.saturating_add(1);
                    lift = None;
                    work = 0;
                }
            } else if spots
                .iter()
                .all(|&(s, sp)| (pos(s) - sp).length_sq_raw() <= (ON_SPOT.raw() as u64).pow(2))
                && along < len
            {
                let speed: Fx = self.setup.squad_types[type_idx].speed.into();
                let step = speed
                    .mul_int(t.sweep_speed_pct as i32)
                    .div_int(100)
                    .div_int(hz.max(1) as i32);
                along = (along + step).min(len);
            }
            for sp in spots.iter_mut() {
                sp.1 = terrain.clamp(self.clear_of_known(sp.1, d, bit));
            }
            let done = along >= len
                && lift.is_none()
                && !self
                    .fort
                    .mines
                    .values()
                    .any(|m| m.known & bit != 0 && near_segment(m.pos, a, b, half));
            let hold = terrain.clamp(a - d.mul_int(HOLD_BACK));
            let sq = self.squads.get_mut(&id).expect("listed");
            if done {
                sq.order = Order::Idle;
                continue;
            }
            if matches!(sq.nav.phase, NavPhase::Arriving | NavPhase::Holding)
                && sq.nav.slots.len() == sq.members.len()
            {
                for (k, m) in sq.members.iter().enumerate() {
                    sq.nav.slots[k] = spots
                        .iter()
                        .find(|(s, _)| s == m)
                        .map_or(hold, |&(_, sp)| sp);
                }
            }
            sq.order = Order::ClearMines {
                a,
                b,
                along,
                lift,
                work,
            };
        }
    }

    /// R12 (end of `combat::burst`): mines within `blast_r` of `pos` (2D,
    /// ascending id) with a clear line from `lift` to 10 cm above them go
    /// off next tick; each counts once in its owner's `mines_blasted` and
    /// becomes known to all (R13).
    pub(crate) fn blast_mines(
        &mut self,
        terrain: &Heightfield,
        pos: FxVec2,
        lift: FxVec3,
        blast_r: Fx,
        tick: u64,
    ) {
        if self.fort.mines.is_empty() || blast_r <= Fx::ZERO {
            return;
        }
        let r2 = (blast_r.raw() as u64).pow(2);
        let all = ((1u16 << self.setup.players.len().min(8)) - 1) as u8;
        let mut hit: Vec<u32> = Vec::new();
        for (&id, m) in &self.fort.mines {
            if m.boom_tick.is_some() || (m.pos - pos).length_sq_raw() > r2 {
                continue;
            }
            let at = terrain.clamp(m.pos);
            let top = at.extend(terrain.height_at(at) + Fx::from_milli(100));
            if terrain.line_of_sight(lift, top) {
                hit.push(id);
            }
        }
        for id in hit {
            let m = self.fort.mines.get_mut(&id).expect("listed");
            m.boom_tick = Some(tick + 1);
            m.known = all;
            let o = m.owner.0 as usize;
            if let Some(acc) = self.fort.accounts.get_mut(o) {
                acc.mines_blasted = acc.mines_blasted.saturating_add(1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detection_roll_rate_is_p_per_tick() {
        // Control: `detects` with `<< 31` -> about half the hits.
        let mut hits = 0u32;
        for tick in 0..20_000u64 {
            for mine in 0..10u32 {
                hits += u32::from(detects(detect_roll(7, tick, SimId(60), mine), 50, 30));
            }
        }
        // p = 1/60 of 200 000 draws: 3333, 3 sd ~ 170.
        assert!((3150..3520).contains(&hits), "{hits}");
    }
}
