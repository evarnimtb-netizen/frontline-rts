//! S6: the HQ, ammunition dumps and crates (docs/slices/S6-hq-crates.md).
//!
//! * **Structures.** `Build` makes the squad dig its def's pit
//!   (`DigProfile::HqPit` / `DumpPit`, a short line with all-round spoil,
//!   `crate::dig`); the structure stands once its builder's dig order has
//!   ended and the pit is at least [`BUILT_DEPTH_PCT`] deep at its centre.
//!   A builder that takes another order (or dies) before that leaves the
//!   structure abandoned: it is removed, its cost is not refunded (S6-8
//!   (A)). Built structures start empty and fill only from carried crates
//!   set down inside their radius (owner Q2 (C)).
//! * **Crates.** `IssueCrate` hands one crate of the faction's def of that
//!   kind to a man of the squad standing in the own base, paid there (kg x
//!   the kind's price, owner D1, through `MatchState::pay`); one per
//!   `base_crate_issue_s` per player; `crate_cap` / `shell_crate_cap` live
//!   crates. A carrier's squad walks at the crate's `carry_speed_pct` and
//!   the carrier does not fire. `PlaceCrate` walks the squad to a point and
//!   sets its crates down there; `PickUpCrate` sends the squad to a crate
//!   and the first man within `pickup_m` takes it. A carrier who falls
//!   drops it where he lies; it stays his player's (S6-3 (A)).
//! * **Sources** (every tick, after the base refill): a set-down crate
//!   inside an own built structure's radius empties into its stock up to
//!   the caps. Mortar squads take one bomb per `structure_bomb_s` inside a
//!   radius, else per `crate_bomb_s` from a shells crate within
//!   `crate_feed_m` of the gunner; the base refills none when
//!   `base_bomb_s = 0` (owner Q4). Men (outside the own base) get small
//!   arms and items every `serve_period_ms`, `serve_men` per source, the
//!   most depleted first, if they have not fired for `unfired_s`. Tanks
//!   (outside the own base) rearm forward (D3): one standard round per
//!   `fwd_rearm_s`, a rare one per `rare_rearm_fwd_s` up to `rare_cap`, the
//!   rack factor's difference credited to the owner. One source per client
//!   and service: structures (ascending id) before crates (ascending id), so
//!   rates never stack. Draws are free (paid at issue).
//! * **Supply trucks (S8b).** A truck (`[supply]`) is a third kind of
//!   source through the same code (`source_for`, `draw`): it rearms vehicles
//!   within `vehicle_radius_m` and men within `radius_m`, from a stock in
//!   `cargo_g` paid as it loads, stopped in the own base (shells on the
//!   crate queue, D1, no surcharge). It never feeds a mortar directly
//!   (`Reach::None`, S8b-4). `DropCrates` drives it to a point and sets
//!   crates down; a penetrating hit on the cargo box detonates it like a
//!   rack.
//! * **Boxes.** Built structures' boxes and set-down crates are oriented
//!   boxes ([`StaticBox`], derived each tick before the Ballistics stage).
//!   Bullets and shells stop at the first one in their path (`crate::
//!   combat::fly`), fragments too, and blasts reach them with a clear line;
//!   they take hit points. A broken crate or stock box loses its kg; when
//!   HE broke it and it held shells or explosives it cooks off ([`CookOff`]:
//!   bursts of the def's cook-off round owned by the player whose HE broke
//!   it, S6-2 (A)). A broken radio box sets `radio = false` (S6-7 (A)).
//!   A structure whose stock box breaks is destroyed.
//!
//! Timers are stateless where they can be: a client with a source is
//! served on ticks where `(tick + id) % every == 0`. All state lives in
//! [`SupplyState`] (saved, hashed through postcard, validated on load).

use std::collections::BTreeMap;

use frontline_content::setup::{MAX_FEEDS, StockKind, SupplyKind, member_trait};
use frontline_content::{ITEM_KINDS, MatchSetup};
use serde::{Deserialize, Serialize};

use crate::ecs::{Entities, EntityKind, PlayerId, SimId};
use crate::fx::{Fx, FxVec2, FxVec3};
use crate::game::{Ctx, MatchState, Order, SquadId};
use crate::hash::HashCache;
use crate::supply::{MICRO, feed_supply, in_own_base, man_load, man_total, micro_price};
use crate::terrain::Heightfield;
use crate::vehicle::{Basis, BoxFrame};

/// A structure stands once its pit is this deep at the centre (% of the
/// profile depth) and its builder's dig order has ended.
pub const BUILT_DEPTH_PCT: i32 = 90;
/// Most boxes per supply def (content checks 1..=4).
pub const MAX_BOXES: usize = 4;
/// Most structures, crates and cook-offs in one match (save bounds).
pub const MAX_STRUCTURES: usize = 256;
pub const MAX_CRATES: usize = 1024;
pub const MAX_COOKOFFS: usize = 256;
/// S8b: most crates in one `DropCrates`.
pub const MAX_DROP: u8 = 8;

/// An HQ or dump.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Structure {
    pub owner: PlayerId,
    /// Index into `MatchSetup::stocks`.
    pub def: u16,
    /// Pit centre.
    pub pos: FxVec2,
    /// Long-axis heading (radians, from +x).
    pub yaw: Fx,
    /// The squad digging its pit (until built).
    pub builder: Option<SquadId>,
    pub built: bool,
    /// Hit points by the def's box index.
    pub box_hp: [u32; MAX_BOXES],
    /// The radio box stands (HQ; S6-7 (A): a state only).
    pub radio: bool,
    /// Stock in grams, by `SupplyKind::index`.
    pub stock_g: [u32; 3],
    /// S12 W8: an aid post's beds (empty on every other kind).
    pub beds: Vec<crate::wounds::Bed>,
}

/// A crate: carried by a man, or set down.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Crate {
    pub owner: PlayerId,
    /// Index into `MatchSetup::stocks`.
    pub def: u16,
    /// Where it lies (while carried: where it was last set down or issued).
    pub pos: FxVec2,
    pub carrier: Option<SimId>,
    /// `PlaceCrate`: set it down here.
    pub dest: Option<FxVec2>,
    /// `PickUpCrate`: the squad sent to take it.
    pub claim: Option<SquadId>,
    /// Grams left.
    pub g: u32,
    pub hp: u32,
}

/// A broken box's cook-off: bursts of `ammo`'s blast at `pos`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct CookOff {
    /// Never reused (RNG key).
    pub id: u32,
    pub pos: FxVec3,
    /// The player whose HE broke the box (its bursts are his).
    pub player: u8,
    /// `game::kill_class` of that HE.
    pub class: u16,
    /// Index into `MatchSetup::ammo`.
    pub ammo: u16,
    pub left: u8,
    pub next_tick: u64,
    /// Ticks between bursts.
    pub every: u32,
}

/// S6 per-player counters and timers (report, HUD; saved and hashed).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct S6Account {
    /// The next tick a crate may be issued.
    pub issue_at: u64,
    /// The tick the last HQ was destroyed (0: never; `rebuild_s`).
    pub hq_lost_at: u64,
    /// The first tick an HQ of this player stood (0: never).
    pub hq_built_at: u64,
    pub structures_built: u32,
    pub structures_lost: u32,
    /// Crates issued, by supply kind, and the µMP paid for them.
    pub crates_issued: [u32; 3],
    pub crate_micro: u64,
    /// Crates set down outside the own base.
    pub crates_delivered: u32,
    /// Grams that entered an HQ or dump stock (only from crates).
    pub stock_in_g: u64,
    /// Bombs the base refilled (0 with `base_bomb_s = 0`).
    pub base_bombs: u32,
    /// Bombs from an HQ or dump, and from a crate.
    pub structure_bombs: u32,
    pub crate_bombs: u32,
    /// Bombs that appeared in mortar squads with no counted source (must
    /// stay 0; checked every supply tick).
    pub unsourced_bombs: u32,
    /// Small-arms rounds and items served forward.
    pub fwd_rounds: u32,
    pub fwd_items: u32,
    /// Main-gun rounds rearmed forward (standard, rare).
    pub tank_rounds_fwd: u32,
    pub tank_rare_fwd: u32,
    /// Main-gun rounds a tank took from a crate (not a structure).
    pub tank_crate_rounds: u32,
    /// µMP of the rack factor credited back (below one MP).
    pub rack_credit_micro: u64,
    /// Whole MP credited back by the rack factor.
    pub rack_credit_mp: u32,
    /// Cook-offs started (boxes HE broke with shells or explosives).
    pub cookoffs: u32,
    /// Crates lost to fire (bullets or HE).
    pub crates_lost: u32,
    /// Grams drawn from HQ and dump stocks, and lost with a broken stock
    /// box (with `stock_in_g` and the stock held: the T22 balance).
    pub stock_out_g: u64,
    pub stock_lost_g: u64,
    /// S8b: grams loaded into trucks and the µMP paid for them.
    pub truck_loaded_g: [u32; 3],
    pub truck_load_micro: u64,
    /// S8b: grams trucks served, dropped as crates and lost (loaded = out +
    /// dropped + lost + held).
    pub truck_out_g: u64,
    pub truck_dropped_g: u64,
    pub truck_lost_g: u64,
    /// S8b: crates set down by trucks.
    pub truck_crates_dropped: u32,
    /// S8b: main-gun rounds and small-arms rounds trucks gave out.
    pub truck_tank_rounds: u32,
    pub truck_men_rounds: u32,
    /// S8b: MG rounds any forward stock put into vehicle stowage.
    pub fwd_belt_rounds: u32,
    /// S8b: truck cargoes that detonated.
    pub cargo_detonations: u32,
}

/// All S6 state (`MatchState::supply6`).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct SupplyState {
    pub structures: BTreeMap<u32, Structure>,
    pub next_structure: u32,
    pub crates: BTreeMap<u32, Crate>,
    pub next_crate: u32,
    pub cookoffs: Vec<CookOff>,
    pub next_cookoff: u32,
    /// Per player.
    pub accounts: Vec<S6Account>,
}

/// Why an S6 command was refused (presentation only, like
/// `CallInRefusal`).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum SupplyRefusal {
    /// Not the player's squad, or no such def for the player's faction.
    NotAllowed,
    /// The squad cannot build this (tag or digging).
    CannotBuild,
    /// At `hq_max` / `dump_max`, `crate_cap` or `shell_crate_cap`.
    Cap,
    /// The HQ was lost less than `rebuild_s` ago: ticks left.
    Rebuild { ticks: u32 },
    /// Inside another player's base, or off the map.
    BadSpot,
    /// Short of manpower by this much.
    Manpower { short: u32 },
    /// No man of the squad in the own base without a crate.
    NoCarrier,
    /// The base issues the next crate in this many ticks.
    Busy { ticks: u32 },
    /// The squad carries no crate / there is no such crate.
    NoCrate,
}

