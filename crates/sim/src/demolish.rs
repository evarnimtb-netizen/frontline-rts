//! S10b Build D: demolition charges (docs/slices/S10b-obstacles.md R19,
//! R20, 5 "Demolition").
//!
//! * **`Demolish { squad, at }`** gives a `demolish` squad
//!   `Order::Demolish`: the target is the standing piece (any owner) whose
//!   box lies nearest `at` (2D, ascending id on ties) within
//!   `demolish_reach_m`; the man is the living member nearest it who
//!   carries an item of kind `satchel` (ascending `SimId` on ties). He
//!   walks detached (as for a Bangalore) to 1.5 m from the box.
//! * **Placing**: within 2 m of the box he adds 100 points per tick (x0.3
//!   suppressed, 0 pinned) toward `demolish_place_s x 100 x hz`; then one
//!   satchel leaves his kit and a `Charge { from = to = the box point
//!   nearest him, piece: Some(id) }` bursts `demolish_fuse_s` later
//!   (`demolitions`); the order ends (squad idle, man back in formation).
//! * **Ends** (spec 5): charge laid; piece gone (`NoTarget` event); man
//!   dead, out of the squad or without a satchel (nothing laid); the man
//!   given another order (he keeps it); the squad given another order.
//! * **Burst (R20)**: the charge's piece takes `damage x demolish_pct /
//!   100` instead of the S6 overpressure (whatever its owner, no line
//!   check: it is tamped against it); everything else the normal rules
//!   (`combat::burst`). A piece it removes counts `pieces_demolished` for
//!   the charge's owner.
//!
//! `demolish_tick` returns at once without a `Demolish` order, so AI-vs-AI
//! play (which never gives one) is unchanged.

use frontline_content::ItemKind;
use frontline_content::setup::Capability;

use crate::ecs::{PlayerId, SimId};
use crate::fort::{Charge, FortRefusal, MAX_CHARGES};
use crate::fx::{Fx, FxVec2};
use crate::game::{Ctx, MatchState, Order, PathReq, SquadId};
use crate::unit::UnitOrder;

/// The man's stand-off from the box he places against.
const STAND: Fx = Fx::from_raw(Fx::ONE.raw() * 3 / 2);
/// He places while this close to the box.
const PLACE_REACH: Fx = Fx::from_int(2);

impl MatchState {
    /// The point of piece `id`'s box (2D) nearest `p`, if it stands.
    pub(crate) fn piece_point(&self, id: u32, p: FxVec2) -> Option<FxVec2> {
        let q = self.fort.pieces.get(&id)?;
        let b = self
            .setup
            .forts
            .get(q.fort as usize)?
            .pieces
            .get(q.part as usize)?;
        let f = crate::vehicle::yaw_dir(q.yaw);
        let l = f.perp();
        let d = p - q.pos;
        let hx: Fx = b.half[0].into();
        let hy: Fx = b.half[1].into();
        let x = d.dot(f).clamp(-hx, hx);
        let y = d.dot(l).clamp(-hy, hy);
        Some(q.pos + f * x + l * y)
    }

    /// Where a man at `from` stands to place against piece `id`: 1.5 m out
    /// from the box point nearest him.
    fn demolish_spot(&self, id: u32, from: FxVec2) -> Option<FxVec2> {
        let pt = self.piece_point(id, from)?;
        let mut out = (from - pt).normalize_or_zero();
        if out == FxVec2::ZERO {
            let q = &self.fort.pieces[&id];
            out = (from - q.pos).normalize_or_zero();
            if out == FxVec2::ZERO {
                out = crate::vehicle::yaw_dir(q.yaw);
            }
        }
        Some(pt + out * STAND)
    }

