//! S12a Build A tests (docs/slices/S12-wounds-medics.md 3, 4, 6): the
//! SAVE 30 layout and its load checks, then the wound rules W1-W5,
//! W12-W14 (every test turns `[wounds] enabled` on in the test only). Each
//! test names its control: the edit that makes it fail.

use super::s6_tests::{flat_world_with, members, spawn};
use super::*;
use crate::ecs::EntityKind;
use crate::wounds::{Casualty, DOWNED, WOUNDED, Wound};

const RIFLES: &str = "ww2:ger_grenadiergruppe";
const US_RIFLES: &str = "ww2:usa_rifle_squad";

fn c(x: i32, y: i32) -> FxVec2 {
    FxVec2::from_int(x, y)
}

fn game_mut(s: &mut SimState) -> &mut MatchState {
    s.game.as_mut().unwrap()
}

fn wd(s: &mut SimState, id: SimId) -> &mut Wound {
    &mut s.entities.combat[id.index()].wound
}

fn beds_mut(s: &mut SimState, id: u32) -> &mut Vec<crate::wounds::Bed> {
    &mut game_mut(s).supply6.structures.get_mut(&id).unwrap().beds
}

fn cas(s: &mut SimState, id: SimId) -> &mut Casualty {
    s.game.as_mut().unwrap().casualties.get_mut(&id).unwrap()
}

/// A flat match with `[wounds] enabled` = `on`.
pub(super) fn wounds_world(on: u32) -> World {
    flat_world_with(1, |s| s.rules.wounds.enabled = on)
}

/// Turns soldier `id` into a downed man with a casualty bleeding out at
/// `bleed_out_at` (the layout staging; the W5 rule does it in play).
pub(super) fn stage_downed(w: &mut World, id: SimId, bleed_out_at: u64) {
    let tick = w.tick();
    let sq = w.state.entities.squad[id.index()].unwrap();
    let card = w.game().unwrap().squads[&sq].type_idx;
    {
        let g = w.state.game.as_mut().unwrap();
        g.kill(&mut w.state.entities, &mut w.hash, id, 30);
        g.players[w.state.entities.owner[id.index()].0 as usize].losses -= 1;
        g.casualties.insert(
            id,
            Casualty {
                from_squad: sq,
                card,
                bleed_out_at: bleed_out_at.max(tick + 1),
                ..Casualty::default()
            },
        );
    }
    let e = &mut w.state.entities;
    e.kind[id.index()] = EntityKind::Downed;
    let cb = &mut e.combat[id.index()];
    cb.corpse = 0;
    cb.hp = 1;
    cb.wound = Wound {
        state: DOWNED,
        by: (1, crate::game::kill_class::RIFLE),
        ..Wound::default()
    };
    w.hash.mark_entity_chunk(id.chunk());
    w.refresh_hashes();
}

/// The S12 staged save: a German rifle squad with a wounded man
/// mid-bandage and a downed man, a few S12 counters and AI counters.
/// Returns `(world, squad, bandaging man, downed man)`.
fn staged() -> (World, SquadId, SimId, SimId) {
    let mut w = wounds_world(1);
    let sq = spawn(&mut w, 0, RIFLES, 300, 300);
    spawn(&mut w, 1, US_RIFLES, 700, 700);
    super::s6_tests::steps(&mut w, 3);
    let m = members(&w, sq);
    let (band, down) = (m[1], m[2]);
    assert!(w.edit_soldier(band, |_, c| {
        c.hp = 30;
        c.wound = Wound {
            state: WOUNDED,
            legs: true,
            paid: true,
            bleed_acc_ms: 1200,
            bandaging: 150,
            by: (1, crate::game::kill_class::MG),
            ..Wound::default()
        };
    }));
    let t = w.tick();
    stage_downed(&mut w, down, t + 90 * 30);
    assert!(w.edit_player(PlayerId(0), |p| {
        p.s12.downed = 1;
        p.s12.head_hits = [2, 1];
        p.s12.leg_hits[1] = 3;
    }));
    assert!(w.edit_player(PlayerId(1), |p| {
        p.s12.downed_by_class[crate::game::kill_class::MG as usize] = 1;
    }));
    (w, sq, band, down)
}

/// Spec 3: the S12 state saves, loads and hashes. Control: an unedited
/// reload keeps the hash (each edit must change it).
#[test]
fn s12_layout_roundtrips_and_hashes() {
    let (w, _sq, band, down) = staged();
    let bytes = w.save();
    let back = World::load(&bytes, 1).expect("the S12 state loads");
    assert_eq!(back.state(), w.state());
    assert_eq!(back.full_state_hash(), w.full_state_hash());
    assert_eq!(back.save(), bytes);
    let h = back.full_state_hash();
    type Edit = Box<dyn Fn(&mut SimState)>;
    let edits: Vec<Edit> = vec![
        Box::new(move |s| wd(s, band).bandaging -= 1),
        Box::new(move |s| wd(s, band).bleed_acc_ms += 1),
        Box::new(move |s| wd(s, band).legs = false),
        Box::new(move |s| wd(s, band).paid = false),
        Box::new(move |s| wd(s, band).treated = true),
        Box::new(move |s| wd(s, band).by.1 = 0),
        Box::new(move |s| wd(s, down).by.0 = 0),
        Box::new(move |s| game_mut(s).casualties.get_mut(&down).unwrap().bleed_out_at += 1),
        Box::new(move |s| game_mut(s).casualties.get_mut(&down).unwrap().card += 1),
        Box::new(|s| game_mut(s).players[0].s12.downed += 1),
        Box::new(|s| game_mut(s).players[0].s12.torso_hits[0] += 1),
        Box::new(|s| game_mut(s).players[1].s12.downed_by_class[0] += 1),
        Box::new(|s| game_mut(s).players[1].s12.finished_by_class[2] += 1),
        Box::new(|s| game_mut(s).players[1].s12.refund_mp += 1),
        Box::new(move |s| cas(s, down).tend = Some((SimId(0), u64::MAX))),
    ];
    assert_eq!(World::load(&back.save(), 1).unwrap().full_state_hash(), h);
    for (k, e) in edits.into_iter().enumerate() {
        let mut x = World::load(&bytes, 1).unwrap();
        e(&mut x.state);
        // Hashed in memory: some edits do not load (the last one).
        assert_ne!(x.full_state_hash(), h, "edit {k} is not hashed");
    }
}

