//! What the match sim thread copies out after each tick (§4.1 rules 1, 3,
//! 7). Plain Rust, no Godot types, so it is unit tested.
//!
//! Two kinds of data leave the sim thread:
//!
//! * [`MatchFrame`]: the continuous state (soldiers, corpses, rounds in
//!   flight, squads, players, points, outcome). It goes through the triple
//!   buffer; the presenter only ever sees the newest one and interpolates
//!   between the last few, so frames may be skipped.
//! * [`TickEvents`]: what happened *in* a tick (rounds fired, HE bursts,
//!   changed terrain tiles with their heights). These must not be lost when
//!   the presenter skips frames, so they go through a channel and are
//!   drained every presenter frame.
//!
//! Fog of war is applied here, in Rust (§5.12.3): every soldier, squad,
//! round and shot carries whether the local player sees it, and the
//! presenter draws, lists and plays only those (own ones always).

use std::collections::BTreeMap;
use std::time::Instant;

use frontline_content::{MatchSetup, SquadRole, WeaponClass};
use frontline_sim::projectile::SUBSTEP_HZ;
use frontline_sim::{
    CorpseSnapshot, EntityKind, ExplosionSnapshot, NavPhase, Order, OrderKind, Outcome,
    PlayerSnapshot, PointSnapshot, World, tile_heights_f32,
};

/// A squad as the presenter needs it (cards, markers, overlays, events).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SquadView {
    pub id: u32,
    pub type_idx: u16,
    pub owner: u8,
    pub men: u8,
    pub order: OrderKind,
    pub phase: NavPhase,
    /// Sim id of the leader (`members[0]`), for the interpolated anchor.
    pub leader: u32,
    /// Leader position (sim axes) and where the squad is headed.
    pub anchor: [f32; 3],
    pub goal: [f32; 2],
    /// Sum of member hit points; suppression summed over members (0..=100
    /// each); members pinned.
    pub hp_sum: u32,
    pub suppression_sum: u32,
    pub pinned: u8,
    /// MG team still setting up its guns after a move.
    pub setup: bool,
    /// Some member is seen by the local player (own squads: always).
    pub visible: bool,
    pub dig_pct: u8,
    /// Dig orders: the trench polyline (sim axes).
    pub dig_line: Vec<[f32; 2]>,
    /// Move, attack-move and barrage target.
    pub target: Option<[f32; 2]>,
    /// Barrage rounds left.
    pub rounds: u16,
}

/// A round or shell in flight.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct RoundView {
    pub pos: [f32; 3],
    pub vel: [f32; 3],
    /// Seconds since it left the muzzle (end of this tick).
    pub age: f32,
    pub he: bool,
    pub visible: bool,
}

/// Presenter-side running stats the sim does not keep (end screen).
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct PlayerExtra {
    /// Squads that arrived after the start (call-ins).
    pub squads_called: u32,
    /// Trench dug (m): each finished or abandoned Dig order's line length ×
    /// its progress.
    pub trench_m: f32,
}

/// One published tick, in sim axes (x east, y north, z up).
#[derive(Clone, Debug)]
pub struct MatchFrame {
    pub tick: u64,
    pub step_ms: f32,
    /// Scheduled start of the tick that produced this frame (interpolation
    /// runs on these stamps).
    pub sched: Instant,
    pub published: Instant,
    pub paused: bool,
    /// Live soldiers, ascending id: `[x, y, z, yaw]`.
    pub soldiers: Vec<[f32; 4]>,
    pub ids: Vec<u32>,
    pub owner: Vec<u8>,
    pub squad: Vec<u32>,
    /// Planar speed (m/s).
    pub speeds: Vec<f32>,
    /// `Stance as u8`: 0 stand, 1 crouch, 2 prone.
    pub stance: Vec<u8>,
    /// Seen by the local player (own soldiers: always).
    pub visible: Vec<bool>,
    pub corpses: Vec<CorpseSnapshot>,
    /// Per corpse: the local player saw him when he died (own: always).
    pub corpse_visible: Vec<bool>,
    pub rounds: Vec<RoundView>,
    pub squads: Vec<SquadView>,
    pub players: Vec<PlayerSnapshot>,
    pub points: Vec<PointSnapshot>,
    pub outcome: Outcome,
    pub extra: Vec<PlayerExtra>,
}

