//! `--json FILE`: one JSON object with every number the text output prints,
//! for tools and agents that must not read the text (docs/modding/tuning.md).
//!
//! Purely additive: the builders only read finished reports, so they never
//! touch a match. Each document carries `"schema": 1` and a `"kind"`.
//!
//! * skirmish (`--seeds`, `--ab`, or one match): `matches` (count),
//!   `per_match`, `wins`, `length_min`, `he_share_pct`, `tick_p99_ms`,
//!   `tank_losses`, `factions.<short name>` (the summary tables), `s3c`.
//! * armour-test and fortify-test: `cases`/`checks` with `id`, `name`,
//!   `value`, `limit`, `pass`, and report `rows`.
//!
//! Metric paths (the `targets` command) walk objects by key and arrays by
//! index, or by the `id` (else `name`) of an element.

use std::path::Path;

use anyhow::{Context, Result};
use frontline_sim::{FireMissionRefusal, fired_class, kill_class};
use serde_json::{Map, Value, json};

use crate::armour_test::ArmourTestReport;
use crate::fortify::FortifyReport;
use crate::report::round3;
use crate::skirmish::{AbReport, ManyMatch, ManyReport, ManySummary, summarise};

/// Writes `value` as pretty JSON to `path` (parent folders are created).
pub fn write(path: &Path, value: &Value) -> Result<()> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let mut text = serde_json::to_string_pretty(value)?;
    text.push('\n');
    std::fs::write(path, text).with_context(|| format!("writing {}", path.display()))
}

/// `ww2:germany` -> `germany`.
pub fn short(faction: &str) -> &str {
    faction.rsplit(':').next().unwrap_or(faction)
}

/// A lowercase identifier for a check or row name (letters, digits, `_`).
fn slug(name: &str) -> String {
    let mut out = String::new();
    let mut gap = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            if gap && !out.is_empty() {
                out.push('_');
            }
            gap = false;
            out.push(ch.to_ascii_lowercase());
        } else {
            gap = true;
        }
        if out.len() >= 48 {
            break;
        }
    }
    out
}

/// Unique ids for a list of names (`_2`, `_3` on repeats).
fn ids<'a>(names: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut seen: std::collections::BTreeMap<String, u32> = Default::default();
    names
        .map(|n| {
            let s = slug(n);
            let c = seen.entry(s.clone()).or_default();
            *c += 1;
            if *c == 1 { s } else { format!("{s}_{c}") }
        })
        .collect()
}

/// Fix F10: a share over an empty set is no number (null: `targets` skips
/// it), not 0.
fn pct_or_null(a: f64, b: f64) -> Value {
    if b == 0.0 {
        Value::Null
    } else {
        json!(pct(a, b))
    }
}

fn pct(a: f64, b: f64) -> f64 {
    round3(if b == 0.0 { 0.0 } else { a * 100.0 / b })
}

fn median(v: &mut [f64]) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(f64::total_cmp);
    let n = v.len();
    if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    }
}

// ------------------------------------------------------------- skirmish --

/// `--side-detail` (B1 diagnosis): adds `sides`, one object per player, to
/// each `per_match` entry of a [`skirmish`] document. Read-only.
pub fn add_sides(doc: &mut Value, r: &ManyReport) {
    let Some(list) = doc.get_mut("per_match").and_then(Value::as_array_mut) else {
        return;
    };
    for (v, m) in list.iter_mut().zip(&r.matches) {
        let sides: Vec<Value> = m
            .report
            .players
            .iter()
            .map(|p| {
                let f = &p.s10.fort;
                let ai = &p.s10.ai;
                // S10c-ai: the same raw counters as `factions.<f>.s10c`.
                let mut one = crate::skirmish::S10cFaction::default();
                one.add(&p.s10c, &p.s10);
                let mut side = json!({
                    "faction": p.faction,
                    "base": p.base,
                    "defeat": p.defeat,
                    "tickets": p.tickets,
                    "manpower": p.manpower,
                    "kills": p.kills,
                    "losses": p.losses,
                    "squads_fielded": p.squads_fielded,
                    "squads_alive": p.squads_alive,
                    "soldiers_alive": p.soldiers_alive,
                    "points_held": p.points_held,
                    "hold_s": p.hold_s,
                    "pop_at_min": p.s4.pop_at_min,
                    "pop_cap_holds": p.s4.pop_cap_holds,
                    "capture_orders": p.s4.capture_orders,
                    "bought": p.s4.bought,
                    "fired": fired_class::NAMES.iter().zip(p.s5a.fired).map(|(n, v)| ((*n).to_string(), json!(v))).collect::<Map<_, _>>(),
                    "kills_by": kill_class::NAMES.iter().zip(p.s5a.kills_by).map(|(n, v)| ((*n).to_string(), json!(v))).collect::<Map<_, _>>(),
                    "ammo_mp": round3(p.s5a.paid_micro.iter().sum::<u64>() as f64 / 1e6),
                    "fort_jobs": ai[0],
                    "fort_build_plans": ai[2],
                    "infantry_plans": ai[3],
                    "works_started": f.works_started,
                    "pieces_built": f.pieces_built,
                    "pieces_lost": f.pieces_lost,
                    "works_done": p.s10.works_done,
                    "wire_built": f.wire_built,
                    "wire_crushed": f.wire_crushed,
                    "men_lost_in_wire": f.men_lost_in_wire,
                    "fort_mp": round3(f.materials_micro as f64 / 1e6),
                    "s10b_mp": round3(p.s10.s10b_materials_micro as f64 / 1e6),
                    "income_mp": p.s10.income_mp,
                    "mines_laid": f.mines_laid,
                    "obstacles_built": f.obstacles_built,
                    "s10b_jobs": p.s10.s10b_ai[0],
                    "own_blocked": p.s10.s10b_ai[11],
                    "vehicles_lost": p.armour.vehicles_lost,
                    "squads_at_min": p.squads_at_min,
                    "deaths": crate::skirmish::DEATH_PLACES.iter().zip(p.death_places).map(|(n, v)| ((*n).to_string(), json!(v))).collect::<Map<_, _>>(),
                });
                // S11a1: the raw counters of this side.
                let mut s11 = crate::skirmish::S11a1Faction::default();
                s11.add(&p.s11a1, p.kills, p.s10.income_mp);
                // S11a2: the raw friendly-fire and counter-battery counters.
                let mut s11a2 = crate::skirmish::S11a2Faction::default();
                s11a2.add(&p.s11a2, p.losses, p.s11a1.s11.gun_rounds);
                // S11b: the raw counters of this side.
                let mut s11b = crate::skirmish::S11bFaction::default();
                s11b.add(&p.s11b, p.kills, m.report.game_seconds, p.s11a2.ai[2]);
                if let Some(o) = side.as_object_mut() {
                    o.insert("s10c".into(), s10c_blocks(&one).0);
                    o.insert("s11a1".into(), s11a1_blocks(&s11).0);
                    o.insert("s11a2".into(), s11a2_blocks(&s11a2).0);
                    o.insert("s11b".into(), s11b_blocks(&s11b).0);
                    let mut s12 = crate::skirmish::S12Faction::default();
                    s12.add(&p.s12);
                    o.insert("s12".into(), s12_blocks(&s12));
                    let mut s13 = crate::skirmish::S13Faction::default();
                    s13.add(&p.s13);
                    o.insert("s13".into(), s13_blocks(&s13));
                    let mut s14 = crate::skirmish::S14Faction::default();
                    s14.add(&p.s14);
                    o.insert("s14".into(), s14_blocks(&s14));
                }
                side
            })
            .collect();
        if let Some(o) = v.as_object_mut() {
            o.insert("sides".into(), Value::Array(sides));
        }
    }
}

/// The summary document of a `--seeds` run (or of one match).
pub fn skirmish(r: &ManyReport) -> Value {
    let s = &r.summary;
    let n = s.matches as f64;
    let mut lens: Vec<f64> = r.matches.iter().map(|m| m.report.game_seconds).collect();
    let median_min = round3(median(&mut lens) / 60.0);

    let per_match: Vec<Value> = r
        .matches
        .iter()
        .map(|m| {
            let rp = &m.report;
            let p = |k: usize| &rp.players[k].armour;
            json!({
                "seed": m.seed,
                "side": if m.swapped { "swapped" } else { "normal" },
                "result": rp.result,
                "winner_faction": rp.winner_faction,
                "winner_base": rp.armour.winner_base,
                "length_s": round3(rp.game_seconds),
                "he_share_pct": rp.armour.he_share_pct,
                "hash": rp.final_hash,
                "tanks_lost": [p(0).vehicles_lost, p(1).vehicles_lost],
                "rockets_fired": [p(0).at_shots, p(1).at_shots],
                "satchels_at_vehicles": [p(0).satchels_at_vehicles, p(1).satchels_at_vehicles],
            })
        })
        .collect();

    let mut wins = Map::new();
    wins.insert("matches".into(), json!(s.matches));
    wins.insert("no_winner".into(), json!(s.no_winner));
    for f in &s.factions {
        wins.insert(short(&f.faction).into(), json!(f.wins));
        wins.insert(
            format!("{}_pct", short(&f.faction)),
            json!(pct(f.wins as f64, n)),
        );
    }
    for b in 0..2 {
        wins.insert(format!("base{b}"), json!(s.wins_by_base[b]));
        wins.insert(
            format!("base{b}_pct"),
            json!(pct(s.wins_by_base[b] as f64, n)),
        );
    }
    // The larger base side, for "no side above X %" targets.
    wins.insert(
        "base_max_pct".into(),
        json!(pct(s.wins_by_base[0].max(s.wins_by_base[1]) as f64, n)),
    );

    let ko_total: u32 = s.ko_causes.values().sum();
    let rack = s.ko_causes.get("ko_ammo_rack").copied().unwrap_or(0);

    let mut factions = Map::new();
    for f in &s.factions {
        factions.insert(short(&f.faction).into(), faction(f, n));
    }

    json!({
        "schema": 1,
        "kind": "skirmish",
        "matches": s.matches,
        "swapped_matches": r.matches.iter().filter(|m| m.swapped).count(),
        "jobs": s.jobs,
        "overrides": r.overrides,
        "per_match": per_match,
        "wins": wins,
        "length_min": {
            "mean": s.minutes_mean,
            "min": s.minutes_min,
            "median": median_min,
            "max": s.minutes_max,
            "over_25": s.over_25_min,
        },
        "he_share_pct": { "mean": s.he_share_mean_pct, "max": s.he_share_max_pct },
        // S12 Build C (S12-15): T8 over kills plus downs.
        "s12_derived": {
            "he_share_casualties_pct": { "mean": s.he_cas_mean_pct, "max": s.he_cas_max_pct },
        },
        "tick_p99_ms": { "peak": s.peak_p99_max_ms, "all": s.all_p99_max_ms },
        "tank_losses": {
            "total": ko_total,
            "by_cause": s.ko_causes,
            "rack_pct": pct(rack as f64, ko_total as f64),
        },
        "factions": factions,
        "s3c": s.s3c,
        "s6": {
            "cookoffs_per_match": round3(
                s.factions.iter().map(|f| f.s6.cookoffs).sum::<u64>() as f64 / n
            ),
        },
    })
}