/// Spec 3 load validation, one case per check. Control: the staged save
/// itself loads (checked first), so each failure is the edit's.
#[test]
fn load_rejects_bad_s12_state() {
    let (mut w, sq, band, down) = staged();
    let hq_def = super::s6_tests::def(&w, 0, frontline_content::setup::StockKind::Hq, None);
    let hq = super::s6_tests::add_built(&mut w, 0, hq_def, c(250, 250), [0; 3]);
    let bytes = w.save();
    World::load(&bytes, 1).expect("control: the staged save loads");
    let tick = w.tick();
    let other = members(&w, sq)[0];
    // Checker F7: an enemy soldier, and an aid post (accepted since Build B)
    // holding one valid bed.
    let enemy = (0..w.state.entities.kind.len())
        .map(|i| SimId(i as u32))
        .find(|&m| {
            kind(&w, m) == EntityKind::Soldier && w.state.entities.owner[m.index()] == PlayerId(1)
        })
        .unwrap();
    let aid_def = super::s6_tests::def(&w, 0, frontline_content::setup::StockKind::Aid, None);
    let aid = super::s6_tests::add_built(&mut w, 0, aid_def, c(320, 320), [0; 3]);
    let bed = |man: SimId, done_at: u64| crate::wounds::Bed {
        man,
        done_at,
        evac: false,
    };
    game_mut(&mut w.state)
        .supply6
        .structures
        .get_mut(&aid)
        .unwrap()
        .beds
        .push(bed(band, tick + 60));
    w.refresh_hashes();
    let bytes = w.save();
    World::load(&bytes, 1).expect("control: an aid post with a bed loads");
    type Edit = Box<dyn Fn(&mut SimState)>;
    let cases: Vec<(&str, Edit)> = vec![
        (
            "F7: more casualties than the cap",
            Box::new(move |s| {
                let c0 = cas(s, down).clone();
                for k in 0..=crate::wounds::MAX_CASUALTIES as u32 {
                    game_mut(s)
                        .casualties
                        .insert(SimId(100_000 + k), c0.clone());
                }
            }),
        ),
        (
            "F7: a downed man in a squad",
            Box::new(move |s| s.entities.squad[down.index()] = Some(sq)),
        ),
        (
            "F7: a downed man with a corpse timer",
            Box::new(move |s| s.entities.combat[down.index()].corpse = 30),
        ),
        (
            "F7: a fit man with a leg wound",
            Box::new(move |s| wd(s, other).legs = true),
        ),
        (
            "F7: an enemy medic tends him",
            Box::new(move |s| cas(s, down).tend = Some((enemy, tick + 30))),
        ),
        (
            "F7: a tend past MAX_TICK",
            Box::new(move |s| cas(s, down).tend = Some((other, crate::bounds::MAX_TICK + 1))),
        ),
        (
            "F7: an enemy carrier",
            Box::new(move |s| cas(s, down).carriers = [Some(other), Some(enemy)]),
        ),
        (
            "F7: evacuation past MAX_TICK",
            Box::new(move |s| cas(s, down).evac_at = crate::bounds::MAX_TICK + 1),
        ),
        (
            "F7: destination beyond the map size",
            Box::new(move |s| cas(s, down).dest = Some(c(100_000, 10))),
        ),
        (
            "F7: more beds than MAX_BEDS",
            Box::new(move |s| {
                let b = beds_mut(s, aid);
                while b.len() <= crate::wounds::MAX_BEDS {
                    b.push(bed(band, tick + 60));
                }
            }),
        ),
        (
            "F7: a bed holding an enemy",
            Box::new(move |s| beds_mut(s, aid)[0].man = enemy),
        ),
        (
            "F7: a bed holding nobody",
            Box::new(move |s| beds_mut(s, aid)[0].man = SimId(99_999)),
        ),
        (
            "F7: a bed done past MAX_TICK",
            Box::new(move |s| beds_mut(s, aid)[0].done_at = crate::bounds::MAX_TICK + 1),
        ),
        (
            "wound state past Downed",
            Box::new(move |s| wd(s, band).state = 3),
        ),
        (
            "a soldier in state Downed",
            Box::new(move |s| wd(s, band).state = DOWNED),
        ),
        (
            "a downed man not in state Downed",
            Box::new(move |s| wd(s, down).state = WOUNDED),
        ),
        (
            "a fit man bandaged",
            Box::new(move |s| wd(s, other).bandaged = true),
        ),
        (
            "a fit man bandaging",
            Box::new(move |s| wd(s, other).bandaging = 5),
        ),
        (
            "bandaging past bandage_s",
            Box::new(move |s| wd(s, band).bandaging = 10 * 30 + 1),
        ),
        (
            "bandaged and bandaging",
            Box::new(move |s| wd(s, band).bandaged = true),
        ),
        (
            "hitter names no player",
            Box::new(move |s| wd(s, band).by.0 = 2),
        ),
        (
            "hitter class unknown",
            // S12b Build C: the class's low nibble (a vehicle type may sit
            // above it, as in every kill class).
            Box::new(move |s| wd(s, band).by.1 = crate::game::kill_class::COUNT as u16),
        ),
        (
            "a wound on a corpse",
            Box::new(move |s| {
                s.entities.kind[down.index()] = EntityKind::Corpse;
                s.entities.combat[down.index()].corpse = 5;
                s.game.as_mut().unwrap().casualties.clear();
            }),
        ),
        (
            "a downed man at 0 HP",
            Box::new(move |s| s.entities.combat[down.index()].hp = 0),
        ),
        (
            "a downed man with no casualty",
            Box::new(|s| s.game.as_mut().unwrap().casualties.clear()),
        ),
        (
            "a casualty that is a soldier",
            Box::new(move |s| {
                let c0 = cas(s, down).clone();
                s.game.as_mut().unwrap().casualties.insert(other, c0);
            }),
        ),
        ("card unknown", Box::new(move |s| cas(s, down).card = 9999)),
        (
            "bleed-out in the past",
            Box::new(move |s| cas(s, down).bleed_out_at = tick - 1),
        ),
        (
            "bleed-out far ahead",
            Box::new(move |s| cas(s, down).bleed_out_at = u64::MAX),
        ),
        (
            "a medic that is an enemy or gone",
            Box::new(move |s| cas(s, down).tend = Some((SimId(9999), tick + 30))),
        ),
        (
            "a tend that ended",
            Box::new(move |s| cas(s, down).tend = Some((other, tick - 1))),
        ),
        (
            "the same carrier twice",
            Box::new(move |s| cas(s, down).carriers = [Some(other), Some(other)]),
        ),
        (
            "a carrier that is downed",
            Box::new(move |s| cas(s, down).carriers[0] = Some(down)),
        ),
        (
            "destination off the map",
            Box::new(move |s| cas(s, down).dest = Some(c(-5, 10))),
        ),
        (
            "evacuation in the past",
            Box::new(move |s| cas(s, down).evac_at = tick - 1),
        ),
        (
            "beds on a structure that is not an aid post",
            Box::new(move |s| {
                let g = s.game.as_mut().unwrap();
                let st = g.supply6.structures.get_mut(&hq).unwrap();
                st.beds.push(crate::wounds::Bed {
                    man: down,
                    done_at: 0,
                    evac: false,
                });
            }),
        ),
        (
            "s12 counter out of range",
            Box::new(|s| game_mut(s).players[0].s12.bled_out = u32::MAX),
        ),
        (
            "s12 class counter out of range",
            Box::new(|s| game_mut(s).players[0].s12.downed_by_class[3] = u32::MAX),
        ),
    ];
    for (name, e) in cases {
        let mut x = World::load(&bytes, 1).unwrap();
        e(&mut x.state);
        assert!(World::load(&x.save(), 1).is_err(), "{name}: loaded");
    }
    // Checker F1: an event due on the tick the save resumes at is legal.
    let mut x = World::load(&bytes, 1).unwrap();
    let c0 = cas(&mut x.state, down);
    c0.bleed_out_at = tick;
    c0.tend = Some((other, tick));
    c0.evac_at = tick;
    World::load(&x.save(), 1).expect("F1: events due at the save's tick load");
}