/// One refused S6 command (presentation only).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct SupplyEvent {
    pub player: PlayerId,
    pub squad: SquadId,
    pub refusal: SupplyRefusal,
}

/// What a static box belongs to.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum StaticRef {
    /// Structure id, box index.
    Structure(u32, u8),
    Crate(u32),
    /// S10a: a fortification piece (`crate::fort`), by piece id.
    Piece(u32),
}

/// This tick's static boxes (derived: rebuilt each tick before the
/// Ballistics stage, never saved or hashed; equal whatever they hold, like
/// `VehicleDerived`).
#[derive(Clone, Debug, Default)]
pub struct StaticList(pub Vec<StaticBox>);

impl PartialEq for StaticList {
    fn eq(&self, _: &StaticList) -> bool {
        true
    }
}

impl Eq for StaticList {}

/// A built structure's box or a set-down crate (derived each tick).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct StaticBox {
    pub owner: u8,
    pub of: StaticRef,
    pub frame: BoxFrame,
    /// Bounding-sphere radius around `frame.center`.
    pub reach: Fx,
    /// S10a: armour (mm) a bullet or fragment must beat to damage it (0:
    /// an S6 box, no gate).
    pub armour_mm: u32,
}

impl SupplyState {
    pub fn new(players: usize) -> SupplyState {
        SupplyState {
            accounts: vec![S6Account::default(); players],
            ..SupplyState::default()
        }
    }

    /// Whether `man` carries a crate.
    pub fn carries(&self, man: SimId) -> bool {
        self.crates.values().any(|c| c.carrier == Some(man))
    }

    /// Live crates of `p` (all, shells).
    pub fn crates_of(&self, setup: &MatchSetup, p: PlayerId) -> (u32, u32) {
        let mut n = (0, 0);
        for c in self.crates.values().filter(|c| c.owner == p) {
            n.0 += 1;
            if setup.stocks[c.def as usize].supply == Some(SupplyKind::Shells) {
                n.1 += 1;
            }
        }
        n
    }

    /// Structures of `p` of `kind` (built or digging).
    pub fn count_of(&self, setup: &MatchSetup, p: PlayerId, kind: StockKind) -> u32 {
        self.structures
            .values()
            .filter(|s| s.owner == p && setup.stocks[s.def as usize].kind == kind)
            .count() as u32
    }

    /// The structure `pos` stands inside the radius of: `p`'s, built, not
    /// an aid post (S12: it holds no stock), ascending id.
    pub fn serving(&self, setup: &MatchSetup, p: PlayerId, pos: FxVec2) -> Option<u32> {
        self.structures
            .iter()
            .find(|(_, s)| {
                s.owner == p
                    && s.built
                    && setup.stocks[s.def as usize].kind != StockKind::Aid
                    && within(s.pos, pos, setup.stocks[s.def as usize].radius.into())
            })
            .map(|(&id, _)| id)
    }

    /// The nearest own built HQ to `from` that lies nearer the own base
    /// (`base`) than `from` does (retreat, S6-9 (A)): an HQ dug at the
    /// front never pulls a retreat toward the enemy.
    pub fn nearest_hq(
        &self,
        setup: &MatchSetup,
        p: PlayerId,
        from: FxVec2,
        base: FxVec2,
    ) -> Option<FxVec2> {
        let behind = (from - base).length_sq_raw();
        self.structures
            .values()
            .filter(|s| {
                s.owner == p
                    && s.built
                    && setup.stocks[s.def as usize].kind == StockKind::Hq
                    && (s.pos - base).length_sq_raw() < behind
            })
            .map(|s| ((s.pos - from).length_sq_raw(), s.pos))
            .min_by_key(|&(d, q)| (d, q.x.raw(), q.y.raw()))
            .map(|(_, q)| q)
    }

    /// Checks for a loaded state (saves are untrusted).
    pub(crate) fn validate(
        &self,
        setup: &MatchSetup,
        ents: &Entities,
        squads: &BTreeMap<SquadId, crate::game::Squad>,
    ) -> Result<(), &'static str> {
        let np = setup.players.len();
        if self.accounts.len() != np {
            return Err("S6 accounts disagree with the players");
        }
        if self.structures.len() > MAX_STRUCTURES
            || self.crates.len() > MAX_CRATES
            || self.cookoffs.len() > MAX_COOKOFFS
        {
            return Err("too many S6 structures, crates or cook-offs");
        }
        let sp = setup.rules.supply.as_ref();
        for (&id, s) in &self.structures {
            if id >= self.next_structure {
                return Err("structure id at or above next_structure");
            }
            let Some(d) = setup.stocks.get(s.def as usize) else {
                return Err("structure def out of range");
            };
            if d.kind == StockKind::Crate || s.owner.0 as usize >= np {
                return Err("structure def is a crate, or owner out of range");
            }
            let size = Fx::from_int(setup.map.size_m);
            if [s.pos.x, s.pos.y]
                .iter()
                .any(|v| *v < Fx::ZERO || *v > size)
            {
                return Err("structure off the map");
            }
            for k in 0..MAX_BOXES {
                let hp = d.boxes.get(k).map_or(0, |b| b.hp);
                if s.box_hp[k] > hp {
                    return Err("structure box hit points above the def's");
                }
            }
            if (0..3).any(|k| s.stock_g[k] > d.cap_g[k]) {
                return Err("structure stock above its cap");
            }
            if s.built == s.builder.is_some() {
                return Err("a structure is built xor has a builder");
            }
            if d.faction != setup.players[s.owner.0 as usize].faction {
                return Err("structure def of another faction");
            }
            // The Status stage drops a structure whose builder is gone or
            // stopped digging, so a saved builder is an own live squad.
            if s.builder
                .is_some_and(|b| squads.get(&b).is_none_or(|q| q.owner != s.owner))
            {
                return Err("structure builder is not an own squad");
            }
        }
        let size = Fx::from_int(setup.map.size_m);
        let on_map = |p: FxVec2| [p.x, p.y].iter().all(|v| *v >= Fx::ZERO && *v <= size);
        let mut carriers: Vec<SimId> = Vec::new();
        for (&id, c) in &self.crates {
            if id >= self.next_crate {
                return Err("crate id at or above next_crate");
            }
            let Some(d) = setup.stocks.get(c.def as usize) else {
                return Err("crate def out of range");
            };
            if d.kind != StockKind::Crate || c.owner.0 as usize >= np {
                return Err("crate def is not a crate, or owner out of range");
            }
            if c.g > d.g || c.hp > d.boxes.first().map_or(0, |b| b.hp) || c.hp == 0 {
                return Err("crate grams or hit points out of range");
            }
            if !on_map(c.pos) || c.dest.is_some_and(|q| !on_map(q)) {
                return Err("crate or its destination off the map");
            }
            if let Some(m) = c.carrier {
                let i = m.index();
                // Carriers who fell drop their crates in the Status stage,
                // after the Damage stage's deaths: a saved carrier lives.
                if i >= ents.len()
                    || ents.kind[i] != EntityKind::Soldier
                    || ents.owner[i] != c.owner
                    || ents.combat[i].hp == 0
                    || carriers.contains(&m)
                {
                    return Err("crate carrier is not a live own soldier, or carries two");
                }
                carriers.push(m);
            }
            if c.claim.is_some_and(|s| !squads.contains_key(&s)) {
                return Err("crate claimed by a missing squad");
            }
        }
        // Caps, once every def index is known good.
        if let Some(sp) = sp {
            for p in 0..np {
                let pl = PlayerId(p as u8);
                if self.count_of(setup, pl, StockKind::Hq) > sp.hq_max
                    || self.count_of(setup, pl, StockKind::Dump) > sp.dump_max
                    || self.count_of(setup, pl, StockKind::Aid) > sp.aid_max
                {
                    return Err("more structures than hq_max / dump_max / aid_max");
                }
                let (n, shells) = self.crates_of(setup, pl);
                if n > sp.crate_cap || shells > sp.shell_crate_cap {
                    return Err("more crates than crate_cap / shell_crate_cap");
                }
            }
        } else if !self.structures.is_empty() || !self.crates.is_empty() {
            return Err("S6 state without a [supply] table");
        }
        for k in &self.cookoffs {
            if k.id >= self.next_cookoff
                || k.player as usize >= np
                || k.left == 0
                || k.every == 0
                || !on_map(k.pos.xy())
                || setup
                    .ammo
                    .get(k.ammo as usize)
                    .is_none_or(|a| a.blast.is_none())
                // S11a1: the widened kill class names a class and a type
                // that exist.
                || crate::game::kill_class::base(k.class) as usize >= crate::game::kill_class::COUNT
                || crate::game::kill_class::vehicle(k.class).is_some_and(|t| t >= setup.vehicles.len())
            {
                return Err("cook-off out of range");
            }
        }
        if self.cookoffs.windows(2).any(|w| w[0].id >= w[1].id) {
            return Err("cook-offs not in ascending id");
        }
        Ok(())
    }
}

/// Greatest common divisor (intervals in ticks).
fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// Whether `a` and `b` are within `r` (m) of each other.
fn within(a: FxVec2, b: FxVec2, r: Fx) -> bool {
    let rr = r.raw().max(0) as u64;
    (a - b).length_sq_raw() <= rr * rr
}

/// Unit vector of yaw `a`.
fn dir(a: Fx) -> FxVec2 {
    let (s, c) = a.sin_cos();
    FxVec2::new(c, s)
}

/// Grams per round of `kind` that the served man's feed `f` uses.
fn round_g(setup: &MatchSetup, f: frontline_content::setup::Feed) -> Option<(SupplyKind, u32)> {
    feed_supply(setup, f)
}

/// Where a client draws: a structure's stock, a truck's cargo or a crate.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Source {
    Structure(u32),
    /// The truck and its owner.
    Truck(SimId, u8),
    Crate(u32),
}

/// S8b: a live supply truck this tick (ascending id).
#[derive(Copy, Clone)]
struct TruckSrc {
    id: SimId,
    owner: PlayerId,
    pos: FxVec2,
    /// Men and vehicles are served within these (m).
    men: Fx,
    vehicles: Fx,
    /// Ticks per standard main-gun round.
    every: u64,
}