/// One faction's tables: the summary struct as is, plus the derived numbers
/// the text prints (percentages, per-match means, per-kind ammunition).
fn faction(f: &crate::skirmish::FactionSummary, matches: f64) -> Value {
    let mut v = serde_json::to_value(f).expect("summary serialises");
    let o = v.as_object_mut().expect("object");
    o.insert("wins_pct".into(), json!(pct(f.wins as f64, matches)));
    let q = &f.s5a;
    let n = q.matches.max(1) as f64;
    let tanks = f.tanks_bought.max(1) as f64;
    let named = |names: &[&str], vals: &dyn Fn(usize) -> f64| {
        let mut m = Map::new();
        for (k, name) in names.iter().enumerate() {
            m.insert((*name).into(), json!(vals(k)));
        }
        Value::Object(m)
    };
    let mp = |k: usize| q.paid_micro[k] as f64 / 1e6;
    let main = q.fired[fired_class::TANK_AP]
        + q.fired[fired_class::TANK_HE]
        + q.fired[fired_class::TANK_RARE];
    let all_kills: u64 = q.kills_by.iter().sum();
    o.insert(
        "ammo".into(),
        json!({
            "fired": named(&fired_class::NAMES, &|k| q.fired[k] as f64),
            "spend_mp": {
                "small_arms": round3(mp(0)),
                "explosives": round3(mp(1)),
                "shells": round3(mp(2)),
                "total": round3(mp(0) + mp(1) + mp(2)),
                "per_match": round3((mp(0) + mp(1) + mp(2)) / n),
            },
            "refilled": { "small_arms": q.refilled[0], "explosives": q.refilled[1], "shells": q.refilled[2] },
            "refill_paused_s": q.paused_s,
            "per_tank_bought": {
                "main_gun": round3(main as f64 / tanks),
                "tank_ap": round3(q.fired[fired_class::TANK_AP] as f64 / tanks),
                "tank_he": round3(q.fired[fired_class::TANK_HE] as f64 / tanks),
                "tank_rare": round3(q.fired[fired_class::TANK_RARE] as f64 / tanks),
                "vehicle_mg": round3(q.fired[fired_class::VEHICLE_MG] as f64 / tanks),
            },
        }),
    );
    o.insert(
        "kills_by_source_pct".into(),
        named(&kill_class::NAMES, &|k| {
            pct(q.kills_by[k] as f64, all_kills as f64)
        }),
    );
    // S4 per-match means and S5b tables with the names of their indices.
    let s4 = &f.s4;
    let n4 = s4.matches.max(1) as f64;
    let pop: Vec<Value> = (0..3)
        .map(|k| {
            if s4.pop_n[k] == 0 {
                Value::Null
            } else {
                json!(round3(s4.pop_sum[k] as f64 / s4.pop_n[k] as f64))
            }
        })
        .collect();
    o.insert(
        "s4_derived".into(),
        json!({
            "pickups_per_match": round3(s4.pickups as f64 / n4),
            "pop_cap_holds_per_match": round3(s4.pop_cap_holds as f64 / n4),
            "mean_pop_at_5_10_20_min": pop,
        }),
    );
    let b = &f.s5b;
    let modes = frontline_sim::fire_mission::mode::NAMES;
    let bn = b.matches.max(1) as f64;
    let bombs: u64 = b.bombs.iter().sum();
    let missions: u64 = b.missions.iter().sum();
    let base: u64 = b.base_bombs.iter().sum();
    let mode_map = |vals: &dyn Fn(usize) -> Value| {
        let mut m = Map::new();
        for (k, name) in modes.iter().enumerate() {
            m.insert((*name).into(), vals(k));
        }
        Value::Object(m)
    };
    let refused: Map<String, Value> = (0..FireMissionRefusal::COUNT)
        .map(|k| {
            (
                FireMissionRefusal::NAMES[k].to_string(),
                json!(b.refused[k]),
            )
        })
        .collect();
    // S6b: `ai.missions` counts orders as planned; dry orders withdrawn
    // before they reach the sim are taken off the total (not per mode: a
    // withdrawn order carries no mode).
    let am_planned: u64 = b.ai.missions.iter().sum();
    let am = am_planned.saturating_sub(b.ai.dry_drops);
    o.insert(
        "s5b_derived".into(),
        json!({
            "missions": mode_map(&|k| json!(b.missions[k])),
            "missions_total": missions,
            "missions_per_match": round3(missions as f64 / bn),
            "observed_direct_pct": pct((b.missions[0] + b.missions[1]) as f64, missions as f64),
            "refused": refused,
            "refused_total": b.refused.iter().sum::<u64>(),
            "bombs": mode_map(&|k| json!(b.bombs[k])),
            "bombs_total": bombs,
            "bombs_per_match": round3(bombs as f64 / bn),
            "mean_planned_miss_m": mode_map(&|k| json!(round3(b.miss_mm[k] as f64 / 1000.0 / b.bombs[k].max(1) as f64))),
            "mean_flight_s": round3(b.flight_ticks as f64 / 30.0 / bombs.max(1) as f64),
            "base_bombs": mode_map(&|k| json!(b.base_bombs[k])),
            "base_bombs_total": base,
            "base_bombs_pct": pct(base as f64, bombs as f64),
            "ai_missions": mode_map(&|k| json!(b.ai.missions[k])),
            "ai_missions_total": am,
            "ai_missions_planned": am_planned,
            "ai_observed_direct_pct": pct((b.ai.missions[0] + b.ai.missions[1]) as f64, am_planned as f64),
            "ai_spotters_per_match": round3(b.ai.spotters as f64 / bn),
        }),
    );
    // S6: per-match means and the T20/T22 checks.
    let t = &f.s6;
    let tn = t.matches.max(1) as f64;
    let per = |v: u64| round3(v as f64 / tn);
    // T22 balance: what the stocks held, gave out and lost, beyond what
    // crates put in (0 when stock enters only from crates).
    let not_crate = (t.stock_held_g + t.stock_out_g + t.stock_lost_g).saturating_sub(t.stock_in_g);
    let fwd_bombs = t.structure_bombs + t.crate_bombs;
    // S7: per-type shares (docs/slices/S7-casemate-td.md 10). Manpower
    // shares are cards bought × card price; vehicle kill shares are of the
    // faction's kills.
    {
        let q4 = &f.s4;
        let u = &f.s7;
        let spent: f64 = q4
            .bought
            .iter()
            .map(|(k, &n)| n as f64 * u.card_cost.get(k).copied().unwrap_or(0) as f64)
            .sum();
        let mut bought_pct = Map::new();
        let mut mp_share = Map::new();
        for (card, &cost) in &u.card_cost {
            let n = q4.bought.get(card).copied().unwrap_or(0) as f64;
            let m = q4.bought_matches.get(card).copied().unwrap_or(0) as f64;
            bought_pct.insert(card.clone(), json!(pct(m, q4.matches as f64)));
            mp_share.insert(card.clone(), json!(pct(n * cost as f64, spent)));
        }
        let vehicle_mp: f64 = u
            .vehicle_card
            .values()
            .map(|c| {
                q4.bought.get(c).copied().unwrap_or(0) as f64
                    * u.card_cost.get(c).copied().unwrap_or(0) as f64
            })
            .sum();
        // S9 (T33, owner S8b-26): tanks and TDs/assault guns only.
        let tank_td_mp: f64 = u
            .vehicle_card
            .iter()
            .filter(|(v, _)| u.tank_td_vehicles.contains(*v))
            .map(|(_, c)| {
                q4.bought.get(c).copied().unwrap_or(0) as f64
                    * u.card_cost.get(c).copied().unwrap_or(0) as f64
            })
            .sum();
        let kills = f.kills.max(1) as f64;
        let mut by_vehicle = Map::new();
        let mut kos_per = Map::new();
        let mut max_pct: f64 = 0.0;
        let mut all = 0u64;
        for (veh, card) in &u.vehicle_card {
            let k = u.kills_by_vehicle.get(veh).copied().unwrap_or(0);
            all += k;
            let p = pct(k as f64, kills);
            max_pct = max_pct.max(p);
            by_vehicle.insert(veh.clone(), json!(p));
            let bought = q4.bought.get(card).copied().unwrap_or(0) as f64;
            let kos = u.kos_by_vehicle.get(veh).copied().unwrap_or(0) as f64;
            kos_per.insert(veh.clone(), json!(round3(kos / bought.max(1.0))));
        }
        o.insert(
            "s7_derived".into(),
            json!({
                "bought_matches_pct": bought_pct,
                "mp_share_pct": mp_share,
                "vehicle_mp_pct": pct(vehicle_mp, spent),
                "tank_td_mp_pct": pct(tank_td_mp, spent),
                "kills_by_vehicle_pct": by_vehicle,
                "vehicle_kills_pct": pct(all as f64, kills),
                "kills_by_vehicle_max_pct": max_pct,
                "kos_per_bought": kos_per,
                "open_top_throws_per_match": round3(u.open_top_throws as f64 / u.matches.max(1) as f64),
                "open_top_hits_per_match": round3(u.open_top_hits as f64 / u.matches.max(1) as f64),
                "relocations_per_match": round3(u.relocations as f64 / u.matches.max(1) as f64),
            }),
        );
    }
    // S8a (docs/slices/S8a-halftracks.md 11): per-match means, the time a
    // ride saved against walking the same straight line, trips per
    // transport bought.
    {
        let e = &f.s8a;
        let en = e.matches.max(1) as f64;
        let rides = e.rides.max(1) as f64;
        let saved_s = (e.ride_walk_ticks as f64 - e.ride_ticks as f64) / 30.0;
        let bought: u64 = e
            .transport_cards
            .iter()
            .map(|c| f.s4.bought.get(c).copied().unwrap_or(0) as u64)
            .sum();
        o.insert(
            "s8a_derived".into(),
            json!({
                "transported_per_match": round3(e.transported as f64 / en),
                "rides_per_match": round3(e.rides as f64 / en),
                "ride_saved_s_per_ride": round3(saved_s / rides),
                "ride_saved_s_per_match": round3(saved_s / en),
                "ride_s_per_ride": round3(e.ride_ticks as f64 / 30.0 / rides),
                // Review M2: from the Embark order to the point, against a
                // straight walk from where the squad stood at the order.
                "obj_rides_per_match": round3(e.obj_rides as f64 / en),
                "obj_saved_s_per_ride": round3(
                    (e.obj_walk_ticks as f64 - e.obj_ride_ticks as f64) / 30.0 / e.obj_rides.max(1) as f64
                ),
                "obj_saved_s_per_match": round3(
                    (e.obj_walk_ticks as f64 - e.obj_ride_ticks as f64) / 30.0 / en
                ),
                "obj_ride_s_per_ride": round3(e.obj_ride_ticks as f64 / 30.0 / e.obj_rides.max(1) as f64),
                "obj_walk_s_per_ride": round3(e.obj_walk_ticks as f64 / 30.0 / e.obj_rides.max(1) as f64),
                "obj_lost_per_match": round3(e.obj_lost as f64 / en),
                "obj_board_s_per_ride": round3(e.ride[0] as f64 / 30.0 / e.obj_rides.max(1) as f64),
                "obj_foot_s_per_ride": round3(e.ride[1] as f64 / 30.0 / e.obj_rides.max(1) as f64),
                "riders_lost_per_match": round3(e.riders_lost as f64 / en),
                "small_arms_pens_per_match": round3(e.small_arms_pens as f64 / en),
                "dismounts_under_fire_pct": pct(e.dismounts_under_fire as f64, e.rides as f64),
                "transports_bought": bought,
                "transport_trips_per_bought": round3(e.transport_trips as f64 / bought.max(1) as f64),
            }),
        );
    }
    {
        let e = &f.s8a;
        let en = e.matches.max(1) as f64;
        let unloads: u64 = e.ride[2] + e.ride[3] + e.ride[4] + e.ride[5];
        o.insert(
            "s8b".into(),
            json!({
                "unload_exposed": e.ride[2],
                "unload_arrived": e.ride[3],
                "unload_stuck": e.ride[4],
                "unload_fire": e.ride[5],
                "recon_stops": e.ai8b[0],
                "recon_backoffs": e.ai8b[1],
                "recon_fails": e.ai8b[2],
                "truck_fails": e.ai8b[3],
                "truck_flees": e.ai8b[4],
                "truck_serves": e.ai8b[5],
                "stranded_served": e.ai8b[6],
                "truck_drops_ordered": e.ai8b[7],
                "truck_loaded_kg": e.truck_loaded_g / 1000,
                "truck_tank_rounds": e.truck_tank_rounds,
                "truck_men_rounds": e.truck_men_rounds,
                "truck_crates_dropped": e.truck_crates_dropped,
                "truck_lost_kg": e.truck_lost_g / 1000,
            }),
        );
        o.insert(
            "s8b_derived".into(),
            json!({
                "unload_exposed_pct": pct(e.ride[2] as f64, unloads as f64),
                "unload_arrived_pct": pct(e.ride[3] as f64, unloads as f64),
                "recon_stops_per_match": round3(e.ai8b[0] as f64 / en),
                "recon_backoffs_per_match": round3(e.ai8b[1] as f64 / en),
                "stranded_served_per_match": round3(e.ai8b[6] as f64 / en),
                "truck_serves_per_match": round3(e.ai8b[5] as f64 / en),
                "truck_crates_dropped_per_match": round3(e.truck_crates_dropped as f64 / en),
                "truck_tank_rounds_per_match": round3(e.truck_tank_rounds as f64 / en),
                "truck_men_rounds_per_match": round3(e.truck_men_rounds as f64 / en),
                "truck_loaded_kg_per_match": round3(e.truck_loaded_g as f64 / 1000.0 / en),
                "cargo_detonations_per_match": round3(e.cargo_detonations as f64 / en),
                "ai_move_fails_per_match": round3(
                    (e.ai_fails + e.ai8b[2] + e.ai8b[3]) as f64 / en
                ),
            }),
        );
    }
    // S9: the guns (`docs/slices/S9-at-guns.md` 16).
    {
        let e = &f.s8a;
        let u = &f.s7;
        let q4 = &f.s4;
        let en = e.matches.max(1) as f64;
        let c = &e.s9;
        let in_cover = e.s9_ai[1];
        let spent: f64 = u
            .card_cost
            .iter()
            .map(|(k, &cost)| q4.bought.get(k).copied().unwrap_or(0) as f64 * cost as f64)
            .sum();
        let gun_mp: f64 = u
            .vehicle_card
            .iter()
            .filter(|(v, _)| u.gun_vehicles.contains(*v))
            .map(|(_, k)| {
                q4.bought.get(k).copied().unwrap_or(0) as f64
                    * u.card_cost.get(k).copied().unwrap_or(0) as f64
            })
            .sum();
        o.insert(
            "s9".into(),
            json!({
                "guns_bought": c.guns_bought, "guns_emplaced": c.guns_emplaced,
                "guns_in_cover": in_cover, "gun_rounds": c.gun_rounds,
                "gun_ko_tanks": c.gun_ko_tanks, "gun_armour_out": e.gun_armour_out,
                "gun_crew_lost": c.gun_crew_lost,
                "guns_neutral": c.guns_neutral, "guns_captured": c.guns_captured,
                "guns_recrewed": c.guns_recrewed, "tows_lost": c.tows_lost,
                "ai_unhitches": e.s9_ai[0], "ai_ambush_rounds": c.ai_ambush_rounds,
                "gun_ai_fails": e.s9_ai[2], "gun_pits": e.s9_ai[3],
                "gun_relays": e.s9_ai[4], "gun_recrew_orders": e.s9_ai[5],
                "gun_resites": e.s9_ai[6],
            }),
        );
        o.insert(
            "s9_derived".into(),
            json!({
                "gun_fired_matches_pct": pct(e.gun_fired_matches as f64, en),
                "gun_in_cover_pct": pct(in_cover as f64, c.guns_emplaced as f64),
                "guns_bought_per_match": round3(c.guns_bought as f64 / en),
                "guns_emplaced_per_match": round3(c.guns_emplaced as f64 / en),
                "gun_rounds_per_match": round3(c.gun_rounds as f64 / en),
                "gun_ko_tanks_per_match": round3(c.gun_ko_tanks as f64 / en),
                "gun_armour_out_per_match": round3(e.gun_armour_out as f64 / en),
                "gun_crew_lost_per_match": round3(c.gun_crew_lost as f64 / en),
                "guns_neutral_per_match": round3(c.guns_neutral as f64 / en),
                "guns_captured_per_match": round3(c.guns_captured as f64 / en),
                "captured_gun_kills_per_match": round3(c.captured_gun_kills as f64 / en),
                "gun_mp_pct": pct(gun_mp, spent),
            }),
        );
    }
    // S10a: fortification counters; S10a-ai: the AI's counters and the
    // derived shares (docs/slices/S10a-ai.md 5).
    {
        let (raw, derived) = s10_blocks(&f.s10);
        o.insert("s10".into(), raw);
        o.insert("s10_derived".into(), derived);
    }
    // S10b (spec 7, R28): the twelve counters and `used`.
    {
        let b = &f.s10b;
        let mut m = Map::new();
        for (k, v) in crate::skirmish::S10B_COUNTERS.iter().zip(b.counters) {
            m.insert((*k).into(), json!(v));
        }
        m.insert("used".into(), json!(b.used()));
        o.insert("s10b".into(), Value::Object(m));
    }
    // S10b-ai (Build A step 8): the AI's counters and the derived shares.
    {
        let (raw, derived) = s10b_ai_blocks(&f.s10b_ai);
        o.insert("s10b_ai".into(), raw);
        o.insert("s10b_ai_derived".into(), derived);
    }
    // S10c-ai (docs/slices/S10c-ai.md 7): counters, exposure, shares.
    {
        let (raw, derived) = s10c_blocks(&f.s10c);
        o.insert("s10c".into(), raw);
        o.insert("s10c_derived".into(), derived);
    }
    // S11a1 (docs/slices/S11a1.md 7): counters by name, derived shares.
    {
        let (raw, derived) = s11a1_blocks(&f.s11a1);
        o.insert("s11a1".into(), raw);
        o.insert("s11a1_derived".into(), derived);
    }
    // S11a2 (docs/slices/S11a2.md 7): counters by name, derived shares.
    {
        let (raw, derived) = s11a2_blocks(&f.s11a2);
        o.insert("s11a2".into(), raw);
        o.insert("s11a2_derived".into(), derived);
    }
    // S11b (docs/slices/S11b.md 7): counters by name, derived shares.
    {
        let (raw, derived) = s11b_blocks(&f.s11b);
        o.insert("s11b".into(), raw);
        o.insert("s11b_derived".into(), derived);
    }
    // S12 (docs/slices/S12-wounds-medics.md 3, 8): counters by name and
    // the derived measures (Build C).
    o.insert("s12".into(), s12_blocks(&f.s12));
    o.insert("s12_derived".into(), s12_derived(&f.s12));
    // S13 (docs/slices/S13-heavy-wave.md 7): the AI's heavy-wave counters
    // and the heavy-wave cards' buys, match shares, kills and knock-outs.
    o.insert("s13".into(), s13_blocks(&f.s13));
    // S14 (docs/slices/S14-assault-kit.md 10): the S14 counters and the
    // AI's flame counters by name (T195-T200 read
    // `factions.<f>.s14.<counter>`).
    o.insert("s14".into(), s14_blocks(&f.s14));
    o.insert(
        "s6_derived".into(),
        json!({
            "hq_built_pct": pct(t.hq_built as f64, t.matches as f64),
            "structures_built_per_match": per(t.structures_built),
            "structures_lost_per_match": per(t.structures_lost),
            "crates_issued_per_match": {
                "small_arms": per(t.crates_issued[0]),
                "explosives": per(t.crates_issued[1]),
                "shells": per(t.crates_issued[2]),
            },
            "crate_mp_per_match": round3(t.crate_micro as f64 / 1e6 / tn),
            "crates_delivered_per_match": per(t.crates_delivered),
            "crates_lost_per_match": per(t.crates_lost),
            "stock_in_kg_per_match": round3(t.stock_in_g as f64 / 1000.0 / tn),
            "stock_in_not_crate_kg": round3(not_crate as f64 / 1000.0),
            "base_bomb_refills": t.base_bombs,
            "bomb_refills_unsourced": t.unsourced_bombs,
            "bombs_from_structures_per_match": per(t.structure_bombs),
            "bombs_from_crates_per_match": per(t.crate_bombs),
            "bombs_forward_per_match": per(fwd_bombs),
            "small_arms_forward_per_match": per(t.fwd_rounds),
            "items_forward_per_match": per(t.fwd_items),
            "tank_rounds_forward_per_match": per(t.tank_rounds_fwd + t.tank_rare_fwd),
            "tank_rounds_from_crates_per_match": per(t.tank_crate_rounds),
            "rack_credit_mp_per_match": per(t.rack_credit_mp),
            "cookoffs_per_match": per(t.cookoffs),
            "tank_crate_runs_per_match": per(t.tank_crate_runs),
            "crate_pickups_per_match": per(t.crate_pickups),
        }),
    );
    v
}

