# S4-variants log

Spec: `docs/slices/S4-variants.md`. Baseline: the spec's "Baseline" section
and appendix (40 hashes at `63c3e36`).

## Part A: content crate (no behaviour change)

### A1 Baseline tests (before any loader change)

- `sdk/crates/content/tests/resolved_defs.rs`:
  - `setup_bytes_baseline`: xxh3 of the postcard bytes of six resolved
    `MatchSetup`s (skirmish both faction orders, the test-unlock rules, the
    example override, a human-vs-AI request, and an override that patches a
    unit, a weapon and the AI mix), recorded with the old loader.
  - `legacy_view_matches_snapshot`: every def of `mods/ww2` as JSON,
    written by the old loader to `tests/snapshots/ww2_resolved.json`. The
    new loader must keep every key and value in it (it may add keys).

### A2-A6 Content crate (commit after this log entry)

- `sdk/crates/content/src/resolve.rs` (new): raw TOML tables per kind
  (`RawDefs`, kept as `ModDefs::raw`), `extends` on all nine def kinds
  (units, weapons, items, factions, maps, rules, vehicles, ammo, training),
  chains up to `MAX_EXTENDS_DEPTH` = 4, cycles listed, unknown parent with
  did-you-mean (or "is a <kind> def"), tables merge / arrays and values
  replace, `id` and `abstract` not inherited. Unit rules: a level with
  `[[members]]` drops inherited legacy soldier keys; legacy keys over a
  `[[members]]` parent are an error; an alias (`recall_cooldown_s` /
  `cooldown_s`, `initial_cooldown_s` / `unlock_s`) set on a level drops the
  other inherited name, both in one file is an error. Issues name the
  child's file and "(from `<parent>`)" for inherited values.
- `sdk/crates/content/src/variant.rs` (new): `MemberDef` (slot, count,
  weapon, sidearm, ammo, items, traits), traits and capabilities with the
  reserved lists, tags (4 reserved), `TrainingDef`, the exact legacy
  expansion and its inverse projection (`project_members`).
- `SquadDef`: `role` optional (derived from tags, vehicle or weapons when
  absent; a card needs `role` or `tags`), card block (`cooldown_s`,
  `unlock_s`, `max_alive`, `limit_group`, `available_from/to`, `tags`,
  `capabilities`, `training`, `ai_weight`, `short_name`, `sources`,
  `abstract`), `[[members]]`. After loading every card carries its
  canonical view (members expanded, tags and capabilities filled, derived
  `barrage` / `at_hunt`, `cooldown_s` / `unlock_s`); the legacy keys stay as
  written.
- Rules: `[call_in] date`, `[training] default`, `[limit_groups]`,
  `[ai.mix]` beside the old `mix_*_pct` keys (exactly one form, 1:1
  mapping). `pop_cap` unchanged (120).
- `mods/ww2`: `defs/training/{conscript,regular,veteran,elite}.toml`;
  `skirmish.toml` gains `date = ""`, `[training] default =
  "ww2:training_regular"`, an empty `[limit_groups]`. No unit file changed.
- Lowering: legacy cards lower exactly as before; a `[[members]]` card is
  projected onto today's `SquadTypeSetup`. `ModDefs::resolve` refuses with
  "needs S4 Part B" what today's sim cannot run (non-projectable members,
  member sidearms, non-regular training, capabilities that differ from the
  role's, `max_alive`, `limit_group`, a vehicle card's own cost/cooldown,
  `[ai.mix]` tags beyond the six legacy ones). Date filter on rosters is
  live (skirmish has no date). Abstract units never reach `squad_types`.
- Overrides patch the raw tables and every def re-resolves (a patch on a
  parent reaches its variants; the change list shows each); a
  `[[members]]` patch replaces a legacy squad's soldier keys;
  `ModDefs::add_defs_dir` loads extra def folders (variants);
  `add_rules_override` rules are kept raw too.
- Example variant `mods/ww2/tests/variants/units/usa_rifle_squad_understrength.toml`
  (extends + members) and `tests/variants/roster.toml`; `validate` loads
  them; `crates/sim/tests/variants.rs` calls it in (spawns in the call-in
  tick, 9 men) and fights a German rifle squad to the end.
- Schemas: `training.schema.json` new; every def schema has `extends`
  (a file with `extends` needs only `id`, via `if`/`then`).
- Docs: `docs/modding/variants.md` (new), `docs/modding/tuning.md` rows,
  `sdk/docs/net/relay-protocol.md` (the content hash changed on purpose;
  protocol unchanged).

### A7 Measure (release build of the Part A tree)

| Check | Result |
|---|---|
| `setup_bytes_baseline` (6 setups) | equal to the old loader's |
| legacy snapshot (`ww2_resolved.json`) | every old key and value kept |
| skirmish 40 (seeds 1-20 × both bases, `--jobs 3 --minutes 45`) | all 40 hashes equal to the appendix; summary identical except tick timing (p99 peak 0.455 ms, quieter machine than the 0.733 baseline) |
| armour-test `--stress --seeds 10` | all hard checks pass; report identical to the Plan baseline except timing lines |
| fortify-test `--seeds 10` | PASS; report identical except timing |
| golden ballistics | 46/46 |
| `validate mods/ww2` | ok (with and without the example variant) |
| `cargo test --workspace --release -- --include-ignored` | 486 passed, 0 failed |
| clippy `-D warnings`, fmt | clean |
| Godot (bridge rebuilt) | `check_project` OK, `test_data_reload` 8/8, `test_match_view` 215/215; settings.cfg unchanged |

Changed on purpose: `ModDefs::content_hash` (new def fields and kinds),
the schema list (`training`), one test expectation for it
(`sdk/crates/cli/tests/simulate.rs`).
