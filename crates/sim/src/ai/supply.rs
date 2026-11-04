//! S6 AI: the HQ, crate runs and supply to tanks (docs/slices/
//! S6-hq-crates.md "AI"; `[ai.supply]`). Called in a plan after the refill
//! trips; it takes the HQ squad (tag `hq`) and the ammunition bearers (tag
//! `bearers`) out of the infantry list, and dry mortar squads it sends to
//! the HQ.
//!
//! * **Buy.** After `hq_after_s` the AI buys its HQ squad (one, the card's
//!   `max_alive`) when it can also pay the HQ, then one bearer squad once
//!   it has an HQ or a mortar. Neither is in `[ai.mix]`.
//! * **HQ siting.** Candidates `hq_behind_m` behind the most valuable own
//!   point (else the point nearest the own base), toward the base: the spot
//!   and `hq_probe_rays - 1` around it 20 m out. Each is scored by how many
//!   of the known enemy men (at most 8, ascending id) and enemy or neutral
//!   points see a man standing there (terrain rays, 1.5 m above both
//!   ends): the fewest wins (a reverse slope), then the nearest to the
//!   spot, then the lower index.
//! * **Crate runs.** Bearers in the own base take crates (one a plan; the
//!   base issues one per `base_crate_issue_s`) while the crate budget
//!   (`shell_mp_max_pct` of the income so far) and the caps allow; once
//!   every man carries one (or no more can be issued) they set them down
//!   at the first of: an own tank below `tank_crate_below_pct` of its
//!   rounds that cannot drive (owner request 2026-11-01), a mortar squad
//!   below `crate_run_below_pct` of its bombs outside every HQ radius, the
//!   HQ when its stock of that kind is below `crate_run_below_pct`, else
//!   the HQ, else the nearest mortar. Bearers without crates outside the
//!   base walk back to it.
//! * **Dry mortars.** A mortar squad with no bombs walks to an own HQ with
//!   shells stock and stays in its radius until `refill_full_pct` of its
//!   load (the mortar planner does not move it meanwhile).

use frontline_content::setup::{StockKind, SupplyKind};

use super::{Known, Mine, MyTank, Planner, ring_slot, sq_m};
use crate::command::CommandKind;
use crate::fx::{Fx, FxVec2, FxVec3};
use crate::game::{Order, SquadId};

/// Most enemy men a candidate HQ spot is tested against.
const PROBE_MEN: usize = 8;
/// Candidate spots lie this far around the first (m).
const RING_M: i32 = 20;

