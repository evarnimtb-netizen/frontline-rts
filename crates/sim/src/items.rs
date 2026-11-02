//! Thrown items (S1b abilities): frag grenades, smoke grenades and satchel
//! charges (`docs/slices/S1b-abilities.md`).
//!
//! * **Kit.** Each soldier carries a [`Kit`] (entity column): item counts by
//!   [`ItemKind`], filled at spawn from his squad type's loadout (the squad
//!   leader, the first man at spawn, has his own counts). A man inside his
//!   own base gets one item back every `items.resupply_ticks`, up to his
//!   loadout (kinds in [`ItemKind::ALL`] order).
//! * **Throw** (`Throw`, `ThrowEach`; Intent stage). Of the named men, those
//!   who carry the item, are not pinned, are not already winding up and
//!   have the target within their item's range are eligible. `Throw` picks
//!   the one nearest the target (ties: lowest id), `ThrowEach` takes them
//!   all; nobody eligible: the command is ignored. A thrower takes the item
//!   out of his kit, becomes detached (`crate::unit`), idle where he stands,
//!   turns to the target and winds up for the item's `windup_ticks`. He does
//!   not fire meanwhile; in a trench he rises to the fire step to throw.
//! * **Release** (Weapons stage, serial, ascending id). A wind-up is called
//!   off, and the item put back, when the man is pinned or given another
//!   order (`StopUnits` included). When it ends, the item leaves his hand (fire height − 0.15 m)
//!   on an arc to the target scattered by `scatter_pct` % of the distance
//!   (triangular per axis, counter RNG keyed by the thrower and his throw
//!   count). The arc is a 45° lob: flight time `sqrt(2 d / g)`, about
//!   2.5 s at 30 m, so it comes down steeply enough to drop into a trench.
//!   Over dug ground it is aimed half-way down (between the floor and the
//!   undug surface), so it clears the lip and parapet in front.
//! * **Flight** (Ballistics stage, serial, ascending id): the exact
//!   parabola sampled at 60 Hz, each step swept against the terrain
//!   (including dug ground). Where it meets the ground the item lands and
//!   lies still; there is no bouncing or rolling.
//! * **Fuse** (Damage stage, after the projectile events): the item goes
//!   off once it has landed and its fuse (from the start of the throw) has
//!   run out. Frags and satchels burst like a mortar shell
//!   (`combat::burst`: damage falloff, terrain between blast and body
//!   blocks, suppression, crater); a satchel's crater levels trenches and
//!   parapets (`Heightfield::crater_cut`). Smoke starts a [`SmokeCloud`].
//! * **Smoke.** A vertical cylinder as tall as it is wide grows to its full
//!   radius over `grow_ticks`, holds, then thins out over `fade_ticks`. A
//!   spotting or aiming sight line that passes through a cloud is blocked;
//!   while it thins, it is blocked with the cloud's density as probability
//!   (counter RNG by tick, target, observer and cloud). Bullets fly through.
//!   At most [`MAX_SMOKE`] clouds, in id order; a new one drops the oldest.
//!
//! Items in flight and clouds live in the match state in id order (ids are
//! never reused); all of it is saved, hashed and checked on load.

use frontline_content::setup::{ItemSetup, SmokeSetup};
use frontline_content::tuning::ThrowTuning;
use frontline_content::{ITEM_KINDS, ItemKind};
use serde::{Deserialize, Serialize};

use crate::combat::tri;
use crate::ecs::{Entities, EntityKind, PlayerId, SimId};
use crate::fx::{Fx, FxVec2, FxVec3};
use crate::game::{Ctx, MatchState, NavPhase};
use crate::hash::HashCache;
use crate::projectile::SUBSTEP_HZ;
use crate::rng::{Purpose, RngStream, below, mix64, rng};
use crate::terrain::Heightfield;
use crate::unit::UnitOrder;

/// Most items in flight or lying fused at once (a throw beyond this is
/// called off and the item kept).
pub const MAX_THROWN: usize = 256;
/// Most smoke clouds at once.
pub const MAX_SMOKE: usize = 64;
/// An item still in the air after this long lands where it is (a safety
/// bound on a flight's substeps, not tuning).
const MAX_FLIGHT_S: i32 = 20;
/// 9.81 m/s².
const GRAVITY: Fx = Fx::from_ratio(981, 100);
// S3b: the release height, lob, shortest flight and smoke reach are the
// rules' `[items.throw]` (`frontline_content::tuning`).