/// Builds the document for one finished match (a one-match summary).
pub fn skirmish_one(m: ManyMatch, overrides: Vec<String>) -> Value {
    let matches = vec![m];
    let summary: ManySummary = summarise(&matches, 1);
    skirmish(&ManyReport {
        matches,
        summary,
        overrides,
    })
}

/// `--ab`: both runs side by side.
pub fn skirmish_ab(r: &AbReport) -> Value {
    json!({ "schema": 1, "kind": "skirmish-ab", "a": skirmish(&r.a), "b": skirmish(&r.b) })
}

// -------------------------------------------------------- armour-test --

/// The armour-test document: every case with its hard checks and rows.
pub fn armour(r: &ArmourTestReport) -> Value {
    let mut hard_failed = 0;
    let cases: Vec<Value> = r
        .cases
        .iter()
        .map(|c| {
            let cids = ids(c.checks.iter().map(|k| k.name.as_str()));
            let rids = ids(c.rows.iter().map(|k| k.name.as_str()));
            let checks: Vec<Value> = c
                .checks
                .iter()
                .zip(&cids)
                .map(|(k, id)| {
                    hard_failed += u32::from(!k.pass);
                    json!({
                        "id": id,
                        "name": k.name,
                        "value": k.value.map(round3),
                        "limit": k.limit,
                        "pass": k.pass,
                        "detail": k.detail,
                    })
                })
                .collect();
            let rows: Vec<Value> = c
                .rows
                .iter()
                .zip(&rids)
                .map(|(k, id)| {
                    json!({ "id": id, "name": k.name, "value": k.value, "target": k.target })
                })
                .collect();
            json!({
                "id": format!("case{}", c.case),
                "case": c.case,
                "name": c.name,
                "runs": c.runs,
                "wall_s": c.wall_s,
                "pass": c.checks.iter().all(|k| k.pass),
                "checks": checks,
                "rows": rows,
            })
        })
        .collect();
    json!({
        "schema": 1,
        "kind": "armour-test",
        "seeds": r.seeds,
        "threads": r.threads,
        "stress": r.stress,
        "wall_s": r.wall_s,
        "pass": r.pass,
        "hard_failed": hard_failed,
        "cases": cases,
    })
}

