//! S4 Part B: members in the sim (per-man weapons, crew-weapon pickup),
//! the Panzerfaust (single-shot AT item), training profiles and the
//! call-in card limits (`docs/slices/S4-variants.md` B3-B5, B7; targets
//! T10, T12, T14, T15).

use std::path::Path;

use frontline_content::{Difficulty, ItemKind, MatchRequest, MatchSetup, ModDefs, SeatRole};
use frontline_sim::{
    CallInRefusal, Command, CommandKind, Fx, FxVec2, PlayerId, SimId, SquadId, World,
};

/// Flat open ground with the test unlock, both sides human.
fn setup() -> MatchSetup {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2");
    let mut defs = ModDefs::load(&root).unwrap_or_else(|e| panic!("{e}"));
    let rules = defs
        .add_rules_override(&root.join("tests/rules_test_unlock.toml"))
        .unwrap_or_else(|e| panic!("{e}"));
    let mut req = MatchRequest::skirmish(1, "ww2:germany", "ww2:usa", Difficulty::Normal);
    req.rules = rules;
    let mut s = defs.resolve(&req).unwrap_or_else(|e| panic!("{e}"));
    s.players.iter_mut().for_each(|p| p.is_ai = false);
    s.rules.start_manpower = 100_000;
    s.map.terrain.octaves.clear();
    s.map.terrain.bands.clear();
    s.map.woods.clear();
    s.rules.vehicles.bail.crew_below = 0;
    s
}

fn xy(x: i32, y: i32) -> FxVec2 {
    FxVec2::from_int(x, y)
}

const EAST: FxVec2 = FxVec2::new(Fx::ONE, Fx::ZERO);
const WEST: FxVec2 = FxVec2::new(Fx::NEG_ONE, Fx::ZERO);
const NORTH: FxVec2 = FxVec2::new(Fx::ZERO, Fx::ONE);

fn ty(w: &World, id: &str) -> u16 {
    w.match_setup().unwrap().squad_type_index(id).unwrap()
}

fn spawn(w: &mut World, player: u8, id: &str, at: FxVec2, f: FxVec2) -> SquadId {
    let t = ty(w, id);
    w.spawn_squad_at(PlayerId(player), t, at, f).unwrap()
}

fn members(w: &World, sq: SquadId) -> Vec<SimId> {
    w.game().unwrap().squad(sq).unwrap().members.clone()
}

fn weapon_id(w: &World, man: SimId) -> String {
    let k = w.entities().kit()[man.index()].weapon;
    w.match_setup().unwrap().weapons[k as usize].id.clone()
}

/// A Sherman whose crew is only its driver: it neither fires nor moves.
fn harmless_sherman(w: &mut World, at: FxVec2, f: FxVec2) -> SimId {
    let sq = spawn(w, 1, "ww2:usa_m4a3_75_squad", at, f);
    let v = members(w, sq)[0];
    let s = w.match_setup().unwrap().clone();
    let vt = &s.vehicles[w.game().unwrap().vehicle(v).unwrap().type_idx as usize];
    let roles: Vec<SeatRole> = vt.seats.iter().map(|s| s.role).collect();
    w.edit_vehicle(v, |vs| {
        for (seat, role) in vs.seats.iter_mut().zip(&roles) {
            if *role != SeatRole::Driver {
                seat.hp = 0;
            }
        }
    });
    v
}

fn pzf_left(w: &World, sq: SquadId) -> u32 {
    let k = ItemKind::SingleShotAt.index();
    members(w, sq)
        .iter()
        .map(|m| w.entities().kit()[m.index()].items[k] as u32)
        .sum()
}

// ------------------------------------------------------------ members --