/// A throw being wound up.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Windup {
    pub kind: ItemKind,
    pub target: FxVec2,
    /// Ticks until the item leaves his hand.
    pub ticks: u16,
}

/// A soldier's items (see the module docs). Default for bench soldiers,
/// corpses and vacant slots.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Kit {
    /// Items carried, by [`ItemKind::index`].
    pub items: [u8; ITEM_KINDS],
    /// S4: his `[[members]]` slot (index into the squad type's `members`):
    /// his loadout and traits. Slot 0's first man led at spawn.
    pub slot: u8,
    /// S4: the weapon he carries (index into `MatchSetup::weapons`), fixed
    /// at spawn; changes only when he picks up a fallen man's crew weapon.
    pub weapon: u16,
    /// Throws released so far (scatter RNG key), wrapping.
    pub throws: u16,
    /// Ticks spent in base toward the next resupplied item.
    pub resupply: u16,
    pub windup: Option<Windup>,
}

/// An item in flight, or landed and waiting for its fuse.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Thrown {
    /// Never reused.
    pub id: u32,
    /// Index into `MatchSetup::items`.
    pub item: u16,
    pub owner: PlayerId,
    pub thrower: SimId,
    /// Release point and velocity: the arc is `from + vel·t − g·t²/2`.
    pub from: FxVec3,
    pub vel: FxVec3,
    /// 60 Hz substeps flown.
    pub step: u16,
    pub pos: FxVec3,
    pub landed: bool,
    /// Ticks until the fuse runs out (it goes off at 0 once landed).
    pub fuse: u16,
}

/// A smoke cloud standing on the ground at `pos`.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct SmokeCloud {
    /// Never reused.
    pub id: u32,
    /// Index into `MatchSetup::items` (a smoke item).
    pub item: u16,
    pub pos: FxVec3,
    /// Ticks since it started.
    pub age: u32,
}

/// What made an explosion (snapshot effects).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum BlastKind {
    /// A mortar shell.
    #[default]
    Shell = 0,
    Frag = 1,
    Satchel = 2,
}

/// Radius and density (per mille) of a cloud `age` ticks old.
pub fn cloud_now(s: &SmokeSetup, age: u32) -> (Fx, u32) {
    let full: Fx = s.radius.into();
    let (grow, hold) = (s.grow_ticks, s.grow_ticks + s.hold_ticks);
    if age < grow {
        let r = Fx::from_raw((full.raw() as i64 * age as i64 / grow as i64) as i32);
        (r, 1000)
    } else if age < hold {
        (full, 1000)
    } else if age < s.life_ticks() {
        let left = s.life_ticks() - age;
        (full, left * 1000 / s.fade_ticks.max(1))
    } else {
        (Fx::ZERO, 0)
    }
}

impl MatchState {
    // ------------------------------------------------------------ queries --

    /// Items in flight or waiting for their fuse, ascending id.
    #[inline]
    pub fn thrown(&self) -> &[Thrown] {
        &self.thrown
    }
    /// Smoke clouds, ascending id (oldest first).
    #[inline]
    pub fn smoke(&self) -> &[SmokeCloud] {
        &self.smoke
    }

    /// The item soldier `m` would throw for `kind` (from his squad type's
    /// loadout) with its index, if any.
    pub(crate) fn item_of(
        &self,
        ents: &Entities,
        m: SimId,
        kind: ItemKind,
    ) -> Option<(u16, &ItemSetup)> {
        let sq = self.squads.get(&ents.squad[m.index()]?)?;
        let ty = &self.setup.squad_types[sq.type_idx as usize];
        ty.kit(&self.setup.items, kind)
    }

    /// Full kit of soldier `m` (his loadout), all zero outside a squad.
    pub(crate) fn full_kit(&self, ents: &Entities, m: SimId) -> [u8; ITEM_KINDS] {
        let i = m.index();
        match ents.squad[i].and_then(|s| self.squads.get(&s)) {
            Some(sq) => self.setup.squad_types[sq.type_idx as usize].full_kit(ents.kit[i].slot),
            None => [0; ITEM_KINDS],
        }
    }