impl Planner<'_> {
    /// S6 (module docs). `mine` loses the HQ and bearer squads and the
    /// mortars walking to the HQ.
    pub(super) fn supply(&mut self, mine: &mut Vec<Mine>, tanks: &[MyTank], known: &[Known]) {
        let g = self.g;
        let s = &g.setup;
        let Some(sp) = s.rules.supply.clone() else {
            return;
        };
        if s.stocks.is_empty() {
            return;
        }
        let me = self.me;
        let faction = s.players[me.0 as usize].faction;
        let tag = |name: &str| s.tag(name);
        let (hq_tag, bearer_tag) = (tag("hq"), tag("bearers"));
        let is = |id: SquadId, t: Option<u16>| {
            t.is_some() && s.squad_types[g.squads[&id].type_idx as usize].has_tag(t)
        };
        let hq_squads: Vec<SquadId> = g
            .squads
            .iter()
            .filter(|(id, sq)| sq.owner == me && is(**id, hq_tag))
            .map(|(id, _)| *id)
            .collect();
        let bearers: Vec<SquadId> = g
            .squads
            .iter()
            .filter(|(id, sq)| sq.owner == me && is(**id, bearer_tag))
            .map(|(id, _)| *id)
            .collect();
        mine.retain(|m| !hq_squads.contains(&m.id) && !bearers.contains(&m.id));
        let hq_def = s.stock_of(faction, StockKind::Hq, None);
        let my_hq = g
            .supply6
            .structures
            .iter()
            .find(|(_, x)| x.owner == me && s.stocks[x.def as usize].kind == StockKind::Hq)
            .map(|(id, x)| (*id, x.clone()));
        self.dry_mortars(mine, my_hq.as_ref().map(|h| &h.1));
        self.buy_supply(&hq_squads, &bearers, my_hq.is_some(), hq_def, mine);
        // HQ squad: build, then stay by the HQ.
        if let (Some(&zt), Some(hd)) = (hq_squads.first(), hq_def) {
            match &my_hq {
                None => self.site_hq(zt, hd, known),
                Some((_, h)) if h.built => {
                    let lead = self.lead_of(zt);
                    let idle = matches!(g.squads[&zt].order, Order::Idle);
                    if idle && (lead - h.pos).length_sq_raw() > sq_m(Fx::from_int(15)) {
                        let to = self.terrain.clamp(ring_slot(h.pos, 1));
                        self.ai.emit(
                            self.due,
                            CommandKind::Move {
                                squad: zt,
                                target: to,
                            },
                        );
                    }
                }
                Some(_) => {}
            }
        }
        for b in bearers {
            self.run_crates(b, my_hq.as_ref().map(|h| &h.1), tanks, &sp);
        }
    }

    fn lead_of(&self, id: SquadId) -> FxVec2 {
        let sq = &self.g.squads[&id];
        self.ents.transform[sq.lead(self.ents).index()].pos.xy()
    }

    /// The HQ squad and the bearers (module docs, "Buy").
    fn buy_supply(
        &mut self,
        hq_squads: &[SquadId],
        bearers: &[SquadId],
        has_hq: bool,
        hq_def: Option<u16>,
        mine: &[Mine],
    ) {
        let g = self.g;
        let s = &g.setup;
        let me = self.me;
        let hz = s.tick_hz.max(1) as u64;
        if self.tick < s.rules.ai.supply.hq_after_s as u64 * hz {
            return;
        }
        let faction = &s.factions[s.players[me.0 as usize].faction as usize];
        let pending = |t: u16| {
            self.ai
                .pending
                .iter()
                .any(|c| c.kind == CommandKind::CallIn { squad_type: t })
        };
        let card = |name: &str| {
            faction
                .roster
                .iter()
                .copied()
                .find(|&t| s.type_has_tag(t, name))
        };
        let mp = g.players[me.0 as usize].manpower;
        let hq_cost = hq_def.map_or(0, |d| s.stocks[d as usize].cost);
        if hq_squads.is_empty()
            && let Some(t) = card("hq")
            && !pending(t)
            && g.call_in_check(self.ents, me, t, self.tick).is_ok()
            && mp >= s.squad_types[t as usize].cost + hq_cost
        {
            self.ai
                .emit(self.due, CommandKind::CallIn { squad_type: t });
            self.ai.counters.s6_buys += 1;
            return;
        }
        let has_mortar = mine.iter().any(|m| m.role == super::AiKind::Mortar);
        if bearers.is_empty()
            && (has_hq || has_mortar)
            && let Some(t) = card("bearers")
            && !pending(t)
            && g.call_in_check(self.ents, me, t, self.tick).is_ok()
        {
            self.ai
                .emit(self.due, CommandKind::CallIn { squad_type: t });
            self.ai.counters.s6_buys += 1;
        }
    }

    /// HQ siting and the Build order (module docs).
    fn site_hq(&mut self, zt: SquadId, hd: u16, known: &[Known]) {
        let g = self.g;
        let s = &g.setup;
        let me = self.me;
        // Already ordered (pending, or digging).
        if self
            .ai
            .pending
            .iter()
            .any(|c| matches!(c.kind, CommandKind::Build { .. }))
            || matches!(g.squads[&zt].order, Order::Dig { .. })
        {
            return;
        }
        let acc = g.supply6.accounts[me.0 as usize];
        let rebuild = s.stocks[hd as usize].rebuild_ticks as u64;
        if acc.hq_lost_at > 0 && self.tick < acc.hq_lost_at + rebuild {
            return;
        }
        if g.players[me.0 as usize].manpower < s.stocks[hd as usize].cost {
            return;
        }
        let base = g.base_of(me);
        // The most valuable own point (ties: nearer the base, lower index);
        // none held: the point nearest the base.
        let pts = &s.map.points;
        let pick = (0..pts.len())
            .filter(|&k| g.points[k].owner == Some(me))
            .max_by_key(|&k| {
                let at: FxVec2 = pts[k].pos.into();
                (
                    pts[k].value,
                    std::cmp::Reverse((at - base).length_sq_raw()),
                    std::cmp::Reverse(k),
                )
            })
            .or_else(|| {
                (0..pts.len()).min_by_key(|&k| {
                    let at: FxVec2 = pts[k].pos.into();
                    ((at - base).length_sq_raw(), k)
                })
            });
        let Some(k) = pick else { return };
        let point: FxVec2 = pts[k].pos.into();
        let back = (base - point).normalize_or_zero();
        let cfg = &s.rules.ai.supply;
        let behind = Fx::from_int(cfg.hq_behind_m as i32).min((base - point).length());
        let first = self.terrain.clamp(point + back * behind);
        // Viewers: known enemy men (ascending id, at most PROBE_MEN) and the
        // points not held by this player.
        let mut eyes: Vec<FxVec2> = known.iter().take(PROBE_MEN).map(|k| k.pos).collect();
        for (j, p) in pts.iter().enumerate() {
            if g.points[j].owner != Some(me) {
                eyes.push(p.pos.into());
            }
        }
        let lift = Fx::from_ratio(3, 2);
        let n = cfg.hq_probe_rays.max(1) as i32;
        let mut best: Option<(u32, u64, i32, FxVec2)> = None;
        for c in 0..n {
            let at = if c == 0 {
                first
            } else {
                let (sn, cs) = Fx::TAU.mul_int(c - 1).div_int(n - 1).sin_cos();
                self.terrain
                    .clamp(first + FxVec2::new(cs, sn) * Fx::from_int(RING_M))
            };
            if crate::supply::in_own_base(s, me, at) && c > 0 {
                continue;
            }
            let h = self.terrain.height_at(at) + lift;
            let seen = eyes
                .iter()
                .filter(|e| {
                    let eh = self.terrain.height_at(**e) + lift;
                    self.terrain
                        .line_of_sight(FxVec3::new(e.x, e.y, eh), FxVec3::new(at.x, at.y, h))
                })
                .count() as u32;
            let key = (seen, (at - first).length_sq_raw(), c, at);
            if best.is_none_or(|b| (key.0, key.1, key.2) < (b.0, b.1, b.2)) {
                best = Some(key);
            }
        }
        let Some((seen, _, _, at)) = best else { return };
        self.ai.counters.s6_hq_rays += (n as usize * eyes.len()) as u32;
        self.ai.counters.s6_hq_seen += seen;
        // The long axis across the front.
        let facing = back.perp();
        self.ai.emit(
            self.due,
            CommandKind::Build {
                squad: zt,
                def: hd,
                pos: at,
                facing,
            },
        );
        self.ai.counters.s6_hq_orders += 1;
    }

    /// Dry mortars walk to an HQ with shells (module docs).
    fn dry_mortars(&mut self, mine: &mut Vec<Mine>, hq: Option<&crate::structures::Structure>) {
        let g = self.g;
        let s = &g.setup;
        let Some(h) = hq.filter(|h| h.built && h.stock_g[SupplyKind::Shells.index()] >= 3_500)
        else {
            return;
        };
        let r = s.stocks[h.def as usize].radius;
        let full = s.rules.ai.ammo.refill_full_pct;
        let mut keep = Vec::with_capacity(mine.len());
        for m in mine.drain(..) {
            if m.role != super::AiKind::Mortar {
                keep.push(m);
                continue;
            }
            let sq = &g.squads[&m.id];
            let (_, pct) = crate::supply::mortar_bombs(s, self.ents, sq);
            let Some(pct) = pct else {
                keep.push(m);
                continue;
            };
            let inside = (m.lead - h.pos).length_sq_raw() <= sq_m(r.into());
            if pct == 0 || (inside && pct < full) {
                if !inside && !matches!(sq.order, Order::Move { .. }) {
                    let to = self.terrain.clamp(ring_slot(h.pos, m.id.0 % 6 + 2));
                    self.ai.emit(
                        self.due,
                        CommandKind::Move {
                            squad: m.id,
                            target: to,
                        },
                    );
                    self.ai.counters.s6_dry_walks += 1;
                }
                continue;
            }
            keep.push(m);
        }
        *mine = keep;
    }

    /// One bearer squad's crate run (module docs).
    fn run_crates(
        &mut self,
        b: SquadId,
        hq: Option<&crate::structures::Structure>,
        tanks: &[MyTank],
        sp: &frontline_content::tuning::SupplyTuning,
    ) {
        let g = self.g;
        let s = &g.setup;
        let me = self.me;
        let sq = &g.squads[&b];
        if self.ai.pending.iter().any(|c| match &c.kind {
            CommandKind::IssueCrate { squad, .. }
            | CommandKind::PlaceCrate { squad, .. }
            | CommandKind::Move { squad, .. } => *squad == b,
            _ => false,
        }) {
            return;
        }
        let carried: Vec<&crate::structures::Crate> = g
            .supply6
            .crates
            .values()
            .filter(|c| c.carrier.is_some_and(|m| sq.members.contains(&m)))
            .collect();
        let placing = carried.iter().any(|c| c.dest.is_some());
        let in_base = self.in_own_base(sq);
        let cfg = &s.rules.ai.supply;
        if placing {
            // On the way: keep walking (a Move ended short is renewed).
            if let Some(d) = carried.iter().find_map(|c| c.dest)
                && matches!(sq.order, Order::Idle)
            {
                self.ai
                    .emit(self.due, CommandKind::PlaceCrate { squad: b, pos: d });
            }
            return;
        }
        // Budget: shell_mp_max_pct of the income so far (start manpower
        // and the base income; points not counted).
        let hz = s.tick_hz.max(1) as u64;
        let minutes = self.tick / (60 * hz);
        let income = s.rules.start_manpower as u64 + s.rules.income_per_min as u64 * minutes;
        let budget = income * cfg.shell_mp_max_pct as u64 / 100 * crate::supply::MICRO;
        let spent = g.supply6.accounts[me.0 as usize].crate_micro;
        let (live, shells) = g.supply6.crates_of(s, me);
        // Only what the sim will accept (no refused commands): the base
        // ready when the command applies, a free man in the base, the
        // caps and the price (a shells crate is the dearest).
        let free_man = sq.members.iter().any(|&m| {
            !g.supply6.carries(m)
                && crate::supply::in_own_base(s, me, self.ents.transform[m.index()].pos.xy())
        });
        let price_mp = s
            .stock_of(
                s.players[me.0 as usize].faction,
                StockKind::Crate,
                Some(SupplyKind::Shells),
            )
            .map_or(u64::MAX, |d| {
                crate::supply::micro_price(s, SupplyKind::Shells, s.stocks[d as usize].g)
                    .div_ceil(crate::supply::MICRO)
            });
        let can_issue = spent < budget
            && live < sp.crate_cap
            && free_man
            && g.supply6.accounts[me.0 as usize].issue_at <= self.due
            && (g.players[me.0 as usize].manpower as u64) >= price_mp + 20
            && carried.len() < sq.members.len();
        if in_base && can_issue {
            // Shells first; small arms when the HQ's are lower.
            let kind = match hq {
                Some(h)
                    if h.built
                        && pct_of(h, SupplyKind::SmallArms, s)
                            < pct_of(h, SupplyKind::Shells, s) =>
                {
                    SupplyKind::SmallArms
                }
                _ if shells >= sp.shell_crate_cap => SupplyKind::SmallArms,
                _ => SupplyKind::Shells,
            };
            let faction = s.players[me.0 as usize].faction;
            if s.stock_of(faction, StockKind::Crate, Some(kind)).is_none() {
                return;
            }
            self.ai
                .emit(self.due, CommandKind::IssueCrate { squad: b, kind });
            self.ai.counters.s6_crates_asked += 1;
            return;
        }
        if carried.is_empty() {
            if !in_base && !matches!(sq.order, Order::Move { .. }) {
                let to = self.terrain.clamp(ring_slot(g.base_of(me), b.0 % 8 + 1));
                self.ai.emit(
                    self.due,
                    CommandKind::Move {
                        squad: b,
                        target: to,
                    },
                );
            }
            return;
        }
        // Where to: a stranded tank, a low mortar, a low HQ, the HQ, the
        // nearest mortar.
        let shells_crate = carried
            .iter()
            .any(|c| s.stocks[c.def as usize].supply == Some(SupplyKind::Shells));
        let lead = self.lead_of(b);
        let mut dest: Option<(FxVec2, bool)> = None;
        if shells_crate {
            for t in tanks {
                let vs = &g.vehicles[&t.id];
                let v = &s.vehicles[vs.type_idx as usize];
                let (have, load) =
                    v.main_gun()
                        .ammo
                        .iter()
                        .enumerate()
                        .fold((0u32, 0u32), |(h, l), (k, a)| {
                            let n = vs.ammo.get(k).copied().unwrap_or(0) as u32
                                + u32::from(vs.loaded == Some(k as u8));
                            (h + n, l + a.count as u32)
                        });
                if t.immobile && have * 100 < load * cfg.tank_crate_below_pct {
                    dest = Some((t.pos + FxVec2::from_int(3, 0), true));
                    break;
                }
            }
        }
        if dest.is_none() && shells_crate {
            let hq_r = hq.map(|h| (h.pos, s.stocks[h.def as usize].radius));
            dest = g
                .squads
                .iter()
                .filter(|(_, q)| {
                    q.owner == me
                        && !q.members.is_empty()
                        && super::ai_kind(s, q.type_idx) == super::AiKind::Mortar
                })
                .filter_map(|(_, q)| {
                    let (_, pct) = crate::supply::mortar_bombs(s, self.ents, q);
                    let pct = pct?;
                    let at = self.ents.transform[q.lead(self.ents).index()].pos.xy();
                    let in_hq =
                        hq_r.is_some_and(|(p, r)| (at - p).length_sq_raw() <= sq_m(r.into()));
                    (pct < cfg.crate_run_below_pct
                        && !in_hq
                        && !crate::supply::in_own_base(s, me, at))
                    .then(|| ((at - lead).length_sq_raw(), at))
                })
                .min_by_key(|&(d, at)| (d, at.x.raw(), at.y.raw()))
                .map(|(_, at)| (at + FxVec2::from_int(2, 2), false));
        }
        // Small arms: the lowest own squad holding a point (a Defend
        // task), below crate_run_below_pct, outside the base.
        if dest.is_none() && !shells_crate {
            dest = self
                .ai
                .tasks
                .iter()
                .filter(|(_, t)| t.kind == super::TaskKind::Defend)
                .filter_map(|(id, _)| {
                    let q = g.squads.get(id)?;
                    let pct = crate::supply::squad_ammo_pct(s, self.ents, q)?;
                    let at = self.ents.transform[q.lead(self.ents).index()].pos.xy();
                    (pct < cfg.crate_run_below_pct && !crate::supply::in_own_base(s, me, at))
                        .then_some((pct, *id, at))
                })
                .min_by_key(|&(p, id, _)| (p, id))
                .map(|(_, _, at)| (at + FxVec2::from_int(2, 0), false));
        }
        if dest.is_none()
            && let Some(h) = hq.filter(|h| h.built)
        {
            dest = Some((h.pos, false));
        }
        let Some((to, tank)) = dest else { return };
        let to = self.terrain.clamp(to);
        self.ai
            .emit(self.due, CommandKind::PlaceCrate { squad: b, pos: to });
        self.ai.counters.s6_crate_runs += 1;
        if tank {
            self.ai.counters.tank_crate_runs += 1;
        }
    }
}

/// Stock of `kind` in `h` as a share of its cap (%; 100 with no cap).
fn pct_of(
    h: &crate::structures::Structure,
    kind: SupplyKind,
    s: &frontline_content::MatchSetup,
) -> u32 {
    let cap = s.stocks[h.def as usize].cap_g[kind.index()];
    if cap == 0 {
        return 100;
    }
    (h.stock_g[kind.index()] as u64 * 100 / cap as u64) as u32
}