/// Which clients a truck can serve in [`MatchState::source_for`].
#[derive(Copy, Clone)]
enum Reach {
    /// Trucks do not serve this client (a mortar: S8b-4).
    NoTruck,
    /// A vehicle: within `vehicle_radius_m`; `every` is the standard round
    /// interval of a structure or crate (the fastest source in reach wins).
    Vehicle { every: u64 },
}

impl MatchState {
    // ------------------------------------------------------------ queries --

    /// S6 state.
    #[inline]
    pub fn supply6(&self) -> &SupplyState {
        &self.supply6
    }

    /// This tick's static boxes (built structures, set-down crates).
    #[inline]
    pub fn static_boxes(&self) -> &[StaticBox] {
        &self.statics.0
    }

    /// Where a retreat of `p` from `from` goes: for infantry the nearest
    /// own built HQ nearer the base than `from`, else the base (S6-9 (A)).
    /// `base_only` (vehicle and crew squads): the base, the only place a
    /// tank is repaired and re-crewed (`crate::vehicle` base service).
    pub fn retreat_goal(&self, p: PlayerId, from: FxVec2, base_only: bool) -> FxVec2 {
        let base = self.base_of(p);
        if base_only {
            return base;
        }
        self.supply6
            .nearest_hq(&self.setup, p, from, base)
            .unwrap_or(base)
    }

    /// The two ends of structure `s`'s pit line.
    pub fn pit_line(&self, def: u16, pos: FxVec2, yaw: Fx) -> [FxVec2; 2] {
        let d = &self.setup.stocks[def as usize];
        let half = Fx::from(self.setup.rules.dig_profile(d.pit).pit_line)
            .div_int(2)
            .max(Fx::from_milli(100));
        let f = dir(yaw);
        [pos - f * half, pos + f * half]
    }

    fn refuse6(&mut self, p: PlayerId, squad: SquadId, refusal: SupplyRefusal) -> bool {
        self.supply_events.push(SupplyEvent {
            player: p,
            squad,
            refusal,
        });
        false
    }

    // ----------------------------------------------------------- commands --

    /// `Build { squad, def, pos, facing }` (module docs).
    pub(crate) fn order_build(
        &mut self,
        ctx: &mut Ctx,
        p: PlayerId,
        squad: SquadId,
        def: u16,
        pos: FxVec2,
        facing: FxVec2,
    ) -> bool {
        let Some(sp) = self.setup.rules.supply.clone() else {
            return false;
        };
        if !self.owned(p, squad) {
            return self.refuse6(p, squad, SupplyRefusal::NotAllowed);
        }
        let faction = self.setup.players[p.0 as usize].faction;
        let Some(d) = self.setup.stocks.get(def as usize).cloned() else {
            return self.refuse6(p, squad, SupplyRefusal::NotAllowed);
        };
        if d.faction != faction || d.kind == StockKind::Crate {
            return self.refuse6(p, squad, SupplyRefusal::NotAllowed);
        }
        let ty = &self.setup.squad_types[self.squads[&squad].type_idx as usize];
        if !d.built_by.iter().any(|&t| ty.has_tag(Some(t))) || !self.digs_profile(ty, d.pit) {
            return self.refuse6(p, squad, SupplyRefusal::CannotBuild);
        }
        let max = match d.kind {
            StockKind::Hq => sp.hq_max,
            // S12 W8.
            StockKind::Aid => sp.aid_max,
            _ => sp.dump_max,
        };
        if self.supply6.count_of(&self.setup, p, d.kind) >= max {
            return self.refuse6(p, squad, SupplyRefusal::Cap);
        }
        let acc = self.supply6.accounts[p.0 as usize];
        if d.kind == StockKind::Hq && acc.hq_lost_at > 0 {
            let ready = acc.hq_lost_at + d.rebuild_ticks as u64;
            if ctx.tick < ready {
                let ticks = (ready - ctx.tick).min(u32::MAX as u64) as u32;
                return self.refuse6(p, squad, SupplyRefusal::Rebuild { ticks });
            }
        }
        let enemy_base = (0..self.setup.players.len())
            .filter(|&q| q != p.0 as usize)
            .any(|q| in_own_base(&self.setup, PlayerId(q as u8), pos));
        if !ctx.terrain.contains(pos) || enemy_base {
            return self.refuse6(p, squad, SupplyRefusal::BadSpot);
        }
        let mp = self.players[p.0 as usize].manpower;
        if mp < d.cost {
            return self.refuse6(p, squad, SupplyRefusal::Manpower { short: d.cost - mp });
        }
        let f = facing.normalize_or_zero();
        let yaw = if f == FxVec2::ZERO {
            Fx::ZERO
        } else {
            Fx::atan2(f.y, f.x)
        };
        let line = self.pit_line(def, pos, yaw);
        if !self.order_dig(ctx, p, squad, &line, d.pit) {
            return self.refuse6(p, squad, SupplyRefusal::BadSpot);
        }
        self.players[p.0 as usize].manpower -= d.cost;
        let hp = |k: usize| d.boxes.get(k).map_or(0, |b| b.hp);
        let id = self.supply6.next_structure;
        self.supply6.next_structure += 1;
        self.supply6.structures.insert(
            id,
            Structure {
                owner: p,
                def,
                pos,
                yaw,
                builder: Some(squad),
                built: false,
                box_hp: [hp(0), hp(1), hp(2), hp(3)],
                radio: d.boxes.iter().any(|b| b.part == 1),
                stock_g: [0; 3],
                beds: Vec::new(),
            },
        );
        true
    }

    /// `IssueCrate { squad, kind }` (module docs): paid now, one per
    /// `base_crate_issue_s` per player.
    pub(crate) fn issue_crate(
        &mut self,
        ctx: &mut Ctx,
        p: PlayerId,
        squad: SquadId,
        kind: SupplyKind,
    ) -> bool {
        let Some(sp) = self.setup.rules.supply.clone() else {
            return false;
        };
        if !self.owned(p, squad) {
            return self.refuse6(p, squad, SupplyRefusal::NotAllowed);
        }
        let faction = self.setup.players[p.0 as usize].faction;
        let Some(def) = self.setup.stock_of(faction, StockKind::Crate, Some(kind)) else {
            return self.refuse6(p, squad, SupplyRefusal::NotAllowed);
        };
        let (n, shells) = self.supply6.crates_of(&self.setup, p);
        if n >= sp.crate_cap || (kind == SupplyKind::Shells && shells >= sp.shell_crate_cap) {
            return self.refuse6(p, squad, SupplyRefusal::Cap);
        }
        let at = self.supply6.accounts[p.0 as usize].issue_at;
        if ctx.tick < at {
            let ticks = (at - ctx.tick).min(u32::MAX as u64) as u32;
            return self.refuse6(p, squad, SupplyRefusal::Busy { ticks });
        }
        let ty = &self.setup.squad_types[self.squads[&squad].type_idx as usize];
        let carrier = self.squads[&squad]
            .members
            .iter()
            .copied()
            .filter(|&m| {
                ctx.ents.kind[m.index()] == EntityKind::Soldier
                    && in_own_base(&self.setup, p, ctx.ents.transform[m.index()].pos.xy())
                    && !self.supply6.carries(m)
            })
            .min_by_key(|&m| {
                let t = ty
                    .members
                    .get(ctx.ents.kit[m.index()].slot as usize)
                    .map_or(0, |s| s.traits);
                (u8::from(t & member_trait::AMMO_BEARER == 0), m.0)
            });
        let Some(carrier) = carrier else {
            return self.refuse6(p, squad, SupplyRefusal::NoCarrier);
        };
        let d = &self.setup.stocks[def as usize];
        let (g, hp) = (d.g, d.boxes.first().map_or(1, |b| b.hp));
        let micro = micro_price(&self.setup, kind, g);
        let have = self.players[p.0 as usize].manpower as u64 * MICRO
            + self.players[p.0 as usize].ammo.credit as u64;
        if have < micro {
            let short = (micro - have).div_ceil(MICRO) as u32;
            return self.refuse6(p, squad, SupplyRefusal::Manpower { short });
        }
        if kind.is_ammo() {
            let paid = self.pay(p.0 as usize, kind, micro, ctx.tick);
            debug_assert!(paid);
            // `pay` counts a round refilled; a crate is not one.
            let r = &mut self.players[p.0 as usize].ammo.refilled[kind.index()];
            *r = r.saturating_sub(1);
        } else {
            // S10a: construction is outside the three-wide arrays.
            self.pay_micro(p.0 as usize, micro);
            let fa = &mut self.fort.accounts[p.0 as usize];
            fa.materials_loaded_g += g as u64;
            fa.materials_micro += micro;
        }
        let hz = self.setup.tick_hz as u64;
        let acc = &mut self.supply6.accounts[p.0 as usize];
        acc.issue_at = ctx.tick + sp.base_crate_issue_s as u64 * hz;
        if kind.is_ammo() {
            acc.crates_issued[kind.index()] += 1;
            acc.crate_micro += micro;
        }
        let id = self.supply6.next_crate;
        self.supply6.next_crate += 1;
        self.supply6.crates.insert(
            id,
            Crate {
                owner: p,
                def,
                pos: ctx.ents.transform[carrier.index()].pos.xy(),
                carrier: Some(carrier),
                dest: None,
                claim: None,
                g,
                hp,
            },
        );
        true
    }

    /// `PlaceCrate { squad, pos }`: the squad walks to `pos` and sets its
    /// crates down there.
    pub(crate) fn place_crate(
        &mut self,
        ctx: &mut Ctx,
        p: PlayerId,
        squad: SquadId,
        pos: FxVec2,
    ) -> bool {
        if self.setup.rules.supply.is_none() {
            return false;
        }
        if !self.owned(p, squad) {
            return self.refuse6(p, squad, SupplyRefusal::NotAllowed);
        }
        let pos = ctx.terrain.clamp(pos);
        let members = self.squads[&squad].members.clone();
        let mut any = false;
        for c in self.supply6.crates.values_mut() {
            if c.carrier.is_some_and(|m| members.contains(&m)) {
                c.dest = Some(pos);
                any = true;
            }
        }
        if !any {
            return self.refuse6(p, squad, SupplyRefusal::NoCrate);
        }
        self.order_move(ctx, p, squad, Order::Move { target: pos })
    }

