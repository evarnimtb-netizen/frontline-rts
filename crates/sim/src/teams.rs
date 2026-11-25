//! S17a teams (docs/slices/S17-team-play.md 3): the only place team logic
//! lives. Every player has a team number ([`frontline_content::PlayerSetup`]
//! `team`); players with the same number are allies, every other player is
//! hostile. The table is derived from the setup (never saved; the setup is
//! hashed) and rebuilt at match start and at load.
//!
//! The code asks three questions:
//! * [`Teams::hostile`]: "is this an enemy" (targets, triggers, capture of
//!   vehicles and guns, contesting points). With every player alone on his
//!   team it is `a != b`, today's test.
//! * [`Teams::allied`]: the negation (own side, `a` included).
//! * [`Teams::sees`]: "does player `p` see it" from a `Combat::seen_by` mask.
//!   With `[teams] shared_sight` it reads the bits of `p`'s whole team; alone
//!   on his team that is the bit `1 << p` the code tested before.
//!
//! "Is this mine" (orders, control, services, pop, cards) stays an owner
//! test and never comes here (rule T8).

use crate::ecs::PlayerId;

/// Players a team table holds (one bit each in a `u16` sight mask).
pub const MAX_TEAM_PLAYERS: usize = 16;

/// The team table of a match (module docs).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Teams {
    /// Number of players.
    n: u8,
    /// Team number per player.
    team: [u8; MAX_TEAM_PLAYERS],
    /// `allies[p]`: bit mask of `p`'s team, `p` included.
    allies: [u16; MAX_TEAM_PLAYERS],
    /// `sight[p]`: whose spots `p` uses: `allies[p]` with shared sight,
    /// else `1 << p`.
    sight: [u16; MAX_TEAM_PLAYERS],
}

impl Default for Teams {
    /// Every player alone on his team (sixteen slots).
    fn default() -> Teams {
        Teams::alone(MAX_TEAM_PLAYERS)
    }
}

/// Index into the per-player arrays: out-of-range ids (never produced by a
/// validated state) fold into the table instead of panicking.
#[inline]
fn ix(p: u8) -> usize {
    p as usize % MAX_TEAM_PLAYERS
}

impl Teams {
    /// `n` players, each alone on his team.
    pub fn alone(n: usize) -> Teams {
        let teams: Vec<u8> = (0..n.min(MAX_TEAM_PLAYERS) as u8).collect();
        Teams::new(&teams, true)
    }

    /// The table for `team[p]` per player; `shared_sight` is `[teams]
    /// shared_sight` (T2).
    pub fn new(team: &[u8], shared_sight: bool) -> Teams {
        let n = team.len().min(MAX_TEAM_PLAYERS);
        // Unused slots are alone (and seen only by themselves).
        let team_of = |p: usize| if p < n { team[p] } else { u8::MAX };
        let allies = |p: usize| -> u16 {
            let mates = (0..n)
                .filter(|&q| p < n && team[q] == team[p])
                .fold(0u16, |m, q| m | 1 << q);
            mates | 1 << p
        };
        let teams: [u8; MAX_TEAM_PLAYERS] = std::array::from_fn(team_of);
        let allies: [u16; MAX_TEAM_PLAYERS] = std::array::from_fn(allies);
        let sight = std::array::from_fn(|p| {
            if shared_sight && p < n {
                allies[p]
            } else {
                1 << p
            }
        });
        Teams {
            n: n as u8,
            team: teams,
            allies,
            sight,
        }
    }

    /// The table of a match setup.
    pub fn of_setup(setup: &frontline_content::MatchSetup) -> Teams {
        let team: Vec<u8> = setup.players.iter().map(|p| p.team).collect();
        Teams::new(&team, setup.rules.teams.shared_sight != 0)
    }

    /// Number of players.
    pub fn len(&self) -> usize {
        self.n as usize
    }

    /// True for a table of no players.
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    /// `p`'s team number.
    pub fn team(&self, p: u8) -> u8 {
        self.team[ix(p)]
    }

    /// Bit mask of `p`'s team, `p` included.
    pub fn allies_mask(&self, p: u8) -> u16 {
        self.allies[ix(p)]
    }

    /// Bit mask of the `seen_by` bits `p` reads (T2).
    pub fn sight_mask(&self, p: u8) -> u16 {
        self.sight[ix(p)]
    }