    /// Squared distance (Q32) from `m` to `target` if he may throw a `kind`
    /// item there now (module docs), else `None`.
    pub(crate) fn can_throw(
        &self,
        ents: &Entities,
        m: SimId,
        kind: ItemKind,
        target: FxVec2,
    ) -> Option<u64> {
        let i = m.index();
        let k = &ents.kit[i];
        // S4: a single-shot AT item is fired (`crate::combat`), not thrown.
        if ents.kind[i] != EntityKind::Soldier
            || !kind.thrown()
            || k.items[kind.index()] == 0
            || k.windup.is_some()
            || ents.combat[i].pinned
            || ents.combat[i].ducked
        {
            return None;
        }
        let (_, it) = self.item_of(ents, m, kind)?;
        let d = (target - ents.transform[i].pos.xy()).length_sq_raw();
        let r = |v: Fx| FxVec2::new(v, Fx::ZERO).length_sq_raw();
        (r(it.range_min.into()) <= d && d <= r(it.range_max.into())).then_some(d)
    }

    // ---------------------------------------------------------- commands --

    /// `Throw` (`each == false`) and `ThrowEach`.
    pub(crate) fn throw(
        &mut self,
        ctx: &mut Ctx,
        p: PlayerId,
        ids: &[SimId],
        kind: ItemKind,
        target: FxVec2,
        each: bool,
    ) -> bool {
        if !ctx.terrain.contains(target) {
            return false;
        }
        let Some(men) = self.unit_set(ctx, p, ids) else {
            return false;
        };
        let mut eligible: Vec<(u64, SimId)> = men
            .iter()
            .filter_map(|&m| self.can_throw(ctx.ents, m, kind, target).map(|d| (d, m)))
            .collect();
        if eligible.is_empty() {
            return false;
        }
        if !each {
            let nearest = *eligible.iter().min().expect("not empty");
            eligible = vec![nearest];
        }
        eligible.sort_unstable_by_key(|&(_, m)| m);
        for (_, m) in eligible {
            self.start_throw(ctx, m, kind, target);
        }
        true
    }

    /// Detaches `m` where he stands and starts his wind-up.
    fn start_throw(&mut self, ctx: &mut Ctx, m: SimId, kind: ItemKind, target: FxVec2) {
        let i = m.index();
        let t = ctx.ents.transform[i];
        let mut facing = (target - t.pos.xy()).normalize_or_zero();
        if facing == FxVec2::ZERO {
            let (s, c) = t.yaw.sin_cos();
            facing = FxVec2::new(c, s);
        }
        let windup = self.item_of(ctx.ents, m, kind).map_or(1, |(_, it)| {
            it.windup_ticks.clamp(1, u16::MAX as u32) as u16
        });
        self.detach(ctx, m, UnitOrder::Idle, facing);
        let nav = &mut ctx.ents.unit[i].nav;
        nav.phase = NavPhase::Holding;
        nav.goal = t.pos.xy();
        let k = &mut ctx.ents.kit[i];
        k.items[kind.index()] -= 1;
        k.windup = Some(Windup {
            kind,
            target,
            ticks: windup,
        });
        ctx.hash.mark_entity_chunk(m.chunk());
    }

    /// Calls off `m`'s wind-up, if any, and puts the item back in his kit
    /// (`StopUnits`).
    pub(crate) fn call_off_throw(&self, ctx: &mut Ctx, m: SimId) {
        let Some(w) = ctx.ents.kit[m.index()].windup else {
            return;
        };
        let full = self.full_kit(ctx.ents, m)[w.kind.index()];
        let k = &mut ctx.ents.kit[m.index()];
        let n = &mut k.items[w.kind.index()];
        *n = n.saturating_add(1).min(full);
        k.windup = None;
        ctx.hash.mark_entity_chunk(m.chunk());
    }

    // ------------------------------------------------------------- stages --

