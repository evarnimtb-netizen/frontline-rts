//! S5a ammunition: counted rounds, sharing inside a squad and the priced
//! base refill (docs/slices/S5a-ammo.md).
//!
//! * **Counters (A1).** Every round a weapon requests this tick is counted
//!   by class in [`AmmoAccount::fired`] when the requests are merged
//!   (Weapons stage, serial, in request order). A request the projectile
//!   pool later drops is still counted: the round was spent when it was
//!   requested. Any future burst bundle (one request standing for several
//!   rounds) must add every round it stands for.
//! * **Payment.** Prices are in micro-manpower (µMP): a round of `g` grams
//!   of a supply kind priced at `mp_per_t` manpower per tonne costs
//!   `g × mp_per_t` µMP. [`AmmoAccount::credit`] holds prepaid manpower
//!   below one point plus one round; a round is paid from it, else whole
//!   manpower moves into it one point at a time if the player has it,
//!   else the refill pauses ([`MatchState::pay`]). Nothing goes into debt.
//!
//! The rest (counted reloads, crew feed, sidearm, sharing and the base
//! source) is in this module too; legacy content without a `[supply]`
//! table (the frozen S3c fixture, S5a-22) keeps unlimited rounds and free
//! refills, so its behaviour and hashes stay those of S4.

use frontline_content::setup::{Feed, MAX_FEEDS, SquadTypeSetup, SupplyKind, member_trait};
use frontline_content::{AmmoMechanism, ITEM_KINDS, MatchSetup, WeaponClass};
use serde::{Deserialize, Serialize};

use crate::combat::pct_of;
use crate::ecs::{Combat, Entities, EntityKind, PlayerId, SimId, Weapon};
use crate::fx::{Fx, FxVec2};
use crate::game::{MatchState, SquadId};
use crate::hash::HashCache;
use crate::items::Kit;
use crate::systems::FireRequest;

/// Report classes of rounds fired ([`AmmoAccount::fired`]).
pub mod fired_class {
    pub const RIFLE: usize = 0;
    pub const SMG: usize = 1;
    pub const MG: usize = 2;
    pub const PISTOL: usize = 3;
    pub const MORTAR: usize = 4;
    /// An AT team's rocket.
    pub const ROCKET: usize = 5;
    /// A single-shot AT item (Panzerfaust).
    pub const PANZERFAUST: usize = 6;
    /// Main-gun rounds: penetrators, HE and rare rounds (APCR, HVAP).
    pub const TANK_AP: usize = 7;
    pub const TANK_HE: usize = 8;
    pub const TANK_RARE: usize = 9;
    /// Coax and hull MGs.
    pub const VEHICLE_MG: usize = 10;
    pub const COUNT: usize = 11;
    /// Report names, by class.
    pub const NAMES: [&str; COUNT] = [
        "rifle",
        "smg",
        "mg",
        "pistol",
        "mortar",
        "rocket",
        "panzerfaust",
        "tank_ap",
        "tank_he",
        "tank_rare",
        "vehicle_mg",
    ];
}

/// Supply kinds priced in S5a (`SupplyKind::index`).
pub const SUPPLY_KINDS: usize = 3;

/// µMP in one manpower point.
pub const MICRO: u64 = 1_000_000;

/// S5a per-player ammunition account (saved, hashed, validated on load).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct AmmoAccount {
    /// Prepaid µMP (below one manpower point plus the dearest round).
    pub credit: u32,
    /// µMP paid for ammunition, by supply kind (report, HUD).
    pub paid: [u64; SUPPLY_KINDS],
    /// Ticks on which some refill of this player paused for lack of
    /// manpower.
    pub paused_ticks: u32,
    /// The last tick a refill paused (HUD flag "No manpower for ammo";
    /// 0: never).
    pub paused_at: u64,
    /// Rounds fired, by [`fired_class`] (report).
    pub fired: [u32; fired_class::COUNT],
    /// Rounds, items and bombs refilled, by supply kind (report).
    pub refilled: [u32; SUPPLY_KINDS],
}

/// The report class of one fire request (`None`: not counted here).
pub(crate) fn class_of(setup: &MatchSetup, ents: &Entities, r: &FireRequest) -> Option<usize> {
    use fired_class as f;
    let w = setup.weapons.get(r.weapon as usize)?;
    if let Some(a) = setup.ammo.get(r.ammo as usize) {
        if w.class == WeaponClass::Launcher {
            let item = setup.items.iter().find(|it| it.weapon == Some(r.weapon));
            // S11b (review A-3): a rifle grenade (an item launcher with an
            // HE round) is no Panzerfaust. It has no class here (a new one
            // would change the saved `fired` array, SAVE 29 is fixed): it
            // is counted in `s11b.rg_fired`.
            if item.is_some_and(|it| it.kind == frontline_content::ItemKind::RifleGrenade) {
                return None;
            }
            return Some(if item.is_some() {
                f::PANZERFAUST
            } else {
                f::ROCKET
            });
        }
        return Some(if a.rare {
            f::TANK_RARE
        } else if a.mechanism == AmmoMechanism::He {
            f::TANK_HE
        } else {
            f::TANK_AP
        });
    }
    let vehicle = matches!(
        ents.kind.get(r.shooter.index()),
        Some(EntityKind::Vehicle | EntityKind::Wreck | EntityKind::Abandoned)
    );
    Some(match w.class {
        WeaponClass::Rifle => f::RIFLE,
        WeaponClass::Smg => f::SMG,
        WeaponClass::Pistol => f::PISTOL,
        WeaponClass::Mg if vehicle => f::VEHICLE_MG,
        WeaponClass::Mg => f::MG,
        WeaponClass::Mortar => f::MORTAR,
        WeaponClass::Launcher => f::ROCKET,
        WeaponClass::Cannon => f::TANK_AP,
    })
}

