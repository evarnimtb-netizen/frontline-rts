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
`same 40/40`; final 40 set on `fd20621`: `skirmish40.json: same 40/40`.

**Left for B.** Wire (slow, path layer, crush, HE and fragment damage), `CutWire`, Bangalore charges (accept the
two commands; `fort_tick` has the hook), cards, engineer caps, items, content tests. `items.rs` has no static-box
code: a thrown item falls through a roof onto the pit floor; the satchel test passes, why was not checked.

## Build B: wire, breaching, the engineer split

**No layout change** (SAVE 22, PROTOCOL 13 as Build A). Commits `144e5dd` (code, content), `4e1b578` (tests),
`f307578`, `03a9734` (fixes). Bangalore placing reuses `Kit::windup` (kind Bangalore, `place_ticks`); charge bursts
go through the item burst path as `Thrown` entries (keys `charge id x 8 + k`, new `Purpose::CHARGE_CRATER` 34 and
`CHARGE_FRAGMENT` 35); `CutWire` and `PlaceBangalore` now apply (no longer `NotAllowed`).

**Rules as built.** Wire slow: `fort::wire_pace` in squad and unit steering, per man, within `band/2` of a section.
Path layer: `FortState::wire_nodes` (nodes within band/2 + 2 m), built in `serve_paths` only when wire stands;
`path::find_foot_path` pays `wire_path_cost_pct` per wire node; smoothing never crosses wire the grid path avoided.
HE: `blast_wire` in `burst` (enemy sections, clear line to mid-height). Fragments: `frag_wire` per ray before its
stop, `frag_damage x falloff x wire_frag_pct`, ray flies on. Crush: moving hull rectangle vs section (`wire_tick`).
Cut: nearest man stands 1 m off the section's middle, works within 1.5 m, not while pinned. Counters: `wire_cut`
(cutter), `wire_crushed` (vehicle owner), `wire_blown` (Bangalore bursts only; other HE removes without a
counter), `bangalores` (charges laid). Any removal sets squads cutting that section to Idle.

**Numbers the tests measured.** Rifleman 3.00 m/s outside, 0.60 m/s (exactly 20 %) inside the band; through an
8 m `CutWire` gap and around a line through a 12 m gap at full speed. Cut times: field 300 ticks (10 s), assault 150
(5 s), construction 600 (20 s). Bangalore: laid within 2.5 m of its target, 3.00 m long, three bursts in one tick at
1/6, 3/6, 5/6 of it, 8 s after a 5 s placing; it removes the sections either side of the target (all counted
blown). A satchel removes every section within 3 m, none past 10 m; a stick grenade 3 m short damages the wire and
the men behind take identical hits with and without it. A Pz IV crushes >= 2 sections, none 10 m off its track.

**Cards.** Roster positions: Germany 17 `ger_sturmpioniergruppe`, 18 `ger_baupioniergruppe`; USA 15
`usa_assault_section` (all `ai_weight` 0). Squad types sort by id, so `ger_baupioniergruppe` is type 0 and every
other type index moved (neutrality unaffected). Items `ww2:gestreckte_ladung`, `ww2:m1a1_bangalore` (satchel [he]
with blast 150 cm, no trench cut; frag values copied from the satchel, verify). Pioniergruppe and Engineer Squad
gain `build_structure`, `place_wire`, `clear_wire`; Baupioniere `dig_rate_pct` 200 (verify).

**Existing tests changed.** `variants.rs`: the "every S4 card is regular" check also exempts the conscript
Baupioniergruppe (spec 3.4). Canonical snapshot re-blessed; `resolved_defs.rs` BASELINE (frozen fixture) and
`simbench/src/report.rs` unchanged (neither holds these values).

**Checks.** 32/32 s10a tests, 3/3 content tests; 40 set on `03a9734`: `skirmish40.json: same 40/40`.

**Left for C.** Snapshot/bridge/HUD/blockouts for wire, charges and the placing windup (`soldier_throwing` is true
while a carrier places); CLI `s10` counters and the fortify rows (`wire`, `roof`, `breach`); net fuzz for
`CutWire`/`PlaceBangalore`. Charges burst with `BlastKind::Frag` and kill class FRAG (no own kind yet). Known
limits: a carrier at the far end of a holding line can stall in his own squad's crowd (ORCA) on the way.
