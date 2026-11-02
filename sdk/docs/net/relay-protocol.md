# Frontline relay protocol

Protocol version **4** (`PROTOCOL_VERSION` in `crates/net/src/protocol.rs`).
This file is the reference for the wire format, so anyone can write a
relay or a tool that speaks to one (§7.9, §17 Q15). The code and this file
change together, and every change a peer could observe bumps the version.

Frontline multiplayer is **relay-closed lockstep**. Both clients run the
whole deterministic sim. The relay never runs the sim: it collects each
player's orders, closes one turn per tick on its own clock, and sends every
client the same closed turns ("bundles"). A match is a pure function of
its `MatchSetup` and the bundles fed to `World::step`.

## Transport

- Plain UDP. The default port is **47015**.
- A datagram is at most **1,200 bytes**, header included. Larger datagrams
  are dropped unread.
- There is no encryption. Anyone on the path can read and forge
  datagrams. Tailscale encrypts the path if that matters.
- Reliability comes from the protocol itself: control messages are resent
  until acked, input is resent until it shows up in a bundle, and bundles
  are resent until acked.

## Header

Every datagram starts with a 15-byte header. All integers are
little-endian.

| Bytes | Field | Meaning |
|---|---|---|
| 0–3 | magic | `b"FLNT"` |
| 4–5 | version | `u16`, the protocol version |
| 6 | kind | `u8`, the message kind (table below) |
| 7–14 | token | `u64`. 0 before the relay assigned one |

