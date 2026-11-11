# S10b log (mechanics: mines, AT obstacles, bunker; at most 150 lines)

Spec: common `docs/slices/S10b-obstacles.md`, stages `S10b-build-A.md` ... `-F.md`; targets `targets/S10b.toml`.
Branch `s10b-obstacles` from main `8acc534`. Two other agents may work in this checkout: commit only your files.

## For the next stage (rewritten by each stage, not appended; at most 30 lines)

**Next: Build A** (`S10b-build-A.md`): layout commit (SAVE 23 -> 24, PROTOCOL 13 -> 14, once), defs and items,
`[fortify]` keys, capabilities 12-14, the AI `fort_of` guard (R29), `s10b` CLI counters, fortify `s10b` row
scaffolding, `target/s10b-base40`. No behaviour yet; `ClearMines` and `Demolish` refused until C and D.
* Neutrality base: `target/s10a-ai-verify` (main's code; four sets, fortify10, armour). 40-only base:
  `target/s10b-base40/skirmish40.json` (copy it first; `neutral.py` fails on a 40-only run against a four-set dir).
* Targets file parses: on the base JSON, 331 pass; old hard misses are exactly T8b, T40c, T54e, T88b, T88d; T102-T108
  read "no such number" until filled.
* Known hazard: new blueprint ids sort before `sandbag_wall` and `dugout`, and `ai/fort.rs` `fort_of` takes the first
  blueprint by shape and pit: without the R29 guard the AI would lay a minefield as a "wall".
* Owner questions S10b-1..4 run on (A) (`docs/TODO.md`). Do not change armour, pen, guns, ammo, lethality, prices.
* Checks: `tools/check.sh quick 8acc534` while working; long jobs through `tools/run.sh` (200 s polls).

## Plan (2026-11-11)

Wrote the common spec (scope, data model, rules R1-R30, job ends, save rules, done-when), six stage files, the
targets (S10a-ai body; T5c/T5d 35-65; T31a-d, T55a/b, T95b/d reports; new T102-T108), this log, STATE and TODO.
Read: STATE, roster S10, infantry-structures 7.6/7.7, S10a spec, `fort.rs` outline, `ai/fort.rs` `fort_of`,
`path.rs`/`vehicle.rs` block layer, `damage.rs` `blast_tracks`, `combat.rs` `Burst`, `fort.rs` `take_charges`.
Design-code facts used: `burst` and `blast_tracks` never hurt own units (so S10b-1 (A)); vehicle boxes have no belly
plate (S10b-3 (A)); charges become `Thrown` items, so a mine is an item def `kind = "mine"`; the block layer already
takes wreck footprints, obstacles join them.
