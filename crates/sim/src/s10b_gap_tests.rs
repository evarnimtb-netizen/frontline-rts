//! S10b-ai Build E item 3 (owner, closes S10b-ai-D3): an obstacle piece
//! blocks the vehicle nodes within its `piece_half` box (at least half a
//! grid step each way, no wreck pad, no dilation), pieces at most
//! `PIECE_BRIDGE_M` apart close the ground between them, and a one-node gap
//! between piece nodes is closed. So an intact 4 m hedgehog row stops every
//! vehicle, one piece gone stays closed, two adjacent gone open a medium
//! tank's way, and a gap the path layer serves is one the tank drives.

use super::s6_tests::{flat_world_with, members, run, spawn, steps};
use super::*;
use crate::game::SquadId;
use crate::vehicle::{BlockLayer, Footprint};

const HZ: u64 = 30;
const TANK: &str = "ww2:ger_pz4_h_squad";
const TRUCK: &str = "ww2:ger_opel_blitz_squad";

fn fort(w: &World, id: &str) -> u16 {
    w.match_setup()
        .unwrap()
        .forts
        .iter()
        .position(|f| f.id == id)
        .unwrap() as u16
}

/// A row of `fort` pieces through (520, 500) along `d` (unit), 4 m apart,
/// shifted `shift_mm` along it, `skip` (indices -10..=10) left out.
fn row(w: &mut World, id: &str, d: FxVec2, shift_mm: i32, skip: &[i32]) {
    let f = fort(w, id);
    let c = FxVec2::from_int(520, 500);
    for k in -10..=10 {
        if skip.contains(&k) {
            continue;
        }
        let along = Fx::from_milli(4000 * k + shift_mm);
        let p = c + d * along;
        assert!(w.build_now(f, &[p, p + d], PlayerId(0)), "piece {k}");
    }
}

/// Whether the vehicle of squad `card`, at (470, 500) ordered to (570,
/// 500), crosses the row through (520, 500) along `d` within `secs`; and
/// whether it ever stood on a blocked node. Round the row's end does not
/// count.
fn crosses(w: &mut World, card: &str, d: FxVec2, secs: u64) -> (bool, bool) {
    let sq: SquadId = spawn(w, 1, card, 470, 500);
    let v = members(w, sq)[0];
    run(
        w,
        1,
        1,
        CommandKind::Move {
            squad: sq,
            target: FxVec2::from_int(570, 500),
        },
    );
    let n = d.perp();
    let c = FxVec2::from_int(520, 500);
    let side = |w: &World| {
        let p = w.state.entities.transform[v.index()].pos.xy();
        (p - c).dot(n) > Fx::ZERO
    };
    let mut was = side(w);
    let mut stood = false;
    for _ in 0..HZ * secs {
        steps(w, 1);
        stood |= w.game().unwrap().vehicles[&v].on_blocked;
        let p = w.state.entities.transform[v.index()].pos.xy();
        let now = side(w);
        if now != was {
            // Through the row, not round its end.
            if (p - c).dot(d).abs() < Fx::from_int(20) {
                return (true, stood);
            }
            was = now;
        }
    }
    (false, stood)
}

fn diag() -> FxVec2 {
    FxVec2::new(Fx::from_milli(707), Fx::from_milli(707)).normalize_or_zero()
}

/// An intact 4 m hedgehog row stops the tracked and the wheeled vehicle,
/// across their way and diagonal, at three offsets. Control (code): piece
/// boxes at their own half extents (no 2 m minimum) and `PIECE_BRIDGE_M` 0
/// -> the hedgehogs' own boxes leave free nodes and the tank crosses
/// at 1300 mm (seen).
#[test]
fn intact_hedgehog_row_stops_every_vehicle() {
    let across = FxVec2::from_int(0, 1);
    for card in [TANK, TRUCK] {
        for d in [across, diag()] {
            for shift in [0, 1300, 2000] {
                let mut w = flat_world_with(1, |_| {});
                row(&mut w, "ww2:hedgehog", d, shift, &[]);
                let (crossed, _) = crosses(&mut w, card, d, 90);
                assert!(!crossed, "{card} crossed the row {d:?} at {shift} mm");
            }
        }
    }
}

