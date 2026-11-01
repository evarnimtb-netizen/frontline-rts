//! HUD events and effect cues derived from published frames (presenter
//! only). Plain Rust, unit tested.
//!
//! The sim has no notion of "Point B lost" or "Rifle squad under fire":
//! [`EventTracker`] diffs consecutive frames for the local player and emits
//! the messages the event feed shows. [`effect_cues`] turns a batch of
//! [`TickEvents`] into the sound and VFX cues `match_state.md` lists,
//! folding a squad's shots into one cue per batch so a firefight does not
//! flood the audio pool. Positions are sim axes; the node converts.
//!
//! S3: own vehicles arriving and the visible armour events (knock-outs,
//! own components destroyed, own vehicles bogged) make feed lines; main
//! guns and launchers firing, armour impacts and ammo-rack detonations
//! make the `cannon`, `rocket_launch`, `armour_hit` and `cook_off` cues,
//! and tank-HE and rocket bursts get their own explosion sizes.

use std::collections::BTreeMap;

use frontline_content::{AmmoMechanism, MatchSetup, WeaponClass};
use frontline_sim::{ArmourEventKind, BlastKind, LoadedSnap, OrderKind, Outcome, vehicle_flags};

use crate::match_frame::{MatchFrame, TickEvents};

/// Own squads get at most one "under fire" message per this many seconds.
pub const ALERT_SQUAD_S: u64 = 30;
/// And the feed at most one per this many seconds overall.
pub const ALERT_ANY_S: u64 = 6;
/// Tickets below which the "tickets low" warning fires once.
pub const TICKETS_LOW: u32 = 100;

#[derive(Clone, Debug, PartialEq)]
pub struct UiEvent {
    /// `point_captured`, `point_lost`, `squad_lost`, `squad_arrived`,
    /// `under_attack`, `tickets_low` or `info`; S3 `vehicle_arrived`,
    /// `vehicle_knocked_out`, `vehicle_damaged`, `vehicle_bogged`.
    pub kind: &'static str,
    pub text: String,
    pub pos: Option<[f32; 2]>,
    pub player: i32,
    pub tick: u64,
}

#[derive(Clone, Debug)]
struct SquadMemo {
    owner: u8,
    men: u8,
    type_idx: u16,
    pos: [f32; 2],
    suppression: u32,
    order: OrderKind,
    dig_pct: u8,
}

#[derive(Clone, Debug, Default)]
pub struct EventTracker {
    local: u8,
    started: bool,
    owners: Vec<Option<u8>>,
    squads: BTreeMap<u32, SquadMemo>,
    alert_tick: BTreeMap<u32, u64>,
    last_alert: Option<u64>,
    tickets_low: bool,
    ended: bool,
    /// Own live vehicles of the last frame.
    vehicles: Vec<u32>,
}

/// A vehicle type's short name ("Panzer IV").
pub fn vehicle_short(setup: &MatchSetup, type_idx: u16) -> &str {
    setup
        .vehicles
        .get(type_idx as usize)
        .map_or("Vehicle", |v| v.short_name.as_str())
}

/// Feed words for a destroyed component (`ComponentKind::index`).
fn component_words(c: u8) -> &'static str {
    match c {
        0 => "engine",
        1 | 2 => "track",
        3 => "main gun",
        4 => "turret ring",
        5 => "ammo rack",
        _ => "component",
    }
}

impl EventTracker {
    pub fn new(local: u8) -> EventTracker {
        EventTracker {
            local,
            ..EventTracker::default()
        }
    }

