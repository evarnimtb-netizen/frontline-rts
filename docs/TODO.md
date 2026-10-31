# Frontline RTS: owner to-do queue

Owner requests and known issues. Each item names the slice that handles
it.

## Queued (owner requests, 2026-10-31)

- [ ] **Individual soldier selection and orders**, GoH-inspired. Spec:
  `docs/slices/S1b-soldier-control.md`.
- [ ] **Call-in panel on the right side** of the screen. Today it sits
  bottom-left, next to the minimap.
- [ ] **Smaller top bar.** Today it is about 44 px tall at 900p and spans the
  full width.
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
- [ ] **Squad card sized to its content.** Today the card has a fixed width
  and cuts long names short ("Granatwerf…"). The card should grow to fit the
  name and the state text, with a sensible maximum, and only then shorten
  the text.
- [ ] **Ability buttons: throw grenade and similar actions.** They can go on
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