/// Fix 23: a grazing approach. The vehicle starts `off` m beside an intact
/// 4 m hedgehog row (through (520, 500) along `d`), 30 m before its middle,
/// and is ordered 30 m past the middle and 10 m to the row's other side, so
/// its way runs along the row and then turns into it (piece_near, escape
/// mode, a repath from a piece node). It never crosses through the row
/// (round its end does not count) at any offset, across its way and
/// diagonal, tracked and wheeled. Control (code): the row left out -> it
/// crosses (checked here, so the test can see a crossing).
#[test]
fn intact_row_holds_a_grazing_approach() {
    let along = FxVec2::from_int(1, 0);
    let cross_at = |d: FxVec2, card: &str, off: i32, shift: i32, built: bool| {
        let mut w = flat_world_with(1, |_| {});
        if built {
            row(&mut w, "ww2:hedgehog", d, shift, &[]);
        }
        let c = FxVec2::from_int(520, 500);
        let n = d.perp();
        let from = c - d * Fx::from_int(30) + n * Fx::from_int(off);
        let to = c + d * Fx::from_int(30) - n * Fx::from_int(10);
        let sq: SquadId = spawn(&mut w, 1, card, from.x.floor_int(), from.y.floor_int());
        let v = members(&w, sq)[0];
        run(
            &mut w,
            1,
            1,
            CommandKind::Move {
                squad: sq,
                target: to,
            },
        );
        let side = |w: &World| {
            let p = w.state.entities.transform[v.index()].pos.xy();
            (p - c).dot(n) > Fx::ZERO
        };
        let mut was = side(&w);
        for _ in 0..HZ * 90 {
            steps(&mut w, 1);
            let p = w.state.entities.transform[v.index()].pos.xy();
            let now = side(&w);
            if now != was {
                if (p - c).dot(d).abs() < Fx::from_int(38) {
                    return true;
                }
                was = now;
            }
        }
        false
    };
    for card in [TANK, TRUCK] {
        for d in [along, diag()] {
            for off in [3, 5, 8] {
                for shift in [0, 2000] {
                    assert!(
                        !cross_at(d, card, off, shift, true),
                        "{card} crossed the row {d:?} from {off} m at {shift} mm"
                    );
                }
            }
        }
    }
    assert!(
        cross_at(along, TANK, 5, 0, false),
        "control: no row -> it crosses"
    );
}

/// Two adjacent hedgehogs gone from a 4 m row (12 m between the standing
/// centres) open it for a medium tank; one gone keeps it closed, three gone
/// always open it, and the tank never stands on a blocked node on its way
/// (the path layer serves only a gap it can drive: the fault of 7e692e3).
/// Measured over 8 offsets: across its way two gone open at all but the one
/// where the free nodes are a single column (pieces half a grid step off
/// the nodes: 2000 mm); a diagonal row opens at 4 of 8 (the rest needs a
/// third piece). Control: `PIECE_BRIDGE_M` 14 -> two gone stay closed
/// (seen).
#[test]
fn two_gone_open_a_hedgehog_row_for_a_medium_tank() {
    let across = FxVec2::from_int(0, 1);
    for d in [across, diag()] {
        let mut opened = 0;
        for shift in (0..4000).step_by(500) {
            let mut res = [false; 3];
            for (k, skip) in [&[0][..], &[0, 1], &[-1, 0, 1]].iter().enumerate() {
                let mut w = flat_world_with(1, |_| {});
                row(&mut w, "ww2:hedgehog", d, shift, skip);
                let (crossed, stood) = crosses(&mut w, TANK, d, 120);
                let n = k + 1;
                assert!(!stood, "{n} gone, {d:?} at {shift} mm: on a blocked node");
                res[k] = crossed;
            }
            assert!(!res[0], "one gone, {d:?} at {shift} mm: open");
            assert!(res[2], "three gone, {d:?} at {shift} mm: closed");
            if d == across && shift != 2000 {
                assert!(res[1], "two gone across at {shift} mm: closed");
            }
            opened += usize::from(res[1]);
        }
        assert!(opened >= 4, "two gone, {d:?}: open at {opened} of 8");
    }
}

/// The log barrier still stops a tank (its pieces take the same rule), and
/// a wreck still blocks its footprint plus `wreck_pad_m`, dilated a node.
/// Control: `wreck_pad_m` 0 -> the node 8 m off the wreck's centre line is
/// free (seen); the barrier as the hedgehog control above.
#[test]
fn log_barrier_and_wreck_pad_still_block() {
    let mut w = flat_world_with(1, |_| {});
    row(&mut w, "ww2:log_barrier", FxVec2::from_int(0, 1), 0, &[]);
    let (crossed, _) = crosses(&mut w, TANK, FxVec2::from_int(0, 1), 90);
    assert!(!crossed, "a tank crossed the log barrier row");
    let w = flat_world_with(1, |_| {});
    let t = w.terrain();
    let path = &w.match_setup().unwrap().rules.vehicles.path;
    let wreck = Footprint {
        center: FxVec2::from_int(200, 200),
        fwd: FxVec2::from_int(1, 0),
        half_x: Fx::from_int(3),
        half_y: Fx::from_int(2),
    };
    let mut b = BlockLayer::new(t.dim(), 19);
    b.mark_all();
    b.update(t, &[], &[wreck], &[], path);
    // Raw within 2 + 2 m (nodes 196..204), dilated a node: 208 blocked;
    // without the pad only node 200 is raw and 208 is free.
    let off = FxVec2::from_int(200, 208);
    assert!(b.blocked_at(t, off), "the wreck pad no longer blocks");
}
