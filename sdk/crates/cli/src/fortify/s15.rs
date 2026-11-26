//! S15 fortify-test rows (docs/slices/S15-ussr.md 9, rows W1-W4, hard):
//! the `fortify10.s15` scene. The S15 switches are on in the row only,
//! on test-made stand-in defs (no def in the mod carries a plate, rider
//! slots, a foreign feed or a bayonet before Build B): every Grenadier
//! wears a 2.0 mm torso plate (120 deg); the Panzer IV H takes ten deck
//! rider slots; the US 81 mm mortar takes the GrW 34 bomb as a German
//! foreign feed (the stand-in Soviet mortar); the Kar 98k takes a
//! bayonet block (reach 140 cm: the stand-in Mosin). Every row is staged
//! fire and player commands; no AI code runs. One run on the first seed
//! (the rows draw nothing but the mortar's aim).
//!
//! - `plate` (W1): a lone Grenadier held 50 m from the fire, facing it.
//!   `rounds` MP 40 rounds at his chest, his stance and suppression put
//!   back before each: `stops` (his player's `plate_stops`; the unplated
//!   control `control_stops`). Rounds until he is down (no reset, at most
//!   `rounds`): `survive_ratio_pct` (plated over `control_rounds_to_down`,
//!   %), `rifle_diff` (Kar 98k: plated less unplated, absolute), `back_diff`
//!   (MP 40 from behind, the same). Pass: stops > 0, control 0, ratio >=
//!   200, both diffs 0.
//! - `riders` (W2): a Grenadier section seated on the stand-in Panzer IV,
//!   the tank ordered 300 m east; after 10 s a mortar bomb bursts 10 m off
//!   the tank: `off_ms` until no rider is on the deck (at most 10 s);
//!   second run, the tank knocked out (staged: its engine set burning, as
//!   no hull rocket here knocks it out and a hit alone empties the deck by
//!   R3b): `ko_on_deck` (riders left on the deck one tick later),
//!   `ko_stunned` (the squad's living men in their stun). Control:
//!   `[riders] enabled` 0 (the slots dropped as the setup build does):
//!   `Embark` boards nobody in 20 s (`control_refused` 1). Pass: off_ms
//!   <= 3000, ko_on_deck 0, control_refused 1.
//! - `foreign` (W3): the stand-in Soviet mortar squad, dry, at three
//!   German shells crates its player holds; missions of 4 rounds at a
//!   point 200 m east whenever it is idle with bombs: `drawn`, `fired`
//!   (its `foreign_drawn`, `foreign_fired`) after at most 300 s or 20
//!   fired; the same squad with its own bombs fires 20 (`own_fired`);
//!   `wider` 1 when the mean miss of the foreign bombs is larger than of
//!   the own (`own_miss_cm`). Control: a GrW 34 squad at a US crate its
//!   player holds for 60 s: `german_drawn` (bombs gained). Pass: fired >=
//!   20, wider 1, german_drawn 0.
//! - `bayonet` (W4): the S14 R5 strike scene (two lone riflemen held)
//!   with `[melee] bayonets` 1: `strikes` (bayonet strikes), `hits_150`
//!   and `hits_190` at a man 1.5 m and 1.9 m ahead; control `[melee]
//!   bayonets` 0 (the block in the setup, unused): `off_hits_150`. Pass:
//!   hits_150 = strikes = 100, hits_190 0, off_hits_150 0.
use anyhow::{Result, bail};
use frontline_content::MatchSetup;
use frontline_sim::command::CommandKind;
use frontline_sim::{Fx, FxVec2, FxVec3, PlayerId, SimId, SquadId, World};
use serde::Serialize;

use super::s10b::fields;
use super::s14::flame_world;
use super::{EAST, FortifyArgs, LINE_Y, TRENCH_X, at, members, step_with};

/// W1: rounds fired per measure.
pub const PLATE_ROUNDS: u32 = 100;
/// W4: strikes per distance.
pub const BAYONET_STRIKES: u32 = 100;
const HZ: u64 = 30;

