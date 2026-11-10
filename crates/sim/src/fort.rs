//! S10a: fortifications, materials, wire and breaching
//! (docs/slices/S10a-fortifications.md).
//!
//! * **Works.** `Fortify` starts a [`Work`] for a blueprint
//!   (`MatchSetup::forts`): pit blueprints dig their pit first (through
//!   `order_dig`, as the S6 `Build`), then the squad gets
//!   `Order::Fortify { work }` and builds segment by segment (line) or the
//!   whole site (point, over_hq). A work draws its kg when a build step
//!   starts: carried kg ([`FortState::carried_g`]), then own construction
//!   crates within `material_reach_m` (ascending id), then own trucks'
//!   `VehicleState::material_g`. A work whose builder takes another order or
//!   dies is removed, its kg lost (S6-8, S10a-6).
//! * **Pieces** are static boxes (`StaticRef::Piece`) with an armour value:
//!   a bullet or fragment must beat `armour_mm` to damage one, a
//!   non-bursting main-gun round takes `shot_piece_damage`, overpressure
//!   with a clear line the HE's `damage`. Own rounds pass own pieces
//!   (S10a-3). Overpressure on a man needs a line clear of every piece not
//!   owned by the burst's player (overhead cover).
//! * **Wire, cutting and Bangalore charges** (Build B) live here too.
//!
//! Every per-tick system returns at once when the state holds nothing (no
//! RNG draw, no list build): AI-vs-AI play without fortifications is
//! identical to S9b. All state is in [`FortState`] (saved, hashed through
//! postcard, validated on load).

use std::collections::BTreeMap;

use frontline_content::MatchSetup;
use frontline_content::setup::{Capability, FortShape, SupplyKind};
use frontline_content::tuning::FortTuning;
use serde::{Deserialize, Serialize};

use crate::ecs::{Entities, PlayerId};
use crate::fx::{Fx, FxVec2, FxVec3};
use crate::game::{Ctx, MatchState, NavPhase, Order, SquadId};
use crate::structures::StaticRef;
use crate::supply::{MICRO, in_own_base, micro_price};
use crate::terrain::Heightfield;

/// Caps on the saved lists (an untrusted save cannot grow them further).
pub const MAX_WORKS: usize = 256;
pub const MAX_PIECES: usize = 4096;
pub const MAX_WIRE: usize = 2048;
pub const MAX_CHARGES: usize = 64;
/// Most points of a `Fortify` line (as `Dig`).
pub const MAX_FORT_POINTS: usize = 16;

/// Where a work stands.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum WorkStep {
    /// The squad digs the blueprint's pit (`Order::Dig`).
    #[default]
    Dig = 0,
    /// The build step waits for its kg (`NoMaterials`).
    Wait = 1,
    /// The squad builds (`Order::Fortify`).
    Build = 2,
}

/// A fortification site being dug or built.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Work {
    pub owner: PlayerId,
    /// Index into `MatchSetup::forts`.
    pub fort: u16,
    /// Point and over_hq: `[pos, toward]`; line: the dragged points.
    pub line: Vec<FxVec2>,
    /// Facing (radians from +x).
    pub yaw: Fx,
    pub builder: SquadId,
    /// over_hq: the roofed HQ's structure id.
    pub hq: Option<u32>,
    pub step: WorkStep,
    /// Line: the segment being built (earlier ones stand).
    pub seg: u8,
    /// Work points done on the current step (`work_s x work_men x 100 x
    /// tick_hz` finishes it).
    pub work: u64,
    /// Grams drawn for the current step.
    pub got_g: u32,
}

/// A built box (sandbags, a roof).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Piece {
    pub owner: PlayerId,
    /// Index into `MatchSetup::forts`.
    pub fort: u16,
    /// Index into the def's `pieces`.
    pub part: u8,
    /// Box centre (xy) and bottom (z).
    pub pos: FxVec2,
    pub yaw: Fx,
    pub z: Fx,
    pub hp: u32,
    /// An HQ roof: the HQ's structure id (it goes with the HQ).
    pub hq: Option<u32>,
}

/// One standing barbed-wire section.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct WireSection {
    pub owner: PlayerId,
    /// Index into `MatchSetup::forts`.
    pub fort: u16,
    pub a: FxVec2,
    pub b: FxVec2,
    pub hp: u32,
}