impl MatchFrame {
    pub fn empty() -> MatchFrame {
        let now = Instant::now();
        MatchFrame {
            tick: 0,
            step_ms: 0.0,
            sched: now,
            published: now,
            paused: false,
            soldiers: Vec::new(),
            ids: Vec::new(),
            owner: Vec::new(),
            squad: Vec::new(),
            speeds: Vec::new(),
            stance: Vec::new(),
            visible: Vec::new(),
            corpses: Vec::new(),
            corpse_visible: Vec::new(),
            rounds: Vec::new(),
            squads: Vec::new(),
            players: Vec::new(),
            points: Vec::new(),
            outcome: Outcome::Running,
            extra: Vec::new(),
        }
    }

    /// Refills `self` from `world` (reuses allocations).
    pub fn fill(&mut self, world: &World, ctx: &mut FrameCtx, step_ms: f32, sched: Instant) {
        let snap = world.snapshot();
        let local = ctx.local;
        let bit = 1u16 << local;
        self.tick = snap.tick;
        self.step_ms = step_ms;
        self.sched = sched;
        self.soldiers.clone_from(&snap.soldiers);
        self.ids.clear();
        self.ids.extend(snap.soldier_ids.iter().map(|id| id.0));
        self.owner.clone_from(&snap.soldier_owner);
        self.squad.clone_from(&snap.soldier_squad);
        self.stance.clear();
        self.stance
            .extend(snap.soldier_stance.iter().map(|s| *s as u8));
        self.visible.clear();
        self.visible
            .extend(snap.soldier_visible.iter().map(|v| v & bit != 0));
        let ents = world.entities();
        let vel = ents.velocity();
        self.speeds.clear();
        for &id in &self.ids {
            let [vx, vy] = vel[id as usize].lin.to_f32();
            self.speeds.push((vx * vx + vy * vy).sqrt());
        }
        // Who the local player saw this tick: a corpse is shown when he was
        // seen as he fell (it stays shown for its 30 s).
        ctx.seen.resize(ents.kind().len(), false);
        self.corpses.clone_from(&snap.corpses);
        self.corpse_visible.clear();
        for c in &self.corpses {
            let i = c.id.0 as usize;
            self.corpse_visible
                .push(c.owner == local || ctx.seen.get(i).copied().unwrap_or(false));
        }
        for v in ctx.seen.iter_mut() {
            *v = false;
        }
        for (i, &id) in self.ids.iter().enumerate() {
            if self.visible[i] {
                ctx.seen[id as usize] = true;
            }
        }
        // Corpses keep the flag they died with.
        for (c, &v) in self.corpses.iter().zip(&self.corpse_visible) {
            if let Some(s) = ctx.seen.get_mut(c.id.0 as usize) {
                *s = v;
            }
        }

        let combat = ents.combat();
        let kind = ents.kind();
        let seen_by = |id: u32| -> bool {
            let i = id as usize;
            i < combat.len() && kind[i] != EntityKind::Vacant && combat[i].seen_by & bit != 0
        };
        self.rounds.clear();
        for p in world.projectiles().slots().iter().flatten() {
            let w = ctx.weapons.get(p.weapon as usize);
            let he = w.is_some_and(|w| w.he);
            let ttl0 = w.map_or(180.0, |w| w.ttl0);
            self.rounds.push(RoundView {
                pos: p.pos.to_f32(),
                vel: p.vel.to_f32(),
                age: ((ttl0 - p.ttl as f32) / SUBSTEP_HZ as f32).max(0.0),
                he,
                visible: p.player == local || seen_by(p.owner.0),
            });
        }

        self.squads.clear();
        self.players.clone_from(&snap.players);
        self.points.clone_from(&snap.points);
        self.outcome = snap.outcome;
        let Some(game) = world.game() else {
            return;
        };
        let setup = game.setup();
        for (sid, sq) in game.squads() {
            let Some(&lead) = sq.members.first() else {
                continue;
            };
            let ty = &setup.squad_types[sq.type_idx as usize];
            let mut v = SquadView {
                id: sid.0,
                type_idx: sq.type_idx,
                owner: sq.owner.0,
                men: sq.members.len() as u8,
                order: sq.order.kind(),
                phase: sq.nav.phase,
                leader: lead.0,
                anchor: ents.transform()[lead.index()].pos.to_f32(),
                goal: sq.nav.goal.to_f32(),
                visible: sq.owner.0 == local,
                ..SquadView::default()
            };
            for m in &sq.members {
                let c = &combat[m.index()];
                v.hp_sum += c.hp as u32;
                v.suppression_sum += (c.suppression / 1000).min(100);
                v.pinned += c.pinned as u8;
                v.visible |= c.seen_by & bit != 0;
            }
            let w = &setup.weapons[ty.weapon as usize];
            v.setup = ty.role == SquadRole::Mg
                && sq.nav.phase == NavPhase::Holding
                && (combat[lead.index()].still as u32) < w.setup_ticks;
            match &sq.order {
                Order::Move { target } | Order::AttackMove { target } => {
                    v.target = Some(target.to_f32());
                }
                Order::Dig { line, job } => {
                    v.dig_pct = job.progress_pct(&setup.rules.dig, setup.tick_hz);
                    v.dig_line.extend(line.iter().map(|p| p.to_f32()));
                }
                Order::Barrage { target, rounds } => {
                    v.target = Some(target.to_f32());
                    v.rounds = *rounds;
                }
                Order::Idle | Order::Retreat => {}
            }
            self.squads.push(v);
        }
        ctx.track(&self.squads, setup.players.len());
        self.extra.clone_from(&ctx.extra);
    }
}