/// `fortify10.s15.plate` (W1, hard).
#[derive(Clone, Debug, Default, Serialize)]
pub struct PlateRow {
    pub rounds: Option<f64>,
    pub stops: Option<f64>,
    pub control_stops: Option<f64>,
    pub rounds_to_down: Option<f64>,
    pub control_rounds_to_down: Option<f64>,
    pub survive_ratio_pct: Option<f64>,
    pub rifle_diff: Option<f64>,
    pub back_diff: Option<f64>,
    pub pass: Option<bool>,
}

/// `fortify10.s15.riders` (W2, hard).
#[derive(Clone, Debug, Default, Serialize)]
pub struct RidersRow {
    pub boarded: Option<f64>,
    pub off_ms: Option<f64>,
    pub ko_on_deck: Option<f64>,
    pub ko_stunned: Option<f64>,
    pub control_refused: Option<f64>,
    pub pass: Option<bool>,
}

/// `fortify10.s15.foreign` (W3, hard).
#[derive(Clone, Debug, Default, Serialize)]
pub struct ForeignRow {
    pub drawn: Option<f64>,
    pub fired: Option<f64>,
    pub own_fired: Option<f64>,
    pub miss_cm: Option<f64>,
    pub own_miss_cm: Option<f64>,
    pub wider: Option<f64>,
    pub german_drawn: Option<f64>,
    pub pass: Option<bool>,
}

/// `fortify10.s15.bayonet` (W4, hard).
#[derive(Clone, Debug, Default, Serialize)]
pub struct BayonetRow {
    pub strikes: Option<f64>,
    pub hits_150: Option<f64>,
    pub hits_190: Option<f64>,
    pub off_hits_150: Option<f64>,
    pub pass: Option<bool>,
}

/// The S15 rows (`fortify10.s15`).
#[derive(Clone, Debug, Default, Serialize)]
pub struct S15Report {
    pub plate: PlateRow,
    pub riders: RidersRow,
    pub foreign: ForeignRow,
    pub bayonet: BayonetRow,
}

const ROWS: [&str; 4] = ["plate", "riders", "foreign", "bayonet"];

/// Summary lines, one per row.
pub fn summary_text(r: &S15Report) -> String {
    let v = serde_json::to_value(r).unwrap_or_default();
    ROWS.iter()
        .map(|k| format!("  S15 {k}: {}\n", fields(&v[*k])))
        .collect()
}

/// The S15 rows.
pub fn s15_rows(args: &FortifyArgs, seeds: std::ops::Range<u64>) -> Result<S15Report> {
    let seed = seeds.start;
    Ok(S15Report {
        plate: plate_row(args, seed)?,
        riders: riders_row(args, seed)?,
        foreign: foreign_row(args, seed)?,
        bayonet: bayonet_row(args, seed)?,
    })
}

fn weapon(s: &MatchSetup, id: &str) -> Result<u16> {
    s.weapons
        .iter()
        .position(|w| w.id == id)
        .map(|k| k as u16)
        .ok_or_else(|| anyhow::anyhow!("no weapon {id}"))
}

/// The player whose faction id ends with `f`.
fn player_of(s: &MatchSetup, f: &str) -> Result<u8> {
    (0..s.players.len() as u8)
        .find(|&p| super::faction(s, p).ends_with(f))
        .ok_or_else(|| anyhow::anyhow!("no {f} player"))
}

// ------------------------------------------------------------------ W1 --

/// The rows' ground: flat and open (the staged fire meets no hill or
/// wood on its line), as the sim's flat test world.
fn flat(s: &mut MatchSetup) {
    s.map.terrain.octaves.clear();
    s.map.terrain.bands.clear();
    s.map.woods.clear();
}