// ------------------------------------------------------------ rules --

use super::s6_tests::steps;
use super::s11a2_tests::{burst_now, hp, lone, pose, weapon};
use crate::FxVec3;
use crate::combat::{BallisticEvent, Blow, DamageCtx};
use crate::ecs::Stance;
use crate::game::kill_class;
use crate::projectile::NO_AMMO;
use crate::wounds::{FIT, ZONE_HEAD, ZONE_LEGS, ZONE_TORSO};

fn kind(w: &World, m: SimId) -> EntityKind {
    w.state.entities.kind[m.index()]
}

fn wound(w: &World, m: SimId) -> Wound {
    w.state.entities.combat[m.index()].wound
}

fn s12(w: &World, p: usize) -> crate::wounds::S12Counters {
    w.game().unwrap().players[p].s12
}

fn set_hp(w: &mut World, m: SimId, hp: u16) {
    w.state.entities.combat[m.index()].hp = hp;
}

/// Applies `events` (and `fallen`, as riders and gun crews add them) as
/// the Damage stage would at the world's tick; `pre` runs on the context
/// first (with `wounds` set as `apply_blows` sets it).
fn blows_now(
    w: &mut World,
    events: &[BallisticEvent],
    pre: impl FnOnce(&mut MatchState, &mut DamageCtx),
) {
    let tick = w.tick();
    let any_down = !w.game().unwrap().casualties.is_empty();
    if any_down {
        w.downed
            .rebuild_kinds(&w.state.entities, |k| k == EntityKind::Downed);
    }
    let World {
        state,
        hash,
        spatial,
        downed,
        ..
    } = w;
    let SimState {
        entities,
        terrain,
        stats,
        game,
        ..
    } = state;
    let g = game.as_mut().unwrap();
    let mut ctx = DamageCtx {
        strikes: &[],
        ents: entities,
        terrain,
        spatial,
        hash,
        stats,
        seed: 1,
        tick,
        fallen: Vec::new(),
        ff_dead: Vec::new(),
        ff_hit: Vec::new(),
        wounds: (g.setup.rules.wounds.enabled != 0).then(|| g.setup.rules.wounds.clone()),
        downed: any_down.then_some(&*downed),
    };
    pre(g, &mut ctx);
    g.apply_blows(
        &mut ctx,
        events,
        &mut Vec::new(),
        &mut Vec::new(),
        &mut Vec::new(),
    );
    w.refresh_hashes();
}

/// A bullet of `weapon` (player `by`) striking man `m` in `zone`.
pub(super) fn bullet(w: &mut World, by: u8, wpn: &str, m: SimId, zone: u8) {
    let ev = BallisticEvent {
        weapon: weapon(w, wpn),
        player: by,
        ammo: NO_AMMO,
        slot: 0,
        shooter: SimId(u32::MAX),
        blow: Blow::Hit(m, zone),
        mission: 0,
        origin: None,
    };
    blows_now(w, &[ev], |_, _| {});
}

/// W1: the head is the top `head_top_pct`, the legs the bottom
/// `legs_bottom_pct` of a standing or crouched capsule; a prone man is
/// torso. Control: with the bands swapped the same heights change zone.
#[test]
fn w1_zone_by_height_and_prone_is_torso() {
    let mut w = wounds_world(1);
    let m = lone(&mut w, 0, 500, 500);
    pose(&mut w, m, Stance::Stand);
    let wt = w.match_setup().unwrap().rules.wounds.clone();
    let c = w.state.entities.combat[m.index()];
    let t = w.state.entities.transform[m.index()];
    let at = |pct: i32| t.pos.z + c.height.mul_int(pct).div_int(100);
    let z = |pct: i32| crate::wounds::zone_at(&t, &c, at(pct), &wt);
    assert_eq!(z(95), ZONE_HEAD);
    assert_eq!(z(85), ZONE_TORSO);
    assert_eq!(z(60), ZONE_TORSO);
    assert_eq!(z(40), ZONE_LEGS);
    assert_eq!(z(5), ZONE_LEGS);
    let mut p = c;
    p.stance = Stance::Prone;
    assert_eq!(crate::wounds::zone_at(&t, &p, at(95), &wt), ZONE_TORSO);
    let mut wide = wt.clone();
    (wide.head_top_pct, wide.legs_bottom_pct) = (50, 10);
    assert_eq!(crate::wounds::zone_at(&t, &c, at(60), &wide), ZONE_HEAD);
}

/// W2, W3 (bullets, `hurt`): a Kar98k torso hit (70) leaves 30 HP:
/// Wounded; a second leaves -40: Downed (out of his squad, a casualty,
/// credited as a down to the rifle). A leg hit does 60 %. Control: with
/// `enabled` 0 the second hit kills, as before S12.
#[test]
fn w3_bullets_wound_then_down_and_off_kills() {
    let mut w = wounds_world(1);
    let sq = spawn(&mut w, 0, RIFLES, 500, 500);
    steps(&mut w, 2);
    let m = members(&w, sq)[1];
    bullet(&mut w, 1, "ww2:kar98k", m, ZONE_LEGS);
    assert_eq!(hp(&w, m), 100 - 42);
    assert_eq!(wound(&w, m).state, FIT);
    // 58 - 70 = -12: down (not -60: no kill).
    bullet(&mut w, 1, "ww2:kar98k", m, ZONE_TORSO);
    assert_eq!(kind(&w, m), EntityKind::Downed);
    let mut w = wounds_world(1);
    let sq = spawn(&mut w, 0, RIFLES, 500, 500);
    steps(&mut w, 2);
    let m = members(&w, sq)[1];
    bullet(&mut w, 1, "ww2:kar98k", m, ZONE_TORSO);
    assert_eq!((hp(&w, m), wound(&w, m).state), (30, WOUNDED));
    assert_eq!(wound(&w, m).by, (1, kill_class::RIFLE));
    assert_eq!(s12(&w, 0).torso_hits[1], 1);
    bullet(&mut w, 1, "ww2:kar98k", m, ZONE_TORSO);
    assert_eq!(kind(&w, m), EntityKind::Downed);
    assert!(!members(&w, sq).contains(&m));
    assert!(w.game().unwrap().casualties.contains_key(&m));
    assert_eq!(s12(&w, 0).downed, 1);
    assert_eq!(s12(&w, 1).downed_by_class[kill_class::RIFLE as usize], 1);
    assert_eq!(w.game().unwrap().players[1].kills, 0, "a down is no kill");
    assert_eq!(w.game().unwrap().players[0].losses, 0, "nor a loss yet");
    World::load(&w.save(), 1).expect("a downed man saves and loads");
    // Control: wounds off.
    let mut w = wounds_world(0);
    let sq = spawn(&mut w, 0, RIFLES, 500, 500);
    steps(&mut w, 2);
    let m = members(&w, sq)[1];
    bullet(&mut w, 1, "ww2:kar98k", m, ZONE_TORSO);
    assert_eq!((hp(&w, m), wound(&w, m)), (30, Wound::default()));
    bullet(&mut w, 1, "ww2:kar98k", m, ZONE_TORSO);
    assert_eq!(kind(&w, m), EntityKind::Corpse);
    assert_eq!(w.game().unwrap().players[1].kills, 1);
}

