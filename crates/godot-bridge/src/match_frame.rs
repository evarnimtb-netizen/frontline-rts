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
//!
//! S1b adds per soldier the state the soldier-level HUD needs (hit points,
//! suppression, stance order, detached, items, a stable formation slot),
//! detached men's own orders, thrown items and smoke clouds. Thrown items
//! and clouds have no spotting of their own: the local player sees his own,
//! items whose thrower he sees or that land near his men, and clouds
//! within sight of one of his men.
//!
//! S3 adds vehicles, shells and armour events, with the fog of war of the
//! snapshot contract (`docs/slices/S3-armour.md`): own vehicles always,
//! enemy vehicles only while the local player sees them (their own-only
//! fields zeroed, flags cut to [`vehicle_flags::PUBLIC`]), wrecks always;
//! shells fired by his units, by a unit he sees, or within
//! [`THROWN_NEAR_M`] of his men; armour events for his vehicles, for
//! vehicles he sees (or saw the tick before: a knock-out turns a vehicle
//! into a wreck everyone sees), or within that distance of his men.
//!
//! S5b adds to own squads on a fire mission their [`MissionView`] (mode,
//! phase, aim error and the first round's, rounds, observer and gunner
//! positions, charge, flight time), to the tick events the local player's
//! refused fire missions, and, while the presenter's fire-mission tool asks
//! ([`PreviewShared`]), the tool's [`PreviewView`] for the asked squad and
//! point (the sim's read-only `MatchState::fire_preview`, computed here on
//! the sim thread between ticks, never inside one) and the local player's
//! knowledge map ([`IntelView`]).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use frontline_content::{ITEM_KINDS, ItemKind, MatchSetup, WeaponClass};
use frontline_sim::projectile::{NO_AMMO, SUBSTEP_HZ};
use frontline_sim::snapshot::{NONE_U8, WRECK_OWNER};
use frontline_sim::{
    ArmourEvent, CasualtySnapshot, CorpseSnapshot, EntityKind, ExplosionSnapshot, NavPhase, Order,
    OrderKind, Outcome, PlayerSnapshot, PointSnapshot, ShellSnapshot, UnitOrder, VehicleSnapshot,
    World, tile_heights_f32, vehicle_flags,
};

/// Enemy thrown items landing or flying this close (m) to one of the local
/// player's men are shown even when he does not see the thrower.
pub const THROWN_NEAR_M: f32 = 40.0;

/// A squad as the presenter needs it (cards, markers, overlays, events).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SquadView {
    pub id: u32,
    pub type_idx: u16,
    pub owner: u8,
    pub men: u8,
    /// Members following their own unit order (S1b).
    pub detached: u8,
    pub order: OrderKind,
    pub phase: NavPhase,
    /// Sim id of the formation leader (`Squad::lead`), for the
    /// interpolated anchor.
    pub leader: u32,
    /// Leader position (sim axes) and where the squad is headed.
    pub anchor: [f32; 3],
    pub goal: [f32; 2],
    /// Sum of member hit points; suppression summed over members (0..=100
    /// each); members pinned.
    pub hp_sum: u32,
    pub suppression_sum: u32,
    pub pinned: u8,
    /// S3b Fix: members ducked below a trench lip (not firing).
    pub ducked: u8,
    /// MG team still setting up its guns after a move.
    pub setup: bool,
    /// Some member is seen by the local player (own squads: always).
    pub visible: bool,
    pub dig_pct: u8,
    /// Dig orders: the trench polyline (sim axes).
    pub dig_line: Vec<[f32; 2]>,
    /// Move, attack-move and fire-mission target.
    pub target: Option<[f32; 2]>,
    /// Fire-mission rounds left.
    pub rounds: u16,
    /// Items the men carry, by [`ItemKind::index`].
    pub items: [u16; ITEM_KINDS],
    /// S3: the vehicle a squad-level `FollowUnits` escorts.
    pub follow: Option<u32>,
    /// S8a: men of the squad aboard a transport (order `Embark`).
    pub mounted: u8,
    /// S3b: the line a squad-level `SuppressUnits` fires at (sim xy).
    pub suppress_line: Option<[[f32; 2]; 2]>,
    /// S10b: a `ClearMines` lane `(a, b, along m)` (sim xy) and a
    /// `Demolish` target piece.
    pub clear_lane: Option<([f32; 2], [f32; 2], f32)>,
    pub demolish: Option<u32>,
    /// S5a: the squad's ammunition share (%, `None`: uncounted), its crew
    /// weapon and the rounds held for it, men holding fire, and whether it
    /// stands in its own base below a full load (the refill serves it).
    pub ammo_pct: Option<u8>,
    pub crew: Option<(u16, u32)>,
    pub holding: u8,
    pub refilling: bool,
    /// S5b: own squads on a fire mission (S5b rules).
    pub mission: Option<MissionView>,
    /// S10a: construction kg the squad carries (own squads; 0 otherwise).
    pub materials_kg: f32,
}

/// S5b: an own squad's fire mission, for the mortar card and the observer
/// link line.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MissionView {
    /// `frontline_sim::fire_mission::mode` and `phase`.
    pub mode: u8,
    pub phase: u8,
    /// Aim error now and at the first round (m).
    pub error_m: f32,
    pub first_error_m: f32,
    /// Rounds an observer saw land, rounds fired, rounds left.
    pub seen: u8,
    pub fired: u16,
    pub rounds_left: u16,
    /// Adjusting rounds fired.
    pub adjust: u8,
    /// The observer of the last round and where he stands, the gunner's
    /// position (sim xy).
    pub observer: Option<u32>,
    pub observer_pos: Option<[f32; 2]>,
    pub gunner_pos: Option<[f32; 2]>,
    /// Charge index of the last round, its flight time (s).
    pub charge: u8,
    pub flight_s: f32,
    /// The mission started from a remembered ranging cell.
    pub ranged: bool,
}

/// S5b: what the presenter's fire-mission tool asks the sim thread.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreviewReq {
    pub squad: u32,
    /// Sim xy.
    pub point: [f32; 2],
    pub rounds: u16,
}

/// S5b: the fire-mission tool's request, shared between the presenter
/// (writes) and the sim thread (reads it when it publishes a frame). `generation`
/// counts writes so a paused sim republishes once per new request.
#[derive(Debug, Default)]
pub struct PreviewShared {
    pub req: Mutex<Option<PreviewReq>>,
    pub generation: AtomicU64,
}

impl PreviewShared {
    /// Sets (or clears) the request.
    pub fn set(&self, req: Option<PreviewReq>) {
        if let Ok(mut r) = self.req.lock()
            && *r != req
        {
            *r = req;
            self.generation.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn get(&self) -> Option<PreviewReq> {
        self.req.lock().ok().and_then(|r| *r)
    }
}

/// S5b: the answer to a [`PreviewReq`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreviewView {
    pub req: PreviewReq,
    pub refusal: Option<frontline_sim::FireMissionRefusal>,
    pub mode: u8,
    pub observer: Option<u32>,
    pub observer_pos: Option<[f32; 2]>,
    pub gunner_pos: [f32; 2],
    pub error_m: f32,
    pub ring_m: f32,
    pub ranged: bool,
    pub walk: bool,
    pub range_m: f32,
}

/// S5b: one cell of the local player's knowledge map (sim xy centre).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IntelView {
    pub pos: [f32; 2],
    pub age_s: f32,
    pub trench: bool,
}

/// A detached own soldier's order (S1b), for markers and the HUD.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct UnitView {
    pub id: u32,
    pub order: OrderKind,
    /// Move target, own base (retreat), middle of the trench line (dig),
    /// his spot (idle). Sim xy.
    pub target: [f32; 2],
    /// S3: the vehicle he escorts (`order` is `Follow`).
    pub follow: Option<u32>,
}

/// A thrown item in the air or lying fused.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct ThrownView {
    pub id: u32,
    pub kind: ItemKind,
    pub owner: u8,
    pub pos: [f32; 3],
    /// Velocity now (m/s, sim axes; zero once landed), for the trail.
    pub vel: [f32; 3],
    pub landed: bool,
    pub visible: bool,
}