/// µMP of one round of `g` grams at the rules' price for `kind`.
pub(crate) fn micro_price(setup: &MatchSetup, kind: SupplyKind, g: u32) -> u64 {
    let Some(s) = &setup.rules.supply else {
        return 0;
    };
    let per_t = match kind {
        SupplyKind::SmallArms => s.small_arms_mp_per_t,
        SupplyKind::Explosives => s.explosives_mp_per_t,
        SupplyKind::Shells => s.shell_mp_per_t,
        // S10a: priced by `[fortify] construction_mp_per_t`.
        SupplyKind::Construction => setup
            .rules
            .fortify
            .as_ref()
            .map_or(0, |f| f.construction_mp_per_t),
    };
    g as u64 * per_t as u64
}

/// The supply (kind, grams) of a feed.
pub(crate) fn feed_supply(setup: &MatchSetup, f: Feed) -> Option<(SupplyKind, u32)> {
    match f {
        Feed::Cartridge(c) => setup.cartridges.get(c as usize).map(|c| (c.kind, c.g)),
        Feed::Ammo(a) => setup
            .ammo
            .get(a as usize)
            .and_then(|a| a.supply)
            .map(|s| (s.kind, s.g)),
    }
}

/// The dearest single round, item or bomb the base can hand out (µMP):
/// the bound of [`AmmoAccount::credit`] on load.
pub(crate) fn dearest_round(setup: &MatchSetup) -> u64 {
    let mut best = 0;
    for c in &setup.cartridges {
        best = best.max(micro_price(setup, c.kind, c.g));
    }
    for a in &setup.ammo {
        if let Some(s) = a.supply {
            best = best.max(micro_price(setup, s.kind, s.g));
        }
    }
    for it in &setup.items {
        if let Some(s) = it.supply {
            best = best.max(micro_price(setup, s.kind, s.g));
        }
    }
    best
}

impl MatchState {
    /// A1 counters: every request of this tick by class and player.
    pub(crate) fn count_fired(&mut self, ents: &Entities, reqs: &[FireRequest]) {
        for r in reqs {
            if let Some(c) = class_of(&self.setup, ents, r)
                && let Some(p) = self.players.get_mut(r.player as usize)
            {
                p.ammo.fired[c] = p.ammo.fired[c].saturating_add(1);
            }
        }
    }

    /// Pays one round of `micro` µMP of `kind` for player `p` (module
    /// docs): from the credit, else whole manpower into the credit, else
    /// false (the refill pauses this tick; nothing changes but the pause
    /// counters).
    pub(crate) fn pay(&mut self, p: usize, kind: SupplyKind, micro: u64, tick: u64) -> bool {
        let Some(pl) = self.players.get_mut(p) else {
            return false;
        };
        let credit = pl.ammo.credit as u64;
        if credit < micro {
            let k = (micro - credit).div_ceil(MICRO);
            if (pl.manpower as u64) < k {
                if pl.ammo.paused_at != tick {
                    pl.ammo.paused_ticks = pl.ammo.paused_ticks.saturating_add(1);
                    pl.ammo.paused_at = tick;
                }
                return false;
            }
            pl.manpower -= k as u32;
            pl.ammo.credit = (credit + k * MICRO - micro) as u32;
        } else {
            pl.ammo.credit = (credit - micro) as u32;
        }
        let i = kind.index();
        pl.ammo.paid[i] = pl.ammo.paid[i].saturating_add(micro);
        pl.ammo.refilled[i] = pl.ammo.refilled[i].saturating_add(1);
        true
    }
}

// ------------------------------------------------------- counted rounds --

/// The weapon man `kit` fights with now: his slot's sidearm while
/// [`Kit::on_sidearm`], else the weapon he carries.
pub fn active_weapon(ty: &SquadTypeSetup, kit: &Kit) -> u16 {
    if kit.on_sidearm
        && let Some(sa) = ty.members.get(kit.slot as usize).and_then(|m| m.sidearm)
    {
        return sa;
    }
    kit.weapon
}

/// Whether this squad type counts rounds (a priced mod: its men have
/// feeds).
#[inline]
pub(crate) fn counted(ty: &SquadTypeSetup) -> bool {
    !ty.feeds.is_empty()
}

/// Rounds of feed slot `k` man `kit` holds: his pouches plus his active
/// weapon's magazine when it uses that feed.
pub fn man_total(setup: &MatchSetup, ty: &SquadTypeSetup, kit: &Kit, c: &Combat, k: usize) -> u32 {
    let w = &setup.weapons[active_weapon(ty, kit) as usize];
    let in_gun = ty.feed_slot(w) == Some(k) && w.he.is_none();
    kit.rounds[k] as u32 + if in_gun { c.mag as u32 } else { 0 }
}