    /// Counts wind-ups down and releases finished throws (Weapons stage;
    /// module docs). Returns the number of items released.
    pub(crate) fn release_throws(
        &mut self,
        ents: &mut Entities,
        terrain: &Heightfield,
        hash: &mut HashCache,
        seed: u64,
    ) -> u64 {
        let mut released = 0;
        let throw = self.setup.rules.items.throw.clone();
        let throw = &throw;
        for i in 0..ents.len() {
            let Some(mut w) = ents.kit[i].windup else {
                continue;
            };
            let m = SimId(i as u32);
            hash.mark_entity_chunk(m.chunk());
            let u = &ents.unit[i];
            // S4: training scales the scatter.
            let scatter = ents.squad[i]
                .and_then(|q| self.squads.get(&q))
                .map_or(100, |q| {
                    crate::combat::training_of(&self.setup, q).throw_scatter_pct
                });
            let item = self.item_of(ents, m, w.kind).map(|(idx, it)| {
                (
                    idx,
                    crate::combat::pct_of(it.scatter_pct, scatter),
                    it.fuse_ticks,
                    it.windup_ticks,
                )
            });
            let full = self.full_kit(ents, m)[w.kind.index()];
            let keep_on = ents.kind[i] == EntityKind::Soldier
                && !ents.combat[i].pinned
                && !ents.combat[i].ducked
                && u.detached
                && u.order == UnitOrder::Idle;
            let k = &mut ents.kit[i];
            let (Some((idx, scatter_pct, fuse_ticks, windup_ticks)), true) = (item, keep_on) else {
                // Called off: the item goes back into his kit.
                let n = &mut k.items[w.kind.index()];
                *n = n.saturating_add(1).min(full);
                k.windup = None;
                continue;
            };
            w.ticks = w.ticks.saturating_sub(1);
            if w.ticks > 0 {
                k.windup = Some(w);
                continue;
            }
            k.windup = None;
            if self.thrown.len() >= MAX_THROWN {
                let n = &mut k.items[w.kind.index()];
                *n = n.saturating_add(1).min(full);
                continue;
            }
            let mut rng = RngStream::new(seed, k.throws as u64, m, Purpose::THROW);
            k.throws = k.throws.wrapping_add(1);
            let pos = ents.transform[i].pos;
            let lift = ents.combat[i].fire_height - Fx::from_milli(throw.release_drop_mm as i32);
            let from = FxVec3::new(pos.x, pos.y, pos.z + lift.max(Fx::from_milli(200)));
            let d = (w.target - from.xy()).length();
            let s = d.mul_int(scatter_pct as i32).div_int(100);
            let aim = terrain.clamp(w.target + FxVec2::new(tri(&mut rng, s), tri(&mut rng, s)));
            let (from, vel) = lob(from, aim.extend(lob_height(terrain, aim)), throw);
            let fuse = fuse_ticks.saturating_sub(windup_ticks) + 1;
            self.thrown.push(Thrown {
                id: self.next_thrown,
                item: idx,
                owner: ents.owner[i],
                thrower: m,
                from,
                vel,
                step: 0,
                pos: from,
                landed: false,
                fuse: fuse.min(u16::MAX as u32) as u16,
            });
            self.next_thrown += 1;
            released += 1;
        }
        released
    }

    /// Flies the items in the air for `substeps` 60 Hz steps (Ballistics
    /// stage; module docs).
    pub(crate) fn fly_thrown(&mut self, terrain: &Heightfield, substeps: u32) {
        let half_g = GRAVITY.div_int(2);
        let max_step = (MAX_FLIGHT_S * SUBSTEP_HZ) as u16;
        for t in self.thrown.iter_mut().filter(|t| !t.landed) {
            for _ in 0..substeps {
                t.step += 1;
                let s = Fx::from_ratio(t.step as i64, SUBSTEP_HZ as i64);
                let mut next = t.from + t.vel * s;
                next.z -= half_g.mul(s).mul(s);
                let ground = terrain.segment_hit(t.pos, next).or_else(|| {
                    (!terrain.contains(next.xy()) || t.step >= max_step).then_some(next)
                });
                if let Some(g) = ground {
                    let xy = terrain.clamp(g.xy());
                    t.pos = xy.extend(terrain.height_at(xy));
                    t.landed = true;
                    break;
                }
                t.pos = next;
            }
        }
    }

    /// Runs every fuse down one tick and takes out the items that go off now
    /// (landed, fuse out), in id order (Damage stage).
    pub(crate) fn take_fused(&mut self, out: &mut Vec<Thrown>) {
        out.clear();
        let mut k = 0;
        while k < self.thrown.len() {
            let t = &mut self.thrown[k];
            t.fuse = t.fuse.saturating_sub(1);
            if t.landed && t.fuse == 0 {
                out.push(self.thrown.remove(k));
            } else {
                k += 1;
            }
        }
    }

    /// Starts a cloud of smoke item `item` at `pos` (drops the oldest when
    /// full).
    pub(crate) fn start_smoke(&mut self, item: u16, pos: FxVec3) {
        if self.smoke.len() >= MAX_SMOKE {
            self.smoke.remove(0);
        }
        self.smoke.push(SmokeCloud {
            id: self.next_smoke,
            item,
            pos,
            age: 0,
        });
        self.next_smoke += 1;
    }

