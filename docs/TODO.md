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
    against bottom-left. It was **north-up by default**, with an optional
    toggle to rotate with the camera (decided 2026-10-31). Since 2026-11-01
    rotating with the camera is the default, drawn as the turned map alone
    with no disc or hatch behind it (owner request).
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

- [x] **Two-player multiplayer (S2).** Spec and slice log:
  `docs/slices/S2-multiplayer.md`. Host or join by IP:port, an in-game
  relay or `frontline-relay` on a server, lobby, relay-closed lockstep,
  pause, hash checks, desync dumps and `desync-diff`, 60 s timeout
  surrender, and a macOS zip (`tools/export_macos.sh`). Built on the
  spec's (A) defaults; the input delay and playout formula was revised
  in the fix pass (owner call, see below). Guide:
  `docs/net/playing-online.md`.
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

- [x] **Tanks and armour (S3).** Spec: `docs/slices/S3-armour.md`; results
  and the fix pass: `docs/slices/S3-log.md`. Panzer IV and Sherman, the
  Panzerschreck and Bazooka teams, per-plate penetration, crew and
  components, fire and ammo rack, wrecks, the AT ditch, escorts, vehicle
  AI, `ballistics` and `armour-test`, net play with vehicles, the HUD and
  presenter additions. Built on the spec's (A) defaults; all ten
  agent-checkable "Done when" items pass.

## S3 owner questions (pending; work used the (A) defaults)

- [ ] **1. Scope.** (A) Tanks and AT teams now; tank destroyers, sticky
  satchels, bail-out crews and AI AT-ditch digging in S3b. (B) The full
  draft in one slice.
- [ ] **2. Vehicle spotting range.** (A) 2× the observer's sight (tanks
  see tanks at about 350 m). (B) About 4× (600–700 m).
- [ ] **3. Do standard trenches stop tanks?** (A) No: they slow tanks to
  1 m/s and can bog them; the AT ditch stops them. (B) Any finished trench
  blocks vehicles.
- [ ] **4. Unlock and limit.** (A) Vehicles from 4:00, at most 4 alive or
  queued per player, 12 pop each. (B) No unlock, cost only. (C) Other
  numbers.
- [ ] **5. Target for "tanks fix the trench assault"** (armour-test case
  7). (A) Attacker losses at most half the no-tank baseline's, and
  defenders lose at least 75 %. (B) A different bar. Case 7 now **misses**
  both (attackers 74 of 80 vs 80 of 80 without the tank; defenders 68.8 %).
  If the target stays, pick the next lever: tank fear, mortar smoke, or
  stronger pinning (S3-log.md "Case 7 vs target").
- [ ] **Confirm the narrowed case 4 hard check** (fix pass): "at least
  one rocket hit per run" now covers the flank and front runs. The HE
  variant, where the team is under HE fire and hits 57 % at 100 m, prints
  its no-hit runs instead of failing.

## S3 owner checklist (needs a person)

- [ ] **11. Play a match vs AI to victory** using tanks, an AT team and an
  AT ditch. Note whether the turret speeds, bogging, the unlock time and
  the Sherman's cost feel right.
- [ ] **12. One net match with the friend** where both sides field
  vehicles.
- [ ] **13. Read armour-test case 7 against the target** (owner question
  5) and pick the next lever if needed.

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

## S2 owner checklist (needs people or a second network)

- [ ] **One manual match**, two humans with mouse and keyboard, played to
  victory (spec item 7).
- [ ] **One match over Tailscale or the VPS relay**, with ping, D, playout
  and the "late" count from the readout's tooltip written down (item 8).
  Wired if possible.
- [ ] **The friend runs the zip on their own Mac** (item 9). Also not done
  by an agent: a second macOS account on the same Mac (agents may not
  create accounts) and an Intel Mac.
- [ ] **Decide the D / playout revision** (`S2-multiplayer.md` "Turns"):
  D now covers half the round trip and a client playout delay covers the
  other half. Same click-to-reaction time, almost no stalls, but on very
  lossy links 5–10 % of orders run a tick or more late. Reverting is one
  function (`frontline_net::suggest_d`).