/// His slot's full load of feed slot `k` (0 when his slot carries none).
#[inline]
pub fn man_load(ty: &SquadTypeSetup, kit: &Kit, k: usize) -> u32 {
    ty.members
        .get(kit.slot as usize)
        .map_or(0, |m| m.rounds[k] as u32)
}

/// Fills a fresh man's weapon and pouches from his slot's load (spawns):
/// the weapon takes a magazine from the load, the rest goes in his kit.
/// Legacy types keep the full magazine and no carried rounds.
pub(crate) fn spawn_load(setup: &MatchSetup, ty: &SquadTypeSetup, kit: &mut Kit, c: &mut Combat) {
    if !counted(ty) {
        return;
    }
    let Some(m) = ty.members.get(kit.slot as usize) else {
        return;
    };
    kit.rounds = m.rounds;
    let w = &setup.weapons[kit.weapon as usize];
    if w.he.is_some() {
        // Mortars draw bombs from the pouches as they fire (none sits in
        // the tube).
        c.mag = 0;
        return;
    }
    if w.class == WeaponClass::Launcher {
        // Launchers fire the team's rockets.
        return;
    }
    if let Some(k) = ty.feed_slot(w) {
        let take = (w.magazine.min(u16::MAX as u32) as u16).min(kit.rounds[k]);
        kit.rounds[k] -= take;
        c.mag = take;
    }
}

/// A dry man (magazine empty, cooldown done): reloads his weapon from his
/// pouches, switches between his main weapon and his slot's sidearm (S5a
/// "Sidearm"), or stays dry. True when he reloaded or switched (the
/// cooldown is set to the reload).
pub(crate) fn reload_dry(
    setup: &MatchSetup,
    ty: &SquadTypeSetup,
    kit: &mut Kit,
    c: &mut Combat,
    ws: &mut Weapon,
) -> bool {
    let slot = ty.members.get(kit.slot as usize);
    let main = &setup.weapons[kit.weapon as usize];
    let side = slot
        .and_then(|m| m.sidearm)
        .map(|w| (w, &setup.weapons[w as usize]));
    let main_k = ty.feed_slot(main);
    let has = |kit: &Kit, k: Option<usize>| k.is_some_and(|k| kit.rounds[k] > 0);
    let load = |kit: &mut Kit,
                c: &mut Combat,
                ws: &mut Weapon,
                w: &frontline_content::setup::WeaponSetup,
                k: usize| {
        let take = (w.magazine.min(u16::MAX as u32) as u16).min(kit.rounds[k]);
        kit.rounds[k] -= take;
        c.mag = take;
        ws.cooldown = w.reload_ticks.max(1).min(u16::MAX as u32) as u16;
    };
    if kit.on_sidearm {
        // S5a-23: back to the main weapon at the next reload once it has
        // rounds again.
        if let Some(k) = main_k
            && has(kit, main_k)
        {
            kit.on_sidearm = false;
            load(kit, c, ws, main, k);
            return true;
        }
        if let Some((_, sw)) = side
            && let Some(k) = ty.feed_slot(sw)
            && kit.rounds[k] > 0
        {
            load(kit, c, ws, sw, k);
            return true;
        }
        return false;
    }
    if let Some(k) = main_k
        && kit.rounds[k] > 0
    {
        load(kit, c, ws, main, k);
        return true;
    }
    if let Some((_, sw)) = side
        && let Some(k) = ty.feed_slot(sw)
        && kit.rounds[k] > 0
        && sw.he.is_none()
        && sw.class != WeaponClass::Launcher
    {
        kit.on_sidearm = true;
        load(kit, c, ws, sw, k);
        return true;
    }
    false
}

/// S5a squad ammunition share (%, the AI's reserve and the HUD marker):
/// the lowest share of its main feeds, each the rounds its living men hold
/// over their slots' loads. The main feeds are its crew weapons' (MG,
/// mortar) when a man carries one, else the feed most of its men's main
/// weapons use (ties: the lower slot); an AT team's rockets count too.
/// `None` for squads that do not count rounds (legacy, vehicles).
pub fn squad_ammo_pct(setup: &MatchSetup, ents: &Entities, sq: &crate::game::Squad) -> Option<u32> {
    let ty = &setup.squad_types[sq.type_idx as usize];
    if !counted(ty) || ty.vehicle.is_some() {
        return None;
    }
    let mut crew = [false; MAX_FEEDS];
    let mut votes = [0u32; MAX_FEEDS];
    for m in &sq.members {
        let kit = &ents.kit[m.index()];
        let w = &setup.weapons[kit.weapon as usize];
        if w.class == WeaponClass::Launcher {
            continue;
        }
        if let Some(k) = ty.feed_slot(w) {
            votes[k] += 1;
            crew[k] |= w.crew;
        }
    }
    let main: Vec<usize> = if crew.iter().any(|&c| c) {
        (0..MAX_FEEDS).filter(|&k| crew[k]).collect()
    } else {
        let best = (0..MAX_FEEDS).max_by_key(|&k| (votes[k], std::cmp::Reverse(k)));
        best.filter(|&k| votes[k] > 0).into_iter().collect()
    };
    let mut pct: Option<u32> = None;
    for k in main {
        let (mut have, mut load) = (0u64, 0u64);
        for m in &sq.members {
            let i = m.index();
            have += man_total(setup, ty, &ents.kit[i], &ents.combat[i], k) as u64;
            load += man_load(ty, &ents.kit[i], k) as u64;
        }
        if let Some(p) = (have * 100).checked_div(load) {
            let p = p.min(100) as u32;
            pct = Some(pct.map_or(p, |q| q.min(p)));
        }
    }
    if let Some(l) = &sq.launcher {
        let full = setup.weapons[ty.weapon as usize].rounds_carried.max(1) as u64;
        let p = (l.rounds as u64 * 100 / full).min(100) as u32;
        pct = Some(pct.map_or(p, |q| q.min(p)));
    }
    pct
}

