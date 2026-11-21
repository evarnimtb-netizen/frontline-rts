//! S12 wounds (docs/slices/S12-wounds-medics.md 3, 4): the saved layout of
//! wound states, casualties and aid-post beds, their load checks, and the
//! wound rules W1-W5, W12-W14 (Build A). Every rule reads `[wounds]
//! enabled`; at 0 a soldier at 0 HP dies exactly as before S12 and no S12
//! RNG draw happens.

use serde::{Deserialize, Serialize};

use frontline_content::tuning::WoundsTuning;

use crate::ecs::{Combat, Entities, EntityKind, SimId, Stance, Transform};
use crate::fx::{Fx, FxVec2};
use crate::game::{MatchState, SquadId, kill_class};

/// `Wound::state`: not hurt past `wounded_hp`.
pub const FIT: u8 = 0;
/// `Wound::state`: hurt, in his squad, bleeding until bandaged (W4).
pub const WOUNDED: u8 = 1;
/// `Wound::state`: down (`EntityKind::Downed`), out of his squad (W5).
pub const DOWNED: u8 = 2;

/// W1 hit zones (`Hit::zone`).
pub const ZONE_TORSO: u8 = 0;
pub const ZONE_HEAD: u8 = 1;
pub const ZONE_LEGS: u8 = 2;
/// Blast overpressure and other hits with no zone.
pub const ZONE_NONE: u8 = 3;

/// Most casualties a match holds at once (spec 3).
pub const MAX_CASUALTIES: usize = 512;
/// Most beds an aid post holds (the def's `beds` is at most this).
pub const MAX_BEDS: usize = 8;

/// A soldier's wound state (spec 3; in `Combat`, hashed with its chunk).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Wound {
    /// [`FIT`], [`WOUNDED`] or [`DOWNED`].
    pub state: u8,
    pub bandaged: bool,
    /// A leg wound (`legs_speed_pct`).
    pub legs: bool,
    /// Treated at an aid post once (W8, S12-7).
    pub treated: bool,
    /// Bought or reinforced: refunded when evacuated (W9).
    pub paid: bool,
    /// Bleed time carried to the next HP (ms, below `bleed_ms_per_hp`).
    pub bleed_acc_ms: u16,
    /// Ticks of bandaging left (0: not bandaging).
    pub bandaging: u16,
    /// The last hitter: player and `kill_class` (W4 credit).
    pub by: (u8, u16),
}

/// A downed man (spec 3; `MatchState::casualties`, by his id).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Casualty {
    /// The squad he fell out of (it may be gone).
    pub from_squad: SquadId,
    /// His squad's card (index into `MatchSetup::squad_types`).
    pub card: u16,
    /// He dies at this tick (0: stabilised).
    pub bleed_out_at: u64,
    /// A medic tending him and the tick he finishes (Build B).
    pub tend: Option<(SimId, u64)>,
    /// The men carrying him (Build B).
    pub carriers: [Option<SimId>; 2],
    /// Where they carry him (Build B).
    pub dest: Option<FxVec2>,
    /// Evacuated at this tick (0: none; Build B).
    pub evac_at: u64,
}

/// One bed of an aid post (Build B; `Structure::beds`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Bed {
    pub man: SimId,
    pub done_at: u64,
    pub evac: bool,
}

/// Where a man was hit and how (W1-W3; the widened `Fallen`).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub struct Hit {
    /// HP left after the hit, signed (`hp - dmg`).
    pub raw: i32,
    /// [`ZONE_TORSO`], [`ZONE_HEAD`], [`ZONE_LEGS`] or [`ZONE_NONE`].
    pub zone: u8,
    /// Blast overpressure (always a death, W3).
    pub blast: bool,
    /// The damage dealt after the zone factor (W3 `head_kill_dmg`).
    pub dmg: u32,
}

impl Hit {
    /// W3: whether a man who fell to this hit dies outright (else he is
    /// Downed).
    pub fn outright(&self, w: &WoundsTuning) -> bool {
        self.blast
            || self.raw <= -(w.overkill_kill as i32)
            || (self.zone == ZONE_HEAD && w.head_kill_dmg > 0 && self.dmg >= w.head_kill_dmg)
    }
}

/// W1: the zone of a hit at world height `z` on a man (standing or
/// crouched: his capsule's top `head_top_pct` is the head, the bottom
/// `legs_bottom_pct` the legs; prone men are torso).
pub(crate) fn zone_at(t: &Transform, c: &Combat, z: Fx, w: &WoundsTuning) -> u8 {
    if c.stance == Stance::Prone || c.height <= Fx::ZERO {
        return ZONE_TORSO;
    }
    let pct = (z - t.pos.z).raw() as i64 * 100 / c.height.raw() as i64;
    if pct >= 100 - w.head_top_pct as i64 {
        ZONE_HEAD
    } else if pct < w.legs_bottom_pct as i64 {
        ZONE_LEGS
    } else {
        ZONE_TORSO
    }
}

