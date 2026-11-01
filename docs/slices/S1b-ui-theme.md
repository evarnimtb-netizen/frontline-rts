# S1b UI theme: Map table (HUD v2.3)

The source is gen_hud_blend.py.

## Palette
- Felt #1d2721 (panels), canvas #3e341e (abilities), paper #e2d8b8 (minimap, tape), slip #d8cca6 (call-in)
- On felt: cream #ece4c8 text, dim #d0c8ac secondary, off #a29d87 disabled
- On paper: ink #22251d, ink-off #67614b disabled, grease-ink #5a3205 costs
- Grease #efc94c: selection only
- Arcs and tickets: own #86a6cf, enemy #d4806b
- Stamp #8e2414 on a #efe5c6 patch; lamp #ffa424; key tab #e8dfc2

## Fonts
All are OFL 1.1; ship each OFL.txt.
- Barlow Medium/SemiBold: body text, 14 px or larger. https://github.com/google/fonts/tree/main/ofl/barlow
- Stardos Stencil: headers only. Its name is reserved, so rename any modified copy. https://github.com/google/fonts/tree/main/ofl/stardosstencil
- Courier Prime: times. https://github.com/google/fonts/tree/main/ofl/courierprime

## StyleBoxes
- **Felt:** felt.png, tiled, square corners. On top: a 1 px dark outer seam, a 2.5 px black binding at 55%, dashed cream stitching at 30% 4 px in, a 1 px cream top edge at 45%, and a 1 px black bottom seam.
- **Neatline:** 1.3 px cream at 46%, with a hairline 2.5 px inside it.
- **Map border:** a 4 px band of alternating 24 px black and cream segments.

## Textures
- felt.png, 128 px: mottled wool, 3 px twill at −38°, highlights at 10% or less
- paper.png, 256 px: grain
- torn_edge.png, 8×64: tape-end mask
- stamp_ink.png, 64 px: break-up alpha
- stamp_frame.png, 32×24: double rule, 9-slice

## Unit glyphs
Original, not APP-6: cream ink, 20×14, on a 24×17 #121813 counter with a 7 px corner in the unit colour.
- Rifle: side-on.
- MG: barrel on a bipod.
- Engineer: D-grip spade.
- Mortar: tube at 45° on a baseplate.
- Sniper: scoped rifle.

## Factions
These go on the top-bar plate, detail header and slip header.
- GER: #46535b, cream text, square emblem.
- USA: #bdb07c with paper grain, ink text, circle emblem.
- USSR: #5e4a35, cream text, triangle emblem. ΔE00 ≥ 21 from both reds.

The emblems are survey marks with a dot, not insignia.

## States
- **Selected:** wobbly grease outline, 12% wash.
- **Disabled:** off text (3:1 or better), dashed key tab, hatched cell.
- **Alarm:** red double-ruled stamp (tilt 2° or less), with an 8 px amber lamp in a bezel to its left.
- **Cooldown:** a 13% cream fill rising from the bottom, with seconds.
- **Keys:** a cream tab with an ink letter at the cell's top-left, 4 px or more from the label. The active stance shows "↺ Auto": press it again for Auto.

## Top-bar chips
- **Objective:** a cream disc with the letter.
- **Ownership:** a grease arc (85% opacity) that runs clockwise from north.
- **Contested:** a dashed own/enemy ring.
- **Tickets:** a checker scale bar labelled 0 and max. The value is a grease stroke plus a number.

## Needs owner sign-off
- 0.92–0.95 panel opacity (the spec says 0.38–0.55)
- 32 px top bar (the spec says 28)
- 4 cards per row at 1600