/// S5a (HUD): a squad's crew weapon (index into `MatchSetup::weapons`)
/// and the rounds its living men hold for it (gun, pouches, the
/// assistant's and bearers' belts), if a man carries one and it counts
/// rounds (launchers: their team's rockets).
pub fn crew_rounds(
    setup: &MatchSetup,
    ents: &Entities,
    sq: &crate::game::Squad,
) -> Option<(u16, u32)> {
    let ty = &setup.squad_types[sq.type_idx as usize];
    let wi = sq
        .members
        .iter()
        .map(|m| ents.kit[m.index()].weapon)
        .find(|&w| setup.weapons[w as usize].crew)?;
    let w = &setup.weapons[wi as usize];
    if w.class == WeaponClass::Launcher {
        return sq.launcher.as_ref().map(|l| (wi, l.rounds as u32));
    }
    let k = ty.feed_slot(w)?;
    let mut n = 0u32;
    for m in &sq.members {
        let i = m.index();
        n += man_total(setup, ty, &ents.kit[i], &ents.combat[i], k);
        if ents.kit[i].weapon == wi && w.he.is_some() {
            n += ents.combat[i].mag as u32;
        }
    }
    Some((wi, n))
}

/// S5a: mortar bombs a mortar squad holds (its mortar's feed, every
/// living man) and their share of the squad's load (%; `None` for legacy
/// types, which do not count bombs: then `held` is `u32::MAX`).
pub(crate) fn mortar_bombs(
    setup: &MatchSetup,
    ents: &Entities,
    sq: &crate::game::Squad,
) -> (u32, Option<u32>) {
    let ty = &setup.squad_types[sq.type_idx as usize];
    let Some(k) = ty.feed_slot(&setup.weapons[ty.weapon as usize]) else {
        return (u32::MAX, None);
    };
    let (mut have, mut load) = (0u32, 0u32);
    for m in &sq.members {
        let i = m.index();
        have += man_total(setup, ty, &ents.kit[i], &ents.combat[i], k);
        // The tube's ready bomb, only while the mortar is the weapon in
        // hand (S6 fix: a gunner on his sidearm holds pistol rounds in
        // `mag`, which counted as bombs).
        have += if active_weapon(ty, &ents.kit[i]) == ty.weapon
            && setup.weapons[ty.weapon as usize].he.is_some()
        {
            ents.combat[i].mag as u32
        } else {
            0
        };
        load += man_load(ty, &ents.kit[i], k);
    }
    (
        have,
        (load > 0).then(|| (have as u64 * 100 / load as u64).min(100) as u32),
    )
}

/// Whether `p` stands inside player `owner`'s base.
pub(crate) fn in_own_base(setup: &MatchSetup, owner: PlayerId, p: FxVec2) -> bool {
    let base = &setup.map.bases[setup.players[owner.0 as usize].base as usize];
    let r: Fx = base.radius.into();
    (p - FxVec2::from(base.center)).length_sq_raw() <= FxVec2::new(r, Fx::ZERO).length_sq_raw()
}

/// Priority of a squadmate as a giver of rounds: ammo bearers, then crew
/// assistants, then the rest (lower first).
fn giver_rank(ty: &SquadTypeSetup, kit: &Kit) -> u8 {
    let t = ty.members.get(kit.slot as usize).map_or(0, |m| m.traits);
    if t & member_trait::AMMO_BEARER != 0 {
        0
    } else if t & member_trait::CREW_ASSISTANT != 0 {
        1
    } else {
        2
    }
}

fn within(ents: &Entities, a: SimId, b: SimId, m: u32) -> bool {
    let r = Fx::from_int(m.min(10_000) as i32);
    (ents.transform[a.index()].pos.xy() - ents.transform[b.index()].pos.xy()).length_sq_raw()
        <= FxVec2::new(r, Fx::ZERO).length_sq_raw()
}

impl MatchState {
    /// S5a Status stage (serial): crew feed and sharing inside squads,
    /// then the base refill of men, AT teams and vehicles, priced round by
    /// round. Legacy content (no `[supply]`) keeps the S4 free resupply
    /// (`update_items`).
    pub(crate) fn supply_tick(&mut self, ents: &mut Entities, hash: &mut HashCache, tick: u64) {
        if self.setup.rules.supply.is_none() {
            return;
        }
        let ids: Vec<SquadId> = self.squads.keys().copied().collect();
        for &id in &ids {
            self.crew_feed(ents, hash, id);
            self.share(ents, hash, id, tick);
        }
        self.refill_men(ents, hash, tick);
        self.refill_launchers(ents, tick);
        self.rearm_vehicles(ents, tick);
    }