    /// `PickUpCrate { squad, crate_id }`: the squad walks to an own
    /// set-down crate; its first man within `pickup_m` takes it.
    pub(crate) fn pick_up_crate(
        &mut self,
        ctx: &mut Ctx,
        p: PlayerId,
        squad: SquadId,
        crate_id: u32,
    ) -> bool {
        if self.setup.rules.supply.is_none() {
            return false;
        }
        if !self.owned(p, squad) {
            return self.refuse6(p, squad, SupplyRefusal::NotAllowed);
        }
        let ok = self.supply6.crates.get(&crate_id).is_some_and(|c| {
            // S9: an enemy's set-down crate can be taken (it keeps its
            // own faction's rounds, `source_for`).
            c.carrier.is_none() && (c.owner == p || c.claim.is_none())
        });
        if !ok {
            return self.refuse6(p, squad, SupplyRefusal::NoCrate);
        }
        let at = self.supply6.crates[&crate_id].pos;
        self.supply6
            .crates
            .get_mut(&crate_id)
            .expect("checked")
            .claim = Some(squad);
        self.order_move(ctx, p, squad, Order::Move { target: at })
    }

    // -------------------------------------------------------------- tick --

    /// S6 Status-stage pass (module docs), after the S5a supply tick.
    /// `bombs_before` is each player's mortar bombs before that tick.
    pub(crate) fn structures_tick(
        &mut self,
        ents: &mut Entities,
        terrain: &Heightfield,
        hash: &mut HashCache,
        tick: u64,
        bombs_before: &[u64],
    ) {
        let Some(sp) = self.setup.rules.supply.clone() else {
            return;
        };
        self.update_structures(terrain, tick);
        self.update_crates(ents);
        self.stock_in(&sp);
        self.feed_mortars(ents, hash, tick, &sp);
        let trucks = self.trucks_now(ents);
        self.load_trucks(tick, &sp, &trucks);
        self.drop_crates(ents, terrain, tick, &sp, &trucks);
        let period = (sp.serve_period_ms as u64 * self.setup.tick_hz as u64 / 1000).max(1);
        if tick.is_multiple_of(period) {
            self.serve_men(ents, hash, tick, &sp, period, &trucks);
        }
        self.rearm_forward(ents, tick, &sp, &trucks);
        // T20: every bomb that entered a mortar squad came from a counted
        // source.
        let after = self.mortar_bombs_by_player(ents);
        for (p, (&b, &a)) in bombs_before.iter().zip(&after).enumerate() {
            if a > b {
                let gained = a - b;
                let explained = self.bombs_explained.get(p).copied().unwrap_or(0);
                if gained > explained {
                    self.supply6.accounts[p].unsourced_bombs += (gained - explained) as u32;
                }
            }
        }
        self.bombs_explained.clear();
    }

    /// Bombs each player's counted mortar squads hold (T20 check).
    pub(crate) fn mortar_bombs_by_player(&self, ents: &Entities) -> Vec<u64> {
        let mut out = vec![0u64; self.players.len()];
        for sq in self.squads.values() {
            let w =
                &self.setup.weapons[self.setup.squad_types[sq.type_idx as usize].weapon as usize];
            if w.class != frontline_content::WeaponClass::Mortar || w.he.is_none() {
                continue;
            }
            let (have, pct) = crate::supply::mortar_bombs(&self.setup, ents, sq);
            if pct.is_some() {
                out[sq.owner.0 as usize] += have as u64;
            }
        }
        out
    }

    /// Records `n` bombs that entered player `p`'s mortar squads from a
    /// counted source this tick (T20 check).
    pub(crate) fn explain_bombs(&mut self, p: usize, n: u64) {
        if self.bombs_explained.len() < self.players.len() {
            self.bombs_explained.resize(self.players.len(), 0);
        }
        self.bombs_explained[p] += n;
    }

    /// Builders finish (or abandon) their pits.
    fn update_structures(&mut self, terrain: &Heightfield, tick: u64) {
        let ids: Vec<u32> = self.supply6.structures.keys().copied().collect();
        for id in ids {
            let s = &self.supply6.structures[&id];
            let Some(b) = s.builder else { continue };
            let d = &self.setup.stocks[s.def as usize];
            let line = self.pit_line(s.def, s.pos, s.yaw);
            let digging = self.squads.get(&b).is_some_and(
                |sq| matches!(&sq.order, Order::Dig { line: l, .. } if l[..] == line[..]),
            );
            if digging {
                continue;
            }
            let depth = Fx::from(self.setup.rules.dig_profile(d.pit).depth);
            let dug = terrain.base_height_at(s.pos) - terrain.height_at(s.pos);
            let owner = s.owner.0 as usize;
            let is_hq = d.kind == StockKind::Hq;
            if dug.mul_int(100) >= depth.mul_int(BUILT_DEPTH_PCT) {
                let s = self.supply6.structures.get_mut(&id).expect("listed");
                s.builder = None;
                s.built = true;
                let acc = &mut self.supply6.accounts[owner];
                acc.structures_built += 1;
                if is_hq && acc.hq_built_at == 0 {
                    acc.hq_built_at = tick.max(1);
                }
            } else {
                self.supply6.structures.remove(&id);
            }
        }
    }

    /// Carriers who fell drop their crates; placed crates are set down;
    /// claimed crates are picked up.
    fn update_crates(&mut self, ents: &Entities) {
        let reach = Fx::from_int(self.setup.rules.supply.as_ref().map_or(2, |s| s.pickup_m) as i32);
        let ids: Vec<u32> = self.supply6.crates.keys().copied().collect();
        for id in ids {
            let c = self.supply6.crates[&id].clone();
            match c.carrier {
                Some(m) => {
                    let i = m.index();
                    let alive = ents.kind.get(i) == Some(&EntityKind::Soldier)
                        && ents.owner[i] == c.owner
                        && ents.combat[i].hp > 0;
                    let at = if i < ents.len() {
                        ents.transform[i].pos.xy()
                    } else {
                        c.pos
                    };
                    let cr = self.supply6.crates.get_mut(&id).expect("listed");
                    cr.pos = at;
                    if !alive {
                        cr.carrier = None;
                        cr.dest = None;
                        continue;
                    }
                    // Set down within 2 x pickup_m of the point, or where
                    // he stands once his squad holds its formation there
                    // (his slot can lie a few metres off the point). The
                    // AI aims at the client (the mortar's gunner, beside
                    // a hull), so the crate lands inside the feed range
                    // (review S6 F2).
                    let holding =
                        ents.squad[i]
                            .and_then(|q| self.squads.get(&q))
                            .is_some_and(|q| {
                                q.nav.phase == crate::game::NavPhase::Holding
                                    && c.dest
                                        .is_some_and(|d| within(q.nav.goal, d, reach.mul_int(2)))
                            });
                    if let Some(dest) = c.dest
                        && (holding || within(at, dest, reach.mul_int(2)))
                    {
                        cr.carrier = None;
                        cr.dest = None;
                        if !in_own_base(&self.setup, c.owner, at) {
                            let a = &mut self.supply6.accounts[c.owner.0 as usize];
                            a.crates_delivered = a.crates_delivered.saturating_add(1);
                        }
                    }
                }
                None => {
                    let Some(sid) = c.claim else { continue };
                    let Some(sq) = self.squads.get(&sid) else {
                        self.supply6.crates.get_mut(&id).expect("listed").claim = None;
                        continue;
                    };
                    let taker = sq
                        .members
                        .iter()
                        .copied()
                        .filter(|&m| {
                            ents.kind[m.index()] == EntityKind::Soldier
                                && within(ents.transform[m.index()].pos.xy(), c.pos, reach)
                        })
                        .find(|&m| !self.supply6.carries(m));
                    if let Some(m) = taker {
                        let cr = self.supply6.crates.get_mut(&id).expect("listed");
                        cr.carrier = Some(m);
                        cr.claim = None;
                        // S9: a captured crate becomes the taker's.
                        cr.owner = sq.owner;
                    }
                }
            }
        }
    }

    /// Set-down crates inside an own built structure's radius empty into
    /// its stock up to the caps (the only way stock comes in, Q2 (C)).
    fn stock_in(&mut self, _sp: &frontline_content::tuning::SupplyTuning) {
        let ids: Vec<u32> = self.supply6.crates.keys().copied().collect();
        for id in ids {
            let c = &self.supply6.crates[&id];
            if c.carrier.is_some() {
                continue;
            }
            let Some(sid) = self.supply6.serving(&self.setup, c.owner, c.pos) else {
                continue;
            };
            let kind = self.setup.stocks[c.def as usize]
                .supply
                .unwrap_or(SupplyKind::Shells);
            // S10a: construction crates never enter a stock.
            if !kind.is_ammo() {
                continue;
            }
            let k = kind.index();
            let s = &self.supply6.structures[&sid];
            // S9: a captured crate stays a crate; it does not fit this stock.
            if self.setup.stocks[c.def as usize].faction
                != self.setup.stocks[s.def as usize].faction
            {
                continue;
            }
            let cap = self.setup.stocks[s.def as usize].cap_g[k];
            let room = cap.saturating_sub(s.stock_g[k]);
            let n = room.min(c.g);
            if n == 0 {
                continue;
            }
            let owner = c.owner.0 as usize;
            self.supply6
                .structures
                .get_mut(&sid)
                .expect("found")
                .stock_g[k] += n;
            let c = self.supply6.crates.get_mut(&id).expect("listed");
            c.g -= n;
            if c.g == 0 {
                self.supply6.crates.remove(&id);
            }
            self.supply6.accounts[owner].stock_in_g += n as u64;
        }
    }

