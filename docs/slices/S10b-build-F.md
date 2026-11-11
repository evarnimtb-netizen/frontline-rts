# S10b Build F: presenter, bridge, net, HUD, docs; full check

Read first: `S10b-obstacles.md` 1, 3, 4 (R13, R27), 7; the log's top block; `godot/docs/match_state.md` "S10a".
About 60 turns. No layout change (the snapshot is not saved).

## 1. Snapshot (`crates/sim/src/snapshot.rs`)
* `FortSnapshot::mines: Vec<MineSnapshot { id, owner, fort, pos, known: u8 }>` (ascending id, every mine; hiding is
  the presenter's job, design 7.6 "a presentation rule"), `FortChargeSnapshot::piece`, `FortPieceSnapshot` already
  has `fort` (the presenter reads `blocks_vehicles` and part from the def list).
* `SquadSnapshot` order kinds `clear_mines` (with `a`, `b`, `along`) and `demolish` (target piece).
* `fort_refusals` carries `NoTarget`. Test `fort_snapshot_lists_mines_with_known_bits`: an enemy mine shows its
  `known` 0 until found. Control: fill `known` with all bits.

## 2. Bridge and HUD (`crates/godot-bridge/src/`, `godot/match/`, `godot/ui/`)
* Intents: `fortify` with `what` in `ap_minefield`, `at_minefield`, `hedgehog`, `log_barrier`, `bunker` (lines
  dragged as wire; points as the MG nest); `clear_mines` (`a`, `b`, dragged); `demolish` (`target`). First
  selected able squad, as `build`. Test `s10b_intents_make_commands`. Control: map `demolish` to `CutWire`.
* `orders_grid.gd`: build-menu entries `lay_ap_mines`, `lay_at_mines`, `build_hedgehog`, `build_log_barrier`,
  `build_bunker`, `clear_mines`, `demolish`, shown only to able squads (caps R27, half-rate note for R21);
  tooltips with kg, time, cap; refusals in the event feed.
* `godot/match/fort_view.gd`: mines as small discs (AP grey, AT dark green), drawn only when the mine's owner is on
  the local player's team or its `known` bit for the local player is set; hedgehog as three crossed bars; log
  segments as brown boxes; bunker walls timber, roof earth-brown, a dark slit; the sweep lane as a dashed line
  with its `along` mark; a demolition charge as the satchel marker until it bursts.
* Godot headless smoke of the mock (as S10a's), and the Windows-safe check the presenter tests use.

## 3. Net (`crates/net/`)
* The fuzz test round-trips `ClearMines` and `Demolish`; `command_in_limits` arms from A tested at the limits
  (`net_s10b_commands_in_limits`; control: lane length + 1 m accepted). `sdk/docs/net/relay-protocol.md` lists
  both commands and PROTOCOL 14.

## 4. Docs
* `docs/modding/fortifications.md`: a "S10b" section: minefield defs (`mine_item`), `kind = "mine"` items and
  `[mine]`, `blocks_vehicles`, six pieces, the bunker's slit, the new `[fortify]` keys with values, the three
  capabilities and who has them, the rules R3-R13 and R19-R21 in plain words, what the AI does (nothing yet).
* `godot/docs/match_state.md` "S10b"; `docs/STATE.md` (S10b row, versions SAVE 24 / PROTOCOL 14, measures);
  `docs/TODO.md` (owner questions S10b-1..4 with the slice's answers on (A)).

## 5. Ends when
* `tools/check.sh full` all ok in the background (`tools/run.sh`, polled): every test of B-F passes, ignored ones
  included; `cargo fmt` applied.
* 40-set `neutral.py` same 40/40; `frontline-cli targets docs/slices/targets/S10b.toml --runs fortify10` with the
  E table unchanged.

## 6. Hand-over to the orchestrator (the log's top block)
Run `tools/measure.sh target/s10b-measure docs/slices/targets/S10b.toml` (480 matches), then
`python3 -I tools/neutral.py target/s10a-ai-verify target/s10b-measure` (expect `same` on all four sets and every
old fortify and armour row), `python3 -I tools/pool.py target/s10b-measure` (Germany 41.67 %), the targets (hard
misses exactly T8b, T40c, T54e, T88b, T88d plus any T102-T107 miss from E), then `tools/verify.sh`. The Review gate
fires (sim, save, protocol): Review, then Fix. List anything left for S10b-ai.