    /// Ages the smoke clouds and resupplies men in their base (Status
    /// stage, serial, ascending id).
    pub(crate) fn update_items(&mut self, ents: &mut Entities, hash: &mut HashCache) {
        let setup = &self.setup;
        self.smoke.retain_mut(|c| {
            c.age += 1;
            let life = setup.items[c.item as usize]
                .smoke
                .as_ref()
                .map_or(0, |s| s.life_ticks());
            c.age < life
        });
        let every = setup.rules.items.resupply_ticks;
        // AT teams (S3): one rocket per `every` while the carrier stands in
        // the own base.
        for sq in self.squads.values_mut() {
            let Some(l) = &mut sq.launcher else { continue };
            let ty = &setup.squad_types[sq.type_idx as usize];
            let full = setup.weapons[ty.weapon as usize].rounds_carried;
            let base = &setup.map.bases[setup.players[sq.owner.0 as usize].base as usize];
            let r: Fx = base.radius.into();
            let in_base = l.carrier.is_some_and(|c| {
                (ents.transform[c.index()].pos.xy() - FxVec2::from(base.center)).length_sq_raw()
                    <= FxVec2::new(r, Fx::ZERO).length_sq_raw()
            });
            if in_base && (l.rounds as u32) < full {
                l.resupply += 1;
                if l.resupply as u32 >= every {
                    l.resupply = 0;
                    l.rounds += 1;
                }
            } else {
                l.resupply = 0;
            }
        }
        for i in 0..ents.len() {
            if ents.kind[i] != EntityKind::Soldier {
                continue;
            }
            let m = SimId(i as u32);
            let full = self.full_kit(ents, m);
            let k = &ents.kit[i];
            let short = (0..ITEM_KINDS).find(|&j| k.items[j] < full[j]);
            let base = &setup.map.bases[setup.players[ents.owner[i].0 as usize].base as usize];
            let r: Fx = base.radius.into();
            let in_base = (ents.transform[i].pos.xy() - FxVec2::from(base.center)).length_sq_raw()
                <= FxVec2::new(r, Fx::ZERO).length_sq_raw();
            let k = &mut ents.kit[i];
            match short {
                Some(j) if in_base => {
                    k.resupply += 1;
                    if k.resupply as u32 >= every {
                        k.resupply = 0;
                        k.items[j] += 1;
                    }
                }
                _ if k.resupply != 0 => k.resupply = 0,
                _ => continue,
            }
            hash.mark_entity_chunk(m.chunk());
        }
    }

    /// Whether active smoke blocks the sight line `a → b` this tick (module
    /// docs). `key` is `(target, observer)` for the thinning roll.
    pub(crate) fn smoke_blocks(
        &self,
        a: FxVec3,
        b: FxVec3,
        tick: u64,
        key: (SimId, SimId),
    ) -> bool {
        if self.smoke.is_empty() {
            return false;
        }
        let d = b - a;
        let dd = d.xy().dot_raw(d.xy()) as i128;
        for c in &self.smoke {
            let Some(s) = self.setup.items[c.item as usize].smoke.as_ref() else {
                continue;
            };
            let (r, density) = cloud_now(s, c.age);
            if r <= Fx::ZERO || density == 0 {
                continue;
            }
            // Closest approach to the cloud's axis, in the plane.
            let f = c.pos.xy() - a.xy();
            let t = if dd > 0 {
                (((f.dot_raw(d.xy()) as i128) << Fx::FRAC_BITS) / dd).clamp(0, 1 << Fx::FRAC_BITS)
            } else {
                0
            };
            let p = a + d * Fx::from_raw(t as i32);
            if (p.xy() - c.pos.xy()).length_sq_raw() > FxVec2::new(r, Fx::ZERO).length_sq_raw()
                || p.z
                    < c.pos.z - Fx::from_milli(self.setup.rules.items.throw.smoke_below_mm as i32)
                || p.z > c.pos.z + r.mul_int(2)
            {
                continue;
            }
            if density >= 1000 {
                return true;
            }
            let roll = mix64(
                rng(self.setup.seed, tick, key.0, Purpose::SMOKE)
                    ^ (((key.1.0 as u64) << 32) | c.id as u64),
            );
            if below(roll, 1000) < density {
                return true;
            }
        }
        false
    }

