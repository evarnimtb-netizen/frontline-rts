//! Player intents (godot/docs/match_state.md) → sim [`CommandKind`]s.
//! Plain Rust: Godot-space floats in, fixed-point commands out.
//!
//! This is the one place presenter floats become sim input. Positions are
//! quantised to millimetres once, here, and the resulting commands are what
//! the sim (and a replay or a lockstep peer) sees, so the conversion never
//! has to be bit-identical across machines.
//!
//! S1b soldier-level intents carry `units` (soldier ids). They are filtered
//! to the local player's living men, sorted and deduplicated here
//! ([`unit_set`]) and sent in commands of at most [`MAX_UNITS`] ids
//! ([`unit_chunks`]); the sim validates them again.
//!
//! S3: own vehicle ids go in `units` next to soldier ids; the vehicle
//! intents (`reverse_units`, `face_units`, `attack_unit`, `set_ammo`,
//! `repair_units`, `follow_units`) become their commands in
//! [`vehicle_command`], dig intents carry a [`DigProfile`], and a call-in
//! may name its squad type id ([`call_in_by_id`]). S3b: `suppress_units`
//! (men and vehicles) carries a `polyline` of one or two points, or a
//! `target` ([`unit_command`]).

use frontline_content::{DigProfile, ItemKind, MatchSetup};
use frontline_sim::game::MAX_DIG_POINTS;
use frontline_sim::{
    AmmoChoice, CommandKind, FireMode, Fx, FxVec2, MAX_UNITS, SimId, SquadId, StanceOrder,
};

/// Lateral spacing (m) between squads sent to one point together.
pub const GROUP_SPACING_M: f32 = 18.0;
/// Shortest piece of a dig line handed to one squad.
pub const MIN_DIG_PIECE_M: f32 = 4.0;

/// Godot `(x, z)` → sim position (millimetre precision).
pub fn godot_to_fx(x: f32, z: f32, half: f32) -> FxVec2 {
    sim_to_fx([x + half, -z + half])
}

pub fn sim_to_fx(p: [f32; 2]) -> FxVec2 {
    let q = |v: f32| Fx::from_milli((v * 1000.0).round().clamp(-2.0e9, 2.0e9) as i32);
    FxVec2::new(q(p[0]), q(p[1]))
}

/// The presenter's `type` word of squad type `t` (`rifle`, `mg`,
/// `engineer`, `mortar`, `at`, `vehicle`, `crew`; S4: derived from the
/// card's tags, `MatchSetup::type_word`).
pub fn role_name(setup: &MatchSetup, t: u16) -> &'static str {
    setup.type_word(t)
}

/// The first squad type of `role` (a `type` word) in player `player`'s
/// faction roster (legacy intent; S4 HUD tiles call in by card id).
pub fn call_in_type(setup: &MatchSetup, player: u8, role: &str) -> Option<u16> {
    let f = setup.players.get(player as usize)?.faction as usize;
    setup
        .factions
        .get(f)?
        .roster
        .iter()
        .copied()
        .find(|&t| setup.type_word(t) == role)
}

/// The squad type with id `id` (`ww2:ger_pz4_h_squad`) in player
/// `player`'s faction roster.
pub fn call_in_by_id(setup: &MatchSetup, player: u8, id: &str) -> Option<u16> {
    let f = setup.players.get(player as usize)?.faction as usize;
    setup.factions.get(f)?.roster.iter().copied().find(|&t| {
        setup
            .squad_types
            .get(t as usize)
            .is_some_and(|ty| ty.id == id)
    })
}

/// Dig intent `profile` names: `"at_ditch"`, else the trench.
pub fn dig_profile(name: &str) -> DigProfile {
    match name {
        "at_ditch" => DigProfile::AtDitch,
        // S6 pits.
        "mortar_pit" => DigProfile::MortarPit,
        "hq_pit" => DigProfile::HqPit,
        "dump_pit" => DigProfile::DumpPit,
        "gun_pit" => DigProfile::GunPit,
        _ => DigProfile::Trench,
    }
}

/// S6 `issue_crate` kind names (`match_state.md`); S10a adds
/// `"construction"`.
pub fn supply_kind(name: &str) -> Option<frontline_content::setup::SupplyKind> {
    frontline_content::setup::SupplyKind::from_crate_name(name)
}

/// S10a: a crate kind's name (`CrateSnapshot::kind`): the three ammunition
/// kinds, then `"construction"`.
pub fn crate_kind_name(kind: u8) -> &'static str {
    use frontline_content::setup::SupplyKind;
    match kind {
        3 => "construction",
        k => SupplyKind::NAMES[k as usize % SupplyKind::NAMES.len()],
    }
}

// ------------------------------------------------------- S10a intents --

/// S10a: the blueprint `what` names (`"sandbag_wall"`, `"barbed_wire"`,
/// `"mg_nest"`, `"dugout"`, `"hq_dugout"`; a full id such as
/// `"ww2:mg_nest"` works too), as an index into `MatchSetup::forts`.
pub fn fort_index(setup: &MatchSetup, what: &str) -> Option<u16> {
    setup
        .forts
        .iter()
        .position(|f| f.id == what || f.id.rsplit(':').next() == Some(what))
        .map(|i| i as u16)
}