    /// Compares `f` with the previous frame and appends the new messages.
    pub fn update(&mut self, f: &MatchFrame, setup: &MatchSetup, out: &mut Vec<UiEvent>) {
        let local = self.local;
        let hz = setup.tick_hz.max(1) as u64;
        let mut push = |kind, text: String, pos: Option<[f32; 2]>| {
            out.push(UiEvent {
                kind,
                text,
                pos,
                player: local as i32,
                tick: f.tick,
            })
        };
        if !self.started {
            push(
                "info",
                format!("Skirmish on {}. Take and hold the points.", setup.map.name),
                None,
            );
        }
        // Points.
        for (k, p) in f.points.iter().enumerate() {
            let name = setup.map.points.get(k).map_or("?", |p| p.name.as_str());
            let before = self.owners.get(k).copied().flatten();
            if self.started && before != p.owner {
                match p.owner {
                    Some(o) if o == local => push(
                        "point_captured",
                        format!("Point {name} captured."),
                        Some(p.pos),
                    ),
                    Some(_) if before == Some(local) => {
                        push("point_lost", format!("Point {name} lost."), Some(p.pos))
                    }
                    Some(_) => push("info", format!("The enemy took point {name}."), Some(p.pos)),
                    None if before == Some(local) => push(
                        "point_lost",
                        format!("Point {name} neutralised."),
                        Some(p.pos),
                    ),
                    None => {}
                }
            }
        }
        self.owners = f.points.iter().map(|p| p.owner).collect();
        // Own squads: arrivals, losses, fire.
        let mut seen: BTreeMap<u32, SquadMemo> = BTreeMap::new();
        for s in f.squads.iter().filter(|s| s.owner == local) {
            let name = setup
                .squad_types
                .get(s.type_idx as usize)
                .map_or("Squad", |t| t.name.as_str());
            let pos = [s.anchor[0], s.anchor[1]];
            let memo = SquadMemo {
                owner: s.owner,
                men: s.men,
                type_idx: s.type_idx,
                pos,
                suppression: s.suppression_sum / s.men.max(1) as u32,
                order: s.order,
                dig_pct: s.dig_pct,
            };
            match self.squads.get(&s.id) {
                None if self.started => {
                    push(
                        "squad_arrived",
                        format!("{name} arrived at base."),
                        Some(pos),
                    );
                }
                Some(prev) => {
                    let hit = s.men < prev.men || (memo.suppression >= 30 && prev.suppression < 30);
                    let quiet_squad = self
                        .alert_tick
                        .get(&s.id)
                        .is_none_or(|&t| f.tick >= t + ALERT_SQUAD_S * hz);
                    let quiet_feed = self
                        .last_alert
                        .is_none_or(|t| f.tick >= t + ALERT_ANY_S * hz);
                    if hit && quiet_squad && quiet_feed {
                        self.alert_tick.insert(s.id, f.tick);
                        self.last_alert = Some(f.tick);
                        push("under_attack", format!("{name} under fire."), Some(pos));
                    }
                    if prev.order == OrderKind::Dig
                        && s.order == OrderKind::Idle
                        && prev.dig_pct >= 90
                    {
                        // A finished trench turns the order to Idle.
                        push("info", format!("{name} finished digging."), Some(pos));
                    }
                }
                None => {}
            }
            seen.insert(s.id, memo);
        }
        for (id, prev) in &self.squads {
            if prev.owner == local && !seen.contains_key(id) {
                let name = setup
                    .squad_types
                    .get(prev.type_idx as usize)
                    .map_or("Squad", |t| t.name.as_str());
                push("squad_lost", format!("{name} lost."), Some(prev.pos));
                self.alert_tick.remove(id);
            }
        }
        self.squads = seen;
        // Own vehicles arriving (S3); losses come from armour events.
        let own_v: Vec<u32> = f
            .vehicles
            .iter()
            .filter(|v| v.owner == local && v.flags & vehicle_flags::WRECK == 0)
            .map(|v| v.id)
            .collect();
        if self.started {
            for v in f.vehicles.iter().filter(|v| own_v.contains(&v.id)) {
                if !self.vehicles.contains(&v.id) {
                    push(
                        "vehicle_arrived",
                        format!("{} arrived.", vehicle_short(setup, v.type_idx)),
                        Some([v.pos[0], v.pos[1]]),
                    );
                }
            }
        }
        self.vehicles = own_v;
        // Tickets.
        if let Some(me) = f.players.get(local as usize)
            && !self.tickets_low
            && me.tickets < TICKETS_LOW
        {
            self.tickets_low = true;
            push("tickets_low", format!("Tickets below {TICKETS_LOW}."), None);
        }
        if !self.ended && f.outcome != Outcome::Running {
            self.ended = true;
            let text = match f.outcome {
                Outcome::Won { winner } if winner.0 == local => "The enemy is beaten.",
                Outcome::Won { .. } => "We have lost the field.",
                _ => "Nobody holds the field.",
            };
            push("info", text.to_owned(), None);
        }
        self.started = true;
    }

