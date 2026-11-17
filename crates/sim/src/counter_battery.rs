//! S11a2 counter-battery by sound (docs/slices/S11a2.md 3, rules K1-K4;
//! design artillery-on-map.md 4.3). Every fire-mission round of an infantry
//! gun is heard by the other players; each keeps a short list of spots
//! (where the rounds came from). A spot with enough evidence becomes a
//! marker after a delay, with an error that shrinks with every further
//! round and a piece of evidence lost every `cb_decay_s`. A marker is known
//! ground for map fire (K4) and reveals nothing else.
//!
//! Switch `[fire_mission] counter_battery` (0: no spot is ever made).

use serde::{Deserialize, Serialize};

use crate::ecs::{Entities, EntityKind, PlayerId, SimId};
use crate::fx::{Fx, FxVec2};
use crate::game::MatchState;
use crate::rng::{Purpose, RngStream};

/// Most spots one player keeps (`PlayerState::cb`).
pub const MAX_CB_SPOTS: usize = 8;

/// One enemy firing spot a player has heard (saved, hashed).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct CbSpot {
    /// The player whose gun fired (never the holder).
    pub of: u8,
    /// Where the first counted round was fired from.
    pub pos: FxVec2,
    /// The marker (meaningful once `marked`).
    pub at: FxVec2,
    /// The marker's error radius (mm; 0 until marked).
    pub err_mm: u32,
    /// The listener distance the first error was made from (mm).
    pub heard_mm: u32,
    /// Rounds counted, less the decay; the spot goes at 0.
    pub evidence: u16,
    /// Rounds counted, never decayed (the AI's "new evidence", R2).
    pub shots: u32,
    /// The next tick one piece of evidence decays (a whole second).
    pub decay_at: u64,
    /// The tick the marker is made (a whole second; 0: none pending).
    pub mark_at: u64,
    /// A marker exists.
    pub marked: bool,
}

/// `t` rounded up to a whole second (`cb_tick` works on whole seconds).
pub(crate) fn whole_second(t: u64, hz: u64) -> u64 {
    let hz = hz.max(1);
    t.div_ceil(hz) * hz
}

/// Length of `v` in mm (non-negative, fixed-point maths only).
fn mm(v: FxVec2) -> u32 {
    (((v.length().raw().max(0) as u64) * 1000) >> 16).min(u32::MAX as u64) as u32
}

/// A length in mm as metres.
fn from_mm(v: u32) -> Fx {
    Fx::from_raw((((v as u64) << 16) / 1000).min(i32::MAX as u64) as i32)
}

impl MatchState {
    /// Player `p`'s living soldier nearest `at` (ties: the lower id), and
    /// its distance. Squads in id order, members in squad order.
    pub(crate) fn nearest_soldier(
        &self,
        ents: &Entities,
        p: PlayerId,
        at: FxVec2,
    ) -> Option<(SimId, Fx)> {
        let mut best: Option<(u64, SimId)> = None;
        for q in self.squads.values().filter(|q| q.owner == p) {
            for &m in &q.members {
                let i = m.index();
                if ents.kind[i] != EntityKind::Soldier || ents.combat[i].hp == 0 {
                    continue;
                }
                let d = (ents.transform[i].pos.xy() - at).length_sq_raw();
                if best.is_none_or(|(bd, bid)| (d, m) < (bd, bid)) {
                    best = Some((d, m));
                }
            }
        }
        best.map(|(_, m)| (m, (ents.transform[m.index()].pos.xy() - at).length()))
    }