/// A laid Bangalore charge waiting for its fuse.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Charge {
    /// Never reused (RNG key).
    pub id: u32,
    pub owner: PlayerId,
    /// Index into `MatchSetup::items`.
    pub item: u16,
    pub from: FxVec2,
    pub to: FxVec2,
    pub fuse_tick: u64,
}

/// S10a per-player counters (report; saved and hashed).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct FortAccount {
    pub works_started: u32,
    pub pieces_built: u32,
    pub pieces_lost: u32,
    pub wire_built: u32,
    pub wire_cut: u32,
    pub wire_blown: u32,
    pub wire_crushed: u32,
    pub bangalores: u32,
    /// Construction grams loaded (carried and trucks) and the µMP paid.
    pub materials_loaded_g: u64,
    pub materials_micro: u64,
    /// Grams drawn by works.
    pub materials_used_g: u64,
}

/// All S10a state (`MatchState::fort`).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct FortState {
    pub works: BTreeMap<u32, Work>,
    pub next_work: u32,
    pub pieces: BTreeMap<u32, Piece>,
    pub next_piece: u32,
    pub wire: BTreeMap<u32, WireSection>,
    pub next_wire: u32,
    /// Ascending id.
    pub charges: Vec<Charge>,
    pub next_charge: u32,
    /// Construction grams a squad carries.
    pub carried_g: BTreeMap<SquadId, u32>,
    /// Per player.
    pub accounts: Vec<FortAccount>,
}

/// Why an S10a command was refused (presentation only, like
/// `SupplyRefusal`).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum FortRefusal {
    /// Not the player's squad, no such blueprint or item.
    NotAllowed,
    /// The squad lacks the capability.
    CannotBuild,
    /// At the blueprint's `limit_per_player`.
    Cap,
    /// Spacing, slope, off the map, in an enemy base, no HQ to roof.
    BadSpot,
    /// Short of manpower by this much.
    Manpower { short: u32 },
    /// Not in the own base (loading).
    NotInBase,
    /// Nothing to cut / no wire within reach.
    NoWire,
}

/// One refused S10a command (presentation only).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct FortEvent {
    pub player: PlayerId,
    pub squad: Option<SquadId>,
    pub refusal: FortRefusal,
}

impl FortState {
    pub fn new(players: usize) -> FortState {
        FortState {
            accounts: vec![FortAccount::default(); players],
            ..FortState::default()
        }
    }

    /// Whether nothing is built, building, laid or carried: every S10a
    /// per-tick system returns at once then.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.works.is_empty()
            && self.pieces.is_empty()
            && self.wire.is_empty()
            && self.charges.is_empty()
            && self.carried_g.is_empty()
    }

    /// Checks a loaded state (untrusted saves).
    pub(crate) fn validate(
        &self,
        setup: &MatchSetup,
        ents: &Entities,
        squads: &BTreeMap<SquadId, crate::game::Squad>,
    ) -> Result<(), &'static str> {
        let np = setup.players.len();
        if self.accounts.len() != np {
            return Err("S10a accounts disagree with the players");
        }
        if self.works.len() > MAX_WORKS
            || self.pieces.len() > MAX_PIECES
            || self.wire.len() > MAX_WIRE
            || self.charges.len() > MAX_CHARGES
        {
            return Err("too many S10a works, pieces, wire sections or charges");
        }
        if [
            self.next_work,
            self.next_piece,
            self.next_wire,
            self.next_charge,
        ]
        .contains(&u32::MAX)
        {
            return Err("an S10a id counter is exhausted");
        }
        let size = Fx::from_int(setup.map.size_m);
        let on_map = |p: FxVec2| [p.x, p.y].iter().all(|v| *v >= Fx::ZERO && *v <= size);
        let fort = |f: u16| setup.forts.get(f as usize);
        for (&id, w) in &self.works {
            let Some(d) = fort(w.fort) else {
                return Err("work blueprint out of range");
            };
            if id >= self.next_work || w.owner.0 as usize >= np {
                return Err("work id or owner out of range");
            }
            if w.line.is_empty()
                || w.line.len() > MAX_FORT_POINTS
                || !w.line.iter().all(|&p| on_map(p))
            {
                return Err("work line empty, too long or off the map");
            }
            if squads.get(&w.builder).is_none_or(|q| q.owner != w.owner) {
                return Err("work builder is not an own squad");
            }
            if w.got_g > d.materials_g {
                return Err("work holds more kg than its step needs");
            }
        }
        for (&id, p) in &self.pieces {
            let Some(d) = fort(p.fort) else {
                return Err("piece blueprint out of range");
            };
            let Some(b) = d.pieces.get(p.part as usize) else {
                return Err("piece part out of range");
            };
            if id >= self.next_piece || p.owner.0 as usize >= np || !on_map(p.pos) {
                return Err("piece id, owner or position out of range");
            }
            if p.hp == 0 || p.hp > b.hp {
                return Err("piece hit points out of range");
            }
        }
        for (&id, s) in &self.wire {
            let Some(d) = fort(s.fort) else {
                return Err("wire blueprint out of range");
            };
            let Some(wd) = &d.wire else {
                return Err("wire section of a blueprint without wire");
            };
            if id >= self.next_wire || s.owner.0 as usize >= np || !on_map(s.a) || !on_map(s.b) {
                return Err("wire id, owner or position out of range");
            }
            if s.hp == 0 || s.hp > wd.hp {
                return Err("wire hit points out of range");
            }
        }
        let mut last: Option<u32> = None;
        for c in &self.charges {
            if last.is_some_and(|l| c.id <= l) || c.id >= self.next_charge {
                return Err("charges not in ascending id order");
            }
            last = Some(c.id);
            if c.owner.0 as usize >= np || !on_map(c.from) || !on_map(c.to) {
                return Err("charge owner or position out of range");
            }
            if setup
                .items
                .get(c.item as usize)
                .is_none_or(|it| it.bangalore.is_none())
            {
                return Err("charge item is not a Bangalore");
            }
        }
        let per_man = setup
            .rules
            .fortify
            .as_ref()
            .map_or(0, |f| f.carry_kg_per_man as u64 * 1000);
        for (sid, &g) in &self.carried_g {
            let Some(q) = squads.get(sid) else {
                return Err("construction kg carried by no squad");
            };
            let alive = q.members.iter().filter(|m| ents.is_alive(**m)).count() as u64;
            if g == 0 || g as u64 > per_man * alive.max(q.members.len() as u64) {
                return Err("carried construction kg above the squad's capacity");
            }
        }
        Ok(())
    }
}