    /// Feed lines for one tick's visible armour events (S3): any vehicle
    /// knocked out, own components destroyed, own vehicles bogged.
    pub fn armour(&mut self, ev: &TickEvents, setup: &MatchSetup, out: &mut Vec<UiEvent>) {
        let local = self.local;
        for a in &ev.armour {
            let e = &a.ev;
            let name = vehicle_short(setup, a.type_idx);
            let own = e.owner == local;
            let (kind, text) = match e.kind {
                k if k == ArmourEventKind::KnockedOut as u8 => (
                    "vehicle_knocked_out",
                    if own {
                        format!("{name} knocked out.")
                    } else {
                        format!("Enemy {name} knocked out.")
                    },
                ),
                k if k == ArmourEventKind::Disabled as u8 && own => (
                    "vehicle_damaged",
                    format!("{name}: {} destroyed.", component_words(e.component)),
                ),
                k if k == ArmourEventKind::Bogged as u8 && own => {
                    ("vehicle_bogged", format!("{name} bogged in trench."))
                }
                _ => continue,
            };
            out.push(UiEvent {
                kind,
                text,
                pos: Some([e.pos[0], e.pos[1]]),
                player: e.owner as i32,
                tick: ev.tick,
            });
        }
    }
}

/// A sound / VFX cue (`match_state.md` "effects").
#[derive(Clone, Debug, PartialEq)]
pub struct EffectCue {
    /// `shot`, `mortar_launch`, `explosion` or `dig`; S3 `cannon`,
    /// `rocket_launch`, `armour_hit`, `cook_off`.
    pub kind: &'static str,
    /// Shots: `rifle`, `mg` or `smg`. Explosions: the blast size
    /// (`mortar`, `frag`, `satchel`, `tank_he` or `rocket`; the state's
    /// `size`). `cannon`: the round (`ap`, `he`). `armour_hit`: the result
    /// (`ricochet`, `no_pen`, `pen`).
    pub weapon: &'static str,
    /// Sim axes.
    pub pos: [f32; 3],
    /// `cannon`, `rocket_launch`: the firing direction (sim axes, unit).
    pub dir: Option<[f32; 3]>,
    /// `cannon`: the vehicle type.
    pub vehicle_type: Option<u16>,
}

impl EffectCue {
    pub fn new(kind: &'static str, weapon: &'static str, pos: [f32; 3]) -> EffectCue {
        EffectCue {
            kind,
            weapon,
            pos,
            dir: None,
            vehicle_type: None,
        }
    }
}

fn unit(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-3);
    [v[0] / l, v[1] / l, v[2] / l]
}

pub fn weapon_cue(class: WeaponClass) -> &'static str {
    match class {
        WeaponClass::Rifle => "rifle",
        WeaponClass::Smg => "smg",
        WeaponClass::Mg => "mg",
        WeaponClass::Mortar => "mortar",
        // S3: main guns and launchers make their own cues (`cannon`,
        // `rocket_launch`); these names are only the fallback.
        WeaponClass::Cannon => "cannon",
        WeaponClass::Launcher => "rocket",
    }
}

/// Cues for a batch of ticks: one `shot` per firing squad (and weapon),
/// one `mortar_launch` per mortar round, one `explosion` per burst. Shots
/// by shooters the local player does not see are left out (fog of war).
/// S3: one `cannon` per main-gun shot (the round from the tick's `Fired`
/// event), one `rocket_launch` per rocket, one `armour_hit` per visible
/// ricochet, non-penetration or penetration, one `cook_off` per ammo-rack
/// knock-out.
pub fn effect_cues(batch: &[TickEvents], setup: &MatchSetup, out: &mut Vec<EffectCue>) {
    let mut by_squad: BTreeMap<(u32, u16), [f32; 3]> = BTreeMap::new();
    for ev in batch {
        for s in ev.shots.iter().filter(|s| s.visible) {
            let class = setup
                .weapons
                .get(s.weapon as usize)
                .map_or(WeaponClass::Rifle, |w| w.class);
            match class {
                WeaponClass::Mortar => out.push(EffectCue::new("mortar_launch", "mortar", s.from)),
                WeaponClass::Cannon => {
                    let he = ev.armour.iter().any(|a| {
                        a.ev.kind == ArmourEventKind::Fired as u8
                            && a.ev.vehicle == s.shooter
                            && a.ev.component == LoadedSnap::He as u8
                    });
                    out.push(EffectCue {
                        dir: Some(unit(s.vel)),
                        vehicle_type: s.vehicle_type,
                        ..EffectCue::new("cannon", if he { "he" } else { "ap" }, s.from)
                    });
                }
                WeaponClass::Launcher => out.push(EffectCue {
                    dir: Some(unit(s.vel)),
                    ..EffectCue::new("rocket_launch", "rocket", s.from)
                }),
                _ => {
                    by_squad.insert((s.squad, s.weapon), s.from);
                }
            }
        }
        for e in &ev.explosions {
            let size = match e.kind {
                BlastKind::Shell => match setup.weapons.get(e.weapon as usize).map(|w| w.class) {
                    Some(WeaponClass::Cannon) => "tank_he",
                    Some(WeaponClass::Launcher) => "rocket",
                    _ => "mortar",
                },
                BlastKind::Frag => "frag",
                BlastKind::Satchel => "satchel",
            };
            out.push(EffectCue::new("explosion", size, e.pos));
        }
        for a in &ev.armour {
            let e = &a.ev;
            let result = match e.kind {
                k if k == ArmourEventKind::Ricochet as u8 => "ricochet",
                k if k == ArmourEventKind::NonPenetration as u8 => "no_pen",
                k if k == ArmourEventKind::Penetration as u8 => "pen",
                k if k == ArmourEventKind::KnockedOut as u8 && e.component == 1 => {
                    out.push(EffectCue::new("cook_off", "", e.pos));
                    continue;
                }
                _ => continue,
            };
            out.push(EffectCue::new("armour_hit", result, e.pos));
        }
    }
    for ((_, weapon), pos) in by_squad {
        let class = setup
            .weapons
            .get(weapon as usize)
            .map_or(WeaponClass::Rifle, |w| w.class);
        out.push(EffectCue::new("shot", weapon_cue(class), pos));
    }
}

