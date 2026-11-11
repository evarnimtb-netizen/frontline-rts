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

use crate::ecs::{Entities, PlayerId, SimId};
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
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
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
    /// S10a-ai (spec 3.5): grams lost, never refunded: carried kg trimmed
    /// with the men or squad, a truck's kg at its wreck, the kg a removed
    /// work had drawn.
    pub materials_lost_g: u64,
    /// S10a-ai: own men killed inside an enemy wire band.
    pub men_lost_in_wire: u32,
    /// S10a-ai: works completed, by blueprint (index into
    /// `MatchSetup::forts`; grows to the blueprint on its first work).
    pub works_done: Vec<u32>,
    /// S10a-ai: the grams of `materials_used_g` drawn from construction
    /// crates and from trucks (the rest was carried).
    pub materials_used_crate_g: u64,
    pub materials_used_truck_g: u64,
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
        if self.accounts.iter().any(|a| {
            a.works_done.len() > setup.forts.len()
                || a.materials_used_crate_g + a.materials_used_truck_g > a.materials_used_g
        }) {
            return Err("S10a account works or materials out of range");
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
            if w.step == WorkStep::Dig && d.pit.is_none() {
                return Err("work digs a pit its blueprint has not");
            }
            if (d.shape == FortShape::OverHq) != w.hq.is_some() {
                return Err("work roofs no HQ, or an HQ it is not over");
            }
            if d.shape != FortShape::Line && w.line.len() != 2 {
                return Err("point work without its facing point");
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
        // Every point on the map (a point site's facing point too: it is
        // stored with the work); the site's points (and a line's every
        // path-node step along it) on fair ground outside enemy bases.
        if line.iter().any(|&q| !ctx.terrain.contains(q))
            || along(pts).any(|q| enemy_base(q) || slope_pct(ctx.terrain, q) > d.max_slope_pct)
        {
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
        }
        self.fort.next_work += 1;
        self.fort.works.insert(id, w);
        if self.fort.works[&id].step != WorkStep::Dig {
            let (site, _) = self.site_of(&self.fort.works[&id]);
            self.clear_detached(ctx, squad);
            self.squads.get_mut(&squad).expect("owned").order = Order::Fortify { work: id };
            self.request_path(squad, site);
        }
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

    /// Carried kg follow the squads that carry them: a squad gone (wiped
    /// out, boarded, merged) loses its kg, and one that lost men keeps at
    /// most `carry_kg_per_man` per man left (the rest is lost with the men;
    /// no counter). Nothing to do with nothing carried.
    pub(crate) fn trim_carried(&mut self) {
        if self.fort.carried_g.is_empty() {
            return;
        }
        let per_man = self
            .setup
            .rules
            .fortify
            .as_ref()
            .map_or(0, |f| f.carry_kg_per_man as u64 * 1000);
        let squads = &self.squads;
        let accounts = &mut self.fort.accounts;
        self.fort.carried_g.retain(|sid, g| {
            // A squad removed without `lose_squad_kg` (none known): its
            // owner is gone with it, so the kg go uncounted.
            let Some(q) = squads.get(sid) else {
                return false;
            };
            let cap = (per_man * q.members.len() as u64).min(u32::MAX as u64) as u32;
            // S10a-ai: the trimmed grams are lost (spec 3.5).
            if let Some(a) = accounts.get_mut(q.owner.0 as usize) {
                a.materials_lost_g += (*g).saturating_sub(cap) as u64;
            }
            *g = (*g).min(cap);
            *g > 0
        });
    }

    /// S10a-ai: squad `sid` is about to be removed: the construction kg it
    /// carries are lost for its owner (spec 3.5). Call before
    /// `squads.remove`.
    pub(crate) fn lose_squad_kg(&mut self, sid: SquadId) {
        let Some(g) = self.fort.carried_g.remove(&sid) else {
            return;
        };
        if let Some(a) = self
            .squads
            .get(&sid)
            .and_then(|q| self.fort.accounts.get_mut(q.owner.0 as usize))
        {
            a.materials_lost_g += g as u64;
        }
    }

    /// The point a work's current step builds at: a point site, or the
    /// middle of the line's current segment.
    pub(crate) fn site_of(&self, w: &Work) -> (FxVec2, Fx) {
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
        // Squads boarded, recrewed or sent home this tick lose their kg.
        self.trim_carried();
        let Some(t) = self.setup.rules.fortify.clone() else {
            return;
        };
        self.wire_tick(ents, terrain, &t);
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
                // Its builder, still on it (its HQ went), stops.
                self.lose_work(id);
                if let Some(q) = self.squads.get_mut(&w.builder)
                    && q.order == (Order::Fortify { work: id })
                {
                    q.order = Order::Idle;
                }
                continue;
            }
            if w.step == WorkStep::Dig {
                if matches!(order, Some(Order::Dig { .. })) {
                    continue;
                }
                let Some(pit) = d.pit else {
                    // Never on a validated state: no pit to wait for.
                    self.lose_work(id);
                    continue;
                };
                let depth = Fx::from(self.setup.rules.dig_profile(pit).depth);
                let c = w.line[0];
                let dug = terrain.base_height_at(c) - terrain.height_at(c);
                if dug.mul_int(100) < depth.mul_int(crate::structures::BUILT_DEPTH_PCT) {
                    self.lose_work(id);
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
                self.fort.accounts[p.0 as usize].materials_used_crate_g += g as u64;
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
                self.fort.accounts[p.0 as usize].materials_used_truck_g += g as u64;
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
            // S10a-ai: a completed work, by blueprint.
            let done = &mut self.fort.accounts[acc].works_done;
            if done.len() <= w.fort as usize {
                done.resize(w.fort as usize + 1, 0);
            }
            done[w.fort as usize] += 1;
            if let Some(q) = self.squads.get_mut(&w.builder) {
                q.order = Order::Idle;
            }
        }
    }

    /// Removes work `id` unfinished: the kg it drew for its current step
    /// are lost for its owner (S10a-ai spec 3.5).
    fn lose_work(&mut self, id: u32) {
        let Some(w) = self.fort.works.remove(&id) else {
            return;
        };
        if let Some(a) = self.fort.accounts.get_mut(w.owner.0 as usize) {
            a.materials_lost_g += w.got_g as u64;
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
        if self.fort.pieces.is_empty() || self.fort_off {
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

// ------------------------------------------------------------ wire (B) --

/// A man this close (m) to a section's line works on cutting it (rule 5).
const CUT_REACH: Fx = Fx::from_ratio(3, 2);
/// A cutter stands this far (m) from the section's middle, on his side.
const CUT_STAND: Fx = Fx::ONE;
/// A Bangalore carrier walks to within this (m) of his target, and places
/// it from there (section 4).
const PLACE_REACH: Fx = Fx::from_int(2);
/// Slack (m) on [`PLACE_REACH`] for the man's arrival.
const PLACE_SLACK: Fx = Fx::from_ratio(1, 2);
/// Grid spacing of the path nodes (m; `crate::path`).
const NODE_M: i32 = 4;
/// A charge's bursts get keys `id * CHARGE_KEYS + k` (k < 8).
const CHARGE_KEYS: u32 = 8;

/// The point of segment `a -> b` closest to `p`.
pub(crate) fn closest_on(p: FxVec2, a: FxVec2, b: FxVec2) -> FxVec2 {
    let v = b - a;
    let vv = v.dot_raw(v) as i128;
    if vv == 0 {
        return a;
    }
    let t = ((p - a).dot_raw(v) as i128).clamp(0, vv);
    a + FxVec2::new(
        Fx::from_raw((v.x.raw() as i128 * t / vv) as i32),
        Fx::from_raw((v.y.raw() as i128 * t / vv) as i32),
    )
}

/// Whether `p` lies within `r` of segment `a -> b`.
pub(crate) fn near_segment(p: FxVec2, a: FxVec2, b: FxVec2, r: Fx) -> bool {
    (p - closest_on(p, a, b)).length_sq_raw() <= (r.raw().max(0) as u64).pow(2)
}

/// 2D cross product of raw vectors (i128, no overflow).
fn det128(a: FxVec2, b: FxVec2) -> i128 {
    a.x.raw() as i128 * b.y.raw() as i128 - a.y.raw() as i128 * b.x.raw() as i128
}

/// Where segment `o -> o + d` crosses segment `a -> b`: the fraction of
/// `d` (Fx in 0..=1), or `None`.
pub(crate) fn crossing(o: FxVec2, d: FxVec2, a: FxVec2, b: FxVec2) -> Option<Fx> {
    let e = b - a;
    let den = det128(d, e);
    if den == 0 {
        return None;
    }
    let w = a - o;
    let (s, u) = (det128(w, e), det128(w, d));
    let inside = |n: i128| {
        if den > 0 {
            n >= 0 && n <= den
        } else {
            n <= 0 && n >= den
        }
    };
    if !inside(s) || !inside(u) {
        return None;
    }
    Some(Fx::from_raw(((s << Fx::FRAC_BITS) / den) as i32))
}

/// Whether segment `a -> b` meets the rectangle of half extents `half`
/// centred at `c` with axes `f` (forward, unit) and `f.perp()`.
fn segment_in_rect(a: FxVec2, b: FxVec2, c: FxVec2, f: FxVec2, half: FxVec2) -> bool {
    let local = |p: FxVec2| {
        let q = p - c;
        FxVec2::new(q.dot(f), q.dot(f.perp()))
    };
    let (la, lb) = (local(a), local(b));
    // Slab test on the two axes (Liang-Barsky), in raw i128.
    let (mut t0, mut t1) = (0i128, 1i128 << 32);
    let d = lb - la;
    for (p0, dp, h) in [(la.x, d.x, half.x), (la.y, d.y, half.y)] {
        let (p0, dp, h) = (p0.raw() as i128, dp.raw() as i128, h.raw() as i128);
        if dp == 0 {
            if p0 < -h || p0 > h {
                return false;
            }
            continue;
        }
        let mut ta = ((-h - p0) << 32) / dp;
        let mut tb = ((h - p0) << 32) / dp;
        if ta > tb {
            std::mem::swap(&mut ta, &mut tb);
        }
        t0 = t0.max(ta);
        t1 = t1.min(tb);
        if t0 > t1 {
            return false;
        }
    }
    true
}

/// Half the band width of section `s` (0 for a blueprint without wire).
fn half_band(forts: &[frontline_content::setup::FortSetup], s: &WireSection) -> Fx {
    forts
        .get(s.fort as usize)
        .and_then(|d| d.wire.as_ref())
        .map_or(Fx::ZERO, |w| Fx::from(w.band).div_int(2))
}

/// How a wire section was removed (the account counter it adds to).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum WireGone {
    Cut,
    Blown,
    Crushed,
    /// HE or fragments other than a Bangalore's: no counter.
    Broken,
}

impl FortState {
    /// Section 5 rule 4: whether `pos` lies inside a standing wire band
    /// (false at once without wire).
    pub fn in_wire(&self, forts: &[frontline_content::setup::FortSetup], pos: FxVec2) -> bool {
        if self.wire.is_empty() {
            return false;
        }
        self.wire.values().any(|s| {
            let half = forts
                .get(s.fort as usize)
                .and_then(|d| d.wire.as_ref())
                .map_or(Fx::ZERO, |w| Fx::from(w.band).div_int(2));
            near_segment(pos, s.a, s.b, half)
        })
    }

    /// S10a-ai: whether `pos` lies inside the band of a standing section
    /// not owned by `player` (false at once without wire).
    pub fn in_enemy_wire(
        &self,
        forts: &[frontline_content::setup::FortSetup],
        pos: FxVec2,
        player: PlayerId,
    ) -> bool {
        if self.wire.is_empty() {
            return false;
        }
        self.wire
            .values()
            .any(|s| s.owner != player && near_segment(pos, s.a, s.b, half_band(forts, s)))
    }

    /// The infantry path layer (rule 4): the sorted path nodes (`dim` per
    /// side, 4 m apart) within half a band plus half a node step of a
    /// standing section, so every section marks the nodes it lies among.
    /// Empty without wire.
    pub(crate) fn wire_nodes(
        &self,
        forts: &[frontline_content::setup::FortSetup],
        dim: u32,
    ) -> Vec<u32> {
        let mut out = Vec::new();
        for s in self.wire.values() {
            let band = forts
                .get(s.fort as usize)
                .and_then(|d| d.wire.as_ref())
                .map_or(Fx::ZERO, |w| Fx::from(w.band).div_int(2));
            let r = band + Fx::from_int(NODE_M / 2);
            let lo = |a: Fx, b: Fx| ((a.min(b) - r).floor_int() / NODE_M).max(0) as u32;
            let hi = |a: Fx, b: Fx| {
                (((a.max(b) + r).floor_int() / NODE_M) + 1).clamp(0, dim as i32 - 1) as u32
            };
            for gy in lo(s.a.y, s.b.y)..=hi(s.a.y, s.b.y) {
                for gx in lo(s.a.x, s.b.x)..=hi(s.a.x, s.b.x) {
                    let p = FxVec2::from_int(gx as i32 * NODE_M, gy as i32 * NODE_M);
                    if gx < dim && gy < dim && near_segment(p, s.a, s.b, r) {
                        out.push(gy * dim + gx);
                    }
                }
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Removes wire section `id`, counting it for `by` as `how`.
    pub(crate) fn remove_wire(&mut self, id: u32, by: Option<PlayerId>, how: WireGone) {
        if self.wire.remove(&id).is_none() {
            return;
        }
        let Some(acc) = by.and_then(|p| self.accounts.get_mut(p.0 as usize)) else {
            return;
        };
        match how {
            WireGone::Cut => acc.wire_cut += 1,
            WireGone::Blown => acc.wire_blown += 1,
            WireGone::Crushed => acc.wire_crushed += 1,
            WireGone::Broken => {}
        }
    }

    /// Takes `dmg` hp from wire section `id`; at 0 it is removed (true).
    pub(crate) fn hurt_wire(&mut self, id: u32, dmg: u32, by: PlayerId, how: WireGone) -> bool {
        let Some(s) = self.wire.get_mut(&id) else {
            return false;
        };
        s.hp = s.hp.saturating_sub(dmg);
        if s.hp == 0 {
            self.remove_wire(id, Some(by), how);
            return true;
        }
        false
    }
}

/// Section 5 rule 4: a man inside a standing wire band moves at
/// `wire_speed_pct` of `v` (unchanged without wire).
pub(crate) fn wire_pace(fort: &FortState, setup: &MatchSetup, pos: FxVec2, v: Fx) -> Fx {
    if fort.wire.is_empty() {
        return v;
    }
    let Some(t) = setup.rules.fortify.as_ref() else {
        return v;
    };
    if fort.in_wire(&setup.forts, pos) {
        v.mul_int(t.wire_speed_pct as i32).div_int(100)
    } else {
        v
    }
}

impl MatchState {
    /// Section `id` is gone: squads cutting it stop (`Idle`).
    pub(crate) fn wire_gone(&mut self, id: u32) {
        for q in self.squads.values_mut() {
            if matches!(q.order, Order::CutWire { section, .. } if section == id) {
                q.order = Order::Idle;
            }
        }
    }

    /// `CutWire { squad, at }` (section 4, rule 5).
    pub(crate) fn order_cut_wire(
        &mut self,
        ctx: &mut Ctx,
        p: PlayerId,
        squad: SquadId,
        at: FxVec2,
    ) -> bool {
        let Some(t) = self.setup.rules.fortify.clone() else {
            return false;
        };
        if !self.owned(p, squad) || self.is_vehicle_squad(squad) {
            return self.refuse_fort(p, Some(squad), FortRefusal::NotAllowed);
        }
        let ty = &self.setup.squad_types[self.squads[&squad].type_idx as usize];
        if !ty.has(Capability::ClearWire) {
            return self.refuse_fort(p, Some(squad), FortRefusal::CannotBuild);
        }
        let reach = Fx::from_int(t.cut_reach_m as i32);
        let r2 = (reach.raw() as u64).pow(2);
        let best = self
            .fort
            .wire
            .iter()
            .map(|(&id, s)| ((at - closest_on(at, s.a, s.b)).length_sq_raw(), id))
            .filter(|&(d, _)| d <= r2)
            .min();
        let Some((_, section)) = best else {
            return self.refuse_fort(p, Some(squad), FortRefusal::NoWire);
        };
        let s = &self.fort.wire[&section];
        let lead = self.squads[&squad].lead(ctx.ents);
        let spot = cut_spot(s, ctx.ents.transform[lead.index()].pos.xy());
        self.clear_detached(ctx, squad);
        self.squads.get_mut(&squad).expect("owned").order = Order::CutWire {
            section,
            man: None,
            work: 0,
        };
        self.request_path(squad, ctx.terrain.clamp(spot));
        true
    }

    /// Rule 5 per tick: cutters work; vehicles crush the sections their
    /// hulls cross (S10a-7 (A)). Nothing to do without wire.
    fn wire_tick(&mut self, ents: &Entities, terrain: &Heightfield, t: &FortTuning) {
        if self.fort.wire.is_empty() {
            return;
        }
        let hz = self.setup.tick_hz as u64;
        let need = t.cut_s as u64 * hz * 100;
        let ids: Vec<SquadId> = self
            .squads
            .iter()
            .filter(|(_, q)| matches!(q.order, Order::CutWire { .. }))
            .map(|(&id, _)| id)
            .collect();
        for id in ids {
            let sq = &self.squads[&id];
            let Order::CutWire { section, man, work } = sq.order.clone() else {
                continue;
            };
            let Some(s) = self.fort.wire.get(&section).cloned() else {
                self.squads.get_mut(&id).expect("listed").order = Order::Idle;
                continue;
            };
            let mid = s.a + (s.b - s.a).div_int(2);
            // The cutter: kept while he lives in the squad, else the man
            // nearest the section (ties: lowest id).
            let able = |m: SimId| ents.is_alive(m) && !ents.unit[m.index()].detached;
            let man = man
                .filter(|m| sq.members.contains(m) && able(*m))
                .or_else(|| {
                    sq.members
                        .iter()
                        .copied()
                        .filter(|&m| able(m))
                        .min_by_key(|&m| {
                            (
                                (ents.transform[m.index()].pos.xy() - mid).length_sq_raw(),
                                m,
                            )
                        })
                });
            let Some(man) = man else {
                self.squads.get_mut(&id).expect("listed").order = Order::Idle;
                continue;
            };
            let rate =
                Self::tier_of(&self.setup.squad_types[sq.type_idx as usize], t).cut_pct as u64;
            let here = ents.transform[man.index()].pos.xy();
            let sq = self.squads.get_mut(&id).expect("listed");
            if matches!(sq.nav.phase, NavPhase::Arriving | NavPhase::Holding)
                && sq.nav.slots.len() == sq.members.len()
                && let Some(k) = sq.members.iter().position(|&m| m == man)
            {
                sq.nav.slots[k] = terrain.clamp(cut_spot(&s, here));
            }
            let at = near_segment(here, s.a, s.b, CUT_REACH) && !ents.combat[man.index()].pinned;
            let work = if at { (work + rate).min(need) } else { work };
            if work >= need {
                let owner = sq.owner;
                self.fort.remove_wire(section, Some(owner), WireGone::Cut);
                self.wire_gone(section);
                continue;
            }
            sq.order = Order::CutWire {
                section,
                man: Some(man),
                work,
            };
        }
        // Vehicles crush the sections their hulls cross.
        let mut crushed: Vec<(u32, PlayerId)> = Vec::new();
        for (&v, vs) in &self.vehicles {
            if vs.wreck_tick.is_some() || vs.speed == Fx::ZERO {
                continue;
            }
            let Some(geo) = self.vderived.geo.get(vs.type_idx as usize) else {
                continue;
            };
            let tr = &ents.transform[v.index()];
            let f = dir(tr.yaw);
            let c = tr.pos.xy() + f * geo.hull_center.x + f.perp() * geo.hull_center.y;
            let half = FxVec2::new(geo.hull_half.x, geo.hull_half.y);
            for (&sid, s) in &self.fort.wire {
                if segment_in_rect(s.a, s.b, c, f, half) {
                    crushed.push((sid, ents.owner[v.index()]));
                }
            }
        }
        for (sid, by) in crushed {
            if self.fort.wire.contains_key(&sid) {
                self.fort.remove_wire(sid, Some(by), WireGone::Crushed);
                self.wire_gone(sid);
            }
        }
    }

    /// Rule 4: overpressure of a burst of `player` at `pos` within
    /// `blast_r` with a clear line from `lift` takes `damage` from each
    /// enemy wire section (`charge`: a Bangalore's, counted as blown).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn blast_wire(
        &mut self,
        terrain: &Heightfield,
        pos: FxVec3,
        lift: FxVec3,
        blast_r: Fx,
        damage: u32,
        player: u8,
        charge: bool,
    ) {
        if self.fort.wire.is_empty() || blast_r <= Fx::ZERO {
            return;
        }
        let half_up = |s: &WireSection| {
            self.setup.forts[s.fort as usize]
                .wire
                .as_ref()
                .map_or(Fx::ZERO, |w| Fx::from(w.height).div_int(2))
        };
        let hit: Vec<u32> = self
            .fort
            .wire
            .iter()
            .filter(|(_, s)| s.owner.0 != player)
            .filter_map(|(&id, s)| {
                let q = closest_on(pos.xy(), s.a, s.b);
                let at = q.extend(terrain.height_at(q) + half_up(s));
                ((at - pos).length_sq_raw() <= (blast_r.raw() as u64).pow(2)
                    && terrain.line_of_sight(lift, at))
                .then_some(id)
            })
            .collect();
        let how = if charge {
            WireGone::Blown
        } else {
            WireGone::Broken
        };
        for id in hit {
            if self.fort.hurt_wire(id, damage, PlayerId(player), how) {
                self.wire_gone(id);
            }
        }
    }

    /// Rule 4: enemy wire sections a fragment ray `o -> o + d` crosses
    /// below their height before `stop` (fraction of `d`) with the earth
    /// clear: `(section, fraction)` in ascending fraction.
    pub(crate) fn frag_wire(
        &self,
        terrain: &Heightfield,
        o: FxVec3,
        d: FxVec3,
        stop: Fx,
        player: u8,
    ) -> Vec<(u32, Fx)> {
        let mut out = Vec::new();
        for (&id, s) in &self.fort.wire {
            if s.owner.0 == player {
                continue;
            }
            let Some(f) = crossing(o.xy(), d.xy(), s.a, s.b) else {
                continue;
            };
            if f >= stop {
                continue;
            }
            let p = o + FxVec3::new(d.x.mul(f), d.y.mul(f), d.z.mul(f));
            let ground = terrain.height_at(p.xy());
            let h = self.setup.forts[s.fort as usize]
                .wire
                .as_ref()
                .map_or(Fx::ZERO, |w| Fx::from(w.height));
            if p.z < ground - Fx::from_ratio(1, 10) || p.z > ground + h {
                continue;
            }
            if terrain.segment_hit(o, p).is_some() {
                continue;
            }
            out.push((id, f));
        }
        out
    }

    /// `PlaceBangalore { units, target }` (section 4): the nearest man
    /// carrying a Bangalore in a `breach` squad walks (detached) to within
    /// 2 m of `target` and places it there (`Windup` of kind Bangalore,
    /// counted down by `release_throws` once he is there).
    pub(crate) fn place_bangalore(
        &mut self,
        ctx: &mut Ctx,
        p: PlayerId,
        units: &[SimId],
        target: FxVec2,
    ) -> bool {
        use frontline_content::ItemKind;
        if self.setup.rules.fortify.is_none() {
            return false;
        }
        if !ctx.terrain.contains(target) {
            return self.refuse_fort(p, None, FortRefusal::BadSpot);
        }
        let Some(men) = self.unit_set(ctx, p, units) else {
            return self.refuse_fort(p, None, FortRefusal::NotAllowed);
        };
        let k = ItemKind::Bangalore.index();
        let best = men
            .iter()
            .copied()
            .filter(|&m| {
                let i = m.index();
                let kit = &ctx.ents.kit[i];
                kit.items[k] > 0
                    && kit.windup.is_none()
                    && !ctx.ents.combat[i].pinned
                    && ctx.ents.squad[i]
                        .and_then(|s| self.squads.get(&s))
                        .is_some_and(|q| {
                            let ty = &self.setup.squad_types[q.type_idx as usize];
                            ty.has(Capability::Breach)
                                && ty.kit(&self.setup.items, ItemKind::Bangalore).is_some()
                        })
            })
            .min_by_key(|&m| {
                (
                    (target - ctx.ents.transform[m.index()].pos.xy()).length_sq_raw(),
                    m,
                )
            });
        let Some(m) = best else {
            return self.refuse_fort(p, None, FortRefusal::CannotBuild);
        };
        let ticks = self
            .item_of(ctx.ents, m, ItemKind::Bangalore)
            .and_then(|(_, it)| it.bangalore.as_ref())
            .map_or(1, |b| b.place_ticks.clamp(1, u16::MAX as u32) as u16);
        let pos = ctx.ents.transform[m.index()].pos.xy();
        let to = target - pos;
        let facing = if to == FxVec2::ZERO {
            dir(ctx.ents.transform[m.index()].yaw)
        } else {
            to.normalize_or_zero()
        };
        let far = to.length_sq_raw() > (PLACE_REACH.raw() as u64).pow(2);
        if far {
            let spot = ctx.terrain.clamp(target - facing * Fx::from_ratio(3, 2));
            self.detach(
                ctx,
                m,
                crate::unit::UnitOrder::Move { target: spot },
                facing,
            );
            self.request_unit_path(ctx, m, spot);
        } else {
            self.detach(ctx, m, crate::unit::UnitOrder::Idle, facing);
            let nav = &mut ctx.ents.unit[m.index()].nav;
            nav.phase = NavPhase::Holding;
            nav.goal = pos;
        }
        let kit = &mut ctx.ents.kit[m.index()];
        kit.items[k] -= 1;
        kit.windup = Some(crate::items::Windup {
            kind: ItemKind::Bangalore,
            target,
            ticks,
        });
        ctx.hash.mark_entity_chunk(m.chunk());
        true
    }

    /// Whether Bangalore carrier `m` with his windup toward `target` may
    /// go on: `Some(true)` placing there now, `Some(false)` still walking,
    /// `None` called off (`release_throws`).
    pub(crate) fn placing(ents: &Entities, m: SimId, target: FxVec2) -> Option<bool> {
        use crate::unit::UnitOrder;
        let i = m.index();
        let u = &ents.unit[i];
        if ents.kind[i] != crate::ecs::EntityKind::Soldier || ents.combat[i].pinned || !u.detached {
            return None;
        }
        let near = (target - ents.transform[i].pos.xy()).length_sq_raw()
            <= ((PLACE_REACH + PLACE_SLACK).raw() as u64).pow(2);
        match u.order {
            UnitOrder::Idle => near.then_some(true),
            UnitOrder::Move { target: t }
                if (t - target).length_sq_raw() <= (PLACE_REACH.raw() as u64).pow(2) =>
            {
                Some(false)
            }
            _ => None,
        }
    }

    /// A called-off placing (`placing` gave `None`): a carrier still on
    /// his placing walk (or standing where it ended) stops and rejoins his
    /// squad; a man the player gave another order keeps it.
    pub(crate) fn placing_called_off(&mut self, ents: &mut Entities, m: SimId, target: FxVec2) {
        use crate::unit::UnitOrder;
        let i = m.index();
        let u = &ents.unit[i];
        if ents.kind[i] != crate::ecs::EntityKind::Soldier || !u.detached {
            return;
        }
        let walk = match u.order {
            UnitOrder::Idle => true,
            UnitOrder::Move { target: t } => {
                (t - target).length_sq_raw() <= (PLACE_REACH.raw() as u64).pow(2)
            }
            _ => false,
        };
        if !walk {
            return;
        }
        ents.unit[i].rejoin();
        let owner = ents.owner[i].0 as usize;
        if let Some(q) = self.path_queue.queues.get_mut(owner) {
            q.retain(|r| *r != crate::game::PathReq::Unit(m));
        }
    }

    /// Lays the charge of Bangalore item `item` that `m` (of `owner`)
    /// placed toward `target`: from his spot `length` toward it; it bursts
    /// at `tick + fuse`. False (item kept) when the list is full.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn lay_charge(
        &mut self,
        terrain: &Heightfield,
        owner: PlayerId,
        from: FxVec2,
        yaw: Fx,
        item: u16,
        target: FxVec2,
        tick: u64,
    ) -> bool {
        if self.fort.charges.len() >= MAX_CHARGES {
            return false;
        }
        let it = &self.setup.items[item as usize];
        let Some(b) = it.bangalore.as_ref() else {
            return false;
        };
        let mut f = (target - from).normalize_or_zero();
        if f == FxVec2::ZERO {
            f = dir(yaw);
        }
        let to = terrain.clamp(from + f * Fx::from(b.length));
        let id = self.fort.next_charge;
        self.fort.next_charge += 1;
        self.fort.charges.push(Charge {
            id,
            owner,
            item,
            from: terrain.clamp(from),
            to,
            fuse_tick: tick + it.fuse_ticks.max(1) as u64,
        });
        if let Some(a) = self.fort.accounts.get_mut(owner.0 as usize) {
            a.bangalores += 1;
        }
        true
    }

    /// Rule 6: the charges whose fuse runs out at `tick` (ascending id)
    /// become `bursts` landed items each, evenly along the charge, appended
    /// to `out` (the Damage stage bursts them as items; their keys are
    /// `id * 8 + k`, their crater purpose `CHARGE_CRATER`).
    pub(crate) fn take_charges(
        &mut self,
        terrain: &Heightfield,
        tick: u64,
        out: &mut Vec<crate::items::Thrown>,
    ) {
        if self.fort.charges.is_empty() {
            return;
        }
        let mut k = 0;
        while k < self.fort.charges.len() {
            if self.fort.charges[k].fuse_tick > tick {
                k += 1;
                continue;
            }
            let c = self.fort.charges.remove(k);
            let n = self.setup.items[c.item as usize]
                .bangalore
                .as_ref()
                .map_or(1, |b| b.bursts.clamp(1, CHARGE_KEYS as u8)) as i32;
            let v = c.to - c.from;
            for j in 0..n {
                let at = terrain.clamp(c.from + v.mul_int(2 * j + 1).div_int(2 * n));
                let pos = at.extend(terrain.height_at(at));
                out.push(crate::items::Thrown {
                    id: c.id.wrapping_mul(CHARGE_KEYS).wrapping_add(j as u32),
                    item: c.item,
                    owner: c.owner,
                    thrower: SimId(u32::MAX),
                    from: pos,
                    vel: FxVec3::ZERO,
                    step: 0,
                    pos,
                    landed: true,
                    fuse: 0,
                    inside: None,
                });
            }
        }
    }
}

/// Where a cutter stands at section `s`: a metre from its middle, on the
/// side of `from`.
fn cut_spot(s: &WireSection, from: FxVec2) -> FxVec2 {
    let mid = s.a + (s.b - s.a).div_int(2);
    let mut n = (s.b - s.a).normalize_or_zero().perp();
    if (from - mid).dot_raw(n) < 0 {
        n = FxVec2::ZERO - n;
    }
    mid + n * CUT_STAND
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

/// The points of `pts` and, along each segment, one every path-node step
/// (4 m; at least the far end).
fn along(pts: &[FxVec2]) -> impl Iterator<Item = FxVec2> + '_ {
    pts.first()
        .copied()
        .into_iter()
        .chain(pts.windows(2).flat_map(|w| {
            let d = w[1] - w[0];
            let n = (d.length().floor_int() / NODE_M + 1).max(1);
            (1..=n).map(move |k| w[0] + d.mul_int(k).div_int(n))
        }))
}

/// The slope of the undug ground at `p` (%): the steeper of the x and y
/// differences over 2 m of the base terrain (trenches, pits, spoil and
/// craters do not count: a site beside a trench is on fair ground).
pub(crate) fn slope_pct(terrain: &Heightfield, p: FxVec2) -> u32 {
    let one = Fx::ONE;
    let h = |dx: Fx, dy: Fx| terrain.base_height_at(terrain.clamp(FxVec2::new(p.x + dx, p.y + dy)));
    let sx = (h(one, Fx::ZERO) - h(-one, Fx::ZERO)).abs();
    let sy = (h(Fx::ZERO, one) - h(Fx::ZERO, -one)).abs();
    (sx.max(sy).mul_int(50)).floor_int().max(0) as u32
}

fn dir(a: Fx) -> FxVec2 {
    let (s, c) = a.sin_cos();
    FxVec2::new(c, s)
}