// ------------------------------------------------------- fortify-test --

fn check(id: &str, name: &str, value: Option<f64>, limit: String, pass: bool) -> Value {
    json!({ "id": id, "name": name, "value": value, "limit": limit, "pass": pass })
}

fn row(id: &str, name: &str, value: Value) -> Value {
    json!({ "id": id, "name": name, "value": value })
}

/// The S14 hard checks (R2; R1 only while `SLIT_HARD`, else a report
/// row): a failing one raises the document's `hard_failed`. A row not run
/// (no verdict) is no check.
fn s14_checks(s14: &crate::fortify::s14::S14Report) -> Vec<Value> {
    let (slit, pool, melee) = (&s14.slit, &s14.pool, &s14.melee);
    let mut checks = Vec::new();
    // S14a Build C: R5 (T188).
    if let Some(pass) = melee.pass {
        checks.push(check(
            "s14_melee",
            "S14 R5: a strike hits only a touched capsule",
            melee.ahead_hits,
            "100 strikes 0.9 m ahead hit 100; 0.9 m aside hit 0".into(),
            pass,
        ));
    }
    if let Some(pass) = pool.pass {
        checks.push(check(
            "s14_pool",
            "S14 R2: flame never takes a pool slot",
            pool.drop_diff,
            "drop_diff, pool_diff_ticks, over_cap 0; saturated, fired".into(),
            pass,
        ));
    }
    if slit.hard
        && let Some(pass) = slit.pass
    {
        checks.push(check(
            "s14_slit",
            "S14 R1: flame at the bunker slit",
            slit.out_pct,
            "out >= 80 %, 8 of 10 runs >= 4 out; closed, back <= 10 %".into(),
            pass,
        ));
    }
    checks
}

