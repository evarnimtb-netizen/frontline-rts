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
        let run = t.metric.split('.').next().unwrap_or("");
        if !f.runs.contains_key(run) {
            bail!(
                "target {}: metric {:?} starts with {run:?}, which is not a [runs.*] table",
                t.id,
                t.metric
            );
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
    let ok = t.min.is_none_or(|m| x >= m) && t.max.is_none_or(|m| x <= m);
    let tag = match (ok, hard) {
        (true, _) => "PASS",
        (false, true) => "MISS",
        (false, false) => "MISS (report)",
    };
    Line {
        text: format!("{head} {} {bounds} {tag}", num(x)),
        verdict: if ok { Verdict::Pass } else { Verdict::Miss },
        hard,
    }
}

/// The last line: `N pass, M miss (hard: K)`, plus the skipped count when
/// there is one. Returns it with the hard-miss count.
pub fn tally(lines: &[Line]) -> (String, usize) {
    let pass = lines.iter().filter(|l| l.verdict == Verdict::Pass).count();
    let miss: Vec<&Line> = lines
        .iter()
        .filter(|l| matches!(l.verdict, Verdict::Miss | Verdict::NoMetric))
        .collect();
    let hard = miss.iter().filter(|l| l.hard).count();
    let skip = lines.iter().filter(|l| l.verdict == Verdict::Skip).count();
    let mut s = format!("{pass} pass, {} miss (hard: {hard})", miss.len());
    if skip > 0 {
        s += &format!(", {skip} skipped");
    }
    (s, hard)
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
        .filter_map(|t| t.metric.split('.').next())
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