/// S10a: whether squad type `t` may build blueprint `fort` (its capability).
pub fn can_fortify(setup: &MatchSetup, t: u16, fort: u16) -> bool {
    let (Some(ty), Some(f)) = (
        setup.squad_types.get(t as usize),
        setup.forts.get(fort as usize),
    ) else {
        return false;
    };
    ty.caps & (1u32 << f.capability) != 0
}

/// S10a `fortify`: the first of `squads` (`(id, type_idx)`, selection
/// order) able to build `what`. A line blueprint takes `line` (sim xy,
/// capped to 16 points; at least 2); a point or over-HQ blueprint takes
/// `target` and `facing` (`[pos, pos + facing]`; `facing` defaults to
/// east; the facing point is kept on the map: turned round at an edge,
/// clamped in a corner, since the sim refuses an off-map one).
pub fn fortify_command(
    setup: &MatchSetup,
    squads: &[(u32, u16)],
    what: &str,
    line: &[[f32; 2]],
    target: Option<[f32; 2]>,
    facing: Option<[f32; 2]>,
) -> Option<CommandKind> {
    use frontline_content::setup::FortShape;
    let fort = fort_index(setup, what)?;
    let &(squad, _) = squads.iter().find(|s| can_fortify(setup, s.1, fort))?;
    let pts: Vec<[f32; 2]> = if setup.forts[fort as usize].shape == FortShape::Line {
        if line.len() < 2 {
            return None;
        }
        cap_points(line)
    } else {
        let p = target.or_else(|| line.first().copied())?;
        let f = facing
            .filter(|f| f[0].hypot(f[1]) > 1e-3)
            .map(|f| {
                let n = f[0].hypot(f[1]);
                [f[0] / n, f[1] / n]
            })
            .unwrap_or([1.0, 0.0]);
        let size = setup.map.size_m.max(0) as f32;
        let on = |q: [f32; 2]| q.iter().all(|v| (0.0..=size).contains(v));
        let mut q = [p[0] + f[0], p[1] + f[1]];
        if !on(q) {
            q = [p[0] - f[0], p[1] - f[1]];
        }
        if !on(q) {
            q = [q[0].clamp(0.0, size), q[1].clamp(0.0, size)];
        }
        vec![p, q]
    };
    Some(CommandKind::Fortify {
        squad: SquadId(squad),
        fort,
        line: pts.into_iter().map(sim_to_fx).collect(),
    })
}

/// S10a `load_materials`: every one of `squads` (`(id, type_idx, men)`)
/// able to build anything loads `kg` (default: a full load,
/// `carry_kg_per_man` per man), and every supply truck's squad fills its
/// truck (default: all the room left, `u16::MAX` kg, which the sim clamps
/// to the cap less cargo). The sim refuses outside the base.
pub fn load_materials_commands(
    setup: &MatchSetup,
    squads: &[(u32, u16, u8)],
    kg: Option<u32>,
) -> Vec<CommandKind> {
    use frontline_content::setup::Capability;
    let per_man = setup
        .rules
        .fortify
        .as_ref()
        .map_or(0, |t| t.carry_kg_per_man);
    squads
        .iter()
        .filter(|s| {
            setup.squad_types.get(s.1 as usize).is_some_and(|ty| {
                ty.has(Capability::BuildStructure) || ty.has(Capability::PlaceWire)
            }) || is_supply_truck(setup, s.1)
        })
        .filter_map(|&(id, t, men)| {
            let full = if is_supply_truck(setup, t) {
                u16::MAX as u32
            } else {
                per_man * men as u32
            };
            let kg = kg.unwrap_or(full).min(u16::MAX as u32) as u16;
            (kg > 0).then_some(CommandKind::LoadMaterials {
                squad: SquadId(id),
                kg,
            })
        })
        .collect()
}

/// S10a: whether squad type `t` is a supply truck's (it can carry
/// construction material).
pub fn is_supply_truck(setup: &MatchSetup, t: u16) -> bool {
    setup
        .squad_types
        .get(t as usize)
        .and_then(|ty| ty.vehicle)
        .and_then(|v| setup.vehicles.get(v as usize))
        .is_some_and(|v| v.supply.is_some())
}

/// S10a `cut_wire`: the first of `squads` (`(id, type_idx)`) with
/// `clear_wire` cuts the section nearest `at`.
pub fn cut_wire_command(
    setup: &MatchSetup,
    squads: &[(u32, u16)],
    at: [f32; 2],
) -> Option<CommandKind> {
    use frontline_content::setup::Capability;
    let &(squad, _) = squads.iter().find(|s| {
        setup
            .squad_types
            .get(s.1 as usize)
            .is_some_and(|ty| ty.has(Capability::ClearWire))
    })?;
    Some(CommandKind::CutWire {
        squad: SquadId(squad),
        at: sim_to_fx(at),
    })
}

/// S10a `bangalore`: the men of the first of `squads` (`(id, type_idx)`)
/// with `breach` whose men (`men_of`) carry a Bangalore; the sim sends the
/// nearest carrier.
pub fn bangalore_command(
    setup: &MatchSetup,
    squads: &[(u32, u16)],
    men_of: impl Fn(u32) -> Vec<(u32, u8)>,
    target: [f32; 2],
) -> Option<CommandKind> {
    use frontline_content::setup::Capability;
    for &(id, t) in squads {
        if !setup
            .squad_types
            .get(t as usize)
            .is_some_and(|ty| ty.has(Capability::Breach))
        {
            continue;
        }
        let men = men_of(id);
        if !men.iter().any(|m| m.1 > 0) {
            continue;
        }
        let mut ids: Vec<u32> = men.iter().map(|m| m.0).collect();
        ids.sort_unstable();
        ids.dedup();
        ids.truncate(MAX_UNITS);
        return Some(CommandKind::PlaceBangalore {
            units: ids.into_iter().map(SimId).collect(),
            target: sim_to_fx(target),
        });
    }
    None
}