impl MatchState {
    /// S10a: the fortification state (read-only; snapshot, CLI, tests).
    pub fn fort(&self) -> &FortState {
        &self.fort
    }

    /// Records a refused S10a command; returns false (not applied).
    pub(crate) fn refuse_fort(
        &mut self,
        player: PlayerId,
        squad: Option<SquadId>,
        refusal: FortRefusal,
    ) -> bool {
        self.fort_events.push(FortEvent {
            player,
            squad,
            refusal,
        });
        false
    }

    /// Pays `micro` µMP that the caller checked `p` has, as `pay` pays a
    /// round but without its per-kind round counters (construction is
    /// outside the three-wide arrays).
    pub(crate) fn pay_micro(&mut self, p: usize, micro: u64) {
        let pl = &mut self.players[p];
        let credit = pl.ammo.credit as u64;
        if micro > credit {
            let k = (micro - credit).div_ceil(MICRO);
            pl.manpower -= k as u32;
            pl.ammo.credit = (credit + k * MICRO - micro) as u32;
        } else {
            pl.ammo.credit = (credit - micro) as u32;
        }
    }

    /// Tier rates of squad type `ty`: construction (`build_heavy`), else
    /// assault (`breach`), else field.
    pub(crate) fn tier_of(
        ty: &frontline_content::setup::SquadTypeSetup,
        t: &FortTuning,
    ) -> frontline_content::tuning::TierTuning {
        if ty.has(Capability::BuildHeavy) {
            t.tiers.construction.clone()
        } else if ty.has(Capability::Breach) {
            t.tiers.assault.clone()
        } else {
            t.tiers.field.clone()
        }
    }

    /// Sites (point, over_hq) or segments (line) player `p` has of
    /// blueprint `f`, built or building.
    fn fort_used(&self, p: PlayerId, f: u16) -> u32 {
        let d = &self.setup.forts[f as usize];
        let mut n = 0u32;
        for w in self.fort.works.values() {
            if w.owner == p && w.fort == f {
                n += match d.shape {
                    FortShape::Line => segments(&w.line, d.segment).saturating_sub(w.seg as u32),
                    _ => 1,
                };
            }
        }
        n + match d.shape {
            FortShape::Line if d.wire.is_some() => self
                .fort
                .wire
                .values()
                .filter(|s| s.owner == p && s.fort == f)
                .count() as u32,
            _ => self
                .fort
                .pieces
                .values()
                .filter(|q| q.owner == p && q.fort == f && q.part == 0)
                .count() as u32,
        }
    }

