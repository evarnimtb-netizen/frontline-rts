//! Match rules (S1): capture points, manpower income, ticket drain and
//! victory. Runs serially once per tick at the end of the Status stage, after
//! deaths and corpse removal, so it sees the tick's final positions.
//!
//! * **Capture.** A point counts the living soldiers of each player within
//!   `capture_radius` (soldiers of retreating squads do not count; vehicles
//!   neither capture nor contest, S3). With
//!   two or more players present it is contested and frozen. With one
//!   player `p` present who does not own it, progress toward `p` grows by
//!   [`capture_rate`] per tick; at [`capture_full`] the point flips to `p`.
//!   Progress toward someone else is first worn back down at the same rate.
//!   The owner alone on his point wears an enemy's progress down; an empty
//!   point loses progress at the one-man rate. Progress is in percent-ticks
//!   of one man's work: one man takes `capture_ticks`, each extra man up to
//!   `capture_max_men` adds [`CAPTURE_EXTRA_MAN_PCT`] speed.
//! * **Income.** Each player earns `income_per_min` plus
//!   `point_income_per_min` per point value held, scaled by the AI
//!   difficulty's `income_pct` for AI players. Fractions carry over in
//!   `PlayerState::income_acc` (units of 1/(100 · ticks per minute)).
//! * **Tickets.** A player whose held point value trails the best other
//!   standing player by `d` loses `d · ticket_drain_per_min_per_point`
//!   tickets per minute (fractions carried in `drain_acc`).
//! * **Victory.** A player is out when his tickets reach 0, when he has no
//!   soldiers and no vehicles, no call-ins on the way and too little manpower for his
//!   cheapest squad, or when he surrenders. The last player standing wins;
//!   if the rest are knocked out in the same tick it is a draw.
//!
//! Points are visited in index order and players in slot order, all
//! integer arithmetic, so the result is the same on every client.

use serde::{Deserialize, Serialize};

use crate::ecs::{Entities, EntityKind, PlayerId};
use crate::fx::{Fx, FxVec2};
use crate::game::{MatchState, Order, Outcome};

/// Extra capture speed per man beyond the first, in percent.
pub const CAPTURE_EXTRA_MAN_PCT: u32 = 50;

/// Why a player is out of the match.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[repr(u8)]
pub enum Defeat {
    /// Tickets ran out.
    Tickets = 1,
    /// No soldiers left and nothing to buy more with.
    Wiped = 2,
    Surrendered = 3,
}

/// Progress needed to capture a point (percent-ticks).
pub fn capture_full(capture_ticks: u32) -> u32 {
    capture_ticks.saturating_mul(100)
}

/// Progress per tick for `men` capturers (0 for none).
pub fn capture_rate(men: u32, max_men: u8) -> u32 {
    match men.min(max_men as u32) {
        0 => 0,
        n => 100 + CAPTURE_EXTRA_MAN_PCT * (n - 1),
    }
}

impl MatchState {
    /// Point value `player` holds.
    pub fn held_value(&self, player: PlayerId) -> u32 {
        self.points
            .iter()
            .zip(&self.setup.map.points)
            .filter(|(c, _)| c.owner == Some(player))
            .map(|(_, s)| s.value)
            .sum()
    }

    /// `player`'s manpower income per minute right now (saturating: the
    /// content ranges allow products past `u32`).
    pub fn income_per_min(&self, player: PlayerId) -> u32 {
        let r = &self.setup.rules;
        let base = r.income_per_min as u64
            + r.point_income_per_min as u64 * self.held_value(player) as u64;
        let ps = &self.setup.players[player.0 as usize];
        let income = if ps.is_ai {
            base * r.difficulty.get(ps.difficulty).income_pct as u64 / 100
        } else {
            base
        };
        income.min(u32::MAX as u64) as u32
    }

    /// `player`'s ticket loss per minute right now: the point value gap to
    /// the best other standing player, times the drain rate.
    pub fn drain_per_min(&self, player: PlayerId) -> u32 {
        let mine = self.held_value(player);
        let best = (0..self.players.len())
            .filter(|&q| q != player.0 as usize && self.players[q].defeat.is_none())
            .map(|q| self.held_value(PlayerId(q as u8)))
            .max()
            .unwrap_or(0);
        best.saturating_sub(mine)
            .saturating_mul(self.setup.rules.ticket_drain_per_min_per_point)
    }

    /// Capture, income, tickets and victory for this tick (module docs).
    pub(crate) fn update_rules(&mut self, ents: &Entities) {
        if self.outcome != Outcome::Running {
            return;
        }
        self.update_capture(ents);
        let per_min = 60 * self.setup.tick_hz;
        for p in 0..self.players.len() {
            let id = PlayerId(p as u8);
            if self.players[p].defeat.is_some() {
                continue;
            }
            let held = self.points.iter().filter(|c| c.owner == Some(id)).count();
            let income = self.income_per_min(id);
            let drain = self.drain_per_min(id);
            let pl = &mut self.players[p];
            pl.point_ticks += held as u64;
            // Income in 1/(100 · per_min) manpower, drain in 1/per_min tickets.
            // Sums in u64: both accumulators stay below their divisor.
            let den = 100 * per_min as u64;
            let acc = pl.income_acc as u64 + income as u64 * 100;
            let gain = (acc / den).min(u32::MAX as u64) as u32;
            pl.manpower = pl.manpower.saturating_add(gain);
            pl.income_acc = (acc % den) as u32;
            let acc = pl.drain_acc as u64 + drain as u64;
            let loss = (acc / per_min as u64).min(u32::MAX as u64) as u32;
            pl.tickets = pl.tickets.saturating_sub(loss);
            pl.drain_acc = (acc % per_min as u64) as u32;
        }
        self.check_defeats(ents);
    }