/// S10b `clear_mines`: the first of `squads` (`(id, type_idx)`) with
/// `clear_mines` sweeps the lane from `line[0]` to its last point (a drag).
/// The sim refuses a lane over `lane_max_m`.
pub fn clear_mines_command(
    setup: &MatchSetup,
    squads: &[(u32, u16)],
    line: &[[f32; 2]],
) -> Option<CommandKind> {
    use frontline_content::setup::Capability;
    let (&a, &b) = (line.first()?, line.last()?);
    if line.len() < 2 || (a[0] - b[0]).hypot(a[1] - b[1]) < 1e-3 {
        return None;
    }
    let &(squad, _) = squads.iter().find(|s| {
        setup
            .squad_types
            .get(s.1 as usize)
            .is_some_and(|ty| ty.has(Capability::ClearMines))
    })?;
    Some(CommandKind::ClearMines {
        squad: SquadId(squad),
        a: sim_to_fx(a),
        b: sim_to_fx(b),
    })
}

/// S10b `demolish`: the first of `squads` (`(id, type_idx)`) with
/// `demolish` blows the piece nearest `at` (the sim picks the piece within
/// `demolish_reach_m` and the nearest man with a satchel).
pub fn demolish_command(
    setup: &MatchSetup,
    squads: &[(u32, u16)],
    at: [f32; 2],
) -> Option<CommandKind> {
    use frontline_content::setup::Capability;
    let &(squad, _) = squads.iter().find(|s| {
        setup
            .squad_types
            .get(s.1 as usize)
            .is_some_and(|ty| ty.has(Capability::Demolish))
    })?;
    Some(CommandKind::Demolish {
        squad: SquadId(squad),
        at: sim_to_fx(at),
    })
}

/// S6 `build` names: `"hq"` or `"dump"`.
pub fn stock_kind(name: &str) -> Option<frontline_content::setup::StockKind> {
    use frontline_content::setup::StockKind;
    Some(match name {
        "hq" => StockKind::Hq,
        "dump" => StockKind::Dump,
        _ => return None,
    })
}

/// `set_ammo` names (`"auto"`, `"ap"`, `"he"`; S3b `"apcr"`).
pub fn ammo_choice(name: &str) -> Option<AmmoChoice> {
    Some(match name {
        "auto" => AmmoChoice::Auto,
        "ap" => AmmoChoice::Ap,
        "he" => AmmoChoice::He,
        "apcr" => AmmoChoice::Apcr,
        _ => return None,
    })
}

pub fn ammo_choice_name(c: u8) -> &'static str {
    match c {
        1 => "ap",
        2 => "he",
        3 => "apcr",
        _ => "auto",
    }
}

/// Destinations for `n` squads at `anchors` (sim xy) ordered to `target`:
/// a line across the group's approach direction, `spacing` apart, in the
/// order that keeps their paths from crossing.
pub fn spread_targets(anchors: &[[f32; 2]], target: [f32; 2], spacing: f32) -> Vec<[f32; 2]> {
    let n = anchors.len();
    if n <= 1 {
        return vec![target; n];
    }
    let c = anchors.iter().fold([0.0f32; 2], |a, p| {
        [a[0] + p[0] / n as f32, a[1] + p[1] / n as f32]
    });
    let (dx, dy) = (target[0] - c[0], target[1] - c[1]);
    let len = (dx * dx + dy * dy).sqrt();
    let dir = if len > 1e-3 {
        [dx / len, dy / len]
    } else {
        [0.0, 1.0]
    };
    let side = [-dir[1], dir[0]];
    // Rank squads by where they stand across the approach line.
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| {
        let pa = anchors[a][0] * side[0] + anchors[a][1] * side[1];
        let pb = anchors[b][0] * side[0] + anchors[b][1] * side[1];
        pa.total_cmp(&pb).then(a.cmp(&b))
    });
    let mut out = vec![target; n];
    for (rank, &i) in order.iter().enumerate() {
        let k = rank as f32 - (n - 1) as f32 * 0.5;
        out[i] = [
            target[0] + side[0] * k * spacing,
            target[1] + side[1] * k * spacing,
        ];
    }
    out
}

/// Splits `line` into `parts` consecutive pieces of equal length (fewer if
/// a piece would be shorter than `min_len`). Each piece keeps the original
/// corners it spans, so the trenches join up.
pub fn split_polyline(line: &[[f32; 2]], parts: usize, min_len: f32) -> Vec<Vec<[f32; 2]>> {
    let total = crate::match_frame::polyline_len(line);
    if line.len() < 2 || parts <= 1 || total < 2.0 * min_len {
        return vec![line.to_vec()];
    }
    let parts = parts.min((total / min_len) as usize).max(1);
    let piece = total / parts as f32;
    let mut cuts = (1..parts).map(|k| k as f32 * piece).peekable();
    let mut out = Vec::with_capacity(parts);
    let mut cur = vec![line[0]];
    let mut acc = 0.0;
    for w in line.windows(2) {
        let (a, b) = (w[0], w[1]);
        let seg = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
        while let Some(&c) = cuts.peek() {
            if seg <= 0.0 || c > acc + seg {
                break;
            }
            let t = ((c - acc) / seg).clamp(0.0, 1.0);
            let p = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
            if cur.last() != Some(&p) {
                cur.push(p);
            }
            out.push(std::mem::replace(&mut cur, vec![p]));
            cuts.next();
        }
        acc += seg;
        if cur.last() != Some(&b) {
            cur.push(b);
        }
    }
    out.push(cur);
    out
}