    /// The source serving a client of `p` at `pos` for supply `kind` with
    /// at least `g` grams: the fastest in reach, ties structures, then
    /// trucks, then crates (ascending ids); a structure whose radius holds
    /// it, a truck within its radius (`reach`), a set-down crate of that
    /// kind within `feed`. One source per client and service: rates never
    /// stack.
    #[allow(clippy::too_many_arguments)]
    fn source_for(
        &self,
        p: PlayerId,
        pos: FxVec2,
        kind: SupplyKind,
        g: u32,
        feed: Fx,
        trucks: &[TruckSrc],
        reach: Reach,
        gun_faction: Option<u16>,
    ) -> Option<Source> {
        let k = kind.index();
        // S9: a gun takes only what fits it: its own def faction's stock.
        // Crates carry their faction for everyone: a captured crate feeds
        // only weapons of that faction.
        let own = self.setup.players[p.0 as usize].faction;
        let crate_faction = gun_faction.unwrap_or(own);
        let structure = self
            .supply6
            .structures
            .iter()
            .find(|(_, s)| {
                s.owner == p
                    && s.built
                    && s.stock_g[k] >= g
                    && gun_faction.is_none_or(|f| self.setup.stocks[s.def as usize].faction == f)
                    && within(s.pos, pos, self.setup.stocks[s.def as usize].radius.into())
            })
            .map(|(&id, _)| Source::Structure(id));
        let (every, truck) = match reach {
            Reach::NoTruck => (0, None),
            Reach::Vehicle { every } => (
                every,
                trucks
                    .iter()
                    .filter(|t| {
                        t.owner == p
                            && within(t.pos, pos, t.vehicles)
                            && self.vehicles[&t.id].cargo_g[k] >= g
                            && gun_faction.is_none_or(|f| {
                                self.setup.vehicles[self.vehicles[&t.id].type_idx as usize].faction
                                    == f
                            })
                    })
                    .min_by_key(|t| (t.every, t.id.0))
                    .map(|t| (Source::Truck(t.id, t.owner.0), t.every)),
            ),
        };
        if let Some((src, t)) = truck
            && t < every
        {
            return Some(src);
        }
        if structure.is_some() {
            return structure;
        }
        if let Some((src, t)) = truck
            && t == every
        {
            return Some(src);
        }
        self.supply6
            .crates
            .iter()
            .find(|(_, c)| {
                c.owner == p
                    && c.carrier.is_none()
                    && c.g >= g
                    && self.setup.stocks[c.def as usize].supply == Some(kind)
                    && self.setup.stocks[c.def as usize].faction == crate_faction
                    && within(c.pos, pos, feed)
            })
            .map(|(&id, _)| Source::Crate(id))
            .or(truck.map(|(src, _)| src))
    }

    /// Grams of `kind` at `src`.
    fn stock_held(&self, src: Source, kind: SupplyKind) -> u32 {
        let k = kind.index();
        match src {
            Source::Structure(id) => self.supply6.structures.get(&id).map_or(0, |s| s.stock_g[k]),
            Source::Truck(id, _) => self.vehicles.get(&id).map_or(0, |v| v.cargo_g[k]),
            Source::Crate(id) => self.supply6.crates.get(&id).map_or(0, |c| {
                if self.setup.stocks[c.def as usize].supply == Some(kind) {
                    c.g
                } else {
                    0
                }
            }),
        }
    }

    /// Takes `g` grams of `kind` from `src` (checked by `source_for`).
    fn draw(&mut self, src: Source, kind: SupplyKind, g: u32) {
        match src {
            Source::Structure(id) => {
                let s = self.supply6.structures.get_mut(&id).expect("source");
                s.stock_g[kind.index()] -= g;
                let o = s.owner.0 as usize;
                self.supply6.accounts[o].stock_out_g += g as u64;
            }
            Source::Truck(id, o) => {
                self.vehicles.get_mut(&id).expect("source").cargo_g[kind.index()] -= g;
                self.supply6.accounts[o as usize].truck_out_g += g as u64;
            }
            Source::Crate(id) => {
                let c = self.supply6.crates.get_mut(&id).expect("source");
                c.g -= g;
                if c.g == 0 {
                    self.supply6.crates.remove(&id);
                }
            }
        }
    }

    /// Mortar squads' bombs from structures and crates (module docs).
    fn feed_mortars(
        &mut self,
        ents: &mut Entities,
        hash: &mut HashCache,
        tick: u64,
        sp: &frontline_content::tuning::SupplyTuning,
    ) {
        let hz = self.setup.tick_hz as u64;
        let feed = Fx::from_int(sp.crate_feed_m as i32);
        let ids: Vec<SquadId> = self.squads.keys().copied().collect();
        for sid in ids {
            let sq = &self.squads[&sid];
            let ty = &self.setup.squad_types[sq.type_idx as usize];
            let w = &self.setup.weapons[ty.weapon as usize];
            if w.he.is_none() || w.class != frontline_content::WeaponClass::Mortar {
                continue;
            }
            let Some(k) = ty.feed_slot(w) else { continue };
            let Some((kind, g)) = round_g(&self.setup, ty.feeds[k]) else {
                continue;
            };
            let Some(gunner) = mortar_gunner(ty, ents, &sq.members) else {
                continue;
            };
            let at = ents.transform[gunner.index()].pos.xy();
            if in_own_base(&self.setup, sq.owner, at) {
                continue;
            }
            // The man furthest below his load of bombs.
            let taker = sq
                .members
                .iter()
                .copied()
                .filter_map(|m| {
                    let i = m.index();
                    let load = man_load(ty, &ents.kit[i], k);
                    let mut t = man_total(&self.setup, ty, &ents.kit[i], &ents.combat[i], k);
                    if crate::supply::active_weapon(ty, &ents.kit[i]) == ty.weapon {
                        t += ents.combat[i].mag as u32;
                    }
                    (load > 0 && t < load).then(|| (t * 1000 / load, m))
                })
                .min_by_key(|&(s, m)| (s, m.0))
                .map(|(_, m)| m);
            // S15 X1: no own source in reach: a crate of the foreign feed.
            let own = taker.and_then(|_| {
                self.source_for(sq.owner, at, kind, g, feed, &[], Reach::NoTruck, None)
            });
            if own.is_none() && w.foreign_feed.is_some() {
                self.feed_foreign(ents, hash, tick, sid, at, k, feed, sp);
                continue;
            }
            let Some(taker) = taker else { continue };
            let Some(src) = own else {
                continue;
            };
            let every = match src {
                Source::Structure(_) | Source::Truck(..) => sp.structure_bomb_s,
                Source::Crate(_) => sp.crate_bomb_s,
            } as u64
                * hz;
            if !(tick + sid.0 as u64).is_multiple_of(every.max(1)) {
                continue;
            }
            let owner = sq.owner.0 as usize;
            self.draw(src, kind, g);
            ents.kit[taker.index()].rounds[k] += 1;
            hash.mark_entity_chunk(taker.chunk());
            let acc = &mut self.supply6.accounts[owner];
            match src {
                Source::Structure(_) | Source::Truck(..) => acc.structure_bombs += 1,
                Source::Crate(_) => acc.crate_bombs += 1,
            }
            self.explain_bombs(owner, 1);
        }
    }

    /// S15 X1: mortar squad `sid` (its mortar has a foreign feed, no own
    /// source in reach of its gunner at `at`) takes one foreign bomb from
    /// a set-down crate its player holds of the feed's faction and supply
    /// kind within `feed`, at the crate bomb rate and no price, into the
    /// man furthest below his load (own `k` plus foreign rounds).
    #[allow(clippy::too_many_arguments)]
    fn feed_foreign(
        &mut self,
        ents: &mut Entities,
        hash: &mut HashCache,
        tick: u64,
        sid: SquadId,
        at: FxVec2,
        k: usize,
        feed: Fx,
        sp: &frontline_content::tuning::SupplyTuning,
    ) {
        let sq = &self.squads[&sid];
        let ty = &self.setup.squad_types[sq.type_idx as usize];
        let Some(ff) = self.setup.weapons[ty.weapon as usize].foreign_feed else {
            return;
        };
        let Some((kind, g)) = round_g(&self.setup, ff.feed) else {
            return;
        };
        let taker = sq
            .members
            .iter()
            .copied()
            .filter_map(|m| {
                let i = m.index();
                let load = man_load(ty, &ents.kit[i], k);
                let mut t = man_total(&self.setup, ty, &ents.kit[i], &ents.combat[i], k);
                if crate::supply::active_weapon(ty, &ents.kit[i]) == ty.weapon {
                    t += ents.combat[i].mag as u32;
                }
                t += ents.kit[i].foreign as u32;
                (load > 0 && t < load).then(|| (t * 1000 / load, m))
            })
            .min_by_key(|&(s, m)| (s, m.0))
            .map(|(_, m)| m);
        let Some(taker) = taker else { return };
        let owner = sq.owner;
        let crate_id = self
            .supply6
            .crates
            .iter()
            .find(|(_, c)| {
                c.owner == owner
                    && c.carrier.is_none()
                    && c.g >= g
                    && self.setup.stocks[c.def as usize].supply == Some(kind)
                    && self.setup.stocks[c.def as usize].faction == ff.faction
                    && within(c.pos, at, feed)
            })
            .map(|(&id, _)| id);
        let Some(id) = crate_id else { return };
        let every = sp.crate_bomb_s as u64 * self.setup.tick_hz as u64;
        if !(tick + sid.0 as u64).is_multiple_of(every.max(1)) {
            return;
        }
        self.draw(Source::Crate(id), kind, g);
        let kit = &mut ents.kit[taker.index()];
        kit.foreign = kit.foreign.saturating_add(1);
        hash.mark_entity_chunk(taker.chunk());
        let c = &mut self.players[owner.0 as usize].s15;
        c.foreign_drawn = c.foreign_drawn.saturating_add(1);
        self.explain_bombs(owner.0 as usize, 1);
    }

