# Slice S1b: individual soldier selection and orders

**Owner request (2026-10-31):** "I want for the player to be able to select
the individual units. Inspired by Gates Of Hell." S1 shipped squad-level
selection and orders only. S1b lets the player pick out single soldiers (or
any set of soldiers) and order them, while squads stay the unit of
organisation, call-in and AI planning.

This builds on `docs/slices/S1-skirmish.md`; its hard rules all apply
(orders are `Command`s, deterministic sim, saved and hashed state, presenter
rules from §4.1). This is a genre convention, not a copy of any game's UI.

## Selection (presenter)

- **Left-click a soldier:** selects that soldier only.
- **Double-click a soldier:** selects his whole squad.
- **Ctrl + double-click:** selects every squad of that type on screen.
- **Box-select:** selects the individual soldiers inside the box (own side
  only; enemies and corpses never).
- **Shift:** adds to or removes from the selection (click and box).
- **Squad cards:** the selection panel shows one card per squad that has a
  selected man. The card shows each man as a small pip (alive, wounded,
  pinned, selected or not).
  - Clicking the card header selects the whole squad.
  - Clicking a pip selects that man.
- **Control groups:** Ctrl+1–9 store the exact set of soldiers; 1–9 recall
  it, and double-tap centres the camera on it. Dead men drop out.
- **Picking:** in screen space, against the interpolated positions the
  renderer draws. Pick the nearest soldier within about 14 px of the cursor;
  on ties, the one closer to the camera. Never one Node per soldier.
- **Feedback:**
  - a small ring under each selected soldier, drawn as one MultiMesh batch;
  - a hover highlight with a tooltip: squad name, weapon, health, state;
  - order markers for detached soldiers' own waypoints.

## Orders on a soldier set (sim)

Add commands that act on a set of soldiers instead of a squad:

| Command | Effect |
|---|---|
| `MoveUnits { units, target }` | The men walk to `target` and spread in a small line around it. |
| `AttackMoveUnits { units, target }` | Same, engaging enemies on the way. |
| `StopUnits { units }` | Stop where they stand. |
| `RetreatUnits { units }` | Only these men fall back to base. |
| `DigUnits { units, line }` | Only these men dig the line, split among them (diggers only). |
| `SetStance { units, stance }` | `Auto`, `Stand`, `Crouch` or `Prone`. Suppression still forces a lower stance; it never forces a higher one. |

**Validation.**
- `units` is 1–120 ids. It is sorted and deduplicated when the command is
  applied.
- Ids that are dead, belong to another player or aren't soldiers are
  dropped. A command with none left is ignored.
- Barrage stays a squad order. A barrage sent for a selected mortar man
  applies to his squad.

**Detached men.**
- A soldier given a unit order becomes *detached*. He follows his own order
  and leaves his squad's formation slot, but stays in the squad: it still
  counts him for strength, cards and the AI.
- Any squad-level order (`Move`, `AttackMove`, `Stop`, `Retreat`, `Dig`, or
  a unit order that covers every living member) clears `detached` for the
  whole squad, and they regroup.
- Detached men still auto-acquire targets, take cover, get suppressed and
  dig as usual.
- Squad cohesion is left out of S1b. Detached men get no buff and no
  penalty for being away from the squad.

**Presenter mapping for right-click and hotkeys:**
- When every living man of a squad is selected, that squad gets the
  squad-level command, as in S1. This keeps formations and keeps the
  command log small.
- Selected men from partly selected squads go into one `*Units` command per
  order.
- Hotkeys: see `S1b-hud.md`. Stance keys are X stand, C crouch and Z
  prone; pressing the active stance again returns to Auto. They match
  direct control and work on any selection.

**State.**
- Per soldier: `detached` flag, own order target, stance override.
- Saved, loaded, hashed, and covered by the determinism tests.

**Snapshot:**
- per soldier: `detached` flag and stance override;
- for detached men, their order target, so the presenter can draw markers.

**AI.** No change: it still plans squads. It may use `SetStance` later.

## Done when

1. `cargo test --workspace --release`, clippy and fmt pass. Determinism
   tests include matches with unit orders mixed in (scripted human commands
   alongside the AI):
   - the same hashes at 1/2/3/8 threads;
   - the same hashes through save → load → continue.
2. Sim tests cover:
   - validation: wrong owner, dead men, duplicates, empty sets, too many ids;
   - detach and regroup rules;
   - `SetStance` versus suppression;
   - `DigUnits` splitting the line.
3. In the Godot match, each of these works and is shown by `--autoplay`
   steps with screenshots in `docs/slices/screens/`:
   - click-select one man;
   - double-click his squad;
   - box-select men from two squads;
   - send one man forward alone;
   - set prone;
   - throw a grenade into an enemy trench (see `S1b-abilities.md`);
   - recall a control group.
4. Picking 240 soldiers and drawing selection rings costs under 0.3 ms per
   frame on the M3.