    /// K1: one fire-mission round of an infantry gun of `firer` fired from
    /// `gun` at `tick` is heard by every other player with a living soldier
    /// (ascending id). Nothing with `counter_battery` 0.
    pub(crate) fn cb_note(&mut self, ents: &Entities, firer: PlayerId, gun: FxVec2, tick: u64) {
        let Some(fm) = self.setup.rules.fire_mission.as_ref() else {
            return;
        };
        if fm.counter_battery == 0 {
            return;
        }
        let (reset, to_mark) = (Fx::from_int(fm.cb_reset_m as i32), fm.cb_rounds_to_mark);
        let (step, floor) = (fm.cb_error_step_pct as u64, fm.cb_error_floor_pct as u64);
        let hz = self.setup.tick_hz.max(1) as u64;
        let decay = fm.cb_decay_s as u64 * hz;
        let delay = fm.cb_delay_s as u64 * hz;
        let mut heard = 0u32;
        for e in 0..self.players.len() {
            let hearer = PlayerId(e as u8);
            if hearer == firer {
                continue;
            }
            // The listener now: the hearer's nearest living soldier.
            let Some((_, listen)) = self.nearest_soldier(ents, hearer, gun) else {
                continue;
            };
            heard += 1;
            let spots = &mut self.players[e].cb;
            // The spot this round counts for: nearest, ties the lower index.
            let mut found: Option<(u64, usize)> = None;
            for (k, s) in spots.iter().enumerate() {
                if s.of != firer.0 {
                    continue;
                }
                let r = if s.marked {
                    reset.max(from_mm(s.err_mm))
                } else {
                    reset
                };
                let d = (s.pos - gun).length_sq_raw();
                if d <= FxVec2::new(r, Fx::ZERO).length_sq_raw()
                    && found.is_none_or(|(fd, _)| d < fd)
                {
                    found = Some((d, k));
                }
            }
            let k = match found {
                Some((_, k)) => {
                    let s = &mut spots[k];
                    s.evidence = s.evidence.saturating_add(1);
                    s.shots = s.shots.saturating_add(1);
                    // K3: a marked spot sharpens (no new draw).
                    if s.marked && s.err_mm > 0 {
                        let l_now = mm(FxVec2::new(listen, Fx::ZERO)) as u64;
                        let new = (s.err_mm as u64 * step / 100).max(l_now * floor / 100) as u32;
                        if new < s.err_mm {
                            let off = s.at - s.pos;
                            let scale = Fx::from_raw(
                                (((new as u64) << 16) / s.err_mm as u64).min(i32::MAX as u64)
                                    as i32,
                            );
                            s.at = s.pos + off * scale;
                            s.err_mm = new;
                        }
                    }
                    k
                }
                None => {
                    if spots.len() >= MAX_CB_SPOTS {
                        // The least evidence goes first (ties: the oldest).
                        let (k, _) = spots
                            .iter()
                            .enumerate()
                            .min_by_key(|(k, s)| (s.evidence, *k))
                            .expect("full");
                        spots.remove(k);
                        let c = &mut self.players[e].s11a2;
                        c.cb_dropped = c.cb_dropped.saturating_add(1);
                    }
                    let spots = &mut self.players[e].cb;
                    spots.push(CbSpot {
                        of: firer.0,
                        pos: gun,
                        at: gun,
                        err_mm: 0,
                        heard_mm: 0,
                        evidence: 1,
                        shots: 1,
                        decay_at: whole_second(tick + decay, hz),
                        mark_at: 0,
                        marked: false,
                    });
                    spots.len() - 1
                }
            };
            // K2: enough evidence: the marker is due after the delay.
            let s = &mut self.players[e].cb[k];
            if !s.marked && s.mark_at == 0 && s.evidence as u32 >= to_mark {
                s.mark_at = whole_second(tick + delay, hz);
            }
        }
        if heard > 0 {
            let c = &mut self.players[firer.0 as usize].s11a2;
            c.cb_heard = c.cb_heard.saturating_add(1);
        }
    }