    /// Is `b` an enemy of `a` (T1)? Never true for `a == b`.
    #[inline]
    pub fn hostile(&self, a: u8, b: u8) -> bool {
        self.allies[ix(a)] & (1 << ix(b)) == 0
    }

    /// [`Teams::hostile`] on player ids.
    #[inline]
    pub fn hostile_id(&self, a: PlayerId, b: PlayerId) -> bool {
        self.hostile(a.0, b.0)
    }

    /// Is `b` on `a`'s team (`a` included)?
    #[inline]
    pub fn allied(&self, a: u8, b: u8) -> bool {
        !self.hostile(a, b)
    }

    /// The bits of a mine's `known` mask (one per player, players 0..8)
    /// that tell `p` where it is (T2: his team's with shared sight).
    #[inline]
    pub fn known_mask(&self, p: u8) -> u8 {
        (self.sight[ix(p)] & 0xff) as u8
    }

    /// Does player `p` see a body with this `Combat::seen_by` mask (T2)?
    #[inline]
    pub fn sees(&self, p: u8, seen_by: u16) -> bool {
        seen_by & self.sight[ix(p)] != 0
    }

    /// A target test: `t_owner` is hostile to `p` and `p` sees him
    /// (`seen_by`, T1 and T2 together).
    #[inline]
    pub fn enemy_seen(&self, p: u8, t_owner: u8, seen_by: u16) -> bool {
        self.hostile(p, t_owner) && self.sees(p, seen_by)
    }

    /// Is `owner` (`None`: nobody) hostile to `p`? An unowned thing is not.
    pub fn hostile_owner(&self, p: PlayerId, owner: Option<PlayerId>) -> bool {
        owner.is_some_and(|o| self.hostile(p.0, o.0))
    }

    /// `p`'s hostile players, ascending slot.
    pub fn foes(&self, p: u8) -> impl Iterator<Item = u8> + '_ {
        (0..self.n).filter(move |&q| self.hostile(p, q))
    }

    /// `p`'s allies other than `p`, ascending slot.
    pub fn mates(&self, p: u8) -> impl Iterator<Item = u8> + '_ {
        (0..self.n).filter(move |&q| q != p && !self.hostile(p, q))
    }

    /// Does `p` have an ally (a team of two or more)? The four `[teams]`
    /// `_pct` keys apply only then (T9).
    pub fn in_team(&self, p: u8) -> bool {
        self.allies[ix(p)].count_ones() > 1
    }

    /// The winner of a match `w` (a player of the winning team) counts as a
    /// win for `p` when they are allies.
    pub fn won_by(&self, p: u8, winner: u8) -> bool {
        self.allied(p, winner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_player_alone_is_the_one_bit_and_the_owner_test() {
        let t = Teams::alone(10);
        for p in 0..10u8 {
            assert_eq!(t.allies_mask(p), 1 << p);
            assert_eq!(t.sight_mask(p), 1 << p);
            assert!(!t.in_team(p));
            for q in 0..10u8 {
                assert_eq!(t.hostile(p, q), p != q);
                assert_eq!(t.sees(p, 1 << q), p == q);
            }
        }
        assert_eq!(
            t.foes(1).collect::<Vec<_>>(),
            vec![0, 2, 3, 4, 5, 6, 7, 8, 9]
        );
    }

    #[test]
    fn two_v_two_allies_and_shared_sight() {
        let t = Teams::new(&[0, 0, 1, 1], true);
        assert!(!t.hostile(0, 1) && !t.hostile(1, 0));
        assert!(t.hostile(0, 2) && t.hostile(3, 1));
        assert_eq!(t.allies_mask(0), 0b0011);
        assert_eq!(t.allies_mask(2), 0b1100);
        assert!(t.sees(0, 0b0010), "my ally's spot is mine");
        assert!(!t.sees(0, 0b0100));
        assert!(t.in_team(3));
        assert_eq!(t.foes(1).collect::<Vec<_>>(), vec![2, 3]);
        assert_eq!(t.mates(1).collect::<Vec<_>>(), vec![0]);
        assert!(t.won_by(1, 0) && !t.won_by(2, 0));
        let own = Teams::new(&[0, 0, 1, 1], false);
        assert!(!own.sees(0, 0b0010), "shared_sight 0: only my own spots");
        assert!(own.sees(0, 0b0001));
        assert!(!own.hostile(0, 1));
    }
}