/// W1 stand-in: every member of every card wears a 2.0 mm torso plate
/// over 120 deg, `[body_armour] enabled` = `on`.
fn plates(s: &mut MatchSetup, on: bool) {
    s.rules.body_armour.enabled = on as u32;
    s.rules.wounds.enabled = 1;
    for t in &mut s.squad_types {
        for m in &mut t.members {
            m.plate = Some(frontline_content::setup::PlateSetup {
                tenths_mm: 20,
                zone: 0,
                arc_deg: 120,
            });
        }
    }
}

/// A lone Grenadier of the German player, held at the row's spot:
/// (world, him, the shooter's player).
fn plate_scene(args: &FortifyArgs, seed: u64, plated: bool) -> Result<(World, SimId, u8)> {
    let mut w = flame_world(args, seed, false, |s| {
        flat(s);
        plates(s, plated)
    })?;
    let s = w.match_setup().unwrap();
    let g = player_of(s, "germany")?;
    let Some(t) = s.squad_type_index("ww2:ger_grenadiergruppe") else {
        bail!("no Grenadiergruppe");
    };
    let c = at(TRENCH_X, LINE_Y + 60);
    let Some(q) = w.spawn_squad_at(PlayerId(g), t, c, EAST) else {
        bail!("the Grenadiergruppe was not spawned");
    };
    w.step(&[]);
    let all = members(&w, q);
    for &m in &all[1..] {
        w.kill_soldier(m);
    }
    w.step(&[]);
    Ok((w, all[0], 1 - g))
}

/// Holds `m` on the row's spot for one tick.
fn hold(w: &mut World, m: SimId) {
    w.place_soldier(m, at(TRENCH_X, LINE_Y + 60));
    w.step(&[]);
}

/// One round of `wi` at `m`'s chest from 50 m where he faces (`back`:
/// from behind), stepped until it lands (he held). Returns whether he is
/// still on his feet.
fn plate_round(w: &mut World, m: SimId, by: u8, wi: u16, back: bool) -> bool {
    hold(w, m);
    let e = w.entities();
    let t = e.transform()[m.index()];
    let h = e.combat()[m.index()].height;
    let wt = &w.match_setup().unwrap().rules.wounds;
    let mid = (wt.legs_bottom_pct as i64 + 100 - wt.head_top_pct as i64) / 2;
    let z = t.pos.z + Fx::from_raw((h.raw() as i64 * mid / 100) as i32);
    let (s, c) = t.yaw.sin_cos();
    let d = Fx::from_int(if back { -50 } else { 50 });
    let from = FxVec3::new(t.pos.x + c.mul(d), t.pos.y + s.mul(d), z);
    w.fire_round_now(by, wi, from, FxVec3::new(t.pos.x, t.pos.y, z));
    for _ in 0..12 {
        hold(w, m);
    }
    let e = w.entities();
    e.kind()[m.index()] == frontline_sim::ecs::EntityKind::Soldier && e.combat()[m.index()].hp > 0
}

/// W1: `PLATE_ROUNDS` MP 40 rounds from the front, his stance, height and
/// suppression put back before each: his player's `plate_stops`.
fn plate_stops(args: &FortifyArgs, seed: u64, plated: bool) -> Result<u32> {
    let (mut w, m, by) = plate_scene(args, seed, plated)?;
    let wi = weapon(w.match_setup().unwrap(), "ww2:mp40")?;
    let fresh = w.entities().combat()[m.index()];
    for _ in 0..PLATE_ROUNDS {
        w.edit_soldier(m, |_, c| *c = fresh);
        plate_round(&mut w, m, by, wi, false);
    }
    let p = w.entities().owner()[m.index()].0 as usize;
    Ok(w.game().unwrap().players()[p].s15.plate_stops)
}

/// W1: rounds of `gun` until he is down (at most `PLATE_ROUNDS`), his
/// stance and suppression put back before each.
fn rounds_to_down(
    args: &FortifyArgs,
    seed: u64,
    plated: bool,
    gun: &str,
    back: bool,
) -> Result<u32> {
    let (mut w, m, by) = plate_scene(args, seed, plated)?;
    let wi = weapon(w.match_setup().unwrap(), gun)?;
    let fresh = w.entities().combat()[m.index()];
    for n in 1..=PLATE_ROUNDS {
        w.edit_soldier(m, |_, c| {
            (c.stance, c.height, c.suppression) = (fresh.stance, fresh.height, 0);
        });
        if !plate_round(&mut w, m, by, wi, back) {
            return Ok(n);
        }
    }
    Ok(PLATE_ROUNDS)
}