    /// `Fortify { squad, fort, line }` (section 4).
    pub(crate) fn order_fortify(
        &mut self,
        ctx: &mut Ctx,
        p: PlayerId,
        squad: SquadId,
        fort: u16,
        line: &[FxVec2],
    ) -> bool {
        if self.setup.rules.fortify.is_none() {
            return false;
        }
        if !self.owned(p, squad) || self.is_vehicle_squad(squad) {
            return self.refuse_fort(p, Some(squad), FortRefusal::NotAllowed);
        }
        let Some(d) = self.setup.forts.get(fort as usize).cloned() else {
            return self.refuse_fort(p, Some(squad), FortRefusal::NotAllowed);
        };
        if self.fort.works.len() >= MAX_WORKS {
            return self.refuse_fort(p, Some(squad), FortRefusal::Cap);
        }
        let ty = &self.setup.squad_types[self.squads[&squad].type_idx as usize];
        let cap_ok = ty.caps & (1 << d.capability) != 0;
        if !cap_ok || d.pit.is_some_and(|pit| !self.digs_profile(ty, pit)) {
            return self.refuse_fort(p, Some(squad), FortRefusal::CannotBuild);
        }
        let point = d.shape != FortShape::Line;
        let len = crate::squad::polyline_length(line);
        let shape_ok = if point {
            line.len() == 2
        } else {
            (2..=MAX_FORT_POINTS).contains(&line.len())
                && len > Fx::ZERO
                && len <= Fx::from(d.max_length)
        };
        if !shape_ok {
            return self.refuse_fort(p, Some(squad), FortRefusal::BadSpot);
        }
        let enemy_base = |q: FxVec2| {
            (0..self.setup.players.len())
                .filter(|&o| o != p.0 as usize)
                .any(|o| in_own_base(&self.setup, PlayerId(o as u8), q))
        };
        let pts: &[FxVec2] = if point { &line[..1] } else { line };
        if pts.iter().any(|&q| {
            !ctx.terrain.contains(q) || enemy_base(q) || slope_pct(ctx.terrain, q) > d.max_slope_pct
        }) {
            return self.refuse_fort(p, Some(squad), FortRefusal::BadSpot);
        }
        let need = if point { 1 } else { segments(line, d.segment) };
        if self.fort_used(p, fort) + need > d.limit_per_player {
            return self.refuse_fort(p, Some(squad), FortRefusal::Cap);
        }
        let mut pos = line[0];
        let toward = line[1] - line[0];
        let mut yaw = if toward == FxVec2::ZERO {
            Fx::ZERO
        } else {
            Fx::atan2(toward.y, toward.x)
        };
        let mut hq = None;
        if d.shape == FortShape::OverHq {
            let five = Fx::from_int(5);
            let roofed = |id: u32| {
                self.fort.pieces.values().any(|q| q.hq == Some(id))
                    || self.fort.works.values().any(|w| w.hq == Some(id))
            };
            let found = self.supply6.structures.iter().find(|(id, s)| {
                s.owner == p
                    && s.built
                    && self.setup.stocks[s.def as usize].kind
                        == frontline_content::setup::StockKind::Hq
                    && (s.pos - pos).length_sq_raw() <= (five.raw() as u64).pow(2)
                    && !roofed(**id)
            });
            let Some((&id, s)) = found else {
                return self.refuse_fort(p, Some(squad), FortRefusal::BadSpot);
            };
            hq = Some(id);
            pos = s.pos;
            yaw = s.yaw;
        } else if point {
            let sp = Fx::from(d.min_spacing);
            let near = |q: FxVec2| (q - pos).length_sq_raw() < (sp.raw() as u64).pow(2);
            if self
                .fort
                .works
                .values()
                .any(|w| w.owner == p && w.fort == fort && near(w.line[0]))
                || self
                    .fort
                    .pieces
                    .values()
                    .any(|q| q.owner == p && q.fort == fort && q.part == 0 && near(q.pos))
            {
                return self.refuse_fort(p, Some(squad), FortRefusal::BadSpot);
            }
        }
        let id = self.fort.next_work;
        let mut w = Work {
            owner: p,
            fort,
            line: if point {
                vec![pos, line[1]]
            } else {
                line.to_vec()
            },
            yaw,
            builder: squad,
            hq,
            step: WorkStep::Wait,
            seg: 0,
            work: 0,
            got_g: 0,
        };
        if let Some(pit) = d.pit {
            let half = Fx::from(self.setup.rules.dig_profile(pit).pit_line)
                .div_int(2)
                .max(Fx::from_milli(100));
            let f = dir(yaw);
            if !self.order_dig(ctx, p, squad, &[pos - f * half, pos + f * half], pit) {
                return self.refuse_fort(p, Some(squad), FortRefusal::BadSpot);
            }
            w.step = WorkStep::Dig;
        } else {
            self.order_move(ctx, p, squad, Order::Fortify { work: id });
        }
        self.fort.next_work += 1;
        self.fort.works.insert(id, w);
        self.fort.accounts[p.0 as usize].works_started += 1;
        true
    }