/// S14: a flame particle in flight.
#[derive(Clone, Debug, PartialEq)]
pub struct FlameView {
    pub id: u32,
    pub pos: [f32; 3],
    pub player: u8,
    pub visible: bool,
}

/// S14: a burning patch of ground.
#[derive(Clone, Debug, PartialEq)]
pub struct PatchView {
    pub id: u32,
    pub pos: [f32; 3],
    pub player: u8,
    pub ticks_left: u32,
    pub visible: bool,
}

/// A smoke cloud (a cylinder standing at `pos`, as tall as it is wide).
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct SmokeView {
    pub id: u32,
    pub pos: [f32; 3],
    pub radius: f32,
    /// 1 while it blocks every sight line, falling to 0 as it thins.
    pub density: f32,
    pub visible: bool,
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
    /// Per soldier (S1b): hit points, suppression (0..=100), pinned.
    pub hp: Vec<u16>,
    pub suppression: Vec<u8>,
    pub pinned: Vec<bool>,
    /// S3b Fix: ducked below a trench lip (not firing).
    pub ducked: Vec<bool>,
    /// Standing in a dug trench (feet at least the sim's trench depth below
    /// the undug surface).
    pub in_trench: Vec<bool>,
    /// Per soldier (S1b): `StanceOrder as u8` (0 auto, 1 stand, 2 crouch,
    /// 3 prone), detached, items by [`ItemKind::index`], winding up a
    /// throw, and his formation slot (stable for his life; see
    /// [`FrameCtx`]).
    pub stance_order: Vec<u8>,
    pub detached: Vec<bool>,
    pub items: Vec<[u8; ITEM_KINDS]>,
    pub throwing: Vec<bool>,
    pub slot: Vec<u8>,
    /// Per soldier (S4): his weapon (index into `MatchSetup::weapons`) and
    /// `[[members]]` slot (index into his squad type's `members`).
    pub weapon: Vec<u16>,
    pub member_slot: Vec<u8>,
    /// Per soldier (S5a): the weapon in hand, rounds held for it
    /// (`u16::MAX`: uncounted), on his sidearm, fire mode (`FireMode as
    /// u8`).
    pub active_weapon: Vec<u16>,
    pub rounds_held: Vec<u16>,
    pub sidearm: Vec<bool>,
    pub fire: Vec<u8>,
    /// The local player's detached men, ascending id.
    pub units: Vec<UnitView>,
    pub thrown: Vec<ThrownView>,
    pub smoke: Vec<SmokeView>,
    /// Bodies, and (S12) downed men drawn as bodies (`ticks_left` 0),
    /// ascending id.
    pub corpses: Vec<CorpseSnapshot>,
    /// S12: downed men, ascending id.
    pub casualties: Vec<CasualtySnapshot>,
    /// S12: per soldier, `frontline_sim::wound_bits`.
    pub wound: Vec<u8>,
    /// S14: per soldier, he burns.
    pub burning: Vec<bool>,
    /// S14: per soldier, his strike phase (0 none, 1 wind-up, 2 recovery).
    pub strike: Vec<u8>,
    /// S14: flame particles and burning ground, ascending id, with
    /// whether the local player sees them.
    pub flame: Vec<FlameView>,
    pub patches: Vec<PatchView>,
    /// Per corpse: the local player sees the body (own: always).
    pub corpse_visible: Vec<bool>,
    /// Small-arms rounds and mortar shells (main-gun shells and rockets
    /// are in `shells`).
    pub rounds: Vec<RoundView>,
    /// S3: vehicles and wrecks the local player may know of, ascending id
    /// (module docs): enemies' own-only fields are zeroed.
    pub vehicles: Vec<VehicleSnapshot>,
    /// S3: main-gun shells and rockets he may see, slot order.
    pub shells: Vec<ShellSnapshot>,
    pub squads: Vec<SquadView>,
    pub players: Vec<PlayerSnapshot>,
    pub points: Vec<PointSnapshot>,
    pub outcome: Outcome,
    pub extra: Vec<PlayerExtra>,
    /// A staged scene (`--autoplay_scene`): its snapshot condition holds
    /// this tick (`frontline_sim::scenarios::scene_ready`), and the first
    /// tick it held (kept, so a one-tick condition is not lost when the
    /// presenter skips frames).
    pub scene_ready: bool,
    pub scene_ready_at: Option<u64>,
    /// S5b: the fire-mission tool's answer and the local player's
    /// knowledge map, while the tool asks (module docs).
    pub preview: Option<PreviewView>,
    pub intel: Vec<IntelView>,
    /// S6: HQs and dumps (own; enemy ones within sight of an own man),
    /// and crates (own; enemy set-down ones within sight), ascending id;
    /// a set-down crate's z is the ground there.
    pub structures: Vec<frontline_sim::StructureSnapshot>,
    pub crates: Vec<frontline_sim::CrateSnapshot>,
    /// S10a: pieces, wire, sites and charges: own ones, and enemy ones
    /// within sight of an own man (a site or charge: its own spot; wire
    /// and pieces: either end or the centre).
    pub fort: frontline_sim::FortSnapshot,
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
            hp: Vec::new(),
            suppression: Vec::new(),
            pinned: Vec::new(),
            ducked: Vec::new(),
            in_trench: Vec::new(),
            stance_order: Vec::new(),
            detached: Vec::new(),
            items: Vec::new(),
            throwing: Vec::new(),
            slot: Vec::new(),
            weapon: Vec::new(),
            member_slot: Vec::new(),
            active_weapon: Vec::new(),
            rounds_held: Vec::new(),
            sidearm: Vec::new(),
            fire: Vec::new(),
            units: Vec::new(),
            thrown: Vec::new(),
            smoke: Vec::new(),
            corpses: Vec::new(),
            casualties: Vec::new(),
            wound: Vec::new(),
            burning: Vec::new(),
            strike: Vec::new(),
            flame: Vec::new(),
            patches: Vec::new(),
            corpse_visible: Vec::new(),
            rounds: Vec::new(),
            vehicles: Vec::new(),
            shells: Vec::new(),
            squads: Vec::new(),
            players: Vec::new(),
            points: Vec::new(),
            outcome: Outcome::Running,
            extra: Vec::new(),
            scene_ready: false,
            scene_ready_at: None,
            preview: None,
            intel: Vec::new(),
            structures: Vec::new(),
            crates: Vec::new(),
            fort: frontline_sim::FortSnapshot::default(),
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
        // S8a: men aboard a transport are drawn seated (crouched at the
        // bottom of their slot).
        self.stance.extend(
            snap.soldier_stance
                .iter()
                .zip(
                    snap.soldier_riding
                        .iter()
                        .chain(std::iter::repeat(&u32::MAX)),
                )
                .map(|(s, r)| {
                    if *r != u32::MAX {
                        frontline_sim::Stance::Crouch as u8
                    } else {
                        *s as u8
                    }
                }),
        );
        self.visible.clear();
        self.visible
            .extend(snap.soldier_visible.iter().map(|v| v & bit != 0));
        self.hp.clone_from(&snap.soldier_hp);
        self.suppression.clone_from(&snap.soldier_suppression);
        self.pinned.clone_from(&snap.soldier_pinned);
        self.ducked.clone_from(&snap.soldier_ducked);
        self.stance_order.clear();
        self.stance_order
            .extend(snap.soldier_stance_order.iter().map(|s| *s as u8));
        self.detached.clone_from(&snap.soldier_detached);
        self.items.clone_from(&snap.soldier_items);
        self.weapon.clone_from(&snap.soldier_weapon);
        self.member_slot.clone_from(&snap.soldier_member_slot);
        self.active_weapon.clone_from(&snap.soldier_active_weapon);
        self.rounds_held.clone_from(&snap.soldier_rounds);
        self.sidearm.clone_from(&snap.soldier_sidearm);
        self.fire.clone_from(&snap.soldier_fire);
        self.wound.clone_from(&snap.soldier_wound);
        self.burning.clone_from(&snap.soldier_burning);
        self.strike.clone_from(&snap.soldier_strike);
        self.throwing.clone_from(&snap.soldier_throwing);
        self.units.clear();
        self.units.extend(
            snap.units
                .iter()
                .filter(|u| u.owner == local)
                .map(|u| UnitView {
                    id: u.id.0,
                    order: u.order,
                    target: u.target,
                    follow: None,
                }),
        );
        let ents = world.entities();
        for u in &mut self.units {
            if let UnitOrder::Follow { vehicle } = ents.unit()[u.id as usize].order {
                u.follow = Some(vehicle.0);
            }
        }
        self.in_trench.clear();
        self.in_trench.extend(
            self.ids
                .iter()
                .map(|&id| ents.combat()[id as usize].in_trench),
        );
        let vel = ents.velocity();
        self.speeds.clear();
        for &id in &self.ids {
            let [vx, vy] = vel[id as usize].lin.to_f32();
            self.speeds.push((vx * vx + vy * vy).sqrt());
        }
        // The sim spots corpses like prone men.
        self.corpses.clone_from(&snap.corpses);
        // S12 F2 (review R5): a man who lay downed already fell; his body
        // keeps lying (`ticks_left` 0, the fall's end) instead of falling
        // again.
        if !ctx.downed.is_empty() {
            for c in &mut self.corpses {
                if ctx.downed.contains(&c.id.0) {
                    c.ticks_left = 0;
                }
            }
        }
        // S12: a downed man lies as a body (ascending id kept).
        self.casualties.clone_from(&snap.casualties);
        self.corpses
            .extend(snap.casualties.iter().map(|c| CorpseSnapshot {
                id: c.id,
                pos: c.pos,
                owner: c.owner,
                ticks_left: 0,
                seen_by: c.seen_by,
            }));
        self.corpses.sort_by_key(|c| c.id);
        // Downed now, or a body that was (kept while the body lies).
        ctx.downed = self
            .corpses
            .iter()
            .filter(|c| {
                c.ticks_left == 0
                    && (ctx.downed.contains(&c.id.0)
                        || self
                            .casualties
                            .binary_search_by_key(&c.id, |k| k.id)
                            .is_ok())
            })
            .map(|c| c.id.0)
            .collect();
        self.corpse_visible.clear();
        for c in &self.corpses {
            self.corpse_visible
                .push(c.owner == local || c.seen_by & bit != 0);
        }

        let combat = ents.combat();
        let kind = ents.kind();
        let seen_by = |id: u32| -> bool {
            let i = id as usize;
            i < combat.len() && kind[i] != EntityKind::Vacant && combat[i].seen_by & bit != 0
        };
        self.rounds.clear();
        for p in world.projectiles().slots().iter().flatten() {
            if p.ammo != NO_AMMO {
                continue;
            }
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

        // Own men's positions (sim xy) for the thrown-item and smoke fog.
        let own: Vec<[f32; 2]> = self
            .soldiers
            .iter()
            .zip(&self.owner)
            .filter(|(_, o)| **o == local)
            .map(|(p, _)| [p[0], p[1]])
            .collect();
        let near = |p: [f32; 3], r: f32| {
            let r2 = r * r;
            own.iter().any(|o| {
                let (dx, dy) = (o[0] - p[0], o[1] - p[1]);
                dx * dx + dy * dy <= r2
            })
        };
        self.thrown.clear();
        if let Some(game) = world.game() {
            for (t, ts) in game.thrown().iter().zip(&snap.thrown) {
                // The sim flies the exact parabola from the release; its
                // slope now is the release velocity minus g·t.
                let [vx, vy, vz] = t.vel.to_f32();
                let flown = t.step as f32 / SUBSTEP_HZ as f32;
                self.thrown.push(ThrownView {
                    id: ts.id,
                    kind: ts.kind,
                    owner: ts.owner,
                    pos: ts.pos,
                    vel: if ts.landed {
                        [0.0; 3]
                    } else {
                        [vx, vy, vz - 9.81 * flown]
                    },
                    landed: ts.landed,
                    visible: ts.owner == local
                        || seen_by(t.thrower.0)
                        || near(ts.pos, THROWN_NEAR_M),
                });
            }
        }
        self.smoke.clear();
        for c in &snap.smoke {
            self.smoke.push(SmokeView {
                id: c.id,
                pos: c.pos,
                radius: c.radius,
                density: c.density,
                visible: near(c.pos, ctx.sight_m + c.radius),
            });
        }

        self.flame.clear();
        for c in &snap.flame {
            self.flame.push(FlameView {
                id: c.id,
                pos: c.pos,
                player: c.player,
                visible: c.player == local || near(c.pos, ctx.sight_m),
            });
        }
        self.patches.clear();
        for c in &snap.patches {
            self.patches.push(PatchView {
                id: c.id,
                pos: c.pos,
                player: c.player,
                ticks_left: c.ticks_left,
                visible: c.player == local || near(c.pos, ctx.sight_m + 6.0),
            });
        }

        self.fill_vehicles(&snap.vehicles, &snap.shells, ctx, &near, &seen_by);
        // S6: structures and crates the local player knows of.
        self.structures.clear();
        for st in &snap.structures {
            if st.owner == local || near([st.pos[0], st.pos[1], 0.0], ctx.sight_m) {
                self.structures.push(st.clone());
            }
        }
        self.crates.clear();
        for c in &snap.crates {
            let mut c = c.clone();
            if c.carrier == u32::MAX {
                c.pos[2] = world
                    .terrain()
                    .height_at(frontline_sim::FxVec2::new(
                        frontline_sim::Fx::from_raw((c.pos[0] * 65536.0) as i32),
                        frontline_sim::Fx::from_raw((c.pos[1] * 65536.0) as i32),
                    ))
                    .to_f32();
            }
            let carried_seen = c.carrier != u32::MAX && (c.owner == local || seen_by(c.carrier));
            if c.owner == local
                || carried_seen
                || (c.carrier == u32::MAX && near(c.pos, ctx.sight_m))
            {
                self.crates.push(c);
            }
        }
        // S10a: fortifications the local player knows of.
        let sf = &snap.fort;
        let fort = &mut self.fort;
        fort.pieces.clear();
        fort.pieces.extend(
            sf.pieces
                .iter()
                .filter(|q| q.owner == local || near(q.center, ctx.sight_m))
                .cloned(),
        );
        fort.wire.clear();
        fort.wire.extend(
            sf.wire
                .iter()
                .filter(|w| w.owner == local || near(w.a, ctx.sight_m) || near(w.b, ctx.sight_m))
                .cloned(),
        );
        fort.works.clear();
        fort.works.extend(
            sf.works
                .iter()
                .filter(|w| w.owner == local || near(w.site, ctx.sight_m))
                .cloned(),
        );
        fort.charges.clear();
        fort.charges.extend(
            sf.charges
                .iter()
                .filter(|c| c.owner == local || near(c.from, ctx.sight_m))
                .cloned(),
        );
        // S10b: mines the local player knows (`known` holds the owner's team
        // and every finder's team; all once one has gone off, R13).
        fort.mines.clear();
        fort.mines.extend(
            sf.mines
                .iter()
                .filter(|m| local < 8 && m.known & (1u8 << local) != 0)
                .cloned(),
        );
        self.scene_ready = ctx
            .scene
            .as_deref()
            .is_some_and(|name| frontline_sim::scenarios::scene_ready(name, world));
        if self.scene_ready && ctx.scene_ready_at.is_none() {
            ctx.scene_ready_at = Some(snap.tick);
        }
        self.scene_ready_at = ctx.scene_ready_at;

        self.squads.clear();
        self.players.clone_from(&snap.players);
        self.points.clone_from(&snap.points);
        self.outcome = snap.outcome;
        self.slot.clear();
        let Some(game) = world.game() else {
            self.slot.resize(self.ids.len(), 0);
            return;
        };
        let setup = game.setup();
        ctx.slots_prev.clear();
        std::mem::swap(&mut ctx.slots, &mut ctx.slots_prev);
        for (sid, sq) in game.squads() {
            // A vehicle's squad of one is in `vehicles`, not here.
            if sq.members.is_empty() || game.is_vehicle_squad(*sid) {
                continue;
            }
            let lead = sq.lead(ents);
            assign_slots(&sq.members, &ctx.slots_prev, &mut ctx.slots);
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
            // S5a: ammunition share, crew weapon rounds, held fire, refill.
            if sq.owner.0 == local {
                v.ammo_pct = frontline_sim::supply::squad_ammo_pct(setup, ents, sq)
                    .map(|p| p.min(100) as u8);
                v.crew = frontline_sim::supply::crew_rounds(setup, ents, sq);
                let base = &setup.map.bases[setup.players[sq.owner.0 as usize].base as usize];
                let r = frontline_sim::Fx::from(base.radius).to_f32();
                let c = frontline_sim::FxVec2::from(base.center).to_f32();
                let a = ents.transform()[lead.index()].pos.to_f32();
                let (dx, dy) = (a[0] - c[0], a[1] - c[1]);
                v.refilling = setup.rules.supply.is_some()
                    && v.ammo_pct.is_some_and(|p| p < 100)
                    && dx * dx + dy * dy <= r * r;
                v.materials_kg = game
                    .fort()
                    .carried_g
                    .get(sid)
                    .map_or(0.0, |&g| g as f32 / 1000.0);
            }
            for m in &sq.members {
                let c = &combat[m.index()];
                v.detached += ents.unit()[m.index()].detached as u8;
                v.holding += (ents.unit()[m.index()].fire == frontline_sim::FireMode::Hold) as u8;
                for (n, k) in v.items.iter_mut().zip(ents.kit()[m.index()].items) {
                    *n += k as u16;
                }
                v.hp_sum += c.hp as u32;
                v.mounted += ents.unit()[m.index()].riding.is_some() as u8;
                v.suppression_sum += (c.suppression / 1000).min(100);
                v.pinned += c.pinned as u8;
                v.ducked += c.ducked as u8;
                v.visible |= c.seen_by & bit != 0;
            }
            let w = &setup.weapons[ty.weapon as usize];
            v.setup = setup.type_word(sq.type_idx) == "mg"
                && sq.nav.phase == NavPhase::Holding
                && (combat[lead.index()].still as u32) < w.setup_ticks;
            match &sq.order {
                Order::Move { target } | Order::AttackMove { target } => {
                    v.target = Some(target.to_f32());
                }
                Order::Dig { line, job } => {
                    v.dig_pct = job.progress_pct(job.rules(&setup.rules), setup.tick_hz);
                    v.dig_line.extend(line.iter().map(|p| p.to_f32()));
                }
                Order::FireMission { target, rounds, m } => {
                    v.target = Some(target.to_f32());
                    v.rounds = *rounds;
                    if sq.owner.0 == local && m.id != 0 {
                        let pos = |id: frontline_sim::SimId| {
                            let p = ents.transform()[id.index()].pos.to_f32();
                            [p[0], p[1]]
                        };
                        v.mission = Some(MissionView {
                            mode: m.mode,
                            phase: m.phase,
                            error_m: m.offset.length().to_f32(),
                            first_error_m: m.err0.to_f32(),
                            seen: m.seen,
                            fired: m.fired,
                            rounds_left: *rounds,
                            adjust: m.adjust,
                            observer: m.observer.map(|o| o.0),
                            observer_pos: m
                                .observer
                                .filter(|o| {
                                    ents.kind()[o.index()] == EntityKind::Soldier
                                        && combat[o.index()].hp > 0
                                })
                                .map(pos),
                            gunner_pos: game.mortar_gunner(ents, sq).map(pos),
                            charge: m.charge,
                            flight_s: m.flight as f32 / setup.tick_hz.max(1) as f32,
                            ranged: m.ranged,
                        });
                    }
                }
                Order::Follow { vehicle } => v.follow = Some(vehicle.0),
                Order::Suppress { line } => {
                    v.suppress_line = Some([line[0].to_f32(), line[1].to_f32()]);
                }
                Order::Recrew { vehicle } => v.follow = Some(vehicle.0),
                // S8a: the transport it walks to or rides in.
                Order::Embark { vehicle } => v.follow = Some(vehicle.0),
                // S10a: building or cutting (Build C draws sites and wire).
                // S10b: the sweep lane and the demolition target.
                Order::ClearMines { a, b, along, .. } => {
                    v.clear_lane = Some((a.to_f32(), b.to_f32(), along.to_f32()));
                }
                Order::Demolish { piece, .. } => v.demolish = Some(*piece),
                Order::Idle | Order::Retreat | Order::Fortify { .. } | Order::CutWire { .. } => {}
            }
            self.squads.push(v);
        }
        self.slot.extend(
            self.ids
                .iter()
                .map(|id| ctx.slots.get(id).copied().unwrap_or(0)),
        );
        ctx.track(&self.squads, setup.players.len());
        self.extra.clone_from(&ctx.extra);
        self.fill_preview(world, ctx);
    }

    /// S5b: the fire-mission tool's preview and the knowledge map, while the
    /// presenter asks for them (module docs).
    fn fill_preview(&mut self, world: &World, ctx: &mut FrameCtx) {
        self.preview = None;
        self.intel.clear();
        let Some(req) = ctx.preview.get() else {
            return;
        };
        ctx.preview_gen = ctx.preview.generation.load(Ordering::Relaxed);
        let Some(game) = world.game() else { return };
        let ents = world.entities();
        let id = frontline_sim::SquadId(req.squad);
        let point = crate::intents::sim_to_fx(req.point);
        let own = game.squad(id).is_some_and(|q| q.owner.0 == ctx.local);
        if own
            && let Some(p) =
                game.fire_preview(ents, world.terrain(), id, point, req.rounds, world.tick())
        {
            let pos = |o: frontline_sim::SimId| {
                let v = ents.transform()[o.index()].pos.to_f32();
                [v[0], v[1]]
            };
            let sq = game.squad(id).expect("own");
            let gunner = game
                .mortar_gunner(ents, sq)
                .unwrap_or_else(|| sq.lead(ents));
            self.preview = Some(PreviewView {
                req,
                refusal: p.refusal,
                mode: p.mode,
                observer: p.observer.map(|o| o.0),
                observer_pos: p.observer.map(pos),
                gunner_pos: pos(gunner),
                error_m: p.error.to_f32(),
                ring_m: p.ring.to_f32(),
                ranged: p.ranged,
                walk: p.walk,
                range_m: p.range.to_f32(),
            });
        }
        let setup = game.setup();
        if let (Some(fm), Some(pl)) = (
            setup.rules.fire_mission.as_ref(),
            game.players().get(ctx.local as usize),
        ) {
            let cols = frontline_sim::fire_mission::intel_cols(setup.map.size_m, fm.intel_cell_m);
            let cell = fm.intel_cell_m as f32;
            let hz = setup.tick_hz.max(1) as f32;
            for (&k, c) in &pl.intel {
                let (x, y) = (k % cols, k / cols);
                self.intel.push(IntelView {
                    pos: [(x as f32 + 0.5) * cell, (y as f32 + 0.5) * cell],
                    age_s: world.tick().saturating_sub(c.seen) as f32 / hz,
                    trench: c.flags & frontline_sim::fire_mission::intel_flag::TRENCH != 0,
                });
            }
        }
    }

    /// The fog-filtered `vehicles` and `shells` (module docs). `near` says
    /// whether a point is within a distance of the local player's men,
    /// `seen_by` whether he sees an entity.
    fn fill_vehicles(
        &mut self,
        vehicles: &[VehicleSnapshot],
        shells: &[ShellSnapshot],
        ctx: &mut FrameCtx,
        near: &dyn Fn([f32; 3], f32) -> bool,
        seen_by: &dyn Fn(u32) -> bool,
    ) {
        let local = ctx.local;
        let bit = 1u16 << local;
        // Who the local player saw the tick before (armour events of a
        // vehicle knocked out this tick: it is a wreck now).
        std::mem::swap(&mut ctx.seen_vehicles, &mut ctx.seen_vehicles_prev);
        ctx.seen_vehicles.clear();
        self.vehicles.clear();
        for v in vehicles {
            let wreck = v.flags & vehicle_flags::WRECK != 0;
            // S3c: an abandoned vehicle is an object anyone may inspect
            // (its damage, crew still inside), like a wreck.
            let abandoned = v.flags & vehicle_flags::ABANDONED != 0;
            if wreck || abandoned || v.owner == local {
                ctx.seen_vehicles.insert(v.id);
                self.vehicles.push(v.clone());
            } else if v.seen_by & bit != 0 {
                ctx.seen_vehicles.insert(v.id);
                self.vehicles.push(enemy_view(v, local));
            }
        }
        self.shells.clear();
        self.shells.extend(
            shells
                .iter()
                .filter(|s| s.player == local || seen_by(s.shooter) || near(s.pos, THROWN_NEAR_M))
                .cloned(),
        );
    }
}