The body follows: one [postcard](https://docs.rs/postcard/1.1.3) value
(postcard 1.1.3: varint integers, length-prefixed sequences and strings,
enums as a varint variant index). Bytes after the body are ignored; only
`Hello` uses that, for padding.

### Kinds

| Kind | Name | Direction | Body |
|---|---|---|---|
| 1 | `Hello` | client → relay | `Hello` |
| 2 | `Welcome` | relay → client | `Welcome` |
| 3 | `Reject` | relay → client | `Reject` |
| 4 | `CtlUp` | client → relay | `{ cseq: u32, msg: Up }` |
| 5 | `CtlDown` | relay → client | `{ cseq: u32, msg: Down }` |
| 6 | `Ack` | both | `{ next: u32 }` |
| 7 | `Ping` | client → relay | `Ping` |
| 8 | `Pong` | relay → client | `Pong` |
| 9 | `Input` | client → relay | `Input` |
| 10 | `Bundle` | relay → client | `Bundle` |

The relay drops every kind a client may not send, and a client drops every
kind the relay may not send. Clients cannot send relay messages.

## Version rule

- Any change to a body, a kind number, a limit or a timing rule that a
  peer can observe bumps `PROTOCOL_VERSION`.
- Two things never change, so peers on different versions can still
  explain the mismatch:
  - the header layout;
  - kind 3 (`Reject`) with a `u16` relay version as its first body field.
- Version 2 (S2 fix pass) changed timing rules only, no layout: bundle
  delivery repeats unacked turns, the D formula and playout delay, the
  lead clamp (`+10`), lobby coalescing, pause-request rate limit, the
  `Start` sanity check, the waiting-pause timeout, and the shutdown
  drain.
- Version 3 (S3) changed the `CommandKind` layout: `Dig` and `DigUnits`
  gained `profile`, and six vehicle commands were added at the end (see
  below).
- Version 4 (S3b, 2026-11-01) changes what `CallIn` means, not its
  layout: the squad is bought and spawned at once in the base, or refused
  (manpower, the type's re-call cooldown, pop cap, vehicle unlock or
  limit); there is no call-in queue and no command for one. The S3b
  slice bumps once, so version 4 also adds `SuppressUnits` at the end of
  `CommandKind` (area fire at a point or a two-point line) and the
  `AmmoChoice` value `Apcr` (3) that `SetAmmo` may carry.
- Version 5 (S3c, 2026-11-01) adds two commands at the end of
  `CommandKind`: `Recrew { squad, vehicle }` (an infantry squad walks to
  an abandoned vehicle and mans it) and `Abandon { squad }` (the crew of a
  vehicle squad bail out). Neither has a list; both are bounded like every
  other command by the 1,024 B single-command limit. The one bump for the
  S3c slice.
- A relay answers a padded `Hello` of another version with
  `Reject { relay_version, nonce: 0, reason: Version }`, sent with its own
  version in the header. The client reads only the frozen part and tells
  the player "the relay speaks protocol N".
- Both players also need the same **build id** (below). The relay compares
  the players' build ids with each other, not with its own, so one
  `frontline-relay` binary can serve many game builds.

## Limits

Checked on every decode. A datagram that breaks any limit is dropped whole.

| Limit | Value |
|---|---|
| Datagram | 1,200 B |
| `Hello` minimum size (padding) | 512 B |
| Player name | 1–24 B of UTF-8 |
| Def id, build id (`faction`, `map`, `build_id`) | 64 B |
| Lobby note | 160 B |
| Own commands per turn | 16 |
| Encoded own commands per turn | 1,024 B (and any single command ≤ 1,024 B) |
| Unit ids per command | 120 (`MAX_UNITS`) |
| Points per dig line | 16 (`MAX_DIG_POINTS`) |
| Points per `SuppressUnits` line (version 4) | 1–2 (`MAX_SUPPRESS_POINTS`) |
| Turns per `Input` | 64 |
| Turns per `Bundle` datagram | 300 |
| Commands per bundled turn | 34 (2 × (16 + 1 relay-injected `Surrender`)) |
| Encoded closed turn | 1,100 B (excess rolls into the next turn) |
| Commands waiting at the relay per player | 256 (beyond: `Bye(Flood)`) |
| Input delay D | suggestion 2–4, slot 0 override 2–6, capped by `--max-delay` |

## Messages

### Session setup

**`Hello`** (client → relay, token 0, padded to ≥ 512 B)

```
Hello { nonce: u64, build_id: String, content_hash: u64, name: String }
```

- `nonce`: random per connection attempt. Resent every 250 ms until a
  `Welcome` or `Reject` with the same nonce arrives.
- `build_id`: `<git commit, 12 hex>[-dirty]-p<protocol>-s<save version>`,
  for example `3f2a9c1d0b7e-p2-s8`.
- `content_hash`: xxh3-64 of postcard(`ModDefs`), from
  `ModDefs::content_hash()`. It covers the resolved defs (after `extends`),
  not the raw files. S4 Part A changed it for the same `mods/ww2` files
  (unit defs gained the card-block, `members`, `tags` and `capabilities`
  fields, rules gained `[training]`, and the training profiles joined the
  defs): a build from before S4 and one from after refuse each other with
  `Content`, as intended. The protocol and its limits are unchanged.
  S4 Part B changed the content (the S4 cards, weapons, the Panzerfaust)
  and the save version (`-s12`: per-man weapons, crew-weapon drops, the
  Panzerfaust state, S4 counters), so `build_id` and `content_hash` both
  differ from Part A builds. No command changed: `PROTOCOL_VERSION` stays
  5 (a `Throw` may name the new item kind `single_shot_at`; the sim refuses
  it, and only builds that agree on the content ever meet). Relay limits
  are unchanged.
- The relay drops `Hello`s shorter than 512 bytes. No reply before
  authentication is larger than the request, so the relay cannot amplify
  a flood with a spoofed source address.
- Rate limit before authentication: 8 `Hello`s per source IP, refilled at
  4 per second. Over the limit, nothing is answered. At most 4,096 source
  IPs are tracked.

**`Welcome`** (relay → client, token 0)

```
Welcome { nonce: u64, token: u64, slot: u8 }
```

- `token`: random, non-zero. Every later datagram in both directions
  carries it in the header. The token is the client's identity, not its
  address. A datagram with a valid token from a new address moves the
  connection there (NAT rebinding, Wi-Fi to Ethernet). Datagrams with an
  unknown token are dropped without a reply.
- `slot`: 0 or 1. Slot 0 is whoever joined first; it chooses the seed, the
  map and D.
- A resent `Hello` with the same nonce gets the same `Welcome`, sent to
  the address it came from.

**`Reject`** (relay → client, token 0)

```
Reject { relay_version: u16, nonce: u64, reason: RejectReason }
RejectReason = Version | Build { lobby: String } | Content | Full | Started | BadName
```

- `Build`: the other player runs another build (`lobby` is theirs). A
  relay started with `--allow-build-mismatch` skips this check (dev only).
- `Content`: the other player loaded different defs (`content_hash`).
- `Full`: both slots are taken. `Started`: no join-in-progress.

### Control stream

Lobby, match control and hashes travel on one reliable, ordered stream per
direction.

- Each message has the sender's next `cseq` (from 0, `u32`, wrapping).
- The receiver processes a message only if `cseq` is exactly the next one
  it expects. It ignores the others (duplicates, or ones that overtook an
  earlier message).
- **Ack rule:** every control datagram is answered with `Ack { next }`,
  where `next` is the next `cseq` the receiver expects. The ack is
  cumulative: every `cseq < next` was processed.
- The sender resends unacked messages every 100 ms, at most the 4 oldest
  per round, and sends new ones at once.
- An `Ack` for a message never sent is ignored.
- At most 1,024 messages wait unacked. A sender never drops one (that
  would leave a `cseq` gap the receiver never gets past): the relay
  instead drops a slot whose stream backs up that far (freed in the
  lobby, a `Surrender` in a match). No honest peer gets near it: lobby
  updates are coalesced (below) and pause requests rate-limited.

**`Up`** (client → relay, inside `CtlUp`)

| Message | Meaning |
|---|---|
| `LobbySet { faction, seed: Option<u64>, map: Option<String>, d: Option<u8> }` | Lobby choice. Only slot 0's `seed`, `map` and `d` count (`d: None` means "use the suggestion"; others are clamped to 2..=`max_d`). Any `LobbySet` clears both players' Ready. |
| `Ready { setup_hash: u64 }` | xxh3-64 of postcard(`MatchSetup`) from the client's own resolve. |
| `Hash { tick, hash }` | A published sim hash: the state hash after `tick` ticks (every `hash_interval` = 30 ticks). |
| `PauseReq` | Pause the match. |
| `ResumeReq` | Resume a player pause. A slot's pause and resume requests closer than 250 ms to its previous accepted one are ignored. |
| `Bye(reason)` | Leaving. In the lobby the slot is freed. In a match, any reason except `MatchOver` makes the relay close a `Surrender` for the slot. |

**`Down`** (relay → client, inside `CtlDown`)

| Message | Meaning |
|---|---|
| `Lobby(LobbyState)` | Sent after a change (at most every 50 ms, carrying the latest state), and at most once a second as pings move the RTT readout and the D suggestion. |
| `Start(Start)` | The match starts. |
| `Paused { at, cause }` | The relay stopped closing turns; turn `at` is the first not closed. `cause`: `Player(slot)` or `Waiting(slot)` (silent for 10 s). |
| `Resume { at, close_at }` | Turn `at` closes at relay time `close_at` (µs), turn `at + k` one period per `k` later. |
| `Desync { tick, hashes: [u64; 2], at }` | The hashes for `tick` differed (slot order). Turn `at` is not closed. The match does not resume. |
| `Bye(reason)` | `TimedOut`, `Flood` or `Shutdown`. `Shutdown`: the relay is stopping (the host left, or the binary stops). Mid-match it first closes the turns holding every queued command (a quitting host's `Surrender`), resumes a player or waiting pause, and sends those bundles for up to 250 ms or until each player holds them; only then `Bye(Shutdown)`, since a client stops reading bundles once it has it. |

```
LobbyState { slots: [Option<SlotInfo>; 2], seed: u64, map: String,
             d: Option<u8>, suggested_d: u8, note: String }
SlotInfo   { name: String, faction: String, ready: bool, rtt_ms: u16 }
Start      { start_at: u64, tick_hz: u32, speed_pct: u16, d: u8, seed: u64,
             map: String, factions: [String; 2], names: [String; 2] }
ByeReason  = Quit | MatchOver | TimedOut | Flood | Shutdown
PauseCause = Player(u8) | Waiting(u8)
```

A client refuses a `Start` with `tick_hz` outside 1–120, `speed_pct`
outside 1–1,000 or `d` outside 2–6 (the datagram is dropped as invalid).

**Names.** A player whose `Hello` name equals the other slot's gets
` (2)` appended, so overlays can tell them apart.

**Start rule.** When both slots are Ready, the relay starts only if:

- both picked a faction, and the factions differ;
- both setup hashes are equal.

Otherwise it clears both Ready flags and puts the reason in
`LobbyState.note`. The match starts `countdown` (3 s) after the decision:
turn 0 opens at `start_at` and closes one period later. Clients map
`start_at` through their synced clock for the 3-2-1.

### Clock and RTT

**`Ping { t_client: u64, rtt_p99_us: u32, jitter_us: u32 }`** every 250 ms
in the lobby, every 1 s in a match. **`Pong { t_client, t_relay }`** at
once.

- Each `Pong` gives an offset sample `t_relay + rtt/2 - now`.
- The client uses the median of the last 8 samples. It slews its offset
  toward the median by at most 1 ms per second, and steps it only when it
  is more than 50 ms off.
- `rtt_p99_us` (the highest of the last 32 RTTs) and `jitter_us` (the mean
  absolute difference of consecutive RTTs) feed the relay's D suggestion:

  `D = clamp(ceil((rtt_p99 / 2 + jitter) / period), 2, 4)`, the larger of
  the two slots, capped by `--max-delay`.

  D covers only the trip to the relay: a command stamped `open + D` must
  arrive before that turn closes, D to D+1 periods later. The trip back
  is the client's **playout delay** (below). An asymmetric path does not
  matter: the synced clock is off by half the asymmetry, in the direction
  that compensates. (Revision 1 used `ceil((rtt_p99 + jitter) / period)
  + 1` with one tick of playout: about twice the D a link needs, and a
  downlink slower than one period stalled anyway.)

### Turns

The tick rate is 30 Hz: one turn is one tick, 33,333 µs. Turn `T` closes
at `close(T) = base_close + span(T - base_tick)`, where `span(n) = n ·
10⁸ / (tick_hz · speed_pct)` µs, computed from the tick count, never
accumulated. `Start` sets `base_tick = 0, base_close = start_at + period`.
`Resume` sets them to `(at, close_at)`.

**`Input`** (client → relay), once per period from `Start` on, even when
empty (it doubles as the heartbeat):

```
Input     { ack: BundleAck, turns: Vec<InputTurn> }
BundleAck { held: u64, mask: u64 }
InputTurn { target: u64, seq0: u32, cmds: Vec<CommandKind> }
```

- **Bundle ack rule.** The client holds every turn below `held`, and turn
  `held + 1 + i` when bit `i` of `mask` is set. Turn `held` itself is
  missing by definition.
- **Stamping.** Once per period the client takes up to 16 queued commands
  (≤ 1,024 B) and stamps them `target = open turn + D`, where the open
  turn is computed from the synced relay clock, not from the newest
  bundle. Each stamp gets its own turn (`target ≥ previous target + 1`),
  and the commands get consecutive per-player `seq`s from `seq0`. The rest
  waits for the next period.
- **Input ack rule.** Every `Input` repeats every unacked stamped turn,
  oldest first, as many as fit. A command counts as acked once its `seq`
  appears in a received bundle.
- The relay accepts only the next `seq` of each player. Lower seqs are
  duplicates and are ignored. A gap stops reading that `Input`, and the
  client's resend fills the gap.
- `target` is clamped to at most `open + D + 10` (input stamped
  implausibly far ahead: a clock a little fast, or the stamps of the
  lead-in after a `Resume`), and never earlier than the previous accepted
  `seq`'s turn.
- A client does not stamp while it has not heard the relay for 2 s (it
  cannot see a pause from there); queued commands wait. On `Resume { at }`
  its next stamp starts again from `at + D`.

**Closing a turn.** At `close(T)` the relay closes turn `T` whatever has
arrived:

- it takes each player's waiting commands with turn ≤ `T`, in `seq` order,
  one at a time, alternating players (the first player alternates by
  tick);
- each player gets at most 16 commands and 1,024 B per turn, and the turn
  at most 1,100 B; a relay-injected `Surrender` may be a 17th;
- whatever does not fit, or arrived after its target closed, goes to the
  next open turn. That is a **restamp**. Input is never dropped;
- the relay overwrites `Command.player` with the sender's slot, keeps each
  command's original `target`, and sorts the turn by `(player, seq)`.

**`Bundle`** (relay → client):

```
Bundle     { confirmed: u64, peers: [PeerStat; 2], turns: Vec<Turn> }
PeerStat   { rtt_ms: u16, unstable: bool, gone: bool }
Turn       { tick: u64, salt: u64, cmds: Vec<BundledCmd> }
BundledCmd { cmd: Command { player: u8, seq: u32, kind: CommandKind }, target: u64 }
```

- `Turn T` holds the commands for `World::step` while `world.tick() == T`.
  `salt` is always 0 in S2.
- `target != tick` marks a restamp. Both clients count them from the
  bundles (§10 "Re-stamp transparency").
- `confirmed`: the highest tick where both clients' hashes agreed.
  Checkpoints at or below it are safe.
- **Delivery.** The relay sends unacked turns **oldest first**, whenever
  a turn is new (just closed) or due for a resend: still unacked
  `max(period, rtt_p99 + period/2)` after its last send (200 ms while the
  RTT is unknown). The first datagram of each such send repeats **every
  unacked turn** from the oldest, as many as fit, so a lost datagram
  costs one period, not a resend timeout. Further datagrams in the same
  period carry only due turns. At most **8 bundle datagrams per client
  per period**, so a 10 s gap (≈ 300 turns) refills in order in a few
  periods.
- The client buffers out-of-order turns (up to 4,096 ahead) and hands them
  to the sim strictly in tick order, each exactly once.
- **Client pacing.** The client steps turn `T` at `close(T) + P·period`,
  mapped to local time, where the playout delay `P = clamp(ceil((rtt_p99
  / 2 + jitter) / period), 1, 6)` is fixed at `Start` from the lobby's
  pings. It is local (when this client shows a tick) and changes no
  result. It never steps past the newest contiguous bundle, and catches
  up at most 3 ticks per period. Click to reaction on screen is about
  `(D + P)` periods.

### `CommandKind`

The postcard encoding of `frontline_sim::CommandKind` (variant index, then
fields). Variants, in index order: `MoveSoldier`, `Spawn`, `Despawn`
(bench only; the match sim rejects them), `Move`, `AttackMove`, `Stop`,
`Retreat`, `Dig`, `Barrage`, `CallIn`, `Surrender`, `MoveUnits`,
`AttackMoveUnits`, `StopUnits`, `RetreatUnits`, `DigUnits`, `SetStance`,
`Regroup`, `Throw`, `ThrowEach`, from version 3 `ReverseUnits`,
`FaceUnits`, `AttackUnit`, `SetAmmo`, `RepairUnits`, `FollowUnits`, and
from version 4 `SuppressUnits { units, line }`, and from version 5
`Recrew { squad, vehicle }` (a `u32` squad id and a `u32` entity id) and
`Abandon { squad }`. `Dig` and `DigUnits` end
with `profile` (`DigProfile`: 0 trench, 1 AT ditch). `SetAmmo` carries an
`AmmoChoice` as its variant index: 0 `Auto`, 1 `Ap`, 2 `He`, and from
version 4 3 `Apcr` (the special penetrator; the standard one on a gun
without it). Every variant with a `units` list is held to 120 ids, every
dig line to 16 points and a `SuppressUnits` line to 1–2 points
(`command_in_limits`, an exhaustive match). Positions are Q16.16
`i32` pairs. The sim
validates every command (dead units, foreign units, out-of-range values)
identically on both clients, so the relay does not.

## Silence, pause and timeout

| Silence of a slot | Relay action |
|---|---|
| 2 s | `PeerStat.unstable`. Turns keep closing without that player's orders. |
| 10 s | `Paused { at, cause: Waiting(slot) }`. Turns stop closing. |
| back, and its ack `held ≥ at` | `Resume { at, close_at = now + 300 ms }`. |
| 60 s after the pause, if it has not caught up | The relay closes a `Surrender` for the slot with its next `seq`, queues `Bye(TimedOut)` to it, and resumes. This applies whether the slot is still silent or is heard again but never holds the bundles up to `at` (a one-way link, or a client that will not ack). |

- A player pause (`PauseReq`) turns into a `Waiting` pause if a slot then
  goes silent for 10 s, so the timeout still runs.
- `Bye(Quit)` mid-match: the relay closes a `Surrender` for the slot at
  once, with no pause.
- A late datagram from a timed-out slot is not executed. The slot only
  receives its queued `Bye(TimedOut)`.
- The match on the relay ends when no slot is still connected (each left,
  timed out or said `Bye`).

## Hash check

- Clients send each published hash (`Up::Hash`).
- When both slots' hashes for a tick are in, the relay compares them.
  Equal: `confirmed` advances. Different: `Desync { tick, hashes, at }`
  with `at` = the next turn to close, and no more turns close.
- Both clients stop before turn `at`, at most 30 ticks plus one RTT after
  the divergence, and write their dump (`manifest.json`, `checkpoint.sav`,
  `paused.sav`, `bundles.jsonl`). The relay log gives `relay.jsonl`.
  `frontline-cli desync-diff slot0/paused.sav slot1/paused.sav` names the
  fields that differ.

## Security notes

- All network input is untrusted. Lengths are checked before decoding,
  every list and string is checked after, and no input can panic either
  side (fuzz-tested).
- Tokens are 64-bit, from SipHash under per-process OS-random keys.
  Without encryption, an on-path attacker can read a token and forge that
  player's input. This is accepted in S2.
- The relay executes nothing it receives. It overwrites `Command.player`,
  and the sim rejects orders for units a player does not own.
- The only way the relay refuses input is the 256-command flood cap.
  An honest client never reaches it.
- Pre-auth: a token bucket per source IP (8 `Hello`s, 4 a second), at
  most 4,096 addresses tracked. When the table is full it is swept at
  most once a second, so a spoofed-source flood costs one lookup per
  datagram. Rejects are logged at most once a second (`more` counts the
  rest).