    /// Checks the loaded item state (untrusted saves): kits within the
    /// loadout, wind-ups for carried kinds, items in flight and clouds in
    /// range and in id order.
    pub(crate) fn validate_items(&self, ents: &Entities) -> Result<(), &'static str> {
        let s = &self.setup;
        let size = Fx::from_int(s.map.size_m);
        let on_map = |q: FxVec2| q.x >= Fx::ZERO && q.y >= Fx::ZERO && q.x <= size && q.y <= size;
        for (i, k) in ents.kit.iter().enumerate() {
            let m = SimId(i as u32);
            if ents.kind[i] != EntityKind::Soldier {
                if *k != Kit::default() {
                    return Err("a corpse carries items");
                }
                continue;
            }
            let full = self.full_kit(ents, m);
            let bad = (0..ITEM_KINDS).any(|j| k.items[j] > full[j])
                || k.resupply as u32 >= s.rules.items.resupply_ticks
                || k.windup.is_some_and(|w| {
                    self.item_of(ents, m, w.kind).is_none_or(|(_, it)| {
                        w.ticks == 0 || w.ticks as u32 > it.windup_ticks.max(1)
                    }) || !on_map(w.target)
                });
            if bad {
                return Err("soldier kit is inconsistent");
            }
        }
        // S4: no slot holds more men than it has (one leader kit), and
        // every man carries a weapon of his squad's members.
        for sq in self.squads.values() {
            let ty = &s.squad_types[sq.type_idx as usize];
            if ty.vehicle.is_some() {
                continue;
            }
            let mut n = vec![0u32; ty.members.len()];
            for m in &sq.members {
                let k = &ents.kit[m.index()];
                let carried = ty
                    .members
                    .iter()
                    .any(|ms| ms.weapon == k.weapon || ms.sidearm == Some(k.weapon));
                match n.get_mut(k.slot as usize) {
                    Some(c) if carried => *c += 1,
                    _ => return Err("a soldier's slot or weapon is not his squad's"),
                }
            }
            // The first slot's kit (the leader's) is never doubled: a man
            // gets slot 0 only as the first of an empty squad. Later slots
            // may overfill (a crew squad refilled by a second bail-out).
            if n.first()
                .zip(ty.members.first())
                .is_some_and(|(&c, ms)| c > ms.count as u32)
            {
                return Err("a squad has more leader kits than its first slot has men");
            }
        }
        let np = s.players.len();
        let in_air = |v: FxVec3| on_map(v.xy()) && v.z.abs() < Fx::from_int(5000);
        if self.thrown.len() > MAX_THROWN
            || !ascending(self.thrown.iter().map(|t| t.id), self.next_thrown)
            || self.thrown.iter().any(|t| {
                s.items.get(t.item as usize).is_none()
                    || t.fuse as u32 > 30 * 30 + 1
                    || t.owner.0 as usize >= np
                    || t.thrower.index() >= ents.len()
                    || !in_air(t.from)
                    || !in_air(t.pos)
                    || t.vel.x.abs() > Fx::from_int(200)
                    || t.vel.y.abs() > Fx::from_int(200)
                    || t.vel.z.abs() > Fx::from_int(200)
                    || t.step as i32 > MAX_FLIGHT_S * SUBSTEP_HZ
            })
        {
            return Err("thrown items are inconsistent");
        }
        if self.smoke.len() > MAX_SMOKE
            || !ascending(self.smoke.iter().map(|c| c.id), self.next_smoke)
            || self.smoke.iter().any(|c| {
                s.items
                    .get(c.item as usize)
                    .and_then(|it| it.smoke.as_ref())
                    .is_none_or(|m| c.age >= m.life_ticks())
                    || !in_air(c.pos)
            })
        {
            return Err("smoke clouds are inconsistent");
        }
        Ok(())
    }
}

/// Whether `ids` ascend strictly and stay below `next`.
fn ascending(ids: impl Iterator<Item = u32>, next: u32) -> bool {
    let mut last = None;
    for id in ids {
        if id >= next || last.is_some_and(|l| l >= id) {
            return false;
        }
        last = Some(id);
    }
    true
}

/// Height the lob is aimed at over `aim`: the ground, but over dug ground
/// half-way between the floor and the undug surface, so the arc clears the
/// lip and parapet in front of a trench and drops in (aimed at the floor
/// itself, it would come down on the near parapet).
fn lob_height(terrain: &Heightfield, aim: FxVec2) -> Fx {
    let (ground, base) = (terrain.height_at(aim), terrain.base_height_at(aim));
    if ground < base {
        Fx::from_raw(((ground.raw() as i64 + base.raw() as i64) / 2) as i32)
    } else {
        ground
    }
}