/// An enemy vehicle as the local player may know it: pose, speed and the
/// public flags; the own-only fields zeroed (`repairing` and `target` take
/// their "none" values, since 0 is a real component and id).
pub fn enemy_view(v: &VehicleSnapshot, local: u8) -> VehicleSnapshot {
    VehicleSnapshot {
        id: v.id,
        squad: v.squad,
        type_idx: v.type_idx,
        owner: v.owner,
        pos: v.pos,
        yaw: v.yaw,
        pitch: v.pitch,
        roll: v.roll,
        turret_yaw: v.turret_yaw,
        gun_pitch: v.gun_pitch,
        speed: v.speed,
        flags: v.flags & vehicle_flags::PUBLIC,
        seen_by: 1 << local,
        repairing: NONE_U8,
        target: u32::MAX,
        // S8a: men over the rim and the MG are seen with the vehicle.
        riders: v.riders,
        capacity: v.capacity,
        unload: v.unload,
        mg_yaw: v.mg_yaw,
        // S9: a gun's state and its crew are seen with it.
        gun_state: v.gun_state,
        gun_men: v.gun_men,
        gun_slots: v.gun_slots,
        hitched: v.hitched,
        towing: v.towing,
        ..VehicleSnapshot::default()
    }
}

/// Per-weapon presenter constants, precomputed from the setup.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct WeaponInfo {
    pub he: bool,
    /// Substeps a round lives (as the sim's `round_ttl`), for its age.
    pub ttl0: f32,
}

