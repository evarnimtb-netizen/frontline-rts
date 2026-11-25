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
//!   `capture_max_men` adds `capture_extra_man_pct` speed.
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
//! S17a teams (`crate::teams`, docs/slices/S17-team-play.md 4): men are
//! counted per team and a point is contested when two hostile teams are
//! present (T4); one team present takes it for its member with the most men
//! there (ties: lowest slot), and an ally on his ally's point acts as the
//! owner; a defeated player's men count for nobody. Income is per player (T5, `[teams] income_pct` for a player with
//! an ally). With `[teams] team_tickets` the drain is the gap between the
//! best standing hostile team's held value and the player's team's (T6).
//! A team is out when all its members are out; the last team standing wins
//! (T7), and with `wiped_waits` a wiped player is not out while an ally
//! stands. With every player alone on his team each rule is the one above.
//!
//! Points are visited in index order and players in slot order, all
//! integer arithmetic, so the result is the same on every client.

use serde::{Deserialize, Serialize};

use crate::ecs::{Entities, EntityKind, PlayerId};
use crate::fx::{Fx, FxVec2};
use crate::game::{MatchState, Order, Outcome};

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

/// Progress per tick for `men` capturers (0 for none); each man beyond the
/// first adds `extra_pct` (the rules' `capture.extra_man_pct`).
pub fn capture_rate(men: u32, max_men: u8, extra_pct: u32) -> u32 {
    match men.min(max_men as u32) {
        0 => 0,
        n => 100 + extra_pct * (n - 1),
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

    /// S17a A1: the hostile player the AI's works, belts and guns face:
    /// with `[ai.team] foe_first` 0 the standing hostile player whose base
    /// is nearest `me`'s (ties: lowest slot), with 1 the lowest standing
    /// hostile slot; with none standing, the lowest hostile slot. In a 1v1
    /// that is the other player, as before S17.
    pub fn main_foe(&self, me: PlayerId) -> Option<PlayerId> {
        let standing: Vec<u8> = self
            .teams
            .foes(me.0)
            .filter(|&q| {
                self.players
                    .get(q as usize)
                    .is_some_and(|p| p.defeat.is_none())
            })
            .collect();
        if standing.is_empty() {
            return self.teams.foes(me.0).next().map(PlayerId);
        }
        if self.setup.rules.ai.team.foe_first != 0 {
            return standing.first().copied().map(PlayerId);
        }
        let mine = self.base_of(me);
        standing
            .iter()
            .map(|&q| ((self.base_of(PlayerId(q)) - mine).length_sq_raw(), q))
            .min()
            .map(|(_, q)| PlayerId(q))
    }

    /// S17a: point value held by `player`'s team (all members).
    pub fn team_value(&self, player: PlayerId) -> u32 {
        (0..self.players.len() as u8)
            .filter(|&q| self.teams.allied(player.0, q))
            .map(|q| self.held_value(PlayerId(q)))
            .sum()
    }

    /// `player`'s manpower income per minute right now (saturating: the
    /// content ranges allow products past `u32`).
    pub fn income_per_min(&self, player: PlayerId) -> u32 {
        let r = &self.setup.rules;
        let mut base = r.income_per_min as u64
            + r.point_income_per_min as u64 * self.held_value(player) as u64;
        // T5: a player with an ally earns the `[teams] income_pct` share.
        if self.teams.in_team(player.0) {
            base = base * r.teams.income_pct as u64 / 100;
        }
        let ps = &self.setup.players[player.0 as usize];
        let income = if ps.is_ai {
            base * r.difficulty.get(ps.difficulty).income_pct as u64 / 100
        } else {
            base
        };
        income.min(u32::MAX as u64) as u32
    }

    /// `player`'s ticket loss per minute right now: the point value gap to
    /// the best standing hostile player, times the drain rate. T6: with
    /// `[teams] team_tickets` the gap is between teams (each team's held
    /// value; a hostile team counts while one of its members stands).
    pub fn drain_per_min(&self, player: PlayerId) -> u32 {
        let team = self.setup.rules.teams.team_tickets != 0;
        let value = |q: PlayerId| {
            if team {
                self.team_value(q)
            } else {
                self.held_value(q)
            }
        };
        let mine = value(player);
        let best = (0..self.players.len())
            .filter(|&q| self.teams.hostile(player.0, q as u8) && self.players[q].defeat.is_none())
            .map(|q| value(PlayerId(q as u8)))
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
        // S17 review (O1): a defeated player's men (surrendered, or out of
        // tickets with an ally still in) neither capture nor contest. In a
        // 1v1 the first defeat ends the match, so this never applies there.
        let out: Vec<bool> = self.players.iter().map(|p| p.defeat.is_some()).collect();
        for id in ents.soldiers() {
            let i = id.index();
            // S8a: men riding a transport neither capture nor contest.
            if ents.unit[i].riding.is_some() || out[ents.owner[i].0 as usize] {
                continue;
            }
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
        let teams = self.teams;
        for (k, c) in self.points.iter_mut().enumerate() {
            let here = &men[k * np..(k + 1) * np];
            // T4: the first present player's team; contested when a hostile
            // player is present too. The team's men are summed and the point
            // goes to its member with the most men (ties: lowest slot).
            let mut present = here.iter().enumerate().filter(|(_, n)| **n > 0);
            let first = present.next();
            c.contested =
                first.is_some_and(|(p0, _)| present.any(|(q, _)| teams.hostile(p0 as u8, q as u8)));
            if c.contested {
                continue;
            }
            let first = first.map(|(p0, _)| {
                let mut lead = (p0, here[p0]);
                let mut sum = 0u32;
                for (q, &n) in here.iter().enumerate() {
                    if n > 0 && teams.allied(p0 as u8, q as u8) {
                        sum += n;
                        if n > lead.1 {
                            lead = (q, n);
                        }
                    }
                }
                (lead.0, sum)
            });
            let Some((p, n)) = first else {
                // Empty: progress fades at the one-man rate.
                c.progress = c.progress.saturating_sub(capture_rate(
                    1,
                    r.capture_max_men,
                    r.capture_extra_man_pct,
                ));
                if c.progress == 0 {
                    c.capturer = None;
                }
                continue;
            };
            let p = PlayerId(p as u8);
            let rate = capture_rate(n, r.capture_max_men, r.capture_extra_man_pct);
            if c.capturer.is_some_and(|q| teams.hostile(q.0, p.0)) {
                // A hostile team's progress is worn down first.
                c.progress = c.progress.saturating_sub(rate);
                if c.progress == 0 {
                    c.capturer = None;
                }
            } else if c.owner.is_none_or(|o| teams.hostile(o.0, p.0)) {
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
        let wiped: Vec<bool> = alive
            .iter()
            .enumerate()
            .map(|(p, a)| {
                let f = &self.setup.factions[self.setup.players[p].faction as usize];
                let cheapest = f
                    .roster
                    .iter()
                    .map(|&t| self.setup.squad_types[t as usize].cost)
                    .min()
                    .unwrap_or(u32::MAX);
                *a == 0 && self.players[p].manpower < cheapest
            })
            .collect();
        // T7: with `wiped_waits` a wiped player stays in while an ally stands
        // who is neither out nor wiped himself.
        let waits = self.setup.rules.teams.wiped_waits != 0;
        let ally_stands = |p: usize| {
            (0..np).any(|q| {
                q != p
                    && self.teams.allied(p as u8, q as u8)
                    && self.players[q].defeat.is_none()
                    && self.players[q].tickets > 0
                    && !wiped[q]
            })
        };
        let stands: Vec<bool> = (0..np).map(|p| waits && ally_stands(p)).collect();
        for p in 0..np {
            let pl = &mut self.players[p];
            if pl.defeat.is_some() {
                continue;
            }
            if pl.tickets == 0 {
                pl.defeat = Some(Defeat::Tickets);
            } else if wiped[p] && !stands[p] {
                pl.defeat = Some(Defeat::Wiped);
            }
        }
        self.update_outcome();
    }

    /// The last team standing wins (T7; `winner` is its lowest standing
    /// slot, callers ask `Teams::won_by`); nobody standing is a draw.
    pub(crate) fn update_outcome(&mut self) {
        if self.outcome != Outcome::Running {
            return;
        }
        let teams = self.teams;
        let mut standing = self
            .players
            .iter()
            .enumerate()
            .filter(|(_, p)| p.defeat.is_none())
            .map(|(w, _)| w as u8);
        match standing.next() {
            None => self.outcome = Outcome::Draw,
            Some(w) => {
                if standing.all(|q| teams.allied(w, q)) {
                    self.outcome = Outcome::Won {
                        winner: PlayerId(w),
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_rate_grows_with_men_up_to_the_cap() {
        assert_eq!(capture_rate(0, 4, 50), 0);
        assert_eq!(capture_rate(1, 4, 50), 100);
        assert_eq!(capture_rate(2, 4, 50), 150);
        assert_eq!(capture_rate(4, 4, 50), 250);
        assert_eq!(capture_rate(9, 4, 50), 250);
        // 20 s at 30 Hz: one man 600 ticks, four men 240 ticks.
        assert_eq!(capture_full(600).div_ceil(capture_rate(1, 4, 50)), 600);
        assert_eq!(capture_full(600).div_ceil(capture_rate(4, 4, 50)), 240);
    }
}