    /// `LoadMaterials { squad, kg }` (section 3.3): in the own base, paid
    /// there; a builder squad carries it, a truck's squad fills its truck.
    pub(crate) fn load_materials(
        &mut self,
        ctx: &mut Ctx,
        p: PlayerId,
        squad: SquadId,
        kg: u16,
    ) -> bool {
        let Some(t) = self.setup.rules.fortify.clone() else {
            return false;
        };
        if !self.owned(p, squad) || kg == 0 {
            return self.refuse_fort(p, Some(squad), FortRefusal::NotAllowed);
        }
        let sq = &self.squads[&squad];
        let lead = sq.lead(ctx.ents);
        if !in_own_base(&self.setup, p, ctx.ents.transform[lead.index()].pos.xy()) {
            return self.refuse_fort(p, Some(squad), FortRefusal::NotInBase);
        }
        let ask = kg as u32 * 1000;
        // A truck: its free capacity (cargo plus material within the cap).
        let truck = sq.members.first().copied().filter(|v| {
            self.vehicles.get(v).is_some_and(|vs| {
                vs.wreck_tick.is_none()
                    && self.setup.vehicles[vs.type_idx as usize].supply.is_some()
            })
        });
        let room = if let Some(v) = truck {
            let vs = &self.vehicles[&v];
            let cap = self.setup.vehicles[vs.type_idx as usize]
                .supply
                .map_or(0, |s| s.cap_g) as u64;
            let held = vs.cargo_g.iter().map(|&g| g as u64).sum::<u64>() + vs.material_g as u64;
            cap.saturating_sub(held).min(u32::MAX as u64) as u32
        } else {
            let ty = &self.setup.squad_types[sq.type_idx as usize];
            if !ty.has(Capability::BuildStructure) && !ty.has(Capability::PlaceWire) {
                return self.refuse_fort(p, Some(squad), FortRefusal::CannotBuild);
            }
            let alive = sq.members.iter().filter(|m| ctx.ents.is_alive(**m)).count() as u32;
            let cap = t.carry_kg_per_man * 1000 * alive;
            cap.saturating_sub(self.fort.carried_g.get(&squad).copied().unwrap_or(0))
        };
        let g = ask.min(room);
        if g == 0 {
            return self.refuse_fort(p, Some(squad), FortRefusal::Cap);
        }
        let micro = micro_price(&self.setup, SupplyKind::Construction, g);
        let pl = &self.players[p.0 as usize];
        let have = pl.manpower as u64 * MICRO + pl.ammo.credit as u64;
        if have < micro {
            let short = (micro - have).div_ceil(MICRO) as u32;
            return self.refuse_fort(p, Some(squad), FortRefusal::Manpower { short });
        }
        self.pay_micro(p.0 as usize, micro);
        match truck {
            Some(v) => self.vehicles.get_mut(&v).expect("truck").material_g += g,
            None => *self.fort.carried_g.entry(squad).or_insert(0) += g,
        }
        let acc = &mut self.fort.accounts[p.0 as usize];
        acc.materials_loaded_g += g as u64;
        acc.materials_micro += micro;
        true
    }

    /// The point a work's current step builds at: a point site, or the
    /// middle of the line's current segment.
    fn site_of(&self, w: &Work) -> (FxVec2, Fx) {
        let d = &self.setup.forts[w.fort as usize];
        if d.shape != FortShape::Line {
            return (w.line[0], w.yaw);
        }
        let seg = Fx::from(d.segment);
        let len = crate::squad::polyline_length(&w.line);
        let a = seg.mul_int(w.seg as i32).min(len);
        let b = (a + seg).min(len);
        let pa = crate::squad::point_along(&w.line, a);
        let pb = crate::squad::point_along(&w.line, b);
        let v = pb - pa;
        let yaw = if v == FxVec2::ZERO {
            Fx::ZERO
        } else {
            Fx::atan2(v.y, v.x)
        };
        (pa + v.div_int(2), yaw)
    }

