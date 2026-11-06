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
                "kills_by_vehicle_pct": by_vehicle,
                "vehicle_kills_pct": pct(all as f64, kills),
                "kills_by_vehicle_max_pct": max_pct,
                "kos_per_bought": kos_per,
            }),
        );
    }
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

/// The fortify-test document: hard checks (rifles, mortar, smoke) and the
/// report rows (ranged mortar, grenades, MG suppression).
pub fn fortify(r: &FortifyReport) -> Value {
    let checks = vec![
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
    ];
    json!({
        "schema": 1,
        "kind": "fortify-test",
        "pass": r.pass,
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