    /// S5a-6: a dry crew weapon (no rounds in it or its carrier's pouches)
    /// takes a magazine's worth from a squadmate within `crew_feed_m`
    /// holding that feed (assistants and bearers first, then ascending id).
    fn crew_feed(&mut self, ents: &mut Entities, hash: &mut HashCache, id: SquadId) {
        let setup = &self.setup;
        let sq = &self.squads[&id];
        let ty = &setup.squad_types[sq.type_idx as usize];
        if !counted(ty) {
            return;
        }
        let reach = setup.rules.combat.share.crew_feed_m;
        for &m in &sq.members {
            let i = m.index();
            let kit = &ents.kit[i];
            let w = &setup.weapons[kit.weapon as usize];
            if !w.crew || w.class == WeaponClass::Launcher || kit.on_sidearm {
                continue;
            }
            let Some(k) = ty.feed_slot(w) else { continue };
            let in_gun = if w.he.is_some() {
                0
            } else {
                ents.combat[i].mag
            };
            if kit.rounds[k] > 0 || in_gun > 0 {
                continue;
            }
            let giver = sq
                .members
                .iter()
                .copied()
                .filter(|&g| {
                    g != m && ents.kit[g.index()].rounds[k] > 0 && within(ents, g, m, reach)
                })
                .min_by_key(|g| (giver_rank(ty, &ents.kit[g.index()]), g.0));
            if let Some(g) = giver {
                let n = (w.magazine.min(u16::MAX as u32) as u16).min(ents.kit[g.index()].rounds[k]);
                ents.kit[g.index()].rounds[k] -= n;
                ents.kit[i].rounds[k] += n;
                hash.mark_entity_chunk(g.chunk());
                hash.mark_entity_chunk(m.chunk());
            }
        }
    }

    /// S5a sharing pass, every `combat.share.interval_s` per squad (ticks
    /// `tick % interval == id % interval`): per feed, the man with the
    /// lowest share of his own load below `below_pct` takes `share_pct` of
    /// the giver's load from the squadmate within `reach_m` with the
    /// highest share above `giver_above_pct` (bearers, then assistants,
    /// then the rest; ties to the lower id), never above his own load.
    fn share(&mut self, ents: &mut Entities, hash: &mut HashCache, id: SquadId, tick: u64) {
        let setup = &self.setup;
        let sh = &setup.rules.combat.share;
        let every = (sh.interval_s as u64 * setup.tick_hz as u64).max(1);
        if tick % every != id.0 as u64 % every {
            return;
        }
        let sq = &self.squads[&id];
        let ty = &setup.squad_types[sq.type_idx as usize];
        for k in 0..ty.feeds.len().min(MAX_FEEDS) {
            let share_of = |m: SimId| {
                let kit = &ents.kit[m.index()];
                let load = man_load(ty, kit, k);
                (load > 0).then(|| {
                    let t = man_total(setup, ty, kit, &ents.combat[m.index()], k);
                    (t * 1000 / load, t, load)
                })
            };
            // The man who carries a crew weapon keeps its rounds (he is
            // fed, he does not feed), and is fed whole magazines by the
            // crew feed only (S5a Part B: a part-filled pouch made part
            // belts).
            let keeps = |g: SimId| {
                let w = &setup.weapons[ents.kit[g.index()].weapon as usize];
                w.crew && ty.feed_slot(w) == Some(k)
            };
            let taker = sq
                .members
                .iter()
                .copied()
                .filter(|&m| !keeps(m))
                .filter_map(|m| share_of(m).map(|s| (s, m)))
                .filter(|((pm, _, _), _)| *pm < sh.below_pct * 10)
                .min_by_key(|((pm, _, _), m)| (*pm, m.0));
            let Some(((_, t_total, t_load), taker)) = taker else {
                continue;
            };
            let giver = sq
                .members
                .iter()
                .copied()
                .filter(|&g| g != taker && ents.kit[g.index()].rounds[k] > 0 && !keeps(g))
                .filter(|&g| within(ents, g, taker, sh.reach_m))
                .filter_map(|g| share_of(g).map(|s| (s, g)))
                .filter(|((pm, _, _), _)| *pm > sh.giver_above_pct * 10)
                .min_by_key(|((pm, _, _), g)| {
                    (giver_rank(ty, &ents.kit[g.index()]), u32::MAX - *pm, g.0)
                });
            let Some(((_, _, g_load), giver)) = giver else {
                continue;
            };
            let room = t_load.saturating_sub(t_total);
            let n = (pct_of(g_load, sh.share_pct).max(1))
                .min(room)
                .min(ents.kit[giver.index()].rounds[k] as u32) as u16;
            if n == 0 {
                continue;
            }
            ents.kit[giver.index()].rounds[k] -= n;
            ents.kit[taker.index()].rounds[k] += n;
            hash.mark_entity_chunk(giver.chunk());
            hash.mark_entity_chunk(taker.chunk());
        }
    }

