# Playing Frontline online

A two-player match needs one thing: both players can reach one **relay**.
The relay is a small program that keeps the match clock and forwards
orders. It runs inside the host's game when you click **Host**, or on its
own as `frontline-relay`. Everything uses **UDP port 47015** unless you pick
another.

Pick the first option that fits:

| Situation | Use |
|---|---|
| Same home or office network | [LAN](#lan) |
| Different places, and you can install an app | [Tailscale](#tailscale-recommended) (recommended) |
| Different places, and the host can open a router port | [Direct IP](#direct-ip-port-forwarding) |
| Neither, or a group that plays often | [A relay on a VPS](#a-relay-on-a-vps) |

Both players need **the same build** (the same zip). The game refuses a
player whose build or mod content differs, and the lobby says why.

## Before the first match (macOS)

### Install the game you were sent

The zip is not signed by Apple, so macOS blocks it until you clear the
download flag:

1. Unzip `Frontline-<commit>-macos.zip`.
2. In Terminal, in the folder where `Frontline.app` is, run:

   ```sh
   xattr -dr com.apple.quarantine Frontline.app
   ```

3. Open `Frontline.app`.

Both players must use the same zip: the build id inside it has to match.

### The incoming-connections prompt

The first time you **Host**, macOS may ask: *"Do you want the application
Frontline.app to accept incoming network connections?"* Click **Allow**.
If you click Deny, nobody can join you; change it later in System
Settings → Network → Firewall → Options. Joining a match never needs this.

## LAN

1. The host picks **Multiplayer → Host** (port 47015 by default). The lobby
   shows the address to give the friend, for example "Friends join:
   192.168.1.20:47015" (LAN and Tailscale addresses; not the public one).
2. The other player picks **Multiplayer → Join** and enters that address.

On a LAN the lobby suggests an input delay of D = 2 (about 67 ms).

## Tailscale (recommended)

[Tailscale](https://tailscale.com) gives both computers a private address
that works through almost any router or mobile network, with no port
forwarding. It also encrypts the traffic (the game's own protocol does
not).

1. Both players install Tailscale and sign in. The host shares their
   machine with the other player (or both join the same tailnet).
2. The host finds their Tailscale address: it starts with `100.` (for
   example `100.101.102.103`), shown in the Tailscale menu.
3. The host picks **Host**. The joiner picks **Join** and enters
   `100.101.102.103:47015`.

Expect D = 2 or 3 between cities. Tailscale sometimes relays traffic
through its own servers (DERP) when it cannot connect directly; that works
but adds latency, and the lobby's suggested D goes up. The lobby's
"Friends join" line lists the host's 100.x address too.

## Direct IP (port forwarding)

1. On the host's router, forward **UDP** port 47015 to the host computer's
   LAN address. Every router does this differently; look for "port
   forwarding" or "virtual server".
2. The host finds their public IP address (for example by searching "what
   is my IP" in a browser).
3. The host picks **Host**. The joiner enters `<public IP>:47015`.

**Carrier-grade NAT.** Some internet providers, and almost all mobile
networks, share one public address among many customers. Behind such a
network the host cannot receive connections, even with port forwarding.
Signs: the router's WAN address starts with `100.64.`–`100.127.`, `10.` or
`192.168.`, or does not match the "what is my IP" address. The joiner can
be behind carrier-grade NAT; only the host cannot. If the host is, swap
roles, or use Tailscale or a VPS relay.

IPv6 addresses work too: enter them in brackets, `[2001:db8::20]:47015`.
The host must allow incoming UDP 47015 on its IPv6 firewall, if it has
one.

## A relay on a VPS

Any small Linux server (around $5 a month) can run the relay. Both
players then **join** it; neither hosts, and no home router needs
changes. Nothing about the relay assumes it plays.

1. Build the relay for the server (it is in the game's source tree):

   ```sh
   cargo build --release -p frontline-net --bin frontline-relay
   ```

   (Build on the server itself, or cross-compile for its OS.)
2. On the server, open UDP port 47015 in its firewall, then run:

   ```sh
   ./frontline-relay --port 47015 --log relay.jsonl
   ```

   It prints its build id and every join, start, pause and desync. After
   a match ends it opens a new lobby on the same port (`--once` exits
   instead).
3. Both players pick **Join** and enter `<server address>:47015`. The
   first to join is slot 0 and chooses the seed, map and input delay.

The relay does not need the same build as the players; it checks that the
two **players** match each other. It must speak the same protocol version
(it says which one if not).

Options: `--bind <IP>` (default `::`, IPv4 and IPv6), `--max-delay <2–6>`
(the highest input delay a lobby may pick), `--log-no-turns` (a smaller
log), `--allow-build-mismatch` (development only).

## What the lobby numbers mean

- **Ping**: round-trip time to the relay. With the in-game relay, the
  host's ping is near 0.
- **Input delay D**: how many 33 ms ticks it takes an order to reach the
  relay's match clock. The lobby suggests one from both players' pings
  (about half a round trip plus jitter); slot 0 can override it (2–6)
  before Ready. A higher D means fewer late orders on a bad connection,
  but slower reactions. D never changes during a match.
- **Playout**: each game also shows every tick a little after the relay
  closes it, so the trip back does not make the picture stutter. It is
  sized the same way (1–6 ticks) and only affects that one screen. On
  screen, units react about (D + playout) × 33 ms after a click: about
  100 ms on a LAN, 130–170 ms between cities.
- **Late** (in the match readout): orders that reached the relay after
  their tick had closed and ran one or more ticks later. A few on Wi-Fi
  or a lossy link are normal; many mean D is too low.
- **Stalls**: ticks this game showed later than planned because a bundle
  from the relay was late. A few at the start are normal.

## When things go wrong

- **"Waiting for <name>"**: the other player's game has been silent for
  10 s. If they come back within 60 s, the match resumes by itself. After
  60 s they surrender automatically, and you win.
- **Desync**: the two games computed different results (a bug). The match
  stops, and both games write a dump to
  `~/Library/Application Support/Godot/app_userdata/Frontline/desync/`.
  Please send both players' dump folders with a bug report.
- **"Different build" / "different content"**: one player has another zip
  or edited mod files. Use the same zip.