/// Per-weapon presenter constants, precomputed from the setup.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct WeaponInfo {
    pub he: bool,
    /// Substeps a round lives (as the sim's `round_ttl`), for its age.
    pub ttl0: f32,
}

/// Sim-thread state carried across ticks while filling frames.
#[derive(Clone, Debug, Default)]
pub struct FrameCtx {
    pub local: u8,
    pub weapons: Vec<WeaponInfo>,
    /// Per entity slot: the local player saw him last tick.
    seen: Vec<bool>,
    /// Squads seen so far: owner, dig line length (m) and progress.
    squads: BTreeMap<u32, (u8, f32, u8)>,
    started: bool,
    pub extra: Vec<PlayerExtra>,
}

impl FrameCtx {
    pub fn new(setup: &MatchSetup, local: u8) -> FrameCtx {
        let weapons = setup
            .weapons
            .iter()
            .map(|w| {
                let range = frontline_sim::Fx::from(w.range_max).to_f32();
                let speed = frontline_sim::Fx::from(w.muzzle_speed).to_f32().max(1.0);
                WeaponInfo {
                    he: w.class == WeaponClass::Mortar,
                    ttl0: (range * 1.25 / speed * SUBSTEP_HZ as f32).floor() + 2.0,
                }
            })
            .collect();
        FrameCtx {
            local,
            weapons,
            ..FrameCtx::default()
        }
    }

    /// Updates the call-in and trench counters from this tick's squads.
    fn track(&mut self, squads: &[SquadView], players: usize) {
        self.extra.resize(players, PlayerExtra::default());
        let mut gone: Vec<u32> = self.squads.keys().copied().collect();
        for s in squads {
            gone.retain(|&g| g != s.id);
            let len = if s.order == OrderKind::Dig {
                polyline_len(&s.dig_line)
            } else {
                0.0
            };
            match self.squads.get_mut(&s.id) {
                None => {
                    if self.started
                        && let Some(e) = self.extra.get_mut(s.owner as usize)
                    {
                        e.squads_called += 1;
                    }
                    self.squads.insert(s.id, (s.owner, len, s.dig_pct));
                }
                Some(entry) => {
                    // A dig order ended or was replaced: book what was dug.
                    let ended = entry.1 > 0.0 && (len != entry.1 || s.order != OrderKind::Dig);
                    let booked = *entry;
                    if ended {
                        entry.1 = 0.0;
                    }
                    if s.order == OrderKind::Dig {
                        entry.1 = len;
                        entry.2 = s.dig_pct;
                    }
                    if ended {
                        self.book(booked.0, booked.1, booked.2);
                    }
                }
            }
        }
        for id in gone {
            if let Some((owner, len, pct)) = self.squads.remove(&id)
                && len > 0.0
            {
                self.book(owner, len, pct);
            }
        }
        self.started = true;
    }

