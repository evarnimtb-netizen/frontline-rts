# S15 presenter P: third faction in menus and lobby, USSR look, blockouts, riders on deck, plate mark

Read first: `S15-ussr.md` sections 4 (matchups), 8; `S15-log.md` "For the next stage"; `godot/docs/match_state.md` "S15"; the S13
presenter pass (blockout recipe; `git log --oneline --grep "S13" -- godot | head`) and the S14 presenter (`godot/match/flame_view.gd`).
Branch: the worktree's own branch from `claude/project-thread-aj1tww` (the orchestrator merges it back). **1 chunk.**
Owner rule: no balance runs, no tuning. Do not touch `crates/sim/` rules, `crates/sim/src/ai/`, `mods/` defs or `json_out.rs`.

1. Bridge (`crates/godot-bridge/`): send the S15 snapshot fields (vehicle `DECK`/`RIDERS_OFF` flags, riders and their seats,
   `soldier_plate`) and the setup's factions and allowed pairs (plan M3); tests as the bridge's existing ones. Keep
   `godot/docs/match_state.md` "S15" exact.
2. Godot: faction buttons and an opponent picker from the setup info (with the USSR off the menu is today's menu); USSR colour
   `#9a5a4a`, name, short name `SOV`; weapon names; vehicle paint and glacis; 12 blockouts by the S13 recipe for the Soviet vehicles
   and guns; riders drawn seated on the deck boxes; a plate mark on the squad card; lobby text for a refused pair. Placeholder quality.
3. Mocks with three factions; `test_net_ui.gd` and the match view test pass headless with Godot 4.7.2 (binary under
   /home/user/frontline-rts/target/godot/ if the worktree has none).
4. `cargo test -p` the bridge crate; `tools/check.sh quick 96d4b4b` through `tools/run.sh`. Commit (`git commit -m "S15 P: ..." --
   FILES`; never `git add -A`, never `target/`, `.claude/`, `tools/__pycache__/`, `godot/match/*.gd.uid`; do not push; no model names
   in files or commits). Append a short "Presenter" section to `docs/slices/S15-log.md` (do not rewrite "For the next stage").