    /// S5a base refill of men (ascending id): small-arms rounds at
    /// `base_small_arms_rounds_s` per man (the most depleted small-arms
    /// feed first), and alongside them items and mortar bombs one at a
    /// time (`items.resupply_s` per item, `base_bomb_s` per bomb; items
    /// first by kind, then bombs; bombs wait `base_bomb_after_fire_s` after
    /// the mortar last fired). Each round is paid as it goes in; with
    /// no manpower the refill pauses and stays due.
    fn refill_men(&mut self, ents: &mut Entities, hash: &mut HashCache, tick: u64) {
        let Some(sp) = self.setup.rules.supply.clone() else {
            return;
        };
        let hz = self.setup.tick_hz.max(1);
        let item_every = self.setup.rules.items.resupply_ticks.max(1);
        let bomb_every = (sp.base_bomb_s * hz).max(1);
        let bomb_wait = (sp.base_bomb_after_fire_s * hz).min(u16::MAX as u32);
        for i in 0..ents.len() {
            if ents.kind[i] != EntityKind::Soldier {
                continue;
            }
            let m = SimId(i as u32);
            let Some(sid) = ents.squad[i] else { continue };
            let Some(sq) = self.squads.get(&sid) else {
                continue;
            };
            let owner = ents.owner[i];
            let ty_idx = sq.type_idx as usize;
            if !in_own_base(&self.setup, owner, ents.transform[i].pos.xy()) {
                let k = &mut ents.kit[i];
                if k.refill != 0 || k.resupply != 0 {
                    k.refill = 0;
                    k.resupply = 0;
                    hash.mark_entity_chunk(m.chunk());
                }
                continue;
            }
            let mut changed = false;
            // Small arms.
            let small = |setup: &MatchSetup, f: Feed| {
                feed_supply(setup, f).is_some_and(|(kd, _)| kd == SupplyKind::SmallArms)
            };
            let pick = |st: &MatchState, ents: &Entities, want_small: bool| -> Option<usize> {
                let ty = &st.setup.squad_types[ty_idx];
                (0..ty.feeds.len().min(MAX_FEEDS))
                    .filter(|&k| small(&st.setup, ty.feeds[k]) == want_small)
                    .filter_map(|k| {
                        let load = man_load(ty, &ents.kit[i], k);
                        let t = man_total(&st.setup, ty, &ents.kit[i], &ents.combat[i], k);
                        (t < load).then_some((t * 1000 / load.max(1), k))
                    })
                    .min()
                    .map(|(_, k)| k)
            };
            {
                let short = pick(self, ents, true);
                let k = &mut ents.kit[i];
                if short.is_none() {
                    if k.refill != 0 {
                        k.refill = 0;
                        changed = true;
                    }
                } else {
                    k.refill =
                        (k.refill as u32 + sp.base_small_arms_rounds_s).min(u16::MAX as u32) as u16;
                    changed = true;
                }
            }
            while ents.kit[i].refill as u32 >= hz {
                let Some(k) = pick(self, ents, true) else {
                    ents.kit[i].refill = 0;
                    break;
                };
                let f = self.setup.squad_types[ty_idx].feeds[k];
                let (kd, g) = feed_supply(&self.setup, f).expect("small arms");
                let price = micro_price(&self.setup, kd, g);
                if !self.pay(owner.0 as usize, kd, price, tick) {
                    ents.kit[i].refill = hz as u16;
                    break;
                }
                ents.kit[i].rounds[k] += 1;
                ents.kit[i].refill -= hz as u16;
            }
            // Items, then bombs (and other non-small-arms feeds).
            let ty = &self.setup.squad_types[ty_idx];
            let full = ty.full_kit(ents.kit[i].slot);
            // S11b G4: never a rifle grenade (crate-only by design, D 954,
            // D 1161-1163).
            let rg = frontline_content::ItemKind::RifleGrenade.index();
            let item = (0..ITEM_KINDS).find(|&j| j != rg && ents.kit[i].items[j] < full[j]);
            // S6 (owner Q4): with `base_bomb_s = 0` the base refills no
            // shells feed (mortar bombs): they come only in crates.
            let no_bombs = sp.base_bomb_s == 0;
            let next = match item {
                Some(j) => Some((false, j, item_every)),
                None => pick(self, ents, false)
                    .filter(|&k| {
                        !(no_bombs
                            && feed_supply(&self.setup, self.setup.squad_types[ty_idx].feeds[k])
                                .is_some_and(|(kd, _)| kd == SupplyKind::Shells))
                    })
                    .filter(|&k| !self.bombs_wait(ents, sid, k, bomb_wait))
                    .map(|k| (true, k, bomb_every)),
            };
            match next {
                None => {
                    if ents.kit[i].resupply != 0 {
                        ents.kit[i].resupply = 0;
                        changed = true;
                    }
                }
                Some((bomb, j, every)) => {
                    let kit = &mut ents.kit[i];
                    kit.resupply = (kit.resupply as u32 + 1).min(every) as u16;
                    changed = true;
                    if kit.resupply as u32 >= every {
                        let ty = &self.setup.squad_types[ty_idx];
                        let (kd, g) = if bomb {
                            feed_supply(&self.setup, ty.feeds[j]).expect("feed")
                        } else {
                            ty.items[j]
                                .and_then(|x| self.setup.items[x as usize].supply)
                                .map_or((SupplyKind::Explosives, 0), |s| (s.kind, s.g))
                        };
                        let price = micro_price(&self.setup, kd, g);
                        if self.pay(owner.0 as usize, kd, price, tick) {
                            if bomb && kd == SupplyKind::Shells {
                                // S6 report and the T20 check.
                                let p = owner.0 as usize;
                                if let Some(a) = self.supply6.accounts.get_mut(p) {
                                    a.base_bombs += 1;
                                }
                                self.explain_bombs(p, 1);
                            }
                            let kit = &mut ents.kit[i];
                            kit.resupply = 0;
                            if bomb {
                                kit.rounds[j] += 1;
                            } else {
                                kit.items[j] += 1;
                            }
                        }
                    }
                }
            }
            if changed {
                hash.mark_entity_chunk(m.chunk());
            }
        }
    }