    fn book(&mut self, owner: u8, len: f32, pct: u8) {
        if let Some(e) = self.extra.get_mut(owner as usize) {
            // A finished order reports 100 % only in its last tick, so
            // anything at 95 % or more counts as the whole line.
            let k = if pct >= 95 { 1.0 } else { pct as f32 / 100.0 };
            e.trench_m += len * k;
        }
    }
}

pub fn polyline_len(line: &[[f32; 2]]) -> f32 {
    line.windows(2)
        .map(|w| {
            let (dx, dy) = (w[1][0] - w[0][0], w[1][1] - w[0][1]);
            (dx * dx + dy * dy).sqrt()
        })
        .sum()
}

/// A round fired this tick.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct ShotView {
    pub from: [f32; 3],
    pub vel: [f32; 3],
    pub weapon: u16,
    pub player: u8,
    pub squad: u32,
    /// The local player sees the shooter (own: always).
    pub visible: bool,
}

/// A terrain detail tile that changed: its 64 × 64 heights (row-major,
/// sim axes), or `None` when it went back to the base surface.
#[derive(Clone, Debug, PartialEq)]
pub struct TileUpdate {
    pub x: u16,
    pub y: u16,
    pub heights: Option<Vec<f32>>,
}

/// What happened in one tick (sent only when something did).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TickEvents {
    pub tick: u64,
    pub shots: Vec<ShotView>,
    pub explosions: Vec<ExplosionSnapshot>,
    pub tiles: Vec<TileUpdate>,
}

impl TickEvents {
    pub fn is_empty(&self) -> bool {
        self.shots.is_empty() && self.explosions.is_empty() && self.tiles.is_empty()
    }

    /// This tick's events of `world` as seen by `local`.
    pub fn collect(world: &World, local: u8) -> TickEvents {
        let snap = world.snapshot();
        let ents = world.entities();
        let bit = 1u16 << local;
        let mut ev = TickEvents {
            tick: snap.tick,
            ..TickEvents::default()
        };
        for s in &snap.shots {
            let i = s.shooter.index();
            let player = ents.owner().get(i).map_or(0, |p| p.0);
            let seen = ents.combat().get(i).is_some_and(|c| c.seen_by & bit != 0);
            ev.shots.push(ShotView {
                from: s.from,
                vel: s.vel,
                weapon: s.weapon,
                player,
                squad: ents
                    .squad()
                    .get(i)
                    .copied()
                    .flatten()
                    .map_or(u32::MAX, |q| q.0),
                visible: player == local || seen,
            });
        }
        ev.explosions.clone_from(&snap.explosions);
        for &(x, y) in &snap.dirty_tiles {
            let mut h = Vec::new();
            let heights = tile_heights_f32(world.terrain(), x, y, &mut h).then_some(h);
            ev.tiles.push(TileUpdate { x, y, heights });
        }
        ev
    }

    /// Every detail tile `world` holds now (after a load, or to rebuild a
    /// presenter from scratch).
    pub fn all_tiles(world: &World) -> Vec<TileUpdate> {
        world
            .terrain()
            .detail()
            .tiles()
            .iter()
            .map(|t| {
                let (x, y) = t.coords();
                TileUpdate {
                    x,
                    y,
                    heights: Some(t.heights().iter().map(|h| h.to_f32()).collect()),
                }
            })
            .collect()
    }
}

#[cfg(test)]
pub(crate) mod tests_support {
    use crate::match_setup::{MenuChoice, find_mods_dir, mods_candidates, resolve};
    use frontline_sim::World;