/// W3: a hit that leaves -`overkill_kill` or less kills outright; one that
/// leaves -59 downs. Control: `overkill_kill` 1000 downs the first too.
#[test]
fn w3_overkill_kills_outright() {
    for (overkill, hp0, want) in [
        (60, 10, EntityKind::Corpse),
        (60, 11, EntityKind::Downed),
        (1000, 10, EntityKind::Downed),
    ] {
        let mut w = flat_world_with(1, |s| {
            s.rules.wounds.enabled = 1;
            s.rules.wounds.overkill_kill = overkill;
        });
        let m = lone(&mut w, 0, 500, 500);
        set_hp(&mut w, m, hp0);
        bullet(&mut w, 1, "ww2:kar98k", m, ZONE_TORSO);
        assert_eq!(kind(&w, m), want, "{overkill} {hp0}");
        let outright = (want == EntityKind::Corpse) as u32;
        assert_eq!(s12(&w, 0).killed_outright, outright);
    }
}

/// W3 (blast and fragments, `hurt_by`): overpressure kills outright
/// whatever is left; fragments of a mortar burst 6 m off wound or down,
/// never kill a fit man outright (70 at most each). Control: the man at
/// the burst point is a corpse, the far ones are not.
#[test]
fn w3_blast_kills_outright_fragments_wound_or_down() {
    let mut w = wounds_world(1);
    let near = lone(&mut w, 0, 500, 500);
    let far: Vec<SimId> = (0..6).map(|k| lone(&mut w, 0, 494 + 2 * k, 506)).collect();
    let mortar = weapon(&w, "ww2:grw34");
    w.place_soldier(near, c(500, 500));
    for (k, &m) in far.iter().enumerate() {
        w.place_soldier(m, c(494 + 2 * k as i32, 506));
    }
    w.spatial.rebuild(&w.state.entities);
    let t = w.tick();
    burst_now(&mut w, 1, mortar, 0, c(500, 500), t);
    assert_eq!(
        kind(&w, near),
        EntityKind::Corpse,
        "hp {} {:?}",
        hp(&w, near),
        w.state.entities.transform[near.index()].pos
    );
    assert_eq!(s12(&w, 0).killed_outright, 1);
    let mut hurt = 0;
    for &m in &far {
        let k = kind(&w, m);
        assert_ne!(
            k,
            EntityKind::Corpse,
            "a fragment killed a fit man outright"
        );
        if k == EntityKind::Downed || hp(&w, m) < 100 {
            hurt += 1;
        }
    }
    assert!(hurt > 0, "no fragment struck");
}

/// W3 (mines): an AP mine's burst under a man kills him outright (blast)
/// and counts in `men_lost_to_mines`. Control: the counter is 0 before.
#[test]
fn w3_mine_blast_kills_outright() {
    let mut w = wounds_world(1);
    let f = w
        .match_setup()
        .unwrap()
        .forts
        .iter()
        .position(|f| f.id == "ww2:ap_minefield")
        .unwrap() as u16;
    w.lay_mine_now(f, c(500, 500), PlayerId(0)).unwrap();
    let walker = lone(&mut w, 1, 490, 500);
    assert_eq!(w.game().unwrap().fort().accounts[1].men_lost_to_mines, 0);
    for k in 0..30 * 10 {
        if kind(&w, walker) == EntityKind::Soldier {
            let x = Fx::from_int(490) + Fx::from_milli(200 * k.min(100));
            w.place_soldier(walker, FxVec2::new(x, Fx::from_int(500)));
        }
        w.step(&[]);
    }
    assert_eq!(kind(&w, walker), EntityKind::Corpse);
    assert_eq!(w.game().unwrap().fort().accounts[1].men_lost_to_mines, 1);
    assert_eq!(s12(&w, 1).killed_outright, 1);
}

/// Checker F3: a man a mine's fragments down counts in
/// `men_lost_to_mines` at the down, once. Control: the stepper (blast,
/// outright) counts too, so the account equals every loss to the mine.
#[test]
fn w3_mine_downs_count_as_mine_losses() {
    let mut w = wounds_world(1);
    let f = w
        .match_setup()
        .unwrap()
        .forts
        .iter()
        .position(|f| f.id == "ww2:ap_minefield")
        .unwrap() as u16;
    w.lay_mine_now(f, c(500, 500), PlayerId(0)).unwrap();
    let walker = lone(&mut w, 1, 490, 500);
    let mut near = Vec::new();
    for (k, (dx, dy)) in [(3, 2), (-3, 3), (2, -3), (-4, -2), (4, 4), (0, 5)]
        .into_iter()
        .enumerate()
    {
        let m = lone(&mut w, 1, 500 + dx, 500 + dy);
        set_hp(&mut w, m, 1 + k as u16);
        near.push((m, c(500 + dx, 500 + dy)));
    }
    for k in 0..30 * 10 {
        if kind(&w, walker) == EntityKind::Soldier {
            let x = Fx::from_int(490) + Fx::from_milli(200 * k.min(100));
            w.place_soldier(walker, FxVec2::new(x, Fx::from_int(500)));
        }
        for &(m, p) in &near {
            if kind(&w, m) == EntityKind::Soldier {
                w.place_soldier(m, p);
            }
        }
        w.step(&[]);
    }
    let sc = s12(&w, 1);
    assert!(sc.downed >= 1, "a fragment downed someone: {sc:?}");
    let lost = w.game().unwrap().fort().accounts[1].men_lost_to_mines;
    assert_eq!(lost, sc.killed_outright + sc.downed, "{sc:?}");
}