#[test]
fn men_spawn_by_slot_with_their_own_weapons_and_kit() {
    let mut w = World::new_match(setup(), 1).unwrap();
    let sq = spawn(&mut w, 0, "ww2:ger_grenadiergruppe", xy(300, 500), NORTH);
    let men = members(&w, sq);
    assert_eq!(men.len(), 10);
    let weapons: Vec<String> = men.iter().map(|&m| weapon_id(&w, m)).collect();
    assert_eq!(weapons[0], "ww2:mp40");
    assert_eq!(weapons[1], "ww2:mg42_bipod");
    assert_eq!(weapons[2], "ww2:p38");
    assert!(weapons[3..].iter().all(|x| x == "ww2:kar98k"));
    // Two Panzerfausts (the two `rifleman_pzf` men), the leader's two
    // grenades and a smoke.
    assert_eq!(pzf_left(&w, sq), 2);
    let k = w.entities().kit();
    assert_eq!(k[men[0].index()].items[ItemKind::Frag.index()], 2);
    assert_eq!(k[men[0].index()].items[ItemKind::Smoke.index()], 1);
    assert_eq!(k[men[4].index()].items[ItemKind::SingleShotAt.index()], 1);
    assert!(w.game().unwrap().squad(sq).unwrap().pzf.is_some());
    // A US rifle squad of twelve with one BAR.
    let us = spawn(&mut w, 1, "ww2:usa_rifle_squad", xy(300, 600), NORTH);
    let men = members(&w, us);
    assert_eq!(men.len(), 12);
    let bars = men
        .iter()
        .filter(|&&m| weapon_id(&w, m) == "ww2:bar_m1918a2")
        .count();
    assert_eq!(bars, 1);
    assert!(w.game().unwrap().squad(us).unwrap().pzf.is_none());
}

#[test]
fn the_crew_assistant_picks_up_the_mg_after_the_hand_over_and_fires_it() {
    let mut w = World::new_match(setup(), 2).unwrap();
    let sq = spawn(&mut w, 0, "ww2:ger_grenadiergruppe", xy(300, 500), EAST);
    let men = members(&w, sq);
    let (gunner, assistant) = (men[1], men[2]);
    let handover = {
        let s = w.match_setup().unwrap();
        s.weapons
            .iter()
            .find(|x| x.id == "ww2:mg42_bipod")
            .unwrap()
            .handover_ticks as u64
    };
    assert_eq!(handover, 90, "3 s at 30 Hz");
    // A target for later: US riflemen 400 m east, out of sight (150 m).
    let us = spawn(&mut w, 1, "ww2:usa_rifle_squad", xy(700, 500), WEST);
    w.kill_soldier(gunner);
    let drops = w.game().unwrap().squad(sq).unwrap().drops.clone();
    assert_eq!(drops.len(), 1);
    // A save in the middle of the hand-over loads and continues alike.
    w.step(&[]);
    let mut loaded = World::load(&w.save(), 3).unwrap();
    assert_eq!(loaded.state_hash(), w.state_hash());
    let t0 = w.tick();
    while weapon_id(&w, assistant) != "ww2:mg42_bipod" {
        w.step(&[]);
        assert!(w.tick() - t0 <= handover, "no pickup after the hand-over");
    }
    assert_eq!(w.tick() - t0, handover - 1);
    // Nobody else picked anything up, and no man of the other squad.
    let picked: Vec<SimId> = members(&w, sq)
        .into_iter()
        .filter(|&m| weapon_id(&w, m) == "ww2:mg42_bipod")
        .collect();
    assert_eq!(picked, [assistant]);
    assert!(
        members(&w, us)
            .iter()
            .all(|&m| weapon_id(&w, m) != "ww2:mg42_bipod")
    );
    assert_eq!(w.game().unwrap().players()[0].s4.pickups, 1);
    while loaded.tick() < w.tick() {
        loaded.step(&[]);
    }
    assert_eq!(loaded.state_hash(), w.state_hash());
    // Move the enemy into sight: the assistant fires the MG.
    let shots0 = w.entities().weapon()[assistant.index()].shots;
    assert!(w.place_squad(us, xy(440, 500), WEST));
    for _ in 0..30 * 20 {
        w.step(&[]);
    }
    assert!(w.entities().weapon()[assistant.index()].shots > shots0);
}

#[test]
fn a_tank_crews_smg_stays_with_its_man() {
    let mut w = World::new_match(setup(), 1).unwrap();
    let crew = spawn(&mut w, 0, "ww2:ger_tank_crew", xy(300, 500), NORTH);
    let men = members(&w, crew);
    assert_eq!(weapon_id(&w, men[0]), "ww2:mp40");
    w.kill_soldier(men[0]);
    for _ in 0..200 {
        w.step(&[]);
    }
    // S4-2 (A): no silent pass of the MP 40 to the next man.
    assert!(
        members(&w, crew)
            .iter()
            .all(|&m| weapon_id(&w, m) == "ww2:p38")
    );
}

// --------------------------------------------------------- panzerfaust --

