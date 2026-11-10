# S10a log (mechanics half)

Spec `docs/slices/S10a-fortifications.md`. Branch `s10a-fortifications`, base `9753cd9`. At most 150 lines.

## Build A: layout, content, pieces, materials

**Layout commit `ccd82e2`** (the slice's one layout change): `SAVE_VERSION` 21 -> **22**, `PROTOCOL_VERSION` 12 ->
**13**. Every new field and variant of the spec: `MatchState::fort` (`fort.rs` `FortState`, `put_postcard` after
`supply6`, validated on load), `Order::{Fortify, CutWire}`, four commands, `SupplyKind::Construction`,
`ItemKind::Bangalore` (`ITEM_KINDS` 5), pits 6-7, `Capability` bits 7-11, `VehicleState::material_g`,
`MatchSetup::forts`, `ItemSetup::bangalore`, `RulesSetup::{mg_nest_pit, dugout_pit, fortify}`. `CutWire` and
`PlaceBangalore` are refused (`NotAllowed`) until B. The commit also holds content, `fort.rs` and materials.

**Content.** `defs/fortifications/` (new def kind; `FortDef`): the five blueprints of spec 3.1; `[fortify]` with
`[fortify.tiers]` (`FortTuning` is a plain struct; its keys are in `tuning::TABLES`); `mg_nest_pit`, `dugout_pit`;
`ger_/usa_construction_crate` (40 kg). No cards, rosters or engineer caps yet (B).

**Sim.** `Fortify` checks cap bit, pit digging, shape, slope (over 2 m), map, enemy base, `limit_per_player`,
spacing; `over_hq` snaps to an own built unroofed HQ within 5 m. Pit first (`order_dig`), then `Order::Fortify`:
kg drawn carried -> crates -> trucks, builders on a slot ring add tier `build_pct` (x0.3 suppressed, 0 pinned).
Pieces: `StaticRef::Piece` after the S6 boxes, `StaticBox::armour_mm` (0 for S6). Gates: bullet by
`armour::small_reach` vs muzzle range (`Blow::Static` carries it), shot `shot_piece_damage`, fragment by
`frag_pen_mm`; cover: `piece_between` in `burst`. HQ roof goes with its HQ. Staging `World::build_now`;
`World::edit_fort`, `World::fort_refusals`; test switch `MatchState::fort_off`.

**Materials.** `LoadMaterials` (own base; carried up to 20 kg/man, trucks within cap less cargo; paid through
`pay_micro`), `IssueCrate { Construction }`, truck `material_g`. Guarded sites: `issue_crate` (`pay`,
`ammo.refilled`, `crates_issued`, `crate_micro` skipped; `FortAccount::materials_loaded_g` instead), `stock_in`
(never enters a stock: `stock_g`, `stock_in_g`), `serve_men` sources, `order_drop_crates` (refused: `cargo_g`,
`truck_loaded_g`), `TruckSupplySetup::crates` (`from_name`, 3 kinds), `ALL`/`NAMES` stay 3 (CLI `crates_issued`).

**Existing tests changed by the spec.** `abilities.rs`, `ww2_mod.rs`: item arrays 5 wide. `variants.rs`: `breach`
is no longer reserved (now `demolish`). `s5a_supply.rs`: a `Construction` arm. BASELINE (`resolved_defs.rs`) and
canonical snapshot refreshed; `simbench/src/report.rs` holds no setup-hash baseline.

**Tests and neutrality.** 21/21 Build A tests in `crates/sim/src/s10a_tests.rs` (engineer caps set by the test
setup). `tools/neutral.py` + `test_neutral.py` (in `check.sh quick`); base vs base same; 40 set on `ccd82e2`:
`same 40/40`. Final 40 set: see the report line below.

**Left for B.** Wire (slow, path layer, crush, HE and fragment damage), `CutWire`, Bangalore charges (accept the
two commands; `fort_tick` has the hook), cards, engineer caps, items, content tests. `items.rs` has no static-box
code: a thrown item falls through a roof onto the pit floor; the satchel test passes, why was not checked.