/// W2, W3, W4: deals `dmg` to the man with combat state `c`, hit in `zone`
/// (`blast`: overpressure, no zone) by `by`. Returns the hit when he falls
/// to 0 HP (a downed man falls to any hit). `w` is `None` with `[wounds]
/// enabled` 0: then exactly the pre-S12 arithmetic (`saturating_sub`) and
/// no wound state.
pub(crate) fn deal(
    c: &mut Combat,
    dmg: u32,
    zone: u8,
    blast: bool,
    by: (u8, u16),
    w: Option<&WoundsTuning>,
) -> Option<Hit> {
    let Some(w) = w else {
        let before = c.hp as i64;
        c.hp = c.hp.saturating_sub(dmg.min(u16::MAX as u32) as u16);
        return (c.hp == 0).then_some(Hit {
            raw: (before - dmg as i64).max(i32::MIN as i64) as i32,
            zone,
            blast,
            dmg,
        });
    };
    let pct = match zone {
        ZONE_HEAD => w.head_damage_pct,
        ZONE_LEGS => w.legs_damage_pct,
        ZONE_TORSO => w.torso_damage_pct,
        _ => 100,
    };
    let dmg = if blast || zone == ZONE_NONE {
        dmg
    } else {
        (dmg as u64 * pct as u64 / 100).min(u32::MAX as u64) as u32
    };
    let raw = (c.hp as i64 - dmg as i64).max(i32::MIN as i64) as i32;
    let wd = &mut c.wound;
    wd.by = by;
    // A new hit breaks a dressing in progress.
    wd.bandaging = 0;
    if wd.state == DOWNED || raw <= 0 {
        c.hp = 0;
        return Some(Hit {
            raw,
            zone,
            blast,
            dmg,
        });
    }
    c.hp = raw as u16;
    if raw <= w.wounded_hp as i32 {
        wd.state = WOUNDED;
        wd.bandaged = false;
        wd.bleed_acc_ms = 0;
        if zone == ZONE_LEGS {
            wd.legs = true;
        }
    }
    None
}

/// S12 per-player counters (saved, hashed; the `s12` JSON block). Pairs
/// are `[trench, open]`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct S12Counters {
    pub downed: u32,
    pub killed_outright: u32,
    pub bled_out: u32,
    /// Downed men killed by a later hit.
    pub downed_killed: u32,
    pub stabilised: u32,
    pub bandaged: u32,
    pub carried: u32,
    pub treated: u32,
    pub evacuated: u32,
    pub refund_mp: u32,
    pub reinforced_men: u32,
    pub reinforce_mp: u32,
    /// Enemy men this player downed, by `kill_class` (W13).
    pub downed_by_class: [u32; kill_class::COUNT],
    /// S12 F2 (review R1): downed enemy men whose death credited this
    /// player a kill, by the finishing `kill_class` (a bleed-out credits
    /// the class that downed him). `kills_by - finished_by_class` is the
    /// men this player killed outright.
    pub finished_by_class: [u32; kill_class::COUNT],
    pub head_hits: [u32; 2],
    pub torso_hits: [u32; 2],
    pub leg_hits: [u32; 2],
}

/// Load checks of one entity's wound (spec 3, `load_rejects_bad_s12_state`).
/// `vehicles`: the setup's vehicle types (S12 F2, review R10: the type
/// bits of `by.1` name one of them, as `kill_class::vehicle` reads them).
pub(crate) fn wound_ok(
    kind: EntityKind,
    w: &Wound,
    players: usize,
    vehicles: usize,
    bandage_ticks: u64,
) -> bool {
    let state_ok = match kind {
        EntityKind::Soldier => w.state <= WOUNDED,
        EntityKind::Downed => w.state == DOWNED,
        _ => *w == Wound::default(),
    };
    let fit_clean = w.state != FIT || (!w.bandaged && !w.legs && w.bandaging == 0);
    state_ok
        && fit_clean
        && (w.bandaging as u64) <= bandage_ticks
        && !(w.bandaged && w.bandaging > 0)
        // S12b Build C: `by.1` may carry the hitting vehicle's type above
        // its class (`kill_class::vehicle`), as every kill class does.
        && (w.by == (0, 0)
            || ((w.by.0 as usize) < players
                && (kill_class::base(w.by.1) as usize) < kill_class::COUNT
                && kill_class::vehicle(w.by.1).is_none_or(|t| t < vehicles)))
}