    /// S10a per-tick works (section 5 rule 7): pit, materials, build.
    /// Returns at once with nothing to do.
    pub(crate) fn fort_tick(&mut self, ents: &mut Entities, terrain: &Heightfield) {
        if self.fort.is_empty() || self.fort_off {
            return;
        }
        let Some(t) = self.setup.rules.fortify.clone() else {
            return;
        };
        // An HQ roof goes with its HQ.
        let gone: Vec<u32> = self
            .fort
            .pieces
            .iter()
            .filter(|(_, q)| {
                q.hq.is_some_and(|h| self.supply6.structures.get(&h).is_none_or(|s| !s.built))
            })
            .map(|(&id, _)| id)
            .collect();
        for id in gone {
            let q = self.fort.pieces.remove(&id).expect("listed");
            self.fort.accounts[q.owner.0 as usize].pieces_lost += 1;
        }
        let reach = Fx::from_milli(self.setup.rules.dig.common.reach_mm as i32);
        let crouch = self.setup.rules.combat.crouch_suppression;
        let ids: Vec<u32> = self.fort.works.keys().copied().collect();
        for id in ids {
            let w = self.fort.works[&id].clone();
            let d = self.setup.forts[w.fort as usize].clone();
            let alive = self
                .squads
                .get(&w.builder)
                .is_some_and(|q| q.owner == w.owner && q.members.iter().any(|m| ents.is_alive(*m)));
            let hq_ok =
                w.hq.is_none_or(|h| self.supply6.structures.get(&h).is_some_and(|s| s.built));
            let order = self.squads.get(&w.builder).map(|q| q.order.clone());
            let keep = alive
                && hq_ok
                && match w.step {
                    WorkStep::Dig => matches!(order, Some(Order::Dig { .. } | Order::Idle)),
                    _ => order == Some(Order::Fortify { work: id }),
                };
            if !keep {
                self.fort.works.remove(&id);
                continue;
            }
            if w.step == WorkStep::Dig {
                if matches!(order, Some(Order::Dig { .. })) {
                    continue;
                }
                let pit = d.pit.expect("a pit work");
                let depth = Fx::from(self.setup.rules.dig_profile(pit).depth);
                let c = w.line[0];
                let dug = terrain.base_height_at(c) - terrain.height_at(c);
                if dug.mul_int(100) < depth.mul_int(crate::structures::BUILT_DEPTH_PCT) {
                    self.fort.works.remove(&id);
                    continue;
                }
                let sq = self.squads.get_mut(&w.builder).expect("alive");
                sq.order = Order::Fortify { work: id };
                self.request_path(w.builder, c);
                self.fort.works.get_mut(&id).expect("listed").step = WorkStep::Wait;
                continue;
            }
            let (site, yaw) = self.site_of(&w);
            if w.step == WorkStep::Wait {
                let got = self.draw_materials(
                    ents,
                    w.owner,
                    w.builder,
                    site,
                    d.materials_g - w.got_g,
                    &t,
                );
                let wm = self.fort.works.get_mut(&id).expect("listed");
                wm.got_g += got;
                if wm.got_g >= d.materials_g {
                    wm.step = WorkStep::Build;
                } else {
                    continue;
                }
            }
            // Build: men walk to spots around the site and work there.
            let ty = &self.setup.squad_types[self.squads[&w.builder].type_idx as usize];
            let rate = Self::tier_of(ty, &t).build_pct as u64;
            let ring = d
                .pieces
                .iter()
                .map(|b| {
                    Fx::from(b.half[0]).max(Fx::from(b.half[1]))
                        + Fx::from(b.offset[0]).abs().max(Fx::from(b.offset[1]).abs())
                })
                .max()
                .unwrap_or(Fx::ONE)
                + Fx::ONE;
            let sq = self.squads.get_mut(&w.builder).expect("alive");
            let n = sq.members.len().max(1) as i32;
            let mut add = 0u64;
            if matches!(sq.nav.phase, NavPhase::Arriving | NavPhase::Holding)
                && sq.nav.slots.len() == sq.members.len()
            {
                for (k, &m) in sq.members.iter().enumerate() {
                    let a = Fx::TAU.mul_int(k as i32).div_int(n) + yaw;
                    let spot = terrain.clamp(site + dir(a) * ring);
                    sq.nav.slots[k] = spot;
                    let i = m.index();
                    if !ents.is_alive(m) || ents.unit[i].detached {
                        continue;
                    }
                    let c = &ents.combat[i];
                    if c.pinned || (spot - ents.transform[i].pos.xy()).length() > reach {
                        continue;
                    }
                    add += if c.suppression / 1000 >= crouch {
                        rate * 3 / 10
                    } else {
                        rate
                    };
                }
            }
            let need = d.work_ticks as u64 * d.work_men as u64 * 100;
            let wm = self.fort.works.get_mut(&id).expect("listed");
            wm.work = (wm.work + add).min(need);
            if wm.work < need {
                continue;
            }
            self.stand_segment(terrain, id, site, yaw);
        }
    }