/// Caps a polyline to at most [`MAX_DIG_POINTS`] points (keeps the ends).
pub fn cap_points(line: &[[f32; 2]]) -> Vec<[f32; 2]> {
    if line.len() <= MAX_DIG_POINTS {
        return line.to_vec();
    }
    let n = MAX_DIG_POINTS;
    (0..n)
        .map(|k| line[k * (line.len() - 1) / (n - 1)])
        .collect()
}

/// A Dig command for `squad` along `line` (sim xy).
pub fn dig_command(squad: u32, line: &[[f32; 2]], profile: DigProfile) -> CommandKind {
    CommandKind::Dig {
        squad: SquadId(squad),
        line: cap_points(line).into_iter().map(sim_to_fx).collect(),
        profile,
    }
}

/// The soldier set of a `*_units`, `set_stance` or `throw` intent: ids that
/// pass `own` (the local player's living men), ascending, without
/// duplicates. Empty means the intent is dropped. Larger than
/// [`MAX_UNITS`] only with a mod pop cap above it: [`unit_chunks`] splits
/// it into commands (S4 review M4).
pub fn unit_set(raw: &[i32], own: impl Fn(u32) -> bool) -> Vec<SimId> {
    let mut ids: Vec<u32> = raw
        .iter()
        .filter(|&&i| i >= 0)
        .map(|&i| i as u32)
        .filter(|&i| own(i))
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids.into_iter().map(SimId).collect()
}

/// A unit set in pieces of at most [`MAX_UNITS`] ids, one command each, in
/// ascending id order (one piece for any selection inside the shipped pop
/// cap of 150).
pub fn unit_chunks(units: &[SimId]) -> impl Iterator<Item = Vec<SimId>> + '_ {
    units.chunks(MAX_UNITS).map(<[SimId]>::to_vec)
}

/// `set_stance` names (`match_state.md`).
pub fn stance_order(name: &str) -> Option<StanceOrder> {
    Some(match name {
        "auto" => StanceOrder::Auto,
        "stand" => StanceOrder::Stand,
        "crouch" => StanceOrder::Crouch,
        "prone" => StanceOrder::Prone,
        _ => return None,
    })
}

pub fn stance_order_name(s: u8) -> &'static str {
    match s {
        1 => "stand",
        2 => "crouch",
        3 => "prone",
        _ => "auto",
    }
}

/// S5a `set_fire` intent mode names.
pub fn fire_mode(name: &str) -> Option<FireMode> {
    Some(match name {
        "discipline" => FireMode::Discipline,
        "at_will" => FireMode::AtWill,
        "hold" => FireMode::Hold,
        _ => return None,
    })
}

pub fn fire_mode_name(m: FireMode) -> &'static str {
    match m {
        FireMode::Discipline => "discipline",
        FireMode::AtWill => "at_will",
        FireMode::Hold => "hold",
    }
}

/// Throw intent item names.
pub fn item_kind(name: &str) -> Option<ItemKind> {
    Some(match name {
        "frag" => ItemKind::Frag,
        "smoke" => ItemKind::Smoke,
        "satchel" => ItemKind::Satchel,
        "panzerfaust" => ItemKind::SingleShotAt,
        "bangalore" => ItemKind::Bangalore,
        // S10b: never in a kit, so a throw intent naming it is refused.
        "mine" => ItemKind::Mine,
        _ => return None,
    })
}

pub fn item_name(k: ItemKind) -> &'static str {
    match k {
        ItemKind::Frag => "frag",
        ItemKind::Smoke => "smoke",
        ItemKind::Satchel => "satchel",
        ItemKind::SingleShotAt => "panzerfaust",
        ItemKind::Bangalore => "bangalore",
        ItemKind::Mine => "mine",
    }
}

/// The Command for a soldier-level intent `kind` on `units` (already
/// filtered by [`unit_set`], non-empty). `target` and `line` are sim xy;
/// they are quantised here, once. `word` is the stance, the item or (dig)
/// the profile. `None` when the intent lacks what it needs (a target, a
/// line of two points, a known stance or item).
pub fn unit_command(
    kind: &str,
    units: Vec<SimId>,
    target: Option<[f32; 2]>,
    line: &[[f32; 2]],
    word: &str,
) -> Option<CommandKind> {
    let t = || target.map(sim_to_fx);
    Some(match kind {
        "move_units" => CommandKind::MoveUnits {
            units,
            target: t()?,
        },
        "attack_move_units" => CommandKind::AttackMoveUnits {
            units,
            target: t()?,
        },
        "stop_units" => CommandKind::StopUnits { units },
        "retreat_units" => CommandKind::RetreatUnits { units },
        "dig_units" if line.len() >= 2 => CommandKind::DigUnits {
            units,
            line: cap_points(line).into_iter().map(sim_to_fx).collect(),
            profile: dig_profile(word),
        },
        "set_stance" => CommandKind::SetStance {
            units,
            stance: stance_order(word)?,
        },
        "set_fire" => CommandKind::SetFire {
            units,
            mode: fire_mode(word)?,
        },
        "throw" => CommandKind::Throw {
            units,
            item: item_kind(word)?,
            target: t()?,
        },
        "throw_each" => CommandKind::ThrowEach {
            units,
            item: item_kind(word)?,
            target: t()?,
        },
        // S3b: a line (its first and last points) or a point (`target`).
        "suppress_units" => CommandKind::SuppressUnits {
            units,
            line: match line {
                [] => vec![t()?],
                [a] => vec![sim_to_fx(*a)],
                [a, .., b] => vec![sim_to_fx(*a), sim_to_fx(*b)],
            },
        },
        _ => return None,
    })
}