/// `fortify10.s15.plate` (W1; module docs).
pub fn plate_row(args: &FortifyArgs, seed: u64) -> Result<PlateRow> {
    let stops = plate_stops(args, seed, true)?;
    let control = plate_stops(args, seed, false)?;
    let rtd = |p: bool, gun: &str, back: bool| rounds_to_down(args, seed, p, gun, back);
    let (plated, bare) = (
        rtd(true, "ww2:mp40", false)?,
        rtd(false, "ww2:mp40", false)?,
    );
    let rifle = rtd(true, "ww2:kar98k", false)?.abs_diff(rtd(false, "ww2:kar98k", false)?);
    let back = rtd(true, "ww2:mp40", true)?.abs_diff(rtd(false, "ww2:mp40", true)?);
    let ratio = plated as f64 * 100.0 / bare.max(1) as f64;
    Ok(PlateRow {
        rounds: Some(PLATE_ROUNDS as f64),
        stops: Some(stops as f64),
        control_stops: Some(control as f64),
        rounds_to_down: Some(plated as f64),
        control_rounds_to_down: Some(bare as f64),
        survive_ratio_pct: Some(ratio),
        rifle_diff: Some(rifle as f64),
        back_diff: Some(back as f64),
        pass: Some(stops > 0 && control == 0 && ratio >= 200.0 && rifle == 0 && back == 0),
    })
}

// ------------------------------------------------------------------ W2 --

/// W2 stand-in: the Panzer IV H gets ten deck rider slots (plan R1), the
/// rules' exit times; `[riders] enabled` = `on` (0: no slots, as the
/// setup build drops them).
fn rider_slots(s: &mut MatchSetup, on: bool) {
    use frontline_content::vehicle::{BoxSetup, FixedVec3, TransportSetup};
    s.rules.riders.enabled = on as u32;
    if !on {
        return;
    }
    let hz = s.tick_hz as u64;
    let ticks = |ms: u32| (ms as u64 * hz).div_ceil(1000) as u32;
    let exit = ticks(s.rules.riders.exit_ms);
    let Some(v) = s.vehicles.iter_mut().find(|v| v.id == "ww2:ger_pz4_h") else {
        return;
    };
    v.transport = Some(TransportSetup {
        entry: FixedVec3::from_cm([-300, 0, 60]),
        board_ticks: ticks(1000),
        exit_ticks: exit,
        exit_over_sides_ticks: Some(exit),
        exit_raise: frontline_content::setup::Fixed(0),
        slots: [-120, -170, -220, -265]
            .into_iter()
            .flat_map(|x| [(x, 60), (x, -60)])
            .chain([(-145, 0), (-245, 0)])
            .map(|(x, y)| BoxSetup {
                center: FixedVec3::from_cm([x, y, 221]),
                half: FixedVec3::from_cm([22, 20, 45]),
            })
            .collect(),
        exposed: true,
    });
}

/// The German player's Panzer IV and a Grenadier section beside it:
/// (world, player, tank, section, the tank's squad).
fn rider_scene(
    args: &FortifyArgs,
    seed: u64,
    on: bool,
) -> Result<(World, u8, SimId, SquadId, SquadId)> {
    let mut w = flame_world(args, seed, false, |s| {
        flat(s);
        rider_slots(s, on)
    })?;
    let s = w.match_setup().unwrap();
    let g = player_of(s, "germany")?;
    let (Some(tk), Some(gr)) = (
        s.squad_type_index("ww2:ger_pz4_h_squad"),
        s.squad_type_index("ww2:ger_grenadiergruppe"),
    ) else {
        bail!("no Panzer IV or Grenadier card");
    };
    let c = at(TRENCH_X - 150, LINE_Y);
    let (Some(t), Some(q)) = (
        w.spawn_squad_at(PlayerId(g), tk, c, EAST),
        w.spawn_squad_at(PlayerId(g), gr, c - at(10, 0), EAST),
    ) else {
        bail!("the rider scene was not spawned");
    };
    for _ in 0..2 {
        w.step(&[]);
    }
    let tank = members(&w, t)[0];
    Ok((w, g, tank, q, t))
}