/// Release point and velocity of a lob from `from` to `to` (45° at the
/// default `lob_tan_milli`; flight time `sqrt(2 d · tan / g)` over the flat
/// distance, at least `min_flight_ms`).
fn lob(from: FxVec3, to: FxVec3, throw: &ThrowTuning) -> (FxVec3, FxVec3) {
    let dxy = to.xy() - from.xy();
    let flight = dxy
        .length()
        .mul_int(2)
        .mul(Fx::from_milli(throw.lob_tan_milli as i32))
        .div(GRAVITY)
        .sqrt()
        .max(Fx::from_milli(throw.min_flight_ms as i32));
    let vz = (to.z - from.z + GRAVITY.mul(flight).mul(flight).div_int(2)).div(flight);
    (from, FxVec3::new(dxy.x.div(flight), dxy.y.div(flight), vz))
}

#[cfg(test)]
mod tests {
    use super::*;
    use frontline_content::Fixed;
    use frontline_content::setup::SmokeSetup;

    #[test]
    fn clouds_grow_hold_and_thin_out() {
        let s = SmokeSetup {
            radius: Fixed::from_int(8),
            grow_ticks: 90,
            hold_ticks: 750,
            fade_ticks: 150,
        };
        assert_eq!(cloud_now(&s, 0), (Fx::ZERO, 1000));
        assert_eq!(cloud_now(&s, 45).0, Fx::from_int(4));
        assert_eq!(cloud_now(&s, 90), (Fx::from_int(8), 1000));
        assert_eq!(cloud_now(&s, 839), (Fx::from_int(8), 1000));
        assert_eq!(cloud_now(&s, 840), (Fx::from_int(8), 1000));
        assert_eq!(cloud_now(&s, 915), (Fx::from_int(8), 500));
        assert_eq!(cloud_now(&s, 989).1, 6);
        assert_eq!(cloud_now(&s, 990), (Fx::ZERO, 0));
    }

    #[test]
    fn lob_lands_on_the_target() {
        let from = FxVec3::new(Fx::from_int(100), Fx::from_int(100), Fx::from_milli(1600));
        let to = FxVec3::new(Fx::from_int(125), Fx::from_int(100), Fx::from_milli(-1600));
        let throw = ThrowTuning {
            release_drop_mm: 150,
            min_flight_ms: 300,
            lob_tan_milli: 1000,
            smoke_below_mm: 2000,
        };
        let (_, v) = lob(from, to, &throw);
        // About 45° up and 2.3 s in the air at 25 m.
        assert!((v.x - v.z).abs() < Fx::from_int(3), "{v:?}");
        let mut step = 0i64;
        let half_g = GRAVITY.div_int(2);
        let land = loop {
            step += 1;
            let s = Fx::from_ratio(step, SUBSTEP_HZ as i64);
            let mut p = from + v * s;
            p.z -= half_g.mul(s).mul(s);
            if p.z <= to.z {
                break (p, step);
            }
        };
        assert!(
            (land.0.x - to.x).abs() < Fx::from_milli(400),
            "{:?}",
            land.0
        );
        assert!((130..=150).contains(&land.1), "{} substeps", land.1);
    }

    fn setup() -> frontline_content::MatchSetup {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2");
        let defs = frontline_content::ModDefs::load(&root).unwrap();
        let req = frontline_content::MatchRequest::skirmish(
            1,
            "ww2:germany",
            "ww2:usa",
            frontline_content::Difficulty::Normal,
        );
        defs.resolve(&req).unwrap()
    }

    fn smoke_item(g: &MatchState) -> u16 {
        g.setup
            .items
            .iter()
            .position(|it| it.kind == ItemKind::Smoke)
            .unwrap() as u16
    }

    fn v3(x: i32, y: i32, z_mm: i32) -> FxVec3 {
        FxVec3::new(Fx::from_int(x), Fx::from_int(y), Fx::from_milli(z_mm))
    }