impl MatchState {
    /// Load checks of the casualties and beds (spec 3). Every Downed entity
    /// has exactly one casualty and every casualty names a Downed entity.
    pub(crate) fn validate_s12(&self, ents: &Entities, tick: u64) -> Result<(), &'static str> {
        let s = &self.setup;
        if self.casualties.len() > MAX_CASUALTIES {
            return Err("more casualties than the cap");
        }
        let downed = ents
            .kind()
            .iter()
            .filter(|k| **k == EntityKind::Downed)
            .count();
        if downed != self.casualties.len() {
            return Err("downed men and casualties disagree");
        }
        let max_bleed = crate::bounds::MAX_TICK
            + (s.rules.wounds.down_bleed_max_s as u64 + 1) * s.tick_hz as u64;
        for (&id, c) in &self.casualties {
            if ents.kind().get(id.index()) != Some(&EntityKind::Downed) {
                return Err("a casualty is not a downed man");
            }
            if ents.squad()[id.index()].is_some() {
                return Err("a downed man is in a squad");
            }
            if c.card as usize >= s.squad_types.len() {
                return Err("a casualty's card is unknown");
            }
            if c.bleed_out_at != 0 && (c.bleed_out_at < tick || c.bleed_out_at > max_bleed) {
                return Err("a casualty's bleed-out tick is out of range");
            }
            let owner = ents.owner()[id.index()];
            let own_man = |m: SimId| {
                ents.kind().get(m.index()) == Some(&EntityKind::Soldier)
                    && ents.owner()[m.index()] == owner
            };
            if let Some((m, at)) = c.tend
                && (!own_man(m) || at < tick || at > crate::bounds::MAX_TICK)
            {
                return Err("a casualty's medic is bad");
            }
            // Checker G9: the second carrier only with a first.
            if c.carriers.iter().flatten().any(|m| !own_man(*m))
                || (c.carriers[0].is_some() && c.carriers[0] == c.carriers[1])
                || (c.carriers[0].is_none() && c.carriers[1].is_some())
            {
                return Err("a casualty's carriers are bad");
            }
            let size = s.map.size_m;
            if let Some(d) = c.dest {
                let on = |v: crate::fx::Fx| {
                    v >= crate::fx::Fx::ZERO && v <= crate::fx::Fx::from_int(size)
                };
                if !on(d.x) || !on(d.y) {
                    return Err("a casualty's destination is off the map");
                }
            }
            if c.evac_at != 0 && (c.evac_at < tick || c.evac_at > crate::bounds::MAX_TICK) {
                return Err("a casualty's evacuation tick is out of range");
            }
        }
        // Checker G9: a man tends or carries one casualty at most.
        let mut jobs: Vec<SimId> = self
            .casualties
            .values()
            .flat_map(|c| {
                c.tend
                    .map(|t| t.0)
                    .into_iter()
                    .chain(c.carriers.iter().flatten().copied())
            })
            .collect();
        jobs.sort_unstable();
        if jobs.windows(2).any(|w| w[0] == w[1]) {
            return Err("a man tends or carries two casualties");
        }
        let mut in_bed: Vec<SimId> = Vec::new();
        for st in self.supply6.structures.values() {
            if st.beds.len() > MAX_BEDS {
                return Err("an aid post has too many beds");
            }
            let aid = s
                .stocks
                .get(st.def as usize)
                .is_some_and(|d| d.kind == frontline_content::setup::StockKind::Aid);
            if !st.beds.is_empty() && !aid {
                return Err("beds on a structure that is not an aid post");
            }
            for b in &st.beds {
                let k = ents.kind().get(b.man.index());
                if !matches!(k, Some(EntityKind::Soldier | EntityKind::Downed))
                    || ents.owner()[b.man.index()] != st.owner
                    || b.done_at > crate::bounds::MAX_TICK
                {
                    return Err("a bed's man is bad");
                }
                // Checker G1: a treatment bed holds a wounded soldier.
                let c = &ents.combat()[b.man.index()];
                if !b.evac && (k != Some(&EntityKind::Soldier) || c.wound.state != WOUNDED) {
                    return Err("a treatment bed's man is not wounded");
                }
                in_bed.push(b.man);
            }
        }
        in_bed.sort_unstable();
        if in_bed.windows(2).any(|w| w[0] == w[1]) {
            return Err("a man in two beds");
        }
        Ok(())
    }
}

