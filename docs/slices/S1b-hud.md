# S1b HUD spec (from mock v2.1)

Mocks: hud-mock-v2.png, hud-mock-v2-1280.png (generator gen_hud_v2.py).

**Owner corrections that override the text below:**
- The minimap shows the **whole map, north-up**, as a rounded square, by default. A setting rotates it with the camera; only then is it drawn as a disc with an off-map hatch. Drop the radar mode that centres on the camera.
- **Minimap (owner, 2026-11-01):** rotating with the camera is now the default. Drop the disc and its hatch: only the turned map square is drawn, floating over the world, and clicks in the empty corners reach the world. The lock button follows the turned map ("follow the map"): it hugs the map's bottom-right corner or edge, sliding smoothly as the camera turns, always inside the box and off the map. North-up it stays in the box's bottom-right corner.
- The A–E capture chips go in the top bar (done in the mock).
- **Hotkeys (owner, 2026-10-31):**
  - Stance uses the direct-control keys: X stand, C crouch, Z prone. Pressing the active stance again returns to Auto. There is no V key, so V stays melee in direct control.
  - Orders: RMB move, Ctrl+RMB attack-move, H stop, R retreat, Y dig, B barrage, L regroup.
  - Abilities: G frag, T smoke, F satchel.
  - WASD/QE stay on the camera.
  - This overrides the S1b soldier-control spec's Z/X/C/V and the S1 spec's A/S/D.

## Godot HUD spec

- **Anchors** (16 px gutters → 12 px at 1600):
  - Top bar: full width, 28 px tall.
  - Minimap: top-left at (12, TB+8).
  - Feed: top-right at (−12, TB+8).
  - Console: bottom-anchored, from left to right: roster, detail (300 px), orders grid (5 × 68 px). All share one top (y = H − 10 − 130) and one bottom (H − 10).
  - Call-in: right-anchored, 10 px above the orders.
- **Translucency:**

  | Element | Background |
  |---|---|
  | Top bar | rgba(12,16,18,0.55) |
  | Cards and detail panel | 0.46 |
  | Order buttons and stance strip | 0.40 |
  | Call-in strip | 0.38 |
  | Ability row | warm rgba(52,40,16,0.50) |

  There is no full-width slab, only a 170 px bottom gradient up to 0.26. Use one accent colour, #E0AD4A.
- **Minimap:**
  - A 250 px disc centred on the camera's look-at point, rotated by −camera_yaw. The frustum is always at the centre, pointing up.
  - North marker on the rim. A lock button on the rim at about 4:30 toggles north-up.
  - Mouse wheel zooms from radar view to the whole map. Ground outside the map is a hatched off-map zone.
  - Click-to-move maps the clicked point back through the rotation (inverse-rotate by yaw, then add the camera position) to world coordinates.
  - Show a yaw readout only while the camera rotates.
  - A setting docks the minimap bottom-left.
- **Call-in:** a 120 px column. Every tile always shows its name and cost. Re-call cooldown is a fill from the bottom plus a clock icon. One queue bar shows "Next: <unit> <s>" plus "n/5", up to 5 queued. Tab focuses the column.
- **Abilities:** Frag G (count), Smoke T (cooldown fill), Satchel F (dimmed unless engineers), Barrage B.
- **Stance:** Z / X / C / V segmented strip, plus Regroup L. Stance keys are not final; the owner needs to confirm them (they differ from direct control).
- **Top bar:** capture chips inline, with progress filled inside each chip and a hatched edge when contested.
- **Feed:** at most 3 lines, mouse_filter = IGNORE, each line fades after 8 s.
- **Font floors:** 13 px only for hotkey letters, times and badges; 14 px for everything else.
- **Pips:** at least 12 px with 3 px gaps, coded by shape as listed in fix 10. When one squad is focused, the detail panel lists its men as clickable rows in pip order.
- **1280 adaptation:** minimap 200 px, 3 cards per row, 62 px buttons, detail panel 262 px, call-in tiles 38 px with the column top no higher than y460. The clear band must stay at or above 70%.