/// `fortify10.s15.riders` (W2; module docs).
pub fn riders_row(args: &FortifyArgs, seed: u64) -> Result<RidersRow> {
    // The burst 10 m off on the ride.
    let (mut w, g, tank, q, t) = rider_scene(args, seed, true)?;
    if !w.seat_now(q, tank) {
        bail!("the section does not fit the deck");
    }
    w.step(&[]);
    let boarded = w.game().unwrap().riders_of(tank).len();
    let to = w.entities().transform()[tank.index()].pos.xy() + at(300, 0);
    step_with(
        &mut w,
        g,
        vec![CommandKind::Move {
            squad: t,
            target: to,
        }],
    );
    for _ in 0..10 * HZ {
        w.step(&[]);
    }
    let s = w.match_setup().unwrap();
    let mortar = s
        .weapons
        .iter()
        .position(|x| x.class == frontline_content::WeaponClass::Mortar && x.he.is_some())
        .unwrap_or(0) as u16;
    let p = w.entities().transform()[tank.index()].pos.xy();
    w.drop_shell_now(1 - g, mortar, p + at(0, 10));
    let mut off_ms = None;
    for k in 1..=10 * HZ {
        w.step(&[]);
        if w.game().unwrap().riders_of(tank).is_empty() {
            off_ms = Some((k * 1000 / HZ) as f64);
            break;
        }
    }
    let off_ms = off_ms.unwrap_or(10_000.0);
    // The knock-out.
    let (mut w, g, tank, q, _) = rider_scene(args, seed, true)?;
    w.seat_now(q, tank);
    w.step(&[]);
    // The knock-out staged as an engine fire (S3b `burn`): Panzerschreck
    // rockets at the hull stage none in 20 tries on this tank, and a hit
    // alone already empties the deck (R3b).
    let out = |w: &World| {
        let vs = &w.game().unwrap().vehicles()[&tank];
        vs.wreck_tick.is_some() || vs.burn > 0 || vs.ammo_fire > 0
    };
    w.edit_vehicle(tank, |vs| vs.burn = 1);
    w.step(&[]);
    let ko = out(&w);
    let on_deck = w.game().unwrap().riders_of(tank).len();
    let e = w.entities();
    let stunned = members(&w, q)
        .iter()
        .filter(|m| e.combat()[m.index()].hp > 0 && e.weapon()[m.index()].cooldown > 0)
        .count();
    let _ = g;
    // Control: switch 0, Embark boards nobody.
    let (mut w, g, tank, q, _) = rider_scene(args, seed, false)?;
    step_with(
        &mut w,
        g,
        vec![CommandKind::Embark {
            squads: vec![q],
            vehicle: tank,
        }],
    );
    for _ in 0..20 * HZ {
        w.step(&[]);
    }
    let refused = u32::from(w.game().unwrap().riders_of(tank).is_empty());
    let deck = if ko { on_deck as f64 } else { -1.0 };
    Ok(RidersRow {
        boarded: Some(boarded as f64),
        off_ms: Some(off_ms),
        ko_on_deck: Some(deck),
        ko_stunned: Some(stunned as f64),
        control_refused: Some(refused as f64),
        pass: Some(off_ms <= 3000.0 && ko && on_deck == 0 && refused == 1),
    })
}

// ------------------------------------------------------------------ W3 --