    #[test]
    fn smoke_blocks_lines_through_it_and_thins_out() {
        let mut g = MatchState::new(setup());
        let item = smoke_item(&g);
        let life = g.setup.items[item as usize]
            .smoke
            .as_ref()
            .unwrap()
            .life_ticks();
        let key = (SimId(1), SimId(2));
        let (a, b) = (v3(100, 100, 1500), v3(200, 100, 1500));
        assert!(!g.smoke_blocks(a, b, 0, key));
        g.start_smoke(item, v3(150, 100, 0));
        // Just started: no radius yet.
        assert!(!g.smoke_blocks(a, b, 0, key));
        g.smoke[0].age = 90;
        assert!(g.smoke_blocks(a, b, 0, key));
        // Beside it (8 m radius), over it (16 m tall), or ending short.
        assert!(!g.smoke_blocks(v3(100, 109, 1500), v3(200, 109, 1500), 0, key));
        assert!(g.smoke_blocks(v3(100, 107, 1500), v3(200, 107, 1500), 0, key));
        assert!(!g.smoke_blocks(v3(100, 100, 17_000), v3(200, 100, 17_000), 0, key));
        assert!(!g.smoke_blocks(a, v3(140, 100, 1500), 0, key));
        // From inside the cloud, either way.
        assert!(g.smoke_blocks(v3(150, 102, 1500), a, 0, key));
        // Thinning: blocked about as often as the density says.
        g.smoke[0].age = life - life / 33 * 5 / 2; // half way through the fade
        let (_, density) = cloud_now(
            g.setup.items[item as usize].smoke.as_ref().unwrap(),
            g.smoke[0].age,
        );
        assert!((400..=600).contains(&density), "{density}");
        let blocked = (0..2000u64)
            .filter(|&t| g.smoke_blocks(a, b, t, key))
            .count();
        assert!((800..=1200).contains(&blocked), "{blocked} of 2000");
        // Gone at the end of its life.
        let mut ents = Entities::default();
        let mut hash = HashCache::default();
        g.smoke[0].age = life - 2;
        g.update_items(&mut ents, &mut hash);
        assert_eq!(g.smoke.len(), 1);
        g.update_items(&mut ents, &mut hash);
        assert!(g.smoke.is_empty());
        assert!(!g.smoke_blocks(a, b, 0, key));
    }

    #[test]
    fn a_full_cloud_list_drops_the_oldest() {
        let mut g = MatchState::new(setup());
        let item = smoke_item(&g);
        for k in 0..MAX_SMOKE as i32 + 3 {
            g.start_smoke(item, v3(10 + k, 10, 0));
        }
        assert_eq!(g.smoke.len(), MAX_SMOKE);
        assert_eq!(g.smoke[0].id, 3);
        assert_eq!(g.smoke[MAX_SMOKE - 1].id, MAX_SMOKE as u32 + 2);
        assert!(g.smoke.windows(2).all(|w| w[0].id < w[1].id));
    }

    #[test]
    fn pinned_men_and_men_without_the_item_cannot_throw() {
        let terrain = Heightfield::generate(crate::terrain::TerrainKind::Flat, 1024, 1);
        let mut g = MatchState::new(setup());
        let ty = g.setup.squad_type_index("ww2:ger_grenadiergruppe").unwrap();
        let mut ents = Entities::default();
        let mut hash = HashCache::default();
        let mut ctx = Ctx {
            ents: &mut ents,
            terrain: &terrain,
            hash: &mut hash,
            tick: 0,
        };
        let sq = g.spawn_squad(&mut ctx, PlayerId(0), ty);
        let men = g.squads[&sq].members.clone();
        let target = |m: SimId| ents.transform[m.index()].pos.xy() + FxVec2::from_int(20, 0);
        // S4: man 4 is a rifleman with a frag (men 1-3: the MG team).
        let m = men[4];
        assert!(g.can_throw(&ents, m, ItemKind::Frag, target(m)).is_some());
        assert!(g.can_throw(&ents, m, ItemKind::Smoke, target(m)).is_none());
        assert!(
            g.can_throw(&ents, men[0], ItemKind::Smoke, target(men[0]))
                .is_some()
        );
        assert!(
            g.can_throw(&ents, m, ItemKind::Satchel, target(m))
                .is_none()
        );
        // S4: he carries a Panzerfaust, which is fired, never thrown.
        assert_eq!(ents.kit[m.index()].items[ItemKind::SingleShotAt.index()], 1);
        assert!(
            g.can_throw(&ents, m, ItemKind::SingleShotAt, target(m))
                .is_none()
        );
        ents.combat[m.index()].pinned = true;
        assert!(g.can_throw(&ents, m, ItemKind::Frag, target(m)).is_none());
        ents.combat[m.index()].pinned = false;
        ents.kit[m.index()].items[ItemKind::Frag.index()] = 0;
        assert!(g.can_throw(&ents, m, ItemKind::Frag, target(m)).is_none());
    }
}