    /// Draws up to `need` construction grams for a work of `p`'s squad
    /// `builder` at `site`: carried, then own crates within
    /// `material_reach_m` (ascending id), then own trucks (ascending id).
    pub(crate) fn draw_materials(
        &mut self,
        ents: &Entities,
        p: PlayerId,
        builder: SquadId,
        site: FxVec2,
        need: u32,
        t: &FortTuning,
    ) -> u32 {
        let mut got = 0u32;
        if need == 0 {
            return 0;
        }
        if let Some(c) = self.fort.carried_g.get_mut(&builder) {
            let g = (*c).min(need);
            *c -= g;
            got += g;
            if *c == 0 {
                self.fort.carried_g.remove(&builder);
            }
        }
        let r = Fx::from_int(t.material_reach_m as i32);
        let r2 = (r.raw() as u64).pow(2);
        if got < need {
            let ids: Vec<u32> = self
                .supply6
                .crates
                .iter()
                .filter(|(_, c)| {
                    c.owner == p
                        && c.carrier.is_none()
                        && self.setup.stocks[c.def as usize].supply
                            == Some(SupplyKind::Construction)
                        && (c.pos - site).length_sq_raw() <= r2
                })
                .map(|(&id, _)| id)
                .collect();
            for id in ids {
                let c = self.supply6.crates.get_mut(&id).expect("listed");
                let g = c.g.min(need - got);
                c.g -= g;
                got += g;
                if c.g == 0 {
                    self.supply6.crates.remove(&id);
                }
                if got == need {
                    break;
                }
            }
        }
        if got < need {
            for (&v, vs) in self.vehicles.iter_mut() {
                if got == need {
                    break;
                }
                if vs.material_g == 0
                    || vs.wreck_tick.is_some()
                    || ents.owner[v.index()] != p
                    || (ents.transform[v.index()].pos.xy() - site).length_sq_raw() > r2
                {
                    continue;
                }
                let g = vs.material_g.min(need - got);
                vs.material_g -= g;
                got += g;
            }
        }
        self.fort.accounts[p.0 as usize].materials_used_g += got as u64;
        got
    }

    /// The current step of work `id` is done: its pieces (or wire section)
    /// stand; a line goes on to its next segment, else the work ends.
    fn stand_segment(&mut self, terrain: &Heightfield, id: u32, site: FxVec2, yaw: Fx) {
        let w = self.fort.works[&id].clone();
        let d = self.setup.forts[w.fort as usize].clone();
        let acc = w.owner.0 as usize;
        if let Some(wd) = &d.wire {
            let half = dir(yaw) * Fx::from(d.segment).div_int(2);
            let sid = self.fort.next_wire;
            self.fort.next_wire += 1;
            self.fort.wire.insert(
                sid,
                WireSection {
                    owner: w.owner,
                    fort: w.fort,
                    a: terrain.clamp(site - half),
                    b: terrain.clamp(site + half),
                    hp: wd.hp,
                },
            );
            self.fort.accounts[acc].wire_built += 1;
        } else {
            let f = dir(yaw);
            let left = f.perp();
            for (k, b) in d.pieces.iter().enumerate() {
                if self.fort.pieces.len() >= MAX_PIECES {
                    break;
                }
                let pos =
                    terrain.clamp(site + f * Fx::from(b.offset[0]) + left * Fx::from(b.offset[1]));
                let ground = if b.z_from_base {
                    terrain.base_height_at(pos)
                } else {
                    terrain.height_at(pos)
                };
                let pid = self.fort.next_piece;
                self.fort.next_piece += 1;
                self.fort.pieces.insert(
                    pid,
                    Piece {
                        owner: w.owner,
                        fort: w.fort,
                        part: k as u8,
                        pos,
                        yaw,
                        z: ground + Fx::from(b.offset[2]),
                        hp: b.hp,
                        hq: w.hq,
                    },
                );
                self.fort.accounts[acc].pieces_built += 1;
            }
        }
        let more = d.shape == FortShape::Line && (w.seg as u32 + 1) < segments(&w.line, d.segment);
        if more {
            let wm = self.fort.works.get_mut(&id).expect("listed");
            wm.seg += 1;
            wm.work = 0;
            wm.got_g = 0;
            wm.step = WorkStep::Wait;
        } else {
            self.fort.works.remove(&id);
            if let Some(q) = self.squads.get_mut(&w.builder) {
                q.order = Order::Idle;
            }
        }
    }