/// W3 stand-in: the US 81 mm mortar fires the GrW 34 bomb from German
/// crates (spread 130 %), `[supply] foreign_ammo` 1.
fn foreign_feed(s: &mut MatchSetup) {
    if let Some(sp) = s.rules.supply.as_mut() {
        sp.foreign_ammo = 1;
    }
    let (Some(grw), Some(m1), Some(ger)) = (
        s.weapons.iter().position(|w| w.id == "ww2:grw34"),
        s.weapons.iter().position(|w| w.id == "ww2:m1_mortar"),
        s.factions.iter().position(|f| f.id == "ww2:germany"),
    ) else {
        return;
    };
    if let Some(feed) = s.weapons[grw].feed {
        s.weapons[m1].foreign_feed = Some(frontline_content::setup::ForeignFeedSetup {
            feed,
            faction: ger as u16,
            spread_pct: 130,
        });
    }
}

/// A mortar squad (`card`) of the `faction` player at the row's spot,
/// dry, with `crates` full shells crates of `crate_faction` its player
/// holds 3-5 m off its tube: (world, player, squad, feed slot, tube man).
fn mortar_scene(
    args: &FortifyArgs,
    seed: u64,
    faction: &str,
    card: &str,
    crate_faction: &str,
    crates: i32,
) -> Result<(World, u8, SquadId, usize, SimId)> {
    use frontline_content::setup::{StockKind, SupplyKind};
    let mut w = flame_world(args, seed, false, |s| {
        flat(s);
        foreign_feed(s)
    })?;
    let s = w.match_setup().unwrap();
    let p = player_of(s, faction)?;
    let Some(t) = s.squad_type_index(card) else {
        bail!("no card {card}");
    };
    let ty = &s.squad_types[t as usize];
    let Some(k) = ty.feed_slot(&s.weapons[ty.weapon as usize]) else {
        bail!("{card} has no bomb feed");
    };
    let wi = ty.weapon;
    let c = at(TRENCH_X - 100, LINE_Y - 60);
    let Some(q) = w.spawn_squad_at(PlayerId(p), t, c, EAST) else {
        bail!("the {card} was not spawned");
    };
    for _ in 0..2 * HZ {
        w.step(&[]);
    }
    let all = members(&w, q);
    let Some(&tube) = all
        .iter()
        .find(|m| w.entities().kit()[m.index()].weapon == wi)
    else {
        bail!("no tube man in the {card}");
    };
    for &m in &all {
        w.edit_soldier(m, |kit, cb| {
            kit.rounds = [0; frontline_content::setup::MAX_FEEDS];
            kit.foreign = 0;
            cb.mag = 0;
        });
    }
    let s = w.match_setup().unwrap();
    let f = s
        .factions
        .iter()
        .position(|x| x.id.ends_with(crate_faction))
        .unwrap_or(0) as u16;
    let Some(d) = s.stock_of(f, StockKind::Crate, Some(SupplyKind::Shells)) else {
        bail!("no {crate_faction} shells crate");
    };
    let at_tube = w.entities().transform()[tube.index()].pos.xy();
    for n in 0..crates {
        w.add_crate_now(PlayerId(p), d, at_tube + at(3, n - 1));
    }
    Ok((w, p, q, k, tube))
}

/// Missions of 4 rounds 200 m east whenever the squad is idle with bombs,
/// until it has fired `n` or `secs` pass: (fired, mean miss cm).
fn fire_until(w: &mut World, p: u8, q: SquadId, n: u32, secs: u64) -> (u32, f64) {
    let fm0 = w.game().unwrap().players()[p as usize].fm;
    let fired = |w: &World| {
        let fm = &w.game().unwrap().players()[p as usize].fm;
        fm.bombs.iter().sum::<u32>() - fm0.bombs.iter().sum::<u32>()
    };
    let target = w.entities().transform()[members(w, q)[0].index()].pos.xy() + at(200, 0);
    for _ in 0..secs {
        if fired(w) >= n {
            break;
        }
        let idle = !matches!(
            w.game().unwrap().squad(q).map(|x| x.order.kind()),
            Some(frontline_sim::game::OrderKind::FireMission)
        );
        let g = w.game().unwrap();
        let have = frontline_sim::supply::mortar_bombs_of(g, w.entities(), q);
        if idle && have > 0 {
            step_with(
                w,
                p,
                vec![CommandKind::FireMission {
                    squad: q,
                    target,
                    rounds: 4.min(n - fired(w)).max(1) as u16,
                }],
            );
        }
        for _ in 0..HZ {
            w.step(&[]);
        }
    }
    let fm = &w.game().unwrap().players()[p as usize].fm;
    let miss: u64 = fm.miss_mm.iter().sum::<u64>() - fm0.miss_mm.iter().sum::<u64>();
    let f = fired(w);
    (f, miss as f64 / 10.0 / f.max(1) as f64)
}

