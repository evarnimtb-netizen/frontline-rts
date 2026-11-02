//! S4 Part A (target T9, first half): a unit card made of `extends` and a
//! `[[members]]` list (`mods/ww2/tests/variants/`) loads, resolves into
//! today's squad setup, is called in at once and fights. The crew-weapon
//! pickup half of T9 comes with S4 Part B.

use std::path::{Path, PathBuf};

use frontline_content::{Difficulty, MatchRequest, MatchSetup, ModDefs};
use frontline_sim::{Command, CommandKind, Fx, FxVec2, PlayerId, SquadId, World};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/ww2")
}

const VARIANT: &str = "ww2:usa_rifle_squad_understrength";

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
fn a_variant_of_extends_and_members_is_called_in_and_fights() {
    let s = setup(3);
    let t = s
        .squad_type_index(VARIANT)
        .expect("the variant is a squad type");
    let ty = &s.squad_types[t as usize];
    let parent = &s.squad_types[s.squad_type_index("ww2:usa_rifle_squad").unwrap() as usize];
    // Nine men with the parent's rifle, cost, cooldown, speed and sight.
    assert_eq!((ty.men, ty.weapon_men, ty.sidearm), (9, 9, None));
    assert_eq!(ty.weapon, parent.weapon);
    assert_eq!(
        (ty.cost, ty.recall_cooldown_ticks, ty.speed, ty.sight),
        (
            parent.cost,
            parent.recall_cooldown_ticks,
            parent.speed,
            parent.sight
        )
    );
    // The leader's two frags and a smoke, one frag for each rifleman.
    let mut kit: Vec<(u8, u8)> = ty
        .loadout
        .iter()
        .map(|k| (k.count, k.leader_count))
        .collect();
    kit.sort();
    assert_eq!(kit, [(0, 1), (1, 2)]);
    let usa = s.factions.iter().position(|f| f.id == "ww2:usa").unwrap();
    assert!(s.factions[usa].roster.contains(&t));

    let mut w = World::new_match(s, 2).unwrap();
    let known: Vec<SquadId> = w.game().unwrap().squads().keys().copied().collect();
    let cmd = Command {
        player: PlayerId(1),
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
    assert_eq!(sq.members.len(), 9);

    // It fights a German rifle squad 120 m away until one side is gone.
    let ger = squad(&w, 0, "ww2:ger_rifle_squad");
    let east = FxVec2::new(Fx::ONE, Fx::ZERO);
    let west = FxVec2::new(Fx::NEG_ONE, Fx::ZERO);
    assert!(w.place_squad(ger, FxVec2::from_int(430, 420), east));
    assert!(w.place_squad(id, FxVec2::from_int(550, 420), west));
    let losses = |w: &World, p: usize| w.game().unwrap().players()[p].losses;
    let alive = |w: &World, q: SquadId| w.game().unwrap().squad(q).is_some();
    while alive(&w, ger) && alive(&w, id) {
        w.step(&[]);
        assert!(w.tick() < 30 * 300, "the fight never ended");
    }
    assert!(losses(&w, 0) > 0 && losses(&w, 1) > 0);
}