/// Gives each member of a squad a formation slot that stays his for life:
/// men already known keep theirs (from `prev`), newcomers take the lowest
/// free slots in member order. A fresh squad's slots follow `members`.
pub fn assign_slots(
    members: &[frontline_sim::SimId],
    prev: &BTreeMap<u32, u8>,
    out: &mut BTreeMap<u32, u8>,
) {
    let mut used = 0u128;
    let mut fresh = Vec::new();
    for m in members {
        match prev.get(&m.0) {
            Some(&s) if s < 128 && used & (1 << s) == 0 => {
                used |= 1 << s;
                out.insert(m.0, s);
            }
            _ => fresh.push(m.0),
        }
    }
    for id in fresh {
        let s = (!used).trailing_zeros().min(127) as u8;
        used |= 1 << s;
        out.insert(id, s);
    }
}

/// Sim-thread state carried across ticks while filling frames.
#[derive(Clone, Debug, Default)]
pub struct FrameCtx {
    pub local: u8,
    pub weapons: Vec<WeaponInfo>,
    /// Longest sight range (m) of the local faction's squad types (smoke
    /// cloud fog).
    pub sight_m: f32,
    /// Soldier id → formation slot, this tick and the last.
    slots: BTreeMap<u32, u8>,
    slots_prev: BTreeMap<u32, u8>,
    /// Squads seen so far: owner, dig line length (m) and progress.
    squads: BTreeMap<u32, (u8, f32, u8)>,
    /// Vehicles in the local player's view this tick and the last
    /// (armour-event fog).
    seen_vehicles: BTreeSet<u32>,
    seen_vehicles_prev: BTreeSet<u32>,
    /// S12 F2 (review R5): downed men at the last frame and the bodies of
    /// those who died since (their fall is over).
    downed: BTreeSet<u32>,
    /// A staged scene's name (`frontline_sim::scenarios::SCENES`): frames
    /// carry whether its condition holds.
    pub scene: Option<String>,
    scene_ready_at: Option<u64>,
    started: bool,
    pub extra: Vec<PlayerExtra>,
    /// S5b: the fire-mission tool's request (shared with the presenter)
    /// and the request generation the last frame answered.
    pub preview: Arc<PreviewShared>,
    pub preview_gen: u64,
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
        let faction = setup.players.get(local as usize).map(|p| p.faction);
        let sight_m = setup
            .squad_types
            .iter()
            .filter(|t| Some(t.faction) == faction)
            .map(|t| frontline_sim::Fx::from(t.sight).to_f32())
            .fold(150.0f32, f32::max);
        FrameCtx {
            local,
            weapons,
            sight_m,
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
    /// The soldier or vehicle that fired.
    pub shooter: u32,
    /// S3: the shooter's vehicle type when a vehicle fired.
    pub vehicle_type: Option<u16>,
    /// The local player sees the shooter (own: always).
    pub visible: bool,
}

/// An armour event the local player may know of, with the vehicle's type
/// (the event itself has only the id).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ArmourView {
    pub ev: ArmourEvent,
    pub type_idx: u16,
    /// S3c: crewmen still inside the vehicle when it happened (an
    /// abandonment's "n of m bailing out").
    pub inside: u8,
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
    /// S3: the armour events the local player may know of (module docs),
    /// in sim order.
    pub armour: Vec<ArmourView>,
    /// S3b: the local player's call-in results this tick (spawned or
    /// refused, with the reason), in command order.
    pub call_ins: Vec<frontline_sim::CallInEvent>,
    /// S3c: the local player's refused re-crews this tick.
    pub recrew_refusals: Vec<frontline_sim::RecrewEvent>,
    /// S5b: the local player's refused fire missions this tick.
    pub fire_mission_refusals: Vec<frontline_sim::FireMissionEvent>,
    /// S4: crew weapons the local player's men picked up this tick.
    pub pickups: Vec<PickupView>,
    /// S6: the local player's refused build and crate orders this tick.
    pub supply_refusals: Vec<frontline_sim::SupplyEvent>,
    /// S10a: the local player's refused fortification orders this tick.
    pub fort_refusals: Vec<frontline_sim::fort::FortEvent>,
}