    fn update_capture(&mut self, ents: &Entities) {
        let r = &self.setup.rules;
        let radius: Fx = r.capture_radius.into();
        let radius_sq = FxVec2::new(radius, Fx::ZERO).length_sq_raw();
        let full = capture_full(r.capture_ticks);
        let np = self.players.len();
        let mut men = vec![0u32; np * self.points.len()];
        for id in ents.soldiers() {
            let i = id.index();
            let retreating = ents.squad[i]
                .and_then(|s| self.squads.get(&s))
                .is_some_and(|s| s.order == Order::Retreat);
            if retreating {
                continue;
            }
            let pos = ents.transform[i].pos.xy();
            let owner = ents.owner[i].0 as usize;
            for (k, ps) in self.setup.map.points.iter().enumerate() {
                if (pos - FxVec2::from(ps.pos)).length_sq_raw() <= radius_sq {
                    men[k * np + owner] += 1;
                }
            }
        }
        for (k, c) in self.points.iter_mut().enumerate() {
            let here = &men[k * np..(k + 1) * np];
            let mut present = here.iter().enumerate().filter(|(_, n)| **n > 0);
            let first = present.next();
            c.contested = present.next().is_some();
            if c.contested {
                continue;
            }
            let Some((p, &n)) = first else {
                // Empty: progress fades at the one-man rate.
                c.progress = c
                    .progress
                    .saturating_sub(capture_rate(1, r.capture_max_men));
                if c.progress == 0 {
                    c.capturer = None;
                }
                continue;
            };
            let p = PlayerId(p as u8);
            let rate = capture_rate(n, r.capture_max_men);
            if c.capturer.is_some_and(|q| q != p) {
                // Someone else's progress is worn down first.
                c.progress = c.progress.saturating_sub(rate);
                if c.progress == 0 {
                    c.capturer = None;
                }
            } else if c.owner != Some(p) {
                c.capturer = Some(p);
                c.progress = (c.progress + rate).min(full);
                if c.progress >= full {
                    c.owner = Some(p);
                    c.capturer = None;
                    c.progress = 0;
                }
            }
        }
    }

    /// Marks players who are out (module docs) and settles the outcome.
    fn check_defeats(&mut self, ents: &Entities) {
        let np = self.players.len();
        let mut alive = vec![0u32; np];
        for id in ents.soldiers() {
            alive[ents.owner[id.index()].0 as usize] += 1;
        }
        // A player with a live vehicle is not wiped out (S3).
        for id in self.vehicles.keys() {
            if ents.kind[id.index()] == EntityKind::Vehicle {
                alive[ents.owner[id.index()].0 as usize] += 1;
            }
        }
        for (p, a) in alive.iter().enumerate() {
            let f = &self.setup.factions[self.setup.players[p].faction as usize];
            let cheapest = f
                .roster
                .iter()
                .map(|&t| self.setup.squad_types[t as usize].cost)
                .min()
                .unwrap_or(u32::MAX);
            let pl = &mut self.players[p];
            if pl.defeat.is_some() {
                continue;
            }
            if pl.tickets == 0 {
                pl.defeat = Some(Defeat::Tickets);
            } else if *a == 0 && pl.call_ins.is_empty() && pl.manpower < cheapest {
                pl.defeat = Some(Defeat::Wiped);
            }
        }
        self.update_outcome();
    }

    /// The last player standing wins; nobody standing is a draw.
    pub(crate) fn update_outcome(&mut self) {
        if self.outcome != Outcome::Running {
            return;
        }
        let mut standing = self
            .players
            .iter()
            .enumerate()
            .filter(|(_, p)| p.defeat.is_none());
        match (standing.next(), standing.next()) {
            (Some((w, _)), None) => {
                self.outcome = Outcome::Won {
                    winner: PlayerId(w as u8),
                }
            }
            (None, _) => self.outcome = Outcome::Draw,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_rate_grows_with_men_up_to_the_cap() {
        assert_eq!(capture_rate(0, 4), 0);
        assert_eq!(capture_rate(1, 4), 100);
        assert_eq!(capture_rate(2, 4), 150);
        assert_eq!(capture_rate(4, 4), 250);
        assert_eq!(capture_rate(9, 4), 250);
        // 20 s at 30 Hz: one man 600 ticks, four men 240 ticks.
        assert_eq!(capture_full(600).div_ceil(capture_rate(1, 4)), 600);
        assert_eq!(capture_full(600).div_ceil(capture_rate(4, 4)), 240);
    }
}
