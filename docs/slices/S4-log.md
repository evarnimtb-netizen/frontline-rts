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