/// S4: an own man picked up a fallen man's crew weapon.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PickupView {
    /// Index into `MatchSetup::weapons`.
    pub weapon: u16,
    /// His squad type and `[[members]]` slot (his role word).
    pub squad_type: u16,
    pub member_slot: u8,
    /// Sim xy where he stands.
    pub pos: [f32; 2],
}

impl TickEvents {
    pub fn is_empty(&self) -> bool {
        self.shots.is_empty()
            && self.explosions.is_empty()
            && self.tiles.is_empty()
            && self.armour.is_empty()
            && self.call_ins.is_empty()
            && self.recrew_refusals.is_empty()
            && self.fire_mission_refusals.is_empty()
            && self.pickups.is_empty()
            && self.supply_refusals.is_empty()
            && self.fort_refusals.is_empty()
    }

    /// This tick's events of `world` as seen by `ctx.local`. Call after
    /// [`MatchFrame::fill`] for the same tick (it updates the vehicles the
    /// player sees).
    pub fn collect(world: &World, ctx: &FrameCtx) -> TickEvents {
        let local = ctx.local;
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
                shooter: s.shooter.0,
                vehicle_type: world
                    .game()
                    .and_then(|g| g.vehicle(s.shooter))
                    .map(|v| v.type_idx),
                visible: player == local || seen,
            });
        }
        ev.explosions.clone_from(&snap.explosions);
        if !snap.armour_events.is_empty() {
            let own: Vec<[f32; 2]> = snap
                .soldiers
                .iter()
                .zip(&snap.soldier_owner)
                .filter(|(_, o)| **o == local)
                .map(|(p, _)| [p[0], p[1]])
                .collect();
            let r2 = THROWN_NEAR_M * THROWN_NEAR_M;
            ev.armour.extend(
                snap.armour_events
                    .iter()
                    .filter(|e| {
                        let wreck_now = snap
                            .vehicles
                            .binary_search_by_key(&e.vehicle, |v| v.id)
                            .is_ok_and(|k| snap.vehicles[k].owner == WRECK_OWNER);
                        e.owner == local
                            || (ctx.seen_vehicles.contains(&e.vehicle) && !wreck_now)
                            || ctx.seen_vehicles_prev.contains(&e.vehicle)
                            || own.iter().any(|o| {
                                let (dx, dy) = (o[0] - e.pos[0], o[1] - e.pos[1]);
                                dx * dx + dy * dy <= r2
                            })
                    })
                    .map(|e| {
                        let v = snap
                            .vehicles
                            .binary_search_by_key(&e.vehicle, |v| v.id)
                            .ok()
                            .map(|k| &snap.vehicles[k]);
                        ArmourView {
                            ev: e.clone(),
                            type_idx: v.map_or(0, |v| v.type_idx),
                            inside: v.map_or(0, |v| v.inside),
                        }
                    }),
            );
        }
        for &(x, y) in &snap.dirty_tiles {
            let mut h = Vec::new();
            let heights = tile_heights_f32(world.terrain(), x, y, &mut h).then_some(h);
            ev.tiles.push(TileUpdate { x, y, heights });
        }
        ev.call_ins.extend(
            snap.call_ins
                .iter()
                .filter(|c| c.player.0 == local)
                .copied(),
        );
        ev.recrew_refusals.extend(
            snap.recrew_refusals
                .iter()
                .filter(|c| c.player.0 == local)
                .copied(),
        );
        ev.supply_refusals.extend(
            snap.supply_refusals
                .iter()
                .filter(|c| c.player.0 == local)
                .copied(),
        );
        ev.fort_refusals.extend(
            snap.fort_refusals
                .iter()
                .filter(|c| c.player.0 == local)
                .copied(),
        );
        ev.fire_mission_refusals.extend(
            snap.fire_mission_refusals
                .iter()
                .filter(|c| c.player.0 == local)
                .copied(),
        );
        // S4: own crew-weapon pickups.
        if let Some(g) = world.game() {
            for p in &snap.pickups {
                let Some(sq) = g.squad(p.squad).filter(|q| q.owner.0 == local) else {
                    continue;
                };
                let k = ents.kit()[p.man.index()];
                let pos = ents.transform()[p.man.index()].pos.to_f32();
                ev.pickups.push(PickupView {
                    weapon: p.weapon,
                    squad_type: sq.type_idx,
                    member_slot: k.slot,
                    pos: [pos[0], pos[1]],
                });
            }
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
    use frontline_sim::{Command, CommandKind, Fx, FxVec2, PlayerId};

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
        // S3b: the squad spawned at once and its type's cooldown runs.
        let t = setup.factions[setup.players[0].faction as usize].roster[0];
        let left = f.players[0].cooldowns[t as usize];
        assert!(
            left > 0 && left <= setup.squad_types[t as usize].recall_cooldown_ticks,
            "{left}"
        );
        assert_eq!(f.extra[0].squads_called, 1);
        assert_eq!(f.extra[1].squads_called, f.extra[1].squads_called);
    }

    /// S14: the frame copies the flame fields one to one; with every
    /// switch off (the shipped rules) there is no particle or patch, and
    /// burning and strike hold one quiet entry per soldier.
    #[test]
    fn frame_carries_the_s14_fields_quiet_when_off() {
        let mut w = test_world(false);
        let setup = w.match_setup().unwrap().clone();
        let mut ctx = FrameCtx::new(&setup, 0);
        let mut f = MatchFrame::empty();
        for _ in 0..30 {
            w.step(&[]);
        }
        f.fill(&w, &mut ctx, 0.0, Instant::now());
        assert!(f.flame.is_empty() && f.patches.is_empty());
        assert_eq!(f.burning.len(), f.ids.len());
        assert_eq!(f.strike.len(), f.ids.len());
        assert!(f.burning.iter().all(|b| !b) && f.strike.iter().all(|s| *s == 0));
    }

    /// S10b review R14: the frame keeps the mines the local player knows
    /// (own, and an enemy one his side found) and drops an unknown enemy
    /// mine, wherever it lies. Control: drop the `known` filter in `fill`
    /// -> the unknown mine shows.
    #[test]
    fn frame_drops_unknown_enemy_mines() {
        let mut w = test_world(false);
        let setup = w.match_setup().unwrap().clone();
        let ap = setup
            .forts
            .iter()
            .position(|f| f.id == "ww2:ap_minefield")
            .unwrap() as u16;
        let c = |x: i32| FxVec2::from_int(x, 300);
        let own = w.lay_mine_now(ap, c(300), PlayerId(0)).unwrap();
        let found = w.lay_mine_now(ap, c(310), PlayerId(1)).unwrap();
        let hidden = w.lay_mine_now(ap, c(320), PlayerId(1)).unwrap();
        assert!(w.edit_fort(|f| f.mines.get_mut(&found).unwrap().known |= 1));
        let mut ctx = FrameCtx::new(&setup, 0);
        let mut f = MatchFrame::empty();
        f.fill(&w, &mut ctx, 0.0, Instant::now());
        let ids: Vec<u32> = f.fort.mines.iter().map(|m| m.id).collect();
        assert!(ids.contains(&own) && ids.contains(&found), "{ids:?}");
        assert!(!ids.contains(&hidden), "an unknown enemy mine: {ids:?}");
        // Player 1's frame shows all three (his own).
        let mut ctx1 = FrameCtx::new(&setup, 1);
        f.fill(&w, &mut ctx1, 0.0, Instant::now());
        let ids1: Vec<u32> = f.fort.mines.iter().map(|m| m.id).collect();
        assert!(ids1.contains(&found) && ids1.contains(&hidden), "{ids1:?}");
        assert!(!ids1.contains(&own), "{ids1:?}");
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
                profile: Default::default(),
            },
        }]);
        let mut tiles = 0;
        for _ in 0..30 * 120 {
            w.step(&[]);
            let ev = TickEvents::collect(&w, &FrameCtx::new(&setup, 0));
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
    fn frame_carries_soldier_orders_items_and_throws() {
        use frontline_sim::{ItemKind, SimId, StanceOrder};
        let mut w = test_world(false);
        let setup = w.match_setup().unwrap().clone();
        let mut ctx = FrameCtx::new(&setup, 0);
        let mut f = MatchFrame::empty();
        f.fill(&w, &mut ctx, 0.0, Instant::now());
        for v in [
            f.hp.len(),
            f.suppression.len(),
            f.pinned.len(),
            f.ducked.len(),
            f.in_trench.len(),
            f.stance_order.len(),
            f.detached.len(),
            f.items.len(),
            f.throwing.len(),
            f.slot.len(),
        ] {
            assert_eq!(v, f.ids.len());
        }
        // A rifle squad: slots 0..n in member order; the leader carries
        // the most items.
        let (sid, sq) = w
            .game()
            .unwrap()
            .squads()
            .iter()
            .find(|(_, s)| s.owner.0 == 0 && setup.type_word(s.type_idx) == "rifle")
            .map(|(id, s)| (*id, s.clone()))
            .unwrap();
        let idx = |f: &MatchFrame, id: SimId| f.ids.binary_search(&id.0).unwrap();
        for (k, m) in sq.members.iter().enumerate() {
            assert_eq!(f.slot[idx(&f, *m)] as usize, k);
        }
        let v = f.squads.iter().find(|s| s.id == sid.0).unwrap();
        let frags: u16 = sq
            .members
            .iter()
            .map(|m| f.items[idx(&f, *m)][ItemKind::Frag.index()] as u16)
            .sum();
        assert!(frags > 0);
        assert_eq!(v.items[ItemKind::Frag.index()], frags);

        // One man sent ahead and set prone; another throws smoke.
        let (lone, thrower) = (sq.members[3], sq.members[0]);
        let p = w.entities().transform()[lone.index()].pos;
        let ahead = FxVec2::new(p.x, p.y) + FxVec2::from_int(0, 40);
        let cmd = |seq, kind| Command {
            player: PlayerId(0),
            seq,
            kind,
        };
        w.step(&[
            cmd(
                0,
                CommandKind::MoveUnits {
                    units: vec![lone],
                    target: ahead,
                },
            ),
            cmd(
                1,
                CommandKind::SetStance {
                    units: vec![lone],
                    stance: StanceOrder::Prone,
                },
            ),
            cmd(
                2,
                CommandKind::Throw {
                    units: vec![thrower],
                    item: ItemKind::Smoke,
                    target: FxVec2::new(p.x, p.y) + FxVec2::from_int(15, 0),
                },
            ),
        ]);
        f.fill(&w, &mut ctx, 0.0, Instant::now());
        let i = idx(&f, lone);
        assert!(f.detached[i]);
        assert_eq!(f.stance_order[i], StanceOrder::Prone as u8);
        assert_eq!(f.squads.iter().find(|s| s.id == sid.0).unwrap().detached, 2);
        let u = f.units.iter().find(|u| u.id == lone.0).unwrap();
        assert_eq!(u.order, OrderKind::Move);
        assert_eq!(u.target, ahead.to_f32());
        assert!(f.throwing[idx(&f, thrower)]);
        // The smoke flies, lands and makes a cloud the owner sees.
        let mut flew = false;
        for _ in 0..30 * 10 {
            w.step(&[]);
            f.fill(&w, &mut ctx, 0.0, Instant::now());
            for t in &f.thrown {
                if t.kind == ItemKind::Smoke && t.visible && !t.landed {
                    flew = true;
                    let v = t.vel;
                    assert!((v[0] * v[0] + v[1] * v[1]).sqrt() > 1.0, "{v:?}");
                }
            }
            if !f.smoke.is_empty() {
                break;
            }
        }
        assert!(flew, "the smoke grenade was seen in flight");
        assert!(f.thrown.iter().all(|t| t.landed && t.vel == [0.0; 3]));
        assert!(f.smoke[0].visible);
        // Slots stay with the men while the squad changes.
        for (k, m) in sq.members.iter().enumerate() {
            assert_eq!(f.slot[idx(&f, *m)] as usize, k);
        }
    }

    #[test]
    fn slots_are_kept_for_life_and_refilled_lowest_first() {
        let ids = |v: &[u32]| {
            v.iter()
                .map(|&i| frontline_sim::SimId(i))
                .collect::<Vec<_>>()
        };
        let mut prev = BTreeMap::new();
        let mut out = BTreeMap::new();
        assign_slots(&ids(&[10, 11, 12, 13]), &prev, &mut out);
        assert_eq!(out.values().copied().collect::<Vec<_>>(), vec![0, 1, 2, 3]);
        // 11 dies, 20 joins: 20 takes slot 1, the rest keep theirs.
        std::mem::swap(&mut prev, &mut out);
        out.clear();
        assign_slots(&ids(&[10, 12, 13, 20]), &prev, &mut out);
        assert_eq!(out[&10], 0);
        assert_eq!(out[&12], 2);
        assert_eq!(out[&13], 3);
        assert_eq!(out[&20], 1);
    }

    /// A flat staging world (`frontline_sim::scenarios`) from the source
    /// tree's defs.
    fn staged() -> World {
        use crate::match_setup::{MenuChoice, find_mods_dir, mods_candidates, resolve};
        let mods = find_mods_dir(&mods_candidates(None, None)).unwrap();
        let mut setup = resolve(&mods, &MenuChoice::default()).unwrap();
        frontline_sim::scenarios::staging(&mut setup, true);
        World::new_match(setup, 2).unwrap()
    }

    /// S5b: the tool's preview answers only while asked (mode, ring,
    /// refusal), an own mission shows its state with the link line ends,
    /// the enemy's does not, and a refused order reaches the local events.
    #[test]
    fn fire_missions_previews_and_refusals_reach_the_frame() {
        use frontline_sim::fire_mission::{mode, phase};
        use frontline_sim::scenarios::{EAST, WEST, at, cmd};
        use frontline_sim::{CommandKind, FireMissionRefusal, PlayerId};
        let mut w = staged();
        let setup = w.match_setup().unwrap().clone();
        let t = setup.squad_type_index("ww2:ger_mortar_team").unwrap();
        let sq = w
            .spawn_squad_at(PlayerId(0), t, at(300, 512), EAST)
            .unwrap();
        let et = setup.squad_type_index("ww2:usa_mortar_team").unwrap();
        let enemy = w
            .spawn_squad_at(PlayerId(1), et, at(700, 600), WEST)
            .unwrap();
        let mut ctx = FrameCtx::new(&setup, 0);
        let mut f = MatchFrame::empty();
        w.step(&[]);
        f.fill(&w, &mut ctx, 0.0, Instant::now());
        assert!(f.preview.is_none() && f.intel.is_empty(), "nothing asked");
        // Unknown ground 438 m out (beyond the leader's 400 m), then a
        // capture point's ground.
        let ask = |pt: [f32; 2]| PreviewReq {
            squad: sq.0,
            point: pt,
            rounds: 6,
        };
        ctx.preview.set(Some(ask([300.0, 950.0])));
        f.fill(&w, &mut ctx, 0.0, Instant::now());
        let p = f.preview.expect("answered");
        assert_eq!(p.refusal, Some(FireMissionRefusal::UnknownGround));
        let cp = frontline_sim::FxVec2::from(setup.map.points[0].pos).to_f32();
        ctx.preview.set(Some(ask(cp)));
        f.fill(&w, &mut ctx, 0.0, Instant::now());
        let p = f.preview.expect("answered");
        assert_eq!(p.refusal, None, "{p:?}");
        assert!(p.ring_m >= p.error_m && p.error_m > 0.0, "{p:?}");
        // The enemy's squad is never previewed.
        ctx.preview.set(Some(PreviewReq {
            squad: enemy.0,
            point: cp,
            rounds: 6,
        }));
        f.fill(&w, &mut ctx, 0.0, Instant::now());
        assert!(f.preview.is_none());
        ctx.preview.set(None);
        // A Direct mission 250 m east (the leader sees it on flat ground):
        // the frame carries its state and the link line ends.
        let target = at(550, 512);
        let mut ev = Vec::new();
        for k in 0..2u32 {
            let kind = CommandKind::FireMission {
                squad: sq,
                // The first at unknown ground far off: refused.
                target: if k == 0 { at(300, 950) } else { target },
                rounds: 4,
            };
            w.step(&[cmd(0, k, kind)]);
            f.fill(&w, &mut ctx, 0.0, Instant::now());
            ev.push(TickEvents::collect(&w, &ctx));
        }
        assert_eq!(ev[0].fire_mission_refusals.len(), 1);
        assert_eq!(
            ev[0].fire_mission_refusals[0].refusal,
            FireMissionRefusal::UnknownGround
        );
        let v = f.squads.iter().find(|s| s.id == sq.0).unwrap();
        assert_eq!(v.order, frontline_sim::OrderKind::FireMission);
        let m = v.mission.expect("own mission");
        assert_eq!(
            (m.mode, m.phase, m.rounds_left),
            (mode::DIRECT, phase::LAYING, 4)
        );
        assert!(m.gunner_pos.is_some() && m.observer_pos.is_some());
        assert!((9.0..=11.0).contains(&m.first_error_m), "{m:?}");
        assert!(f.squads.iter().all(|s| s.owner == 0 || s.mission.is_none()));
    }

    /// S8b (S8a HUD fault): the frame carries the stowed MG rounds of a
    /// half-track (no main gun, `mounts` are all MGs).
    #[test]
    fn a_fresh_half_track_reports_its_mg_stowage() {
        use frontline_sim::scenarios::{EAST, at};
        let mut w = staged();
        let setup = w.match_setup().unwrap().clone();
        let t = setup.squad_type_index("ww2:ger_sdkfz251_1_squad").unwrap();
        let sq = w
            .spawn_squad_at(frontline_sim::PlayerId(0), t, at(400, 512), EAST)
            .unwrap();
        let mut ctx = FrameCtx::new(&setup, 0);
        let mut f = MatchFrame::empty();
        for _ in 0..3 {
            w.step(&[]);
            f.fill(&w, &mut ctx, 0.0, Instant::now());
        }
        let v = f.vehicles.iter().find(|v| v.squad == sq.0).unwrap();
        let vt = &setup.vehicles[v.type_idx as usize];
        let full: u32 = vt.mg_mounts().iter().filter_map(|m| m.rounds).sum();
        assert!(full > 0, "{}", vt.id);
        assert_eq!(v.mg_rounds, full, "the card's MG count");
    }

    #[test]
    fn vehicles_follow_the_fog_of_war() {
        use frontline_sim::scenarios::{EAST, WEST, at, tank};
        let mut w = staged();
        let setup = w.match_setup().unwrap().clone();
        let (own_sq, own) = tank(&mut w, 0, at(400, 512), EAST).unwrap();
        // 500 m off: beyond twice the tank's sight.
        let (_, far) = tank(&mut w, 1, at(900, 512), WEST).unwrap();
        let mut ctx = FrameCtx::new(&setup, 0);
        let mut f = MatchFrame::empty();
        for _ in 0..10 {
            w.step(&[]);
            f.fill(&w, &mut ctx, 0.0, Instant::now());
        }
        assert_eq!(f.vehicles.len(), 1, "{:?}", f.vehicles);
        let v = &f.vehicles[0];
        assert_eq!(v.id, own.0);
        assert_eq!(v.squad, own_sq.0);
        // S8b: the parts a tank has are whole; wheels and cargo read 0.
        assert!(v.seats.iter().any(|&s| s != 0) && v.components[..7].iter().all(|&c| c == 100));
        assert!(v.components[7..].iter().all(|&c| c == 0));
        assert!(v.ammo[0] > 0);
        // A vehicle's squad is not a squad of men.
        assert!(f.squads.iter().all(|s| s.id != own_sq.0));
        assert!(!f.ids.contains(&own.0));

        // Driven to 200 m it is spotted: pose and public flags only.
        w.place_vehicle(far, at(600, 512), Fx::from_int(3));
        w.edit_vehicle(far, |vs| vs.burn = 1);
        let mut seen = false;
        for _ in 0..60 {
            w.step(&[]);
            f.fill(&w, &mut ctx, 0.0, Instant::now());
            if f.vehicles.iter().any(|v| v.id == far.0) {
                seen = true;
                break;
            }
        }
        assert!(seen, "the enemy tank 200 m off was never seen");
        let e = f.vehicles.iter().find(|v| v.id == far.0).unwrap();
        assert_eq!(e.owner, 1);
        assert_eq!(e.flags & !vehicle_flags::PUBLIC, 0);
        assert_eq!(e.seats, [0; 5]);
        assert_eq!(e.components, [0; frontline_content::COMPONENTS]);
        assert_eq!(e.ammo, [0, 0, 0]);
        assert_eq!(
            (e.loaded, e.ammo_choice, e.reload_pct, e.followers),
            (0, 0, 0, 0)
        );
        assert_eq!((e.repairing, e.target), (NONE_U8, u32::MAX));
        assert_eq!(e.seen_by, 1);
        // The enemy sees ours too, but player 1's frame keeps our tank's
        // private fields out.
        let mut ctx1 = FrameCtx::new(&setup, 1);
        let mut f1 = MatchFrame::empty();
        f1.fill(&w, &mut ctx1, 0.0, Instant::now());
        if let Some(o) = f1.vehicles.iter().find(|v| v.id == own.0) {
            assert_eq!(o.seats, [0; 5]);
        }

        // Back out of sight it is dropped again; as a wreck it is always
        // there (for both players).
        w.place_vehicle(far, at(950, 512), Fx::from_int(3));
        for _ in 0..60 {
            w.step(&[]);
            f.fill(&w, &mut ctx, 0.0, Instant::now());
        }
        assert!(f.vehicles.iter().all(|v| v.id != far.0), "{:?}", f.vehicles);
        assert!(w.wreck_vehicle(far));
        w.step(&[]);
        f.fill(&w, &mut ctx, 0.0, Instant::now());
        let wr = f.vehicles.iter().find(|v| v.id == far.0).unwrap();
        assert_eq!(wr.owner, WRECK_OWNER);
        assert_ne!(wr.flags & vehicle_flags::WRECK, 0);
    }

    #[test]
    fn shells_and_armour_events_follow_the_fog_of_war() {
        use frontline_sim::ArmourEventKind;
        use frontline_sim::scenarios::{NORTH, WEST, at, tank};
        // Our tank 150 m off an enemy's flank: it fires, hits, and we see
        // all of it. Player 1's view of the same ticks: he sees our tank
        // (it fires at him) and the hits on his own tank.
        let mut w = staged();
        let setup = w.match_setup().unwrap().clone();
        let (_, own) = tank(&mut w, 0, at(350, 512), frontline_sim::scenarios::EAST).unwrap();
        let (_, foe) = tank(&mut w, 1, at(500, 512), NORTH).unwrap();
        // A third tank far away that nobody sees.
        let (_, hidden) = tank(&mut w, 1, at(950, 900), WEST).unwrap();
        let mut ctx = FrameCtx::new(&setup, 0);
        let mut ctx1 = FrameCtx::new(&setup, 1);
        let mut f = MatchFrame::empty();
        let mut f1 = MatchFrame::empty();
        let (mut shells, mut fired, mut impacts, mut theirs) = (0, 0, 0, 0);
        for _ in 0..30 * 20 {
            w.step(&[]);
            f.fill(&w, &mut ctx, 0.0, Instant::now());
            let ev = TickEvents::collect(&w, &ctx);
            f1.fill(&w, &mut ctx1, 0.0, Instant::now());
            let ev1 = TickEvents::collect(&w, &ctx1);
            shells += f.shells.len();
            for a in &ev.armour {
                assert_ne!(a.ev.vehicle, hidden.0);
                if a.ev.kind == ArmourEventKind::Fired as u8 && a.ev.vehicle == own.0 {
                    fired += 1;
                }
                if a.ev.vehicle == foe.0 && a.ev.kind != ArmourEventKind::Fired as u8 {
                    impacts += 1;
                    assert_eq!(
                        a.type_idx,
                        w.game().unwrap().vehicle(foe).map_or(0, |v| v.type_idx)
                    );
                }
            }
            theirs += ev1.armour.iter().filter(|a| a.ev.owner == 1).count();
            for s in &ev.shots {
                if s.shooter == own.0 && s.vehicle_type.is_some() {
                    assert!(s.visible);
                }
            }
            if fired > 0 && impacts > 0 && shells > 0 && theirs > 0 {
                break;
            }
        }
        assert!(
            fired > 0 && impacts > 0 && shells > 0,
            "fired {fired} impacts {impacts} shells {shells}"
        );
        assert!(theirs > 0, "player 1 saw no events of his own tank");
        // Small-arms rounds and mortar shells only in `rounds`; main-gun
        // shells only in `shells`.
        assert!(f.rounds.len() <= w.projectiles().slots().iter().flatten().count());
    }

    #[test]
    fn every_scene_meets_its_condition() {
        use crate::match_setup::{MenuChoice, find_mods_dir, mods_candidates, resolve};
        let mods = find_mods_dir(&mods_candidates(None, None)).unwrap();
        let setup = resolve(&mods, &MenuChoice::default()).unwrap();
        for name in frontline_sim::scenarios::SCENES {
            let sc = frontline_sim::scenarios::scene(name, setup.clone(), 2).unwrap();
            let mut w = sc.world;
            let mut ctx = FrameCtx::new(&setup, 0);
            ctx.scene = Some(name.to_owned());
            let mut f = MatchFrame::empty();
            w.step(&sc.commands);
            f.fill(&w, &mut ctx, 0.0, Instant::now());
            let mut t = 1;
            while f.scene_ready_at.is_none() && t < sc.max_ticks {
                w.step(&[]);
                f.fill(&w, &mut ctx, 0.0, Instant::now());
                t += 1;
            }
            let at = f.scene_ready_at;
            assert!(at.is_some(), "scene {name}: not ready after {t} ticks");
            // The latch keeps the first tick.
            w.step(&[]);
            f.fill(&w, &mut ctx, 0.0, Instant::now());
            assert_eq!(f.scene_ready_at, at, "{name}");
            eprintln!("scene {name}: ready at tick {}", at.unwrap());
        }
    }

    #[test]
    fn polyline_length() {
        assert_eq!(polyline_len(&[[0.0, 0.0], [3.0, 4.0], [3.0, 10.0]]), 11.0);
        assert_eq!(polyline_len(&[]), 0.0);
    }
}
