//! `targets FILE.toml`: check a slice's numeric targets against the runs
//! that feed them, and print one line per target.
//!
//! ```toml
//! [runs.skirmish40]            # which runs feed the targets
//! kind = "skirmish"            # skirmish | armour-test | fortify-test
//! seeds = 20
//! both_bases = true
//! minutes = 45
//!
//! [[target]]
//! id = "T5"
//! metric = "skirmish40.wins.germany_pct"   # <run>.<path into that run's JSON>
//! min = 35
//! max = 65
//! kind = "hard"                # hard (default) | report
//! ```
//!
//! Output: `T5 skirmish40.wins.germany_pct 47.8 [35,65] PASS`, then
//! `N pass, M miss (hard: K)`. Exit code 1 when a hard target misses.
//!
//! Row honesty (S10b-ai spec 7): a target may name `exposure` (a metric
//! path), `exposure_min` and `control` (the same measure with the thing
//! off). A file key `three_parts_from = "T109"` makes all three required
//! from that id on (the id's number compared: T109 > T99). A hard row whose
//! exposure is under `exposure_min` is `UNEXPOSED` (fails the run, never a
//! hard miss); a hard row whose control also lies inside its bounds is
//! `CONTROL-SAME` (fails). Report rows print exposure and control only.
//! A control or exposure key the played run lacks prints `CONTROL-MISSING`
//! / `EXPOSURE-MISSING` and fails, report rows too (a JSON null does not).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;
use serde_json::Value;

use crate::json_out;
use crate::skirmish::SkirmishArgs;
use crate::{ArmourArgs, FortifyArgs};

