//! S10a fortify-test rows (docs/slices/S10a-fortifications.md 7): wire,
//! roof and breach, per seed with each faction in turn. Report rows; the
//! targets T91-T93 read them (`fortify10.s10a.*`), `pass` ignores them.
//!
//! * **Wire.** A rifle squad walks [`WIRE_RUN_M`] east to west across the
//!   middle of a 20 m enemy wire line (`World::build_now`) to a goal just
//!   past it (from farther on both sides the path would go round it). Its leader's
//!   speed per tick inside the band (`FortState::in_wire`) against his
//!   speed outside it: `inside_speed_pct` (design done-when "wire slows
//!   infantry to 20 %"). Then the same walk through an 8 m gap the
//!   squad's own field engineers cut (`CutWire`, four sections) and through
//!   the gap an assault card's two Bangalores blew, 3 m either side of the
//!   middle (12 m; one charge opens about 4 m, narrower than the squad's
//!   column, whose men brush the wire and hold the leader to the stretched
//!   pace; two 2 m apart leave their craters on the walk line, whose slope
//!   slows him): the leader's speed while he
//!   crosses the wire's line (within half a band of it) against his speed
//!   before (`cut_gap_speed_pct`, `blown_gap_speed_pct`).
//! * **Roof.** A dugout (pit dug and roof built at once) with five men of
//!   a rifle squad under the roof: an 81 mm bomb of the other faction's
//!   mortar on the roof centre (`World::drop_shell_now`; it must damage
//!   the roof and leave it standing), and in a fresh world a satchel
//!   dropped from 2 m above the roof's top (`World::drop_item_now`; it
//!   rests on the roof and bursts there): `survived_81mm_pct`,
//!   `sheltered_losses`, `broken_heavy_pct`.
//! * **Breach.** The fortify trench with a rifle squad, an MG team in an
//!   MG nest [`NEST_DY`] beside it and 30 m of wire [`BREACH_WIRE_M`] out,
//!   against the faction's assault card (Sturmpioniere, Assault Section)
//!   advancing from 150 m crouched with an MG suppressing the trench as in
//!   the S3b suppressed assault. The engineers head for a spot 8 m short of
//!   the wire; once their lead is [`BANGALORE_FROM_M`] from it (or held
//!   up) the nearest carrier lays a Bangalore at its middle (sent again
//!   every [`RETRY_S`] when he stops), and after the
//!   burst they go through the gap to 25 m, throw and close in;
//!   [`BREACH_S`] in all. The control has no wire and no nest (the MG
//!   team stands on the same spot) and advances straight to 25 m. Reported
//!   against the S3b bar (T10: defenders lose >= 75 %, attackers <= 50 %).
//! * **Breach by the AI** (S10a-ai spec 6, `breach_ai`). The same trench,
//!   rifle squad and MG nest, the wire [`BREACH_AI_WIRE_LEN_M`] long (see
//!   there), and the map's first point moved onto the trench's middle.
//!   The attacker's assault card, an MG team and a rifle squad (the S3c
//!   two-bases composition) stand [`SUPPRESS_START_M`] out; the AI's breach
//!   habit alone (`World::drive_breach`, a fresh `AiState`, the planner's
//!   own phase code: wire seen, path, stage, both support squads as bases
//!   aimed at the defender nearest the crossing, smoke, Bangalore or cut,
//!   retries) runs until its job ends (at most [`BREACH_S`]); then the
//!   scripted assault phase of the breach row runs for [`BREACH_AI_ASSAULT_S`]
//!   (the support squads lift as the MG did there). The control has no
//!   wire and no nest: the hook finds nothing to breach and the assault
//!   runs at once. `gap_opened_pct`: runs in which the attacker cut or blew
//!   a section; `placed_pct`: a charge laid or a cut done.

use super::*;

/// Length of the wire line in the wire row (spec 7).
pub const WIRE_LEN_M: i32 = 20;
/// The wire row's walk: start this far east of the wire; the leader's
/// speed outside counts over the last `WIRE_SETTLED_M` before it.
pub const WIRE_RUN_M: i32 = 70;
const WIRE_SETTLED_M: i32 = 35;
/// Through standing wire the goal lies this far past it.
const WIRE_GOAL_M: i32 = 4;
/// S10a-ai: the plain-wire variant stages this long a line. Since the S17
/// path fix a man walks round a 20 m line's end (2 m past it is free
/// ground), so only a line whose detour costs more than two wired steps
/// makes him walk through the band the row measures.
pub const WIRE_THROUGH_LEN_M: i32 = 100;
/// The breach: wire this far in front of the trench, this long.
pub const BREACH_WIRE_M: i32 = 35;
pub const BREACH_WIRE_LEN_M: i32 = 30;
/// The engineers stop this far short of the wire to lay the Bangalore.
const BREACH_STOP_M: i32 = 8;
/// The breach fight lasts this long (game seconds).
pub const BREACH_S: u64 = 240;
/// The defending MG nest's centre, north of the trench's middle.
pub const NEST_DY: i32 = 22;
/// The breach: the Bangalore goes in once the lead is this close to the
/// wire (the carrier runs the rest), or once the approach gains no metre
/// in `STALL_S` game seconds.
const BANGALORE_FROM_M: i32 = 30;
const STALL_S: u64 = 5;
/// A carrier who stopped is sent again after this long (game seconds).
const RETRY_S: u64 = 5;
/// Longest wait for a cut or a Bangalore (game seconds).
const WAIT_S: u64 = 60;
/// S10a-ai: the AI breach row's wire. Since the S17 path fix the AI's path
/// goes round a 30 m line's end (a few metres' detour beats the wire's
/// cost), so it would breach nothing; a line across the whole approach
/// makes the path cross it (as `WIRE_THROUGH_LEN_M` does for the wire row).
pub const BREACH_AI_WIRE_LEN_M: i32 = 100;
/// S10a-ai: the AI breach row's assault after the job (game seconds).
pub const BREACH_AI_ASSAULT_S: u64 = 150;