    /// K2, K3: on whole seconds, each player's spots decay one piece of
    /// evidence per `cb_decay_s` (gone at none) and due markers are made.
    pub(crate) fn cb_tick(&mut self, ents: &Entities, tick: u64) {
        let hz = self.setup.tick_hz.max(1) as u64;
        if !tick.is_multiple_of(hz) || self.players.iter().all(|p| p.cb.is_empty()) {
            return;
        }
        let Some(fm) = self.setup.rules.fire_mission.clone() else {
            return;
        };
        let size = Fx::from_int(self.setup.map.size_m);
        let seed = self.setup.seed;
        for e in 0..self.players.len() {
            if self.players[e].cb.is_empty() {
                continue;
            }
            // K3: decay, in spot order.
            let pl = &mut self.players[e];
            let mut dropped = 0u32;
            pl.cb.retain_mut(|s| {
                while tick >= s.decay_at && s.evidence > 0 {
                    s.evidence -= 1;
                    s.decay_at += fm.cb_decay_s as u64 * hz;
                }
                let keep = s.evidence > 0;
                dropped += (!keep) as u32;
                keep
            });
            pl.s11a2.cb_dropped = pl.s11a2.cb_dropped.saturating_add(dropped);
            // K2: due markers, in spot order.
            for k in 0..self.players[e].cb.len() {
                let s = self.players[e].cb[k];
                if s.mark_at == 0 || tick < s.mark_at {
                    continue;
                }
                let Some((_, d)) = self.nearest_soldier(ents, PlayerId(e as u8), s.pos) else {
                    // Nobody alive to mark it: a second later.
                    self.players[e].cb[k].mark_at = tick + hz;
                    continue;
                };
                let heard_mm = mm(FxVec2::new(d, Fx::ZERO));
                let err_mm = (heard_mm as u64 * fm.cb_error_pct as u64 / 100) as u32;
                // The offset: a uniform bearing, then a uniform length in
                // 0..=err (not the mission aim offset's fixed length).
                let mut rng = RngStream::new(
                    seed,
                    tick,
                    SimId(((e as u32) << 8) | k as u32),
                    Purpose::CB_MARK,
                );
                let a = rng.range_fx(Fx::ZERO, Fx::TAU);
                let len = rng.range_i32(0, err_mm.min(i32::MAX as u32) as i32) as u32;
                let (sn, cs) = a.sin_cos();
                let l = from_mm(len);
                let at = s.pos + FxVec2::new(cs.mul(l), sn.mul(l));
                let at = FxVec2::new(at.x.clamp(Fx::ZERO, size), at.y.clamp(Fx::ZERO, size));
                let pl = &mut self.players[e];
                let s = &mut pl.cb[k];
                s.at = at;
                s.err_mm = err_mm;
                s.heard_mm = heard_mm;
                s.marked = true;
                s.mark_at = 0;
                pl.s11a2.cb_marks = pl.s11a2.cb_marks.saturating_add(1);
                pl.s11a2.cb_err_first_mm = pl.s11a2.cb_err_first_mm.saturating_add(err_mm as u64);
            }
        }
    }

    /// K4: player `p` holds a marker within `intel_snap_m` of `pt`.
    pub(crate) fn near_own_marker(&self, p: PlayerId, pt: FxVec2) -> bool {
        let Some(fm) = self.setup.rules.fire_mission.as_ref() else {
            return false;
        };
        let snap = Fx::from_int(fm.intel_snap_m as i32);
        let r = FxVec2::new(snap, Fx::ZERO).length_sq_raw();
        self.players.get(p.0 as usize).is_some_and(|pl| {
            pl.cb
                .iter()
                .any(|s| s.marked && (s.at - pt).length_sq_raw() <= r)
        })
    }
}

/// Load check of `holder`'s spots at `tick` (spec 3; one rule per clause).
pub(crate) fn validate_spots(
    g: &MatchState,
    holder: PlayerId,
    spots: &[CbSpot],
    tick: u64,
) -> Result<(), &'static str> {
    if spots.is_empty() {
        return Ok(());
    }
    let s = &g.setup;
    let Some(fm) = s.rules.fire_mission.as_ref() else {
        return Err("counter-battery spots without fire-mission rules");
    };
    if spots.len() > MAX_CB_SPOTS {
        return Err("too many counter-battery spots");
    }
    let size = Fx::from_int(s.map.size_m);
    let on_map = |p: FxVec2| p.x >= Fx::ZERO && p.y >= Fx::ZERO && p.x <= size && p.y <= size;
    // A listener can stand anywhere on the map: `heard_mm` (and with
    // `cb_error_pct` 100, `err_mm`) can reach the diagonal (review A-2).
    let size_mm = (s.map.size_m.max(0) as u64 * 1000 * 3 / 2).min(u32::MAX as u64) as u32;
    let hz = s.tick_hz.max(1) as u64;
    let decay_max = tick.saturating_add((fm.cb_decay_s as u64 + 1) * hz);
    let mark_max = tick.saturating_add((fm.cb_delay_s as u64 + 1) * hz);
    for c in spots {
        let ok = (c.of as usize) < s.players.len()
            && c.of != holder.0
            && on_map(c.pos)
            && on_map(c.at)
            && c.err_mm <= size_mm
            && c.heard_mm <= size_mm
            && c.evidence >= 1
            // Upper bounds first, and no overflow on hostile values (A-1).
            && c.decay_at <= decay_max
            && tick < c.decay_at.saturating_add(hz)
            && (c.mark_at == 0
                || (c.mark_at <= mark_max && tick < c.mark_at.saturating_add(hz) && !c.marked))
            && (!c.marked || c.shots >= 1);
        if !ok {
            return Err("bad counter-battery spot");
        }
    }
    Ok(())
}
