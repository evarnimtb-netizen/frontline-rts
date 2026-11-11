//! S10b fortify-test rows (docs/slices/S10b-obstacles.md 7): AT and AP
//! mines, sweeping, obstacles, demolition and the bunker. Report rows; the
//! targets T102-T107 read them (`fortify10.s10b.*`), `pass` ignores them.
//!
//! Build A lays out every field as `null`; the stage that gives a mechanic
//! its behaviour fills its row (C: `atmine`, `apmine`, `sweep`; D:
//! `obstacle`, `demolish`; E: `bunker`).

use serde::Serialize;

/// T102: a tank driven over one AT mine, and through an AT field.
#[derive(Clone, Debug, Default, Serialize)]
pub struct AtMineRow {
    pub direct_immobilised_pct: Option<f64>,
    pub field_immobilised_pct: Option<f64>,
    pub field_knocked_out_pct: Option<f64>,
    pub infantry_triggered: Option<f64>,
}

/// T103: AP mines go off on contact only.
#[derive(Clone, Debug, Default, Serialize)]
pub struct ApMineRow {
    pub near_miss_triggered: Option<f64>,
    pub contact_triggered_pct: Option<f64>,
    pub vehicle_triggered: Option<f64>,
    pub crossing_losses_pct: Option<f64>,
}

/// T104: a sweep against the expected find rate.
#[derive(Clone, Debug, Default, Serialize)]
pub struct SweepRow {
    pub found_vs_expected_pts: Option<f64>,
    pub lifted_of_found_pct: Option<f64>,
    pub found_pct: Option<f64>,
    pub sweeper_losses: Option<f64>,
    pub bangalore_cleared_pct: Option<f64>,
}

/// T105: obstacles block vehicles, not infantry.
#[derive(Clone, Debug, Default, Serialize)]
pub struct ObstacleRow {
    pub hedgehog_crossed_pct: Option<f64>,
    pub log_crossed_pct: Option<f64>,
    pub infantry_speed_pct: Option<f64>,
    pub tank_reached_pct: Option<f64>,
}

/// T106: demolition opens an obstacle line.
#[derive(Clone, Debug, Default, Serialize)]
pub struct DemolishRow {
    pub destroyed_pct: Option<f64>,
    pub gap_crossed_pct: Option<f64>,
    pub charge_laid_s: Option<f64>,
}

/// T107h: the open-pit control of the bunker's MG row.
#[derive(Clone, Debug, Default, Serialize)]
pub struct BunkerControl {
    pub mg_losses_pct: Option<f64>,
}

/// T107: the bunker against named weapons.
#[derive(Clone, Debug, Default, Serialize)]
pub struct BunkerRow {
    pub small_arms_hp_lost: Option<f64>,
    pub survived_81mm_x3_pct: Option<f64>,
    pub sheltered_losses: Option<f64>,
    pub slit_satchel_losses_pct: Option<f64>,
    pub roof_satchel_losses: Option<f64>,
    pub ap_shots_to_break: Option<f64>,
    pub mg_losses_pct: Option<f64>,
    pub control: BunkerControl,
}

/// The six S10b rows (`fortify10.s10b`).
#[derive(Clone, Debug, Default, Serialize)]
pub struct S10bReport {
    pub atmine: AtMineRow,
    pub apmine: ApMineRow,
    pub sweep: SweepRow,
    pub obstacle: ObstacleRow,
    pub demolish: DemolishRow,
    pub bunker: BunkerRow,
}

/// One row's fields as `name value` pairs (`-` for a row not yet run).
fn fields(v: &serde_json::Value) -> String {
    let Some(m) = v.as_object() else {
        return String::new();
    };
    m.iter()
        .map(|(k, x)| match x {
            serde_json::Value::Object(_) => format!("{k} ({})", fields(x)),
            serde_json::Value::Number(n) => format!("{k} {n}"),
            _ => format!("{k} -"),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Summary lines, one per row.
pub fn summary_text(r: &S10bReport) -> String {
    let v = serde_json::to_value(r).unwrap_or_default();
    let mut out = String::new();
    for row in [
        "atmine", "apmine", "sweep", "obstacle", "demolish", "bunker",
    ] {
        out += &format!("  S10b {row}: {}\n", fields(&v[row]));
    }
    out
}