/// Checker F4: two hits in one tick from the same player first down a man,
/// then kill him (any later hit kills, W3). Control: one hit only downs.
#[test]
fn w3_second_same_tick_hit_from_the_same_player_kills() {
    for two in [false, true] {
        let mut w = wounds_world(1);
        let m = lone(&mut w, 0, 500, 500);
        set_hp(&mut w, m, 20);
        let ev = BallisticEvent {
            weapon: weapon(&w, "ww2:m1_garand"),
            player: 1,
            ammo: NO_AMMO,
            slot: 0,
            shooter: SimId(u32::MAX),
            blow: Blow::Hit(m, ZONE_TORSO),
            mission: 0,
            origin: None,
        };
        let evs = if two { vec![ev, ev] } else { vec![ev] };
        blows_now(&mut w, &evs, |_, _| {});
        let want = if two {
            EntityKind::Corpse
        } else {
            EntityKind::Downed
        };
        assert_eq!(kind(&w, m), want, "two hits: {two}");
    }
}

/// Checker F6: a player whose only enemy is a downed man still refreshes
/// that man's marker (spotting no longer returns early). Control: a downed
/// man counts as an enemy the same as a corpse.
#[test]
fn spotting_refreshes_a_lone_downed_enemy() {
    let mut w = wounds_world(1);
    let _eye = lone(&mut w, 0, 500, 500);
    let m = lone(&mut w, 1, 510, 500);
    assert!(w.down_soldier(m, (0, kill_class::RIFLE)));
    w.state.entities.combat[m.index()].seen_by = 0;
    steps(&mut w, 60);
    assert_eq!(kind(&w, m), EntityKind::Downed);
    assert_ne!(
        w.state.entities.combat[m.index()].seen_by & 1,
        0,
        "the downed man's marker refreshed"
    );
}

/// W3 (own fire, S11a2 F4): an own mission burst's fragment that downs an
/// own man credits no down to his own player, and his bleed-out is an
/// own-fire kill, not a kill. Control: an enemy's down is credited.
#[test]
fn w3_own_fire_down_is_no_credit() {
    let mut w = flat_world_with(1, |s| {
        s.rules.wounds.enabled = 1;
        s.rules.fire_mission.as_mut().unwrap().friendly_fire = 1;
    });
    let m = lone(&mut w, 0, 500, 500);
    set_hp(&mut w, m, 20);
    let ev = BallisticEvent {
        weapon: weapon(&w, "ww2:kar98k"),
        player: 0,
        ammo: NO_AMMO,
        slot: 0,
        shooter: SimId(u32::MAX),
        blow: Blow::Hit(m, ZONE_TORSO),
        mission: 0,
        origin: None,
    };
    blows_now(&mut w, &[ev], |_, _| {});
    assert_eq!(kind(&w, m), EntityKind::Downed);
    assert_eq!(s12(&w, 0).downed, 1);
    assert_eq!(s12(&w, 0).downed_by_class, [0; kill_class::COUNT]);
    let at = w.game().unwrap().casualties[&m].bleed_out_at;
    while w.tick() <= at {
        w.step(&[]);
    }
    assert_eq!(kind(&w, m), EntityKind::Corpse);
    let p0 = &w.game().unwrap().players[0];
    assert_eq!((p0.kills, p0.s11a2.ff_kills[1], p0.s12.bled_out), (0, 1, 1));
}

/// W3 (gun crews, `core_through_crew`): an AP core through the shield
/// downs a gunner it leaves between 0 and -59 and frees his slot.
/// Control: `enabled` 0 kills him.
#[test]
fn w3_gun_crew_core_hit_downs() {
    for on in [1, 0] {
        let mut w = wounds_world(on);
        let sq = spawn(&mut w, 0, "ww2:ger_pak40_squad", 500, 500);
        let g = members(&w, sq)[0];
        steps(&mut w, 2);
        let slot0 = |w: &World| {
            w.game().unwrap().vehicles[&g]
                .served
                .as_ref()
                .unwrap()
                .slots[0]
                .man
        };
        let gunner = slot0(&w).unwrap();
        let (kwk, ammo, core) = {
            let s = w.match_setup().unwrap();
            let a = s
                .ammo
                .iter()
                .position(|a| a.id == "ww2:75mm_pzgr39")
                .unwrap();
            (
                s.weapons
                    .iter()
                    .position(|w| w.id == "ww2:kwk40_l48")
                    .unwrap() as u16,
                a as u16,
                s.ammo[a].core.as_ref().unwrap().damage,
            )
        };
        set_hp(&mut w, gunner, (core - 10).min(100) as u16);
        let p = w.entities().transform[gunner.index()].pos;
        let aim = FxVec3::new(p.x, p.y, p.z + Fx::from_ratio(8, 10));
        let from = FxVec3::new(p.x + Fx::from_int(100), p.y, aim.z);
        assert!(w.fire_shell_now(1, kwk, ammo, from, aim));
        steps(&mut w, 8);
        let want = if on == 1 {
            EntityKind::Downed
        } else {
            EntityKind::Corpse
        };
        assert_eq!(kind(&w, gunner), want, "enabled {on}");
        assert_ne!(slot0(&w), Some(gunner));
    }
}

/// W3 (rider core hits, `damage::deal`): a rider struck to between 0 and
/// -59 is downed, leaves his slot and lies at the door. Control: `enabled`
/// 0 kills him.
#[test]
fn w3_rider_hit_downs_at_the_door() {
    for on in [1, 0] {
        let mut w = wounds_world(on);
        let ht = spawn(&mut w, 0, "ww2:ger_sdkfz251_1_squad", 500, 500);
        let v = members(&w, ht)[0];
        let inf = spawn(&mut w, 0, RIFLES, 510, 500);
        steps(&mut w, 2);
        assert!(w.seat_now(inf, v));
        steps(&mut w, 1);
        let vs = &w.game().unwrap().vehicles[&v];
        let (k, rider) = vs
            .riders
            .iter()
            .enumerate()
            .find_map(|(k, r)| r.map(|m| (k, m)))
            .expect("a rider");
        set_hp(&mut w, rider, 30);
        blows_now(&mut w, &[], |g, ctx| {
            g.deal(
                ctx,
                v,
                &[crate::damage::Dealt {
                    inner: crate::damage::Inner::Rider(k as u8),
                    dmg: 50,
                }],
                (1, kill_class::TANK_HE),
                SimId(0),
                &mut crate::damage::FireRolls::default(),
                &mut Vec::new(),
                FxVec3::ZERO,
                0,
            );
        });
        let want = if on == 1 {
            EntityKind::Downed
        } else {
            EntityKind::Corpse
        };
        assert_eq!(kind(&w, rider), want, "enabled {on}");
        assert!(
            w.game().unwrap().vehicles[&v]
                .riders
                .iter()
                .all(|r| *r != Some(rider))
        );
        if on == 1 {
            assert!(w.state.entities.unit[rider.index()].riding.is_none());
            let d = (w.state.entities.transform[rider.index()].pos.xy()
                - w.state.entities.transform[v.index()].pos.xy())
            .length();
            assert!(d < Fx::from_int(8), "lies by the vehicle ({d:?})");
            World::load(&w.save(), 1).expect("loads");
        }
    }
}

