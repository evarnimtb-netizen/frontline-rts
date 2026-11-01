# Frontline RTS: owner to-do queue

Owner requests and known issues. Each item names the slice that handles
it.

## Queued (owner requests, 2026-10-31)

- [x] **Individual soldier selection and orders**, GoH-inspired. Spec:
  `docs/slices/S1b-soldier-control.md`. Built in S1b (see the known issues
  below for what is still rough).
- [x] **Call-in panel on the right side** of the screen. Built in S1b: a
  right-anchored column above the orders.
- [x] **Smaller top bar.** Built in S1b: 28 px, with the capture chips in
  it.
- [x] **HUD direction, decided by the owner 2026-10-31: a docked console, but
  lighter.**
  - **Console:** a translucent bottom home for the roster with per-man pips,
    the soldier detail panel, and the orders, stance and ability buttons. Not
    a heavy solid slab.
  - **Minimap:** free-standing, larger than today's 220 px, and lightly
    framed. The owner suggested the top-left corner; that is being weighed
    against bottom-left. It is **north-up by default**, with an optional
    toggle to rotate with the camera (decided 2026-10-31).
  - **Capture points:** the A–E buttons move into the top bar, with capture
    progress drawn inside each button (owner request).
  - **Call-in:** a column at the bottom right, just above or beside the
    orders. The tiles always show the name and cost, and the queue is a
    compact bar with a count.

  Spec: `docs/slices/S1b-hud.md`. Mocks: `docs/slices/mocks/`. Built in S1b.
- [x] **Squad card sized to its content.** Built in S1b
  (`roster.card_width`). Today the card has a fixed width
  and cuts long names short ("Granatwerf…"). The card should grow to fit the
  name and the state text, with a sensible maximum, and only then shorten
  the text.
- [x] **Ability buttons: throw grenade and similar actions.** Built in S1b
  (`docs/slices/S1b-abilities.md`): G frag, T smoke, F satchel; the AI
  throws too, but rarely (see below). They can go on
  the same panel as the orders or elsewhere on the HUD, depending on the
  layout decision. They need sim support:
  - a per-soldier ammo or inventory count;
  - a ballistic frag-grenade throw, about 30 m, with HE blast reusing the
    mortar code;
  - smoke grenades that block LOS for spotting;
  - possibly a satchel charge for engineers.

  Each ability is a `Command` aimed at a position, with a hotkey and a
  cooldown or count shown on the button. Write this up as a spec next to
  S1b; the AI should use grenades as well.

- [ ] **A more military UI look (owner, 2026-10-31).** "This one looks like
  something from GMod or 'the office'. Way too corporate. The look itself is
  good, just not the right mood." Keep the HUD v2 layout and its
  readability; change the mood.
  - Three moods were mocked (field manual, olive steel, map table) and a
    blend of the best parts was revised as HUD v2.3 "Map table": felt
    panels, paper minimap, grease-pencil objectives. Mocks:
    `docs/slices/mocks/mood/`; theme spec: `docs/slices/S1b-ui-theme.md`.
  - Open owner question: the blend's panels are 0.92–0.95 opaque, while
    the owner asked for a lighter, see-through console (0.38–0.55).
  - Applied through theme constants (`frontline_theme.tres`), so it is a
    theme pass, not a re-layout.

## Known issues found during S1

- [ ] Map balance: the south-west base wins about 75 % of AI-vs-AI matches,
  whichever faction holds it. Mirror the valley, then look for a slot-0
  tick-order bias.
- [ ] Mortars cause most kills (2–4× small arms). Tune infantry fire.
- [ ] GitHub Actions is blocked by account billing, so there is no
  cross-OS CI.
- [ ] From M0:
  - paced ticks on macOS are 4–5× slower (efficiency cores);
  - Regiment drops 34 % of shots (pool full);
  - MPL-2.0 text missing from THIRD_PARTY_NOTICES;
  - Metal reports 0 GPU time.

## Known issues found during S1b

- [ ] **Assaulting a finished trench across open ground barely works.**
  Men in a trench lose nobody to rifle fire, while defenders see attackers
  at 150 m and attackers see them at 90 m. `fortify-test` (5 seeds × both
  factions): from 60 m the attackers are wiped in every run, with or
  without frags. Smoke first, then walking into the cloud to 27 m and
  throwing, costs the defenders 61 of 80 men, but the attackers still lose
  75 of 80 once the smoke thins. Frags from 25 m work (defenders 80 of 80
  lost). The open question is how a player gets to 60 m: suppression that
  really pins trench men, mortar smoke, or longer smoke throws. Needs an
  owner decision.
- [ ] The AI throws rarely: 0–4 items per AI-vs-AI match, no satchel seen in
  any seed, because its fights happen at rifle range.
