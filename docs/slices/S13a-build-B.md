# S13a Build B brief (data stage)
Plan: `S13-heavy-wave.md` 4 and 6 ("B data"). Stage A hand-over: `S13-log.md` "For the next stage". Data only: no sim change, no
SAVE or PROTOCOL bump.

## Files
- 4 weapons (`defs/weapons`): `kwk42_l70`, `kwk36_l56`, `m1a1_76mm`, `m3_90mm` (range 600 m, fire reveal 300 % / 3 s, as the KwK 40).
- 11 ammo (`defs/ammo`): `75mm_pzgr3942`, `75mm_pzgr4042`, `75mm_sprgr42`, `88mm_pzgr39`, `88mm_pzgr40`, `88mm_sprgr`, `76mm_m62`,
  `76mm_m93`, `76mm_m42a1`, `90mm_m82`, `90mm_m71`. `slope_class` ("apcbc" / "apcr") on the AP rounds only; none on HE, none on old rounds.
- 5 vehicles (`defs/vehicles`): `ger_panther_g`, `ger_tiger_1`, `usa_m4a3_76w`, `usa_m36`, `usa_m4a3e2` (frames, seats, hatches,
  components copied from the nearest def; plates and racks per plan 4.2; Jumbo with the 75 mm M3, S13-5 (A)).
- 5 cards (`defs/units`), `ai_weight = 0`; `ai_heavy` on Panther, Tiger, Jumbo; `ai_overwatch` (vehicle def) on Panther, Tiger.
- `skirmish.toml`: `[limit_groups] "ww2:heavy" = 1` (cards Tiger, Jumbo); roster appends (Germany, USA) after S12's last entry.
- Append-sensitive tests, `resolved_defs.rs` baseline, two JSON snapshots (`BLESS=1`).

## Sources
Every value carries `sources` as the plan section 4 gives; (verify) values are looked up in `S13-research-history.md` and, where
that also carries (M) / verify, the def comment says so. Penetration rows are the plan's B&L 0 degree, 50 % rows.

## Checks
Content tests; `frontline-cli validate`; `tools/check.sh quick 6f7bda0`; default 40 set vs `target/s13-base40` (`neutral.py`: same
40/40, weight 0); T151a-g read with `frontline-cli ballistics`; armour-test 41-46 as reports only if cheap.