/// A German line squad and a harmless Sherman `d` m apart; steps `ticks`
/// and returns (Panzerfausts fired, left in the squad, rockets fired).
fn pzf_duel(d: i32, ticks: u32, threads: usize) -> (u32, u32, u64, World) {
    let mut w = World::new_match(setup(), threads).unwrap();
    let sq = spawn(&mut w, 0, "ww2:ger_grenadiergruppe", xy(300, 500), EAST);
    let _v = harmless_sherman(&mut w, xy(300 + d, 500), NORTH);
    for _ in 0..ticks {
        w.step(&[]);
    }
    let fired = w.game().unwrap().players()[0].s4.single_shots;
    let left = pzf_left(&w, sq);
    let rockets = w.stats().rockets_fired;
    (fired, left, rockets, w)
}

#[test]
fn a_line_squad_fires_its_panzerfausts_at_a_tank_in_reach() {
    let (fired, left, rockets, w) = pzf_duel(40, 30 * 30, 1);
    assert!(fired >= 1, "no Panzerfaust fired at 40 m");
    assert_eq!(fired + left, 2);
    assert_eq!(rockets, fired as u64);
    // One per squad per `items.single_shot_gap_ticks`: never two in a
    // gap (S4 review L4: checked tick by tick).
    let gap = w.match_setup().unwrap().rules.items.single_shot_gap_ticks;
    assert_eq!(gap, 90);
    let mut w = World::new_match(setup(), 1).unwrap();
    let _sq = spawn(&mut w, 0, "ww2:ger_grenadiergruppe", xy(300, 500), EAST);
    let _v = harmless_sherman(&mut w, xy(340, 500), NORTH);
    let mut shot_ticks = Vec::new();
    let mut seen = 0;
    for _ in 0..30 * 30 {
        w.step(&[]);
        let n = w.game().unwrap().players()[0].s4.single_shots;
        if n > seen {
            assert_eq!(n, seen + 1, "two Panzerfausts in one tick");
            shot_ticks.push(w.tick());
            seen = n;
        }
    }
    assert!(!shot_ticks.is_empty());
    assert!(
        shot_ticks.windows(2).all(|t| t[1] - t[0] >= gap as u64),
        "two Panzerfausts inside one gap: {shot_ticks:?}"
    );
    // At 90 m (beyond engage_still 60 m) nobody fires.
    let (fired, left, _, _) = pzf_duel(90, 30 * 20, 1);
    assert_eq!((fired, left), (0, 2));
}

#[test]
fn panzerfaust_fire_is_the_same_on_1_2_3_and_8_threads() {
    let hashes: Vec<u64> = [1, 2, 3, 8]
        .iter()
        .map(|&t| pzf_duel(50, 30 * 20, t).3.state_hash())
        .collect();
    assert!(hashes.windows(2).all(|h| h[0] == h[1]), "{hashes:x?}");
}

// ------------------------------------------------------------ training --

/// Hits and pinned US men after `secs` of a German Grenadiergruppe of
/// `profile` and a regular US rifle squad 140 m apart (inside the 150 m
/// sight).
fn training_fire(profile: &str, secs: u32) -> (u64, u32) {
    let mut s = setup();
    let t = s
        .training
        .iter()
        .position(|x| x.id == profile)
        .expect("profile") as u16;
    let g = s.squad_type_index("ww2:ger_grenadiergruppe").unwrap() as usize;
    s.squad_types[g].training = t;
    let mut w = World::new_match(s, 1).unwrap();
    let _ger = spawn(&mut w, 0, "ww2:ger_grenadiergruppe", xy(300, 500), EAST);
    let _us = spawn(&mut w, 1, "ww2:usa_rifle_squad", xy(440, 500), WEST);
    for _ in 0..30 * secs {
        w.step(&[]);
    }
    // Germans killed or hurt count against them: the US losses alone.
    let hits = w.game().unwrap().players()[1].losses as u64;
    let pinned = w
        .entities()
        .combat()
        .iter()
        .enumerate()
        .filter(|(i, c)| w.entities().owner()[*i] == PlayerId(1) && c.pinned)
        .count() as u32;
    (hits, pinned)
}

#[test]
fn training_profiles_change_the_men_not_the_weapon() {
    let (conscript, _) = training_fire("ww2:training_conscript", 60);
    let (elite, _) = training_fire("ww2:training_elite", 60);
    let (regular, _) = training_fire("ww2:training_regular", 60);
    eprintln!(
        "US losses in 60 s at 140 m: conscript {conscript}, regular {regular}, elite {elite}"
    );
    assert!(elite > conscript, "elite {elite} <= conscript {conscript}");
}

// ------------------------------------------------------------- call-in --

fn call(w: &mut World, p: u8, seq: u32, t: u16) -> Result<SquadId, CallInRefusal> {
    w.step(&[Command {
        player: PlayerId(p),
        seq,
        kind: CommandKind::CallIn { squad_type: t },
    }]);
    w.snapshot().call_ins[0].result
}