/// `fortify10.s15.foreign` (W3; module docs).
pub fn foreign_row(args: &FortifyArgs, seed: u64) -> Result<ForeignRow> {
    const US: &str = "ww2:usa_mortar_team";
    let (mut w, p, q, _, _) = mortar_scene(args, seed, "usa", US, "germany", 3)?;
    let (fired, miss) = fire_until(&mut w, p, q, 20, 300);
    let drawn = w.game().unwrap().players()[p as usize].s15.foreign_drawn;
    let foreign_fired = w.game().unwrap().players()[p as usize].s15.foreign_fired;
    // The same squad with its own bombs (no crate).
    let (mut w, p, q, k, tube) = mortar_scene(args, seed, "usa", US, "germany", 0)?;
    w.edit_soldier(tube, |kit, _| kit.rounds[k] = 40);
    let (own_fired, own_miss) = fire_until(&mut w, p, q, 20, 300);
    // Control: the GrW 34 at a US crate.
    let (mut w, p, q, _, _) = mortar_scene(args, seed, "germany", "ww2:ger_mortar_team", "usa", 1)?;
    for _ in 0..60 * HZ {
        w.step(&[]);
    }
    let g = w.game().unwrap();
    let german = frontline_sim::supply::mortar_bombs_of(g, w.entities(), q)
        + g.players()[p as usize].s15.foreign_drawn;
    let wider = u32::from(fired >= 20 && own_fired >= 20 && miss > own_miss);
    let _ = fired;
    Ok(ForeignRow {
        drawn: Some(drawn as f64),
        fired: Some(foreign_fired as f64),
        own_fired: Some(own_fired as f64),
        miss_cm: Some(miss),
        own_miss_cm: Some(own_miss),
        wider: Some(wider as f64),
        german_drawn: Some(german as f64),
        pass: Some(foreign_fired >= 20 && wider == 1 && german == 0),
    })
}

// ------------------------------------------------------------------ W4 --

/// W4 stand-in: the Kar 98k's bayonet block (reach 140 cm; the S14 butt's
/// other values), `[melee] bayonets` = `on`.
fn bayonet(s: &mut MatchSetup, on: bool) {
    s.rules.melee.enabled = 1;
    s.rules.melee.bayonets = on as u32;
    if let Some(w) = s.weapons.iter_mut().find(|w| w.id == "ww2:kar98k")
        && let Some(m) = w.melee
    {
        w.bayonet = Some(frontline_content::defs::MeleeDef { reach_cm: 140, ..m });
    }
}