    /// `Demolish { squad, at }` (S10b-build-D 2).
    pub(crate) fn order_demolish(
        &mut self,
        ctx: &mut Ctx,
        p: PlayerId,
        squad: SquadId,
        at: FxVec2,
    ) -> bool {
        let Some(t) = self.setup.rules.fortify.clone() else {
            return false;
        };
        if self.fort_off || !self.owned(p, squad) || self.is_vehicle_squad(squad) {
            return self.refuse_fort(p, Some(squad), FortRefusal::NotAllowed);
        }
        let ty = &self.setup.squad_types[self.squads[&squad].type_idx as usize];
        if !ty.has(Capability::Demolish) {
            return self.refuse_fort(p, Some(squad), FortRefusal::CannotBuild);
        }
        let reach = Fx::from_int(t.demolish_reach_m as i32);
        let r2 = (reach.raw() as u64).pow(2);
        let best = self
            .fort
            .pieces
            .keys()
            .filter_map(|&id| {
                let pt = self.piece_point(id, at)?;
                Some(((at - pt).length_sq_raw(), id))
            })
            .filter(|&(d, _)| d <= r2)
            .min();
        let Some((_, piece)) = best else {
            return self.refuse_fort(p, Some(squad), FortRefusal::NoTarget);
        };
        let k = ItemKind::Satchel.index();
        let centre = self.fort.pieces[&piece].pos;
        let man = self.squads[&squad]
            .members
            .iter()
            .copied()
            .filter(|&m| ctx.ents.is_alive(m) && ctx.ents.kit[m.index()].items[k] > 0)
            .min_by_key(|&m| {
                (
                    (ctx.ents.transform[m.index()].pos.xy() - centre).length_sq_raw(),
                    m,
                )
            });
        let Some(man) = man else {
            return self.refuse_fort(p, Some(squad), FortRefusal::NotAllowed);
        };
        let here = ctx.ents.transform[man.index()].pos.xy();
        let spot = ctx
            .terrain
            .clamp(self.demolish_spot(piece, here).expect("standing"));
        self.clear_detached(ctx, squad);
        let facing = (centre - spot).normalize_or_zero();
        self.detach(ctx, man, UnitOrder::Move { target: spot }, facing);
        self.request_unit_path(ctx, man, spot);
        self.squads.get_mut(&squad).expect("owned").order = Order::Demolish {
            piece,
            man: Some(man),
            work: 0,
        };
        true
    }

    /// Ends squad `id`'s `Demolish` (idle); its man, still on his placing
    /// walk or standing there, rejoins the formation.
    fn end_demolish(&mut self, ctx: &mut Ctx, id: SquadId, man: Option<SimId>) {
        if let Some(q) = self.squads.get_mut(&id) {
            q.order = Order::Idle;
        }
        let Some(m) = man else { return };
        let i = m.index();
        let u = &ctx.ents.unit[i];
        if !u.detached || !matches!(u.order, UnitOrder::Idle | UnitOrder::Move { .. }) {
            return;
        }
        ctx.ents.unit[i].rejoin();
        ctx.hash.mark_entity_chunk(m.chunk());
        let owner = ctx.ents.owner[i].0 as usize;
        if let Some(q) = self.path_queue.queues.get_mut(owner) {
            q.retain(|r| *r != PathReq::Unit(m));
        }
    }

