# S6b: mortar AI pass (log)

Owner choice 2026-11-05: "Mortar AI pass first" (before S7). Branch
`s6b-mortar-ai` from `55b322d` (S6 head `d355350` plus the infra uid commit),
not pushed. Spec `S6b-mortar-ai.md`; targets `targets/S6b.toml`. Commits:
`be43379` spec, `c2aa665` code, `886bba4` build log, `7b51028` Tune,
`529d134` fix, then this docs commit.

## Diagnosis (S6 binary, 40 and 160 probes; scratchpad `s6b/`)

- AI map fire starts 27-36 m off. Without it the same tubes fire fewer bombs
  but kill more (USA 9.9 -> 11.1 mortar kills per match on 40).
- Tube-plans on 160: on a mission 23 %, walking 34 %, held at the 30 %
  reserve with targets 24 % (mean 2.4 bombs left), idle or no observer 13-16 %.
- Crate runs start only below 50 %, from one bearer squad. Spotter missions
  are only 1.1-1.4 per match.
- USA buys fewer mortars only because it buys fewer squads (22.5 % of buys vs
  21.6 %). Its mortar kills per match were close to Germany's (10.4 vs 10.7).
- Tried, no change in kills per bomb (0.10-0.11): target scoring, and
  `mission_rounds` 4 or 8. More bombs did move the share.
- Rejected: a standing observation post. It cut Direct bombs 26 -> 16 per
  match and lowered both shares. Removed.

## Changes (both factions, same code; every value in TOML)

- **C1 map fire limit.** `[ai.mortar] map_fire_max_miss_m` (m; default 1000 =
  off; ww2 **20**). The AI holds map fire whose start error is above this
  (`fm_miss_holds`). The ww2 floor is 27 m, so 20 is an off switch for AI map
  fire. Players are unaffected.
- **C2 bomb load.** `[ai.ammo] mortar_reserve_pct` 30 -> **0** (a mission
  still needs 2 bombs); `[ai.supply] crate_run_below_pct` 50 -> **90**; new
  `[ai.supply] mortars_per_bearers` (0 = one bearer squad; ww2 **2**).
- **C3 dry orders withdrawn** (S5b-22). A due AI fire mission whose squad has
  no bombs is dropped (`fm_dry_drops`), not refused. On 160: 4 / 6 dropped,
  0 refused.
- State: `fm_miss_holds`, `fm_dry_drops` in `AiCounters`, saved, hashed,
  per-field tests. `SAVE_VERSION` 15 -> 16; `PROTOCOL_VERSION` 9 (no command
  change).
- CLI: `ai_missions_total` = missions sent; `ai_missions_planned` is the old
  number.
- Unchanged: blast, fragments, dispersion, damage, buys, `mission_rounds` 6,
  bomb load 20, prices.

## Tests

- `crates/sim/tests/s6b_ai_mortar.rs` (release-only, 4): map fire past the
  limit held; dry order withdrawn with no refusal; 2 bearer squads for 3
  mortars; determinism at 1/2/3/8 threads and across save, load, continue.
- `s5a_ai.rs` `all_dry_ai_vs_ai_fires_again`: 6 -> 7 min. One seed takes 388 s
  (341 s at S6 values): the walk back to base, not a stuck side. Accepted.
- `check.sh quick 55b322d` after the fix: 440 passed, 0 failed. Release AI
  tests all pass.

## Targets (shipped data: S6 prices)

| Run | Result |
|---|---|
| skirmish40 (seeds 1-20, both bases) | 12 pass, 0 miss. Refusals 0 / 0; wins 50 / 50 %; length median 17.6 min; HE 21.8 % |
| skirmish160 (seeds 1-80) | 31 pass, 0 miss. Germany 52.5 %; base 51.3 %; HE 21.9 %; mortar 14.2 / 12.0 % |
| skirmish160_oos (101-180) | 1 hard miss: T5d Germany 43.1 %; HE 21.5 %; mortar 14.0 / 11.3 %; base 59.4 % |
| third set (301-380) | 1 hard miss: Germany 41.9 %; HE 21.5 %; mortar 14.2 / 11.0 %; base 58.1 % |

The 160 and oos rows are from the Build run (same sim and data as shipped).
S6 on the same sets: mortar 11.6 / 9.2 % (USA miss), HE 19.8 / 19.1 %, Germany
50.6 / 51.9 %, base 64.4 % oos.

## Tune (reverted)

Build's T5d miss led to a German card price multiplier (x0.99; solved
0.992 [0.970-1.021]). Its own checks read Germany 43.1 / 55.6 %, so it moved
the miss from one hard target to two. Review (MEDIUM): out of scope, and the
interval includes 1.00. Reverted in `529d134`. The Tune gave no usable lever.

## Review and fixes

- MEDIUM: the price change above. Reverted.
- LOW: stray `variants.rs-e` (a sed backup). Removed.
- LOW: dry orders counted as missions in the CLI total. Fixed in `json_out.rs`.
- LOW: `mortar_reserve_pct` 0 drops the defence override. Not fixed: a 10 %
  reserve cut mortar share (12.8 / 8.6 -> 11.9 / 8.1). Owner note, S6b-2.
- Checked clean: determinism, no floats or HashMap, no faction branch, dry
  check matches the sim's NoBombs check.

## Known gaps

- **T5 not settled.** Germany: 52.5 % (1-80), 43.1 % (101-180), 41.9 %
  (301-380). Pooled 45.8 % over 480. Owner question S6b-1.
- Germany's mortar share 13.6-14.5 % (cap 15 %); map bombs still 28 % of bombs.
- Cook-offs 0.05 -> 0.19 per match; crates lost 16 / 6 -> 54 / 21 (G / U).
- Not run: `timing40`, armour-test, fortify-test, Godot checks, `check.sh full`.

## What S7 must know

- Raw outputs: `/private/tmp/claude-501/-Users-evarnimtb-Documents-Claude/21d3d3cd-8805-4e7e-9ecf-7b70b7e7aea7/scratchpad/s6b/`.
- Prices are S6 values. A German price lever exists for T5 but is out of
  scope; decide per S6b-1.
- AI map fire is off in ww2. Player map fire is unchanged.
- The observation post is gone. A forward observer must not take the tube's
  Direct role (measured).
- Reserve 0 means no bombs kept for defence.
