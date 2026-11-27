# S14 presenter P: flame stream, burning ground, burning men, strike pose, Clear trench button, Strike key `V`

Read first: `S14-assault-kit.md` sections 1, 3, 5, 6, 11 row P; `S14-log.md` "For the next stage" ("Presenter needs");
`godot/docs/match_state.md` "S14"; the S12 presenter pass as the pattern (`godot/match/wound_view.gd`, `git log --oneline --grep S12
-- godot | head`). Branch: the worktree's own branch from `claude/project-thread-aj1tww` (merged back by the orchestrator). **1 chunk.**
Owner rule (2026-11-25): no balance runs, no tuning. Do not touch `crates/sim/` rules or `mods/`.

1. Bridge (`crates/godot-bridge/`): send the snapshot fields `flame[]`, `patches[]`, `soldier_burning[]`, `soldier_strike[]` to Godot
   (pattern: how S12 sends wound state); add intent `clear_trench {squad, at}` -> `Command::ClearTrench`; tests as the bridge's existing
   intent and snapshot tests. Keep `godot/docs/match_state.md` "S14" exact.
2. Godot (`godot/match/`): a `flame_view.gd` (stream particles as short bright billboards or meshes, patches as flickering ground
   decals with a light, burning men tinted/with a small flame), the strike pose (wind-up and recovery: a short lunge/arm motion, or a
   tint if the model has no rig), a Clear trench command button on infantry squads (click a trench point; refused orders show the
   existing `fort_refused` feedback) and the `V` key for `strike_units` on the selected men. Everything placeholder-quality, cheap,
   and hidden when the fields are empty.
3. The mock (`godot/` mock data used by the headless smoke) shows one of each; headless smoke with Godot 4.7.2 (`target/godot/` has
   the binary; see how earlier stages ran it, e.g. `grep -rn "headless" tools/ docs/process | head`) passes with no new errors.
4. `cargo test -p` the bridge crate, `tools/check.sh quick a27ce63` through `tools/run.sh` (IPv6 test excepted). Commit
   (`git commit -m "S14 P: ..." -- FILES`; never `git add -A`, never `target/`, `.claude/`, `tools/__pycache__/`,
   `godot/match/fort_view.gd.uid`; do not push; no model names in files or commits). Append a short "Presenter" section to
   `docs/slices/S14-log.md` (do not rewrite the "For the next stage" block).