/// The fortify-test document: hard checks (rifles, mortar, smoke) and the
/// report rows (ranged mortar, grenades, MG suppression).
pub fn fortify(r: &FortifyReport) -> Value {
    let mut checks = vec![
        check(
            "rifles_trench_vs_open",
            r.rifles.name,
            r.rifles.ratio_pct,
            format!("<= {} %", r.rifles.limit_pct),
            r.rifles.pass,
        ),
        check(
            "mortar_trench_vs_open",
            r.mortar.name,
            r.mortar.ratio_pct,
            format!("<= {} %", r.mortar.limit_pct),
            r.mortar.pass,
        ),
        check(
            "smoke_hides_squad",
            r.smoke.name,
            None,
            "never seen or fired on behind smoke; control seen".into(),
            r.smoke.pass,
        ),
    ];
    checks.extend(s14_checks(&r.s14));
    let slit = &r.s14.slit;
    let exp = |e: &crate::fortify::Experiment| {
        json!({
            "trench_losses": e.trench_losses,
            "open_losses": e.open_losses,
            "ratio_pct": e.ratio_pct,
            "limit_pct": e.limit_pct,
        })
    };
    let totals = |t: &crate::fortify::SuppressTotals| {
        json!({
            "defender_losses": t.defender_losses,
            "defender_men": t.defender_men,
            "defender_loss_pct": pct(t.defender_losses as f64, t.defender_men as f64),
            "attacker_losses": t.attacker_losses,
            "attacker_men": t.attacker_men,
            "attacker_loss_pct": pct(t.attacker_losses as f64, t.attacker_men as f64),
            "defender_ducked_pct": pct(t.defender_ducked as f64, t.defender_ticks as f64),
            "defender_pinned_pct": pct(t.defender_pinned as f64, t.defender_ticks as f64),
        })
    };
    let variants: Vec<Value> = r
        .grenade
        .variants
        .iter()
        .map(|v| {
            json!({
                "start_m": v.start_m,
                "smoke": v.smoke,
                "defender_losses_grenades": v.grenades.defender_losses,
                "defender_losses_rifles": v.rifle_only.defender_losses,
                "attacker_losses_grenades": v.grenades.attacker_losses,
                "attacker_losses_rifles": v.rifle_only.attacker_losses,
                "frags_thrown": v.grenades.frags_thrown,
                "frags_in_trench": v.grenades.frags_in_trench,
                "ratio_pct": v.ratio_pct,
            })
        })
        .collect();
    let rows = vec![
        row("rifles", "rifles totals", exp(&r.rifles)),
        row("mortar", "mortar totals", exp(&r.mortar)),
        row(
            "mortar_ranged",
            "mortar after ranging (report only)",
            exp(&r.mortar_ranged),
        ),
        row("grenade", "grenade assault variants", json!(variants)),
        row(
            "suppress_supported",
            "assault with the MG (T10)",
            totals(&r.suppress.supported),
        ),
        row(
            "suppress_unsupported",
            "assault without the MG (T11)",
            totals(&r.suppress.unsupported),
        ),
        row(
            "suppress_defended",
            "assault with the MG against a defending MG (report)",
            totals(&r.suppress.defended),
        ),
        row(
            "suppress_met",
            "T10 / T11 thresholds met",
            json!({ "t10": r.suppress.supported_met, "t11": r.suppress.unsupported_met }),
        ),
        row(
            "s14_slit",
            "S14 R1: flame at the bunker slit (report while not hard)",
            json!({ "hard": slit.hard, "pass": slit.pass, "out_pct": slit.out_pct }),
        ),
    ];
    // S10a (spec 7): the wire, roof and breach rows, at `s10a.*`.
    let x = &r.s10a;
    let breach = |t: &crate::fortify::s10a::BreachTotals| {
        json!({
            "runs": t.runs,
            "defender_losses": t.defender_losses,
            "defender_men": t.defender_men,
            "attacker_losses": t.attacker_losses,
            "attacker_men": t.attacker_men,
            "defender_loss_pct": t.defender_loss_pct(),
            "attacker_loss_pct": t.attacker_loss_pct(),
            "gap_opened_pct": t.gap_opened_pct(),
        })
    };
    let mut fortified = breach(&x.breach.fortified);
    fortified["control"] = breach(&x.breach.control);
    // S10a-ai (spec 6): the AI-driven row; `placed_pct` beside the rest.
    let ai_row = |t: &crate::fortify::s10a::BreachTotals| {
        let mut v = breach(t);
        v["placed"] = json!(t.placed);
        v["placed_pct"] = json!(t.placed_pct());
        v
    };
    let mut breach_ai = ai_row(&x.breach_ai.fortified);
    breach_ai["control"] = ai_row(&x.breach_ai.control);
    let s10a = json!({
        "wire": {
            "inside_speed_pct": x.wire.inside_speed_pct,
            "cut_gap_speed_pct": x.wire.cut_gap_speed_pct,
            "blown_gap_speed_pct": x.wire.blown_gap_speed_pct,
            "runs": x.wire.runs.len(),
        },
        "roof": {
            "survived_81mm_pct": x.roof.survived_81mm_pct,
            "sheltered_losses": x.roof.sheltered_losses,
            "broken_heavy_pct": x.roof.broken_heavy_pct,
            "runs": x.roof.runs.len(),
        },
        "breach": fortified,
        "breach_ai": breach_ai,
    });
    json!({
        "schema": 1,
        "kind": "fortify-test",
        "pass": r.pass,
        "s10a": s10a,
        "s10b": serde_json::to_value(&r.s10b).unwrap_or_default(),
        // S10b-ai Build E stage check: the S10b-ai rows (T110i-T116b read
        // `fortify10.s10b_ai.*`); before this they were only in the text.
        "s10b_ai": serde_json::to_value(&r.s10b_ai).unwrap_or_default(),
        // S10c-ai Build C chunk 2 (C7): `fortify10.s10c.belt.*`.
        "s10c": serde_json::to_value(&r.s10c).unwrap_or_default(),
        // S11a1 Build B (C5): `fortify10.s11a1.{gun_fm,mask,barrage,breach_prep}.*`.
        "s11a1": serde_json::to_value(&r.s11a1).unwrap_or_default(),
        // S11a2 Build B (M2, M3): `fortify10.s11a2.{ff,cb,ff_lift}.*`.
        "s11a2": serde_json::to_value(&r.s11a2).unwrap_or_default(),
        // S11b Build C (M2): `fortify10.s11b.{how_mask,how_barrage}.*`.
        "s11b": serde_json::to_value(&r.s11b).unwrap_or_default(),
        // S14 Builds A, B (R1-R4, R6):
        // `fortify10.s14.{slit,pool,trench,clear,under_fire}.*`.
        "s14": serde_json::to_value(&r.s14).unwrap_or_default(),
        "hard_failed": checks.iter().filter(|c| c["pass"] == json!(false)).count(),
        "checks": checks,
        "rows": rows,
    })
}

// ------------------------------------------------------- metric paths --

/// Walks `path` (dot-separated) into `v`. Object segments are keys; array
/// segments are an index or the `id` (else `name`) of an element.
pub fn lookup<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cur = v;
    for seg in path.split('.').filter(|s| !s.is_empty()) {
        cur = match cur {
            Value::Object(m) => m.get(seg)?,
            Value::Array(a) => match seg.parse::<usize>() {
                Ok(i) => a.get(i)?,
                Err(_) => a.iter().find(|e| {
                    e.get("id").and_then(Value::as_str) == Some(seg)
                        || e.get("name").and_then(Value::as_str) == Some(seg)
                })?,
            },
            _ => return None,
        };
    }
    Some(cur)
}

/// S10a and S10a-ai: a faction's `s10` sums and `s10_derived` shares
/// (docs/slices/S10a-ai.md 5).
pub fn s10_blocks(c: &crate::skirmish::S10Faction) -> (Value, Value) {
    let n = c.matches.max(1) as f64;
    let per = |v: u64| round3(v as f64 / n);
    let kg = |g: u64| round3(g as f64 / 1000.0);
    let ai = |name: &str| {
        crate::skirmish::S10_AI
            .iter()
            .position(|k| *k == name)
            .map_or(0, |k| c.ai[k])
    };
    let mut raw = Map::new();
    for (k, v) in [
        ("works_started", c.works_started),
        ("pieces_built", c.pieces_built),
        ("pieces_lost", c.pieces_lost),
        ("wire_built", c.wire_built),
        ("wire_cut", c.wire_cut),
        ("wire_blown", c.wire_blown),
        ("wire_crushed", c.wire_crushed),
        ("bangalores", c.bangalores),
        ("new_cards_bought", c.new_cards_bought),
        ("men_lost_in_wire", c.men_lost_in_wire),
    ] {
        raw.insert(k.into(), json!(v));
    }
    for (k, g) in [
        ("materials_loaded_kg", c.materials_loaded_g),
        ("materials_used_kg", c.materials_used_g),
        ("materials_lost_kg", c.materials_lost_g),
        ("materials_used_crate_kg", c.materials_used_crate_g),
        ("materials_used_truck_kg", c.materials_used_truck_g),
    ] {
        raw.insert(k.into(), json!(kg(g)));
    }
    for (k, name) in crate::skirmish::S10_AI.iter().enumerate() {
        raw.insert((*name).into(), json!(c.ai[k]));
    }
    raw.insert("works_done".into(), json!(c.works_done));
    let runs = ai("material_runs_truck") + ai("material_runs_crate") + ai("material_runs_walk");
    let derived = json!({
        "matches_with_works_pct": pct(c.matches_with_works as f64, c.matches as f64),
        "matches_with_wire_pct": pct(c.matches_with_wire as f64, c.matches as f64),
        "matches_with_gap_pct": pct(c.matches_with_gap as f64, c.matches as f64),
        "breach_when_enemy_wire_pct": pct_or_null(
            c.breach_when_enemy_wire as f64,
            c.matches_enemy_wire as f64
        ),
        "builder_share_pct": pct(ai("fort_build_plans") as f64, ai("infantry_plans") as f64),
        "materials_mp_pct": pct(c.materials_micro as f64 / 1e6, c.income_mp as f64),
        "materials_loaded_kg_per_match": round3(c.materials_loaded_g as f64 / 1000.0 / n),
        "fort_refused_per_match": per(ai("fort_refused")),
        "new_cards_per_match": per(c.new_cards_bought),
        "works_per_match": per(c.works_non_wire),
        "wire_sections_per_match": per(c.wire_built),
        "breaches_started_per_match": per(ai("breaches_started")),
        "breach_opened_pct": pct(ai("breaches_opened") as f64, ai("breaches_started") as f64),
        "men_lost_in_wire_per_match": per(c.men_lost_in_wire),
        "materials_lost_kg_per_match": round3(c.materials_lost_g as f64 / 1000.0 / n),
        "hq_roofed_pct": pct_or_null(c.matches_hq_roofed as f64, c.matches_hq_built as f64),
        "shelter_moves_per_match": per(ai("shelter_moves")),
        "wire_bypassed_per_match": per(ai("wire_bypassed")),
        "material_runs_per_match": per(runs),
    });
    (Value::Object(raw), derived)
}

/// S10b-ai: a faction's `s10b_ai` counters and `s10b_ai_derived` shares
/// (`docs/slices/S10b-ai-build-A.md` 8). Shares over a subset of matches
/// (a bunker chance, a sweep or demolition wanted) are null when it is
/// empty; every field is 0 or null when no S10b-ai behaviour ran.
pub fn s10b_ai_blocks(c: &crate::skirmish::S10bAiFaction) -> (Value, Value) {
    let n = c.matches.max(1) as f64;
    let per = |v: u64| round3(v as f64 / n);
    let mut raw = Map::new();
    for (k, name) in crate::skirmish::S10B_AI.iter().enumerate() {
        raw.insert((*name).into(), json!(c.ai[k]));
    }
    for (k, v) in [
        ("lost_on_known", c.lost_on_known),
        ("at_exposure_ticks", c.at_exposure_ticks),
        ("own_mine_losses", c.own_mine_losses),
        ("own_pieces_demolished", c.own_pieces_demolished),
    ] {
        raw.insert(k.into(), json!(v));
    }
    // Build D: why no bunker started (per plan) and how bunker jobs ended.
    for (k, name) in frontline_sim::ai::BUNKER_WHY.iter().enumerate() {
        raw.insert(format!("bunker_why_{name}"), json!(c.bunker_why[k]));
    }
    let m = c.matches as f64;
    let derived = json!({
        "matches_with_mines_pct": pct(c.matches_with_mines as f64, m),
        "mines_laid_per_match": per(c.mines_laid),
        "at_fields_per_match": per(c.at_fields),
        "lay_chance_matches": c.lay_chance_matches,
        "tanks_stopped_per_match": per(c.vehicles_mined),
        "at_exposure_s_per_match": round3(c.at_exposure_ticks as f64 / c.tick_hz.max(1) as f64 / n),
        "matches_with_obstacle_pct": pct(c.matches_with_obstacle as f64, m),
        "obstacles_per_match": per(c.obstacles_built),
        "obst_chance_matches": c.obst_chance_matches,
        // Review finding 8: over all matches; the job-completion rate
        // under its own name; the exposure from the plans' checks.
        "matches_with_bunker_pct": pct(c.matches_with_bunker as f64, m),
        "bunker_ready_matches": c.bunker_ready_matches,
        "bunker_done_when_started_pct": pct_or_null(c.bunker_matches as f64, c.bunker_chance_matches as f64),
        "bunker_chance_matches": c.bunker_chance_matches,
        "sweep_when_wanted_pct": pct_or_null(c.sweep_when_wanted as f64, c.sweep_wanted_matches as f64),
        "sweep_wanted_matches": c.sweep_wanted_matches,
        "mines_swept_per_match": per(c.mines_lifted),
        "demolish_when_wanted_pct": pct_or_null(
            c.demolish_when_wanted as f64,
            c.demolish_wanted_matches as f64
        ),
        "demolish_wanted_matches": c.demolish_wanted_matches,
        "gaps_opened_per_match": per(c.get("gaps_opened")),
        "own_side_mistakes": c.get("own_blocked") + c.own_mine_losses + c.own_pieces_demolished,
        "s10b_materials_mp_pct": pct(c.materials_micro as f64 / 1e6, c.income_mp as f64),
        "jobs_overdue": c.get("jobs_overdue"),
        "refused_reorders": c.get("refused_reorders"),
        "lost_on_known_per_match": per(c.lost_on_known),
    });
    (Value::Object(raw), derived)
}