#[test]
fn a_call_in_spawns_at_once_and_each_card_has_its_own_cooldown() {
    let mut s = setup();
    let full = s.squad_type_index("ww2:ger_grenadiergruppe").unwrap();
    let field = s.squad_type_index("ww2:ger_grenadiergruppe_7").unwrap();
    // Cap the field-strength card at one alive.
    s.squad_types[field as usize].max_alive = 1;
    let cd = s.squad_types[full as usize].recall_cooldown_ticks;
    let mut w = World::new_match(s, 1).unwrap();
    let before = w.game().unwrap().squads().len();
    let a = call(&mut w, 0, 0, full).expect("spawns");
    assert_eq!(w.game().unwrap().squads().len(), before + 1);
    assert_eq!(members(&w, a).len(), 10);
    // The same card again: refused for exactly its cooldown.
    assert_eq!(
        call(&mut w, 0, 1, full),
        Err(CallInRefusal::Cooldown { ticks: cd - 1 })
    );
    // Its sibling card (no cooldown_group, Q21 (B)) at once.
    let b = call(&mut w, 0, 2, field).expect("sibling spawns at once");
    // max_alive 1: a second one is refused until the first is gone.
    let cdf = w.match_setup().unwrap().squad_types[field as usize].recall_cooldown_ticks;
    for _ in 0..cdf {
        w.step(&[]);
    }
    assert_eq!(
        call(&mut w, 0, 3, field),
        Err(CallInRefusal::MaxAlive { max: 1 })
    );
    for m in members(&w, b) {
        w.kill_soldier(m);
    }
    w.step(&[]);
    assert!(call(&mut w, 0, 4, field).is_ok());
}

#[test]
fn a_limit_group_counts_every_card_in_it() {
    let mut s = setup();
    s.rules
        .limit_groups
        .push(frontline_content::setup::LimitGroupSetup {
            id: "ww2:line".into(),
            limit: 3,
        });
    let full = s.squad_type_index("ww2:ger_grenadiergruppe").unwrap();
    let field = s.squad_type_index("ww2:ger_grenadiergruppe_7").unwrap();
    for t in [full, field] {
        s.squad_types[t as usize].limit_group = Some(0);
        s.squad_types[t as usize].recall_cooldown_ticks = 0;
    }
    let mut w = World::new_match(s, 1).unwrap();
    // The two start squads are Grenadiergruppen: one more fits.
    assert!(call(&mut w, 0, 0, field).is_ok());
    assert_eq!(
        call(&mut w, 0, 1, full),
        Err(CallInRefusal::GroupLimit { group: 0 })
    );
    assert_eq!(
        call(&mut w, 0, 2, field),
        Err(CallInRefusal::GroupLimit { group: 0 })
    );
}

/// S4 review M1: a crew card whose men carry a Panzerfaust (legal content)
/// bails out with its single-shot state, and saves taken with that crew on
/// the map load and continue on the same hash.
#[test]
fn a_crew_with_a_panzerfaust_bails_out_and_its_saves_load() {
    use frontline_sim::scenarios::{cmd, tank};
    let mut s = setup();
    let pzf = s
        .items
        .iter()
        .position(|it| it.kind == ItemKind::SingleShotAt)
        .unwrap() as u16;
    let crew = s.squad_type_index("ww2:ger_tank_crew").unwrap() as usize;
    let k = ItemKind::SingleShotAt.index();
    s.squad_types[crew].members[0].items[k] = 1;
    s.squad_types[crew].items[k] = Some(pzf);
    let mut w = World::new_match(s, 1).unwrap();
    let (sq, v) = tank(&mut w, 0, xy(500, 500), EAST).unwrap();
    w.step(&[cmd(0, 0, CommandKind::Abandon { squad: sq })]);
    for _ in 0..30 * 6 {
        w.step(&[]);
    }
    let cq = w
        .game()
        .unwrap()
        .vehicle(v)
        .unwrap()
        .crew_squad
        .expect("a crew squad");
    let c = w.game().unwrap().squad(cq).unwrap();
    assert!(!c.members.is_empty());
    assert!(c.pzf.is_some(), "the crew squad has single-shot state");
    let loaded = World::load(&w.save(), 2).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(loaded.state_hash(), w.state_hash());
    let mut a = w;
    let mut b = loaded;
    for _ in 0..300 {
        a.step(&[]);
        b.step(&[]);
    }
    assert_eq!(a.state_hash(), b.state_hash());
}