## Known issues found during S2

- [ ] **The zip is an "editor runtime" build**, not a template export:
  the Godot 4.7.2 export templates are not installed (the official
  `.tpz` is 1.28 GB). `tools/export_macos.sh` falls back to a copy of the
  Godot editor app with the game packed in (176 MB zip, debug-capable).
  It runs and plays a net match from the unzipped zip. Install the
  templates (Editor > Manage Export Templates) and rerun the script for
  a proper release export; `--templates-only` refuses the fallback.
- [ ] `frontline-relay` is only type-checked for Linux (as
  `aarch64-linux-android`) and Windows; no glibc Linux build or run yet
  (needs `rustup target add x86_64-unknown-linux-gnu` and a linker, or
  building on the VPS as `playing-online.md` says).
- [ ] Lossy links now cost restamps instead of stalls: at 10–30 % loss,
  5–10 % of orders run a tick or more late. At 0–5 % loss: 0–0.4 %.
- [ ] Two windowed instances on one Mac: about 10 stalls per 3-minute
  match, all in the first seconds (start-up and the S1b tour's
  screenshots). Headless runs show none.
- [ ] App Nap: not hit with both windows visible (partly overlapping);
  the spec's `defaults write ... NSAppSleepDisabled` was not run (no
  persistent defaults). A hidden or minimised instance is untested.
- [ ] After a host **crash** the guest waits about 75 s before the
  disconnect screen (10 s of silence, then the "Reconnecting" countdown).
  The texts now say the match is over if the host's game closed, but the
  wait is still long. A clean host quit is fast: the relay delivers the
  host's surrender before it shuts down (tested on the relay and in the
  bridge over real UDP, `host_leaving_mid_match_surrenders_to_the_guest`;
  not yet seen with a human clicking Leave in Godot).
- [ ] Hosting again right after leaving joins the old net threads on the
  main thread: up to about 1.5 s of frozen menu if the guest vanished
  (it frees the port first). Not measured.
- [ ] Possible IPv6 issue (unverified, no global IPv6 here): a relay bound
  to `[::]` may answer from a temporary address, which the client would
  drop (it only accepts its configured relay address).
- [ ] The waiting overlay has no "leave now / claim the win" button: the
  remaining player waits out the 60 s or leaves through the Esc menu.
- [ ] The pending-order marker is a bare dashed ring; nothing links it to
  the ordered unit. At D = 2 it lives one or two frames.
- [ ] `desync-diff --replay` is not built (spec: out of S2). Dumps now
  always have a checkpoint (tick 0 counts as confirmed).
- [ ] In two-instance autoplay the S1/S1b shots share file names, so the
  two instances overwrite each other's shots (test only).
- [ ] `build_id` is stamped when `crates/net` is rebuilt (HEAD or index
  changes); edits to tracked files without staging do not set `-dirty`
  until something rebuilds it. `tools/export_macos.sh` refuses a dirty
  tree unless `--allow-dirty`.
- [ ] `THIRD_PARTY_NOTICES` not regenerated for `socket2`.

## Known issues found during S3 integration

- [ ] **Net stalls in windowed two-instance runs:** the integration saw
  250–550 stall ticks per 3-minute match with tanks (55–61 without, S2:
  10), in 0.3–1.1 s bursts at the same tick on both instances. The
  performance review then saw 483 stalls before any vehicle existed, and
  971 in a headless run with no vehicles at all, at load average 5–17
  with swap nearly full. The fix pass, at load 3, saw no bursts (hash
  arrival at most 43 ms late). Probably machine pressure, not vehicles;
  re-measure on an idle machine.
- [ ] The S1b frag-into-trench autoplay shot still fails most runs; with
  S3 the seed-1 match also ends earlier (AI won on tickets at 13:06 while
  we spent 1,280 manpower on tanks).