/// S11a1: a faction's `s11a1` (raw: every `S11Counters` field by name,
/// arrays by mode name; every S11a1 `AiCounters` field; the two new
/// refusals as `refused_masked` / `refused_not_ready`; denominators) and
/// `s11a1_derived` (spec 7 names; null where the denominator is 0).
/// Gun kills also count as `tank_he` in `kills_by` and in
/// `tank_mp_killed` (the round class of a gun's HE; review A-3): the flag
/// `gun_kills_in_tank_he` says so.
pub fn s11a1_blocks(c: &crate::skirmish::S11a1Faction) -> (Value, Value) {
    use frontline_sim::fire_mission::mode;
    let n = c.matches.max(1) as f64;
    let m = c.matches as f64;
    let per = |v: u64| round3(v as f64 / n);
    let by_mode = |a: &[u64; 3]| -> Value {
        Value::Object(
            mode::NAMES
                .iter()
                .zip(a.iter())
                .map(|(k, v)| ((*k).to_string(), json!(v)))
                .collect(),
        )
    };
    let mut raw = Map::new();
    raw.insert("matches".into(), json!(c.matches));
    raw.insert("gun_missions".into(), by_mode(&c.gun_missions));
    raw.insert("gun_rounds".into(), by_mode(&c.gun_rounds));
    raw.insert("gun_miss_mm".into(), by_mode(&c.gun_miss_mm));
    for (k, v) in [
        ("masked", c.masked),
        ("not_ready", c.not_ready),
        ("refused_masked", c.masked),
        ("refused_not_ready", c.not_ready),
        ("radio_rounds", c.radio_rounds),
        ("low_rounds", c.low_rounds),
        ("high_rounds", c.high_rounds),
        ("gun_breaks", c.gun_breaks),
        ("gun_shell_micro", c.gun_shell_micro),
        ("gun_kills", c.gun_kills),
        ("guns_bought", c.guns_bought),
        ("gun_direct_rounds", c.gun_direct_rounds),
        ("near_own_bursts", c.near_own_bursts),
        ("kills", c.kills),
        ("income_mp", c.income_mp),
    ] {
        raw.insert(k.into(), json!(v));
    }
    for (k, name) in crate::skirmish::S11A1_AI.iter().enumerate() {
        raw.insert((*name).into(), json!(c.ai[k]));
    }
    for (k, v) in [
        ("matches_with_gun", c.matches_with_gun),
        ("matches_gun_fired", c.matches_gun_fired),
        ("matches_with_fo", c.matches_with_fo),
    ] {
        raw.insert(k.into(), json!(v));
    }
    raw.insert("gun_kills_in_tank_he".into(), json!(true));
    let rounds: u64 = c.gun_rounds.iter().sum();
    let seen = (c.gun_rounds[mode::DIRECT as usize] + c.gun_rounds[mode::OBSERVED as usize]) as f64;
    let posts = c.get("fo_posts") + c.get("fo_post_fails");
    let with_target = c.get("arty_missions") + c.get("arty_no_observer");
    let derived = json!({
        "gun_bought_matches_pct": pct_or_null(c.matches_with_gun as f64, m),
        "guns_bought_per_match": per(c.guns_bought),
        "fo_bought_matches_pct": pct_or_null(c.matches_with_fo as f64, m),
        "gun_fired_matches_pct": pct_or_null(c.matches_gun_fired as f64, c.matches_with_gun as f64),
        "gun_rounds_per_match": per(rounds),
        "gun_observed_pct": pct_or_null(seen, rounds as f64),
        "gun_kill_share_pct": pct_or_null(c.gun_kills as f64, c.kills as f64),
        "gun_shell_mp_pct": pct_or_null(c.gun_shell_micro as f64 / 1e6, c.income_mp as f64),
        "near_own_bursts_per_match": per(c.near_own_bursts),
        "gun_refused_total": c.gun_refused,
        "prep_missions_per_match": per(c.get("arty_prep_missions") + c.get("mortar_prep_missions")),
        "fo_lost_per_match": per(c.get("fo_lost")),
        "fo_post_fail_pct": pct_or_null(c.get("fo_post_fails") as f64, posts as f64),
        "arty_no_observer_pct": pct_or_null(c.get("arty_no_observer") as f64, with_target as f64),
    });
    (Value::Object(raw), derived)
}

/// S11a2: a faction's `s11a2` (raw: every `S11a2Counters` field by name,
/// `ff_kills` as {mortar, gun}, arrays by mode name; every S11a2
/// `AiCounters` field, the AI's `cb_missions` as `ai_cb_missions` (the
/// sim's own `cb_missions` counts every mission on a marker, any
/// player); `matches_with_marker`, `matches_with_counter_fire`) and
/// `s11a2_derived` (spec 7 names; null where the denominator is 0).
/// Counter-fire bursts count also after the mission ended (`cb_bursts`,
/// `cb_near_bursts`; S11a2 fix B-2).
pub fn s11a2_blocks(c: &crate::skirmish::S11a2Faction) -> (Value, Value) {
    use frontline_sim::fire_mission::mode;
    let n = c.matches.max(1) as f64;
    let per = |v: u64| round3(v as f64 / n);
    let by_mode = |a: &[u64; 3]| -> Value {
        Value::Object(
            mode::NAMES
                .iter()
                .zip(a.iter())
                .map(|(k, v)| ((*k).to_string(), json!(v)))
                .collect(),
        )
    };
    let mut raw = Map::new();
    raw.insert("matches".into(), json!(c.matches));
    raw.insert(
        "ff_kills".into(),
        json!({"mortar": c.ff_kills[0], "gun": c.ff_kills[1]}),
    );
    raw.insert("cb_rounds".into(), by_mode(&c.cb_rounds));
    for (k, v) in [
        ("ff_wounds", c.ff_wounds),
        ("ff_bursts", c.ff_bursts),
        ("mortar_near_own_bursts", c.mortar_near_own_bursts),
        ("cb_heard", c.cb_heard),
        ("cb_marks", c.cb_marks),
        ("cb_err_first_mm", c.cb_err_first_mm),
        ("cb_dropped", c.cb_dropped),
        ("cb_missions", c.cb_missions),
        ("cb_bursts", c.cb_bursts),
        ("cb_near_bursts", c.cb_near_bursts),
        ("losses", c.losses),
    ] {
        raw.insert(k.into(), json!(v));
    }
    for (k, name) in crate::skirmish::S11A2_AI.iter().enumerate() {
        raw.insert((*name).into(), json!(c.ai[k]));
    }
    for (k, v) in [
        ("matches_with_marker", c.matches_with_marker),
        ("matches_marker_fired", c.matches_marker_fired),
        ("matches_with_counter_fire", c.matches_with_counter_fire),
    ] {
        raw.insert(k.into(), json!(v));
    }
    let ff = c.ff_kills[0] + c.ff_kills[1];
    let gun: u64 = c.gun_rounds.iter().sum();
    let cb: u64 = c.cb_rounds.iter().sum();
    let seen = c.gun_rounds[mode::DIRECT as usize]
        + c.gun_rounds[mode::OBSERVED as usize]
        + c.cb_rounds[mode::MAP as usize];
    let first_m = if c.cb_marks == 0 {
        Value::Null
    } else {
        json!(round3(
            c.cb_err_first_mm as f64 / c.cb_marks as f64 / 1000.0
        ))
    };
    let derived = json!({
        "ff_kills_per_match": per(ff),
        "ff_kills_mortar_per_match": per(c.ff_kills[0]),
        "ff_kills_gun_per_match": per(c.ff_kills[1]),
        "ff_loss_share_pct": pct_or_null(ff as f64, c.losses as f64),
        "ff_wounds_per_match": per(c.ff_wounds),
        "ff_lifts_per_match": per(c.get("ff_lifts")),
        "ff_holds_per_match": per(c.get("ff_holds")),
        "mortar_near_own_bursts_per_match": per(c.mortar_near_own_bursts),
        "cb_marks_per_match": per(c.cb_marks),
        "cb_first_error_m": first_m,
        "cb_fired_matches_pct": pct_or_null(c.matches_marker_fired as f64, c.matches_with_marker as f64),
        "cb_missions_per_match": per(c.cb_missions),
        "cb_rounds_per_match": per(cb),
        "cb_near_burst_pct": pct_or_null(c.cb_near_bursts as f64, c.cb_bursts as f64),
        "cb_masked_per_match": per(c.get("cb_masked")),
        "cb_wide_holds_per_match": per(c.get("cb_wide_holds")),
        "cb_stale_holds_per_match": per(c.get("cb_stale_holds")),
        "cb_shell_share_pct": pct_or_null(cb as f64, gun as f64),
        "gun_observed_or_cb_pct": pct_or_null(seen as f64, gun as f64),
    });
    (Value::Object(raw), derived)
}