- [ ] Sim step spikes in windowed Godot runs (max 13–44 ms, 5–13 late
  ticks per 12–18 minute run, nothing else running) that headless runs
  never show (P99 under 0.7 ms, max 1.4 ms). Probably the sim thread being
  descheduled (see the M0 efficiency-core note); not investigated.
- [ ] `--autoplay` lands a frag in an enemy trench in only 1 of 3 complete runs
  (2026-11-01). Assault squads sent across the map die on the way, and a
  thrower standing up for his 1 s wind-up is often shot. The committed
  `s1b_frag_tool.png` and `s1b_frag_trench.png` come from that one run;
  the other S1b shots come from the last full run. Autoplay exits 3 when
  the trench shot is missing.
- [ ] Autoplay pushes clicks and keys into the viewport, so the OS
  window-to-viewport path is untested.
- [ ] Throw streaks and frag blasts are small at the 40–55 m RTS camera
  height; a frag's 0.2 m crater is hard to see.
- [ ] At 1280x800 the roster shows 5 cards and a "+3" chip with 8 squads.
- [ ] A man winding up a throw in a trench stands on the fire step even when
  ordered prone (aiming, not suppression).
- [ ] `MoveUnits` puts all detached men on one line, n × spacing wide (about
  117 m for 40 men). The spec says "a small line".
- [ ] Throw flight is 2–2.7 s at 25–30 m (45° lob, deliberate); the spec
  said about 1 s.
- [ ] Fog of war for thrown items and smoke clouds (visible if ours, if we
  see the thrower, or within 40 m of our men) is our own rule, not from
  the spec.
- [ ] Not measured: the cost of 240 soldier dictionaries in `get_state`, and
  picking + rings on an M3 export build. Headless debug measures
  0.27–0.32 ms against the 0.3 ms budget (the view test's own limit is a
  loose 2 ms).
- [ ] `bench/results` S1 hashes are stale since S1b.
- [ ] Capture chips: the progress fill covers the letter (the "C" chip at
  1280 is hard to read). Draw the fill under the letter or as a bar.
- [ ] Trench lines are hard to see among mortar craters at RTS camera
  height; they need a stronger edge or darker floor shading.
- [ ] The fix pass cut the call-in queue from 10 to 5 to match the HUD
  spec. That is a game rule; confirm with the owner.

## Known issues found during S2

- [ ] **No exported zip yet.** `tools/export_macos.sh` builds the universal
  bridge (arm64 + x86_64 with lipo, checked) and stops before the export:
  the Godot 4.7.2 export templates are not installed, and the official
  `.tpz` is 1.28 GB, over the ~1 GB the integration stage was allowed to
  download. Install them (Editor > Manage Export Templates), then run
  `tools/export_macos.sh --verify`. The export itself, the ad hoc
  re-signing after adding `mods/ww2` to `Contents/Resources`, and the
  second-account run are untested.
- [ ] **Playout buffer too small on jittery links.** `frontline-net-test
  --seconds 1200 --jitter 40 --drop 0.05`: 0 restamps, hashes agree, but
  about 16 % of ticks step more than one period late (3,918 and 3,566
  stalls out of 24,142 ticks). The view lags in bursts; the match stays
  correct. A client-only adaptive playout delay would fix it without
  touching determinism. Owner call.
- [ ] Two windowed instances on one Mac: 10–14 stalls per 3-minute match,
  almost all in the first minute (match start, the S1b selection tour's
  screenshots). Headless runs show none. Same family as the S1b windowed
  step spikes.
- [ ] App Nap: not hit with both windows visible side by side; the
  `defaults write ... NSAppSleepDisabled` step from the spec was not run
  (no persistent defaults). A hidden or minimised instance is untested.
- [ ] After a host crash the guest waits about 75 s before the disconnect
  screen (silent relay for 10 s, then the "Reconnecting" countdown; the
  bridge gives up at 75 s). Correct per the design, but long when the
  relay died with the host's game.
- [ ] The match readout's raw `net.state` turns "unstable" 2 s after the
  match ends (the other side stops sending after its goodbye). The HUD
  freezes the readout at the end; the bridge value is still misleading for
  other readers.
- [ ] `build_id` is stamped when `crates/net` is rebuilt (HEAD or index
  changes); edits to tracked files without staging do not set `-dirty`
  until something rebuilds it. `tools/export_macos.sh` refuses a dirty tree
  unless `--allow-dirty`.
- [ ] `THIRD_PARTY_NOTICES` not regenerated for `socket2` (from net stage 1).
- [ ] `relay-protocol.md` does not mention the `Bye(Shutdown)` the relay sends
  when it stops (net stage 2).
