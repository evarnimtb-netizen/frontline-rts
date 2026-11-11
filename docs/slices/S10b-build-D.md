# S10b Build D: AT obstacles and demolition

Read first: `S10b-obstacles.md` 4 (R14-R21, R27), 5 "Demolition", 6 (`Order::Demolish`, `Charge::piece`); the
log's top block. About 60 turns. No layout change.

## 1. Obstacles block vehicle nodes (`vehicle.rs`, `fort.rs`)
* `BlockLayer::update` takes the wreck footprints today; add the `blocks_vehicles` pieces as footprints (box half
  extents, yaw, plus `wreck_pad_m`) in ascending piece id, after the wrecks. `MatchState::mark_piece_block(id)`
  (as `mark_wreck_block`) when such a piece is added or removed; tracked and wheeled layers both. The derived layer
  is rebuilt on load (`rebuild_vehicle_derived`), so nothing new is saved.
* A vehicle path never enters a blocked node; a vehicle ordered across a closed line goes round an end or stops at
  its partial path (existing behaviour). Infantry are not touched (R17).
* R18 at `Fortify` for `blocks_vehicles` defs and the bunker: no vehicle hull within `vehicle_clear_m`.
* R21: a `build_heavy` blueprint accepts a `build_structure` squad; its builders add `build_pct x
  heavy_by_field_pct / 100`. A squad with both bits uses its own tier rate (Baupioniere 150 %).
* Counters: `obstacles_built` per finished hedgehog or log segment, `pieces_lost` as S10a.

## 2. Demolition (`command.rs`, `fort.rs`, `items.rs`)
* `Demolish { squad, at }`: own `demolish` squad (else `CannotBuild`); the nearest standing piece (any owner,
  ascending id on ties) within `demolish_reach_m` of `at` (else `NoTarget`); the nearest living member carrying an
  item of kind `satchel` (else `NotAllowed`). Sets `Order::Demolish { piece, man, work: 0 }`.
* The man walks (detached, as `PlaceBangalore`) to 1.5 m from the piece box; then `work` grows by 100 per tick
  (x0.3 suppressed, 0 pinned) to `demolish_place_s x 100 x hz`; then one satchel leaves his kit and a `Charge {
  from = to = the box point nearest him, item, piece: Some(id), fuse_tick: tick + demolish_fuse_s x hz }` is laid;
  `demolitions += 1`; the order ends (squad idle).
* Ends (spec 5): charge laid; piece gone before (`NoTarget` event, order ends); man dead or kit empty (order ends,
  nothing laid); another order.
* Burst (R20): when a charge with `piece: Some(id)` bursts, that piece takes `damage x demolish_pct / 100` instead
  of the S6 overpressure damage; every other piece, man, wire and mine the normal rules (C's mine hook included).
  A piece removed this way: `pieces_demolished += 1` for the charge owner.
* Cards (R27, `demolish`): `ger_engineer_squad`, `usa_engineer_squad`, `ger_sturmpioniergruppe` (each has a satchel).

## 3. Tests (in `s10b_tests.rs`; each with its failing control)
* `obstacle_blocks_vehicle_nodes`: a 30 m hedgehog line and a 20 m log line mark every node under them blocked; a
  tank path from one side to a point behind never crosses either line. Control: `blocks_vehicles = false`.
* `removed_obstacle_reopens_the_path`: destroy one hedgehog (`edit_fort`): the nodes clear in the same tick and the
  next path goes through. Control: skip `mark_piece_block` on removal.
* `infantry_walks_through_obstacles`: a man's speed across the line equals his speed off it (+-1 %). Control: give
  pieces the wire speed rule.
* `obstacle_refused_near_a_vehicle`: a hedgehog site 4 m from a halted tank: `BadSpot`. Control: `vehicle_clear_m` 0.
* `heavy_blueprint_at_half_rate_for_field_engineers`: US Engineers build a hedgehog in 2x the time of 100 %;
  Baupioniere in 1/1.5. Control: `heavy_by_field_pct` 100.
* `demolish_lays_a_satchel_and_destroys_a_hedgehog`: Pioniere demolish a hedgehog (hp 600): gone after the fuse,
  one satchel used, counters 1/1. Control: `demolish_pct` 100 -> it stands at 300.
* `demolish_needs_capability_and_satchel`: a rifle squad -> `CannotBuild`; an engineer squad with no satchel ->
  `NotAllowed`. Control: give the rifle squad the bit.
* `demolish_ends_when_charge_laid_or_target_gone`: target destroyed by test HE while walking -> order ends, satchel
  kept. Control: keep the order.
* `demolition_is_deterministic`: two runs plus a save/load mid-walk give equal hashes. Control: none needed beyond
  the hash compare (change a fuse by one tick in one run -> differs).

## 4. fortify-test rows (`fortify/s10b.rs`)
* `obstacle`: a 30 m hedgehog line (hedgehogs every 3 m) and separately a 20 m log line, each across the tank's
  straight route; the tank is ordered 60 m behind. `hedgehog_crossed_pct`, `log_crossed_pct` (hull centre crossed
  the line between its ends), `tank_reached_pct`; a rifle squad crosses: `infantry_speed_pct` (leader inside vs
  outside the line band).
* `demolish`: engineers (Pioniere / US Engineers) demolish the middle hedgehog; `destroyed_pct`, `charge_laid_s`;
  then the tank is ordered through: `gap_crossed_pct`.

## 5. Ends when
Tests pass; `check.sh quick 8acc534` green; rows filled; T105, T106 read; 40-set `neutral.py` same 40/40.

## 6. Hand-over
T105/T106 numbers, how many nodes a hedgehog blocks, gaps that let a tank through (hedgehog spacing), anything for E.