    /// Spec 5 "Demolition" per tick (status stage, after `fort_tick`).
    /// Returns at once without a `Demolish` order.
    pub(crate) fn demolish_tick(&mut self, ctx: &mut Ctx) {
        if self.fort_off
            || !self
                .squads
                .values()
                .any(|q| matches!(q.order, Order::Demolish { .. }))
        {
            return;
        }
        let Some(t) = self.setup.rules.fortify.clone() else {
            return;
        };
        let hz = self.setup.tick_hz as u64;
        let need = t.demolish_place_s as u64 * hz * 100;
        let crouch = self.setup.rules.combat.crouch_suppression;
        let k = ItemKind::Satchel.index();
        let ids: Vec<SquadId> = self
            .squads
            .iter()
            .filter(|(_, q)| matches!(q.order, Order::Demolish { .. }))
            .map(|(&id, _)| id)
            .collect();
        for id in ids {
            let sq = &self.squads[&id];
            let owner = sq.owner;
            let Order::Demolish { piece, man, work } = sq.order.clone() else {
                continue;
            };
            if !self.fort.pieces.contains_key(&piece) {
                self.refuse_fort(owner, Some(id), FortRefusal::NoTarget);
                self.end_demolish(ctx, id, man);
                continue;
            }
            let Some(m) = man.filter(|&m| {
                sq.members.contains(&m)
                    && ctx.ents.is_alive(m)
                    && ctx.ents.kit[m.index()].items[k] > 0
            }) else {
                self.end_demolish(ctx, id, man);
                continue;
            };
            let i = m.index();
            let u = &ctx.ents.unit[i];
            // Another order for the man (he keeps it): the demolition ends.
            if !u.detached || !matches!(u.order, UnitOrder::Idle | UnitOrder::Move { .. }) {
                self.squads.get_mut(&id).expect("listed").order = Order::Idle;
                continue;
            }
            let here = ctx.ents.transform[i].pos.xy();
            let pt = self.piece_point(piece, here).expect("standing");
            let near = (here - pt).length_sq_raw() <= (PLACE_REACH.raw() as u64).pow(2);
            if !near {
                // Arrived short (a partial path): walk on to the spot.
                if u.order == UnitOrder::Idle {
                    let spot = ctx
                        .terrain
                        .clamp(self.demolish_spot(piece, here).expect("standing"));
                    ctx.ents.unit[i].order = UnitOrder::Move { target: spot };
                    self.request_unit_path(ctx, m, spot);
                }
                continue;
            }
            let c = &ctx.ents.combat[i];
            let add = if c.pinned {
                0
            } else if c.suppression / 1000 >= crouch {
                30
            } else {
                100
            };
            let work = (work + add).min(need);
            if work < need {
                self.squads.get_mut(&id).expect("listed").order = Order::Demolish {
                    piece,
                    man: Some(m),
                    work,
                };
                continue;
            }
            // Placed: the satchel leaves his kit and its charge is laid.
            let item = self.item_of(ctx.ents, m, ItemKind::Satchel).map(|(x, _)| x);
            if let Some(item) = item
                && self.fort.charges.len() < MAX_CHARGES
            {
                ctx.ents.kit[i].items[k] -= 1;
                ctx.hash.mark_entity_chunk(m.chunk());
                let at = ctx.terrain.clamp(pt);
                let cid = self.fort.next_charge;
                self.fort.next_charge += 1;
                self.fort.charges.push(Charge {
                    id: cid,
                    owner,
                    item,
                    from: at,
                    to: at,
                    fuse_tick: ctx.tick + t.demolish_fuse_s as u64 * hz,
                    piece: Some(piece),
                });
                if let Some(a) = self.fort.accounts.get_mut(owner.0 as usize) {
                    a.demolitions = a.demolitions.saturating_add(1);
                }
            }
            self.end_demolish(ctx, id, Some(m));
        }
    }

    /// R20: the tamped damage a demolition charge of `player` does to
    /// piece `piece` (its static box `idx` this tick); a piece it removes
    /// counts `pieces_demolished`.
    pub(crate) fn demolish_piece(
        &mut self,
        idx: usize,
        piece: u32,
        damage: u32,
        he: (u8, u8),
        tick: u64,
    ) {
        let pct = self
            .setup
            .rules
            .fortify
            .as_ref()
            .map_or(100, |t| t.demolish_pct);
        let dmg = (damage as u64 * pct as u64 / 100).min(u32::MAX as u64) as u32;
        if !self.fort.pieces.contains_key(&piece) {
            return;
        }
        self.hit_static(idx, dmg, Some(he), tick);
        if !self.fort.pieces.contains_key(&piece)
            && let Some(a) = self.fort.accounts.get_mut(he.0 as usize)
        {
            a.pieces_demolished = a.pieces_demolished.saturating_add(1);
        }
    }
}