/// Options of the `targets` subcommand.
#[derive(Clone, Debug, clap::Args)]
pub struct TargetsArgs {
    /// The targets file (see docs/modding/tuning.md).
    #[arg(value_name = "FILE.toml")]
    pub file: PathBuf,
    /// Read each run's saved `<run>.json` from this folder instead of
    /// playing it. A run with no file there is skipped.
    #[arg(long, value_name = "DIR")]
    pub from_json: Option<PathBuf>,
    /// Matches run at once for skirmish runs (default 6, the balance-batch
    /// setting; tick-time targets need a clean run with `jobs = 3` in the
    /// run's own table).
    #[arg(long, value_name = "J")]
    pub jobs: Option<usize>,
    /// Only these runs (comma-separated names); targets of other runs print
    /// SKIP.
    #[arg(long, value_name = "RUN", value_delimiter = ',')]
    pub runs: Vec<String>,
    /// Where the played runs' JSON is saved (default
    /// `target/targets/<file name>`).
    #[arg(long, value_name = "DIR")]
    pub out: Option<PathBuf>,
    /// Mod directory (default: `mods/ww2`).
    #[arg(long, value_name = "DIR")]
    pub mods: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetsFile {
    /// Free text; not used.
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub runs: BTreeMap<String, RunSpec>,
    #[serde(default, rename = "target")]
    pub targets: Vec<TargetSpec>,
    /// S10b-ai: from this id on (by number) every target needs `exposure`,
    /// `exposure_min` and `control`.
    #[serde(default)]
    pub three_parts_from: Option<String>,
}

/// The number in a target id (`T109a` -> 109); `None` with no digits.
pub fn id_number(id: &str) -> Option<u32> {
    let d: String = id
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit())
        .collect();
    d.parse().ok()
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunSpec {
    /// `skirmish`, `armour-test` or `fortify-test`.
    pub kind: String,
    /// Skirmish: seeds per base order. Armour and fortify: `--seeds`.
    pub seeds: u64,
    #[serde(default = "one")]
    pub first_seed: u64,
    /// Skirmish only: play every seed with both base orders.
    #[serde(default)]
    pub both_bases: bool,
    /// Skirmish only: the `--minutes` limit.
    #[serde(default)]
    pub minutes: Option<u32>,
    /// Skirmish only: matches at once, overriding the command line.
    #[serde(default)]
    pub jobs: Option<usize>,
    /// Armour-test only: `--stress`.
    #[serde(default)]
    pub stress: bool,
    /// `--rules-override` files, relative to the working folder.
    #[serde(default)]
    pub rules_override: Vec<PathBuf>,
    /// Skirmish only (S15-ai): `--faction0` / `--faction1` (absent: the
    /// CLI's defaults, Germany against the USA).
    #[serde(default)]
    pub faction0: Option<String>,
    #[serde(default)]
    pub faction1: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

fn one() -> u64 {
    1
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetSpec {
    pub id: String,
    /// `<run>.<path into the run's JSON>`; see [`json_out::lookup`].
    pub metric: String,
    /// Inclusive bounds; either may be left out.
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    /// `hard` (default) or `report`.
    #[serde(default = "hard")]
    pub kind: String,
    #[serde(default)]
    pub note: Option<String>,
    /// Row honesty: how often the thing really happened (a metric path)
    /// and the least that judges the row.
    #[serde(default)]
    pub exposure: Option<String>,
    #[serde(default)]
    pub exposure_min: Option<f64>,
    /// The same measure with the thing off (a metric path).
    #[serde(default)]
    pub control: Option<String>,
}

fn hard() -> String {
    "hard".into()
}

/// How one target came out.
#[derive(Clone, Debug, PartialEq)]
pub enum Verdict {
    Pass,
    Miss,
    /// The run was not played or read.
    Skip,
    /// The run exists but the metric path does not, or is not a number.
    NoMetric,
    /// Exposure under `exposure_min`: the run fails, never a hard miss.
    Unexposed,
    /// A hard row whose control also lies inside its bounds: fails.
    ControlSame,
    /// A played run without the control or exposure key (a typo, or a
    /// number the run does not write): fails, report rows too (review
    /// finding 10; a JSON null is not missing).
    KeyMissing,
}

/// One printed line's data.
#[derive(Clone, Debug)]
pub struct Line {
    pub text: String,
    pub verdict: Verdict,
    pub hard: bool,
}

/// Parses and checks a targets file.
pub fn parse(text: &str) -> Result<TargetsFile> {
    let f: TargetsFile = toml::from_str(text).map_err(|e| anyhow!("{e}"))?;
    for (name, r) in &f.runs {
        if name.contains('.') || name.is_empty() {
            bail!("run name {name:?} must be non-empty and have no '.'");
        }
        if !matches!(r.kind.as_str(), "skirmish" | "armour-test" | "fortify-test") {
            bail!(
                "run {name}: kind {:?} is not skirmish, armour-test or fortify-test",
                r.kind
            );
        }
    }
    let mut seen = BTreeSet::new();
    for t in &f.targets {
        if !seen.insert(t.id.clone()) {
            bail!("target id {} appears twice", t.id);
        }
        if !matches!(t.kind.as_str(), "hard" | "report") {
            bail!("target {}: kind {:?} is not hard or report", t.id, t.kind);
        }
        if t.min.is_none() && t.max.is_none() {
            bail!("target {}: give min, max or both", t.id);
        }
        for (what, m) in [
            ("metric", Some(&t.metric)),
            ("exposure", t.exposure.as_ref()),
            ("control", t.control.as_ref()),
        ] {
            let Some(m) = m else { continue };
            let run = m.split('.').next().unwrap_or("");
            if !f.runs.contains_key(run) {
                bail!(
                    "target {}: {what} {m:?} starts with {run:?}, which is not a [runs.*] table",
                    t.id
                );
            }
        }
        if t.exposure.is_some() != t.exposure_min.is_some() {
            bail!("target {}: give exposure and exposure_min together", t.id);
        }
    }
    if let Some(from) = &f.three_parts_from {
        let Some(n0) = id_number(from) else {
            bail!("three_parts_from {from:?} has no number");
        };
        for t in &f.targets {
            if id_number(&t.id).is_some_and(|n| n >= n0)
                && (t.exposure.is_none() || t.exposure_min.is_none() || t.control.is_none())
            {
                bail!(
                    "target {}: from {from} on every target needs exposure, exposure_min and control",
                    t.id
                );
            }
        }
    }
    Ok(f)
}

fn num(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.3}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Checks one target against the documents of the runs that exist.
pub fn evaluate(t: &TargetSpec, docs: &BTreeMap<String, Value>) -> Line {
    let hard = t.kind == "hard";
    let bounds = format!(
        "[{},{}]",
        t.min.map_or("-".into(), num),
        t.max.map_or("-".into(), num)
    );
    let (run, path) = t.metric.split_once('.').unwrap_or((&t.metric, ""));
    let head = format!("{} {}", t.id, t.metric);
    let Some(doc) = docs.get(run) else {
        return Line {
            text: format!("{head} - {bounds} SKIP (run {run} not played)"),
            verdict: Verdict::Skip,
            hard,
        };
    };
    // Fix F10 (S10a-ai): a share over an empty set is null: skipped.
    if json_out::lookup(doc, path).is_some_and(Value::is_null) {
        return Line {
            text: format!("{head} - {bounds} SKIP (no matches to measure)"),
            verdict: Verdict::Skip,
            hard,
        };
    }
    let value = json_out::lookup(doc, path).and_then(|v| match v {
        Value::Number(n) => n.as_f64(),
        Value::Bool(b) => Some(f64::from(u8::from(*b))),
        _ => None,
    });
    let Some(x) = value else {
        return Line {
            text: format!("{head} - {bounds} MISS (no such number)"),
            verdict: Verdict::NoMetric,
            hard,
        };
    };
    let inside = |x: f64| t.min.is_none_or(|m| x >= m) && t.max.is_none_or(|m| x <= m);
    let ok = inside(x);
    // Row honesty: exposure and control beside the value.
    // None: run not played; Some(Err): the key names no number (fix 10);
    // Some(Ok(None)): a JSON null.
    let read = |m: &str| -> Option<Result<Option<f64>, ()>> {
        let (run, path) = m.split_once('.').unwrap_or((m, ""));
        let doc = docs.get(run)?;
        Some(match json_out::lookup(doc, path) {
            Some(Value::Null) => Ok(None),
            Some(v) => v.as_f64().map(Some).ok_or(()),
            None => Err(()),
        })
    };
    let mut extra = String::new();
    let mut unexposed = false;
    let mut missing = None;
    if let (Some(e), Some(min)) = (&t.exposure, t.exposure_min) {
        match read(e) {
            None => extra += &format!("; exposure {e} - (not played)"),
            Some(Err(())) => {
                missing = Some("EXPOSURE-MISSING");
                extra += &format!("; exposure {e} no such number");
            }
            Some(Ok(v)) => {
                unexposed = v.is_none_or(|v| v < min);
                extra += &format!(
                    "; exposure {} (min {})",
                    v.map_or("-".into(), num),
                    num(min)
                );
            }
        }
    }
    let mut same = false;
    if let Some(c) = &t.control {
        match read(c) {
            None => extra += &format!("; control {c} - (not played)"),
            Some(Err(())) => {
                missing = Some("CONTROL-MISSING");
                extra += &format!("; control {c} no such number");
            }
            Some(Ok(v)) => {
                same = v.is_some_and(inside);
                extra += &format!("; control {}", v.map_or("null".into(), num));
            }
        }
    }
    let (tag, verdict) = match (ok, hard) {
        _ if missing.is_some() => (missing.unwrap_or_default(), Verdict::KeyMissing),
        _ if hard && unexposed => ("UNEXPOSED", Verdict::Unexposed),
        (true, true) if same => ("CONTROL-SAME", Verdict::ControlSame),
        (true, _) => ("PASS", Verdict::Pass),
        (false, true) => ("MISS", Verdict::Miss),
        (false, false) => ("MISS (report)", Verdict::Miss),
    };
    Line {
        text: format!("{head} {} {bounds} {tag}{extra}", num(x)),
        verdict,
        hard,
    }
}

/// The last line: `N pass, M miss (hard: K)`, plus the unexposed,
/// control-same and skipped counts when there are any. Returns it with the
/// count of failing lines (hard misses, unexposed, control-same).
pub fn tally(lines: &[Line]) -> (String, usize) {
    let pass = lines.iter().filter(|l| l.verdict == Verdict::Pass).count();
    let miss: Vec<&Line> = lines
        .iter()
        .filter(|l| matches!(l.verdict, Verdict::Miss | Verdict::NoMetric))
        .collect();
    let hard = miss.iter().filter(|l| l.hard).count();
    let skip = lines.iter().filter(|l| l.verdict == Verdict::Skip).count();
    let unexposed = lines
        .iter()
        .filter(|l| l.verdict == Verdict::Unexposed)
        .count();
    let same = lines
        .iter()
        .filter(|l| l.verdict == Verdict::ControlSame)
        .count();
    let mut s = format!("{pass} pass, {} miss (hard: {hard})", miss.len());
    if unexposed > 0 {
        s += &format!(", {unexposed} unexposed");
    }
    if same > 0 {
        s += &format!(", {same} control-same");
    }
    let missing = lines
        .iter()
        .filter(|l| l.verdict == Verdict::KeyMissing)
        .count();
    if missing > 0 {
        s += &format!(", {missing} key-missing");
    }
    if skip > 0 {
        s += &format!(", {skip} skipped");
    }
    (s, hard + unexposed + same + missing)
}

fn play(name: &str, r: &RunSpec, args: &TargetsArgs) -> Result<Value> {
    eprintln!("targets: running {name} ({})", r.kind);
    Ok(match r.kind.as_str() {
        "skirmish" => {
            let a = SkirmishArgs {
                seed: r.first_seed,
                seeds: Some(r.seeds),
                both_bases: r.both_bases,
                minutes: r.minutes.unwrap_or(30),
                jobs: Some(r.jobs.or(args.jobs).unwrap_or(6)),
                mods: args.mods.clone(),
                rules_override: r.rules_override.clone(),
                summary: true,
                faction0: r
                    .faction0
                    .clone()
                    .unwrap_or_else(|| SkirmishArgs::default().faction0),
                faction1: r
                    .faction1
                    .clone()
                    .unwrap_or_else(|| SkirmishArgs::default().faction1),
                ..SkirmishArgs::default()
            };
            json_out::skirmish(&crate::skirmish::skirmish_many(&a)?)
        }
        "armour-test" => {
            let a = ArmourArgs {
                seeds: r.seeds as u32,
                first_seed: r.first_seed,
                stress: r.stress,
                mods: args.mods.clone(),
                rules_override: r.rules_override.clone(),
                ..ArmourArgs::default()
            };
            json_out::armour(&crate::armour_test::armour_test(&a)?)
        }
        _ => {
            let a = FortifyArgs {
                seeds: r.seeds as u32,
                first_seed: r.first_seed,
                mods: args.mods.clone(),
                rules_override: r.rules_override.clone(),
                ..FortifyArgs::default()
            };
            json_out::fortify(&crate::fortify::fortify_test(&a)?)
        }
    })
}

fn read_saved(dir: &Path, name: &str) -> Result<Option<Value>> {
    let p = dir.join(format!("{name}.json"));
    if !p.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&p).with_context(|| format!("reading {}", p.display()))?;
    Ok(Some(
        serde_json::from_str(&text).with_context(|| format!("parsing {}", p.display()))?,
    ))
}

/// Runs the command. Returns the lines to print and whether a hard target
/// missed.
pub fn run(args: &TargetsArgs) -> Result<(Vec<String>, bool)> {
    let text = std::fs::read_to_string(&args.file)
        .with_context(|| format!("reading {}", args.file.display()))?;
    let file = parse(&text).with_context(|| format!("{}", args.file.display()))?;
    for r in &args.runs {
        if !file.runs.contains_key(r) {
            bail!("--runs {r}: no such [runs.*] table in the file");
        }
    }
    let wanted: BTreeSet<&str> = file
        .targets
        .iter()
        .flat_map(|t| [Some(&t.metric), t.exposure.as_ref(), t.control.as_ref()])
        .flatten()
        .filter_map(|m| m.split('.').next())
        .filter(|r| args.runs.is_empty() || args.runs.iter().any(|x| x == r))
        .collect();
    let stem = args
        .file
        .file_stem()
        .map_or("targets".into(), |s| s.to_string_lossy().into_owned());
    let out = args
        .out
        .clone()
        .unwrap_or_else(|| Path::new("target/targets").join(stem));
    let mut docs = BTreeMap::new();
    for name in wanted {
        let doc = match &args.from_json {
            Some(dir) => read_saved(dir, name)?,
            None => {
                let v = play(name, &file.runs[name], args)?;
                json_out::write(&out.join(format!("{name}.json")), &v)?;
                Some(v)
            }
        };
        if let Some(v) = doc {
            docs.insert(name.to_string(), v);
        }
    }
    let lines: Vec<Line> = file.targets.iter().map(|t| evaluate(t, &docs)).collect();
    let (last, hard) = tally(&lines);
    let mut out: Vec<String> = lines.into_iter().map(|l| l.text).collect();
    out.push(last);
    Ok((out, hard > 0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const FILE: &str = r#"
        [runs.s40]
        kind = "skirmish"
        seeds = 20
        both_bases = true
        [runs.arm]
        kind = "armour-test"
        seeds = 5

        [[target]]
        id = "T1"
        metric = "s40.wins.germany_pct"
        min = 35
        max = 65
        [[target]]
        id = "T2"
        metric = "s40.length_min.median"
        min = 15
        max = 30
        kind = "report"
        [[target]]
        id = "T3"
        metric = "arm.pass"
        min = 1
        [[target]]
        id = "T4"
        metric = "arm.cases.case9.checks.x.value"
        max = 40
    "#;

    fn docs() -> BTreeMap<String, Value> {
        let mut d = BTreeMap::new();
        d.insert(
            "s40".into(),
            json!({"wins": {"germany_pct": 47.8}, "length_min": {"median": 12.5}}),
        );
        d
    }

    #[test]
    fn lines_pass_miss_report_and_skip() {
        let f = parse(FILE).unwrap();
        let d = docs();
        let l: Vec<Line> = f.targets.iter().map(|t| evaluate(t, &d)).collect();
        assert_eq!(l[0].text, "T1 s40.wins.germany_pct 47.8 [35,65] PASS");
        assert_eq!(
            l[1].text,
            "T2 s40.length_min.median 12.5 [15,30] MISS (report)"
        );
        assert!(
            l[2].text.ends_with("SKIP (run arm not played)"),
            "{}",
            l[2].text
        );
        assert_eq!(l[2].text.split(' ').nth(2), Some("-"));
        let (last, hard) = tally(&l);
        assert_eq!(last, "1 pass, 1 miss (hard: 0), 2 skipped");
        assert_eq!(hard, 0);
    }

    #[test]
    fn a_hard_miss_and_a_missing_path_count_as_hard() {
        let f = parse(FILE).unwrap();
        let mut d = docs();
        d.insert(
            "arm".into(),
            json!({"pass": true, "cases": [{"id": "case9", "checks": [{"id": "x", "value": 46.0}]}]}),
        );
        let l: Vec<Line> = f.targets.iter().map(|t| evaluate(t, &d)).collect();
        assert_eq!(l[2].text, "T3 arm.pass 1 [1,-] PASS");
        assert_eq!(
            l[3].text,
            "T4 arm.cases.case9.checks.x.value 46 [-,40] MISS"
        );
        d.get_mut("s40").unwrap()["wins"] = json!({});
        let l: Vec<Line> = f.targets.iter().map(|t| evaluate(t, &d)).collect();
        assert!(l[0].text.ends_with("MISS (no such number)"));
        let (last, hard) = tally(&l);
        assert_eq!(last, "1 pass, 3 miss (hard: 2)");
        assert_eq!(hard, 2);
    }

    #[test]
    fn a_null_share_is_skipped_not_missed() {
        // S10a-ai fix F10: a share over an empty set is null.
        let f = parse(FILE).unwrap();
        let mut d = docs();
        d.insert(
            "arm".into(),
            json!({"pass": true, "cases": [{"id": "case9", "checks": [{"id": "x", "value": null}]}]}),
        );
        let l = evaluate(&f.targets[3], &d);
        assert!(
            l.text.ends_with("SKIP (no matches to measure)"),
            "{}",
            l.text
        );
        assert_eq!(l.verdict, Verdict::Skip);
    }

    /// S10b-ai rows in three parts (value, exposure, control).
    const THREE: &str = r#"
        three_parts_from = "T109"
        [runs.on]
        kind = "skirmish"
        seeds = 80
        [runs.off]
        kind = "skirmish"
        seeds = 20
        [[target]]
        id = "T109a"
        metric = "on.mines_pct"
        min = 50
        exposure = "on.chances"
        exposure_min = 40
        control = "off.mines_pct"
        [[target]]
        id = "T109b"
        metric = "on.mines_pct"
        min = 0
        kind = "report"
        exposure = "on.chances"
        exposure_min = 40
        control = "off.mines_pct"
    "#;

    fn three_docs(chances: u32, off: f64) -> BTreeMap<String, Value> {
        let mut d = BTreeMap::new();
        d.insert("on".into(), json!({"mines_pct": 70.0, "chances": chances}));
        d.insert("off".into(), json!({"mines_pct": off}));
        d
    }

    /// Control: the old evaluate path (no exposure read) passes the row at
    /// 70 % although only 3 matches had a chance, and the run would not fail.
    #[test]
    fn unexposed_row_fails_not_hard() {
        let f = parse(THREE).unwrap();
        let d = three_docs(3, 0.0);
        let l: Vec<Line> = f.targets.iter().map(|t| evaluate(t, &d)).collect();
        assert_eq!(l[0].verdict, Verdict::Unexposed, "{}", l[0].text);
        assert_eq!(
            l[0].text,
            "T109a on.mines_pct 70 [50,-] UNEXPOSED; exposure 3 (min 40); control 0"
        );
        // A report row only prints its parts.
        assert_eq!(l[1].verdict, Verdict::Pass, "{}", l[1].text);
        let (last, fails) = tally(&l);
        assert_eq!(last, "1 pass, 0 miss (hard: 0), 1 unexposed");
        assert_eq!(fails, 1);
        // Exposed: the row passes.
        let d = three_docs(60, 0.0);
        let l = evaluate(&f.targets[0], &d);
        assert_eq!(l.verdict, Verdict::Pass, "{}", l.text);
        // A null control (no want with the thing off) is not "same".
        let mut d = three_docs(60, 0.0);
        d.insert("off".into(), json!({"mines_pct": null}));
        assert_eq!(evaluate(&f.targets[0], &d).verdict, Verdict::Pass);
    }

    /// Control: the old evaluate path passes a row whose off-run reads the
    /// same (the row cannot tell on from off).
    #[test]
    fn control_inside_bounds_fails() {
        let f = parse(THREE).unwrap();
        let d = three_docs(60, 55.0);
        let l: Vec<Line> = f.targets.iter().map(|t| evaluate(t, &d)).collect();
        assert_eq!(l[0].verdict, Verdict::ControlSame, "{}", l[0].text);
        assert!(
            l[0].text
                .contains("CONTROL-SAME; exposure 60 (min 40); control 55"),
            "{}",
            l[0].text
        );
        // Report rows get no verdict from their control.
        assert_eq!(l[1].verdict, Verdict::Pass, "{}", l[1].text);
        let (last, fails) = tally(&l);
        assert_eq!(last, "1 pass, 0 miss (hard: 0), 1 control-same");
        assert_eq!(fails, 1);
    }

    /// Review finding 10: a control or exposure key that names no number in
    /// a played run is an error line (CONTROL-MISSING / EXPOSURE-MISSING)
    /// that fails the run, report rows too. A JSON null stays "null".
    /// Failing control: the old lookup printed "control null" and passed.
    #[test]
    fn a_missing_control_or_exposure_key_fails() {
        let f = parse(THREE).unwrap();
        let mut d = three_docs(60, 0.0);
        d.insert("off".into(), json!({"mines_pc": 0.0}));
        let l: Vec<Line> = f.targets.iter().map(|t| evaluate(t, &d)).collect();
        for x in &l {
            assert_eq!(x.verdict, Verdict::KeyMissing, "{}", x.text);
            assert!(x.text.contains("CONTROL-MISSING"), "{}", x.text);
        }
        let (last, fails) = tally(&l);
        assert_eq!(last, "0 pass, 0 miss (hard: 0), 2 key-missing");
        assert_eq!(fails, 2);
        let mut d = three_docs(60, 0.0);
        d.insert("on".into(), json!({"mines_pct": 70.0}));
        let l = evaluate(&f.targets[0], &d);
        assert_eq!(l.verdict, Verdict::KeyMissing, "{}", l.text);
        assert!(l.text.contains("EXPOSURE-MISSING"), "{}", l.text);
    }

    /// Control: the old parse accepts a T109+ row without its parts.
    #[test]
    fn three_parts_required_from_id() {
        assert!(parse(THREE).is_ok());
        assert_eq!(id_number("T109a"), Some(109));
        assert_eq!(id_number("T99"), Some(99));
        // T110 without control: refused; T99 without parts: accepted.
        let no_control = THREE.replace(
            "        control = \"off.mines_pct\"\n        [[target]]",
            "        [[target]]",
        );
        assert!(parse(&no_control).is_err());
        let early =
            format!("{THREE}\n[[target]]\nid = \"T99\"\nmetric = \"on.mines_pct\"\nmin = 1\n");
        assert!(
            parse(&early).is_ok(),
            "T99 < T109 (numeric, not text, compare)"
        );
        let late =
            format!("{THREE}\n[[target]]\nid = \"T110c\"\nmetric = \"on.mines_pct\"\nmin = 1\n");
        assert!(parse(&late).is_err());
        // A part's run must be a [runs.*] table.
        assert!(parse(&THREE.replace("off.mines_pct", "nope.x")).is_err());
    }

    #[test]
    fn bad_files_are_refused() {
        assert!(parse("[[target]]\nid=\"a\"\nmetric=\"x.y\"\nmin=1\n").is_err());
        assert!(parse("[runs.a]\nkind=\"nope\"\nseeds=1\n").is_err());
        assert!(
            parse("[runs.a]\nkind=\"skirmish\"\nseeds=1\n[[target]]\nid=\"a\"\nmetric=\"a.y\"\n")
                .is_err()
        );
        assert!(
            parse("[runs.a]\nkind=\"skirmish\"\nseeds=1\nbogus=2\n").is_err(),
            "unknown keys are typos"
        );
    }
}
