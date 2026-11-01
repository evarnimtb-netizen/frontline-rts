//! HUD events and effect cues derived from published frames (presenter
//! only). Plain Rust, unit tested.
//!
//! The sim has no notion of "Point B lost" or "Rifle squad under fire":
//! [`EventTracker`] diffs consecutive frames for the local player and emits
//! the messages the event feed shows. [`effect_cues`] turns a batch of
//! [`TickEvents`] into the sound and VFX cues `match_state.md` lists,
//! folding a squad's shots into one cue per batch so a firefight does not
//! flood the audio pool. Positions are sim axes; the node converts.

use std::collections::BTreeMap;

use frontline_content::{MatchSetup, WeaponClass};
use frontline_sim::{BlastKind, OrderKind, Outcome};

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
    /// `under_attack`, `tickets_low` or `info`.
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
}

/// A sound / VFX cue (`match_state.md` "effects").
#[derive(Clone, Debug, PartialEq)]
pub struct EffectCue {
    /// `shot`, `mortar_launch`, `explosion` or `dig`.
    pub kind: &'static str,
    /// Shots: `rifle`, `mg` or `smg`. Explosions: the blast size
    /// (`mortar`, `frag` or `satchel`; the state's `size`).
    pub weapon: &'static str,
    /// Sim axes.
    pub pos: [f32; 3],
}

pub fn weapon_cue(class: WeaponClass) -> &'static str {
    match class {
        WeaponClass::Rifle => "rifle",
        WeaponClass::Smg => "smg",
        WeaponClass::Mg => "mg",
        WeaponClass::Mortar => "mortar",
        // S3: no vehicles or AT teams in play before the presenter
        // integration; the names are the contract's (`cannon`, `rocket`).
        WeaponClass::Cannon => "cannon",
        WeaponClass::Launcher => "rocket",
    }
}

/// Cues for a batch of ticks: one `shot` per firing squad (and weapon),
/// one `mortar_launch` per mortar round, one `explosion` per burst. Shots
/// by shooters the local player does not see are left out (fog of war).
pub fn effect_cues(batch: &[TickEvents], setup: &MatchSetup, out: &mut Vec<EffectCue>) {
    let mut by_squad: BTreeMap<(u32, u16), [f32; 3]> = BTreeMap::new();
    for ev in batch {
        for s in ev.shots.iter().filter(|s| s.visible) {
            let class = setup
                .weapons
                .get(s.weapon as usize)
                .map_or(WeaponClass::Rifle, |w| w.class);
            if class == WeaponClass::Mortar {
                out.push(EffectCue {
                    kind: "mortar_launch",
                    weapon: "mortar",
                    pos: s.from,
                });
            } else {
                by_squad.insert((s.squad, s.weapon), s.from);
            }
        }
        for e in &ev.explosions {
            out.push(EffectCue {
                kind: "explosion",
                weapon: match e.kind {
                    BlastKind::Shell => "mortar",
                    BlastKind::Frag => "frag",
                    BlastKind::Satchel => "satchel",
                },
                pos: e.pos,
            });
        }
    }
    for ((_, weapon), pos) in by_squad {
        let class = setup
            .weapons
            .get(weapon as usize)
            .map_or(WeaponClass::Rifle, |w| w.class);
        out.push(EffectCue {
            kind: "shot",
            weapon: weapon_cue(class),
            pos,
        });
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
                tiles: vec![],
            },
        ];
        let mut out = Vec::new();
        effect_cues(&batch, &setup, &mut out);
        let kinds: Vec<&str> = out.iter().map(|c| c.kind).collect();
        assert_eq!(kinds, vec!["mortar_launch", "explosion", "shot"]);
        assert_eq!(out[2].weapon, "rifle");
    }
}