    /// A fresh Kessel Ridge match from the source tree's `mods/ww2`.
    pub(crate) fn test_world(ai_vs_ai: bool) -> World {
        let mods = find_mods_dir(&mods_candidates(None, None)).unwrap();
        let choice = MenuChoice {
            ai_vs_ai,
            ..MenuChoice::default()
        };
        World::new_match(resolve(&mods, &choice).unwrap(), 2).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::tests_support::test_world;
    use super::*;
    use frontline_sim::{Command, CommandKind, FxVec2, PlayerId};

    #[test]
    fn frame_matches_the_world() {
        let mut w = test_world(false);
        let setup = w.match_setup().unwrap().clone();
        let mut ctx = FrameCtx::new(&setup, 0);
        let mut f = MatchFrame::empty();
        f.fill(&w, &mut ctx, 0.0, Instant::now());
        assert_eq!(f.ids.len(), f.soldiers.len());
        assert_eq!(f.visible.len(), f.ids.len());
        assert_eq!(f.stance.len(), f.ids.len());
        assert_eq!(f.players.len(), 2);
        assert_eq!(f.points.len(), 5);
        assert!(!f.squads.is_empty());
        // Own squads are visible; at the start nobody sees the enemy base.
        for s in &f.squads {
            assert_eq!(s.visible, s.owner == 0, "squad {} owner {}", s.id, s.owner);
            assert!(s.hp_sum > 0);
        }
        for (i, &o) in f.owner.iter().enumerate() {
            assert_eq!(f.visible[i], o == 0);
        }
        // A Move order shows its target; a later call-in is counted.
        let own = f.squads.iter().find(|s| s.owner == 0).unwrap().id;
        let target = FxVec2::from_int(300, 300);
        let cmd = |seq, kind| Command {
            player: PlayerId(0),
            seq,
            kind,
        };
        w.step(&[
            cmd(
                0,
                CommandKind::Move {
                    squad: frontline_sim::SquadId(own),
                    target,
                },
            ),
            cmd(
                1,
                CommandKind::CallIn {
                    squad_type: setup.factions[setup.players[0].faction as usize].roster[0],
                },
            ),
        ]);
        f.fill(&w, &mut ctx, 0.0, Instant::now());
        let s = f.squads.iter().find(|s| s.id == own).unwrap();
        assert_eq!(s.order, OrderKind::Move);
        assert_eq!(s.target, Some([300.0, 300.0]));
        assert_eq!(f.players[0].call_ins.len(), 1);
        let ticks = f.players[0].call_ins[0].1;
        for _ in 0..ticks + 2 {
            w.step(&[]);
            f.fill(&w, &mut ctx, 0.0, Instant::now());
        }
        assert_eq!(f.extra[0].squads_called, 1);
        assert_eq!(f.extra[1].squads_called, f.extra[1].squads_called);
    }

    #[test]
    fn events_carry_dug_tiles() {
        let mut w = test_world(false);
        let setup = w.match_setup().unwrap().clone();
        let own = w
            .game()
            .unwrap()
            .squads()
            .iter()
            .find(|(_, s)| {
                s.owner.0 == 0 && setup.squad_types[s.type_idx as usize].dig_rate_pct > 0
            })
            .map(|(id, _)| *id)
            .unwrap();
        let base = w.game().unwrap().base_of(PlayerId(0));
        let a = base + FxVec2::from_int(20, 30);
        let b = a + FxVec2::from_int(12, 0);
        w.step(&[Command {
            player: PlayerId(0),
            seq: 0,
            kind: CommandKind::Dig {
                squad: own,
                line: vec![a, b],
            },
        }]);
        let mut tiles = 0;
        for _ in 0..30 * 120 {
            w.step(&[]);
            let ev = TickEvents::collect(&w, 0);
            for t in &ev.tiles {
                tiles += 1;
                assert_eq!(t.heights.as_ref().map(|h| h.len()), Some(64 * 64));
            }
            if tiles > 0 {
                break;
            }
        }
        assert!(tiles > 0, "digging reported dirty tiles");
        assert_eq!(
            TickEvents::all_tiles(&w).len(),
            w.terrain().detail().tiles().len()
        );
    }

    #[test]
    fn polyline_length() {
        assert_eq!(polyline_len(&[[0.0, 0.0], [3.0, 4.0], [3.0, 10.0]]), 11.0);
        assert_eq!(polyline_len(&[]), 0.0);
    }
}