    /// Men outside the own base take small arms and items from a source
    /// (module docs).
    fn serve_men(
        &mut self,
        ents: &mut Entities,
        hash: &mut HashCache,
        tick: u64,
        sp: &frontline_content::tuning::SupplyTuning,
        period: u64,
        trucks: &[TruckSrc],
    ) {
        let hz = self.setup.tick_hz as u64;
        let unfired = (sp.unfired_s as u64 * hz).min(u16::MAX as u64) as u16;
        let per = (sp.base_small_arms_rounds_s as u64 * sp.serve_period_ms as u64 / 1000).max(1);
        let item_every = (self.setup.rules.items.resupply_ticks as u64 / period).max(1);
        let feed = Fx::from_int(sp.crate_feed_m as i32);
        // Sources in order: structures, trucks, then crates.
        let mut sources: Vec<(Source, PlayerId, FxVec2, Fx)> = Vec::new();
        for (&id, s) in &self.supply6.structures {
            // S12: an aid post holds no stock and serves nobody.
            if s.built && self.setup.stocks[s.def as usize].kind != StockKind::Aid {
                let r = self.setup.stocks[s.def as usize].radius;
                sources.push((Source::Structure(id), s.owner, s.pos, r.into()));
            }
        }
        for t in trucks {
            sources.push((Source::Truck(t.id, t.owner.0), t.owner, t.pos, t.men));
        }
        for (&id, c) in &self.supply6.crates {
            let kind = self.setup.stocks[c.def as usize].supply;
            // S10a: a construction crate serves no rounds.
            if c.carrier.is_none()
                && kind != Some(SupplyKind::Shells)
                && kind != Some(SupplyKind::Construction)
            {
                sources.push((Source::Crate(id), c.owner, c.pos, feed));
            }
        }
        if sources.is_empty() {
            return;
        }
        let mut served: Vec<SimId> = Vec::new();
        for (src, owner, at, r) in sources {
            // Clients: (most depleted share, id, man, feed slot or none).
            let mut cands: Vec<(u32, SimId)> = Vec::new();
            for m in ents.soldiers() {
                let i = m.index();
                if ents.owner[i] != owner
                    || served.binary_search(&m).is_ok()
                    || ents.combat[i].hp == 0
                    || ents.combat[i].pinned
                    || ents.combat[i].since_shot < unfired
                {
                    continue;
                }
                let pos = ents.transform[i].pos.xy();
                if !within(at, pos, r) || in_own_base(&self.setup, owner, pos) {
                    continue;
                }
                let Some(sq) = ents.squad[i].and_then(|s| self.squads.get(&s)) else {
                    continue;
                };
                let ty = &self.setup.squad_types[sq.type_idx as usize];
                if !crate::supply::counted(ty) {
                    continue;
                }
                let mut share = 1000u32;
                for k in 0..ty.feeds.len().min(MAX_FEEDS) {
                    if round_g(&self.setup, ty.feeds[k]).map(|x| x.0) != Some(SupplyKind::SmallArms)
                    {
                        continue;
                    }
                    let load = man_load(ty, &ents.kit[i], k);
                    if load == 0 {
                        continue;
                    }
                    let t = man_total(&self.setup, ty, &ents.kit[i], &ents.combat[i], k);
                    share = share.min(t * 1000 / load);
                }
                let full = ty.full_kit(ents.kit[i].slot);
                let items_short = (0..ITEM_KINDS).any(|j| ents.kit[i].items[j] < full[j]);
                if share < 1000 || items_short {
                    cands.push((share, m));
                }
            }
            cands.sort_unstable_by_key(|&(s, m)| (s, m.0));
            for &(_, m) in cands.iter().take(sp.serve_men as usize) {
                let i = m.index();
                let ty_idx = self.squads[&ents.squad[i].expect("squad")].type_idx as usize;
                let ty = &self.setup.squad_types[ty_idx];
                let mut any = false;
                // Small arms: the most depleted feed.
                let pick = (0..ty.feeds.len().min(MAX_FEEDS))
                    .filter_map(|k| {
                        let (kind, g) = round_g(&self.setup, ty.feeds[k])?;
                        let load = man_load(ty, &ents.kit[i], k);
                        let t = man_total(&self.setup, ty, &ents.kit[i], &ents.combat[i], k);
                        (kind == SupplyKind::SmallArms && t < load)
                            .then(|| (t * 1000 / load.max(1), k, g, load - t))
                    })
                    .min();
                if let Some((_, k, g, short)) = pick {
                    let have = self.stock_held(src, SupplyKind::SmallArms);
                    let n = (per as u32).min(short).min(have / g.max(1));
                    if n > 0 {
                        self.draw(src, SupplyKind::SmallArms, n * g);
                        ents.kit[i].rounds[k] += n as u16;
                        let acc = &mut self.supply6.accounts[owner.0 as usize];
                        acc.fwd_rounds += n;
                        if matches!(src, Source::Truck(..)) {
                            acc.truck_men_rounds += n;
                        }
                        any = true;
                    }
                }
                // Items: one at a time, at the base's item pace.
                if (tick / period + m.0 as u64).is_multiple_of(item_every) {
                    let ty = &self.setup.squad_types[ty_idx];
                    let full = ty.full_kit(ents.kit[i].slot);
                    let item = (0..ITEM_KINDS).find(|&j| ents.kit[i].items[j] < full[j]);
                    if let Some(j) = item {
                        let (kind, g) = ty.items[j]
                            .and_then(|x| self.setup.items[x as usize].supply)
                            .map_or((SupplyKind::Explosives, 0), |s| (s.kind, s.g));
                        let have = self.stock_held(src, kind);
                        if g > 0 && have >= g {
                            self.draw(src, kind, g);
                            ents.kit[i].items[j] += 1;
                            self.supply6.accounts[owner.0 as usize].fwd_items += 1;
                            any = true;
                        }
                    }
                }
                if any {
                    hash.mark_entity_chunk(m.chunk());
                }
                let at = served.binary_search(&m).unwrap_or_else(|e| e);
                served.insert(at, m);
                // A crate used up is gone: stop serving from it.
                if let Source::Crate(id) = src
                    && !self.supply6.crates.contains_key(&id)
                {
                    break;
                }
            }
        }
    }

    /// D3 forward rearm of vehicles outside the own base (module docs): one
    /// main-gun round per due step and (S8b) MG belts, from one source at a
    /// time, the fastest in reach.
    fn rearm_forward(
        &mut self,
        ents: &Entities,
        tick: u64,
        sp: &frontline_content::tuning::SupplyTuning,
        trucks: &[TruckSrc],
    ) {
        let hz = self.setup.tick_hz as u64;
        let std_every = (sp.fwd_rearm_s as u64 * hz).max(1);
        let rare_every = (sp.rare_rearm_fwd_s as u64 * hz).max(1);
        // Every interval a standard round can be due on.
        let std_gate = trucks.iter().fold(std_every, |a, t| gcd(a, t.every));
        // A crate within crate_feed_m of the hull's edge (3 m from its
        // centre for these tanks).
        let feed = Fx::from_int(sp.crate_feed_m as i32 + 3);
        let reach = Reach::Vehicle { every: std_every };
        let ids: Vec<SimId> = self.vehicles.keys().copied().collect();
        for id in ids {
            let i = id.index();
            if ents.kind[i] != EntityKind::Vehicle {
                continue;
            }
            let now = tick + id.0 as u64;
            let std_due = now.is_multiple_of(std_gate);
            let rare_due = now.is_multiple_of(rare_every);
            let belts_due = sp.fwd_mg_rounds_s > 0 && now.is_multiple_of(hz);
            if !std_due && !rare_due && !belts_due {
                continue;
            }
            let owner = ents.owner[i];
            let at = ents.transform[i].pos.xy();
            if in_own_base(&self.setup, owner, at) {
                continue;
            }
            for rare in [false, true] {
                if (rare && !rare_due) || (!rare && !std_due) {
                    continue;
                }
                let vs = &self.vehicles[&id];
                let v = &self.setup.vehicles[vs.type_idx as usize];
                let Some(k) = crate::supply::pick_round(vs, v, rare) else {
                    continue;
                };
                let gun = v.main_gun().expect("pick_round found a round");
                let rack = v.rack_price_pct as u64;
                let Some(s) = self.setup.ammo[gun.ammo[k].ammo as usize].supply else {
                    continue;
                };
                let gun_faction = v.served().then_some(v.faction);
                let Some(src) =
                    self.source_for(owner, at, s.kind, s.g, feed, trucks, reach, gun_faction)
                else {
                    continue;
                };
                // The chosen source's interval: one source, one draw per
                // due tick, so rates never stack.
                let every = if rare {
                    rare_every
                } else {
                    match src {
                        Source::Truck(t, _) => trucks
                            .iter()
                            .find(|x| x.id == t)
                            .map_or(std_every, |x| x.every),
                        _ => std_every,
                    }
                };
                if !now.is_multiple_of(every) {
                    continue;
                }
                self.draw(src, s.kind, s.g);
                self.vehicles.get_mut(&id).expect("listed").ammo[k] += 1;
                // D4: the round was paid at the full shell price in the
                // crate; the base would have charged the rack factor.
                let credit =
                    micro_price(&self.setup, s.kind, s.g) * 100u64.saturating_sub(rack) / 100;
                let p = owner.0 as usize;
                let acc = &mut self.supply6.accounts[p];
                if rare {
                    acc.tank_rare_fwd += 1;
                } else {
                    acc.tank_rounds_fwd += 1;
                }
                match src {
                    Source::Crate(_) => acc.tank_crate_rounds += 1,
                    Source::Truck(..) => acc.truck_tank_rounds += 1,
                    Source::Structure(_) => {}
                }
                acc.rack_credit_micro += credit;
                let whole = acc.rack_credit_micro / MICRO;
                acc.rack_credit_micro %= MICRO;
                acc.rack_credit_mp += whole as u32;
                self.players[p].manpower = self.players[p].manpower.saturating_add(whole as u32);
            }
            if belts_due {
                self.rearm_belts(id, owner, at, sp.fwd_mg_rounds_s, feed, trucks, reach);
            }
        }
    }

    /// S8b (7.3): `budget` MG rounds into vehicle `id`'s stowage this
    /// second, mount by mount, each from one source (the only forward code
    /// that fills MG stowage).
    #[allow(clippy::too_many_arguments)]
    fn rearm_belts(
        &mut self,
        id: SimId,
        owner: PlayerId,
        at: FxVec2,
        budget: u32,
        feed: Fx,
        trucks: &[TruckSrc],
        reach: Reach,
    ) {
        let mut budget = budget;
        let nm = self.vehicles[&id].mgs.len();
        for k in 0..nm {
            let vs = &self.vehicles[&id];
            let spent = vs.mgs[k].spent;
            if spent == 0 || budget == 0 {
                continue;
            }
            let v = &self.setup.vehicles[vs.type_idx as usize];
            let w = &self.setup.weapons[v.mg_mounts()[k].weapon as usize];
            let Some((kd, g)) = w.feed.and_then(|f| feed_supply(&self.setup, f)) else {
                continue;
            };
            if g == 0 {
                continue;
            }
            let Some(src) = self.source_for(owner, at, kd, g, feed, trucks, reach, None) else {
                continue;
            };
            let n = budget.min(spent).min(self.stock_held(src, kd) / g);
            if n == 0 {
                continue;
            }
            self.draw(src, kd, n * g);
            self.vehicles.get_mut(&id).expect("listed").mgs[k].spent -= n;
            self.supply6.accounts[owner.0 as usize].fwd_belt_rounds += n;
            budget -= n;
        }
    }

    // ------------------------------------------------------------- trucks --

    /// S8b: the live supply trucks of this tick, ascending id.
    fn trucks_now(&self, ents: &Entities) -> Vec<TruckSrc> {
        let mut out = Vec::new();
        for (&id, vs) in &self.vehicles {
            let i = id.index();
            let v = &self.setup.vehicles[vs.type_idx as usize];
            let Some(t) = &v.supply else { continue };
            // A burning cargo serves nobody (it detonates or burns out).
            if ents.kind[i] != EntityKind::Vehicle || vs.ammo_fire > 0 {
                continue;
            }
            out.push(TruckSrc {
                id,
                owner: ents.owner[i],
                pos: ents.transform[i].pos.xy(),
                men: t.radius.into(),
                vehicles: t.vehicle_radius.into(),
                every: (t.round_interval_ticks as u64).max(1),
            });
        }
        out
    }