impl MatchState {
    /// S12 W4, W5 (Status stage, serial, ascending id; nothing with
    /// `[wounds] enabled` 0): downed men whose bleed-out tick came die,
    /// credited to their last hitter; wounded men bandage themselves while
    /// under `crouch_suppression` and not firing (`bandage_s`; firing,
    /// suppression or a new hit breaks it) and bleed 1 HP per
    /// `bleed_ms_per_hp` until bandaged (counted once a second, on the
    /// tick `(tick + id) % hz == 0`); at 0 HP a wounded man goes down.
    pub(crate) fn wounds_tick(
        &mut self,
        ctx: &mut crate::game::Ctx,
        stats: &mut crate::world::Stats,
        seed: u64,
    ) {
        let w = &self.setup.rules.wounds;
        if w.enabled == 0 {
            return;
        }
        let tick = ctx.tick;
        let hz = self.setup.tick_hz.max(1) as u64;
        let (bandage, per_hp) = (w.bandage_s as u64 * hz, w.bleed_ms_per_hp.max(1));
        let crouch = self.setup.rules.combat.crouch_suppression;
        let corpse_ticks = self.setup.rules.combat.corpse_ticks;
        if !self.casualties.is_empty() {
            let due: Vec<SimId> = self
                .casualties
                .iter()
                .filter(|(_, c)| c.bleed_out_at != 0 && c.bleed_out_at <= tick)
                .map(|(&id, _)| id)
                .collect();
            for id in due {
                let i = id.index();
                let (owner, by) = (ctx.ents.owner()[i].0, ctx.ents.combat()[i].wound.by);
                self.bump_s12(owner, |c| &mut c.bled_out);
                self.kill(ctx.ents, ctx.hash, id, corpse_ticks);
                // Checker F2: every death counts in `deaths`.
                stats.deaths += 1;
                self.credit_finish(by, by.0 == owner);
            }
        }
        // W6: the possible tenders (buddies, medics) and the carriers.
        let idle = Self::idle_detached(ctx.ents);
        let carrying = self.carriers();
        let mut done_buddies: Vec<SimId> = Vec::new();
        let mut fell: Vec<SimId> = Vec::new();
        for i in 0..ctx.ents.len() {
            if ctx.ents.kind()[i] != EntityKind::Soldier
                || ctx.ents.combat()[i].wound.state != WOUNDED
            {
                continue;
            }
            let id = SimId(i as u32);
            // W6: a buddy standing by him bandages him, under fire too.
            let buddy = if ctx.ents.combat[i].wound.bandaged || idle.is_empty() {
                None
            } else {
                self.buddy_of(ctx.ents, &idle, i, &carrying)
            };
            let c = &mut ctx.ents.combat[i];
            let owner = ctx.ents.owner[i].0;
            if !c.wound.bandaged {
                let calm = buddy.is_some() || (c.suppression / 1000 < crouch && c.since_shot > 1);
                if c.wound.bandaging > 0 {
                    if !calm {
                        c.wound.bandaging = 0;
                    } else {
                        c.wound.bandaging -= 1;
                        if c.wound.bandaging == 0 {
                            c.wound.bandaged = true;
                            c.wound.bleed_acc_ms = 0;
                            if let Some(p) = self.players.get_mut(owner as usize) {
                                p.s12.bandaged = p.s12.bandaged.saturating_add(1);
                            }
                            done_buddies.push(id);
                        }
                    }
                } else if calm {
                    c.wound.bandaging = bandage.min(u16::MAX as u64) as u16;
                }
            }
            if !c.wound.bandaged && (tick + i as u64).is_multiple_of(hz) {
                let mut acc = c.wound.bleed_acc_ms as u32 + 1000;
                while acc >= per_hp && c.hp > 0 {
                    c.hp -= 1;
                    acc -= per_hp;
                }
                c.wound.bleed_acc_ms = acc.min(u16::MAX as u32) as u16;
                if c.hp == 0 {
                    fell.push(id);
                }
            }
            ctx.hash.mark_entity_chunk(id.chunk());
        }
        for id in fell {
            let i = id.index();
            let (owner, by) = (ctx.ents.owner()[i].0, ctx.ents.combat()[i].wound.by);
            if self.casualties.len() >= MAX_CASUALTIES {
                self.bump_s12(owner, |c| &mut c.bled_out);
                self.kill(ctx.ents, ctx.hash, id, corpse_ticks);
                // Checker F2: every death counts in `deaths`.
                stats.deaths += 1;
                self.credit_kill(by, by.0 == owner);
            } else {
                ctx.ents.combat[i].hp = 1;
                self.down(ctx.ents, ctx.hash, Some(ctx.terrain), id, by, tick, seed);
            }
        }
        // W6: the buddies of a man whose dressing is done (his own or
        // theirs; checker G4: also a buddy still walking to him) rejoin
        // their squads.
        for id in done_buddies {
            let at = ctx.ents.transform[id.index()].pos.xy();
            let reach = crate::medics::BUDDY_REACH_M;
            self.release_helpers(ctx.ents, ctx.hash, id, at, reach, &[]);
        }
        // W6, W7 (`crate::medics`).
        self.medics_tick(ctx);
    }
}
