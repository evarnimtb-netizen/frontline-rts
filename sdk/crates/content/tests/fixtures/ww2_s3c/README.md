# Frozen S3c content (S4 legacy guard)

A copy of `mods/ww2` (`mod.toml` and `defs/`) as it stood before the S4
content switch (S4 Part B, step B6): the S1-S3c legacy unit files (`men`,
`weapon`, `weapon_men`, `sidearm`, `loadout`, `recall_cooldown_s`,
`initial_cooldown_s`). `tests/resolved_defs.rs` loads it to prove that every
legacy unit file still loads unchanged and resolves to the same def
(`snapshots/ww2_resolved.json`, never re-blessed) and that its lowered
setups keep their bytes. Never edit these files; the live content is
`mods/ww2`.