    /// S5a Fix (review M2, `supply.base_bomb_after_fire_s`): squad `sid`'s
    /// feed `k` waits for base bombs while a man carrying the crew weapon
    /// it feeds fired within `wait` ticks (the base battery brake).
    fn bombs_wait(&self, ents: &Entities, sid: SquadId, k: usize, wait: u32) -> bool {
        if wait == 0 {
            return false;
        }
        let Some(sq) = self.squads.get(&sid) else {
            return false;
        };
        let ty = &self.setup.squad_types[sq.type_idx as usize];
        sq.members.iter().any(|g| {
            let w = &self.setup.weapons[ents.kit[g.index()].weapon as usize];
            w.crew
                && ty.feed_slot(w) == Some(k)
                && (ents.combat[g.index()].since_shot as u32) < wait
        })
    }

    /// S5a: AT teams' rockets in the own base, one per `items.resupply_s`,
    /// each paid at its round's supply price (S5a-5: a team count).
    fn refill_launchers(&mut self, ents: &Entities, tick: u64) {
        let every = self.setup.rules.items.resupply_ticks.max(1);
        let ids: Vec<SquadId> = self.squads.keys().copied().collect();
        for id in ids {
            let setup = &self.setup;
            let sq = &self.squads[&id];
            let Some(l) = &sq.launcher else { continue };
            let ty = &setup.squad_types[sq.type_idx as usize];
            let w = &setup.weapons[ty.weapon as usize];
            let in_base = l
                .carrier
                .is_some_and(|c| in_own_base(setup, sq.owner, ents.transform[c.index()].pos.xy()));
            let owner = sq.owner.0 as usize;
            let (kd, g) = w
                .ammo
                .and_then(|a| setup.ammo[a as usize].supply)
                .map_or((SupplyKind::Explosives, 0), |s| (s.kind, s.g));
            let price = micro_price(setup, kd, g);
            let full = w.rounds_carried;
            let due = {
                let l = self
                    .squads
                    .get_mut(&id)
                    .and_then(|s| s.launcher.as_mut())
                    .expect("launcher");
                if in_base && (l.rounds as u32) < full {
                    l.resupply = (l.resupply as u32 + 1).min(every) as u16;
                    l.resupply as u32 >= every
                } else {
                    l.resupply = 0;
                    false
                }
            };
            if due && self.pay(owner, kd, price, tick) {
                let l = self
                    .squads
                    .get_mut(&id)
                    .and_then(|s| s.launcher.as_mut())
                    .expect("launcher");
                l.resupply = 0;
                l.rounds += 1;
            }
        }
    }

    /// S5a base rearm of vehicles (ascending id), priced: one standard
    /// main-gun round per `vehicles.base_rearm_s` (the entry furthest below
    /// its load), one rare round per `supply.rare_rearm_base_s` up to the
    /// vehicle's `rare_cap`, and a belt into each MG with spent stowage per
    /// standard step. A step that cannot be paid stays due
    /// ([`crate::vehicle::VehicleState::supply_due`]).
    fn rearm_vehicles(&mut self, ents: &Entities, tick: u64) {
        let Some(sp) = self.setup.rules.supply.clone() else {
            return;
        };
        let hz = self.setup.tick_hz.max(1) as u64;
        let std_every = self.setup.rules.vehicles.base_rearm_ticks.max(1) as u64;
        let rare_every = (sp.rare_rearm_base_s as u64 * hz).max(1);
        let ids: Vec<SimId> = self.vehicles.keys().copied().collect();
        for id in ids {
            if ents.kind[id.index()] != EntityKind::Vehicle {
                continue;
            }
            let owner = ents.owner[id.index()].0 as usize;
            // S9: a captured gun is not rearmed by its captor's base.
            let faction = self.setup.players[owner].faction;
            let vs = self.vehicles.get_mut(&id).expect("listed");
            let vd = &self.setup.vehicles[vs.type_idx as usize];
            if vs.base_ticks == 0 || (vd.served() && vd.faction != faction) {
                vs.supply_due = 0;
                continue;
            }
            let n = vs.base_ticks as u64;
            if n.is_multiple_of(std_every) {
                vs.supply_due |= DUE_STD | DUE_MG;
            }
            if n.is_multiple_of(rare_every) {
                vs.supply_due |= DUE_RARE;
            }
            if vs.supply_due == 0 {
                continue;
            }
            // MG belts: each round of a belt paid at small-arms price.
            if vs.supply_due & DUE_MG != 0 {
                let mut all = true;
                let nm = self.vehicles[&id].mgs.len();
                for k in 0..nm {
                    let v = &self.setup.vehicles[self.vehicles[&id].type_idx as usize];
                    let m = &v.mg_mounts()[k];
                    let w = &self.setup.weapons[m.weapon as usize];
                    let belt = w.magazine.max(1);
                    let (kd, g) = w
                        .feed
                        .and_then(|f| feed_supply(&self.setup, f))
                        .unwrap_or((SupplyKind::SmallArms, 0));
                    let price = micro_price(&self.setup, kd, g);
                    let mut todo = self.vehicles[&id].mgs[k].spent.min(belt);
                    while todo > 0 {
                        if !self.pay(owner, kd, price, tick) {
                            all = false;
                            break;
                        }
                        let st = &mut self.vehicles.get_mut(&id).expect("listed").mgs[k];
                        st.spent -= 1;
                        todo -= 1;
                    }
                }
                if all {
                    self.vehicles.get_mut(&id).expect("listed").supply_due &= !DUE_MG;
                }
            }
            for rare in [false, true] {
                let bit = if rare { DUE_RARE } else { DUE_STD };
                let vs = &self.vehicles[&id];
                if vs.supply_due & bit == 0 {
                    continue;
                }
                let v = &self.setup.vehicles[vs.type_idx as usize];
                let Some(gun) = v.main_gun() else {
                    continue;
                };
                let Some(k) = pick_round(vs, v, rare) else {
                    self.vehicles.get_mut(&id).expect("listed").supply_due &= !bit;
                    continue;
                };
                let (kd, g) = self.setup.ammo[gun.ammo[k].ammo as usize]
                    .supply
                    .map_or((SupplyKind::Shells, 0), |s| (s.kind, s.g));
                let price = micro_price(&self.setup, kd, g) * v.rack_price_pct as u64 / 100;
                let inf = v.role == frontline_content::VehicleRole::InfGun;
                if self.pay(owner, kd, price, tick) {
                    let vs = self.vehicles.get_mut(&id).expect("listed");
                    vs.ammo[k] += 1;
                    vs.supply_due &= !bit;
                    // S11a1: an infantry gun's shells bought in base.
                    if inf {
                        let c = &mut self.players[owner].s11;
                        c.gun_shell_micro = c.gun_shell_micro.saturating_add(price);
                    }
                }
            }
        }
    }
}