#[derive(Clone, Debug, Serialize)]
pub struct S10aReport {
    pub wire: WireExperiment,
    pub roof: RoofExperiment,
    pub breach: BreachExperiment,
    /// S10a-ai: the breach row driven by the AI's breach job.
    pub breach_ai: BreachExperiment,
}

#[derive(Clone, Debug, Serialize)]
pub struct WireExperiment {
    pub name: &'static str,
    pub runs: Vec<WireRun>,
    /// Means over the runs of each variant (0 when any run measured
    /// nothing: a cut or blown run that removed no section counts so).
    pub inside_speed_pct: f64,
    pub cut_gap_speed_pct: f64,
    pub blown_gap_speed_pct: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct WireRun {
    pub seed: u64,
    /// Faction of the crossing squad.
    pub crossing: String,
    /// `wire`, `cut` or `blown`.
    pub variant: &'static str,
    /// Sections removed before the crossing (cut or blown).
    pub sections_removed: u32,
    /// The leader's median speed outside the band before the wire (m/s).
    pub outside_mps: f64,
    /// `wire`: his median speed inside the band; gaps: his mean speed
    /// while within half a band of the wire's line (m/s).
    pub crossing_mps: f64,
    /// Ticks measured for `crossing_mps`.
    pub crossing_ticks: u32,
    /// `crossing_mps` as a percentage of `outside_mps` (None: not measured,
    /// or a cut or blown run that opened no gap). Gap runs count only
    /// ticks within the removed sections' span.
    pub speed_pct: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RoofExperiment {
    pub name: &'static str,
    pub runs: Vec<RoofRun>,
    /// Runs whose roof took the 81 mm bomb and stood (hp between 0 and full; %).
    pub survived_81mm_pct: f64,
    /// Men under the roof lost to the 81 mm bombs, all runs.
    pub sheltered_losses: u32,
    /// Runs whose roof broke under the satchel (%).
    pub broken_heavy_pct: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct RoofRun {
    pub seed: u64,
    /// Faction of the dugout.
    pub sheltered: String,
    /// Roof hit points left after the 81 mm bomb (0: broken) and before it.
    pub roof_hp_after_81mm: u32,
    pub roof_hp_full: u32,
    pub sheltered_losses: u32,
    /// Whether the satchel broke the roof.
    pub broken_by_satchel: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct BreachExperiment {
    pub name: &'static str,
    pub runs: Vec<BreachRun>,
    pub fortified: BreachTotals,
    pub control: BreachTotals,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct BreachTotals {
    pub runs: u32,
    pub defender_losses: u32,
    pub defender_men: u32,
    pub attacker_losses: u32,
    pub attacker_men: u32,
    /// Runs in which a Bangalore blew a gap in the wire (S10a-ai row: or
    /// a section was cut).
    pub gaps_opened: u32,
    /// S10a-ai row: runs with a charge laid or a cut done.
    pub placed: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct BreachRun {
    pub seed: u64,
    /// Faction of the defenders.
    pub entrenched: String,
    /// Wire and MG nest (false: the control).
    pub fortified: bool,
    pub defender_losses: u32,
    pub defender_men: u32,
    pub attacker_losses: u32,
    pub attacker_men: u32,
    pub bangalores: u32,
    pub wire_blown: u32,
    pub frags_thrown: u32,
    /// S10a-ai row: sections the attacker cut; how the AI job ended
    /// (`opened`, `given_up`, `none`: nothing to breach, `running`: still
    /// on at [`BREACH_S`]); its smokes and suppressions.
    pub wire_cut: u32,
    pub job: &'static str,
    pub smokes: u32,
    pub suppressions: u32,
}

fn pct1(a: f64, b: f64) -> f64 {
    if b <= 0.0 {
        0.0
    } else {
        (a * 1000.0 / b).round() / 10.0
    }
}

fn fort_of(w: &World, id: &str) -> Result<u16> {
    w.match_setup()
        .unwrap()
        .forts
        .iter()
        .position(|f| f.id == id)
        .map(|i| i as u16)
        .ok_or_else(|| anyhow::anyhow!("no blueprint {id}"))
}

/// Buys player `p`'s roster card with short id in `ids` and steps until it
/// arrives.
fn call_in_card(w: &mut World, p: u8, ids: &[&str]) -> Result<SquadId> {
    let s = w.match_setup().expect("match world");
    let faction = &s.factions[s.players[p as usize].faction as usize];
    let Some(&ty) = faction.roster.iter().find(|&&t| {
        let id = &s.squad_types[t as usize].id;
        ids.contains(&id.split_once(':').map_or(id.as_str(), |x| x.1))
    }) else {
        bail!("{} has none of {ids:?} in its roster", faction.id);
    };
    let known: Vec<SquadId> = w.game().unwrap().squads().keys().copied().collect();
    step_with(w, p, vec![CommandKind::CallIn { squad_type: ty }]);
    for _ in 0..30 * 120 {
        if let Some(id) = w
            .game()
            .unwrap()
            .squads()
            .keys()
            .find(|id| !known.contains(id))
        {
            return Ok(*id);
        }
        w.step(&[]);
    }
    bail!("the {ids:?} call-in never arrived")
}

/// The assault cards of the slice (spec 3.4).
const ASSAULT_CARDS: [&str; 2] = ["ger_sturmpioniergruppe", "usa_assault_section"];
/// The field engineer cards (cut wire at the field tier).
const FIELD_ENGINEERS: [&str; 2] = ["ger_engineer_squad", "usa_engineer_squad"];

fn lead_pos(w: &World, sq: SquadId) -> Option<FxVec2> {
    w.game()
        .unwrap()
        .squad(sq)
        .map(|s| pos(w, s.lead(w.entities())))
}

/// Places squad `sq` (a line across `facing`) so that its leader stands
/// on `p`.
fn place_lead_at(w: &mut World, sq: SquadId, p: FxVec2, facing: FxVec2) {
    w.place_squad(sq, p, facing);
    if let Some(l) = lead_pos(w, sq) {
        w.place_squad(sq, p + (p - l), facing);
    }
}

fn alive(w: &World, m: SimId) -> bool {
    w.entities().kind()[m.index()] == frontline_sim::EntityKind::Soldier
        && w.entities().combat()[m.index()].hp > 0
}

/// Steps until `done` holds or `secs` pass; whether it held.
fn wait_for(w: &mut World, secs: u64, done: impl Fn(&World) -> bool) -> bool {
    let hz = w.match_setup().unwrap().tick_hz as u64;
    for _ in 0..secs * hz {
        if done(w) {
            return true;
        }
        w.step(&[]);
    }
    done(w)
}

fn median(v: &mut [f64]) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    Some(v[v.len() / 2])
}

/// One wire run (see the module docs): `variant` is `wire`, `cut` or
/// `blown`; `side` crosses, the other player owns the wire.
fn wire_run(args: &FortifyArgs, seed: u64, side: u8, variant: &'static str) -> Result<WireRun> {
    let mut w = world(args, seed)?;
    let owner = 1 - side;
    let wx = TRENCH_X;
    let wire = fort_of(&w, "ww2:barbed_wire")?;
    let half = if variant == "wire" {
        WIRE_THROUGH_LEN_M / 2
    } else {
        WIRE_LEN_M / 2
    };
    if !w.build_now(
        wire,
        &[at(wx, LINE_Y - half), at(wx, LINE_Y + half)],
        PlayerId(owner),
    ) {
        bail!("seed {seed}: the wire line was not staged");
    }
    let before = w.game().unwrap().fort().wire.clone();
    let sections = before.len() as u32;
    let rifles = squad(&w, side, "line")?;
    w.place_squad(rifles, at(wx + 3 * WIRE_RUN_M, LINE_Y + 40), WEST);
    match variant {
        "cut" => {
            let eng = call_in_card(&mut w, side, &FIELD_ENGINEERS)?;
            w.place_squad(eng, at(wx + 6, LINE_Y), WEST);
            // Four sections from 4 m south to 4 m north of the middle.
            for k in 0..4 {
                step_with(
                    &mut w,
                    side,
                    vec![CommandKind::CutWire {
                        squad: eng,
                        at: at(wx, LINE_Y - 3 + 2 * k),
                    }],
                );
                wait_for(&mut w, WAIT_S, |w| {
                    w.game()
                        .unwrap()
                        .squad(eng)
                        .is_none_or(|s| s.order == frontline_sim::Order::Idle)
                });
            }
            w.place_squad(eng, at(wx + 60, LINE_Y - 60), WEST);
        }
        "blown" => {
            // Both charges the card carries, 3 m either side of the
            // middle (module docs): a 12 m gap with its craters off the
            // walk line.
            let eng = call_in_card(&mut w, side, &ASSAULT_CARDS)?;
            let p = side as usize;
            for (k, dy) in [-3, 3].into_iter().enumerate() {
                w.place_squad(eng, at(wx + 12, LINE_Y + dy), WEST);
                let units = members(&w, eng);
                step_with(
                    &mut w,
                    side,
                    vec![CommandKind::PlaceBangalore {
                        units,
                        target: at(wx, LINE_Y + dy),
                    }],
                );
                wait_for(&mut w, WAIT_S, |w| {
                    let f = w.game().unwrap().fort();
                    f.accounts[p].bangalores > k as u32 && f.charges.is_empty()
                });
            }
            w.place_squad(eng, at(wx + 60, LINE_Y - 60), WEST);
        }
        _ => {}
    }
    let removed = sections - w.game().unwrap().fort().wire.len() as u32;
    // The walk goes through the middle of the gap (the wire's middle when
    // nothing was removed).
    let gone: Vec<Fx> = before
        .iter()
        .filter(|(id, _)| !w.game().unwrap().fort().wire.contains_key(id))
        .map(|(_, s)| (s.a.y + s.b.y).div_int(2))
        .collect();
    // The gap's span along the line (a crossing tick counts only there).
    let span = gone.iter().fold(None, |acc: Option<(Fx, Fx)>, &y| {
        Some(acc.map_or((y, y), |(lo, hi)| (lo.min(y), hi.max(y))))
    });
    let walk_y = if gone.is_empty() {
        Fx::from_int(LINE_Y)
    } else {
        gone.iter()
            .fold(Fx::ZERO, |a, &y| a + y)
            .div_int(gone.len() as i32)
    };
    let on = |x: i32| FxVec2::new(Fx::from_int(x), walk_y);
    // From WIRE_RUN_M east (the column forms on the way; only the last
    // WIRE_SETTLED_M before the wire count as "outside"). Through the
    // standing wire the goal lies just past it: from farther on both
    // sides the infantry path would go round a 20 m line.
    let goal_x = if variant == "wire" {
        wx - WIRE_GOAL_M
    } else {
        wx - WIRE_RUN_M / 2
    };
    place_lead_at(&mut w, rifles, on(wx + WIRE_RUN_M), WEST);
    w.step(&[]);
    step_with(
        &mut w,
        side,
        vec![CommandKind::Move {
            squad: rifles,
            target: on(goal_x),
        }],
    );
    let hz = w.match_setup().unwrap().tick_hz as f64;
    let forts = w.match_setup().unwrap().forts.clone();
    let band = forts[wire as usize]
        .wire
        .as_ref()
        .map_or(0.75, |b| Fx::from(b.band).to_f32() as f64 / 2.0);
    let (mut outside, mut inside, mut crossing) = (Vec::new(), Vec::new(), Vec::new());
    let mut entered = false;
    let end = w.tick() + 120 * hz as u64;
    while w.tick() < end {
        let Some(from) = lead_pos(&w, rifles) else {
            break;
        };
        w.step(&[]);
        let Some(to) = lead_pos(&w, rifles) else {
            break;
        };
        let v = (to - from).length().to_f32() as f64 * hz;
        let in_band = w.game().unwrap().fort().in_wire(&forts, from);
        let x = from.x.to_f32() as f64;
        let y = from.y.to_f32() as f64;
        let in_gap = span
            .is_some_and(|(lo, hi)| y >= lo.to_f32() as f64 - 1.0 && y <= hi.to_f32() as f64 + 1.0);
        let on_line = (x - wx as f64).abs() <= band && (variant == "wire" || in_gap);
        entered |= in_band || on_line;
        if in_band {
            inside.push(v);
        } else if !entered && v > 0.1 && x <= (wx + WIRE_SETTLED_M) as f64 {
            outside.push(v);
        }
        if on_line {
            crossing.push(v);
        }
        if x < wx as f64 - band - 1.0 {
            break;
        }
    }
    let out = median(&mut outside).unwrap_or(0.0);
    let (cross, n) = if variant == "wire" {
        (median(&mut inside).unwrap_or(0.0), inside.len())
    } else {
        let n = crossing.len();
        (crossing.iter().sum::<f64>() / n.max(1) as f64, n)
    };
    Ok(WireRun {
        seed,
        crossing: faction(w.match_setup().unwrap(), side),
        variant,
        sections_removed: removed,
        outside_mps: (out * 1000.0).round() / 1000.0,
        crossing_mps: (cross * 1000.0).round() / 1000.0,
        crossing_ticks: n as u32,
        // A gap variant that opened no gap measured nothing.
        speed_pct: (n > 0 && out > 0.0 && (variant == "wire" || removed > 0))
            .then(|| pct1(cross, out)),
    })
}

/// A dugout of `own` at the line's middle with five men of its rifle
/// squad under the roof (the rest 60 m off); the roof piece and the men.
fn dugout(w: &mut World, own: u8) -> Result<(u32, Vec<SimId>)> {
    let f = fort_of(w, "ww2:dugout")?;
    let pit = w.match_setup().unwrap().rules.dugout_pit.pit_line;
    let half = Fx::from(pit).div_int(2).max(Fx::from_milli(100));
    let c = at(TRENCH_X, LINE_Y);
    let e = FxVec2::new(half, Fx::ZERO);
    if !w.dig_pit_now(&[c - e, c + e], frontline_content::DigProfile::DugoutPit) {
        bail!("the dugout pit was not dug");
    }
    let before: Vec<u32> = w.game().unwrap().fort().pieces.keys().copied().collect();
    if !w.build_now(f, &[c, c + EAST], PlayerId(own)) {
        bail!("the dugout was not staged");
    }
    let roof = *w
        .game()
        .unwrap()
        .fort()
        .pieces
        .keys()
        .find(|k| !before.contains(k))
        .ok_or_else(|| anyhow::anyhow!("no roof piece"))?;
    let sq = squad(w, own, "line")?;
    let all = members(w, sq);
    for (k, &m) in all.iter().enumerate() {
        let p = if k < 5 {
            c + FxVec2::new(Fx::from_milli(400 * (k as i32 - 2)), Fx::ZERO)
        } else {
            at(TRENCH_X + 60 + k as i32, LINE_Y + 60)
        };
        w.place_soldier(m, p);
    }
    w.step(&[]);
    Ok((roof, all.into_iter().take(5).collect()))
}

fn roof_hp(w: &World, roof: u32) -> u32 {
    w.game()
        .unwrap()
        .fort()
        .pieces
        .get(&roof)
        .map_or(0, |q| q.hp)
}

/// The other faction's 81 mm mortar weapon.
fn mortar_of(w: &World, p: u8) -> Result<u16> {
    let s = w.match_setup().unwrap();
    let id = if faction(s, p).ends_with("germany") {
        "ww2:grw34"
    } else {
        "ww2:m1_mortar"
    };
    s.weapons
        .iter()
        .position(|x| x.id == id)
        .map(|i| i as u16)
        .ok_or_else(|| anyhow::anyhow!("no {id}"))
}

fn roof_run(args: &FortifyArgs, seed: u64, own: u8) -> Result<RoofRun> {
    let att = 1 - own;
    let hz = |w: &World| w.match_setup().unwrap().tick_hz as u64;
    // The 81 mm bomb on the roof centre.
    let mut w = world(args, seed)?;
    let (roof, men) = dugout(&mut w, own)?;
    let full = roof_hp(&w, roof);
    let mortar = mortar_of(&w, att)?;
    if !w.drop_shell_now(att, mortar, at(TRENCH_X, LINE_Y)) {
        bail!("seed {seed}: the bomb was not dropped");
    }
    for _ in 0..2 * hz(&w) {
        w.step(&[]);
    }
    let hp_after = roof_hp(&w, roof);
    let lost = men.iter().filter(|&&m| !alive(&w, m)).count() as u32;
    // A satchel on a fresh roof.
    let mut w = world(args, seed)?;
    let (roof, _) = dugout(&mut w, own)?;
    let thrower_sq = squad(&w, att, "line")?;
    w.place_squad(thrower_sq, at(TRENCH_X + 40, LINE_Y + 40), WEST);
    let thrower = members(&w, thrower_sq)[0];
    let satchel = w
        .match_setup()
        .unwrap()
        .items
        .iter()
        .position(|x| x.kind == ItemKind::Satchel)
        .ok_or_else(|| anyhow::anyhow!("no satchel item"))? as u16;
    // Dropped from 2 m above the roof's top: it comes to rest on the roof
    // and bursts there.
    let c = at(TRENCH_X, LINE_Y);
    let q = w.game().unwrap().fort().pieces[&roof].clone();
    let top = q.z
        + Fx::from(w.match_setup().unwrap().forts[q.fort as usize].pieces[q.part as usize].half[2])
            .mul_int(2);
    let above = top + Fx::from_int(2) - w.terrain().height_at(c);
    if !w.drop_item_now(thrower, satchel, c, above) {
        bail!("seed {seed}: the satchel was not dropped");
    }
    for _ in 0..5 * hz(&w) {
        w.step(&[]);
    }
    Ok(RoofRun {
        seed,
        sheltered: faction(w.match_setup().unwrap(), own),
        roof_hp_after_81mm: hp_after,
        roof_hp_full: full,
        sheltered_losses: lost,
        broken_by_satchel: roof_hp(&w, roof) == 0,
    })
}

/// One breach (see the module docs): `dug` defends, the other attacks.
fn breach_run(args: &FortifyArgs, seed: u64, dug: u8, fortified: bool) -> Result<BreachRun> {
    let mut w = world(args, seed)?;
    let att = 1 - dug;
    let a = squad(&w, dug, "line")?;
    dig(&mut w);
    w.place_squad(a, at(TRENCH_X, LINE_Y), EAST);
    let nest_c = at(TRENCH_X, LINE_Y + NEST_DY);
    let wire_x = TRENCH_X + BREACH_WIRE_M;
    if fortified {
        let nest = fort_of(&w, "ww2:mg_nest")?;
        let pit = w.match_setup().unwrap().rules.mg_nest_pit.pit_line;
        let half = Fx::from(pit).div_int(2).max(Fx::from_milli(100));
        let e = FxVec2::new(half, Fx::ZERO);
        if !w.dig_pit_now(
            &[nest_c - e, nest_c + e],
            frontline_content::DigProfile::MgNestPit,
        ) || !w.build_now(nest, &[nest_c, nest_c + EAST], PlayerId(dug))
        {
            bail!("seed {seed}: the MG nest was not staged");
        }
        let wire = fort_of(&w, "ww2:barbed_wire")?;
        let half = BREACH_WIRE_LEN_M / 2;
        if !w.build_now(
            wire,
            &[at(wire_x, LINE_Y - half), at(wire_x, LINE_Y + half)],
            PlayerId(dug),
        ) {
            bail!("seed {seed}: the wire was not staged");
        }
    }
    let mg = call_in(&mut w, dug, "mg")?;
    w.place_squad(mg, nest_c, EAST);
    let eng = call_in_card(&mut w, att, &ASSAULT_CARDS)?;
    w.place_squad(eng, at(TRENCH_X + SUPPRESS_START_M, LINE_Y), WEST);
    let amg = call_in(&mut w, att, "mg")?;
    w.place_squad(amg, at(TRENCH_X + SUPPRESS_START_M, LINE_Y - 20), WEST);
    let units = members(&w, amg);
    step_with(
        &mut w,
        att,
        vec![CommandKind::SuppressUnits {
            units,
            line: vec![at(TRENCH_X, LINE_Y - 15), at(TRENCH_X, LINE_Y + 15)],
        }],
    );
    let def_men = (members(&w, a).len() + members(&w, mg).len()) as u32;
    let att_men = (members(&w, eng).len() + members(&w, amg).len()) as u32;
    let hz = w.match_setup().unwrap().tick_hz as u64;
    let start = w.tick();
    let (lost_d, lost_a) = (losses(&w, dug), losses(&w, att));
    let throws = w.stats().throws;
    while w.tick() < start + SUPPRESS_LEAD_S * hz {
        w.step(&[]);
    }
    let men = members(&w, eng);
    let stop_x = wire_x + BREACH_STOP_M;
    let first_goal = if fortified {
        at(stop_x, LINE_Y)
    } else {
        at(TRENCH_X + ASSAULT_GAP_M, LINE_Y)
    };
    step_with(
        &mut w,
        att,
        vec![
            CommandKind::SetStance {
                units: men,
                stance: frontline_sim::StanceOrder::Crouch,
            },
            CommandKind::Move {
                squad: eng,
                target: first_goal,
            },
        ],
    );
    #[derive(PartialEq)]
    enum Phase {
        Approach,
        Placing(u64),
        Assault,
    }
    let mut phase = if fortified {
        Phase::Approach
    } else {
        Phase::Assault
    };
    let throw_x = Fx::from_int(TRENCH_X + ASSAULT_THROW_M);
    let lift_x =
        Fx::from_int(TRENCH_X + w.match_setup().unwrap().rules.ai.assault.lift_fire_m as i32);
    let (mut first_wave, mut closing, mut lifted) = (None, false, false);
    let p = att as usize;
    // Approach progress: the lead's best x and when it last improved by 1 m.
    let mut best = (Fx::from_int(i32::MAX / 65536), w.tick());
    let mut last_order = w.tick();
    while w.tick() < start + BREACH_S * hz {
        let lead = w
            .game()
            .unwrap()
            .squad(eng)
            .map(|s| (s.nav.phase, pos(&w, s.lead(w.entities())).x));
        let Some((nav, lead_x)) = lead else {
            w.step(&[]);
            continue;
        };
        match phase {
            Phase::Approach => {
                if lead_x + Fx::ONE < best.0 {
                    best = (lead_x, w.tick());
                }
                // At the stop, or held up (no metre gained in STALL_S): the
                // carrier goes from here.
                if nav == frontline_sim::NavPhase::Holding
                    || lead_x <= Fx::from_int(wire_x + BANGALORE_FROM_M)
                    || w.tick() >= best.1 + STALL_S * hz
                {
                    phase = Phase::Placing(w.tick());
                    last_order = w.tick();
                    let units = members(&w, eng);
                    step_with(
                        &mut w,
                        att,
                        vec![CommandKind::PlaceBangalore {
                            units,
                            target: at(wire_x, LINE_Y),
                        }],
                    );
                } else {
                    w.step(&[]);
                }
                continue;
            }
            Phase::Placing(t0) => {
                let f = w.game().unwrap().fort();
                let burst = f.accounts[p].bangalores > 0 && f.charges.is_empty();
                // A carrier who stopped (pinned, order dropped) is sent
                // again every RETRY_S, as a player would.
                let placing = members(&w, eng).iter().any(|m| {
                    w.entities().kit()[m.index()]
                        .windup
                        .is_some_and(|x| x.kind == ItemKind::Bangalore)
                });
                let laid = f.accounts[p].bangalores > 0;
                if !burst
                    && !laid
                    && !placing
                    && w.tick() >= last_order + RETRY_S * hz
                    && w.tick() < t0 + WAIT_S * hz
                {
                    last_order = w.tick();
                    let units = members(&w, eng);
                    step_with(
                        &mut w,
                        att,
                        vec![CommandKind::PlaceBangalore {
                            units,
                            target: at(wire_x, LINE_Y),
                        }],
                    );
                    continue;
                }
                if burst || w.tick() >= t0 + WAIT_S * hz {
                    phase = Phase::Assault;
                    step_with(
                        &mut w,
                        att,
                        vec![CommandKind::Move {
                            squad: eng,
                            target: at(TRENCH_X + ASSAULT_GAP_M, LINE_Y),
                        }],
                    );
                } else {
                    w.step(&[]);
                }
                continue;
            }
            Phase::Assault => {}
        }
        let close = nav == frontline_sim::NavPhase::Holding || lead_x <= throw_x;
        let frags_left = members(&w, eng)
            .iter()
            .any(|m| w.entities().kit()[m.index()].items[ItemKind::Frag.index()] > 0);
        if close && first_wave.is_none() {
            first_wave = Some(w.tick());
        }
        if !closing && first_wave.is_some_and(|t| !frags_left || w.tick() >= t + CLOSE_IN_S * hz) {
            closing = true;
            step_with(
                &mut w,
                att,
                vec![CommandKind::Move {
                    squad: eng,
                    target: at(TRENCH_X, LINE_Y),
                }],
            );
        } else if !lifted && lead_x <= lift_x {
            lifted = true;
            step_with(&mut w, att, vec![CommandKind::Stop { squad: amg }]);
        } else if close && !closing && (w.tick() - start).is_multiple_of(hz) {
            throw_wave(&mut w, att, eng);
        } else {
            w.step(&[]);
        }
    }
    let acc = w.game().unwrap().fort().accounts[p].clone();
    Ok(BreachRun {
        seed,
        entrenched: faction(w.match_setup().unwrap(), dug),
        fortified,
        defender_losses: losses(&w, dug) - lost_d,
        defender_men: def_men,
        attacker_losses: losses(&w, att) - lost_a,
        attacker_men: att_men,
        bangalores: acc.bangalores,
        wire_blown: acc.wire_blown,
        frags_thrown: (w.stats().throws - throws) as u32,
        wire_cut: acc.wire_cut,
        job: "scripted",
        smokes: 0,
        suppressions: 0,
    })
}

/// A world for the AI breach row: the map's first point on the trench's
/// middle (the point the AI attacks).
fn breach_ai_world(args: &FortifyArgs, seed: u64) -> Result<World> {
    let mut setup = match_setup(&SkirmishArgs {
        seed,
        mods: args.mods.clone(),
        rules_override: args.rules_override.clone(),
        ..SkirmishArgs::default()
    })?;
    setup.players.iter_mut().for_each(|p| p.is_ai = false);
    setup.rules.start_manpower = 10_000;
    let Some(p0) = setup.map.points.first_mut() else {
        bail!("the map has no capture point");
    };
    p0.pos = frontline_content::setup::FixedVec2::from_m([TRENCH_X as u32, LINE_Y as u32]);
    Ok(World::new_match(setup, args.threads)?)
}

/// One AI breach (see the module docs): `dug` defends, the other attacks.
fn breach_ai_run(args: &FortifyArgs, seed: u64, dug: u8, fortified: bool) -> Result<BreachRun> {
    let mut w = breach_ai_world(args, seed)?;
    let att = 1 - dug;
    let a = squad(&w, dug, "line")?;
    dig(&mut w);
    w.place_squad(a, at(TRENCH_X, LINE_Y), EAST);
    let nest_c = at(TRENCH_X, LINE_Y + NEST_DY);
    let wire_x = TRENCH_X + BREACH_WIRE_M;
    if fortified {
        let nest = fort_of(&w, "ww2:mg_nest")?;
        let pit = w.match_setup().unwrap().rules.mg_nest_pit.pit_line;
        let half = Fx::from(pit).div_int(2).max(Fx::from_milli(100));
        let e = FxVec2::new(half, Fx::ZERO);
        if !w.dig_pit_now(
            &[nest_c - e, nest_c + e],
            frontline_content::DigProfile::MgNestPit,
        ) || !w.build_now(nest, &[nest_c, nest_c + EAST], PlayerId(dug))
        {
            bail!("seed {seed}: the MG nest was not staged");
        }
        let wire = fort_of(&w, "ww2:barbed_wire")?;
        let half = BREACH_AI_WIRE_LEN_M / 2;
        if !w.build_now(
            wire,
            &[at(wire_x, LINE_Y - half), at(wire_x, LINE_Y + half)],
            PlayerId(dug),
        ) {
            bail!("seed {seed}: the wire was not staged");
        }
    }
    let mg = call_in(&mut w, dug, "mg")?;
    w.place_squad(mg, nest_c, EAST);
    let eng = call_in_card(&mut w, att, &ASSAULT_CARDS)?;
    let start_x = TRENCH_X + SUPPRESS_START_M;
    w.place_squad(eng, at(start_x, LINE_Y), WEST);
    let amg = call_in(&mut w, att, "mg")?;
    w.place_squad(amg, at(start_x, LINE_Y - 20), WEST);
    let rifle = call_in(&mut w, att, "line")?;
    w.place_squad(rifle, at(start_x, LINE_Y + 20), WEST);
    let def_men = (members(&w, a).len() + members(&w, mg).len()) as u32;
    let att_men =
        (members(&w, eng).len() + members(&w, amg).len() + members(&w, rifle).len()) as u32;
    let hz = w.match_setup().unwrap().tick_hz as u64;
    let start = w.tick();
    let (lost_d, lost_a) = (losses(&w, dug), losses(&w, att));
    let throws = w.stats().throws;
    let p = att as usize;
    // The AI's breach job.
    let mut ai = frontline_sim::ai::AiState::new(w.tick());
    let mut job = "running";
    while w.tick() < start + BREACH_S * hz {
        if let Some(end) = w.drive_breach(&mut ai, PlayerId(att), eng, &[amg, rifle], 0) {
            job = match end {
                Some(frontline_sim::ai::BreachEnd::Opened) => "opened",
                Some(frontline_sim::ai::BreachEnd::GivenUp) => "given_up",
                None => "none",
            };
            break;
        }
    }
    // Commands the job still had waiting are dropped: the assault takes over.
    let support = [amg, rifle];
    let assault_end = w.tick() + BREACH_AI_ASSAULT_S * hz;
    if w.game().unwrap().squad(eng).is_some() {
        step_with(
            &mut w,
            att,
            vec![CommandKind::Move {
                squad: eng,
                target: at(TRENCH_X + ASSAULT_GAP_M, LINE_Y),
            }],
        );
    }
    let throw_x = Fx::from_int(TRENCH_X + ASSAULT_THROW_M);
    let lift_x =
        Fx::from_int(TRENCH_X + w.match_setup().unwrap().rules.ai.assault.lift_fire_m as i32);
    let (mut first_wave, mut closing, mut lifted) = (None, false, false);
    while w.tick() < assault_end {
        let lead = w
            .game()
            .unwrap()
            .squad(eng)
            .map(|s| (s.nav.phase, pos(&w, s.lead(w.entities())).x));
        let Some((nav, lead_x)) = lead else {
            w.step(&[]);
            continue;
        };
        let close = nav == frontline_sim::NavPhase::Holding || lead_x <= throw_x;
        let frags_left = members(&w, eng)
            .iter()
            .any(|m| w.entities().kit()[m.index()].items[ItemKind::Frag.index()] > 0);
        if close && first_wave.is_none() {
            first_wave = Some(w.tick());
        }
        if !closing && first_wave.is_some_and(|t| !frags_left || w.tick() >= t + CLOSE_IN_S * hz) {
            closing = true;
            step_with(
                &mut w,
                att,
                vec![CommandKind::Move {
                    squad: eng,
                    target: at(TRENCH_X, LINE_Y),
                }],
            );
        } else if !lifted && lead_x <= lift_x {
            lifted = true;
            let stops = support
                .iter()
                .filter(|&&q| w.game().unwrap().squad(q).is_some())
                .map(|&q| CommandKind::Stop { squad: q })
                .collect();
            step_with(&mut w, att, stops);
        } else if close && !closing && (w.tick() - start).is_multiple_of(hz) {
            throw_wave(&mut w, att, eng);
        } else {
            w.step(&[]);
        }
    }
    let acc = w.game().unwrap().fort().accounts[p].clone();
    Ok(BreachRun {
        seed,
        entrenched: faction(w.match_setup().unwrap(), dug),
        fortified,
        defender_losses: losses(&w, dug) - lost_d,
        defender_men: def_men,
        attacker_losses: losses(&w, att) - lost_a,
        attacker_men: att_men,
        bangalores: acc.bangalores,
        wire_blown: acc.wire_blown,
        frags_thrown: (w.stats().throws - throws) as u32,
        wire_cut: acc.wire_cut,
        job,
        smokes: ai.counters.breach_smokes,
        suppressions: ai.counters.breach_suppress,
    })
}

/// The three S10a rows over `seeds`, each faction in turn.
pub fn s10a_rows(args: &FortifyArgs, seeds: std::ops::Range<u64>) -> Result<S10aReport> {
    let (mut wire, mut roof, mut breach) = (Vec::new(), Vec::new(), Vec::new());
    let mut breach_ai = Vec::new();
    for seed in seeds {
        for side in [0u8, 1] {
            for variant in ["wire", "cut", "blown"] {
                wire.push(wire_run(args, seed, side, variant)?);
            }
            roof.push(roof_run(args, seed, side)?);
            for fortified in [true, false] {
                breach.push(breach_run(args, seed, side, fortified)?);
            }
            for fortified in [true, false] {
                breach_ai.push(breach_ai_run(args, seed, side, fortified)?);
            }
        }
    }
    // A variant mean is 0 (the row fails) when any of its runs measured
    // nothing (a gap variant that opened no gap included).
    let mean = |v: &str| {
        let runs: Vec<&WireRun> = wire.iter().filter(|r| r.variant == v).collect();
        let xs: Vec<f64> = runs.iter().filter_map(|r| r.speed_pct).collect();
        if xs.is_empty() || xs.len() < runs.len() {
            0.0
        } else {
            (xs.iter().sum::<f64>() * 10.0 / xs.len() as f64).round() / 10.0
        }
    };
    let wire = WireExperiment {
        name: "rifle squad across 100 m of wire, through a cut gap and a Bangalore gap in 20 m",
        inside_speed_pct: mean("wire"),
        cut_gap_speed_pct: mean("cut"),
        blown_gap_speed_pct: mean("blown"),
        runs: wire,
    };
    let n = roof.len() as f64;
    let roof = RoofExperiment {
        name: "dugout with five men under its roof: an 81 mm bomb, a satchel on the roof centre",
        survived_81mm_pct: pct1(
            roof.iter()
                .filter(|r| r.roof_hp_after_81mm > 0 && r.roof_hp_after_81mm < r.roof_hp_full)
                .count() as f64,
            n,
        ),
        sheltered_losses: roof.iter().map(|r| r.sheltered_losses).sum(),
        broken_heavy_pct: pct1(
            roof.iter().filter(|r| r.broken_by_satchel).count() as f64,
            n,
        ),
        runs: roof,
    };
    let totals = |runs: &[BreachRun], f: bool, ai: bool| {
        let mut t = BreachTotals::default();
        for r in runs.iter().filter(|r| r.fortified == f) {
            t.runs += 1;
            t.defender_losses += r.defender_losses;
            t.defender_men += r.defender_men;
            t.attacker_losses += r.attacker_losses;
            t.attacker_men += r.attacker_men;
            let cut = if ai { r.wire_cut } else { 0 };
            t.gaps_opened += (r.wire_blown + cut > 0) as u32;
            t.placed += (r.bangalores + cut > 0) as u32;
        }
        t
    };
    let breach = BreachExperiment {
        name: "assault engineers with an MG against a trench, MG nest and wire 35 m out (control: no wire, no nest)",
        fortified: totals(&breach, true, false),
        control: totals(&breach, false, false),
        runs: breach,
    };
    let breach_ai = BreachExperiment {
        name: "the AI's breach job: assault card, MG team and rifle squad against a trench, MG nest and 100 m of wire 35 m out (control: no wire, no nest)",
        fortified: totals(&breach_ai, true, true),
        control: totals(&breach_ai, false, true),
        runs: breach_ai,
    };
    Ok(S10aReport {
        wire,
        roof,
        breach,
        breach_ai,
    })
}

impl BreachTotals {
    pub fn defender_loss_pct(&self) -> f64 {
        pct1(self.defender_losses as f64, self.defender_men as f64)
    }
    pub fn attacker_loss_pct(&self) -> f64 {
        pct1(self.attacker_losses as f64, self.attacker_men as f64)
    }
    pub fn gap_opened_pct(&self) -> f64 {
        pct1(self.gaps_opened as f64, self.runs as f64)
    }
    pub fn placed_pct(&self) -> f64 {
        pct1(self.placed as f64, self.runs as f64)
    }
}

/// Text lines for the summary.
pub fn summary_text(r: &S10aReport) -> String {
    let mut out = String::new();
    let w = &r.wire;
    out += &format!("S10a: {}\n", w.name);
    out += "  seed  crossing      variant  removed  outside_mps  crossing_mps  ticks  pct\n";
    for run in &w.runs {
        out += &format!(
            "  {:<5} {:<13} {:<8} {:>7}  {:>11.3}  {:>12.3}  {:>5}  {}\n",
            run.seed,
            run.crossing,
            run.variant,
            run.sections_removed,
            run.outside_mps,
            run.crossing_mps,
            run.crossing_ticks,
            run.speed_pct
                .map_or_else(|| "n/a".into(), |p| format!("{p:.1} %"))
        );
    }
    out += &format!(
        "  leader in the band {:.1} % of his speed outside (T91a 19-21); through a cut gap {:.1} %, a blown gap {:.1} % (T91b-c >= 95)\n",
        w.inside_speed_pct, w.cut_gap_speed_pct, w.blown_gap_speed_pct
    );
    let f = &r.roof;
    out += &format!("S10a: {}\n", f.name);
    out += "  seed  sheltered     roof_hp_after_81mm  losses  satchel_broke\n";
    for run in &f.runs {
        out += &format!(
            "  {:<5} {:<13} {:>18}  {:>6}  {}\n",
            run.seed,
            run.sheltered,
            run.roof_hp_after_81mm,
            run.sheltered_losses,
            if run.broken_by_satchel { "yes" } else { "no" }
        );
    }
    out += &format!(
        "  roof survived the 81 mm in {:.1} % of runs, {} sheltered men lost (T92a-b); a satchel broke it in {:.1} % (T92c)\n",
        f.survived_81mm_pct, f.sheltered_losses, f.broken_heavy_pct
    );
    let b = &r.breach;
    out += &format!("S10a: {}\n", b.name);
    out += "  seed  entrenched    fortified  defender_losses  attacker_losses  bangalores  blown  frags\n";
    for run in &b.runs {
        out += &format!(
            "  {:<5} {:<13} {:<10} {:>9} of {:<4} {:>9} of {:<4} {:>10}  {:>5}  {:>5}\n",
            run.seed,
            run.entrenched,
            if run.fortified { "yes" } else { "no" },
            run.defender_losses,
            run.defender_men,
            run.attacker_losses,
            run.attacker_men,
            run.bangalores,
            run.wire_blown,
            run.frags_thrown
        );
    }
    for (name, t) in [("fortified", &b.fortified), ("control", &b.control)] {
        out += &format!(
            "  {name}: defenders lost {:.1} %, attackers {:.1} %, gap opened in {:.1} % of runs (report against S3b T10: >= 75 %, <= 50 %)\n",
            t.defender_loss_pct(),
            t.attacker_loss_pct(),
            t.gap_opened_pct()
        );
    }
    let b = &r.breach_ai;
    out += &format!("S10a-ai: {}\n", b.name);
    out += "  seed  entrenched    fortified  defender_losses  attacker_losses  job       bangalores  blown  cut  smokes  frags\n";
    for run in &b.runs {
        out += &format!(
            "  {:<5} {:<13} {:<10} {:>9} of {:<4} {:>9} of {:<4} {:<9} {:>10}  {:>5}  {:>3}  {:>6}  {:>5}\n",
            run.seed,
            run.entrenched,
            if run.fortified { "yes" } else { "no" },
            run.defender_losses,
            run.defender_men,
            run.attacker_losses,
            run.attacker_men,
            run.job,
            run.bangalores,
            run.wire_blown,
            run.wire_cut,
            run.smokes,
            run.frags_thrown
        );
    }
    for (name, t) in [("fortified", &b.fortified), ("control", &b.control)] {
        out += &format!(
            "  {name}: gap opened in {:.1} % of runs (T93f >= 60), placed {:.1} % (T93h >= 60), attackers lost {:.1} % (T93g <= 60), defenders {:.1} %\n",
            t.gap_opened_pct(),
            t.placed_pct(),
            t.attacker_loss_pct(),
            t.defender_loss_pct()
        );
    }
    out
}