    /// Staging (`World::build_now`): every segment (or the site) of
    /// blueprint `fort` stands at once for `owner`, no builder, no kg.
    pub(crate) fn build_now(
        &mut self,
        terrain: &Heightfield,
        fort: u16,
        line: &[FxVec2],
        owner: PlayerId,
    ) -> bool {
        let Some(d) = self.setup.forts.get(fort as usize).cloned() else {
            return false;
        };
        if line.len() < 2 || owner.0 as usize >= self.setup.players.len() {
            return false;
        }
        let toward = line[1] - line[0];
        let yaw = if toward == FxVec2::ZERO {
            Fx::ZERO
        } else {
            Fx::atan2(toward.y, toward.x)
        };
        let hq = if d.shape == FortShape::OverHq {
            self.supply6
                .structures
                .iter()
                .find(|(_, s)| {
                    s.owner == owner && s.built && (s.pos - line[0]).length() <= Fx::from_int(5)
                })
                .map(|(&id, _)| id)
        } else {
            None
        };
        // A transient work: never saved (removed below in this call).
        let id = self.fort.next_work;
        self.fort.works.insert(
            id,
            Work {
                owner,
                fort,
                line: line.to_vec(),
                yaw,
                builder: SquadId(u32::MAX),
                hq,
                step: WorkStep::Build,
                seg: 0,
                work: 0,
                got_g: 0,
            },
        );
        let n = if d.shape == FortShape::Line {
            segments(line, d.segment)
        } else {
            1
        };
        for _ in 0..n {
            let w = self.fort.works[&id].clone();
            let (site, y) = self.site_of(&w);
            self.stand_segment(terrain, id, site, y);
        }
        self.fort.works.remove(&id);
        true
    }

    /// Section 5 rule 3: whether a piece not owned by `player` stands on
    /// the segment `a -> b` (false at once without pieces).
    pub(crate) fn piece_between(&self, a: FxVec3, b: FxVec3, player: u8) -> bool {
        if self.fort.pieces.is_empty() {
            return false;
        }
        let d = b - a;
        self.statics.0.iter().any(|sb| {
            matches!(sb.of, StaticRef::Piece(_))
                && sb.owner != player
                && sb.reach >= Fx::ZERO
                && crate::armour::segment_near(a, d, sb.frame.center, sb.reach)
                && crate::armour::segment_box(&sb.frame, a, d).is_some()
        })
    }
}

/// Segments of a line blueprint (at least one).
pub(crate) fn segments(line: &[FxVec2], segment: frontline_content::setup::Fixed) -> u32 {
    let len = crate::squad::polyline_length(line);
    let seg = Fx::from(segment).max(Fx::from_milli(100));
    let n = len.div(seg);
    let whole = n.floor_int().max(0) as u32;
    if Fx::from_int(whole as i32) < n {
        whole + 1
    } else {
        whole.max(1)
    }
}

/// The ground slope at `p` (%): the steeper of the x and y differences
/// over 2 m.
pub(crate) fn slope_pct(terrain: &Heightfield, p: FxVec2) -> u32 {
    let one = Fx::ONE;
    let h = |dx: Fx, dy: Fx| terrain.height_at(terrain.clamp(FxVec2::new(p.x + dx, p.y + dy)));
    let sx = (h(one, Fx::ZERO) - h(-one, Fx::ZERO)).abs();
    let sy = (h(Fx::ZERO, one) - h(Fx::ZERO, -one)).abs();
    (sx.max(sy).mul_int(50)).floor_int().max(0) as u32
}

fn dir(a: Fx) -> FxVec2 {
    let (s, c) = a.sin_cos();
    FxVec2::new(c, s)
}