    /// S8b: trucks stopped in the own base take on their load, each kg paid
    /// at its kind's price as it loads (D1, no surcharge D2); shells share
    /// the player's crate queue.
    fn load_trucks(
        &mut self,
        tick: u64,
        sp: &frontline_content::tuning::SupplyTuning,
        trucks: &[TruckSrc],
    ) {
        let hz = self.setup.tick_hz as u64;
        for t in trucks {
            let vs = &self.vehicles[&t.id];
            if vs.base_ticks == 0 || vs.speed != Fx::ZERO {
                continue;
            }
            let d = self.setup.vehicles[vs.type_idx as usize]
                .supply
                .expect("listed truck");
            let p = t.owner.0 as usize;
            for kind in SupplyKind::ALL {
                let k = kind.index();
                let vs = &self.vehicles[&t.id];
                let held = vs.cargo_g[k];
                let mut room = d.load_g[k].min(d.cap_g).saturating_sub(held);
                // S10a: construction material shares the cap (cargo plus
                // material within `cap_g`); without material as before.
                if vs.material_g > 0 {
                    let all =
                        vs.cargo_g.iter().map(|&g| g as u64).sum::<u64>() + vs.material_g as u64;
                    room = room.min((d.cap_g as u64).saturating_sub(all) as u32);
                }
                if room == 0 {
                    continue;
                }
                let step = if kind == SupplyKind::Shells {
                    if tick < self.supply6.accounts[p].issue_at {
                        continue;
                    }
                    let Some(c) = d.crates[k] else { continue };
                    self.setup.stocks[c as usize].g
                } else {
                    if !(tick + t.id.0 as u64).is_multiple_of(hz) {
                        continue;
                    }
                    sp.truck_load_kg_s * 1000
                };
                let g = step.min(room);
                let micro = micro_price(&self.setup, kind, g);
                if !self.pay(p, kind, micro, tick) {
                    continue;
                }
                // `pay` counts a round refilled; a load is not one.
                let r = &mut self.players[p].ammo.refilled[k];
                *r = r.saturating_sub(1);
                self.vehicles.get_mut(&t.id).expect("listed").cargo_g[k] += g;
                let acc = &mut self.supply6.accounts[p];
                acc.truck_loaded_g[k] += g;
                acc.truck_load_micro += micro;
                if kind == SupplyKind::Shells {
                    acc.issue_at = tick + sp.base_crate_issue_s as u64 * hz;
                }
            }
        }
    }

    /// S8b `DropCrates`: own live truck `vehicle` goes to `point` and sets
    /// down `count` crates of `kind`.
    pub(crate) fn order_drop_crates(
        &mut self,
        ctx: &mut Ctx,
        p: PlayerId,
        vehicle: SimId,
        point: FxVec2,
        count: u8,
        kind: SupplyKind,
    ) -> bool {
        let Some(sp) = self.setup.rules.supply.clone() else {
            return false;
        };
        let i = vehicle.index();
        if ctx.ents.kind.get(i) != Some(&EntityKind::Vehicle) || ctx.ents.owner[i] != p {
            return false;
        }
        let Some(squad) = ctx.ents.squad[i] else {
            return false;
        };
        let Some(vs) = self.vehicles.get(&vehicle) else {
            return false;
        };
        let Some(d) = self.setup.vehicles[vs.type_idx as usize].supply else {
            return self.refuse6(p, squad, SupplyRefusal::NotAllowed);
        };
        if !(1..=MAX_DROP).contains(&count) {
            return self.refuse6(p, squad, SupplyRefusal::NotAllowed);
        }
        // S10a: trucks drop ammunition crates only.
        if !kind.is_ammo() {
            return self.refuse6(p, squad, SupplyRefusal::NotAllowed);
        }
        let Some(def) = d.crates[kind.index()] else {
            return self.refuse6(p, squad, SupplyRefusal::NotAllowed);
        };
        let size = Fx::from_int(self.setup.map.size_m);
        if [point.x, point.y]
            .iter()
            .any(|v| *v < Fx::ZERO || *v > size)
        {
            return self.refuse6(p, squad, SupplyRefusal::BadSpot);
        }
        if vs.cargo_g[kind.index()] < self.setup.stocks[def as usize].g {
            return self.refuse6(p, squad, SupplyRefusal::NoCrate);
        }
        let (n, shells) = self.supply6.crates_of(&self.setup, p);
        if n >= sp.crate_cap || (kind == SupplyKind::Shells && shells >= sp.shell_crate_cap) {
            return self.refuse6(p, squad, SupplyRefusal::Cap);
        }
        // Already at the point: stop here; else the ordinary vehicle path.
        // A goal it cannot reach ends like an arrival.
        let at = ctx.ents.transform[i].pos.xy();
        let moved = if within(at, point, Fx::from_int(sp.drop_reach_m as i32)) {
            self.order_stop(ctx, p, squad)
        } else {
            self.order_move(ctx, p, squad, Order::Move { target: point })
        };
        if moved && let Some(vs) = self.vehicles.get_mut(&vehicle) {
            vs.drop = Some(crate::vehicle::CrateDrop {
                point,
                count,
                kind,
                next_tick: ctx.tick,
            });
        }
        moved
    }

    /// S8b: trucks with a drop order: once at the point (or stopped short of
    /// one they cannot reach) one crate per `drop_s` behind the tailgate.
    fn drop_crates(
        &mut self,
        ents: &Entities,
        terrain: &Heightfield,
        tick: u64,
        sp: &frontline_content::tuning::SupplyTuning,
        trucks: &[TruckSrc],
    ) {
        let hz = self.setup.tick_hz as u64;
        let reach = Fx::from_int(sp.drop_reach_m as i32);
        for t in trucks {
            let vs = &self.vehicles[&t.id];
            let Some(d) = vs.drop else { continue };
            let i = t.id.index();
            let Some(squad) = ents.squad[i] else {
                self.vehicles.get_mut(&t.id).expect("listed").drop = None;
                continue;
            };
            let near = within(t.pos, d.point, reach);
            let order = &self.squads[&squad].order;
            let driving = matches!(order, Order::Move { target } if *target == d.point);
            if !driving && (*order != Order::Idle || vs.face.is_some()) {
                // Another order took over.
                self.vehicles.get_mut(&t.id).expect("listed").drop = None;
                continue;
            }
            if driving {
                if !near {
                    continue;
                }
                // At the point: stop here.
                let sq = self.squads.get_mut(&squad).expect("listed");
                sq.order = Order::Idle;
                crate::vehicle::hold_vehicle(&mut sq.nav, t.pos);
                self.vehicles.get_mut(&t.id).expect("listed").reverse = false;
            }
            let vs = &self.vehicles[&t.id];
            if vs.speed != Fx::ZERO || tick < d.next_tick {
                continue;
            }
            let v = &self.setup.vehicles[vs.type_idx as usize];
            let Some(def) = v.supply.and_then(|s| s.crates[d.kind.index()]) else {
                self.vehicles.get_mut(&t.id).expect("listed").drop = None;
                continue;
            };
            let k = d.kind.index();
            let st = &self.setup.stocks[def as usize];
            let (g, hp) = (st.g, st.boxes.first().map_or(1, |b| b.hp));
            let (n, shells) = self.supply6.crates_of(&self.setup, t.owner);
            if vs.cargo_g[k] < g
                || n >= sp.crate_cap
                || (d.kind == SupplyKind::Shells && shells >= sp.shell_crate_cap)
                || self.supply6.crates.len() >= MAX_CRATES
            {
                self.vehicles.get_mut(&t.id).expect("listed").drop = None;
                continue;
            }
            // Behind the tailgate, in three lanes so crates do not stack.
            let f = dir(ents.transform[i].yaw);
            let back = Fx::from(v.hull.half.x) + Fx::from_int(2);
            let lane = Fx::from_int(d.count as i32 % 3 - 1) * Fx::from_ratio(6, 5);
            let side = FxVec2::new(-f.y, f.x);
            let pos = terrain.clamp(t.pos - f * back + side * lane);
            let id = self.supply6.next_crate;
            self.supply6.next_crate += 1;
            self.supply6.crates.insert(
                id,
                Crate {
                    owner: t.owner,
                    def,
                    pos,
                    carrier: None,
                    dest: None,
                    claim: None,
                    g,
                    hp,
                },
            );
            let acc = &mut self.supply6.accounts[t.owner.0 as usize];
            acc.truck_crates_dropped += 1;
            acc.truck_dropped_g += g as u64;
            let vs = self.vehicles.get_mut(&t.id).expect("listed");
            vs.cargo_g[k] -= g;
            vs.drop = (d.count > 1).then(|| crate::vehicle::CrateDrop {
                count: d.count - 1,
                next_tick: tick + sp.drop_s as u64 * hz,
                ..d
            });
        }
    }