/// Holds man `m`'s suppression at `points` (as the Status stage would
/// leave it under fire).
fn suppress(w: &mut World, m: SimId, points: u32) {
    w.state.entities.combat[m.index()].suppression = points * 1000;
}

/// W4: an unbandaged wounded man under fire bleeds 1 HP per
/// `bleed_ms_per_hp` and goes down at 0, credited to his last hitter.
/// Control: a man held calm bandages himself in `bandage_s` (walking
/// included) and then stops bleeding.
#[test]
fn w4_bleeding_downs_and_a_bandage_stops_it() {
    let mut w = wounds_world(1);
    let crouch = w.match_setup().unwrap().rules.combat.crouch_suppression;
    let sq = spawn(&mut w, 0, RIFLES, 300, 300);
    steps(&mut w, 2);
    let ms = members(&w, sq);
    let (a, b) = (ms[1], ms[2]);
    for m in [a, b] {
        bullet(&mut w, 1, "ww2:mg42", m, ZONE_TORSO);
        bullet(&mut w, 1, "ww2:mg42", m, ZONE_TORSO);
        assert_eq!((hp(&w, m), wound(&w, m).state), (10, WOUNDED));
    }
    // The squad walks (b bandages while walking).
    super::s6_tests::run(
        &mut w,
        0,
        0,
        CommandKind::Move {
            squad: sq,
            target: c(300, 400),
        },
    );
    for _ in 0..45 * 30 {
        suppress(&mut w, a, crouch + 5);
        suppress(&mut w, b, 0);
        w.step(&[]);
    }
    assert_eq!(kind(&w, a), EntityKind::Downed, "bled to 0 under fire");
    assert_eq!(wound(&w, a).by, (1, kill_class::MG));
    assert_eq!(s12(&w, 1).downed_by_class[kill_class::MG as usize], 1);
    let wb = wound(&w, b);
    assert!(wb.bandaged && wb.bandaging == 0, "{wb:?}");
    assert!(
        hp(&w, b) >= 10 - 3,
        "bled at most bandage_s / 4 s: {}",
        hp(&w, b)
    );
    assert_eq!(s12(&w, 0).bandaged, 1);
    let moved = w.state.entities.transform[b.index()].pos.y - Fx::from_int(300);
    assert!(moved > Fx::from_int(5), "he walked while bandaging");
}

/// W4 (spec 6 row Bandage): a new hit or firing breaks a dressing in
/// progress; it then restarts. Control: an undisturbed dressing finishes.
#[test]
fn w4_a_new_hit_or_firing_breaks_the_bandage() {
    let mut w = wounds_world(1);
    let m = lone(&mut w, 0, 500, 500);
    bullet(&mut w, 1, "ww2:kar98k", m, ZONE_TORSO);
    steps(&mut w, 30);
    assert!(wound(&w, m).bandaging > 0, "a dressing started");
    bullet(&mut w, 1, "ww2:mg42", m, ZONE_LEGS);
    assert_eq!(wound(&w, m).bandaging, 0, "the hit broke it");
    assert!(wound(&w, m).legs);
    suppress(&mut w, m, 0);
    steps(&mut w, 3);
    assert!(wound(&w, m).bandaging > 0, "restarted");
    w.state.entities.combat[m.index()].since_shot = 0;
    w.step(&[]);
    assert_eq!(wound(&w, m).bandaging, 0, "firing broke it");
    steps(&mut w, 11 * 30);
    assert!(wound(&w, m).bandaged, "control: it finishes undisturbed");
    assert_eq!(s12(&w, 0).bandaged, 1, "one dressing counted");
}

/// W4: a squad walks at its slowest wounded man's pace (legs 60 %).
/// Control: the same squad unhurt walks farther.
#[test]
fn w4_the_squad_paces_to_its_wounded_man() {
    let walk = |legs: bool| {
        let mut w = wounds_world(1);
        let sq = spawn(&mut w, 0, RIFLES, 300, 300);
        steps(&mut w, 2);
        let m = members(&w, sq)[2];
        if legs {
            w.edit_soldier(m, |_, c| {
                c.hp = 40;
                c.wound.state = WOUNDED;
                c.wound.legs = true;
                c.wound.bandaged = true;
            });
        }
        super::s6_tests::run(
            &mut w,
            0,
            0,
            CommandKind::Move {
                squad: sq,
                target: c(300, 600),
            },
        );
        steps(&mut w, 20 * 30);
        w.state.entities.transform[members(&w, sq)[0].index()].pos.y - Fx::from_int(300)
    };
    let (slow, fast) = (walk(true), walk(false));
    assert!(
        slow.mul_int(100) < fast.mul_int(75),
        "slow {slow:?} fast {fast:?}"
    );
}

/// W4 (checker F5): a detached wounded man walks at his own wounded pace
/// (legs 60 %). Control: the same man unhurt walks farther.
#[test]
fn w4_a_detached_wounded_man_walks_slower() {
    let walk = |legs: bool| {
        let mut w = wounds_world(1);
        let sq = spawn(&mut w, 0, RIFLES, 300, 300);
        steps(&mut w, 2);
        let ms = members(&w, sq);
        let m = ms[2];
        // Only the leader stays: nobody in his way.
        for &o in &ms[1..] {
            if o != m {
                assert!(w.kill_soldier(o));
            }
        }
        if legs {
            w.edit_soldier(m, |_, c| {
                c.hp = 40;
                c.wound.state = WOUNDED;
                c.wound.legs = true;
                c.wound.bandaged = true;
            });
        }
        let y0 = w.state.entities.transform[m.index()].pos.y;
        super::s6_tests::run(
            &mut w,
            0,
            0,
            CommandKind::MoveUnits {
                units: vec![m],
                target: c(300, 600),
            },
        );
        steps(&mut w, 20 * 30);
        w.state.entities.transform[m.index()].pos.y - y0
    };
    let (slow, fast) = (walk(true), walk(false));
    assert!(
        slow.mul_int(100) < fast.mul_int(75),
        "slow {slow:?} fast {fast:?}"
    );
}