/// The Command for an S3 vehicle intent `kind` on `units` (filtered by
/// [`unit_set`], non-empty): `target` is sim xy (reverse, face), `id` the
/// vehicle named (`attack_unit`'s target, `follow_units`' vehicle), `word`
/// the ammo choice. `None` when the intent lacks what it needs.
pub fn vehicle_command(
    kind: &str,
    units: Vec<SimId>,
    target: Option<[f32; 2]>,
    id: Option<u32>,
    word: &str,
) -> Option<CommandKind> {
    let t = || target.map(sim_to_fx);
    Some(match kind {
        "reverse_units" => CommandKind::ReverseUnits {
            units,
            target: t()?,
        },
        "face_units" => CommandKind::FaceUnits {
            units,
            toward: t()?,
        },
        "attack_unit" => CommandKind::AttackUnit {
            units,
            target: SimId(id?),
        },
        "set_ammo" => CommandKind::SetAmmo {
            units,
            ammo: ammo_choice(word)?,
        },
        "repair_units" => CommandKind::RepairUnits { units },
        "follow_units" => CommandKind::FollowUnits {
            units,
            vehicle: SimId(id?),
        },
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::match_frame::polyline_len;

    #[test]
    fn godot_points_become_sim_fixed_point() {
        let p = godot_to_fx(-412.0, 384.25, 512.0);
        assert_eq!(p.to_f32(), [100.0, 127.75]);
        assert_eq!(sim_to_fx([1.0006, -2.0]).x, Fx::from_milli(1001));
    }

    #[test]
    fn group_targets_spread_across_the_approach() {
        let anchors = [[0.0, 0.0], [10.0, 0.0], [5.0, -3.0]];
        let t = spread_targets(&anchors, [5.0, 100.0], 18.0);
        assert_eq!(t.len(), 3);
        // Moving north: spread along x, left-most squad keeps the left slot.
        let mut xs: Vec<f32> = t.iter().map(|p| p[0]).collect();
        xs.sort_by(f32::total_cmp);
        assert!((xs[2] - xs[0] - 36.0).abs() < 0.5, "{xs:?}");
        assert!(t[0][0] < t[1][0]);
        assert_eq!(
            spread_targets(&anchors[..1], [5.0, 100.0], 18.0),
            vec![[5.0, 100.0]]
        );
    }

    #[test]
    fn dig_lines_split_evenly() {
        let line = [[0.0, 0.0], [10.0, 0.0], [10.0, 20.0]];
        let parts = split_polyline(&line, 3, 4.0);
        assert_eq!(parts.len(), 3);
        for p in &parts {
            assert!((polyline_len(p) - 10.0).abs() < 0.01, "{p:?}");
        }
        assert_eq!(parts[0].first(), Some(&[0.0, 0.0]));
        assert_eq!(parts[2].last(), Some(&[10.0, 20.0]));
        // Consecutive pieces share their joint.
        assert_eq!(parts[0].last(), parts[1].first());
        // Too short to split.
        assert_eq!(split_polyline(&[[0.0, 0.0], [6.0, 0.0]], 3, 4.0).len(), 1);
        let two = split_polyline(&[[0.0, 0.0], [10.0, 0.0]], 4, 4.0);
        assert_eq!(two.len(), 2);
        assert!((polyline_len(&two[1]) - 5.0).abs() < 0.01);
    }

    #[test]
    fn unit_sets_are_own_sorted_unique_and_chunked() {
        let own = |i: u32| i.is_multiple_of(2);
        let ids = unit_set(&[8, 2, -1, 3, 2, 6], own);
        assert_eq!(ids, vec![SimId(2), SimId(6), SimId(8)]);
        assert!(unit_set(&[1, 3], own).is_empty());
        // S4 review M4: no ids are lost; commands carry MAX_UNITS each.
        let many: Vec<i32> = (0..400).collect();
        let all = unit_set(&many, |_| true);
        assert_eq!(all.len(), 400);
        let chunks: Vec<Vec<SimId>> = unit_chunks(&all).collect();
        assert_eq!(chunks.len(), 400usize.div_ceil(MAX_UNITS));
        assert!(chunks.iter().all(|c| c.len() <= MAX_UNITS));
        assert_eq!(chunks.concat(), all);
        const { assert!(MAX_UNITS >= 150, "the shipped pop cap fits one command") };
    }

    #[test]
    fn unit_intents_become_commands() {
        let u = || vec![SimId(4), SimId(9)];
        let t = Some([100.0006, 20.0]);
        match unit_command("move_units", u(), t, &[], "") {
            Some(CommandKind::MoveUnits { units, target }) => {
                assert_eq!(units, u());
                assert_eq!(target, sim_to_fx([100.0006, 20.0]));
                assert_eq!(target.x, Fx::from_milli(100_001));
            }
            c => panic!("{c:?}"),
        }
        assert!(matches!(
            unit_command("attack_move_units", u(), t, &[], ""),
            Some(CommandKind::AttackMoveUnits { .. })
        ));
        assert!(unit_command("move_units", u(), None, &[], "").is_none());
        assert!(matches!(
            unit_command("stop_units", u(), None, &[], ""),
            Some(CommandKind::StopUnits { .. })
        ));
        assert!(matches!(
            unit_command("retreat_units", u(), None, &[], ""),
            Some(CommandKind::RetreatUnits { .. })
        ));
        let line: Vec<[f32; 2]> = (0..40).map(|i| [i as f32, 0.0]).collect();
        match unit_command("dig_units", u(), None, &line, "") {
            Some(CommandKind::DigUnits { line, .. }) => assert_eq!(line.len(), MAX_DIG_POINTS),
            c => panic!("{c:?}"),
        }
        assert!(unit_command("dig_units", u(), None, &line[..1], "").is_none());
        assert!(matches!(
            unit_command("set_stance", u(), None, &[], "prone"),
            Some(CommandKind::SetStance {
                stance: StanceOrder::Prone,
                ..
            })
        ));
        assert!(unit_command("set_stance", u(), None, &[], "lie").is_none());
        assert!(matches!(
            unit_command("set_fire", u(), None, &[], "hold"),
            Some(CommandKind::SetFire {
                mode: FireMode::Hold,
                ..
            })
        ));
        assert!(unit_command("set_fire", u(), None, &[], "fire").is_none());
        assert!(matches!(
            unit_command("throw", u(), t, &[], "smoke"),
            Some(CommandKind::Throw {
                item: ItemKind::Smoke,
                ..
            })
        ));
        assert!(matches!(
            unit_command("throw_each", u(), t, &[], "frag"),
            Some(CommandKind::ThrowEach {
                item: ItemKind::Frag,
                ..
            })
        ));
        assert!(unit_command("throw", u(), t, &[], "rock").is_none());
        for s in ["auto", "stand", "crouch", "prone"] {
            assert_eq!(stance_order_name(stance_order(s).unwrap() as u8), s);
        }
        for k in ItemKind::ALL {
            assert_eq!(item_kind(item_name(k)), Some(k));
        }
    }

    #[test]
    fn vehicle_intents_become_commands() {
        let u = || vec![SimId(70), SimId(71)];
        let t = Some([300.0004, 512.0]);
        match vehicle_command("reverse_units", u(), t, None, "") {
            Some(CommandKind::ReverseUnits { units, target }) => {
                assert_eq!(units, u());
                assert_eq!(target, sim_to_fx([300.0004, 512.0]));
            }
            c => panic!("{c:?}"),
        }
        assert!(matches!(
            vehicle_command("face_units", u(), t, None, ""),
            Some(CommandKind::FaceUnits { .. })
        ));
        assert!(vehicle_command("face_units", u(), None, None, "").is_none());
        assert!(matches!(
            vehicle_command("attack_unit", u(), None, Some(90), ""),
            Some(CommandKind::AttackUnit {
                target: SimId(90),
                ..
            })
        ));
        assert!(vehicle_command("attack_unit", u(), None, None, "").is_none());
        for (w, c) in [
            ("auto", AmmoChoice::Auto),
            ("ap", AmmoChoice::Ap),
            ("he", AmmoChoice::He),
            ("apcr", AmmoChoice::Apcr),
        ] {
            match vehicle_command("set_ammo", u(), None, None, w) {
                Some(CommandKind::SetAmmo { ammo, .. }) => assert_eq!(ammo, c),
                c => panic!("{c:?}"),
            }
            assert_eq!(ammo_choice_name(c as u8), w);
        }
        assert!(vehicle_command("set_ammo", u(), None, None, "heat").is_none());
        assert!(matches!(
            vehicle_command("repair_units", u(), None, None, ""),
            Some(CommandKind::RepairUnits { .. })
        ));
        assert!(matches!(
            vehicle_command("follow_units", u(), None, Some(70), ""),
            Some(CommandKind::FollowUnits {
                vehicle: SimId(70),
                ..
            })
        ));
        assert!(vehicle_command("move_units", u(), t, None, "").is_none());
        // Dig profiles: AT ditch by name, the trench otherwise.
        let line = [[0.0, 0.0], [10.0, 0.0]];
        for (w, p) in [
            ("at_ditch", DigProfile::AtDitch),
            ("trench", DigProfile::Trench),
            ("", DigProfile::Trench),
        ] {
            match unit_command("dig_units", u(), None, &line, w) {
                Some(CommandKind::DigUnits { profile, .. }) => assert_eq!(profile, p),
                c => panic!("{c:?}"),
            }
            match dig_command(3, &line, dig_profile(w)) {
                CommandKind::Dig { profile, .. } => assert_eq!(profile, p),
                c => panic!("{c:?}"),
            }
        }
    }

    #[test]
    fn long_polylines_are_capped() {
        let line: Vec<[f32; 2]> = (0..40).map(|i| [i as f32, 0.0]).collect();
        let c = cap_points(&line);
        assert_eq!(c.len(), MAX_DIG_POINTS);
        assert_eq!(c[0], [0.0, 0.0]);
        assert_eq!(c[MAX_DIG_POINTS - 1], [39.0, 0.0]);
    }

    /// S10a: the fortification intents become their commands for the first
    /// able squad; unable squads and missing input drop the intent.
    #[test]
    fn fortify_intents_make_commands() {
        use crate::match_setup::{MenuChoice, find_mods_dir, mods_candidates, resolve};
        use frontline_content::setup::SupplyKind;
        let mods = find_mods_dir(&mods_candidates(None, None)).unwrap();
        let setup = resolve(&mods, &MenuChoice::default()).unwrap();
        let ty = |id: &str| setup.squad_types.iter().position(|t| t.id == id).unwrap() as u16;
        let rifles = ty("ww2:ger_grenadiergruppe");
        let eng = ty("ww2:ger_engineer_squad");
        let sturm = ty("ww2:ger_sturmpioniergruppe");
        let bau = ty("ww2:ger_baupioniergruppe");
        let wall = fort_index(&setup, "sandbag_wall").unwrap();
        let nest = fort_index(&setup, "ww2:mg_nest").unwrap();
        assert_eq!(fort_index(&setup, "mg_nest"), Some(nest));
        // S10b ships a bunker; an unknown blueprint resolves to none.
        assert_eq!(fort_index(&setup, "pillbox"), None);
        for what in ["barbed_wire", "dugout", "hq_dugout"] {
            assert!(fort_index(&setup, what).is_some(), "{what}");
        }
        // A wall line goes to the first able squad (rifles cannot build).
        let line = [[10.0, 20.0], [14.0, 20.0], [18.0, 22.0]];
        match fortify_command(
            &setup,
            &[(1, rifles), (2, eng), (3, bau)],
            "sandbag_wall",
            &line,
            None,
            None,
        ) {
            Some(CommandKind::Fortify {
                squad,
                fort,
                line: l,
            }) => {
                assert_eq!((squad, fort), (SquadId(2), wall));
                assert_eq!(l.len(), 3);
                assert_eq!(l[2], sim_to_fx([18.0, 22.0]));
            }
            other => panic!("{other:?}"),
        }
        assert!(
            fortify_command(
                &setup,
                &[(1, rifles), (4, sturm)],
                "sandbag_wall",
                &line,
                None,
                None
            )
            .is_none()
        );
        assert!(
            fortify_command(&setup, &[(2, eng)], "sandbag_wall", &line[..1], None, None).is_none()
        );
        // A long drag is capped to 16 points.
        let long: Vec<[f32; 2]> = (0..40).map(|i| [i as f32, 0.0]).collect();
        match fortify_command(&setup, &[(2, eng)], "barbed_wire", &long, None, None) {
            Some(CommandKind::Fortify { line, .. }) => assert_eq!(line.len(), MAX_DIG_POINTS),
            other => panic!("{other:?}"),
        }
        // A point blueprint: [pos, pos + unit facing].
        match fortify_command(
            &setup,
            &[(3, bau)],
            "mg_nest",
            &[],
            Some([50.0, 60.0]),
            Some([0.0, 4.0]),
        ) {
            Some(CommandKind::Fortify { squad, fort, line }) => {
                assert_eq!((squad, fort), (SquadId(3), nest));
                assert_eq!(line, vec![sim_to_fx([50.0, 60.0]), sim_to_fx([50.0, 61.0])]);
            }
            other => panic!("{other:?}"),
        }
        assert!(fortify_command(&setup, &[(3, bau)], "mg_nest", &[], None, None).is_none());
        // Materials: every able squad, a full load by default.
        let per_man = setup.rules.fortify.as_ref().unwrap().carry_kg_per_man;
        let cmds =
            load_materials_commands(&setup, &[(1, rifles, 9), (2, eng, 6), (3, bau, 8)], None);
        assert_eq!(
            cmds,
            vec![
                CommandKind::LoadMaterials {
                    squad: SquadId(2),
                    kg: (per_man * 6) as u16
                },
                CommandKind::LoadMaterials {
                    squad: SquadId(3),
                    kg: (per_man * 8) as u16
                },
            ]
        );
        assert_eq!(
            load_materials_commands(&setup, &[(2, eng, 6)], Some(30)),
            vec![CommandKind::LoadMaterials {
                squad: SquadId(2),
                kg: 30
            }]
        );
        // A supply truck fills its room (the sim clamps u16::MAX kg); a
        // tank does not load.
        let truck = ty("ww2:ger_opel_blitz_squad");
        let tank = ty("ww2:ger_pz4_h_squad");
        assert!(is_supply_truck(&setup, truck) && !is_supply_truck(&setup, tank));
        assert_eq!(
            load_materials_commands(&setup, &[(7, truck, 1), (8, tank, 1)], None),
            vec![CommandKind::LoadMaterials {
                squad: SquadId(7),
                kg: u16::MAX
            }]
        );
        // A point blueprint at the map's west edge facing west: the facing
        // point turns round onto the map; in a corner it is clamped.
        let size = setup.map.size_m as f32;
        for (p, f) in [([0.5, 60.0], [-1.0, 0.0]), ([0.0, 0.0], [-1.0, -1.0])] {
            match fortify_command(&setup, &[(3, bau)], "mg_nest", &[], Some(p), Some(f)) {
                Some(CommandKind::Fortify { line, .. }) => {
                    let lim = sim_to_fx([size, size]);
                    assert!(
                        line.iter().all(|q| q.x >= Fx::ZERO
                            && q.y >= Fx::ZERO
                            && q.x <= lim.x
                            && q.y <= lim.y),
                        "{line:?}"
                    );
                }
                other => panic!("{other:?}"),
            }
        }
        // Cutting: the first clear_wire squad.
        assert_eq!(
            cut_wire_command(&setup, &[(1, rifles), (4, sturm), (2, eng)], [5.0, 6.0]),
            Some(CommandKind::CutWire {
                squad: SquadId(4),
                at: sim_to_fx([5.0, 6.0])
            })
        );
        assert!(cut_wire_command(&setup, &[(1, rifles)], [5.0, 6.0]).is_none());
        // Bangalore: the breach squad's men, sorted, if one carries a charge.
        let men = |id: u32| match id {
            4 => vec![(41, 0), (40, 1), (42, 0)],
            5 => vec![(50, 0)],
            _ => vec![(10, 1)],
        };
        assert_eq!(
            bangalore_command(&setup, &[(1, rifles), (4, sturm)], men, [7.0, 8.0]),
            Some(CommandKind::PlaceBangalore {
                units: vec![SimId(40), SimId(41), SimId(42)],
                target: sim_to_fx([7.0, 8.0]),
            })
        );
        assert!(bangalore_command(&setup, &[(5, sturm)], men, [7.0, 8.0]).is_none());
        assert!(bangalore_command(&setup, &[(1, rifles), (2, eng)], men, [7.0, 8.0]).is_none());
        // A construction crate.
        assert_eq!(supply_kind("construction"), Some(SupplyKind::Construction));
        assert_eq!(supply_kind("shells"), Some(SupplyKind::Shells));
        assert_eq!(crate_kind_name(3), "construction");
        assert_eq!(crate_kind_name(1), "explosives");
    }

    #[test]
    fn s10b_intents_make_commands() {
        // Control: map `demolish` to `CutWire` in `demolish_command` -> the
        // Demolish assertion fails.
        use crate::match_setup::{MenuChoice, find_mods_dir, mods_candidates, resolve};
        let mods = find_mods_dir(&mods_candidates(None, None)).unwrap();
        let setup = resolve(&mods, &MenuChoice::default()).unwrap();
        let ty = |id: &str| setup.squad_types.iter().position(|t| t.id == id).unwrap() as u16;
        let rifles = ty("ww2:ger_grenadiergruppe");
        let eng = ty("ww2:ger_engineer_squad");
        let sturm = ty("ww2:ger_sturmpioniergruppe");
        let bau = ty("ww2:ger_baupioniergruppe");
        // Mines and obstacles go through `fortify`: lines for the fields and
        // the log barrier, points for the hedgehog and the bunker.
        let line = [[10.0, 20.0], [30.0, 20.0]];
        for (what, squads, want) in [
            ("ap_minefield", vec![(1, rifles), (4, sturm), (3, bau)], 3),
            ("at_minefield", vec![(2, eng), (3, bau)], 2),
            ("log_barrier", vec![(1, rifles), (2, eng)], 2),
        ] {
            let f = fort_index(&setup, what).expect(what);
            assert_eq!(
                fortify_command(&setup, &squads, what, &line, None, None),
                Some(CommandKind::Fortify {
                    squad: SquadId(want),
                    fort: f,
                    line: line.iter().copied().map(sim_to_fx).collect(),
                }),
                "{what}"
            );
            assert!(fortify_command(&setup, &[(1, rifles)], what, &line, None, None).is_none());
        }
        for what in ["hedgehog", "bunker"] {
            let f = fort_index(&setup, what).expect(what);
            let pt = Some([50.0, 60.0]);
            match fortify_command(&setup, &[(1, rifles), (3, bau)], what, &[], pt, None) {
                Some(CommandKind::Fortify { squad, fort, line }) => {
                    assert_eq!((squad, fort, line.len()), (SquadId(3), f, 2), "{what}");
                    assert_eq!(line[0], sim_to_fx([50.0, 60.0]));
                }
                other => panic!("{what}: {other:?}"),
            }
        }
        // Sweeping: the first clear_mines squad (assault engineers can).
        let lane = [[5.0, 6.0], [5.0, 26.0]];
        assert_eq!(
            clear_mines_command(&setup, &[(1, rifles), (4, sturm), (2, eng)], &lane),
            Some(CommandKind::ClearMines {
                squad: SquadId(4),
                a: sim_to_fx(lane[0]),
                b: sim_to_fx(lane[1]),
            })
        );
        assert!(clear_mines_command(&setup, &[(1, rifles)], &lane).is_none());
        assert!(
            clear_mines_command(&setup, &[(4, sturm)], &lane[..1]).is_none(),
            "no drag"
        );
        // Demolition: the first demolish squad (construction engineers cannot).
        assert_eq!(
            demolish_command(&setup, &[(1, rifles), (3, bau), (2, eng)], [7.0, 8.0]),
            Some(CommandKind::Demolish {
                squad: SquadId(2),
                at: sim_to_fx([7.0, 8.0]),
            })
        );
        assert!(demolish_command(&setup, &[(1, rifles), (3, bau)], [7.0, 8.0]).is_none());
    }
}