/// S11b: a faction's `s11b` (raw: every `S11bCounters` field by name,
/// `how_rounds` by mode name; every S11b `AiCounters` field; the buy and
/// kill sums and the match tallies of spec 7) and `s11b_derived` (spec 7
/// names; null where the denominator is 0). `cb_alt_holds` counts planner
/// passes that held, not markers (review A-4): `cb_alt_hold_passes_per_match`
/// says so by name; `cb_fired_matches_any_pct` is the share of all
/// matches in which the faction's AI still ordered counter-fire.
/// The `s13` block of one faction (or one side of one match): the AI's
/// heavy-wave counters by name and, per `heavy_wave` card (short id), its
/// buys, the matches with a buy (count and share of the faction's matches,
/// T155) and the men (`kills`) and vehicles (`kos`) its vehicle put out
/// (T157). Every value is 0 when the AI buys no heavy-wave card
/// (`tools/neutral.py` reads a new block so).
pub fn s13_blocks(c: &crate::skirmish::S13Faction) -> Value {
    let mut raw = Map::new();
    for (k, v) in crate::skirmish::S13_AI.iter().zip(c.ai) {
        raw.insert((*k).into(), json!(v));
    }
    let pct = |n: u64| {
        if c.matches == 0 {
            0.0
        } else {
            round3(n as f64 * 100.0 / c.matches as f64)
        }
    };
    let mut cards = Map::new();
    for (k, &n) in &c.bought {
        let m = c.bought_matches.get(k).copied().unwrap_or(0);
        cards.insert(
            k.clone(),
            json!({ "bought": n, "matches": m, "matches_pct": pct(m) }),
        );
    }
    raw.insert("cards".into(), Value::Object(cards));
    let map = |m: &std::collections::BTreeMap<String, u64>| {
        Value::Object(m.iter().map(|(k, v)| (k.clone(), json!(v))).collect())
    };
    raw.insert("kills".into(), map(&c.kills));
    raw.insert("kos".into(), map(&c.kos));
    Value::Object(raw)
}

/// The `s14` block of one faction (or one side of one match): every
/// `S14Counters` field and the AI's flame counters by name. Every value is
/// 0 with the S14 switches off (`tools/neutral.py` reads a new block so).
pub fn s14_blocks(c: &crate::skirmish::S14Faction) -> Value {
    let mut raw = Map::new();
    for (k, v) in crate::skirmish::S14_COUNTERS.iter().zip(c.s) {
        raw.insert((*k).into(), json!(v));
    }
    for (k, v) in crate::skirmish::S14_AI.iter().zip(c.ai) {
        raw.insert((*k).into(), json!(v));
    }
    Value::Object(raw)
}

/// The `s12` block of one faction (or one side of one match): the
/// `S12Counters` and the AI's medic counters by name. Every value is 0
/// with the S12 switches off (`tools/neutral.py` reads a new block so).
pub fn s12_blocks(c: &crate::skirmish::S12Faction) -> Value {
    let mut raw = Map::new();
    for (k, v) in crate::skirmish::S12_COUNTERS.iter().zip(c.counters) {
        raw.insert((*k).into(), json!(v));
    }
    raw.insert(
        "downed_by".into(),
        Value::Object(
            kill_class::NAMES
                .iter()
                .zip(c.downed_by_class)
                .map(|(k, v)| ((*k).to_string(), json!(v)))
                .collect(),
        ),
    );
    raw.insert(
        "finished_by".into(),
        Value::Object(
            kill_class::NAMES
                .iter()
                .zip(c.finished_by_class)
                .map(|(k, v)| ((*k).to_string(), json!(v)))
                .collect(),
        ),
    );
    for (k, v) in [
        ("head_hits", c.head_hits),
        ("torso_hits", c.torso_hits),
        ("leg_hits", c.leg_hits),
    ] {
        raw.insert(k.into(), json!({ "trench": v[0], "open": v[1] }));
    }
    for (k, v) in crate::skirmish::S12_AI.iter().zip(c.ai) {
        raw.insert((*k).into(), json!(v));
    }
    Value::Object(raw)
}

/// S12 Build C (docs/slices/S12-wounds-medics.md 8): a faction's
/// `s12_derived`. Per match: `downed_per_match`, `treated_per_match`,
/// `evac_per_match`, `refund_mp_per_match`, `reinforced_men_per_match`,
/// `losses_per_match`, `length_min` (mean); shares of the faction's downs
/// (men of this faction): `outright_kill_pct` (killed outright over killed
/// outright plus downed), `bled_out_pct`, `stabilised_pct`,
/// `downed_killed_pct`; `head_hit_pct` (trench / open: head over all zone
/// hits on its men); `pop_downed_peak` (mean and most of the per-match
/// peaks of own downed men lying at once); `medic_matches_pct` (matches
/// in which a medic stabilised or a post treated an own man);
/// `team_matches_pct` (matches with a medic team bought);
/// `he_share_casualties_pct` (S12-15: HE casualties the faction
/// inflicted over all its casualties, one per man by the class that first
/// took him out, `he_cas_pct`). Null where a denominator is 0.
pub fn s12_derived(c: &crate::skirmish::S12Faction) -> Value {
    let n = c.matches.max(1) as f64;
    let per = |v: u64| round3(v as f64 / n);
    let k = |name: &str| {
        crate::skirmish::S12_COUNTERS
            .iter()
            .position(|x| *x == name)
            .map_or(0, |i| c.counters[i])
    };
    let downed = k("downed");
    let of_downs = |v: u64| pct_or_null(v as f64, downed as f64);
    let head = |z: usize| {
        let all = c.head_hits[z] + c.torso_hits[z] + c.leg_hits[z];
        pct_or_null(c.head_hits[z] as f64, all as f64)
    };
    json!({
        "matches": c.matches,
        "downed_per_match": per(downed),
        "outright_kill_pct": pct_or_null(
            k("killed_outright") as f64,
            (k("killed_outright") + downed) as f64
        ),
        "bled_out_pct": of_downs(k("bled_out")),
        "stabilised_pct": of_downs(k("stabilised")),
        "downed_killed_pct": of_downs(k("downed_killed")),
        "treated_per_match": per(k("treated")),
        "evac_per_match": per(k("evacuated")),
        "refund_mp_per_match": per(k("refund_mp")),
        "reinforced_men_per_match": per(k("reinforced_men")),
        "head_hit_pct": { "trench": head(0), "open": head(1) },
        "pop_downed_peak": {
            "mean": per(c.downed_peak_sum),
            "max": c.downed_peak_max,
        },
        "medic_matches_pct": pct_or_null(c.medic_matches as f64, c.matches as f64),
        "team_matches_pct": pct_or_null(c.team_matches as f64, c.matches as f64),
        "he_share_casualties_pct": crate::skirmish::he_cas_pct(&c.kills_by, &c.finished_by_class, &c.downed_by_class),
        "length_min": round3(c.seconds / 60.0 / n),
        "losses_per_match": per(c.losses),
    })
}

pub fn s11b_blocks(c: &crate::skirmish::S11bFaction) -> (Value, Value) {
    use frontline_sim::fire_mission::mode;
    let n = c.matches.max(1) as f64;
    let m = c.matches as f64;
    let per = |v: u64| round3(v as f64 / n);
    let mut raw = Map::new();
    raw.insert("matches".into(), json!(c.matches));
    raw.insert(
        "how_rounds".into(),
        Value::Object(
            mode::NAMES
                .iter()
                .zip(c.how_rounds.iter())
                .map(|(k, v)| ((*k).to_string(), json!(v)))
                .collect(),
        ),
    );
    for (k, v) in [
        ("arty_emplaced", c.arty_emplaced),
        ("arty_crew_lost", c.arty_crew_lost),
        ("rg_fired", c.rg_fired),
        ("how_bought", c.how_bought),
        ("light_bought", c.light_bought),
        ("mortar_bought", c.mortar_bought),
        ("light_bombs", c.light_bombs),
        ("how_kills", c.how_kills),
        ("rg_kills", c.rg_kills),
        ("kills", c.kills),
    ] {
        raw.insert(k.into(), json!(v));
    }
    for (k, name) in crate::skirmish::S11B_AI.iter().enumerate() {
        raw.insert((*name).into(), json!(c.ai[k]));
    }
    for (k, v) in [
        ("matches_with_how", c.matches_with_how),
        ("matches_with_how_fired", c.matches_with_how_fired),
        ("matches_how_window", c.matches_how_window),
        ("matches_with_light", c.matches_with_light),
        ("matches_counter_fired", c.matches_counter_fired),
    ] {
        raw.insert(k.into(), json!(v));
    }
    let how_rounds: u64 = c.how_rounds.iter().sum();
    let first = if c.how_first_n == 0 {
        Value::Null
    } else {
        json!(round3(c.how_first_s_sum as f64 / c.how_first_n as f64))
    };
    let derived = json!({
        "rg_fired_per_match": per(c.rg_fired),
        "rg_kill_share_pct": pct_or_null(c.rg_kills as f64, c.kills as f64),
        "how_window_matches_pct": pct_or_null(c.matches_how_window as f64, m),
        "how_bought_matches_pct": pct_or_null(c.matches_with_how as f64, m),
        "how_bought_window_pct": pct_or_null(c.matches_how_in_window as f64, c.matches_how_window as f64),
        "how_fired_matches_pct": pct_or_null(c.matches_with_how_fired as f64, c.matches_with_how as f64),
        "how_rounds_per_match": per(how_rounds),
        "how_kill_share_pct": pct_or_null(c.how_kills as f64, c.kills as f64),
        "how_first_on_map_s_mean": first,
        "how_first_on_map_s_min": c.how_first_s_min.map_or(Value::Null, |v| json!(v)),
        "how_pct_holds_per_match": per(c.get("how_pct_holds")),
        "how_mp_holds_per_match": per(c.get("how_mp_holds")),
        "light_bought_matches_pct": pct_or_null(c.matches_with_light as f64, m),
        "light_buy_share_pct": pct_or_null(c.light_bought as f64, c.mortar_bought as f64),
        "light_bombs_per_match": per(c.light_bombs),
        "cb_alt_holds_per_match": per(c.get("cb_alt_holds")),
        "cb_alt_hold_passes_per_match": per(c.get("cb_alt_holds")),
        "cb_fired_matches_any_pct": pct_or_null(c.matches_counter_fired as f64, m),
        "arty_emplaced_per_match": per(c.arty_emplaced),
        "arty_crew_lost_per_match": per(c.arty_crew_lost),
    });
    (Value::Object(raw), derived)
}