/// W5: a downed man bleeds out at the seeded tick (one `WOUND_BLEED`
/// draw, in [60, 180] s) and dies there, credited to his hitter as a kill
/// and counted his player's loss. W12: nobody aims at him meanwhile, but a
/// stray round kills him (`downed_killed`). Control: the second man,
/// not hit again, lives to his own bleed-out tick.
#[test]
fn w5_bleed_out_seeded_and_w12_strays_kill() {
    let mut w = wounds_world(1);
    let a = lone(&mut w, 0, 500, 500);
    let b = lone(&mut w, 0, 520, 500);
    let enemy = spawn(&mut w, 1, US_RIFLES, 500, 560);
    for m in [a, b] {
        set_hp(&mut w, m, 20);
        bullet(&mut w, 1, "ww2:m1_garand", m, ZONE_TORSO);
        assert_eq!(kind(&w, m), EntityKind::Downed);
    }
    let hz = 30u64;
    let t0 = w.tick();
    let at = w.game().unwrap().casualties[&b].bleed_out_at;
    assert!(
        (t0 + 60 * hz..=t0 + 180 * hz).contains(&at),
        "{at} from {t0}"
    );
    let draw =
        crate::rng::RngStream::new(w.state.config.seed, t0, b, crate::rng::Purpose::WOUND_BLEED)
            .range_i32(60 * hz as i32, 180 * hz as i32);
    assert_eq!(at, t0 + draw as u64, "the seeded draw");
    // The enemy squad stands near and never aims at the downed men.
    for _ in 0..60 {
        w.step(&[]);
        for m in members(&w, enemy) {
            let t = w.state.entities.combat[m.index()].target;
            assert!(t != Some(a) && t != Some(b), "aimed at a downed man");
        }
    }
    // A stray round kills a.
    bullet(&mut w, 1, "ww2:m1_garand", a, ZONE_TORSO);
    assert_eq!(kind(&w, a), EntityKind::Corpse);
    assert_eq!(s12(&w, 0).downed_killed, 1);
    let kills = w.game().unwrap().players[1].kills;
    let deaths = w.state.stats.deaths;
    while w.tick() <= at {
        w.step(&[]);
    }
    assert_eq!(kind(&w, b), EntityKind::Corpse);
    // Checker F2: a bleed-out counts in `deaths`.
    assert_eq!(w.state.stats.deaths, deaths + 1);
    assert_eq!(s12(&w, 0).bled_out, 1);
    assert_eq!(w.game().unwrap().players[1].kills, kills + 1);
    assert!(w.game().unwrap().casualties.is_empty());
}

/// S12 F2 (review R1): a downed man's death (a later hit, a same-tick
/// second hit, a bleed-out) counts in the hitter's `finished_by_class`
/// as well as `kills_by`, so `kills_by - finished_by_class` per class is
/// the men killed outright and each casualty counts once, by the class
/// that first took him out. Control: a man killed outright adds a kill
/// and no finish.
#[test]
fn r1_finished_downed_men_count_once() {
    let mut w = wounds_world(1);
    let _keep = lone(&mut w, 0, 900, 900);
    // (kills_by, finished_by_class) of player 1 at class `c`.
    let kf = |w: &World, c: u16| {
        let p = &w.game().unwrap().players[1];
        (p.kills_by[c as usize], p.s12.finished_by_class[c as usize])
    };
    let (rifle, mortar, he) = (kill_class::RIFLE, kill_class::MORTAR, kill_class::TANK_HE);
    // Downed by a mortar fragment, finished by a rifle: a kill and a
    // finish for the rifle, a down for the mortar.
    let a = lone(&mut w, 0, 500, 500);
    assert!(w.down_soldier(a, (1, mortar)));
    let r0 = kf(&w, rifle);
    bullet(&mut w, 1, "ww2:m1_garand", a, ZONE_TORSO);
    assert_eq!(kind(&w, a), EntityKind::Corpse);
    assert_eq!(kf(&w, rifle), (r0.0 + 1, r0.1 + 1));
    assert_eq!(kf(&w, mortar), (0, 0));
    // Control: killed outright by a rifle: a kill, no finish.
    let b = lone(&mut w, 0, 530, 500);
    set_hp(&mut w, b, 10);
    let r0 = kf(&w, rifle);
    bullet(&mut w, 1, "ww2:kar98k", b, ZONE_TORSO);
    assert_eq!(kind(&w, b), EntityKind::Corpse);
    assert_eq!(kf(&w, rifle), (r0.0 + 1, r0.1));
    // Down and finished in one tick by two rifle hits.
    let c2 = lone(&mut w, 0, 560, 500);
    set_hp(&mut w, c2, 20);
    let ev = BallisticEvent {
        weapon: weapon(&w, "ww2:m1_garand"),
        player: 1,
        ammo: NO_AMMO,
        slot: 0,
        shooter: SimId(u32::MAX),
        blow: Blow::Hit(c2, ZONE_TORSO),
        mission: 0,
        origin: None,
    };
    let r0 = kf(&w, rifle);
    blows_now(&mut w, &[ev, ev], |_, _| {});
    assert_eq!(kind(&w, c2), EntityKind::Corpse);
    assert_eq!(kf(&w, rifle), (r0.0 + 1, r0.1 + 1));
    // Downed by a tank HE round, bleeds out: the bleed-out finishes him
    // for the class that downed him.
    let d = lone(&mut w, 0, 590, 500);
    assert!(w.down_soldier(d, (1, he)));
    let h0 = kf(&w, he);
    let at = w.game().unwrap().casualties[&d].bleed_out_at;
    while w.tick() <= at {
        w.step(&[]);
    }
    assert_eq!(kind(&w, d), EntityKind::Corpse);
    assert_eq!(kf(&w, he), (h0.0 + 1, h0.1 + 1));
    // Over the whole run (player 1's AI may fight too): per class
    // `kills_by - finished_by_class` sums to player 0's men killed
    // outright, and outright plus downs to every man taken out once.
    let p1 = &w.game().unwrap().players[1];
    let (k, f, dn) = (
        p1.kills_by,
        p1.s12.finished_by_class,
        p1.s12.downed_by_class,
    );
    let s0 = s12(&w, 0);
    let outright: u32 = (0..kill_class::COUNT).map(|c| k[c] - f[c]).sum();
    assert_eq!(outright, s0.killed_outright, "{k:?} {f:?} {s0:?}");
    let cas: u32 = (0..kill_class::COUNT).map(|c| k[c] - f[c] + dn[c]).sum();
    assert_eq!(cas, s0.killed_outright + s0.downed);
    assert_eq!(dn[mortar as usize], 1, "a counts once, for the mortar");
    assert_eq!(k[mortar as usize], 0);
}

/// W5 (S12-6 (A)): a downed man counts toward pop until he dies. W14: a
/// player whose men are all downed and who cannot buy is wiped. Control:
/// with one man standing he is not.
#[test]
fn w14_downed_men_count_in_pop_but_do_not_keep_a_player_in() {
    for standing in [false, true] {
        let mut w = wounds_world(1);
        let a = lone(&mut w, 1, 500, 500);
        let b = lone(&mut w, 1, 520, 500);
        lone(&mut w, 0, 100, 100);
        // Only a and b stand for player 1 (the start forces go).
        let others: Vec<SimId> = w
            .state
            .entities
            .soldiers()
            .filter(|&s| w.state.entities.owner[s.index()] == PlayerId(1) && s != a && s != b)
            .collect();
        for s in others {
            assert!(w.kill_soldier(s));
        }
        let pop = w.game().unwrap().population(&w.state.entities, PlayerId(1));
        set_hp(&mut w, a, 20);
        bullet(&mut w, 0, "ww2:kar98k", a, ZONE_TORSO);
        if !standing {
            set_hp(&mut w, b, 20);
            bullet(&mut w, 0, "ww2:kar98k", b, ZONE_TORSO);
        }
        assert_eq!(
            w.game().unwrap().population(&w.state.entities, PlayerId(1)),
            pop,
            "downed men keep their pop"
        );
        assert!(w.edit_player(PlayerId(1), |p| p.manpower = 0));
        // The US roster holds free cards (cost 0); price them so a player
        // with no manpower cannot buy.
        for t in game_mut(&mut w.state).setup.squad_types.iter_mut() {
            t.cost = t.cost.max(1);
        }
        steps(&mut w, 3 * 30);
        let out = w.game().unwrap().players[1].defeat.is_some();
        assert_eq!(out, !standing, "standing {standing}");
    }
}

