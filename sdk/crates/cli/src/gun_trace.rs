//! `skirmish --gun-trace FILE` (S9b diagnostics): one JSON line per armour
//! event and, every five game seconds, one line with every live vehicle and
//! the sight line from each served gun to each enemy vehicle within 600 m.
//! `tools/gun_funnel.py` turns a set of these files into the funnel
//! "tank in reach, in the arc, in sight, fired at, hit, penetrated, lost".
//! Reads the world only: a traced match has the hash of an untraced one.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use anyhow::{Context, Result};
use frontline_sim::{ArmourEventKind, Fx, PlayerId, SimId, World, vehicle_flags};
use serde_json::json;

/// Game seconds between two vehicle lines.
const SAMPLE_S: u64 = 5;
/// Gun-to-vehicle pairs are listed inside this range (m).
const PAIR_M: f64 = 600.0;
/// Eye heights of the pair's sight line (mm), as in `ai/guns.rs`.
const GUN_EYE_MM: i32 = 1200;
const TARGET_EYE_MM: i32 = 1500;

pub struct GunTrace {
    out: BufWriter<File>,
}

fn r1(v: f32) -> f64 {
    (v as f64 * 10.0).round() / 10.0
}

impl GunTrace {
    pub fn create(path: &Path, w: &World) -> Result<GunTrace> {
        let f = File::create(path).with_context(|| format!("create {}", path.display()))?;
        let mut t = GunTrace {
            out: BufWriter::new(f),
        };
        let g = w.game().context("match world")?;
        let s = g.setup();
        let fx = |v: frontline_content::setup::Fixed| v.0 as f64 / 65536.0;
        let head = json!({
            "ev": "setup",
            "seed": s.seed,
            "hz": s.tick_hz,
            "map_m": s.map.size_m,
            "players": s.players.iter().map(|p| s.factions[p.faction as usize].id.clone()).collect::<Vec<_>>(),
            "bases": (0..s.players.len() as u8).map(|p| g.base_of(PlayerId(p)).to_f32()).collect::<Vec<_>>(),
            "points": s.map.points.iter().map(|p| json!([fx(p.pos.x), fx(p.pos.y)])).collect::<Vec<_>>(),
            "types": s.vehicles.iter().map(|v| json!({
                "id": v.id, "role": format!("{:?}", v.role), "arc": v.gun_arc_deg, "cost": v.cost,
            })).collect::<Vec<_>>(),
            "ammo": s.ammo.iter().map(|a| a.id.clone()).collect::<Vec<_>>(),
        });
        writeln!(t.out, "{head}")?;
        Ok(t)
    }

    /// Call once per tick, after the step.
    pub fn observe(&mut self, w: &World) -> Result<()> {
        let snap = w.snapshot();
        let tick = w.tick();
        for e in &snap.armour_events {
            let k = e.kind;
            let wanted = [
                ArmourEventKind::Fired,
                ArmourEventKind::Ricochet,
                ArmourEventKind::NonPenetration,
                ArmourEventKind::Penetration,
                ArmourEventKind::KnockedOut,
                ArmourEventKind::Abandoned,
            ];
            if !wanted.iter().any(|&x| x as u8 == k) {
                continue;
            }
            // The shooter's target when it fired (a vehicle id, a soldier
            // id, or none).
            let target = (k == ArmourEventKind::Fired as u8)
                .then(|| snap.vehicles.iter().find(|v| v.id == e.vehicle))
                .flatten()
                .map(|v| v.target);
            let line = json!({
                "ev": k, "t": tick, "v": e.vehicle, "o": e.owner, "a": e.attacker,
                "c": e.component, "am": e.ammo, "tg": target,
                "p": [r1(e.pos[0]), r1(e.pos[1])],
            });
            writeln!(self.out, "{line}")?;
        }
        let Some(g) = w.game() else { return Ok(()) };
        let s = g.setup();
        if !tick.is_multiple_of(SAMPLE_S * s.tick_hz as u64) {
            return Ok(());
        }
        let live = |fl: u16| fl & (vehicle_flags::WRECK | vehicle_flags::ABANDONED) == 0;
        let veh: Vec<_> = snap
            .vehicles
            .iter()
            .filter(|v| live(v.flags))
            .map(|v| {
                json!([
                    v.id,
                    v.owner,
                    v.type_idx,
                    r1(v.pos[0]),
                    r1(v.pos[1]),
                    (v.yaw as f64 * 1000.0).round() / 1000.0,
                    v.flags,
                    v.gun_state,
                    v.gun_men,
                    v.target,
                    v.seen_by,
                    v.ammo.iter().map(|&n| n as u32).sum::<u32>(),
                ])
            })
            .collect();
        let ents = w.entities();
        let ter = w.terrain();
        let mut pairs = Vec::new();
        for gun in snap
            .vehicles
            .iter()
            .filter(|v| v.gun_state > 0 && live(v.flags))
        {
            for v in snap
                .vehicles
                .iter()
                .filter(|v| v.owner != gun.owner && live(v.flags))
            {
                let (dx, dy) = (
                    (v.pos[0] - gun.pos[0]) as f64,
                    (v.pos[1] - gun.pos[1]) as f64,
                );
                let d = (dx * dx + dy * dy).sqrt();
                if d > PAIR_M {
                    continue;
                }
                let a = ents.transform()[SimId(gun.id).index()].pos;
                let b = ents.transform()[SimId(v.id).index()].pos;
                let los = ter.line_of_sight(
                    a.xy().extend(a.z + Fx::from_milli(GUN_EYE_MM)),
                    b.xy().extend(b.z + Fx::from_milli(TARGET_EYE_MM)),
                );
                let mut off = dy.atan2(dx) - gun.yaw as f64;
                while off > std::f64::consts::PI {
                    off -= std::f64::consts::TAU;
                }
                while off < -std::f64::consts::PI {
                    off += std::f64::consts::TAU;
                }
                pairs.push(json!([
                    gun.id,
                    v.id,
                    d.round(),
                    off.to_degrees().round(),
                    los as u8
                ]));
            }
        }
        let own: Vec<_> = g
            .points()
            .iter()
            .map(|p| p.owner.map_or(-1, |o| o.0 as i32))
            .collect();
        let line = json!({"ev": "s", "t": tick, "veh": veh, "pairs": pairs, "own": own});
        writeln!(self.out, "{line}")?;
        Ok(())
    }

    pub fn finish(mut self) -> Result<()> {
        self.out.flush()?;
        Ok(())
    }
}