/// `armour_events[].ammo` and `shells[].kind` words for a round.
pub fn ammo_word(setup: &MatchSetup, ammo: u16) -> &'static str {
    match setup.ammo.get(ammo as usize).map(|a| a.mechanism) {
        Some(AmmoMechanism::Ap | AmmoMechanism::Aphe) => "ap",
        Some(AmmoMechanism::He) => "he",
        Some(AmmoMechanism::Heat) => "heat",
        None => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::match_frame::tests_support::test_world;
    use crate::match_frame::{FrameCtx, ShotView};
    use frontline_sim::{Command, CommandKind, ExplosionSnapshot, PlayerId};
    use std::time::Instant;

    #[test]
    fn start_arrival_and_loss_messages() {
        let mut w = test_world(false);
        let setup = w.match_setup().unwrap().clone();
        let mut ctx = FrameCtx::new(&setup, 0);
        let mut f = MatchFrame::empty();
        let mut tr = EventTracker::new(0);
        let mut out = Vec::new();
        f.fill(&w, &mut ctx, 0.0, Instant::now());
        tr.update(&f, &setup, &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, "info");
        out.clear();
        let rifle = crate::intents::call_in_type(&setup, 0, "rifle").unwrap();
        w.step(&[Command {
            player: PlayerId(0),
            seq: 0,
            kind: CommandKind::CallIn { squad_type: rifle },
        }]);
        for _ in 0..setup.squad_types[rifle as usize].call_in_ticks + 2 {
            w.step(&[]);
        }
        f.fill(&w, &mut ctx, 0.0, Instant::now());
        tr.update(&f, &setup, &mut out);
        assert!(out.iter().any(|e| e.kind == "squad_arrived"), "{out:?}");
        // A squad that vanishes from the frame is reported lost.
        out.clear();
        let gone = f.squads.iter().position(|s| s.owner == 0).unwrap();
        f.squads.remove(gone);
        tr.update(&f, &setup, &mut out);
        assert_eq!(out.iter().filter(|e| e.kind == "squad_lost").count(), 1);
        // Tickets low fires once.
        out.clear();
        f.players[0].tickets = 50;
        tr.update(&f, &setup, &mut out);
        tr.update(&f, &setup, &mut out);
        assert_eq!(out.iter().filter(|e| e.kind == "tickets_low").count(), 1);
    }

    #[test]
    fn point_owner_changes() {
        let w = test_world(false);
        let setup = w.match_setup().unwrap().clone();
        let mut ctx = FrameCtx::new(&setup, 0);
        let mut f = MatchFrame::empty();
        f.fill(&w, &mut ctx, 0.0, Instant::now());
        let mut tr = EventTracker::new(0);
        let mut out = Vec::new();
        tr.update(&f, &setup, &mut out);
        out.clear();
        f.points[2].owner = Some(0);
        tr.update(&f, &setup, &mut out);
        assert_eq!(out[0].kind, "point_captured");
        assert_eq!(out[0].text, "Point C captured.");
        f.points[2].owner = Some(1);
        tr.update(&f, &setup, &mut out);
        assert_eq!(out[1].kind, "point_lost");
    }

    #[test]
    fn shots_fold_per_squad_and_respect_fog() {
        let w = test_world(false);
        let setup = w.match_setup().unwrap().clone();
        let rifle = setup
            .weapons
            .iter()
            .position(|w| w.class == WeaponClass::Rifle)
            .unwrap() as u16;
        let mortar = setup
            .weapons
            .iter()
            .position(|w| w.class == WeaponClass::Mortar)
            .unwrap() as u16;
        let shot = |squad, weapon, visible| ShotView {
            squad,
            weapon,
            visible,
            ..ShotView::default()
        };
        let batch = vec![
            TickEvents {
                tick: 1,
                shots: vec![
                    shot(1, rifle, true),
                    shot(1, rifle, true),
                    shot(2, rifle, false),
                ],
                ..TickEvents::default()
            },
            TickEvents {
                tick: 2,
                shots: vec![shot(1, rifle, true), shot(3, mortar, true)],
                explosions: vec![ExplosionSnapshot::default()],
                ..TickEvents::default()
            },
        ];
        let mut out = Vec::new();
        effect_cues(&batch, &setup, &mut out);
        let kinds: Vec<&str> = out.iter().map(|c| c.kind).collect();
        assert_eq!(kinds, vec!["mortar_launch", "explosion", "shot"]);
        assert_eq!(out[2].weapon, "rifle");
    }

    #[test]
    fn vehicle_cues_and_feed_lines() {
        use crate::match_frame::ArmourView;
        use frontline_sim::{ArmourEvent, ArmourEventKind as K};
        let w = test_world(false);
        let setup = w.match_setup().unwrap().clone();
        let class =
            |c: WeaponClass| setup.weapons.iter().position(|w| w.class == c).unwrap() as u16;
        let (cannon, launcher) = (class(WeaponClass::Cannon), class(WeaponClass::Launcher));
        let ev = |kind: K, vehicle, owner, component| ArmourView {
            ev: ArmourEvent {
                kind: kind as u8,
                vehicle,
                owner,
                component,
                ..ArmourEvent::default()
            },
            type_idx: 0,
        };
        let batch = vec![TickEvents {
            tick: 9,
            shots: vec![
                ShotView {
                    weapon: cannon,
                    shooter: 70,
                    vel: [0.0, 700.0, 0.0],
                    vehicle_type: Some(0),
                    visible: true,
                    ..ShotView::default()
                },
                ShotView {
                    weapon: launcher,
                    shooter: 12,
                    vel: [100.0, 0.0, 0.0],
                    visible: true,
                    ..ShotView::default()
                },
                // Unseen: no cue.
                ShotView {
                    weapon: cannon,
                    shooter: 71,
                    visible: false,
                    ..ShotView::default()
                },
            ],
            explosions: vec![ExplosionSnapshot {
                weapon: cannon,
                ..ExplosionSnapshot::default()
            }],
            armour: vec![
                ev(K::Fired, 70, 0, LoadedSnap::He as u8),
                ev(K::Ricochet, 80, 1, 255),
                ev(K::Penetration, 80, 1, 255),
                ev(K::Disabled, 70, 0, 1),
                ev(K::Bogged, 70, 0, 255),
                ev(K::KnockedOut, 80, 1, 1),
            ],
            ..TickEvents::default()
        }];
        let mut out = Vec::new();
        effect_cues(&batch, &setup, &mut out);
        let got: Vec<(&str, &str)> = out.iter().map(|c| (c.kind, c.weapon)).collect();
        assert_eq!(
            got,
            vec![
                ("cannon", "he"),
                ("rocket_launch", "rocket"),
                ("explosion", "tank_he"),
                ("armour_hit", "ricochet"),
                ("armour_hit", "pen"),
                ("cook_off", ""),
            ]
        );
        assert_eq!(out[0].dir, Some([0.0, 1.0, 0.0]));
        assert_eq!(out[0].vehicle_type, Some(0));
        // Feed: the enemy knock-out, our destroyed track and the bogging;
        // not the hits.
        let mut tr = EventTracker::new(0);
        let mut feed = Vec::new();
        tr.armour(&batch[0], &setup, &mut feed);
        let lines: Vec<(&str, &str)> = feed.iter().map(|e| (e.kind, e.text.as_str())).collect();
        let short = vehicle_short(&setup, 0);
        assert_eq!(
            lines,
            vec![
                (
                    "vehicle_damaged",
                    format!("{short}: track destroyed.").as_str()
                ),
                (
                    "vehicle_bogged",
                    format!("{short} bogged in trench.").as_str()
                ),
                (
                    "vehicle_knocked_out",
                    format!("Enemy {short} knocked out.").as_str()
                ),
            ]
        );
        assert_eq!(feed[2].player, 1);
    }
}