/// S10c-ai: a faction's `s10c` (raw: the S10c counters by name,
/// `front_why_<name>`, `mine_want_why_<name>`, the by-point arrays in map
/// point order, `mines_home`, `mines_front`, the exposure tallies) and
/// `s10c_derived` (spec 7). A share over an empty set is null, not 0.
pub fn s10c_blocks(c: &crate::skirmish::S10cFaction) -> (Value, Value) {
    let n = c.matches.max(1) as f64;
    let m = c.matches as f64;
    let per = |v: u64| round3(v as f64 / n);
    let mut raw = Map::new();
    for (k, name) in crate::skirmish::S10C_AI.iter().enumerate() {
        raw.insert((*name).into(), json!(c.ai[k]));
    }
    for (k, name) in frontline_sim::ai::FRONT_WHY.iter().enumerate() {
        raw.insert(format!("front_why_{name}"), json!(c.front_why[k]));
    }
    for (k, name) in frontline_sim::ai::MINE_WANT_WHY.iter().enumerate() {
        raw.insert(format!("mine_want_why_{name}"), json!(c.mine_want_why[k]));
    }
    raw.insert("works_by_point".into(), json!(c.works_by_point));
    raw.insert("mines_by_point".into(), json!(c.mines_by_point));
    for (k, v) in [
        ("mines_home", c.mines_home),
        ("mines_front", c.mines_front),
        ("mines_seen", c.mines_seen),
        ("mines_approached", c.mines_approached),
        ("mines_set_off", c.mines_set_off),
        ("s10b_jobs", c.s10b_jobs),
    ] {
        raw.insert(k.into(), json!(v));
    }
    for (k, v) in [
        ("matches_with_mines", c.matches_with_mines),
        ("matches_mines_approached", c.matches_mines_approached),
        ("matches_wire_seen", c.matches_wire_seen),
        ("matches_wire_approached", c.matches_wire_approached),
        ("matches_obstacle_approached", c.matches_obstacle_approached),
        ("matches_knew_enemy_wire", c.matches_knew_enemy_wire),
        ("matches_with_front_field", c.matches_with_front_field),
        ("matches_with_belt", c.matches_with_belt),
    ] {
        raw.insert(k.into(), json!(v));
    }
    let mines = (c.mines_home + c.mines_front) as f64;
    let derived = json!({
        "defence_job_fail_pct": pct_or_null(c.get("fort_job_fails") as f64, c.s10b_jobs as f64),
        "front_mines_share_pct": pct_or_null(c.mines_front as f64, mines),
        "matches_mines_approached_pct": pct_or_null(
            c.matches_mines_approached as f64,
            c.matches_with_mines as f64
        ),
        "mines_set_off_per_match": per(c.mines_set_off),
        "matches_with_wire": c.matches_with_wire,
        "wire_seen_by_enemy_pct": pct_or_null(c.matches_wire_seen as f64, c.matches_with_wire as f64),
        "matches_wire_approached_pct": pct_or_null(
            c.matches_wire_approached as f64,
            c.matches_with_wire as f64
        ),
        "matches_obstacle_approached_pct": pct_or_null(c.matches_obstacle_approached as f64, m),
        "front_calm_pct": pct_or_null(c.get("front_calm_plans") as f64, c.get("front_plans") as f64),
        "front_jobs_per_match": per(c.get("front_jobs")),
        "front_drop_pct": pct_or_null(c.get("front_drops") as f64, c.get("front_jobs") as f64),
        "front_builders_lost_per_match": per(c.get("front_lost")),
        "matches_with_belt_pct": pct_or_null(c.matches_with_belt as f64, m),
        "matches_with_front_field_pct": pct_or_null(c.matches_with_front_field as f64, m),
        "builder_buys_per_match": per(c.get("builder_buys")),
        "knew_enemy_wire_pct": pct_or_null(c.matches_knew_enemy_wire as f64, m),
    });
    (Value::Object(raw), derived)
}

#[cfg(test)]
mod tests {
    /// S14 Build B: a failing R2 is a hard check; R1 is one only when
    /// `hard` (report only now); rows not run add none.
    #[test]
    fn s14_rows_feed_the_hard_checks() {
        use crate::fortify::s14::S14Report;
        assert!(s14_checks(&S14Report::default()).is_empty());
        let mut r = S14Report::default();
        r.pool.pass = Some(false);
        r.slit.pass = Some(false);
        let c = s14_checks(&r);
        assert_eq!(c.len(), 1);
        assert_eq!(
            (c[0]["id"].as_str(), &c[0]["pass"]),
            (Some("s14_pool"), &json!(false))
        );
        r.slit.hard = true;
        let c = s14_checks(&r);
        assert_eq!(c.len(), 2);
        assert_eq!(c[1]["id"], json!("s14_slit"));
        // S14a Build C: R5 is hard.
        r.melee.pass = Some(false);
        let c = s14_checks(&r);
        assert_eq!(c.len(), 3);
        assert_eq!(
            (c[0]["id"].as_str(), &c[0]["pass"]),
            (Some("s14_melee"), &json!(false))
        );
    }

    use super::*;

    /// Review finding 8 (T113): `matches_with_bunker_pct` is over all the
    /// set's matches; the share of bunker jobs that ended in a bunker is
    /// `bunker_done_when_started_pct`; the exposure `bunker_ready_matches`
    /// counts matches whose plans got past the point checks. With the
    /// bunker off the share reads 0, not null. Failing control: the old
    /// share divided by the job matches (1 of 2 = 50, control null).
    #[test]
    fn bunker_share_is_over_all_matches() {
        use crate::skirmish::{CHANCE_BUNKER, S10Player, S10bAiFaction};
        let mut f = S10bAiFaction::default();
        let mut built = S10Player {
            s10b_chances: CHANCE_BUNKER,
            ..S10Player::default()
        };
        built.fort.bunkers_built = 1;
        built.bunker_why[15] = 1;
        let mut started = S10Player {
            s10b_chances: CHANCE_BUNKER,
            ..S10Player::default()
        };
        started.bunker_why[7] = 3;
        let mut waiting = S10Player::default();
        waiting.bunker_why[1] = 9;
        for p in [&built, &started, &waiting, &S10Player::default()] {
            f.add(p, 30);
        }
        let (_, d) = s10b_ai_blocks(&f);
        assert_eq!(d["matches_with_bunker_pct"], json!(25.0));
        assert_eq!(d["bunker_done_when_started_pct"], json!(50.0));
        assert_eq!(d["bunker_ready_matches"], json!(2));
        let mut off = S10bAiFaction::default();
        off.add(&S10Player::default(), 30);
        let (_, d) = s10b_ai_blocks(&off);
        assert_eq!(d["matches_with_bunker_pct"], json!(0.0), "control reads 0");
        assert_eq!(d["bunker_ready_matches"], json!(0));
    }

    #[test]
    fn slugs_and_ids() {
        assert_eq!(slug("3c: ≥ 40 % of hits"), "3c_40_of_hits");
        assert_eq!(
            ids(["a b", "a-b", "c"].into_iter()),
            vec!["a_b", "a_b_2", "c"]
        );
    }

    #[test]
    fn lookup_walks_objects_arrays_and_ids() {
        let v = json!({"a": {"b": [ {"id": "x", "v": 1}, {"name": "y z", "v": 2} ]}});
        assert_eq!(lookup(&v, "a.b.0.v"), Some(&json!(1)));
        assert_eq!(lookup(&v, "a.b.x.v"), Some(&json!(1)));
        assert_eq!(lookup(&v, "a.b.1.v"), Some(&json!(2)));
        assert_eq!(lookup(&v, "a.c"), None);
    }

    #[test]
    fn median_of_even_and_odd() {
        assert_eq!(median(&mut [3.0, 1.0, 2.0]), 2.0);
        assert_eq!(median(&mut [4.0, 1.0, 2.0, 3.0]), 2.5);
    }

    /// S14 step 8: the `s14` block names every `S14Counters` field and AI
    /// flame counter, each with its own value (a player's counters reach
    /// the right key), and reads all 0 with the switches off. Control: a
    /// counter set on one player shows only under its name.
    #[test]
    fn s14_block_names_every_counter() {
        use crate::skirmish::{S14_AI, S14_COUNTERS, S14Faction, S14Player};
        let off = s14_blocks(&S14Faction::default());
        let m = off.as_object().unwrap();
        assert_eq!(m.len(), S14_COUNTERS.len() + S14_AI.len());
        assert!(m.values().all(|v| v == &json!(0)), "{off}");
        let c = frontline_sim::flame::S14Counters {
            burn_kills: 3,
            clears_done: 5,
            ..Default::default()
        };
        let mut f = S14Faction::default();
        f.add(&S14Player::of(&c, None));
        f.add(&S14Player::of(&c, None));
        let on = s14_blocks(&f);
        assert_eq!(on["burn_kills"], json!(6));
        assert_eq!(on["clears_done"], json!(10));
        assert_eq!(on["clear_steps"], json!(0));
        let nonzero = on
            .as_object()
            .unwrap()
            .values()
            .filter(|v| *v != &json!(0))
            .count();
        assert_eq!(nonzero, 2);
    }
}