/// `n` strikes by the German Kar 98k man at the US man `cm` ahead, both
/// held and put back as fresh before each: hits (and bayonet strikes).
fn bayonet_strikes(args: &FortifyArgs, seed: u64, on: bool, cm: i32) -> Result<(u32, u32)> {
    use frontline_sim::flame::{STRIKE_RECOVER, STRIKE_WINDUP};
    let mut w = flame_world(args, seed, false, |s| {
        flat(s);
        bayonet(s, on)
    })?;
    let mut men = [SimId(0); 2];
    for (k, (f, card, gun)) in [
        ("germany", "ww2:ger_grenadiergruppe", "ww2:kar98k"),
        ("usa", "ww2:usa_rifle_squad", "ww2:m1_garand"),
    ]
    .into_iter()
    .enumerate()
    {
        let s = w.match_setup().unwrap();
        let p = player_of(s, f)?;
        let Some(t) = s.squad_type_index(card) else {
            bail!("no card {card}");
        };
        let Some(q) = w.spawn_squad_at(PlayerId(p), t, at(TRENCH_X + 40 * k as i32, LINE_Y), EAST)
        else {
            bail!("the {card} was not spawned");
        };
        let all = members(&w, q);
        let s = w.match_setup().unwrap();
        let Some(&man) = all
            .iter()
            .find(|m| s.weapons[w.entities().kit()[m.index()].weapon as usize].id == gun)
        else {
            bail!("no {gun} man");
        };
        for &m in &all {
            if m != man {
                w.kill_soldier(m);
            }
        }
        men[k] = man;
    }
    w.step(&[]);
    let [a, b] = men;
    let (fa, fb) = (
        w.entities().combat()[a.index()],
        w.entities().combat()[b.index()],
    );
    // His kit as fresh too; the strikes 100 m from the spawn, so no
    // fallen squadmate's crew weapon is within his pickup reach (S4).
    let ka = w.entities().kit()[a.index()];
    let pa = w.entities().owner()[a.index()].0 as usize;
    // One warm-up strike first (the first tick after the spawn), not
    // counted.
    let c = at(TRENCH_X, LINE_Y + 100);
    let t = FxVec2::new(c.x + Fx::from_int(cm).div_int(100), c.y);
    let mut base = (0, 0);
    for k in 0..=BAYONET_STRIKES {
        if k == 1 {
            let pl = &w.game().unwrap().players()[pa];
            base = (pl.s14.strike_hits, pl.s15.bayonet_strikes);
        }
        w.place_soldier(a, c);
        w.place_soldier(b, t);
        w.edit_soldier(b, |_, cb| {
            *cb = fb;
            cb.close.strike_phase = STRIKE_RECOVER;
            cb.close.strike = 200;
            cb.close.strike_at = FxVec2::new(c.x - Fx::from_int(5), c.y);
        });
        w.edit_soldier(a, |kit, cb| {
            *kit = ka;
            *cb = fa;
            cb.close.strike_phase = STRIKE_WINDUP;
            cb.close.strike = 1;
            cb.close.strike_at = FxVec2::new(c.x + Fx::from_int(3), c.y);
        });
        w.step(&[]);
    }
    let pl = &w.game().unwrap().players()[pa];
    Ok((pl.s14.strike_hits - base.0, pl.s15.bayonet_strikes - base.1))
}

/// `fortify10.s15.bayonet` (W4; module docs).
pub fn bayonet_row(args: &FortifyArgs, seed: u64) -> Result<BayonetRow> {
    let (h150, strikes) = bayonet_strikes(args, seed, true, 150)?;
    let (h190, _) = bayonet_strikes(args, seed, true, 190)?;
    let (off, _) = bayonet_strikes(args, seed, false, 150)?;
    Ok(BayonetRow {
        strikes: Some(strikes as f64),
        hits_150: Some(h150 as f64),
        hits_190: Some(h190 as f64),
        off_hits_150: Some(off as f64),
        pass: Some(strikes == BAYONET_STRIKES && h150 == BAYONET_STRIKES && h190 == 0 && off == 0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// W1-W4 on one seed: every row passes (the hard verdicts of
    /// `targets/S15.toml` W1a-W4c).
    #[test]
    fn s15_rows_pass() {
        let r = s15_rows(&FortifyArgs::default(), 1..2).unwrap();
        eprintln!("{}", serde_json::to_string(&r).unwrap());
        assert_eq!(r.plate.pass, Some(true), "{:?}", r.plate);
        assert_eq!(r.riders.pass, Some(true), "{:?}", r.riders);
        assert_eq!(r.foreign.pass, Some(true), "{:?}", r.foreign);
        assert_eq!(r.bayonet.pass, Some(true), "{:?}", r.bayonet);
    }
}