    /// S8b: a truck's cargo when it is wrecked: lost, and after a
    /// detonation or an ammo fire it bursts (the S6 cook-off) when it holds
    /// at least `cargo_detonate_min_kg` of shells and explosives.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn truck_wrecked(
        &mut self,
        id: SimId,
        owner: usize,
        tick: u64,
        pos: FxVec3,
        cook: bool,
        by: u8,
        class: u16,
    ) {
        let Some(vs) = self.vehicles.get_mut(&id) else {
            return;
        };
        let Some(t) = self.setup.vehicles[vs.type_idx as usize].supply else {
            return;
        };
        let cargo = std::mem::take(&mut vs.cargo_g);
        vs.drop = None;
        let held: u64 = cargo.iter().map(|&g| g as u64).sum();
        if let Some(acc) = self.supply6.accounts.get_mut(owner) {
            acc.truck_lost_g += held;
        }
        let Some(sp) = self.setup.rules.supply.clone() else {
            return;
        };
        let kg = (cargo[SupplyKind::Shells.index()] + cargo[SupplyKind::Explosives.index()]) / 1000;
        if cook && kg >= sp.cargo_detonate_min_kg && kg > 0 {
            if let Some(acc) = self.supply6.accounts.get_mut(owner) {
                acc.cargo_detonations += 1;
            }
            self.start_cookoff(&sp, tick, pos, by, class, t.cookoff, kg, owner);
        }
    }

    // -------------------------------------------------------------- boxes --

    /// Rebuilds this tick's static boxes (before the Ballistics stage).
    pub(crate) fn rebuild_statics(&mut self, terrain: &Heightfield) {
        self.statics.0.clear();
        if self.setup.rules.supply.is_none() {
            return;
        }
        for (&id, s) in &self.supply6.structures {
            if !s.built {
                continue;
            }
            let d = &self.setup.stocks[s.def as usize];
            let f = dir(s.yaw);
            let left = f.perp();
            for (k, b) in d.boxes.iter().enumerate() {
                if s.box_hp[k] == 0 {
                    continue;
                }
                let c =
                    terrain.clamp(s.pos + f * Fx::from(b.offset[0]) + left * Fx::from(b.offset[1]));
                self.statics.0.push(static_box(
                    terrain,
                    s.owner.0,
                    StaticRef::Structure(id, k as u8),
                    c,
                    s.yaw,
                    b,
                ));
            }
        }
        for (&id, c) in &self.supply6.crates {
            if c.carrier.is_some() {
                continue;
            }
            let d = &self.setup.stocks[c.def as usize];
            if let Some(b) = d.boxes.first() {
                self.statics.0.push(static_box(
                    terrain,
                    c.owner.0,
                    StaticRef::Crate(id),
                    c.pos,
                    Fx::ZERO,
                    b,
                ));
            }
        }
        // S10a: pieces after the S6 boxes and crates (S6 indices and ray
        // ties unchanged).
        if self.fort_off {
            return;
        }
        for (&id, q) in &self.fort.pieces {
            let b = &self.setup.forts[q.fort as usize].pieces[q.part as usize];
            let half = FxVec3::new(b.half[0].into(), b.half[1].into(), b.half[2].into());
            self.statics.0.push(StaticBox {
                owner: q.owner.0,
                of: StaticRef::Piece(id),
                frame: BoxFrame {
                    center: FxVec3::new(q.pos.x, q.pos.y, q.z + half.z),
                    axes: Basis::hull(q.yaw, Fx::ZERO, Fx::ZERO),
                    half,
                },
                reach: half.length(),
                armour_mm: b.armour_mm,
            });
        }
    }

    /// Takes `dmg` hit points from static box `idx` (this tick's list).
    /// `he`: the player and kill class of the HE that hit it (a cook-off
    /// when it breaks a box holding shells or explosives); `None`: a
    /// bullet or a solid shot.
    pub(crate) fn hit_static(&mut self, idx: usize, dmg: u32, he: Option<(u8, u16)>, tick: u64) {
        let Some(b) = self.statics.0.get(idx).copied() else {
            return;
        };
        if dmg == 0 {
            return;
        }
        let sp = self
            .setup
            .rules
            .supply
            .clone()
            .expect("statics need [supply]");
        // (def, kg of shells + explosives, owner) of what broke.
        let broke: Option<(u16, u32, PlayerId)> = match b.of {
            StaticRef::Piece(id) => {
                let Some(q) = self.fort.pieces.get_mut(&id) else {
                    return;
                };
                q.hp = q.hp.saturating_sub(dmg);
                if q.hp > 0 {
                    return;
                }
                let q = self.fort.pieces.remove(&id).expect("listed");
                self.fort.accounts[q.owner.0 as usize].pieces_lost += 1;
                None
            }
            StaticRef::Crate(id) => {
                let Some(c) = self.supply6.crates.get_mut(&id) else {
                    return;
                };
                c.hp = c.hp.saturating_sub(dmg);
                if c.hp > 0 {
                    return;
                }
                let c = self.supply6.crates.remove(&id).expect("listed");
                self.supply6.accounts[c.owner.0 as usize].crates_lost += 1;
                let kind = self.setup.stocks[c.def as usize].supply;
                let kg = if matches!(kind, Some(SupplyKind::Shells | SupplyKind::Explosives)) {
                    c.g / 1000
                } else {
                    0
                };
                Some((c.def, kg, c.owner))
            }
            StaticRef::Structure(id, k) => {
                let Some(s) = self.supply6.structures.get_mut(&id) else {
                    return;
                };
                let k = k as usize;
                if s.box_hp[k] == 0 {
                    return;
                }
                s.box_hp[k] = s.box_hp[k].saturating_sub(dmg);
                if s.box_hp[k] > 0 {
                    return;
                }
                let d = &self.setup.stocks[s.def as usize];
                match d.boxes.get(k).map(|x| x.part) {
                    Some(1) => {
                        s.radio = false;
                        None
                    }
                    _ => {
                        // The stock box: the stock is lost and the
                        // structure with it (S6-10 (A)). S12 W8: the
                        // aid post's tent likewise (no stock: no
                        // cook-off); men in its beds stay where they lie.
                        let kg = (s.stock_g[SupplyKind::Shells.index()]
                            + s.stock_g[SupplyKind::Explosives.index()])
                            / 1000;
                        let (def, owner, is_hq) = (s.def, s.owner, d.kind == StockKind::Hq);
                        let held: u64 = s.stock_g.iter().map(|&x| x as u64).sum();
                        self.supply6.structures.remove(&id);
                        let acc = &mut self.supply6.accounts[owner.0 as usize];
                        acc.structures_lost += 1;
                        acc.stock_lost_g += held;
                        if is_hq {
                            acc.hq_lost_at = tick.max(1);
                        }
                        Some((def, kg, owner))
                    }
                }
            }
        };
        // Mark the derived box broken for the rest of this tick.
        if let Some(sb) = self.statics.0.get_mut(idx) {
            sb.reach = Fx::from_int(-1);
        }
        let (Some((def, kg, owner)), Some((player, class))) = (broke, he) else {
            return;
        };
        let Some(ammo) = self.setup.stocks[def as usize].cookoff else {
            return;
        };
        self.start_cookoff(
            &sp,
            tick,
            b.frame.center,
            player,
            class,
            ammo,
            kg,
            owner.0 as usize,
        );
    }

    /// Starts a cook-off of `kg` at `pos` (S6; S8b: a truck's cargo).
    #[allow(clippy::too_many_arguments)]
    fn start_cookoff(
        &mut self,
        sp: &frontline_content::tuning::SupplyTuning,
        tick: u64,
        pos: FxVec3,
        player: u8,
        class: u16,
        ammo: u16,
        kg: u32,
        owner: usize,
    ) {
        let hz = self.setup.tick_hz;
        let n = (kg / sp.cookoff_kg_per_burst.max(1)).min(sp.cookoff_max);
        if n == 0 || self.supply6.cookoffs.len() >= MAX_COOKOFFS {
            return;
        }
        let every = (sp.cookoff_s * hz / n).max(1);
        let id = self.supply6.next_cookoff;
        self.supply6.next_cookoff += 1;
        self.supply6.cookoffs.push(CookOff {
            id,
            pos,
            player,
            class,
            ammo,
            left: n as u8,
            next_tick: tick + 1,
            every,
        });
        self.supply6.accounts[owner].cookoffs += 1;
    }

    /// Cook-off bursts due at `tick`: `(id, pos, player, class, ammo)`, in
    /// id order; finished cook-offs are dropped.
    pub(crate) fn due_cookoffs(&mut self, tick: u64) -> Vec<(u32, FxVec3, u8, u16, u16)> {
        let mut out = Vec::new();
        for k in self.supply6.cookoffs.iter_mut() {
            if k.next_tick <= tick && k.left > 0 {
                out.push((k.id, k.pos, k.player, k.class, k.ammo));
                k.left -= 1;
                k.next_tick = tick + k.every as u64;
            }
        }
        self.supply6.cookoffs.retain(|k| k.left > 0);
        out
    }
}

/// Where a mortar squad's tube stands for crate and stock feeding: the man
/// laying it (his kit weapon is the squad's), else the first member. The
/// AI sets mortar crates down at this man (review S6 F2).
pub(crate) fn mortar_gunner(
    ty: &frontline_content::setup::SquadTypeSetup,
    ents: &Entities,
    members: &[SimId],
) -> Option<SimId> {
    members
        .iter()
        .copied()
        .find(|m| ents.kit[m.index()].weapon == ty.weapon)
        .or_else(|| members.first().copied())
}

/// One static box of def box `b` centred over `c` at yaw `yaw`.
fn static_box(
    terrain: &Heightfield,
    owner: u8,
    of: StaticRef,
    c: FxVec2,
    yaw: Fx,
    b: &frontline_content::setup::StockBoxSetup,
) -> StaticBox {
    let half = FxVec3::new(b.half[0].into(), b.half[1].into(), b.half[2].into());
    let z = terrain.height_at(c) + Fx::from(b.offset[2]) + half.z;
    StaticBox {
        owner,
        of,
        frame: BoxFrame {
            center: FxVec3::new(c.x, c.y, z),
            axes: Basis::hull(yaw, Fx::ZERO, Fx::ZERO),
            half,
        },
        reach: half.length(),
        armour_mm: 0,
    }
}

/// The first live static box not owned by `player` that the segment
/// `from → from + d` enters: `(t, index)`, earliest first, ties by index.
pub(crate) fn first_static(
    statics: &[StaticBox],
    from: FxVec3,
    d: FxVec3,
    player: u8,
) -> Option<(Fx, usize)> {
    let mut best: Option<(Fx, usize)> = None;
    for (k, b) in statics.iter().enumerate() {
        if b.owner == player || b.reach < Fx::ZERO {
            continue;
        }
        if !crate::armour::segment_near(from, d, b.frame.center, b.reach) {
            continue;
        }
        if let Some((t, _, _)) = crate::armour::segment_box(&b.frame, from, d)
            && best.is_none_or(|(bt, _)| t < bt)
        {
            best = Some((t, k));
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_static_skips_own_and_broken_boxes() {
        let b = |owner: u8, x: i32, reach: i32| StaticBox {
            owner,
            of: StaticRef::Crate(x as u32),
            frame: BoxFrame {
                center: FxVec3::new(Fx::from_int(x), Fx::ZERO, Fx::ONE),
                axes: Basis::hull(Fx::ZERO, Fx::ZERO, Fx::ZERO),
                half: FxVec3::new(Fx::ONE, Fx::ONE, Fx::ONE),
            },
            reach: Fx::from_int(reach),
            armour_mm: 0,
        };
        let from = FxVec3::new(Fx::ZERO, Fx::ZERO, Fx::ONE);
        let d = FxVec3::new(Fx::from_int(20), Fx::ZERO, Fx::ZERO);
        let list = [b(0, 5, 2), b(1, 10, 2), b(1, 8, -1)];
        // Player 0's own box at 5 m is skipped; the broken one at 8 too.
        let (t, k) = first_static(&list, from, d, 0).expect("hit");
        assert_eq!(k, 1);
        assert_eq!(t, Fx::from_ratio(9, 20));
        // Player 1 hits player 0's box first.
        assert_eq!(first_static(&list, from, d, 1).map(|x| x.1), Some(0));
    }
}