/// Spec 6 row Bandage, save/load: a man mid-bandage and a downed man
/// play on after a save and load exactly as without (hashes equal, at 1
/// and 3 threads). Control: the staged state differs from a fresh one.
#[test]
fn s12_save_load_mid_bandage_and_downed_keeps_the_hash() {
    let (mut w, _sq, band, down) = staged();
    assert!(wound(&w, band).bandaging > 0 && kind(&w, down) == EntityKind::Downed);
    steps(&mut w, 2);
    let bytes = w.save();
    let mut a = World::load(&bytes, 1).unwrap();
    let mut b = World::load(&bytes, 3).unwrap();
    for _ in 0..20 * 30 {
        w.step(&[]);
        a.step(&[]);
        b.step(&[]);
    }
    assert_eq!(a.full_state_hash(), w.full_state_hash());
    assert_eq!(b.full_state_hash(), w.full_state_hash());
    assert!(wound(&w, band).bandaged || kind(&w, band) != EntityKind::Soldier);
}

// ------------------------------------------------- checker F8 tests --

/// Checker F8 (W3 `head_kill_dmg`): with it above 0 a head hit dealing at
/// least that much kills a man who falls to it outright (counted).
/// Control: at 0 (and `overkill_kill` out of reach) the same hit downs.
#[test]
fn w3_head_kill_dmg_kills_outright() {
    for head_kill in [0, 1] {
        let mut w = flat_world_with(1, |s| {
            s.rules.wounds.enabled = 1;
            s.rules.wounds.overkill_kill = 1000;
            s.rules.wounds.head_kill_dmg = head_kill;
        });
        let m = lone(&mut w, 0, 500, 500);
        // He falls to the hit (W3 decides only then).
        set_hp(&mut w, m, 20);
        bullet(&mut w, 1, "ww2:kar98k", m, ZONE_HEAD);
        assert_ne!(kind(&w, m), EntityKind::Soldier);
        let dead = kind(&w, m) == EntityKind::Corpse;
        assert_eq!(dead, head_kill > 0, "head_kill_dmg {head_kill}");
        assert_eq!(s12(&w, 0).killed_outright, dead as u32);
        assert_eq!(s12(&w, 0).head_hits[1], 1, "in the open");
    }
}

/// Checker F8 (W5): a crew weapon's carrier going down leaves it to the
/// squad's drops where he fell, as a death does. Control: a rifleman's
/// down adds no drop.
#[test]
fn w5_crew_weapon_to_the_squad_drops_on_down() {
    let mut w = wounds_world(1);
    let sq = spawn(&mut w, 0, RIFLES, 500, 500);
    steps(&mut w, 2);
    let crew = |w: &World, m: SimId| {
        let k = w.state.entities.kit[m.index()].weapon;
        w.match_setup().unwrap().weapons[k as usize].crew
    };
    let ms = members(&w, sq);
    let gunner = *ms.iter().find(|&&m| crew(&w, m)).expect("an MG gunner");
    let rifle = *ms.iter().rev().find(|&&m| !crew(&w, m)).unwrap();
    let drops = |w: &World| w.game().unwrap().squads[&sq].drops.len();
    set_hp(&mut w, rifle, 20);
    bullet(&mut w, 1, "ww2:kar98k", rifle, ZONE_TORSO);
    assert_eq!(kind(&w, rifle), EntityKind::Downed);
    assert_eq!(drops(&w), 0, "control: a rifle is not dropped");
    let at = w.state.entities.transform[gunner.index()].pos.xy();
    set_hp(&mut w, gunner, 20);
    bullet(&mut w, 1, "ww2:kar98k", gunner, ZONE_TORSO);
    assert_eq!(kind(&w, gunner), EntityKind::Downed);
    let d = &w.game().unwrap().squads[&sq].drops;
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].at, at);
    World::load(&w.save(), 1).expect("loads with the drop");
}

/// Checker F8 (W5): a downed man leaves his squad's fire focus and its
/// formation slots (one slot per member). Control: before the down the
/// focus is his.
#[test]
fn w5_downed_man_leaves_focus_and_slots() {
    let mut w = wounds_world(1);
    let sq = spawn(&mut w, 0, RIFLES, 500, 500);
    steps(&mut w, 2);
    let m = members(&w, sq)[3];
    w.state
        .game
        .as_mut()
        .unwrap()
        .squads
        .get_mut(&sq)
        .unwrap()
        .focus = Some(m);
    w.refresh_hashes();
    let q = &w.game().unwrap().squads[&sq];
    assert_eq!(q.focus, Some(m), "control");
    let n = q.members.len();
    assert_eq!(q.nav.slots.len(), n);
    set_hp(&mut w, m, 20);
    bullet(&mut w, 1, "ww2:kar98k", m, ZONE_TORSO);
    assert_eq!(kind(&w, m), EntityKind::Downed);
    let q = &w.game().unwrap().squads[&sq];
    assert_eq!(q.focus, None);
    assert_eq!(q.members.len(), n - 1);
    assert_eq!(q.nav.slots.len(), n - 1);
    steps(&mut w, 30);
    World::load(&w.save(), 1).expect("loads");
}

/// Checker F8 (W4): suppression alone (no hit, no firing) above
/// `crouch_suppression` breaks a dressing in progress. Control: just
/// under it the dressing goes on.
#[test]
fn w4_suppression_alone_breaks_the_bandage() {
    for over in [false, true] {
        let mut w = wounds_world(1);
        let crouch = w.match_setup().unwrap().rules.combat.crouch_suppression;
        let m = lone(&mut w, 0, 500, 500);
        bullet(&mut w, 1, "ww2:kar98k", m, ZONE_TORSO);
        suppress(&mut w, m, 0);
        steps(&mut w, 30);
        assert!(wound(&w, m).bandaging > 0, "a dressing started");
        for _ in 0..5 {
            suppress(&mut w, m, if over { crouch + 5 } else { crouch - 1 });
            w.step(&[]);
        }
        assert_eq!(wound(&w, m).bandaging == 0, over, "over {over}");
    }
}