- [ ] Call-in tiles hide the "need N" reason when it is wider than the
  room next to the cost: a vehicle tile with "need 509" shows only
  "640 MP" (greyed).
- [ ] The rocket trail is a straight pale box (9 m, no taper); it reads
  as a white stick from 30 m up.
- [ ] A knocked-out tank lit by its cook-off light shows a flat orange
  patch on the hull for about 0.3 s.
- [ ] Staged scenes all sit on point C (the sim's staging lane runs
  through the map centre).

## Known issues found during the S3 fix pass

Balance (numbers in `docs/slices/S3-log.md`; spec numbers, not tuned
here):
- [ ] **The Sherman wins tank fights:** 26 of 30 staged duels (3a frontal
  6–4, 3b angled 10–0, 3c flank 10–0). In AI matches Germany lost 17 tanks
  and the USA 8, and the USA won 8 of 10. The Panzer IV's 14°/s turret
  loses the race to the first hit (about 1.1 s).
- [ ] **Almost every knock-out is an ammo-rack detonation:** the rack
  (60 HP) sits in the hull centre, and an AP core does 100.
- [ ] **AT teams are weak:** case 6 disabled the tank 0 of 10 times
  despite 56 penetrations; case 4 flank rate 42.9 % (target ≥ 50 %). A
  HEAT jet (1.0 m) through the Panzer IV's hull front stops about 5 cm
  short of the driver and radio-operator boxes (x 115–185 cm, plate at
  290 cm). Levers: 1.5 m jet or crew boxes further forward.
- [ ] Crossing-target hit rate is 83 % of stationary (target ≤ 60 %).
- [ ] A tank in front of a trench kills 0 of 80 dug-in men in 3 minutes
  and pins nobody (case 5); from the trench's end it kills 75 of 80 (case
  11). Tank HE erodes a parapet by 0.001 m per burst (case 9).
- [ ] Satchels open a 30 m AT ditch in only 2 of 10 runs (7 each; the
  rest still closed after 16). Engineers sent at a tank from 40 m all die
  (case 8).
- [ ] The AI never throws satchels at vehicles (0 in 10 matches), rarely
  fires its AT teams (0 rockets in 6 of 10), and no tank bogged in 10
  matches. Escorts reach 30 m of an enemy trench 0–4 times per side.
- [ ] Map side: base 1 won 7 of 10 AI matches in this pass (S1: the
  south-west base about 75 %). Too few games to say more.

Other:
- [ ] At 1280x800 the roster now keeps vehicle cards before unselected
  infantry: with 4 tanks and 8 squads it shows the 4 vehicle cards and
  "+8" (before: 5 infantry cards and "+7"). Owner's call which is better.
- [ ] AT-ditch sections are recorded when dug (fix pass), in one shared
  list of 1,024 for both players; no per-player cap.
- [ ] `vehicle_view.gd` draws at most 48 vehicles per type (Battalion
  tier blocker; S3 rules need 40).
- [ ] Vehicle path ticks can serve the 4,000 budget plus one capped
  8,000-expansion search (11,694 expansions, intent stage up to 2.2 ms,
  4.4–5.7 ms under load). Within the spec; sets the worst tick at the
  Battalion tier.
- [ ] The vehicle spatial index is rebuilt twice per tick even with no
  vehicles (about 2 % of step samples); small per-tick allocations in
  gunnery and the block layer.
- [ ] Case 1's 3σ check is nearly empty (σ 0.0–0.6): the aim-point choice
  avoids borderline plates.
- [ ] A 7-minute `--autoplay` can miss the enemy's first tank (called in
  at about 5:40); run at least 10 minutes.
- [ ] The net loopback and soak use 4,000 starting manpower (not in the
  spec) so that 2 tanks per side arrive in 3 minutes.
- [ ] No in-game 1280x800 S3 screenshot: `--resolution 1280x800` keeps a
  1600x900 window for the staged scenes. The mock's `s3_mock_*_1280`
  shots cover the HUD.

