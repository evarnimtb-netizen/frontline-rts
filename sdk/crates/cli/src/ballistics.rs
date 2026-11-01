//! `ballistics`: penetration of one round against one plate, a whole
//! table, or a golden file (§8.1, S3 "`frontline-cli ballistics`").
//!
//! Everything goes through the sim's own armour functions
//! (`frontline_sim::armour`), built from the mod's defs lowered exactly as
//! a match lowers them. Floats appear only to print angles and percentages
//! for people.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use clap::Args;
use frontline_content::vehicle::{AmmoSetup, VehicleSetup};
use frontline_content::{ArmourZone, ModDefs};
use frontline_sim::Fx;
use frontline_sim::armour::{
    self, AmmoTable, PPM, PenResult, PlateShot, VehicleArmour, aimed_descent, deg, plate_shot,
};
use serde::{Deserialize, Serialize};

use crate::skirmish::mods_dir;

/// Ranges and angles of `--table`.
pub const TABLE_RANGES_M: [i32; 7] = [100, 150, 300, 450, 600, 1000, 1500];
pub const TABLE_ANGLES_DEG: [i32; 4] = [0, 20, 30, 60];

#[derive(Args, Clone, Debug, Default)]
pub struct BallisticsArgs {
    /// Ammo id, e.g. `ww2:75mm_pzgr39` (the mod prefix may be left out).
    #[arg(long, value_name = "ID")]
    pub ammo: Option<String>,
    /// Target vehicle id, e.g. `ww2:usa_m4a3_75`.
    #[arg(long, value_name = "ID")]
    pub target: Option<String>,
    /// Armour zone: hull_front, hull_side, hull_rear, hull_top,
    /// turret_front, turret_side, turret_rear or turret_top.
    #[arg(long, value_name = "ZONE")]
    pub zone: Option<String>,
    /// Distance from the muzzle, metres.
    #[arg(long, value_name = "M", default_value_t = 300)]
    pub range: i32,
    /// Horizontal angle from the plate's normal, degrees.
    #[arg(
        long,
        value_name = "DEG",
        default_value_t = 0,
        allow_negative_numbers = true
    )]
    pub angle: i32,
    /// Descent below the horizontal: degrees, or `auto` for a round aimed
    /// at that range on the muzzle's level (no drag).
    #[arg(long, value_name = "0|auto|DEG", default_value = "0")]
    pub descent: String,
    /// Add the running gear's armour (hull sides' track band).
    #[arg(long)]
    pub track_band: bool,
    /// One ammo against every zone of the target, at 100-1500 m and 0°,
    /// 20°, 30° and 60°.
    #[arg(long, conflicts_with_all = ["golden", "zone"])]
    pub table: bool,
    /// Run the golden rows in a file (e.g. `mods/ww2/tests/ballistics.toml`);
    /// exits 1 if any row fails.
    #[arg(long, value_name = "FILE", conflicts_with_all = ["ammo", "target", "zone"])]
    pub golden: Option<PathBuf>,
    /// Print machine-readable JSON.
    #[arg(long)]
    pub json: bool,
    /// Mod directory (default: `mods/ww2` here or in the source tree).
    #[arg(long, value_name = "DIR")]
    pub mods: Option<PathBuf>,
}

/// The mod's armour, lowered and tabled as a match would.
pub struct Armoury {
    pub ns: String,
    pub ammo: Vec<AmmoSetup>,
    pub vehicles: Vec<VehicleSetup>,
    pub tables: frontline_sim::ArmourTables,
}

impl Armoury {
    pub fn new(defs: &ModDefs) -> Armoury {
        let ammo = defs.lower_ammo();
        // Tick rate only matters for durations, which ballistics never uses.
        let vehicles = defs.lower_vehicles(30);
        let tables = frontline_sim::ArmourTables::new(&ammo, &vehicles);
        Armoury {
            ns: defs
                .manifest
                .as_ref()
                .map(|m| m.id.clone())
                .unwrap_or_default(),
            ammo,
            vehicles,
            tables,
        }
    }

