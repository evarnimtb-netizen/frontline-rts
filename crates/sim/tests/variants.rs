//! S4 target T9: a unit card made only of `extends` and a `[[members]]`
//! list (`mods/ww2/tests/variants/`) loads, resolves, is called in at once,
//! fights, and its crew assistant picks up the MG when the gunner falls.

use std::path::{Path, PathBuf};

use frontline_content::{Difficulty, MatchRequest, MatchSetup, ModDefs};
use frontline_sim::{Command, CommandKind, Fx, FxVec2, PlayerId, SquadId, World};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2")
}

const VARIANT: &str = "ww2:ger_grenadiergruppe_example";

fn setup(seed: u64) -> MatchSetup {
    let r = root();
    let mut defs = ModDefs::load(&r).unwrap_or_else(|e| panic!("{e}"));
    let added = defs
        .add_defs_dir(&r.join("tests/variants"))
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(added, [VARIANT]);
    defs.apply_override(&r.join("tests/variants/roster.toml"), "ww2:skirmish")
        .unwrap_or_else(|e| panic!("{e}"));
    let mut s = defs
        .resolve(&MatchRequest::skirmish(
            seed,
            "ww2:germany",
            "ww2:usa",
            Difficulty::Normal,
        ))
        .unwrap_or_else(|e| panic!("{e}"));
    s.players.iter_mut().for_each(|p| p.is_ai = false);
    // S11b Build C: written with every S11b switch off (shipped on).
    s.s11b_off();
    s
}

fn squad(w: &World, p: u8, ty: &str) -> SquadId {
    let t = w.match_setup().unwrap().squad_type_index(ty).unwrap();
    *w.game()
        .unwrap()
        .squads()
        .iter()
        .find(|(_, s)| s.owner == PlayerId(p) && s.type_idx == t)
        .unwrap()
        .0
}

#[test]
fn a_variant_of_extends_and_members_is_called_in_fights_and_picks_up_its_mg() {
    // S5a Part B: seed 3 -> 6. With MG and BAR bursts the six men lose the
    // 120 m fight against the twelve without a US loss on seeds 1-5, 7 and
    // 8 (a lone German survivor out of sight then outlasts the limit);
    // seed 6 has losses on both sides.
    let s = setup(6);
    let t = s
        .squad_type_index(VARIANT)
        .expect("the variant is a squad type");
    let ty = &s.squad_types[t as usize];
    let parent = &s.squad_types[s.squad_type_index("ww2:ger_grenadiergruppe").unwrap() as usize];
    // Six men by its own member list, with the parent's card keys.
    assert_eq!(ty.men, 6);
    assert_eq!(
        ty.members.iter().map(|m| m.count).collect::<Vec<_>>(),
        [1, 1, 1, 3]
    );
    assert_eq!(
        (
            ty.cost,
            ty.recall_cooldown_ticks,
            ty.speed,
            ty.sight,
            &ty.tags
        ),
        (
            parent.cost,
            parent.recall_cooldown_ticks,
            parent.speed,
            parent.sight,
            &parent.tags
        )
    );
    // Its main weapon is the MG (the first crew weapon).
    assert_eq!(s.weapons[ty.weapon as usize].id, "ww2:mg42_bipod");
    let ger = s
        .factions
        .iter()
        .position(|f| f.id == "ww2:germany")
        .unwrap();
    assert!(s.factions[ger].roster.contains(&t));

    let mut w = World::new_match(s, 2).unwrap();
    let known: Vec<SquadId> = w.game().unwrap().squads().keys().copied().collect();
    let cmd = Command {
        player: PlayerId(0),
        seq: 0,
        kind: CommandKind::CallIn { squad_type: t },
    };
    w.step(&[cmd]);
    // S3b call-in: it spawns at once at the base, nothing is queued.
    let id = *w
        .game()
        .unwrap()
        .squads()
        .keys()
        .find(|id| !known.contains(id))
        .expect("the variant spawns in the call-in tick");
    let sq = w.game().unwrap().squad(id).unwrap();
    assert_eq!(sq.type_idx, t);
    assert_eq!(sq.members.len(), 6);
    let (gunner, assistant) = (sq.members[1], sq.members[2]);
    let weapon = |w: &World, m: frontline_sim::SimId| {
        let k = w.entities().kit()[m.index()].weapon;
        w.match_setup().unwrap().weapons[k as usize].id.clone()
    };
    assert_eq!(weapon(&w, gunner), "ww2:mg42_bipod");

    // It fights a US rifle squad 120 m away; its MG gunner falls first and
    // the crew assistant takes the MG up.
    let us = squad(&w, 1, "ww2:usa_rifle_squad");
    let east = FxVec2::new(Fx::ONE, Fx::ZERO);
    let west = FxVec2::new(Fx::NEG_ONE, Fx::ZERO);
    assert!(w.place_squad(id, FxVec2::from_int(430, 420), east));
    assert!(w.place_squad(us, FxVec2::from_int(550, 420), west));
    for _ in 0..30 {
        w.step(&[]);
    }
    w.kill_soldier(gunner);
    let losses = |w: &World, p: usize| w.game().unwrap().players()[p].losses;
    // S4 review L4: the test's own kill counts as a German loss; the fight
    // must take more than that.
    let staged = losses(&w, 0);
    let alive = |w: &World, q: SquadId| w.game().unwrap().squad(q).is_some();
    let mut picked = false;
    // S4 Tune: the fight is over once the MG changed hands and men fell on
    // both sides, or one squad is gone (the ridge moved 12 m in the Tune; a
    // lone survivor out of the other squad's sight can outlast any limit).
    while alive(&w, id) && alive(&w, us) && !(picked && losses(&w, 0) > staged && losses(&w, 1) > 0)
    {
        w.step(&[]);
        picked |= alive(&w, id) && weapon(&w, assistant) == "ww2:mg42_bipod";
        assert!(w.tick() < 30 * 300, "the fight never ended");
    }
    assert!(picked, "the crew assistant never had the MG");
    assert!(w.game().unwrap().players()[0].s4.pickups >= 1);
    // The fight itself is what T9 asks for: men fell on both sides.
    assert!(losses(&w, 0) > staged && losses(&w, 1) > 0);
}