/// S8b: the main-gun entry (standard or `rare`) furthest below its load, the
/// one a rearm step fills; `None` when nothing is short, a rare round is
/// at `rare_cap`, or the vehicle has no main gun. Base and forward rearm
/// both choose with this.
pub(crate) fn pick_round(
    vs: &crate::vehicle::VehicleState,
    v: &frontline_content::vehicle::VehicleSetup,
    rare: bool,
) -> Option<usize> {
    let gun = v.main_gun()?;
    let held = |k: usize| vs.ammo[k] as u32 + (vs.loaded == Some(k as u8)) as u32;
    let rare_held: u32 = gun
        .ammo
        .iter()
        .enumerate()
        .filter(|(_, a)| a.rare)
        .map(|(k, _)| held(k))
        .sum();
    if rare && rare_held >= v.rare_cap {
        return None;
    }
    gun.ammo
        .iter()
        .enumerate()
        .filter(|(_, a)| a.rare == rare)
        .map(|(k, a)| (k, (a.count as u32).saturating_sub(held(k))))
        .filter(|(_, short)| *short > 0)
        .max_by_key(|&(k, short)| (short, std::cmp::Reverse(k)))
        .map(|(k, _)| k)
}

/// S5a: takes one mortar bomb for `gunner` (squad `members` of type
/// `type_idx`) from his weapon, his pouches, then a squadmate within
/// `crew_feed_m` (assistants and bearers first, then ascending id). False:
/// no bombs left in reach. Legacy types fire without counting.
pub(crate) fn take_bomb(
    setup: &MatchSetup,
    ents: &mut Entities,
    members: &[SimId],
    type_idx: u16,
    gunner: SimId,
    wi: u16,
) -> bool {
    let ty = &setup.squad_types[type_idx as usize];
    let Some(k) = ty.feed_slot(&setup.weapons[wi as usize]) else {
        return true;
    };
    let i = gunner.index();
    if ents.combat[i].mag > 0 {
        ents.combat[i].mag -= 1;
        return true;
    }
    if ents.kit[i].rounds[k] > 0 {
        ents.kit[i].rounds[k] -= 1;
        return true;
    }
    let reach = setup.rules.combat.share.crew_feed_m;
    let giver = members
        .iter()
        .copied()
        .filter(|&g| {
            g != gunner && ents.kit[g.index()].rounds[k] > 0 && within(ents, g, gunner, reach)
        })
        .min_by_key(|g| (giver_rank(ty, &ents.kit[g.index()]), g.0));
    match giver {
        Some(g) => {
            ents.kit[g.index()].rounds[k] -= 1;
            true
        }
        None => false,
    }
}

/// [`crate::vehicle::VehicleState::supply_due`] bits.
pub const DUE_STD: u8 = 1;
pub const DUE_RARE: u8 = 2;
pub const DUE_MG: u8 = 4;

/// S5a T5: ticks weapon `w` needs to fire `rounds` rounds steadily, first
/// shot to last, from full magazines: after each shot the interval
/// (Bresenham on the shot counter), plus the burst pause after every
/// `burst_rounds` shots (B1); between magazines one interval (the empty
/// gun waits for a squadmate's feed) plus the reload.
pub fn expected_fire_ticks(w: &frontline_content::setup::WeaponSetup, rounds: u32) -> u64 {
    let mag = w.magazine.max(1);
    let ceil = w.interval_milli.div_ceil(1000) as u64;
    let mut t = 0u64;
    for n in 1..rounds {
        t += if n.is_multiple_of(mag) {
            ceil + w.reload_ticks as u64
        } else {
            crate::combat::burst_cooldown(w, n) as u64
        };
    }
    t
}