    pub fn load(mods: Option<&Path>) -> Result<Armoury> {
        let dir = mods_dir(mods)?;
        let defs = ModDefs::load(&dir).map_err(|e| anyhow!("{e}"))?;
        Ok(Armoury::new(&defs))
    }

    fn full_id(&self, id: &str) -> String {
        if id.contains(':') || self.ns.is_empty() {
            id.to_owned()
        } else {
            format!("{}:{id}", self.ns)
        }
    }

    pub fn ammo_index(&self, id: &str) -> Result<usize> {
        let full = self.full_id(id);
        self.ammo.iter().position(|a| a.id == full).ok_or_else(|| {
            anyhow!(
                "unknown ammo `{full}`; known: {}",
                self.ammo
                    .iter()
                    .map(|a| a.id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
    }

    pub fn vehicle_index(&self, id: &str) -> Result<usize> {
        let full = self.full_id(id);
        self.vehicles
            .iter()
            .position(|v| v.id == full)
            .ok_or_else(|| {
                anyhow!(
                    "unknown vehicle `{full}`; known: {}",
                    self.vehicles
                        .iter()
                        .map(|v| v.id.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
    }

    fn ammo_table(&self, i: usize) -> &AmmoTable {
        &self.tables.ammo[i]
    }

    fn armour(&self, i: usize) -> &VehicleArmour {
        &self.tables.vehicles[i]
    }
}

pub fn parse_zone(z: &str) -> Result<ArmourZone> {
    ArmourZone::from_name(z).ok_or_else(|| {
        anyhow!(
            "unknown zone `{z}`; zones: {}",
            ArmourZone::ALL.map(|z| z.name()).join(", ")
        )
    })
}

/// `--descent`: `auto`, or whole degrees.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Descent {
    Deg(i32),
    Auto(AutoWord),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AutoWord {
    Auto,
}

impl Default for Descent {
    fn default() -> Descent {
        Descent::Deg(0)
    }
}

impl Descent {
    pub fn parse(s: &str) -> Result<Descent> {
        if s == "auto" {
            return Ok(Descent::Auto(AutoWord::Auto));
        }
        s.parse::<i32>()
            .map(Descent::Deg)
            .map_err(|_| anyhow!("--descent takes whole degrees or `auto`, got `{s}`"))
    }

    fn radians(self, ammo: &AmmoSetup, range: Fx) -> Fx {
        match self {
            Descent::Deg(d) => deg(d),
            Descent::Auto(_) => aimed_descent(ammo.speed.into(), range),
        }
    }
}

/// One shot, fully described (the one-shot output and a golden row's
/// result).
#[derive(Clone, Debug, Serialize)]
pub struct ShotReport {
    pub ammo: String,
    pub ammo_name: String,
    pub mechanism: String,
    pub speed_m_s: f64,
    pub target: String,
    pub zone: String,
    pub range_m: i32,
    pub angle_deg: i32,
    pub descent_deg: f64,
    pub thickness_mm: f64,
    pub slope_deg: i32,
    pub track_mm: f64,
    /// Angle between the round's path and the plate normal.
    pub impact_deg: f64,
    pub impact_cos: f64,
    pub effective_mm: f64,
    /// Normalised penetration at this range, and its ±7 % band.
    pub pen0_mm: f64,
    pub pen_lo_mm: f64,
    pub pen_hi_mm: f64,
    pub ricochet_over_deg: i32,
    /// `penetrates`, `does not penetrate`, `ricochet` or `borderline`.
    pub result: String,
    pub pen_pct: f64,
}

#[allow(clippy::too_many_arguments)]
pub fn shot(
    a: &Armoury,
    ammo: usize,
    target: usize,
    zone: ArmourZone,
    range_m: i32,
    angle_deg: i32,
    descent: Descent,
    track_band: bool,
) -> ShotReport {
    let am = &a.ammo[ammo];
    let range = Fx::from_int(range_m);
    let desc = descent.radians(am, range);
    let s: PlateShot = plate_shot(
        a.ammo_table(ammo),
        a.armour(target),
        zone,
        range,
        deg(angle_deg),
        desc,
        track_band,
    );
    let f = |x: Fx| f64::from(x.to_f32());
    let c = f(s.c);
    let result = if s.ricochet {
        "ricochet"
    } else if s.at_low == PenResult::Penetration && s.chance_ppm == PPM {
        "penetrates"
    } else if s.at_high == PenResult::NonPenetration && s.chance_ppm == 0 {
        "does not penetrate"
    } else {
        "borderline"
    };
    let band = f(armour::PEN_BAND);
    ShotReport {
        ammo: am.id.clone(),
        ammo_name: am.name.clone(),
        mechanism: am.mechanism.name().to_owned(),
        speed_m_s: f64::from(Fx::from(am.speed).to_f32()),
        target: a.vehicles[target].id.clone(),
        zone: zone.name().to_owned(),
        range_m,
        angle_deg,
        descent_deg: f(desc).to_degrees(),
        thickness_mm: f(s.thickness),
        slope_deg: a.vehicles[target].armour[zone.index()].slope_deg,
        track_mm: f(s.extra),
        impact_deg: c.clamp(-1.0, 1.0).acos().to_degrees(),
        impact_cos: c,
        effective_mm: if c > 0.0 {
            f(s.effective)
        } else {
            f64::INFINITY
        },
        pen0_mm: f(s.pen0),
        pen_lo_mm: f(s.pen0) * (1.0 - band),
        pen_hi_mm: f(s.pen0) * (1.0 + band),
        ricochet_over_deg: am.ricochet_deg,
        result: result.to_owned(),
        pen_pct: f64::from(s.chance_ppm) / f64::from(PPM) * 100.0,
    }
}

pub fn shot_text(r: &ShotReport) -> String {
    let track = if r.track_mm > 0.0 {
        format!(" + {:.0} mm running gear", r.track_mm)
    } else {
        String::new()
    };
    let effective = if r.effective_mm.is_finite() {
        format!("{:.1} mm", r.effective_mm)
    } else {
        "-".to_owned()
    };
    let verdict = match r.result.as_str() {
        "borderline" => format!("borderline: {:.1} % penetrate", r.pen_pct),
        "penetrates" => "penetrates (100 %)".to_owned(),
        "does not penetrate" => "does not penetrate (0 %)".to_owned(),
        other => format!(
            "{other} (impact {:.1}° > {}°)",
            r.impact_deg, r.ricochet_over_deg
        ),
    };
    format!(
        "ammo     {} ({}, {}, {:.0} m/s)\n\
         target   {} {}: {} mm at {}°{track}\n\
         shot     {} m, horizontal {}°, descent {:.2}°\n\
         impact   {:.1}° from the normal (cos {:.3}), effective {effective}\n\
         pen      {:.1} mm at 0° (band {:.1} .. {:.1})\n\
         result   {verdict}",
        r.ammo,
        r.ammo_name,
        r.mechanism,
        r.speed_m_s,
        r.target,
        r.zone,
        r.thickness_mm,
        r.slope_deg,
        r.range_m,
        r.angle_deg,
        r.descent_deg,
        r.impact_deg,
        r.impact_cos,
        r.pen0_mm,
        r.pen_lo_mm,
        r.pen_hi_mm,
    )
}

// ---------------------------------------------------------------- table --

#[derive(Clone, Debug, Serialize)]
pub struct TableRow {
    pub zone: String,
    pub angle_deg: i32,
    pub effective_mm: Vec<f64>,
    /// Per range: penetration chance in percent, `None` for a ricochet.
    pub pen_pct: Vec<Option<f64>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TableReport {
    pub ammo: String,
    pub target: String,
    pub ranges_m: Vec<i32>,
    pub pen0_mm: Vec<f64>,
    pub rows: Vec<TableRow>,
}

pub fn table(
    a: &Armoury,
    ammo: usize,
    target: usize,
    descent: Descent,
    track_band: bool,
) -> TableReport {
    let pen0 = TABLE_RANGES_M
        .iter()
        .map(|&r| f64::from(a.ammo_table(ammo).pen0(Fx::from_int(r)).to_f32()))
        .collect();
    let mut rows = Vec::new();
    for zone in ArmourZone::ALL {
        for angle in TABLE_ANGLES_DEG {
            let shots: Vec<ShotReport> = TABLE_RANGES_M
                .iter()
                .map(|&r| shot(a, ammo, target, zone, r, angle, descent, track_band))
                .collect();
            rows.push(TableRow {
                zone: zone.name().to_owned(),
                angle_deg: angle,
                effective_mm: shots.iter().map(|s| s.effective_mm).collect(),
                pen_pct: shots
                    .iter()
                    .map(|s| (s.result != "ricochet").then_some(s.pen_pct))
                    .collect(),
            });
        }
    }
    TableReport {
        ammo: a.ammo[ammo].id.clone(),
        target: a.vehicles[target].id.clone(),
        ranges_m: TABLE_RANGES_M.to_vec(),
        pen0_mm: pen0,
        rows,
    }
}

pub fn table_text(t: &TableReport) -> String {
    let mut out = format!("{} vs {} (penetration %; R = ricochet)\n", t.ammo, t.target);
    out += &format!("{:<14}{:>5}{:>9}", "zone", "angle", "eff mm");
    for r in &t.ranges_m {
        out += &format!("{:>8}", format!("{r} m"));
    }
    out += &format!("\n{:<28}", "pen mm at 0°");
    for p in &t.pen0_mm {
        out += &format!("{p:>8.1}");
    }
    out.push('\n');
    for row in &t.rows {
        let eff = row.effective_mm[0];
        let eff = if eff.is_finite() {
            format!("{eff:.1}")
        } else {
            "-".into()
        };
        out += &format!("{:<14}{:>4}°{eff:>9}", row.zone, row.angle_deg);
        for p in &row.pen_pct {
            out += &match p {
                None => format!("{:>8}", "R"),
                Some(p) => format!("{p:>8.1}"),
            };
        }
        out.push('\n');
    }
    out
}

// --------------------------------------------------------------- golden --

/// `mods/<mod>/tests/ballistics.toml`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoldenFile {
    pub row: Vec<GoldenRow>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoldenRow {
    pub ammo: String,
    pub target: String,
    pub zone: String,
    pub range: i32,
    #[serde(default)]
    pub angle: i32,
    #[serde(default)]
    pub descent: Descent,
    #[serde(default)]
    pub track_band: bool,
    /// `pen`, `nonpen` or `ricochet` ...
    #[serde(default)]
    pub expect: Option<String>,
    /// ... or a penetration chance range in percent (inclusive).
    #[serde(default)]
    pub pen_pct: Option<[f64; 2]>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct GoldenResult {
    pub row: usize,
    pub pass: bool,
    pub want: String,
    pub got: String,
    pub shot: ShotReport,
}

#[derive(Clone, Debug, Serialize)]
pub struct GoldenReport {
    pub file: String,
    pub rows: usize,
    pub failed: usize,
    pub results: Vec<GoldenResult>,
}

/// Runs every row of a golden file through the sim's armour functions.
pub fn golden(a: &Armoury, path: &Path) -> Result<GoldenReport> {
    let src =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let file: GoldenFile = toml::from_str(&src)
        .map_err(|e| anyhow!("{}: {}", path.display(), e.to_string().trim_end()))?;
    let mut results = Vec::new();
    for (i, row) in file.row.iter().enumerate() {
        let n = i + 1;
        let ammo = a
            .ammo_index(&row.ammo)
            .with_context(|| format!("row {n}"))?;
        let target = a
            .vehicle_index(&row.target)
            .with_context(|| format!("row {n}"))?;
        let zone = parse_zone(&row.zone).with_context(|| format!("row {n}"))?;
        let s = shot(
            a,
            ammo,
            target,
            zone,
            row.range,
            row.angle,
            row.descent,
            row.track_band,
        );
        let got = match s.result.as_str() {
            "ricochet" => "ricochet".to_owned(),
            _ => format!("{} ({:.1} %)", s.result, s.pen_pct),
        };
        let (pass, want) = match (&row.expect, row.pen_pct) {
            (Some(e), None) => {
                let pass = match e.as_str() {
                    "pen" => s.result == "penetrates",
                    "nonpen" => s.result == "does not penetrate",
                    "ricochet" => s.result == "ricochet",
                    other => {
                        bail!("row {n}: expect must be pen, nonpen or ricochet, got `{other}`")
                    }
                };
                (pass, e.clone())
            }
            (None, Some([lo, hi])) => (
                s.result != "ricochet" && s.pen_pct >= lo && s.pen_pct <= hi,
                format!("pen_pct [{lo}, {hi}]"),
            ),
            _ => bail!("row {n}: give exactly one of `expect` and `pen_pct`"),
        };
        results.push(GoldenResult {
            row: n,
            pass,
            want,
            got,
            shot: s,
        });
    }
    Ok(GoldenReport {
        file: path.display().to_string(),
        rows: results.len(),
        failed: results.iter().filter(|r| !r.pass).count(),
        results,
    })
}

pub fn golden_text(g: &GoldenReport) -> String {
    let mut out = String::new();
    for r in &g.results {
        let s = &r.shot;
        out += &format!(
            "{} #{:<3} {} vs {} {} {} m {}°{}{}: want {}, got {}\n",
            if r.pass { "ok  " } else { "FAIL" },
            r.row,
            s.ammo,
            s.target,
            s.zone,
            s.range_m,
            s.angle_deg,
            if s.descent_deg != 0.0 {
                format!(" descent {:.2}°", s.descent_deg)
            } else {
                String::new()
            },
            if s.track_mm > 0.0 { " +track" } else { "" },
            r.want,
            r.got
        );
    }
    out += &format!("{}: {} rows, {} failed\n", g.file, g.rows, g.failed);
    out
}

/// What the subcommand prints, and whether it passed.
pub fn run(args: &BallisticsArgs) -> Result<(String, bool)> {
    let a = Armoury::load(args.mods.as_deref())?;
    let descent = Descent::parse(&args.descent)?;
    if let Some(path) = &args.golden {
        let g = golden(&a, path)?;
        let ok = g.failed == 0;
        let out = if args.json {
            serde_json::to_string_pretty(&g)?
        } else {
            golden_text(&g)
        };
        return Ok((out, ok));
    }
    let need = |v: &Option<String>, flag: &str| {
        v.clone()
            .ok_or_else(|| anyhow!("{flag} is required (see --help)"))
    };
    let ammo = a.ammo_index(&need(&args.ammo, "--ammo")?)?;
    let target = a.vehicle_index(&need(&args.target, "--target")?)?;
    if args.table {
        let t = table(&a, ammo, target, descent, args.track_band);
        let out = if args.json {
            serde_json::to_string_pretty(&t)?
        } else {
            table_text(&t)
        };
        return Ok((out, true));
    }
    let zone = parse_zone(&need(&args.zone, "--zone")?)?;
    let s = shot(
        &a,
        ammo,
        target,
        zone,
        args.range,
        args.angle,
        descent,
        args.track_band,
    );
    let out = if args.json {
        serde_json::to_string_pretty(&s)?
    } else {
        shot_text(&s)
    };
    Ok((out, true))
}
