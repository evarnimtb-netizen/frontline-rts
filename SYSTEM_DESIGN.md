# Frontline RTS — System Design (Draft v0.1)

> Working title. A WW2-era tactical RTS in the "direct control + simulated ballistics" genre:
> squad-scale battles, per-soldier inventories, crewed vehicles with real armor values,
> destructible terrain, and the option to grab any single unit and drive/aim it yourself.
>
> **Two non-negotiable goals**
> 1. **Performance** — thousands of simulated entities and projectiles at a stable frame rate: 2,000 soldiers + 300 vehicles (Regiment tier) on a 6-core desktop, 700 + 100 (Battalion tier) on a Steam Deck (§2.2).
> 2. **Moddability** — humans *and AI agents* can add or change units, weapons, maps, missions and AI without recompiling the game.

Status: draft for discussion. Sections marked **[DECIDED date]** record final owner decisions; **[OPEN]** items are unresolved (see §17).

---

## 1. Design Principles

| # | Principle | Why it matters |
|---|-----------|----------------|
| P1 | **Simulation is headless and separate from rendering.** | Run the sim in CI, on servers, in agent test loops — no GPU needed. |
| P2 | **The simulation is deterministic.** | Enables lockstep multiplayer, tiny replays, reproducible bug reports and automated balance testing. |
| P3 | **Data-oriented (ECS) all the way down.** | Cache-friendly iteration over thousands of entities; easy parallelism. |
| P4 | **Everything is content; the base game is just the first mod.** | If the shipped game can be built with the mod tools, so can everything else. |
| P5 | **Text-first, schema-validated formats.** | Diffable, mergeable, and directly writable by AI agents with immediate validation feedback. |
| P6 | **Every player action is a Command.** | UI, AI, scripts, network and replays all go through one path. No hidden side doors. |
| P7 | **Budgets are explicit.** | Each system has a deterministic per-tick work-count budget and a per-stage ms target (§2.2), both measured and enforced in CI. |

---

## 2. Scope & Target Numbers

### 2.1 Gameplay feature set (MVP → full)

- Squad-based infantry: stances (stand/crouch/prone), cover use, suppression, morale, individual inventories (weapons, ammo, grenades, kits).
- Vehicles: tanks, halftracks, trucks, artillery; seat/crew model (driver, gunner, commander, loader); component damage (engine, tracks, turret ring, gun, optics, ammo rack, fuel).
- **Direct control (core feature, §5.12)**: take over any of your units (soldier, tank, gun, mortar) and move/aim/fire by hand with WASD + mouse or controller, using the same simulation as AI control; the rest of your army keeps fighting.
- Ballistics: every round is a simulated projectile; penetration vs. armor thickness, angle and material; HE/AP/APCR/HEAT/smoke.
- **Deformable terrain & player fortification (core pillar, §5.10)**: dig trenches, foxholes, gun pits and anti-tank ditches; raise berms and sandbag walls; build dugouts, bunkers and buildings. All of it gives *real*, geometry-based protection, and all of it can be destroyed.
- Destructible environment: buildings (modular pieces with support graphs), fences, trees, craters that permanently reshape terrain.
- Engineering: mines, wire, obstacles, demolition, repair and resupply.
- Logistics: ammo, fuel, repair; supply trucks; reinforcements via a points economy.
- Modes: PvP (up to 10 players, 5v5) and co-op PvE (1–5 players vs AI) are the launch priorities (§2.5); skirmish/conquest, scripted missions, solo vs AI; persistent campaign post-launch. Battalion or Regiment battle-size tiers (§2.2).

### 2.2 Battle scale & performance targets **[DECIDED 2026-10-31: two lobby tiers]**

> Decided: two lobby tiers (Battalion + Regiment), and 8 GB PCs stay supported at a Low preset (§17 Q4, Q12).

> In lockstep every client simulates the whole battle, so sim cost cannot be scaled per client. Battle size is therefore a **lobby-level tier**, and rendering scales per client. All ms figures are **estimates**: kernels were measured on an M3 and scaled to Zen 2, ±40 %. They hold until the M0 spike measures real hardware. Tier caps are data (`defs/rules/battle_tiers.toml`) and change with measurements.

**Battle-size tiers.** A tier sets only total entity caps and work budgets. Per-player share = tier cap ÷ players, up to 500 soldiers / 75 vehicles per player. Bots count toward the caps.

| Tier | Soldiers / vehicles / projectile pool | Players | Map | Global rays/tick (est.) | Path expansions/tick | Nav re-bake units/tick | Use |
|---|---|---|---|---|---|---|---|
| **Battalion** (standard) | 700 / 100 / 1,800 | 1–10 | ≤ 3×3 km | 2,400 | 10k | 4 | All lobbies, co-op, single-player; ranked 1v1–3v3 once ranked play exists (post-launch, §2.5). **Steam Deck = minimum spec** |
| **Regiment** (grand) | 2,000 / 300 / 5,000 | 1–10 | ≤ 4×4 km | 7,500 | 15k | 8 | Custom lobbies and scheduled events; every member must be Regiment-eligible. Single-player on any device except 8 GB PCs (may run below real time in heavy fighting) |

**Gameplay rules are the same in every tier:**
- **Tick rate.** 30 Hz. `tick_hz` is a match constant from M0. 20 Hz is allowed only as a wholesale replacement for 30 Hz, after a blind direct-control feel test. It does not bring the Deck into Regiment: Deck at 20 Hz ≈ 31 ms mean / 47 ms P99 per 50 ms tick (est.).
- **Projectiles.** Integrated at fixed 1/60 s substeps, so trajectories match at any tick rate.
- **Spotting cadence.** Every observer–candidate pair is refreshed at least every 1.0 s. Priority pairs are refreshed at least every 0.2 s: directly controlled units (in-cone pairs only, capped, §5.12.3), gunners, firing units, targets within 150 m, and units that moved more than 2 m. One sample credits at most 0.5 s of exposure.
- **How spotting is built.** Squad-level ray bundles with per-member refinement, cached terrain-blocked pairs, and a max-mip pretest. The budget is sized for about 100 candidates per observer. Denser fronts slow non-priority pairs (to at most 2.0 s) identically on every client.
- **Ray budget.** One global budget per tick, split perception 70 %, AI cover/fire-position 20 %, weapon line-of-fire 10 %. Rays include the building-occluder BVH.
- **HE.** 32 fragment rays per shell, weighted to the def's lethality and checked statistically in CI.
- **Overflow.** Small-arms rounds beyond a player's pool quota merge into deterministic burst bundles. AP/HE/AT shells are always fully simulated. Rounds fired by a directly controlled unit are never bundled; they are served first from the owner's quota (§5.12.3).
- **In-sim budgets are work counts, never ms.** Rays, node expansions, re-bake units, terrain-op cells (per-tier value set at the M2.5 terrain spike, in `battle_tiers.toml`), and at most 4 detail-tile inits per tick. They are served per player, round-robin, with a seeded rotation.

**Hardware**

| Class | CPU | GPU | RAM | Tiers |
|---|---|---|---|---|
| Minimum | Steam Deck LCD/OLED (4C/8T Zen 2, ~16 W shared APU); PCs that pass the Battalion SCS | 8 CU RDNA2 / GTX 1050 Ti class | Deck 16 GB unified; PC 16 GB, or 8 GB at the Low preset (≤ 4 GB total, verified at M0) | Battalion; Regiment single-player only (not on 8 GB PCs) |
| Recommended | 6C/12T Ryzen 5 3600 / i5-10400 or better | GTX 1660 at 1080p60 or 1440p60 with FSR2 Quality; RTX 3060 / RX 6600 at 1440p60 high | 16 GB + 6 GB VRAM | Battalion + Regiment (if the SCS passes) |

Other devices (Z2 Go handhelds, Steam Machine) are classed by measurement and are not listed in advance.

**Lockstep with mixed hardware**
1. **Relay-closed turns.** All multiplayer goes through a relay: an official one, or a host-run one for custom lobbies.
   - The network thread samples input once per tick period on a wall-clock schedule (1/`tick_hz`: 33.3 ms at 30 Hz).
   - The relay closes tick N at its deadline and re-stamps late commands to the next open tick. Input delay D is set at match start from the worst client's P99 one-way latency, capped per lobby (§10).
   - Each packet repeats the sender's last 4 ticks of commands.
   - A slow CPU delays only its own view. No peer ever waits on another peer's CPU.
2. **Eligibility (Sim Capability Score, SCS).**
   - Deck LCD/OLED use a device table set by M0 measurements.
   - Unknown PCs run a paced 30 Hz benchmark on a late-game snapshot while the presenter renders at the user's settings. It runs during the first battle load and never blocks a screen.
   - A device gains a tier at P95 ≤ 13.5 ms and P99 ≤ 18 ms. It loses the tier only after two runs above 15 / 20 ms.
   - Before each match the game checks power state (Deck TDP below 15 W, battery saver) and warns.
3. **Lobby.** The host picks the tier, and a lobby never changes tier when someone joins. Ineligible players can't join. Adding one during setup asks the host to accept the lower tier.
4. **Governor.** The relay judges each client by its acknowledged tick; self-reports are only hints.
   - A client more than 15 ticks behind for over 10 s makes the relay lower match speed, floor 0.8×. This is a `SetSpeed` command at N+D; speed changes only the turn period, and no sim system reads it.
   - Speed drops quickly and recovers slowly. Everyone sees the speed; the slow player is not named.
   - A client still behind at 0.8× for 60 s gets a `SetController` command at N+D. Its units go to a teammate (default) or to AI, any direct control is released (§5.12.1), and the player spectates.
   - That player may rejoin if headless catch-up is estimated at 60 s or less.
5. **Join-in-progress.**
   - The snapshot comes from the official relay's headless sim, or from the fastest non-handheld peer. Never from a Deck.
   - The joiner fast-forwards headless with the presenter paused.
   - Joining is refused if catch-up would take over 60 s.
6. **Deterministic budgets.**
   - Quotas are assigned before each parallel phase, using prefix sums and per-observer rotating cursors.
   - Parallel stages may not use atomics or read a "remaining budget" (enforced by lint).
   - Every queue, cursor, accumulator and non-pure cache is in the snapshot and the state hash.

**Steam Deck presenter profile** (set per client, no effect on lockstep)
- **Frame rate and resolution.** 30 fps lock (40 fps option on OLED for light maps). 1280×800 output at 0.75 render scale + FSR1. Forward+ renderer; Mobile gets an A/B test at M0 but is not planned on.
- **Power and threads.**
  - GPU at most 60 % busy at about 1.0 GHz (≈16–20 ms per frame), so the APU's ~16 W budget goes to the CPU.
  - Presenter uses at most 1.5 cores. Godot `threading/worker_pool/max_threads = 2`. Sim threads are pinned.
- **Units.**
  - Rust-side culling into one MultiMesh per (mesh, LOD), with `visible_instance_count` and `custom_aabb`.
  - Bone-matrix texture skinning. At most 32 full skeletons, using AnimationPlayer.
  - LOD distances: LOD0 ≤ 25 m, LOD1 ≤ 80 m, LOD2 ≤ 200 m, impostors beyond.
- **Scene limits.**
  - At most 1,500 draws and 2 M triangles across all passes.
  - One directional light with 2 × 2048 cascades, out to 150 m.
  - Volumetric fog, SSAO, SSR, SSIL and SDFGI off. Gameplay fog uses a cheap depth/height fog that keeps its minimum opacity.
  - 20k particles, 256 decals, at most 64 transient lights.
  - Terrain from GPU height textures (CDLOD).
- **Auto-degrade.** Before the sim is ever delayed, the presenter drops quality in this order: render scale → particles → skeletons → shadows → 20 fps floor.
- **Fairness.**
  - A spotted unit is always drawn, with a class marker readable at 800p.
  - Smoke, gameplay foliage and fog keep a minimum opacity on every preset.
- **Required workstreams.** 800p UI scale and a controller scheme (radial orders, squad cycling, direct control with gyro aim, §5.12.7). Battery life is about 1.5–2 h (est.).

**Per-stage sim budget** (ms per tick; the budget is the mean, and every tier must also meet P95 ≤ 15 and P99 ≤ 20)
- **Threads.** The sim thread joins the job pool. Deck: 3 sim threads ≈ 2.0 effective cores. 3600: 4 threads ≈ 3.5 effective cores; the presenter keeps 2 cores at 60 fps.
- **Estimate basis.**
  - Clocks: Deck 2.6 GHz under GPU load, 3600 4.0 GHz all-core.
  - Added costs: +20 % for building occluders, an allowance for unmodelled AI and Luau, fork/join overhead, and 2 ballistic substeps per tick.

| Stage | Deck @ Battalion est. (CPU / wall) | **Deck budget** | 3600 @ Regiment est. (CPU / wall) | **Desktop budget** |
|---|---|---|---|---|
| Intent + AI + Luau (order apply and Luau serial; AI decision parallel per player with sorted command merge; instruction-capped) | 2.5 / 1.6 | 1.75 | 4.6 / 1.6 | 1.75 |
| Perception (rays + per-squad candidates) | 8.1 / 4.1 | 4.0 | 10.0 / 2.85 | 3.0 |
| Movement + ORCA (Q16.16) | 1.2 / 0.6 | 0.75 | 2.2 / 0.65 | 0.75 |
| Pathfinding + flow fields | 2.2 / 1.1 | 1.25 | 2.0 / 0.6 | 0.75 |
| Weapons + ballistics | 2.3 / 1.15 | 1.5 | 3.8 / 1.1 | 1.25 |
| Damage (serial) + status | 0.8 / 0.65 | 0.75 | 1.4 / 0.95 | 1.0 |
| World (terrain ops, re-bake units) | 1.4 / 1.0 | 1.0 | 1.5 / 0.7 | 0.75 |
| Hash, snapshot, misc, fork/join | 1.2 / 0.95 | 1.0 | 1.0 / 0.65 | 0.75 |
| **Total** | **19.7 / 11.2** | **12.0** | **26.5 / 9.1** | **10.0** |

- **No Deck margin.** The Deck's Battalion fit has no margin against a ±40 % error. If M0 measures higher, the caps shrink by the measured ratio.
- **Deck at Regiment.** ≈50 CPU-ms per tick, ≈26 ms mean and ≈39 ms P99 (est.). Not supported in multiplayer.
- **The ray budget is mandatory.** The old "re-check 1/4 of candidates per tick" rule would be about 57k rays per tick, ≈51 ms on a Deck.
- **Required code choices:**
  - Q16.16 / i64 in hot loops; I32F32 is ≈3× slower in ORCA.
  - Tiled/Morton heightfield layout plus a max-mip pyramid, because the Battalion battle-area heightfield (≈18 MB of u16 heights, ≈27 MB with material at 3 × 3 km; §5.10.1) is far bigger than the Deck's 4 MB L3.
  - A BVH for the armour narrowphase.
  - An incremental per-chunk state hash.
  - Nav re-bake in ~0.2 ms work units, with pessimistic immediate invalidation, so stale nav is suboptimal but never wrong.
  - An immediate coarse HPA* path, then budgeted refinement.

**RAM budgets (est.)**
- **Sim:** ≈0.35–0.5 GB, identical on every client. The 512-tile detail pool is sim state.
- **Deck:** at most 5.5 GB total including GPU memory; hard cap 7 GB.
- **Desktop:** RAM at most 6 GB, plus VRAM 4–4.5 GB at high and at most 7 GB at ultra.
- **8 GB PCs:** Low preset at most 4 GB total, Battalion only, maps up to 2×2 km; lobbies on larger maps treat them as ineligible. Decided 2026-10-31.

**Other targets, unchanged:**
- Load time under 15 s for a large map from SSD.
- Replays under 2 MB per hour for 10 players in RTS play, plus at most 0.5 MB per player-hour of direct control (est., delta-encoded, §11).
- A 200-shell barrage plus 20 squads digging stays inside the work budgets.
- At most 512 detail tiles live at once (pool cap); flat-again tiles are demoted.

**M0 spike** (real Deck LCD at 15 W and Deck OLED, Ryzen 5 3600, M3), with pass/fail:
1. **Kernels**, run with 3 threads under the GPU load scene. Pass: Deck LOS ≤ 2.9 µs per ray (tiled heightfield, max-mip, occluder BVH) and ORCA ≤ 0.9 µs per agent. Fail: scale the Battalion caps by the ratio.
2. **Paced 30 Hz composed tick**, on a late-game snapshot with the presenter running.
   - Pass: Deck Battalion P95 ≤ 13.5 ms and P99 ≤ 18 ms. The 3600 at Regiment must meet the same with the 60 fps presenter.
   - Fail: shrink caps by the measured ratio.
   - If the Deck needs caps below 500 soldiers / 70 vehicles, the owner decides: either a Zen 3+ minimum, or the Deck becomes a single-player/co-op device.
3. **Power and concurrency.** Pass: the Deck CPU holds ≥ 2.6 GHz under Battalion load with the Deck preset, GPU at most 60 % busy, presenter at most 1.5 cores, sim inflation versus headless at most 30 %.
4. **Spotting cadence** at the tier ray budgets. Pass: at least 99 % of pairs refreshed within 1.0 s, and priority pairs within 0.2 s.
5. **Memory.**
   - The tiled layout must cut heightfield L3 misses (measured with `perf`).
   - Deck Regiment scene at most 5.5 GB.
   - 8 GB iGPU PC on Battalion Low at most 4 GB. Fail: trim the Low preset (texture sizes, other presenter memory) and tighten the maps 8 GB PCs may join (the map-size limit, now 2 × 2 km, and the per-map detail-tile pool cap, which applies to every client in that lobby) until it fits. A 16 GB minimum is the last resort and needs owner sign-off.
6. **Determinism.** Identical hashes on SteamOS native, SteamOS + Proton, Windows and macOS ARM, plus a thread-count fuzz (1/2/3/8 workers) and save → load → continue mid-barrage.
7. **Rendering.** Bone-texture skinning works on a skeleton-less MultiMesh in Godot 4.7.2 (or via CUSTOM0/1 packing). Measure Forward+ versus Mobile GPU ms.
8. **Benchmark reliability.** SCS run-to-run variance under 10 % over 10 runs.

**After M0.** Re-check the caps at M2.5 (terrain), M4 (buildings) and M6 (AI); final caps at M6. No public minimum-spec claim before M6. Apply for Steam Deck Verified after M7.

**Sources**
- Steam Deck hardware: [Valve tech specs (PDF)](https://cdn.steamstatic.com/steamdeck/images/press/techspecs/steamdeck_techspecs.pdf), [Chips and Cheese: Van Gogh APU](https://chipsandcheese.com/p/van-gogh-amds-steam-deck-apu), [Valve developer wiki](https://developer.valvesoftware.com/wiki/Steam_Deck), [Notebookcheck: Deck OLED APU](https://www.notebookcheck.net/AMD-Steam-Deck-OLED-APU-Processor-Benchmarks-and-Specs.799065.0.html), [CPU-Monkey: Deck OLED](https://www.cpu-monkey.com/en/cpu-valve_steam_deck_2023_refresh), [CPU-Monkey: Deck LCD](https://www.cpu-monkey.com/en/cpu-valve_steam_deck_amd_custom_apu_0405), [Windows Central: Deck VRAM](https://www.windowscentral.com/gaming/pc-gaming/how-to-increase-vram-on-steam-deck), [AMD Z2 Go](https://www.amd.com/en/products/processors/handhelds/ryzen-z-series/z2-series/z2-go.html)
- Desktop CPU baselines: [CPU-Monkey: Ryzen 5 3600 Cinebench R23](https://www.cpu-monkey.com/en/benchmark-amd_ryzen_5_3600-cinebench_r23_single_core), [Geekbench 6 baseline](https://browser.geekbench.com/v6/cpu/baseline/204702), [Agner Fog instruction tables (PDF)](https://www.agner.org/optimize/instruction_tables.pdf)
- Godot rendering: [Kodeco: targeting the Deck with Godot](https://www.kodeco.com/41495624-targeting-the-steam-deck-with-godot?page=3), [renderers](https://docs.godotengine.org/en/stable/tutorials/rendering/renderers.html), [MultiMesh class](https://docs.godotengine.org/en/stable/classes/class_multimesh.html), [using MultiMesh](https://docs.godotengine.org/en/stable/tutorials/performance/using_multimesh.html), [volumetric fog](https://docs.godotengine.org/en/stable/tutorials/3d/volumetric_fog.html), [animating thousands of fish](https://docs.godotengine.org/en/latest/tutorials/performance/vertex_animation/animating_thousands_of_fish.html), [Godot 4.0 optimization report](https://godotengine.org/article/godot-40-optimization-progress-report/), [godot#68959](https://github.com/godotengine/godot/issues/68959), [forum: animated crowds](https://forum.godotengine.org/t/performance-of-crowds-of-animated-rigged-characters-in-3d/130409), [forum: empty-project memory](https://forum.godotengine.org/t/default-memory-usage-concerns-200mb-400mb-for-empty-project/129801)
- Genre and lockstep references: [SteamDeckHQ: Company of Heroes 3](https://steamdeckhq.com/game-reviews/company-of-heroes-3/), [CoH3 population cap](https://www.ggrecon.com/guides/company-of-heroes-3-population-cap/), [pixelnitro](https://pixelnitro.com/?p=20328), [Gates of Hell Steam discussion](https://steamcommunity.com/app/400750/discussions/0/3801652929331792530), [Gates of Hell Deck compatibility report](https://store.steampowered.com/saleaction/ajaxgetdeckappcompatibilityreport?nAppID=400750), [Recoil `GameServer.cpp`](https://github.com/beyond-all-reason/RecoilEngine/blob/master/rts/Net/GameServer.cpp), [Spring units of measurement](https://beyond-all-reason.github.io/spring/articles/units-of-measurement), [Factorio multiplayer wiki](https://wiki.factorio.com/Multiplayer), [Factorio FFF-147](https://factorio.com/blog/post/fff-147), [1500 Archers on a 28.8](https://www.gamedeveloper.com/programming/1500-archers-on-a-28-8-network-programming-in-age-of-empires-and-beyond), [Synchronous RTS engines 2: sync harder](https://www.gamedeveloper.com/business/opinion-synchronous-rts-engines-2-sync-harder)
- Hashing: [xxHash](https://github.com/Cyan4973/xxHash)

### 2.3 Setting & factions **[DECIDED 2026-10-31]**

**Base game (`ww2` content mod): June 1944 – early 1945, Eastern and Western Front, 3 factions, authentic.**

| Faction | Fronts | Indicative v1 roster (to be validated against sources) |
|---------|--------|--------------------------------------------------------|
| **Germany** (Wehrmacht/Heer 1944) | East + West (shared roster, front-specific variants and camouflage) | Pz IV H/J, Panther A/G, Tiger I (late), Tiger II, StuG III G, Jagdpanzer IV, Hetzer, Sd.Kfz. 251, Sd.Kfz. 234, PaK 40, 8.8 cm FlaK, leFH 18, Nebelwerfer |
| **USSR** (Red Army 1944) | East | T-34-85, T-34/76 (1943), IS-2, SU-85, SU-100 (late '44), ISU-152, SU-76M, lend-lease M4A2(76)W, BA-64, ZiS-3, 53-K, ML-20 |
| **USA** (US Army 1944) | West | M4A1(76)W, M4A3(75)W, M4A3E2 "Jumbo", M4A3E8 (from Dec '44), M10, M18, M36, M5A1, M24 (from Dec '44), M8 Greyhound, M3 halftrack, 57 mm M1, 105 mm M2 |

- **Matchups:** Germany vs USSR (East) and Germany vs USA (West). USSR vs USA is allowed only in an explicit "free skirmish" setting.
- **Roster size:** about 15–25 vehicles + about 10–15 infantry squad types per faction. Units carry `available_from`/`available_to` dates, so missions and modes can filter by date.
- **Theaters & map biomes:**
  - East: Belorussian forests and marshes (Bagration), villages, river crossings, the German defensive lines.
  - West: Normandy bocage and coast (Atlantic Wall), French towns, the Siegfried Line, the Ardennes in winter (Dec 1944).
- **Fidelity: authentic.**
  - Real designations, organization (squad/platoon TO&E) and armor/penetration data from published sources. Each data file cites its source in a `sources = [...]` field.
  - National insignia: Balkenkreuz, Red Star, US white star. **No swastikas or other banned symbols** (legal in Germany under StGB §86a, storefront policies), enforced by a validator rule for the base game and recommended to mods.
  - Voice lines in German, Russian and English via locale packs.
- **Expansion path (data-only, using the mod system):** more factions (UK/Commonwealth, Finland, Hungary, Romania, Italy…), earlier war (1939 – mid-1944) and the end of the war (spring 1945), and other theaters. Expansions are mods that depend on `ww2`; they add units and patch in era-specific rules.

### 2.4 Era-agnostic engine (future Cold War / modern content)

The engine must not assume WW2. Later content packs (`coldwar`, `modern`) should be **new mods on the same engine**, not a fork. Things that would be expensive to retrofit are designed in now. Things that are cheap to add later are only *reserved*.

| Area | Designed in now | Reserved for later eras |
|------|-----------------|-------------------------|
| Content split | `core` mod = era-agnostic rules (materials, base behaviours, UI shell, soil types); `ww2` mod = all base-game content, Jun 1944 – early 1945 (§2.3, §7.1) | `coldwar`, `modern` mods depend on `core` only |
| Armor | Per-face `material` + **effectiveness matrix** `material.vs[mechanism]` (kinetic, chemical/HEAT, blast, fragment) instead of one `material_factor` (§5.7). `material.vs` is keyed by mechanism class; each ammo `mechanism` maps to a class (ap/aphe/apcr/apds → kinetic, heat → chemical, hesh/he → blast, frag → fragment; smoke has no armor effect) | composite and spaced armor (data only); **ERA tiles** and slat armor as destroyable armor components |
| Ammunition | `mechanism` field (`ap`, `aphe`, `apcr`, `apds`, `heat`, `hesh`, `he`, `frag`, `smoke`) | `apfsds`, `tandem_heat`, `top_attack` (data) |
| Guidance | Projectile has an optional `Guidance` component slot; v1 has only `ballistic` | `saclos`, `beam_riding`, `ir_fire_and_forget`, `laser`, with deterministic steering each tick; countermeasures (smoke, IR jamming, active protection systems) |
| Sensors | Perception (§5.6) modelled as **sensor channels**: v1 = `visual_day`, `visual_night`, `acoustic`; targets have a signature per channel | `thermal`, `radar`, `image_intensifier`; multispectral smoke blocks thermal |
| Fire control | Accuracy from data: `rangefinding`, `stabilization` (none/vertical/two-axis), `fire_on_move_penalty` | ballistic computers, laser rangefinders (data) |
| Movement layers | `Transform` is fully 3D; nav layers are a data list (`infantry`, `wheeled`, `tracked`) | `air_low` layer for helicopters and drones, flying above heightfield/buildings with simple avoidance |
| Map scale | Terrain resolution tiers & streaming sized by data; battle maps ≤ 3 × 3 km (Battalion) / ≤ 4 × 4 km (Regiment) (§2.2), with fixed-point range checked for up to 16 × 16 km | modern 2–4 km engagements on 8 × 8 km maps |
| Off-map support | Generic `SupportCall` (artillery, air strike, recon, supply drop) defined in data | drones, guided artillery, counter-battery radar |
| Fortifications | Pieces & blueprints are generic (§5.10.3) | HESCO barriers, concrete T-walls, modern trench kits (data only) |
| Command & comms | Faction knowledge maps (§5.6) | radio/datalink sharing delay, electronic warfare as a modifier |

**Rule for reviewers:** a PR that hard-codes an era-specific concept (e.g. a `"panzer"` string check or a "WW2-only" branch) in engine code is rejected. That concept belongs in data or in a mod.

### 2.5 Product priorities **[DECIDED 2026-10-31]**

**Multiplayer first: PvP and co-op PvE are the launch priorities.** Solo skirmish vs AI comes almost for free, since it's a co-op lobby with one human, but it doesn't drive scope.

| Launch scope | Status |
|---|---|
| **PvP** (1v1 – 5v5, Battalion + Regiment tiers) | **Launch.** Starts as **custom lobbies** with a server browser and host-run relays. No ranked play at launch. |
| **Co-op PvE** (1–5 players vs AI, conquest/skirmish and missions) | **Launch.** Same lobbies and relays as PvP. |
| **Mission set** (standalone historical missions, East + West) | **Launch.** Playable solo or in co-op. |
| **Solo skirmish vs AI** | Launch, as a by-product of co-op PvE. |
| **Ranked matchmaking** | **Post-launch.** Added once balance and player numbers justify it. Battalion tier only (§2.2). Needs official relays, ratings and abandon/demotion rules. |
| **Persistent campaign** (roster, veterancy, strategic map) | **Post-launch.** Design to be decided separately (§17 Q5). The campaign state format in §11 is reserved. |

**What this means for development:**
- **Multiplayer is built from the start, not bolted on.** A minimal 2-client lockstep match through a local relay is part of M1, and every later feature is tested in a networked match by default (§15).
- **AI quality matters for launch.** Co-op PvE depends on it, so AI (M6) is launch-critical, not polish.
- **No official game servers are required at launch.** Custom lobbies use host-run relays. Join-in-progress snapshots come from the fastest non-handheld peer (§10). Official relays arrive with ranked play. The only official service at launch is the mod id registry used when publishing (§7.8).
- **Balance work starts early:** `frontline-cli batch` balance runs (§8) and PvP playtests from M6 on.

---

## 3. High-Level Architecture

```
┌────────────────────────────── Client process ────────────────────────────────┐
│                                                                              │
│  ┌──────────────┐  Commands   ┌───────────────────────────┐                  │
│  │  Input / UI  │────────────▶│      Command Pipeline     │                  │
│  └──────────────┘             │  (validate, timestamp,    │                  │
│                               │   route to sim tick N+D)  │                  │
│                               └────────────┬──────────────┘                  │
│  ┌──────────────┐  Commands   ┌────────────▼──────────────┐  ┌────────────┐  │
│  │  AI Players  │────────────▶│   SIMULATION CORE (sim)   │─▶│  Snapshot  │  │
│  └──────────────┘             │  deterministic, headless, │  │  Buffer    │  │
│  ┌──────────────┐  Commands   │  fixed-point, ECS         │  │ (triple-   │  │
│  │ Mod Scripts  │────────────▶│  30 Hz fixed tick         │  │  buffered) │  │
│  └──────────────┘             │                           │  └─────┬──────┘  │
│  ┌──────────────┐  Commands   │                           │        │         │
│  │   Network    │◀───────────▶│                           │        │         │
│  │  (lockstep)  │             └────────────▲──────────────┘        │         │
│  └──────────────┘                          │ read-only defs        │         │
│                               ┌────────────┴──────────────┐  ┌─────▼──────┐  │
│                               │   Content DB (immutable   │  │ Presenter  │  │
│                               │   after load: units,      │  │ (render,   │  │
│                               │   weapons, armor, maps)   │  │ audio, VFX)│  │
│                               └────────────▲──────────────┘  └────────────┘  │
│                               ┌────────────┴──────────────┐                  │
│                               │  Mod Loader (layers,      │                  │
│                               │  patches, schema check)   │                  │
│                               └───────────────────────────┘                  │
└──────────────────────────────────────────────────────────────────────────────┘

Headless tools reuse sim + content + mod loader without the presenter:
  frontline-cli validate | simulate | ballistics | replay | schema | docs
```

AI players and sim Luau run inside the sim tick (§5.2 stage 1, §9); their Commands apply without the +D network delay. Only human input goes through the pipeline and relay.

**Key boundary:** the `sim` crate/module has **zero** dependencies on rendering, audio, OS time, threads it doesn't own, or floating-point-sensitive libraries. The presenter only *reads* snapshots and emits cosmetic effects (particles, sounds, ragdolls) that never feed back into the sim.

---

## 4. Technology Stack **[DECIDED 2026-10-31]**

### Decision: Rust simulation core + Godot 4.7 presenter + Luau scripting

**Pinned versions** (checked 2026-10-31; upgrade only deliberately, see §4.2):

| Component | Version | Notes |
|-----------|---------|-------|
| Godot Engine | **4.7.2-stable** (released 2026-08-18) | Latest stable. Use the standard (single-precision) build and the **Forward+** (Vulkan/Metal/D3D12) renderer. |
| godot-rust (`godot` crate, a.k.a. gdext) | **0.5.5** (2026-08-09), with feature `api-4-7` | Rust ↔ Godot bindings; the 4.7 API level was added in 0.5.4. |
| Rust toolchain | latest stable, pinned via `rust-toolchain.toml` | |

| Layer | Choice | Rationale |
|-------|--------|-----------|
| Language (sim) | **Rust** | C++-class performance, memory safety (matters with third-party mods), excellent tooling, strong WASM story, compiler feedback that AI agents can act on. |
| ECS | `bevy_ecs` used standalone in the sim crate, or a small custom archetype ECS | Proven, parallel scheduler; it's only a library here, and Bevy's renderer isn't used. |
| Math (sim) | Fixed-point: Q16.16 in i32/i64 for positions and all hot loops; `I32F32` only off hot paths (measured ≈3× slower in ORCA) + custom deterministic trig tables | Cross-platform determinism without fighting compiler float behaviour. |
| Math (render) | Godot `Vector3`/`Transform3D` (f32) | Render is allowed to be non-deterministic. |
| Engine / presenter | **Godot 4.7.2** via a GDExtension written in Rust (`godot` 0.5.5) | Mature editor (maps, missions, UI, VFX, animation), audio buses, asset import, Jolt physics for cosmetics, MIT license. Modders get the editor free. |
| UI & tools glue | GDScript / Godot scenes | Menus, HUD and editor plugins only. **Never** gameplay logic. |
| Scripting | **Luau** via `mlua` (sandboxed, typed, fast, no JIT needed) | Familiar to modders, gradual types help AI agents, easy instruction-count limits. GDScript is *not* exposed to mods for gameplay: it can't be sandboxed or made deterministic. |
| Heavy mods | **WASM** (`wasmtime`) for compiled plugins (custom AI, new systems) | Sandboxed, deterministic if host imports are deterministic, any source language. |
| Data | **TOML** for hand-authored defs, **JSON Schema** generated from Rust types (`schemars`) | Human-friendly + machine-validated. |
| Assets | glTF 2.0 (models), KTX2/Basis (textures), Opus/OGG (audio) | Open standards, every tool exports them. |
| Networking | Deterministic lockstep over UDP (`quinn`/QUIC) through a **required relay** that closes turns on a wall-clock deadline (§10) | Bandwidth is independent of unit count. |

**Alternatives considered**

- *Rust + Bevy* — the most agent-friendly (everything is code), single language, but pre-1.0 API churn, immature UI, and no editor (we would have to build the map/mission editor ourselves). Former recommendation; dropped in favour of Godot's editor and tooling.
- *C++ + EnTT + custom renderer* — max control, but slower iteration and unsafe mod surface.
- *Unity DOTS / Unreal* — capable renderers, but licensing risk, binary assets that agents and modders can't easily edit, and hard determinism.

### 4.1 Godot integration rules (performance-critical)

1. **The sim crate has no Godot dependency.** The GDExtension crate (`godot-bridge`) is a thin adapter: it owns the sim thread, forwards input as Commands, and copies snapshots out.
2. **No Node per unit.** Soldiers, vehicles, projectiles and tracers are drawn through `RenderingServer` / `MultiMesh` instances. Nodes are only for the camera, UI, the directly-controlled unit's attached effects, the capped near-camera `Skeleton3D` set of rule 4, and editor tooling.
3. **One batched transfer per frame.** The bridge writes all instance transforms + custom data into a `PackedFloat32Array` and calls `multimesh_set_buffer` once per MultiMesh (≈128 KB for 2,000 units: 12-float transform + 4-float custom data each). Never one FFI call per entity.
4. **Animation LOD.** Full `Skeleton3D` only near the camera and for the directly controlled unit. Desktop: up to 150 with `AnimationTree`. Steam Deck: up to 32 with `AnimationPlayer`. The controlled unit's skeleton counts toward the cap and is never dropped by auto-degrade (§2.2). All other units use **bone-matrix animation textures** (baked bone transforms, about 3–6 MB per rig, shared by every mesh on that rig) skinned in the shader on MultiMesh instances. Per-vertex VAT is not used (about 64 MB per variant).
   **Culling in Rust.** Godot MultiMesh has no per-instance culling. The bridge does frustum, distance and shadow-caster culling in Rust and writes only visible instances into one MultiMesh per (mesh, LOD), setting `visible_instance_count` and `custom_aabb`, plus a separate shadow-only caster MultiMesh.
5. **Godot physics (Jolt, built in) is cosmetic only:** ragdolls, debris, shell casings. It never affects the sim.
6. **Terrain** renders from GPU height textures: a base heightmap plus an R16 detail-tile texture array (128 KB per tile), drawn as a displaced CDLOD grid with triplanar texturing. An edit re-uploads a small texture rectangle. There is no CPU re-meshing (§5.10.7).
7. **Threading:** the sim runs on its own Rust pool outside Godot's main loop, and the sim thread joins that pool. Pool size = physical cores − presenter reserve (Deck: 2 workers + sim thread; 6-core desktop: 3 + sim thread). Godot `threading/worker_pool/max_threads` is capped (Deck: 2). Sim threads are pinned where the OS allows. The bridge reads the newest snapshot in `_process`. The presenter degrades before it ever delays a sim tick.
8. **Profiling:** Tracy in the Rust core + Godot's built-in profiler/monitors in the presenter; CI runs the headless sim benchmarks without Godot.

### 4.2 Upgrade policy

- The project stays on the pinned Godot + `godot` crate versions. A minor upgrade (Godot 4.8+ or a new `godot` crate minor) is its own task: bump both together, run the benchmark scenarios and replay tests, and check the godot-rust changelog for API-level changes.
- Patch releases (4.7.x): smoke test plus replay tests; bump the `godot` crate only if its changelog requires it.
- Mods and maps record the Godot version they were authored with (in `mod.toml`), so asset-format changes can be migrated.

---

## 5. Simulation Core

### 5.1 Tick loop

```
for each sim tick N (`tick_hz`, a match constant; 30 Hz):
  1. Collect commands scheduled for tick N (from all sources, sorted by (player_id, seq))
  2. Apply commands → order/intent components
  3. Run systems in fixed stages (below)
  4. Update incremental per-chunk state hashes; publish the combined hash every K ticks (no 30-tick spike)
  5. Publish snapshot N to snapshot buffer
```

Rendering runs at display rate and interpolates between snapshot N-1 and N.

### 5.2 System stages (fixed order)

| Stage | Systems | Parallel? |
|-------|---------|-----------|
| 1. Intent | order resolution, squad formation, AI decision, Luau (budgeted) | order apply + Luau **serial**; AI decision per player, sorted merge; instruction-capped (§2.2) |
| 2. Perception | spotting, LOS, sound events, knowledge maps | per-entity chunks |
| 3. Movement | path search (budgeted HPA* refinement, flow fields), path follow, local avoidance, vehicle kinematics, stance | per-chunk |
| 4. Weapons | aim, reload, fire → spawn projectiles | per-entity |
| 5. Ballistics | projectile integration, ray/segment tests, impacts | per-projectile chunks |
| 6. Damage | penetration, component damage, injuries, destruction events | **serial merge** |
| 7. Status | suppression, morale, fire, fuel, ammo, repair, healing | per-entity |
| 8. World | construction progress, terrain deformation, structure collapse, nav/cover re-bake (budgeted), cleanup, spawns | serial apply, parallel re-bake |

**Determinism rule for parallelism:** parallel systems may only write to their own entity's components, or emit events into per-thread buffers that are **sorted by a stable key** (entity id, event seq) before a serial merge stage consumes them. Budgeted work (rays, path expansions, re-bake units, projectile pool) is assigned to entities and players **before** the parallel phase by a deterministic serial or prefix-sum step. Parallel code never reads a shared 'remaining budget' or uses atomics for sim decisions (lint-enforced). CI runs every stage with 1/2/3/8 workers and requires identical hashes.

### 5.3 Core components (illustrative)

```rust
// Pure data, no methods with side effects.
struct Transform   { pos: FxVec3, yaw: Fx, pitch: Fx }
struct Velocity    { lin: FxVec3, ang: Fx }
struct Faction     (u8);
struct Health      { hp: u16, bleeding: u8 }
struct Stance      (StanceKind);
struct Morale      { value: Fx, suppression: Fx }
struct Inventory   { slots: SmallVec<[ItemStack; 12]> }
struct WeaponState { def: WeaponId, ammo_in_mag: u16, reload_left: u16, aim: FxVec3 }
struct Crew        { seats: SmallVec<[Option<Entity>; 6]> }
struct Components  { parts: SmallVec<[PartState; 16]> } // vehicle subsystems
struct Projectile  { def: AmmoId, pos: FxVec3, vel: FxVec3, owner: Entity, ttl: u16 }
struct DirectControl { player: PlayerId, station: StationId, control_gen: u16, held: HeldInput } // held Move/Aim/Trigger; snapshot + hash (§5.12)
```

Definitions (`WeaponId`, `AmmoId`, `UnitDefId`) are `u32` handles into the immutable Content DB — mod data is never copied per entity.

### 5.4 Spatial structures

- **Uniform spatial hash** (e.g., 16 m cells) for units — rebuilt each tick, O(n).
- **Building/structure BVH** (refit or rebuilt incrementally in the World stage when pieces are placed or destroyed; sim state); **dynamic BVH** (refit) for vehicles' armor hulls.
- **Two-tier heightfield** terrain: 1 m base + sparse 0.25 m detail tiles where deformed (§5.10.1), chunked 64 × 64 m; edits are chunk-local and mark nav/cover/visibility dirty.
- **Visibility grid** (2 m cells, per-team bitsets) for coarse fog of war, painted per squad and amortized. Perception candidates come from a coarse 64–128 m per-squad grid rebuilt every 8 ticks. Precise LOS runs only on candidates.

### 5.5 Pathfinding & movement

- **Hierarchical**: HPA* over 32 m clusters gives an immediate coarse path the next tick; fine refinement is budgeted in node expansions per tick (§2.2), served per player round-robin.
- **Flow fields** for large group moves, split across ticks or limited to the corridor; individual A* only inside the expansion budget.
- **Separate nav layers** for infantry, wheeled, tracked (different slope/obstacle rules); updated incrementally on every terrain or structure edit (§5.10.5).
- **Local avoidance**: simplified ORCA in fixed-point, neighbor count capped (e.g., 8). A directly controlled unit is not an ORCA agent: others avoid it, and it doesn't avoid them (§5.12.3).
- Vehicles use kinematic models (tracked: differential steering; wheeled: bicycle model) with terrain sampling — **no general rigid-body physics in the sim**. Ragdolls and debris physics live in the presenter only.

### 5.6 Perception

- Each unit has a view cone, range and per-target spotting accumulator (affected by stance, movement, cover, camouflage, optics, smoke, light).
- LOS raycasts against heightfield + building-occluder BVH, using squad-level ray bundles with per-member refinement when visibility changes, cached terrain-blocked pairs (invalid once either unit moves more than 2 m or the corridor is edited), and a max-mip pretest. Rays come from the tier's global per-tick ray budget (§2.2). The cadence is the same in every tier: each pair is refreshed at least every 1.0 s, and priority pairs at least every 0.2 s. Denser fronts than the budget assumes slow non-priority pairs to at most 2.0 s, identically on every client (§2.2). One sample credits at most 0.5 s of exposure.
- Sound events (gunfire, engines) reveal approximate positions to the *knowledge map*, not exact positions.
- **Direct control (§5.12.3):** the controlled unit gets the focused-observation bonus inside its sim view cone, and its in-cone pairs (capped) are priority pairs.

### 5.7 Ballistics & armor (the heart of the genre)

**Projectile integration** — semi-implicit Euler at fixed 1/60 s substeps (2 per 30 Hz tick) with gravity + drag (`drag_coeff` from ammo def). Each substep tests the swept segment `pos → pos + vel·dt` (dt = 1/60 s).

**Hit testing** — two levels:
1. Broadphase: segment vs spatial hash / BVH.
2. Narrowphase: segment vs **armor mesh** (low-poly, authored separately from the visual mesh). Each armor face carries a `zone` tag (hull_front, turret_side, engine_deck…); `thickness_mm` and `material` come from the unit def's `[armor]` zone entries (§7.4), so patches can change them. Armor meshes use a per-hull BVH; per-tick triangle tests are capped.

**Penetration model (v1, data-driven):**

```
effective_thickness = thickness_mm / cos(impact_angle) * material.vs[class_of(ammo.mechanism)]   # matrix, §2.4
penetration_at_range = pen_table(ammo, distance)          # interpolated curve from ammo def
if impact_angle > ammo.ricochet_angle → ricochet (new projectile, reduced energy)
elif penetration_at_range >= effective_thickness → penetrate:
    spawn spall cone (N fragments, deterministic RNG) → raycast vs internal components & crew
else → non-penetration (shock damage to nearby components, crew stun)
```

The ammo def's `penetration` table (read by `pen_table`) is normalized to 0° at load using the def's `pen_reference_angle_deg` (penetration / cos(reference angle)).

HE uses blast radius + fragmentation vs. soft targets and thin armor. HE fragmentation uses 32 weighted fragment rays per shell in every tier. All curves live in data so modders can rebalance without code.

**Projectile LOD:** small-arms bullets beyond 400 m from any relevant target collapse into **hitscan with delayed resolution** (time-of-flight queued), saving per-tick integration. Tank/artillery rounds always fully simulated. Small-arms rounds beyond the player's projectile-pool quota (pool ÷ players, applied after the stable (entity id, seq) sort) merge into deterministic burst bundles: one projectile carrying N rounds and a dispersion cone, rolled per target on impact. Rounds from a directly controlled unit are never bundled and are served first from the owner's quota (§5.12.3).

### 5.8 Infantry model

- Individual soldiers grouped into squads; squad leader holds orders, members hold slots in a formation.
- Inventory is a grid/slot list of item stacks; anything droppable can be picked up (weapons, ammo, mines).
- Cover: nav cells tagged with cover direction & quality (pre-baked, re-baked on terrain/structure edits, §5.10.5).
- Suppression accumulates from near-misses and explosions; reduces accuracy/movement; high suppression → pinned / retreat (morale).

### 5.9 Vehicle model

- `Crew` seats map to roles; losing a role disables capabilities (no driver → can't move; no loader → slow reload).
- `Components` per vehicle: engine, transmission, left/right track, turret ring, main gun, MG, optics, fuel tank, ammo rack — each with HP and failure effects (fire, immobilize, detonation).
- Infantry can mount/dismount, repair components with kits, and capture abandoned vehicles.
- **Stations** (§5.12.1) group seats for direct control, per vehicle in data: e.g. `driver_gunner` (composite, default), `commander`, `hull_mg`. A station with an empty seat can be taken, but that function stays off until a crewman moves in (crew swap). One player per vehicle: a teammate can't take a second station of a controlled vehicle.

### 5.10 Deformable Terrain, Fortifications & Destruction

> **Core pillar.** Players dig trenches, foxholes and gun pits, raise berms and sandbag walls,
> roof over dugouts and put up whole buildings. Artillery tears all of it back down.
> Fortifications aren't decorations with a "+50 % cover" tag. **They work because of their
> real shape**: a soldier in a trench is safe because the ballistics and LOS raycasts hit
> the earth in front of him.

#### 5.10.1 Terrain representation: two-tier sparse heightfield

A single high-res heightfield for a 4 × 4 km (Regiment-max) map is too big (0.25 m cells ≈ 256 M cells). So:

| Tier | Resolution | Coverage | Storage |
|------|-----------|----------|---------|
| **Base** | 1 m | whole map, chunked 64 × 64 m | `u16` height in cm + `u8` material ≈ 48 MB for 4 × 4 km (≈ 27 MB at the 3 × 3 km Battalion max) |
| **Detail tiles** | 0.25 m | allocated **only** for chunks that have been dug, cratered or built on | 256 × 256 cells × 4 B (u16 height + u8 material + u8 flags) ≈ 256 KB per tile in the sim (the GPU copy is R16 height only, 128 KB; §4.1 rule 6); pooled, with a hard memory cap |

- A chunk gets upgraded to a detail tile the first time it's edited (the tile is initialized by bilinear sampling of the base).
- All sim queries (`height_at`, `raycast`, `slope_at`) go through one `TerrainQuery` API that checks the detail tile first and falls back to the base.
- Heights are **integers (cm)**, and edits are integer stamps, so deformation is deterministic by construction.
- A per-cell **material layer** drives gameplay: `topsoil`, `clay`, `sand`, `mud`, `rock`, `frozen`, `road`, `concrete`. Each material has a dig speed, crater depth multiplier, max wall steepness (sand slumps, clay holds) and ballistic absorption (from data).
- A per-cell **fortification flags** layer (`u8`) marks things like *revetted* (walls won't slump), *duckboarded* (no mud penalty) and *covered* (has an overhead structure above).

**Why not voxels?** A heightfield can't do overhangs or tunnels. Instead, overhead cover (dugouts, bunkers, covered trenches) comes from **structures placed over a dug depression** (§5.10.3). That covers about 95 % of WW2 field fortification at a fraction of the cost. True tunnels/caves are not in v1; free-form tunnels (sparse voxel islands embedded in the heightfield) remain a possible v2 feature (§5.10.9, §17 Q8).

#### 5.10.2 Deformation operations

All terrain edits are sim **events** that apply integer brushes to the heightfield in the World stage:

| Op | Source | Behaviour |
|----|--------|-----------|
| `Dig { profile, path }` | engineers / infantry with shovels | Lowers terrain along a path using a **cross-section profile** (depth, floor width, wall slope, fire step). Progresses incrementally with work. |
| `Spoil { volume, side }` | automatic, paired with `Dig` | **Soil is conserved**: dug volume is deposited as a parapet/berm on the chosen side (default: toward the enemy), so a trench automatically gains a raised parapet. |
| `Fill { brush }` | engineers, bulldozers | Raises terrain (filling trenches, building berms, ramps). |
| `Crater { energy, material }` | HE impacts, mines, demolition | Deterministic crater stamp sized from explosive energy × material factor; throws up a rim. |
| `Flatten { area, height }` | building foundations, gun emplacements | Levels an area; excess/missing soil is accounted for. |
| `Slump { area }` | automatic after large edits | One relaxation pass: walls steeper than the material's limit erode (unless *revetted*). Bounded iterations, deterministic order. |

Large edits (artillery barrages) are **queued and applied with a per-tick terrain-op cell budget (§2.2)**, so a 200-shell barrage spreads its terrain work over a few ticks instead of spiking one. First-edit detail-tile initialisation is capped at 4 per tick.

#### 5.10.3 Fortifications as data: profiles + pieces + blueprints

Construction combines three things, all defined by mods:

1. **Dig profiles**: cross-sections swept along a player-drawn path: foxhole, slit trench, standard trench with fire step, communication trench, anti-tank ditch, vehicle hull-down pit, gun pit, mortar pit.
2. **Construction pieces**: placed objects with collision, armor faces and a chunk graph (same format as map buildings): sandbag segments, log/plank revetments, duckboards, timber dugout roofs, concrete pillbox shells, barbed wire, hedgehogs, dragon's teeth, camouflage nets, doors, walls, floors, roofs.
3. **Blueprints**: composite recipes that combine digs and pieces: "MG nest" = gun pit + sandbag ring + camo net; "Dugout" = 2 m deep pit + timber roof + 0.5 m earth `Fill` on top; "Field HQ" = flattened foundation + modular building pieces.

```toml
# defs/fortifications/trench_standard.toml
id          = "ww2:trench_standard"
kind        = "dig_profile"
name        = "@loc:fort.trench_standard"
# cross-section points (offset_m, depth_m) from enemy side to rear
section     = [[-0.9, 0.0], [-0.6, -1.0], [-0.6, -1.3], [0.6, -1.3], [0.7, 0.0]]
fire_step   = { offset = -0.6, depth = -1.0, width = 0.4 }
spoil_side  = "front"           # parapet toward enemy
work_per_m  = 60                # work points per metre (scaled by material dig speed)
tools       = ["shovel"]
required_capability = "dig_full"
decorators  = ["ww2:revetment_planks", "ww2:duckboards"]   # presenter-only meshes (§5.10.7); the revetted/duckboarded flags (§5.10.1) are set by placing revetment/duckboard construction pieces
```

```toml
# defs/fortifications/dugout_small.toml
id    = "ww2:dugout_small"
kind  = "blueprint"
steps = [
  { dig    = "ww2:pit_2x3_deep" },
  { place  = "ww2:timber_roof_3m", at = "pit_top" },
  { fill   = { depth = 0.5 },       at = "roof_top" },     # earth on the roof = overhead protection
]
materials = { timber = 6, sandbags = 0 }
required_capability = "build_structure"
capacity  = 6                                             # soldiers sheltered
```

**Construction flow:**
1. Player (or AI) issues `PlanFortification { fortification, path/transform }` (any dig_profile, piece or blueprint id). A *ghost* appears and the plan is validated: slope, material (can't dig into `rock` or `concrete` without demolition), overlap, faction territory rules from mode data.
2. Engineers (or any squad with shovels, more slowly and within its capability tier, §5.10.9) are assigned. Each worker contributes **work points per tick** (from unit def × tool × material × fatigue).
3. Progress is applied **incrementally to the real terrain**: a half-dug trench is genuinely half as deep and gives partial cover. Interrupted work can be resumed.
4. Pieces that need materials (sandbags, timber, concrete, wire) draw from carried inventory or a nearby supply truck/depot (ties into logistics, §2.1).
5. Completion fires `FortificationCompleted`, Luau hook `on_fortification_completed`.

#### 5.10.4 Why fortifications are functional (no fake cover values)

| Effect | Mechanism |
|--------|-----------|
| **Protection from direct fire** | Bullet/shell segments raycast against the heightfield + pieces' armor faces. A soldier on the fire step exposes only head and shoulders, so only those hitboxes are hittable. |
| **Concealment** | Spotting LOS tests (§5.6) sample several points on the target's body; points below the parapet are hidden. Camo nets add a concealment modifier, defined in data. |
| **Blast & fragments** | HE fragmentation is resolved as a deterministic fan of 32 weighted fragment rays per shell (§5.7) that hit terrain. A shell landing outside a trench throws fragments into the parapet; one landing *in* the trench is lethal. That's why trenches zig-zag, and here they genuinely need to. |
| **Overhead cover** | Airbursts and plunging mortar/artillery fire are checked against structures with the *covered* flag. Roof pieces have thickness/HP and get destroyed by heavy shells. |
| **Hull-down vehicles** | A vehicle in a dug pit has its lower hull below grade; hits on the hull strike earth first. |
| **Obstacles** | Anti-tank ditches deeper/wider than a vehicle's `trench_crossing_m` (mobility def) block vehicles in navigation; wire slows infantry and must be cut. |
| **Suppression** | Near-miss suppression is reduced when the unit's head point is below terrain/cover relative to the shot's path. This is computed, not tagged. |

**Stance in trenches:** a unit's stance resolves against local terrain. "Crouch" in a 1.3 m trench means standing upright below the parapet, and "fire position" snaps to the nearest fire step cell. Units moving through trenches get an auto-posture so the player doesn't have to micromanage.

#### 5.10.5 Navigation & cover-map updates

- Each terrain or structure edit marks a **dirty rect**. A background job re-bakes the affected nav cells (every nav layer defined in data, §2.4), cover/fire-position tags and the coarse HPA* cluster edges.
- Re-baking runs in ~0.2 ms work units (one entrance Dijkstra per nav layer). A fixed number runs per tick (Battalion 4, Regiment 8), with coalesced dirty clusters and a deterministic priority order: active path corridors → digging sites → barrage remainder. Each edit applies an immediate pessimistic invalidation (block edited cells, raise cluster-edge costs), so stale nav is suboptimal, never wrong. When a re-bake commits, only paths crossing changed clusters re-path. The queue is sim state (snapshot + hash). Nav resolution inside detail tiles is decided at M2.5.
- Trench floors form connected **trench networks** that the AI recognizes as covered movement routes (a cost discount in pathfinding when under threat).

#### 5.10.6 Buildings

- Map-placed and player-built buildings share **one format**: modular pieces (foundation, wall, wall-with-window, doorway, floor, roof, stairs) with a **support graph** and per-piece armor/HP.
- Placing a foundation auto-applies `Flatten`. Pieces snap to a 0.5 m grid (per-blueprint override).
- Damage removes pieces, and a structural pass (still connected to ground through the support graph) collapses unsupported pieces, spawning rubble that becomes terrain `Fill` plus cover objects.
- Garrisoning uses authored "slots" (window, door, roof firing positions) generated from the piece layout, so player-built buildings are garrisonable automatically.

#### 5.10.7 Rendering deformed terrain (presenter)

- Dirty detail tiles upload as R16 height-texture rectangles to a texture array; normals are recomputed on the GPU. There is no CPU re-meshing. Uploads are capped per frame (Deck: 1–2 MB).
- Steep trench walls use **triplanar texturing** with per-material wall textures. **Decorators** (data-driven) auto-place cosmetic meshes along dug edges: planks, wattle, sandbag tops, duckboards, puddles. Decorators are presenter-only and never affect the sim.
- Craters get decals + blended "scorched/churned" material, so a battlefield visibly accumulates scars over a match. Craters write scorch into a presenter-side splat/scorch mask (not the sim material layer). Decal caps: Deck 256, desktop 1,024.

#### 5.10.8 Performance & networking notes

- **Budgets:** terrain-op cells (§5.10.2), detail-tile inits (4 per tick in every tier) and nav re-bake units (Battalion 4, Regiment 8; §2.2) are fixed work counts, inside the World stage budget. Overflow is queued, never dropped.
- **Memory cap:** detail-tile pool of 512 tiles (≈ 128 MB, sim state; §2.2), lowerable per map in data (512 is the hard maximum). If it's exhausted, the oldest *flat-again* tiles are demoted back to base. Edits are never lost: they're stored as compressed deltas.
- **Multiplayer:** edits are deterministic results of commands, so **no terrain data crosses the network** in normal play. Join-in-progress sends compressed per-chunk deltas vs. the base map.
- **Saves:** terrain is saved as zstd-compressed deltas per edited chunk (§11); replays store only commands and re-derive terrain.

#### 5.10.9 Scope decisions **[DECIDED 2026-10-31]**

**Underground: dugouts only.** Overhead cover is always *structure pieces over a dug pit* (§5.10.1, §5.10.3). No tunnels and no voxel carving in v1; free-form tunnels remain a possible v2 feature.

**Pacing: compressed realism.** Durations are targets for the base content at default settings. Every unit's work rate is multiplied by one global data value, `construction_time_scale` in `defs/rules/construction.toml`, so modes and mods can retune pacing in one place.

| Job (base content, clay soil, not under fire) | Crew | Target time |
|---|---|---|
| Shell scrape (prone cover) | 1 rifleman | ~10 s |
| Foxhole (1–2 men, standing depth) | 1 rifleman | ~30 s |
| Standard trench with fire step, 20 m | 8-man engineer squad | ~3–4 min |
| Sandbagged MG nest | 4 engineers | ~1.5 min |
| Small dugout (6 men, earth-covered roof) | 6 engineers | ~6 min |
| Vehicle hull-down pit | 6 engineers | ~3 min |
| Barbed wire, 10 m | 2 engineers | ~30 s |
| Anti-tank ditch, 10 m | 8 engineers | ~6 min |

These are roughly 50–100× faster than real WW2 digging. Modifiers (all in data): soil material (sand fast, frozen/rock slow or impossible), tool (entrenching tool vs. full shovel vs. engineering vehicle), fatigue, and **under fire** (work rate × 0.3 while suppressed, stops when pinned).

**Placement: anywhere a unit can stand, with limits set by mode.** Base rules: the plan has to pass slope/material/overlap checks (§5.10.3), and builders have to physically reach the site. Game-mode defs can add `build_zones`, `no_build_zones`, `own_sector_only` or `max_forward_distance_m`. There's no hidden "can't build near enemies" rule: building under fire is just slow and deadly.

**Builders: tiered.**

| Tier | Who | Can build |
|---|---|---|
| Basic | any infantry carrying an entrenching tool/shovel | shell scrapes, foxholes, shallow slit trenches, small sandbag piles (from carried bags) |
| Engineer | engineer squads (and mod-defined specialist units) | everything: full trenches, gun/vehicle pits, dugouts, bunkers, wire, mines, obstacles, AT ditches, modular buildings, demolition. 1.5× work rate |
| Vehicle | engineering vehicles (e.g. dozer, crane truck) | heavy digging/filling and flattening at high rates; no fine work |

Tiers are data: each fortification def lists `required_capability` (`dig_basic`, `dig_full`, `build_structure`, `place_wire`, `place_mines`, `demolish`), and each unit def lists the capabilities it has. Mods can add new tiers without code changes.

### 5.11 Determinism toolkit

- Fixed-point math everywhere in sim; trig via lookup tables shipped as data.
- One **seeded RNG per stream** (`rng(world_seed, tick, tick_salt[tick], entity_id, purpose)`) — order-independent, so parallel systems stay deterministic.
- **Unpredictable future rolls.** Each closed turn bundle carries a 64-bit `tick_salt` chosen by the relay at close time. A client learns `tick_salt[N+D]` only after its commands for N+D are fixed, so it can't time a shot to a good dispersion or penetration roll (§5.12). Salts are part of the command stream, so replays, saves and join-in-progress record them (§11). The single-player local relay draws them from the OS RNG; headless `simulate` derives them from `--seed`. A host-run relay knows its own salts, so ranked play uses official relays only (§10).
- No `HashMap` iteration in sim (use `BTreeMap`/`IndexMap` or sorted vectors).
- State hash = incremental xxhash3 per chunk/archetype, combined and published every K ticks (§5.1). It covers all queues, cursors, spotting accumulators, the entity-id allocator, free lists and non-pure caches. CI saves at a random tick (including mid-barrage), loads, continues and compares per-tick hashes. Mismatches dump both states for diffing.
- Luau `math` is replaced by a vendored correctly-rounded/fixed-point library; `os.clock` and time sources are banned. wasmtime runs with NaN canonicalisation and no relaxed-SIMD. Proton (Wine ucrtbase) is not trusted for determinism.
- CI test: run the same replay on SteamOS native, SteamOS + Proton, Windows and macOS ARM, with 1/2/3/8 workers, and compare hashes.

### 5.12 Direct Control

> **Core feature.** Any unit you own can be taken over and played by hand: a rifleman, an MG gunner, a tank, an AT gun or a mortar.
> WASD to move, mouse to aim, click to fire, while the rest of the battle carries on under its orders.
> Direct control uses **exactly the same simulation** as AI control: same ballistics, dispersion, crew rules and armor.
> The only differences are who supplies the inputs, the D-tick input delay (§5.12.4), and the control-specific rules in §5.12.3 (focused observation, local avoidance, projectile quota, `fire_rewind`) and §5.12.6 (reflexes).

#### 5.12.1 Taking and releasing control

- **Take control:** select one unit and press `Ctrl+F`, or `Alt`+double-click it. For vehicles, pick a crew **station** (§5.9; driver+gunner composite by default). `F1`–`F6` jump to another station of the same vehicle, `[`/`]` to the previous/next member of the squad or crew, without leaving direct control.
- **Crew swap:** `Ctrl+F1`–`F6` moves the selected crewman into that seat in `seat_swap_time_s` (vehicle data, 2–6 s; both seats count as empty meanwhile), e.g. the loader replacing a dead gunner.
- **Release:** the same key (modifiers match exactly, so `F` never triggers `Ctrl+F`); `Esc` releases only with the client setting `esc_releases_control`. The unit executes any RTS orders queued during control, otherwise returns to AI with "hold position, engage at will"; infantry first take the best cover cell within 8 m (§5.8).
- **Forced release** (before any other command that tick, held inputs zeroed): death, ownership change (`SetController` from disconnect or governor, §2.2; co-op share revoked), `max_direct_control_time_s`, or a morale break (§5.12.6). On death, mode rule `on_control_death`: `return_to_rts` (PvP default) or `next_in_squad` (solo/co-op default, after 1 s).
- **One unit per player, one player per unit.** Taking a second unit releases the first in the same tick. Only your own units, or ones a co-op teammate shared (`ShareUnit`); same-tick ties go to the first in (player_id, seq) order. RTS orders for a controlled unit are queued until release (see **Release** above).
- **Mode rules (data):** `allow_direct_control` (default true), `direct_control_classes` (e.g. infantry only), `max_direct_control_time_s` (default none), `direct_control_switch_cooldown_s` (default 1.0), `on_control_death`.

#### 5.12.2 Controls (defaults, rebindable)

| Unit type | Movement | Aim & fire | Other |
|---|---|---|---|
| **Infantry** | WASD (camera- or unit-relative, client setting), `Shift` sprint, `C` crouch, `Z` prone, `Q`/`E` lean, `Space` vault/climb up to `vault_height_m` | Mouse aim; LMB fire (hold for automatic); RMB aim down sights; `R` reload | `1`–`5` slots; `G` throw (hold for range), `T` cycle throwable; `F` interact (pick up, enter, mount MG, cut wire, dig); `V` melee; `H` bandage |
| **Inventory** | — | — | `I` opens it (game keeps running). Drag items to/from any container within 2 m (body, crate, vehicle stowage, truck); `Backspace` drops the weapon. Transfers are timed and interrupted by moving or being pinned |
| **Binoculars** | as infantry | `B` raise (optic def) | `LMB` marks a target or point for teammates; `Ctrl+LMB` calls the selected mortar, gun or off-map support onto it if a radio link is in range. Both are ordinary RTS Commands |
| **Vehicle** (composite station) | WASD drives (driver crew executes); `Shift+W`/`S` step a held cruise throttle | Mouse aims the gun (real traverse rate). LMB main gun, RMB coaxial/hull MG | `1`–`4` next ammo type (double-tap: unload and load it now); wheel sets range in manual-range optics, `Ctrl`+wheel zoom; `X` smoke; `B` engine (noise) |
| **Vehicle crew** | — | — | `U` button/unbutton (better spotting, exposed head); `J` repair worst repairable component; `K` extinguish; `Ctrl+B` bail out; `Y` dismount passengers |
| **Static guns** (AT gun, howitzer in direct fire) | — (crew can push/turn the gun slowly with WASD) | As vehicle gun, in optic or over the shield | `1`–`4` ammo; `F` at a tow vehicle hitches/unhitches (`hitch_time_s`); a hitched gun can't fire and is towed when you drive |
| **Mortar / indirect** | — | Click target on the map or set elevation/bearing manually; spotting rounds | `1`–`3` ammo; ranging aid from data |

The player fires; the **crew still does the work**. Reload speed depends on the loader, and the gun can't fire without a gunner. A dead or wounded crew member degrades exactly as under AI control (§5.9).

- **Limited-traverse and turretless guns:** mount defs list traverse and elevation limits. If the aim is outside the arc and A/D are released, the driver turns the hull toward it (`hull_assist`, default on); while you steer, the gun stops at its limit and the reticle shows it. Static gun crews re-lay the trail the same way.
- **Driving:** drivetrain defs set `max_reverse_speed`, `can_neutral_steer`, `gear_change_time_s` and `brake_decel`; `S` while moving forward brakes first. Cruise throttle and `invert_steer_in_reverse` only shape input on the client.

#### 5.12.3 Simulation side (deterministic)

New Commands (P6), sent only when the input changes. **Any input that changes a sim result (accuracy, exposure, spotting, noise, ammo) is a Command;** view choice, camera and HUD are presenter-only and never do.

| Command | Payload (quantized) |
|---|---|
| `TakeControl` / `ReleaseControl` | unit id, station |
| `ShareUnit` | unit id, teammate, on/off (co-op only) |
| `MoveInput` | vehicles: throttle, steer (i8); infantry: forward, strafe (i8) in a client-chosen `frame_yaw` (u16, camera or unit); flags: sprint, stance, lean, `hull_assist`; held until changed |
| `AimInput` | aim **point** (unit-relative, 2 cm steps), or yaw/pitch (16-bit) in optics; sight (none / ADS / optic zoom); range (u16 m); highest seq per tick wins |
| `Trigger` | weapon slot, pressed/released, `target_tick`, `view_tick` (tick + u8 fraction the player saw); held until released |
| `UseAction` | action id + argument (reload, slot, ammo, throw, interact, vault, melee, bandage, item transfer, smoke, engine, hatch, repair, extinguish, bail out, dismount, seat swap, hitch) |

- **Aim point and ranging.** Outside optics the presenter raycasts the cursor against terrain, structures and *spotted* units and sends the hit point; the sim computes yaw/pitch from the real muzzle, so there's no camera parallax, and solves elevation from the ammo's trajectory. Optics with `manual_range` use the range setting (mode rule `force_manual_range_in_optics` for realism lobbies).
- **The sim owns all limits.** Turrets and guns turn at their traverse/elevation rates, so a flick can't beat a slow Tiger turret. Infantry aim in a fast **weapon-aim cone** (±60° from the body, `aim_turn_rate_deg_s`, default 540) and the body follows; fast aim changes add sway and reset aim-settle, so snap shots are less accurate.
- **Shot direction.** A shot leaves along the sim's weapon aim, never a client-sent angle; shots between ticks lerp (fixed-point) between the last two ticks' aim. A held trigger fires at the cyclic rate; semi-auto and bolt weapons fire once per press. A modified client can't fire faster, wider, or beyond the traverse actually reached.
- **Same weapon physics as AI; the player supplies the aim.** Dispersion, sway, recoil, aim-settle, stance/movement, crew-skill and suppression modifiers apply identically. Only the AI's target-estimation error (`ai_aim_error`) is replaced by the player's aim. `direct_control_accuracy_mult` (mode rule, default 1.0) scales dispersion only.
- **Focused observation.** Spotting gets a bonus inside the unit's **sim** view cone (never the camera), larger with an optic or binoculars (zoom from the def, via `AimInput`), ramping in over `focus_settle_s` (0.5 s) of steady aim. Only in-cone pairs become priority pairs (§2.2), at most `direct_control_priority_candidates` (64). A `TakeControl` cooldown and instant loss of priority on release make cycling units pointless.
- **Input validation (in the sim, so it holds on every client).** `TakeControl` bumps a per-player `control_gen` that every direct-control command carries; commands with a stale `control_gen`, unit or station are dropped, so held or late input never carries over to another unit, and each session starts neutral. Per player per tick at most 1 `MoveInput`, 1 `AimInput`, 4 `Trigger`, 2 `UseAction` apply. `UseAction` checks reach, throw strength and capabilities (§5.10.9). The relay caps too, but the sim never relies on it.
- **Buildings and openings.** `F` on a doorway enters; inside, the soldier walks floor nav cells and snaps into firing slots (§5.10.6), where `Q`/`E` move to the next slot. Windows, doors, hatches and `open_top` compartments are **openings** in the ballistic world, so a grenade through a window or into a halftrack is ordinary projectile sim.
- **Fog of war (honest scope).** Lockstep clients hold the full world, so fog binds only unmodified clients. The bridge filters *before* anything leaves Rust: unspotted enemies never reach render buffers, skeletons, optics, minimap, cursor raycasts, aim assist or the Luau UI API, and their sounds play at the knowledge-map position (§5.6). Cheat detection: §10.
- **Lead and latency.** Without compensation, moving targets need (D + 1)/30 s extra lead (≈ 1 m on a runner at D = 5), and you can be hit that long after reaching cover. Fine for tank and AT guns (shell flight dominates); for small arms the M1 gate (§15) compares it with **`fire_rewind`** (rules flag, small arms only): the sim keeps a hitbox history ring of every infantry unit (pose, stance, lean; snapshot + hash) for `max_rewind_ticks` = min(D + 2, 6). `view_tick` is clamped to [`target_tick` − `max_rewind_ticks`, `target_tick`], and for its first `N_fire − view_tick` ticks (N_fire = the tick in which the shot is fired in the sim) the bullet's normal swept substeps test infantry hitboxes as they were then (terrain, structures, vehicles as now). A rewound shot uses one slot of the shooter's projectile quota (§5.7) like any other round; its history hitbox tests count toward the per-tick capped narrowphase tests (§5.7). AI shots never rewind; re-stamped commands get no extra rewind.
- **Local avoidance.** A directly controlled unit is not an ORCA agent (§5.5): others avoid it as an obstacle; it collides only with static geometry and vehicles.
- **Projectile quota.** Rounds from a directly controlled unit are never merged into burst bundles and are allocated first from the owner's projectile-pool quota (§5.7).

#### 5.12.4 Feel: hiding lockstep delay (presentation only)

A command takes effect D ticks later, plus up to a tick of input sampling (§2.2) and a tick of snapshot interpolation (§5.1) before the result is drawn: ≈ 70–100 ms solo (D = 1, local relay), ≈ 150–220 ms online (D = 3–5). The presenter hides this for the controlled unit only:

- **Prediction.** Each snapshot carries a **prediction slice** per controlled unit: its full kinematic, aim and weapon-cycle state (pose, velocity, stance timers, stamina, settle, speed modifiers, traverse/elevation, reload/ammo/crew; vehicles: tracks, engine, gear), `last_applied_seq` with its tick, and the fixed-point height/material/collision tiles within 64 m when they change. Each frame the predictor runs the sim crate's own `step_controlled(slice, input, world_view)` (Intent → Movement → Weapons-aim, §5.2) for every input with seq > `last_applied_seq`, at the tick the relay stamped it. Same code, same data: prediction is exact unless the world changes in a way the snapshot didn't show. The presenter never reads live sim state.
- **Predicted aim.** Optic view, ADS sight picture and reticle hang on the *predicted* gun, so the reticle shows where the shot will go; other cameras follow the mouse directly. A **gun-laid marker** shows where the gun points now and turns solid once it's laid within dispersion of the target.
- **Instant locally:** camera, crosshair, weapon pose, turret target indicator. Muzzle flash, recoil, sound and a predicted tracer play at once only if the predicted weapon cycle says it will fire; `Fired` events carry the command seq so effects aren't played twice, and a shot the sim doesn't fire fades out with the reason on the HUD.
- **Expected mispredictions:** suppression, pinning and wounds, blocked stance changes, re-stamped commands, terrain edits and crushing, death. Corrections under 0.5 m blend over 100 ms; larger ones snap. Rate and size are logged for the M1 feel test.
- **Only the sim decides outcomes.** Hit feedback comes from sim events (`Ricochet`, `NonPenetration`, `Penetration`, `ComponentHit(zone)`, `TargetDisabled`), shown only for impacts the controlled unit observed, so players learn to aim at tracks, turret ring or ammo rack. No penetration-chance cursor in PvP (`show_pen_hint`).
- **Targets:** solo runs at D = 1. Online co-op and PvP use the §10 rule, capped by `max_input_delay_ticks`, and the lobby shows the resulting delay. Controlled units react D ticks later than AI units; this is inherent to lockstep and said in the mode description. PvP feel is a **gate** in the M1 feel test (§15).

#### 5.12.5 Cameras & optics

All views send the same Commands, so the view is presenter-only: `Tab` cycles top-down → third person/chase → optic or first person at any time.

- **Top-down (default):** the RTS camera keeps panning, zooming and rotating; WASD moves the unit and the cursor's ground or target point is the aim point (`Alt`+`RMB`-drag sets aim height for arcing weapons).
- **Infantry:** over-the-shoulder third person; aim-down-sights zooms toward the weapon's sights.
- **Vehicles:** chase camera (free-look on `Alt`) and **gunner optic view**. Each optic is a data def: reticle, zoom levels, FOV, range scale/stadia, `manual_range`.
- **Optic rendering:** the optic replaces the main camera (no second viewport). LOD, impostor, culling and shadow distances use distance ÷ zoom, and the §2.2 draw/triangle/skeleton caps still apply. The lens effect is one full-screen pass.
- **RTS overlay one key away:** `` ` `` or `M` shows a minimap/command overlay without leaving direct control.

#### 5.12.6 The rest of your army while you're in direct control

- Other units keep executing their orders and AI.
- If you control a **squad leader**, the squad stays within `follow_leash_m` (default 15 m), taking cover cells near you (§5.8) rather than formation slots. Quick orders: `MMB` fire at my target, `Alt+1` follow, `Alt+2` hold, `Alt+3` regroup. Controlling a **squad member** makes the rest of the squad hold or follow the leader's current order.
- In a vehicle, **AI crew** fill the stations you don't control. Driving only? The AI gunner engages at will. Gunning only? The AI driver follows the last move order.
- **Reflexes and morale:** reflexes (§9 layer 3) are off for the controlled unit/station only. Morale and suppression act through handling (sway, settle, reload, speed; §5.8); pinned means no sprint and no standing up. A morale *break* releases control to AI after a HUD warning (`morale_break_releases_control`, default true). Bail-out from a burning vehicle (§9) still happens.
- **Co-op:** ownership can be shared per unit (`ShareUnit`); a shared unit has one controller at a time (§5.12.1).

#### 5.12.7 Controller & Steam Deck

- Steam Input layout: left stick move, right stick aim, **gyro aim** enabled by default on Deck, triggers fire/aim-down-sights, radial menu for slots/ammo/stances.
- **Aim assist** is client-side smoothing and slowdown near *spotted* targets, never snapping. It shapes input before it becomes a Command, so determinism holds, but the sim can't see it: the PvP lobby setting binds only unmodified clients, and real fairness comes from sim turn-rate limits and dispersion. Defaults: on in solo/co-op; in PvP the lobby chooses (default on for gamepad/gyro, off for mouse). Strength is capped in `defs/controls/aim_assist.toml`, and the replay audit (§10) flags anything stronger.

#### 5.12.8 Modding & testing

- **Data:** control schemes (`defs/controls/*.toml`, client-only), optics (`defs/optics/*.toml`), per-weapon handling (sway, recoil, settle, aim cone), vehicle stations (§5.9) and mode rules (§5.12.1), all moddable without code.
- **Luau hooks:** `on_take_control(unit, player)`, `on_release_control(unit, player)`. Missions can forbid direct control per unit or zone, or offer it as a prompt that sends the player's own `TakeControl` ("take the gunner seat of this PaK 40").
- **Testing:** `frontline-cli simulate --inputs session.jsonl` replays recorded or scripted input, so agents can test "a player-driven Panther at 800 m against a T-34-85" headlessly and deterministically (§14).

---

## 6. Threading & Performance Strategy

```
Main thread:      OS events, input → commands; bridge reads newest snapshot in `_process`,
                  culling + MultiMesh buffer writes (§4.1); present
Sim thread:       owns World; fixed tick; uses job pool for parallel stages
Render thread(s): build draw lists from Godot's scene state, GPU submit
Network thread:   wall-clock input sampling once per tick (33.3 ms at 30 Hz), relay I/O (§2.2, §10)
Godot worker pool: capped (Deck: 2, §4.1)
Job pool:         physical cores − presenter reserve; the sim thread participates (Deck: 2 workers + sim;
                  6-core desktop: 3 + sim); fork/join regions merged per stage
IO thread:        asset/terrain streaming, save writing
```

**Snapshots**: the sim writes a compact *render-relevant* view (transforms, animation state, visual flags) into a triple buffer. No locks on the hot path; renderer always reads the newest complete snapshot. Each directly controlled unit also gets a prediction slice: full movement/aim/weapon state and changed local terrain tiles (§5.12.4).

### Performance techniques checklist

- [ ] Struct-of-arrays component storage; hot/cold split (`Transform` hot, `Inventory` cold).
- [ ] **Simulation LOD**: AI think rate scales with distance to the nearest enemy (e.g., 30 Hz → 5 Hz). Perception follows the fixed §2.2 cadence and ray budget. Sim LOD is a function of sim state only; *camera-based LOD only in presenter*.
- [ ] Time-sliced expensive work (pathfinding requests queue with per-tick budget).
- [ ] Object pools / free lists for projectiles and effects; zero allocations per tick in steady state (enforced with a counting allocator in tests).
- [ ] GPU instancing for soldiers (shared skeletons; animation via GPU skinning with baked animation textures for distant units).
- [ ] Mesh LODs + **impostors** beyond ~300 m (Deck ~200 m); HLOD for building clusters.
- [ ] Tiled/Morton heightfield + max-mip pyramid (M0 deliverable).
- [ ] Clustered forward lighting; cascaded shadows with distance-limited caster sets.
- [ ] Decal atlas for craters/scorch; projected blood/mud kept in a ring buffer.
- [ ] Terrain: CDLOD rendering from GPU height textures (§4.1), streamed heightfield chunks.
- [ ] Profiling: `tracy` integration from day 1; per-system timings shown in an in-game overlay and exported in CLI `--report`.

### Performance budget per sim tick

Per-stage budgets per tier and hardware class are in §2.2 (Deck Battalion 12.0 ms, desktop Regiment 10.0 ms mean; P95 ≤ 15, P99 ≤ 20).

CI runs `bench/battalion` and `bench/regiment` (late-game snapshots) plus `bench/spike` (barrage + building collapse + mass order in one tick), paced at the tick rate, and fails if any stage regresses by more than 10 % or a work-count budget is exceeded.

---

## 7. Modding Architecture

### 7.1 Mod package layout

```
mods/
  core/                         # era-agnostic rules: materials, soil, base behaviours, UI shell
    mod.toml
    defs/
      materials/ rules/ controls/ ai/ ...   # controls = default direct-control schemes (§5.12.2), client-only
  ww2/                          # the base game content is a mod (depends on core)
    mod.toml
    defs/
      units/       *.toml       # soldiers, vehicles, guns
      weapons/     *.toml
      ammo/        *.toml
      factions/    *.toml
      squads/      *.toml
      fortifications/ *.toml    # dig profiles, construction pieces, blueprints
      materials/   *.toml       # soil/material properties (dig speed, slump, absorption)
      optics/      *.toml       # gunner/commander optics, binoculars: zoom, FOV, reticle, range scale (§5.12.5)
      rules/       *.toml       # battle tiers, construction, game modes incl. direct-control rules
      art/         *.toml       # theater palettes, grade LUTs (presentation only, §12.1)
    maps/<map_id>/
      map.toml                  # metadata, spawns, zones, triggers
      terrain.heightmap.png     # 16-bit
      splat.png, objects.toml
    scripts/        *.luau      # missions, AI, game modes (sim)
    ui/             *.tscn, *.tres, *.luau   # UI scenes, themes, UI Luau (presenter only, §7.8)
    assets/
      models/ *.glb  (visual mesh + armor mesh + chunk graph in glTF extras)
      textures/ *.ktx2
      audio/ *.ogg
    locale/ en.ftl, de.ftl, ru.ftl # Fluent localization
    tests/          *.scenario.toml  # golden tests (§8.2)
  my_tiger_mod/
    mod.toml
    patches/ *.patch.toml       # edits to other mods' defs
    defs/ ...
```

Control schemes (including aim-assist caps) are client-only: they are excluded from the sim hash checked at join (§7.8, §10), and user rebinds live in Settings (§11). Optic defs are sim data (zoom feeds spotting, §5.12.3) and are hashed; their reticle and lens images are presenter assets.

### 7.2 Manifest

```toml
# mods/my_tiger_mod/mod.toml
id          = "my_tiger_mod"
name        = "Late-War Heavy Tanks"
version     = "1.2.0"
kind        = "content"        # "content" | "cosmetic" (presentation allowlist only, §7.8)
api_version = "1"              # game data/scripting API this mod targets
godot       = "4.7"            # engine version assets were authored with (for migrations)
authors     = ["someone"]
depends     = [{ id = "ww2", version = ">=0.4" }]
load_after  = ["realism_pack"]
deterministic = true           # required for multiplayer; checked by validator
```

### 7.3 Identity & layering

- Every definition has a **namespaced, stable string ID**: `ww2:pz4_h`, `my_tiger_mod:tiger_e`.
- IDs are interned to `u32` handles at load; saves/replays store the string table + the mod set record (§11).
- Load order resolved via dependency graph (topological sort, `load_after` hints, cycle = error).
- **Three ways to change content**, from safest to strongest:
  1. **Add** new defs (new IDs).
  2. **Patch** existing defs with field-level operations:
     ```toml
     # patches/realism.patch.toml
     [[patch]]
     target = "ww2:pz4_h"
     set    = { "armor.hull_side.thickness_mm" = 35 }
     append = { "weapons[0].ammo" = [{ id = "ww2:75mm_apcr", count = 5 }] }
     ```
     Patch paths address array elements by index (`weapons[0]`).
  3. **Override** (replace a def entirely) — allowed but warned, since it breaks compatibility with other patches.
- Conflict report: the loader prints which mod last touched each field (`frontline-cli explain ww2:pz4_h`, §8.1).

### 7.4 Example definitions

```toml
# defs/units/pz4_h.toml
id       = "ww2:pz4_h"
kind     = "vehicle"
name     = "@loc:unit.pz4_h"          # localization key
faction  = "ww2:germany"
cost     = 420
model    = "assets/models/pz4_h.glb"
available_from = "1943-04"            # date filter (§2.3)
available_to   = "1945-05"
sources  = ["..."]                    # required citation (§2.3)

[mobility]
type        = "tracked"
max_speed   = 38.0     # km/h
hp_per_ton  = 12.0
fuel_l      = 470

[crew]
seats = ["driver", "gunner", "commander", "loader", "radio_mg"]

[[stations]]                           # direct control (§5.9, §5.12.1)
id    = "driver_gunner"                # default station
seats = ["driver", "gunner"]
optic = "ww2:tzf5f"
[[stations]]
id    = "commander"
seats = ["commander"]

[[weapons]]
def    = "ww2:kwk40_l48"
mount  = "turret_main"
ammo   = [{ id = "ww2:75mm_apcbc", count = 50 }, { id = "ww2:75mm_he", count = 37 }]

[armor]                                # zones map to faces tagged in the armor mesh
hull_front   = { thickness_mm = 80, material = "rha" }
hull_side    = { thickness_mm = 30, material = "rha" }
turret_front = { thickness_mm = 50, material = "rha" }
```

```toml
# defs/ammo/75mm_apcbc.toml
id             = "ww2:75mm_apcbc"
mechanism      = "aphe"     # APCBC with HE filler (§2.4)
caliber_mm     = 75
muzzle_vel     = 750        # m/s (KwK 40 L/48)
mass_kg        = 6.8
drag_coeff     = 0.28
ricochet_angle = 70         # degrees from normal
penetration    = [[100, 106], [500, 96], [1000, 85], [1500, 74], [2000, 64]] # [range_m, mm]
pen_reference_angle_deg = 30   # published table is vs 30° plate
he_filler_g    = 18
sources        = ["..."]    # required citation (§2.3)
```

### 7.5 Scripting (Luau)

Used for missions, triggers, custom game modes, UI widgets and lightweight AI behaviours. UI Luau runs in a separate presenter-side VM with no Commands (§7.8); the rules below cover sim Luau.

**Rules that keep scripts safe and deterministic:**
- Scripts **read** through a query API and **act only by issuing Commands** (same as a player). They can't mutate components directly.
- No access to wall-clock time, filesystem, network, or `math.random` — they get `sim.rng(stream)` and `sim.tick`.
- Per-tick instruction budget per mod (e.g., 200k VM instructions); overruns are logged and the handler deferred, never crash the game.
- Luau runs serially with per-tick instruction budgets and an incremental GC step per tick, counted in the Intent stage budget (§2.2).
- Event-driven hooks:

```lua
-- scripts/mission_bridge.luau
local M = {}

function M.on_start(ctx)
  ctx:spawn_squad("ww2:ger_rifle_squad", { zone = "bridge_west", faction = "ww2:germany" })
  ctx:set_objective("hold_bridge", { zone = "bridge", duration_s = 600 })
end

function M.on_zone_entered(ctx, ev)  -- ev.unit, ev.zone
  if ev.zone == "bridge" and ctx:faction_of(ev.unit) ~= "ww2:germany" then
    ctx:command(ctx:ai_player("ww2:germany"), { type = "attack_zone", zone = "bridge" })
  end
end

return M
```

Script API typings (`.d.luau`) are generated from the Rust bindings so editors and AI agents get autocomplete and type checks.

### 7.6 WASM plugins (advanced)

For mods that need real compute — a new AI commander, a custom morale model — a WASM module can register a **system** into a designated stage with declared component reads/writes. The host only exposes deterministic imports. Requires `deterministic = true` and is sandboxed with a fuel (instruction) limit.

### 7.7 Hot reload

In dev mode, the loader watches `mods/`; changed defs re-validate and swap in the Content DB between ticks; Luau scripts reload with state handoff (`on_reload(old_state)`). Asset changes reload in the presenter. This is what makes iteration fast for humans and agents alike.

### 7.8 Mod distribution **[DECIDED 2026-10-31]**

**Channels at launch: Steam Workshop + mod.io + plain folders.**

| Channel | Role |
|---|---|
| **Plain folders** (`mods/`, user mods dir) | Always works: manual installs, development, AI agents, offline play. The other channels feed the same loader through the content-addressed cache (below). |
| **Steam Workshop** | Main channel for Steam/Steam Deck players: subscribe, auto-update, collections. Needs a logged-in Steam user. Steam installs items unpacked in its own folder (`steamapps/workshop/content/<appid>/<itemid>/`) and updates them in place. |
| **mod.io** | Store-independent channel: GOG/Epic/other stores now, consoles later (consoles need a paid mod.io tier and platform approval). Optional on Steam, where players sign in with their Steam app ticket. There is no official Godot or Rust SDK (official: C++, Unreal, Unity), so the `modio` adapter calls the REST API from Rust and the in-game mod browser is our own Godot UI. Players accept mod.io's Terms of Use once, before their first mod.io use. |

**One mod, one identity, everywhere.**
- The loader only sees folders. Each channel is a `ModSource` adapter (`folder`, `steam_workshop`, `modio`) feeding a **content-addressed cache** (`cache/<package_hash>/`). Several versions of a mod can coexist. mod.io zips are extracted into the cache. The Workshop adapter indexes Steam's folder in place (no copy) while it matches the hash; versions that saves need are copied into the cache, because Steam updates its folder in place.
- **Hashes** (all BLAKE3; the per-tick state hash, §5.11, is unrelated):
  - **Package hash:** one mod's identity, over every file in its folder. Paths relative, `/`-separated, NFC, case-sensitive, sorted bytewise; file bytes as-is; `mod.toml` without its `[publish]` table. Always computed after extraction (mod.io's MD5 covers only the zip). Cache, lobby sets, saves and takedowns use it.
  - **Sim hash:** one per resolved mod set, over the sim-relevant parts of every mod in load order (Lobby policy, below). Compared at join (§10).
  - **Presentation hash:** everything else. Shown in the lobby; never blocks a join on its own.
  - `id` + `version` from `mod.toml` are only the human-readable name.
- **One upload path for modders:** `frontline-cli publish --to workshop,modio` validates the mod, creates or looks up both channel items, writes their ids into `mod.toml`, computes the package hash, then uploads the identical folder to both channels:
  ```toml
  [publish]
  workshop_id = 3141592653
  modio_id    = 271828
  ai_assisted = true          # shown as an 'AI-assisted' tag (§7.9)
  ```
  - `ai_assisted` must be set explicitly (`true` or `false`); `publish` refuses a missing value with a suggested fix. It sits in `[publish]`, so changing it needs no version bump; the next `publish` updates the channel tags. The "AI-assisted" tag is pre-defined in the Workshop app config and in the mod.io game's tag options (mod.io accepts only admin-defined tags).
  - Before a first upload, `publish` shows the mod terms (§7.9) once per account. If Steam reports `m_bUserNeedsToAcceptWorkshopLegalAgreement`, `publish` prints the Workshop agreement link and stops: the item stays hidden until the author accepts.
  - Credentials are the author's own only: the running Steam client / SteamCMD login for Workshop, and a mod.io token from `FRONTLINE_MODIO_TOKEN` or the OS keychain. Never files in the mod folder (the validator rejects credential-like files).
  - Publishing asks for confirmation (`--yes` skips it in scripted use). A first publish is always **private/hidden**. Making it Unlisted or Public is a separate `publish --visibility …` that `--yes` never implies. Lobbies need Unlisted or Public items, since other players can't download private or friends-only ones.
  - Re-publishing an already-published `version` with a different package hash is an error (bump `version`). `--dry-run` prints the package hash, sim/presentation split, size and file diff without uploading.
  - Each Workshop upload is tagged with the game branches it supports (Workshop item versioning, Steamworks SDK ≥ 1.60, "Enable Game Branch Versions" on), derived from `api_version` via `SetRequiredGameVersions`, so a game update doesn't break installed mods. The `steamworks` crate doesn't wrap that call; we add our own binding.
- **Namespace ownership:** the first publish of an `id` registers it to that author in a small **official id registry** (a stateless web service, not a game server; the one official service at launch, §2.5). An author links their Steam and mod.io accounts to one registry account; another account publishing the same `id` gets an error. `core`, `ww2`, `coldwar` and `modern` are reserved. Folder-only mods aren't registered, and a mod set with two mods of the same `id` is rejected. If the registry is down, `publish` fails closed. Loading and playing never contact it.

**Joining a modded lobby (auto-download).** Lockstep needs every client to run identical sim content (§10).
1. The lobby advertises its **mod set**: game build + `api_version`; every non-cosmetic mod, including `core`, `ww2` and all transitive dependencies, **in the host's resolved load order**; for each mod `id`, `version`, package hash, unpacked size and channel locators (Workshop id, mod.io id + file id); and the set's sim hash. The host resolves once (topological sort, ties broken by `id` bytewise, `load_after` names outside the set ignored, `depends` ranges resolved by the host). Clients never re-resolve: they load in the advertised order and check that each `depends` is met by an earlier entry.
2. On join, the client compares the set with its cache, shows the missing mods and their total size (from the mod set; Workshop reports exact bytes only once a download starts), and asks once. If the mods don't fit the cache limit, the prompt says so and offers to raise it.
3. Missing mods download from the first locator the player can use: Workshop (Steam only, `DownloadItem`, no subscription needed), then mod.io (by file id, any store). Every download is **hash-checked** and run through the `content` crate's validator, the same one `frontline-cli validate` uses (schema, `api_version`, `deterministic = true`; golden tests skipped).
4. The game activates that exact set as a temporary **match profile**, kept until the match ends so a crashed client can reconnect. The player's own mod profile (§11 Settings) is restored afterwards. For join-in-progress and reconnect, downloads finish before the snapshot is requested, so they don't count toward the 60 s catch-up limit (§10).
- **Version pinning:** Workshop serves only the newest upload valid for the player's game branch. So hosting with a Workshop mod requires its newest version for the host's build (the game offers to update before hosting). mod.io keeps old files (fetched by file id), so a mod published to both channels can be pinned through mod.io. An author can still delete a mod.io file that isn't the active release.
- **Mid-lobby updates:** if a Workshop item changes while a lobby is open, its download no longer matches the advertised hash and the client falls back to mod.io. If no channel still serves that hash, new joiners see "host's mod is outdated" and the host is prompted to update and re-advertise.
- **No channel can supply a mod** (e.g. a local-folder mod, Workshop-only for a non-Steam player, a deleted mod.io file): the lobby browser marks the lobby "missing mods" and joining is blocked. Host-to-peer transfer is not in v1.
- **Cache limits:** default 10 GB (Deck: 4 GB), least-recently-used cleanup. Mods in the current profile or referenced by a save (§11) are never evicted. Workshop items indexed in place live in Steam's storage, not the cache.
- **Offline:** cached and folder mods load with no network. Takedown and update checks run when a channel is reachable (launch, host, join), and their results are cached.

**Lobby policy (PvP and co-op).**
- **Custom lobbies:** any mod set.
- **Ranked (post-launch, §2.5):** vanilla only, plus an allowlist of approved `kind = "cosmetic"` mods (§7.2). The ranked sim hash must equal vanilla's. The allowlist is a list of package hashes served and enforced by the official relays (§10); a client with any other cosmetic mod is refused at queue time.
- **Cosmetic mods are per-client:** they aren't part of the lobby's required mod set, so they never trigger downloads for other players. The lobby lists each player's cosmetic mods.
- **Sim-relevant** (in the sim hash): `mod.toml` without its `[publish]` table (same exclusion as the package hash); `defs/` except `defs/controls/` and `defs/art/`; `patches/`; WASM; Luau under `scripts/` (missions, AI, game modes); per map `map.toml`, `terrain.heightmap.png`, `objects.toml` and the splat's material ids; and the **armor mesh + chunk graph extracted from each `.glb`** (not the file bytes). Any file not on the presentation allowlist counts as sim-relevant.
- **Presentation allowlist** (presentation hash): visual meshes, textures, audio, shaders, `locale/`, UI scenes/themes/Luau under `ui/`, `defs/art/` (theater palettes, grade LUTs, §12.1), and licence/credit metadata (`CREDITS.toml`, `*.license.toml`, `LICENSE*`, `README*`). `defs/controls/` is client-only and in neither hash (§7.1). `tests/` (golden tests, §8.2) is in neither the sim nor the presentation hash.
- A cosmetic mod may only contain allowlisted files. A replacement `.glb` must keep a bit-identical armor mesh and chunk graph (loader-checked). Materials, textures and shaders tagged `vision_blocking` (smoke, gameplay foliage, fog) can't be replaced or patched by cosmetic mods; minimum opacity is enforced in the presenter (§12).
- **UI Luau runs in a separate presenter-side VM:** it reads only the post-spotting-filter snapshot (§5.12.3) and can't issue Commands. Widgets send input only through the normal input path, on a real user input event.

**Safety (mods are untrusted code and data).**
- **No native code ever:** no DLLs, GDExtensions or executables in mods. Behaviour comes only through sandboxed Luau and WASM (§7.5, §7.6).
- **Godot resources from mods are restricted:** meshes (`.glb`), textures, audio, shaders, and UI scenes/themes (`.tscn`/`.tres` under `ui/`). Scenes and themes go through our own allowlist parser: only `Control`-derived nodes and Theme/StyleBox/Font resources pass, and any `script` property, `GDScript` sub-resource or script `ext_resource` rejects the file. UI behaviour goes through the Luau UI API (§12). This stops a mod from running GDScript.
- **Hardened loading:** asset parsers and the def loader are fuzzed (§14); per-file and per-mod size limits; paths can't escape the mod folder.
- **Moderation:** Steam Workshop and mod.io report/takedown systems. On launch (for cached mods) and before any lobby download, adapters check channel status: Workshop `m_bBanned`; mod.io `MOD_UNAVAILABLE`/`MOD_DELETED` mod events. A taken-down package hash is blocked in the cache: it isn't loaded or advertised in lobbies, and saves/replays that need it show "mods unavailable" (§11).

### 7.9 Licensing & mod rights **[DECIDED 2026-10-31; legal text needs lawyer review before M7]**

These are the owner's decisions. The legal texts are drafted at M3 and reviewed by a lawyer before the first external release of the SDK or base assets, and no later than M7 (mod publishing): (1) game EULA incl. mod terms (licence grant to us, mods must be free, branding); (2) Frontline Base Content License; (3) prebuilt sim library licence ("Closed parts" below); (4) SDK `LICENSE-MIT`/`LICENSE-APACHE` + `THIRD_PARTY_NOTICES`; (5) Steam Workshop App-Specific Terms and mod.io Game Terms; (6) id registry privacy notice (stores linked Steam/mod.io accounts, §7.8); (7) contributor policy for the public SDK repo (§17 Q15). Uploads made directly through Steam or the mod.io website skip `frontline-cli publish`, so every mod rule below lives in (1) and (5), not only in the CLI.

| Topic | Decision |
|---|---|
| **Base game assets** (our own and commissioned models, textures, sounds, maps, defs, house shaders) | **Frontline Base Content License:** modders may use, modify and redistribute base assets **inside mods for this game only** (including its official era packs and DLC, §2.4), free of charge. Not allowed: use in other games or products, selling them, or extracting them into asset packs. The licence lets the base-asset parts of a mod be licensed to Steam/Valve and mod.io as their upload terms require (SSA §6.A, mod.io §6.2), for distributing this game's mods only. Using base assets as reference or input while making mods (including with AI tools) is allowed; training generative models for use outside this game's modding is not. **Exceptions:** assets whose manifest has `noai = true`, and voice recordings, performance capture and real-person likenesses, never enter AI generation, cloning or training. |
| **Third-party assets in the base game** | Keep their own license (§12.1); the Base Content License does not restrict them (CC0 can't be restricted; CC BY 4.0 §2(a)(5)(B) forbids added restrictions). Each base asset's `.license.toml` ships with the game and SDK, and its `mod_redistributable` field decides what mods may do: `yes` (CC0, our own, commissioned) may be copied; `with_credits` (CC-BY, OGA-BY) may be copied only with its credit entry carried into the mod's `CREDITS.toml` (validator-checked); `no` is referenced by dependency, never copied. Referencing by dependency is always preferred. |
| **Mod ownership** | Modders own their mods (to the extent they hold rights; purely AI-generated content may carry limited rights). Our mod terms (EULA, Workshop App-Specific Terms, mod.io Game Terms, and the `publish` prompt) grant us a non-exclusive, royalty-free license to host, distribute (via Steam Workshop/mod.io, §7.8), display, feature in marketing/streams/showcases, and run in compatibility testing and CI. The channels' own licences go to Valve and mod.io, not to us, so this grant must come from our terms. Steam Workshop and mod.io terms apply on top. |
| **Money** | **Mods must be free.** No paywalls, paid unlocks or patron-only gating of mod content; donation/support links in the description are fine. Paid mods may be reconsidered later. Enforced through the mod terms, which also cover uploads made outside `frontline-cli`: Workshop paid-item rules are never set up, mod.io monetisation is never enabled for this game, and our mod.io Game Terms state that a creator's mod.io "Enhanced License Rights" don't cover selling mods for this game or the base assets inside them. |
| **Name and branding** | Mods may say "for Frontline RTS" but may not use the game's logo/name to look official or endorsed. |
| **Takedowns** | Copyright complaints go through the channel's process (Workshop/mod.io); as mod.io game admin we also review reports and can remove content that breaks our terms directly. A takedown on any channel blocks that package hash in every client cache and lobby, even if another channel still serves it (§7.8). Folder-only mods and off-platform copies are handled only through the mod terms and legal notices. Our own complaints use the same channels. |
| **Code** | **Game and engine code stay proprietary.** The **modding SDK is open source under MIT OR Apache-2.0** (dual, the Rust convention), published from `sdk/` (§13) as the public `frontline-sdk` repo: the `frontline-cli` front-end, the **whole `content` crate**: def types, schema generation, validators, mod loader, load-order resolution, patching/layering and package/sim/presentation hashing (the game links the same code, §7.8 step 3), `sim-sys` bindings, JSON schemas, Luau typings, the modding docs (`MODDING.md`, `docs/llm/`, style guide), the sample mod's code, the Godot editor plugin (`addons/frontline_tools`) and the lookdev scene. Internal design docs are not part of the SDK. Base assets inside the SDK (golden samples, house shaders) stay under the Base Content License. |
| **Contributions** | The public `frontline-sdk` repo accepts outside pull requests. Every commit carries a DCO `Signed-off-by` line (Developer Certificate of Origin, checked in CI); contributions come in under the repo's MIT OR Apache-2.0 license (inbound = outbound), so they can also be used in the closed game. `CONTRIBUTING.md` explains the process; base assets in the repo (Base Content License) don't take outside contributions. |
| **Relay server** | Ships as a **free standalone binary** (Windows/Linux) for community-hosted custom lobbies, alongside the in-client host relay. The relay protocol is documented in the SDK (`docs/net/relay-protocol.md`); the relay source stays closed for now and may be opened after a security review. |
| **Closed parts the SDK needs** | `simulate`, `batch`, `ballistics`, `fortify-test`, `terrain render-section`, `replay verify`, `replay audit`, `validate --tests` and the in-editor sim preview call the closed sim through `sim-ffi` (a C-ABI library, since Rust has no stable ABI) and `sim-editor-gdext` (§13). These ship as **free prebuilt libraries** with the SDK (Windows/Linux/macOS), licensed for use with the SDK tools for this game only: mod development, testing, CI, and replay verification/analysis. No use in other products, no redistribution outside the SDK, no reverse engineering. The `replay audit` heuristics stay in the closed library. The SDK README says the CLI can't be fully built from the public repo alone. |
| **AI-made mods** | **Welcome** (the game is built for agent modding, §8). Authors declare AI use with `[publish] ai_assisted = true \| false` in `mod.toml` (required by `publish`, §7.8), shown as an "AI-assisted" tag on Workshop/mod.io and in the in-game browser. Uploads made outside `frontline-cli` are bound by the same rule through the mod terms. License and quality rules apply as for any mod; assets with `noai = true` never enter generative or training steps (§12.1). |

**Compliance tasks (engineering):**
- `cargo-deny` in CI with a license allowlist; a generated `THIRD_PARTY_NOTICES` file shipped with the game, the SDK and each prebuilt closed library (covering what it statically includes). Key dependencies: Godot (MIT, plus its bundled third-party components from `COPYRIGHT.txt` / `Engine.get_copyright_info()`), godot-rust `godot` (MPL-2.0: every executable that contains it must tell recipients where to get the source of the exact version we ship, linked from `THIRD_PARTY_NOTICES`; we also publish any modified MPL files; the rest of the game stays closed as a Larger Work, MPL §3.2–3.3), `mlua` (MIT), Luau (MIT; keep both the Roblox and Lua.org/PUC-Rio notices), `wasmtime` (Apache-2.0 with LLVM exception), `bevy_ecs`, `steamworks` and `modio` crates (all MIT OR Apache-2.0), and font licences (e.g. OFL).
- Valve's Steamworks redistributables (the Steam API library used by `publish`) are under the Steamworks SDK Access Agreement, not MIT/Apache: our official game and SDK builds may bundle them, but they are not committed to the public SDK repo, forks can't rely on our right to ship them, and SDK notices list them separately.
- The game's own Steam store page carries Valve's AI-content disclosure if any AI-generated content ships in the base game or appears on its store page or in marketing; a live-generated declaration with guardrails is needed only if the game ever generates AI content at runtime. AI tools used only behind the scenes (e.g. code assistants) need no disclosure (Steamworks form revised January 2026). Valve's rules don't cover Workshop items; the `ai_assisted` mod tag is our own rule. We treat a community mod curated into the base game or shown on the store page as our own content for the disclosure.
- Hosted base assets (SDK downloads, asset pages) carry a machine-readable AI-training/TDM reservation (e.g. `robots.txt` and TDM-Rep), since some EU courts read the DSM Directive Art. 4(3) opt-out as needing a machine-readable form. The licence clause binds only those who accept it.
- In-game credits merge base `CREDITS.toml` and every active mod's credits (§12.1).

**Sources:** <https://www.mozilla.org/en-US/MPL/2.0/>, <https://www.mozilla.org/en-US/MPL/2.0/FAQ/>, <https://godotengine.org/license/>, <https://raw.githubusercontent.com/Noxime/steamworks-rs/master/Cargo.toml>, <https://raw.githubusercontent.com/nickelc/modio-rs/master/Cargo.toml>, <https://partner.steamgames.com/documentation/sdk_access_agreement>, <https://partner.steamgames.com/doc/api/ISteamUGC>, <https://store.steampowered.com/subscriber_agreement/>, <https://mod.io/legal/terms>, <https://docs.mod.io/monetization/enabling>, <https://creativecommons.org/licenses/by/4.0/legalcode.en>, <https://www.pcgamer.com/software/ai/steam-updates-ai-disclosure-form-to-specify-that-its-focused-on-ai-generated-content-that-is-consumed-by-players-not-efficiency-tools-used-behind-the-scenes/>, <https://legalblogs.wolterskluwer.com/copyright-blog/the-new-copyright-directive-text-and-data-mining-articles-3-and-4/>

---

## 8. AI-Agent-Friendly Tooling

The goal: an AI agent (or a human with a terminal) can **create, validate, test and balance** content without launching the graphical game.

### 8.1 `frontline-cli`

| Command | Purpose |
|---------|---------|
| `frontline-cli schema --out schemas/` | Export JSON Schemas for every def type (from Rust types). |
| `frontline-cli validate [mod]` | Parse, schema-check, resolve references, check assets exist, lint scripts. Machine-readable output with `--json` (file, line, path, message, suggested fix). |
| `frontline-cli explain <id>` | Show the final merged def and which mod set each field. |
| `frontline-cli ballistics --ammo ww2:75mm_apcbc --target ww2:t34_85 --zone hull_front --range 800 --angle 30` | One-shot penetration calculation for balancing. |
| `frontline-cli fortify-test --fortification ww2:trench_standard --material clay --vs ww2:81mm_he --samples 500` | Fire N deterministic shots/shells at a fortification and report the casualty rate for occupants. Lets an agent prove that a new fortification actually protects. |
| `frontline-cli terrain render-section --map m --line x1,y1,x2,y2 --out section.svg` | Cross-section plot of the terrain (incl. edits) for debugging dig profiles. |
| `frontline-cli simulate --scenario x.toml --ticks 18000 --seed 7 [--inputs session.jsonl] --report out.json` | Headless battle; outputs casualties, kill matrix, timings, hash. `--inputs` injects a recorded or scripted per-tick Command stream (JSONL text form of the replay command stream, §11), e.g. direct-control input (§5.12.8). |
| `frontline-cli batch --scenario x.toml --seeds 1..200 --parallel` | Monte-Carlo balance runs → win-rate & confidence intervals. |
| `frontline-cli replay verify <file>` | Re-simulate and check hashes (regression tests for mods). |
| `frontline-cli replay export <file> --with-mods` | Bundle a replay with its exact mod folders for bug reports and archives (§11). |
| `frontline-cli replay audit <file>` | Per-player cheat indicators from the full input stream (§10): aim toward unspotted enemies, hits on unspotted targets, spot-to-shot reaction time, hit rate vs D-adjusted human baselines, aim-assist strength, re-stamp rates per player. |
| `frontline-cli publish --to workshop,modio [mod]` | Validate, hash and upload a mod to Steam Workshop and/or mod.io from one folder; records channel ids in `mod.toml` (§7.8). Asks for confirmation unless `--yes`. First publish is private; `--visibility` makes it Unlisted/Public as a separate step. `--dry-run` uploads nothing. Requires `[publish] ai_assisted` (§7.9). |
| `frontline-cli asset import <src> --class <c> --theater <t>` | Licence gate, manifest and house-look processing for a new asset (§12.1). |
| `frontline-cli art lookdev --asset <id> --out <png>` | Render the lookdev views of an asset (§12.1). |
| `frontline-cli docs --out docs/api/` | Generate Markdown API docs for Luau and data formats. |

### 8.2 Agent-oriented conventions

- **Everything round-trips through text** (TOML/JSON/Luau). Binary assets are referenced, never required to change logic.
- **Error messages are actionable**: include the expected type/range and the closest valid ID ("did you mean `ww2:75mm_apcbc`?").
- A **`MODDING.md` + `docs/llm/` pack**: concise, example-heavy reference designed to fit in an agent's context window (one file per def type, with a full working example), including `docs/llm/licensing.md`: the §7.9 rules as checklists (copy vs reference, `ai_assisted`, mods must be free, `noai`).
- **Template generator**: `frontline-cli new unit --kind vehicle --based-on ww2:pz4_h my_mod:pz4_j`.
- **Live dev bridge (optional)**: a local JSON-RPC / MCP endpoint in dev builds exposing read-only queries (entity list, unit state, profiler data) plus a "submit command" call — so an agent can observe a running match, reproduce bugs, or script tests.
- **Golden tests for mods**: a mod can ship `tests/*.scenario.toml` with expected outcomes (e.g., "Pz IV front at 500 m must not be penetrated by 45 mm AP"); `validate --tests` runs them.
- **Publishing is human-gated:** agents use `publish --dry-run` freely. A real publish needs the author's own credentials, and making a mod public is always a separate, explicit step (§7.8).

---

## 9. Game AI (Computer Opponents)

Layered, so each layer is moddable independently:

1. **Strategic commander** (per AI player, ~1 Hz): reads influence maps (threat, control, resources) and allocates squads to goals (capture, defend, flank, reinforce, **fortify**). Fortify goals pick a defensive line from threat direction + terrain and request blueprints along it. Utility-scored goals; weights in data.
2. **Tactical squad AI** (~5 Hz): behaviour trees defined in data (`ai/squad_attack.bt.toml`) — move by bounds, use cover, suppress, request support.
3. **Unit reflexes** (every tick, cheap): return fire, take cover when suppressed, crew bail-out, reload. Skipped for a directly controlled unit or station, except bail-out from a burning vehicle (§5.12.6).

AI emits Commands only (P6), runs inside the sim tick (so it's deterministic and replayable) and has hard per-tick work-count budgets (§2.2) with simulation LOD (§6). Difficulty modifiers are data, never hidden cheats unless declared ("vision_bonus = true" in difficulty def). AI cover/fire-position rays come from 20 % of the tier's global ray budget.

---

## 10. Networking

- Up to 10 players (5v5). Every match runs through a relay (official or host-run). The relay closes each tick's bundle at a wall-clock deadline; late commands are re-stamped to the next open tick, so no client ever waits on another client's CPU.
- Input delay D is set at match start from the worst client's P99 one-way latency, capped by the lobby rule `max_input_delay_ticks` (PvP default 4, or 3 if the M1 feel gate fails without `fire_rewind`, §15, §17 Q13; co-op 6). A client above the cap has its commands re-stamped late by the relay, so only that player feels the extra delay, and the lobby flags it. D changes mid-match only through a relay-issued `SetInputDelay` command scheduled at N+D, like `SetSpeed`. Packets repeat the sender's last 4 ticks of commands. The lobby shows each player's latency.
- Direct control input is sent as change-only Commands (`TakeControl`/`ReleaseControl`, `MoveInput`, `AimInput`, `Trigger`, `UseAction`; §5.12.3), at most one `AimInput` per player per tick (≈ 1 KB/s uplink with the 4-tick repeat, est.). Presentation prediction covers only the directly controlled unit (§5.12.4).
- **Re-stamp transparency.** Each command carries `target_tick`. Every client counts how often and how far its commands were re-stamped (from the sim's applied-tick records) and shows it in the network HUD; the lobby shows each player's re-stamp rate at match end, and the replay audit flags a relay that delays some players more than its latency measurements explain.
- **Cheating (honest scope).** Lockstep clients hold the full world, so fog of war is enforced only on unmodified clients (Rust-side spotting filter, §5.12.3). What any client can't bypass: sim-side turn rates, dispersion and input validation (§5.12.3) and unpredictable RNG (§5.11). Every replay holds the full input stream, so `frontline-cli replay audit` (§8.1) detects aimbot/ESP patterns offline at no sim cost; host-run lobbies get kick/ban lists, and ranked play (official relays) runs the audit live on the relay's headless sim. No kernel anti-cheat is planned (Proton/Deck, modding).
- Speed governor, `SetSpeed` and `SetController` are relay-issued commands scheduled at N+D, driven by relay-observed tick acks (§2.2). No sim system reads game speed.
- Desync handling: hash mismatch → pause, dump, auto-diff. At join the client must hold every package hash in the lobby's mod set (auto-downloading the rest, §7.8) and reproduce the advertised load order and sim hash; any mismatch blocks the join.
- Join-in-progress/reconnect: the snapshot comes from the official relay's headless sim (rolling incremental snapshots, once official relays exist) or the fastest non-handheld peer (the only source at launch). The joiner fast-forwards headless; refuse if catch-up exceeds 60 s.
- Battle-size tier, eligibility (SCS) and lobby rules: §2.2.
- **Launch setup (§2.5):** custom lobbies with a server browser and host-run relays for both PvP and co-op; no ranked play. Host-run relays come in two forms: built into the host's game client, or the **free standalone relay binary** for community/dedicated hosting (protocol documented in the SDK, §7.9). Official relays and matchmaking come with ranked play post-launch.

---

## 11. Persistence

| Artifact | Contents | Format |
|----------|----------|--------|
| Save game | Full sim world (canonical serialization) + mod set record | `bincode`/`postcard` + zstd |
| Terrain state | Per-chunk height/material/flag deltas vs. base map, plus placed structures | zstd-compressed delta tiles inside the save |
| Replay | Initial seed + scenario + mod set record + command stream | Same, append-only |
| Settings | User options, keybinds, active mod profile and enabled cosmetic mods | TOML |
| Campaign state (reserved, post-launch, §2.5) | Persistent roster, veterancy, resources | TOML/JSON (mod-readable) |

**Mod set record.** Saves and replays store the same mod set a lobby advertises (§7.8): game build, `api_version`, every mod in load order with `id`, `version`, package hash and channel locators, and the sim hash. Loading activates that set as a match profile and fetches missing mods by hash through any channel that still serves them. If none does (Workshop item updated since, takedown, local-folder mod), the file shows "mods unavailable" with the missing list; it never loads with a substitute version. Mods referenced by the player's saves are pinned in the cache. Replays don't pin; `frontline-cli replay export --with-mods` bundles a replay with its exact mod folders for bug reports and archives.

Save versioning: each component type has a schema version and migration function; mods declare migrations for their own components.

Replay encoding: the command stream includes each tick's `tick_salt` (§5.11). Direct-control commands are delta-encoded per player (aim as i8/i16 deltas from the previous `AimInput`, tick as a varint gap) before zstd. A JSONL text form of the same command stream (one Command per line, with its tick and player) is accepted by `simulate --inputs` for scripted and agent input. CI checks the §2.2 replay budget with a scripted 10-player replay in which each player is in direct control half the time (M1).

---

## 12. Presenter (Rendering, Audio, UI)

- Reads snapshots only; maintains its own *visual entities* mapped from sim entity IDs.
- Cosmetic-only systems: particles, tracers, ragdolls, debris physics, foliage sway, shell casings — free to be non-deterministic and quality-scaled.
- Animation: state machine driven by sim state (stance, moving, firing, reloading) defined in data.
- Audio: event-based (sim emits `Fired`, `Impact`, `Ricochet`, `NonPenetration`, `Penetration`, `ComponentHit`, `TargetDisabled`, `Destroyed`; §5.12.4); distance attenuation, occlusion via cheap raycasts, voice lines per faction via locale packs.
- Implemented in **Godot 4.7.2** (§4): `RenderingServer`/`MultiMesh` for units, Godot particles/decals/volumetric fog for VFX, Godot audio buses for sound (in-tank muffling, battlefield reverb), skeletons per §4.1 rule 4.
- Quality presets are per client. Vision-blocking elements (smoke, gameplay foliage, fog) keep a minimum opacity on every preset. Spotted units are always drawn with a class marker readable at 800p. Steam Deck profile: §2.2.
- **Direct control (§5.12):** top-down, over-the-shoulder and chase cameras, gunner optic view (replaces the main camera, zoom-adjusted LOD), reticle and gun-laid marker HUD, and prediction plus instant feedback for the controlled unit. Predicted effects are cosmetic and never feed the sim. Unspotted enemies are filtered out in Rust before render, audio, aim assist and UI (§5.12.3).
- UI: Godot `Control` scenes + themes. In-game UI layouts are scenes that mods can add (script-free, allowlist-parsed, §7.8); UI Luau binds widgets and behaviour through a small UI API, presenter-side only.
- **Editor**: a **Godot editor plugin** ("Frontline Tools") for terrain sculpting, base-material painting, object/fortification placement, zones, triggers and script attachment. It runs the real Rust sim in-editor for previewing, and saves to the text formats in §7 (TOML/heightmap PNG), *not* to `.tscn` scenes, so maps stay engine-agnostic and agent-editable. Modders get it with the free Godot editor + our SDK: the plugin is open (§7.9), and the sim preview runs through the free prebuilt `sim-editor-gdext` library (§13).

### 12.1 Art direction **[DECIDED 2026-10-31 except the style line; §17 Q7]**

**Style: decided by the style-gate A/B test at M2 exit (owner decision 2026-10-31).** Candidates: *grounded realism, mid-fidelity, graded* (physically based PBR, real paint colours, photoscan environment; research favourite, ~70 % confidence) vs *stylized realism*, both with the discipline layer below.
- **Until the test:** content work stays style-neutral: blockouts, proportions, LOD chains, armour meshes, rigs and UVs, which both styles share. Final texturing and hero assets wait for the result. Library intake (CC0 photoscan materials) runs only for prototypes. Everything in this section except the "Style" line is decided and applies to either choice. Budget numbers, palette values and value bands are estimates, checked at M0 and in the style slice.
- **Look:** physical sun + sky, filmic tone curve, per-theater grade. Terrain sits slightly darker and less saturated than units. Authentic proportions and details (Zimmerit, castings, HVSS) hold up in close cameras (§5.12.5).
- **Style gate (before content production, by M2 exit):**
  - Build Panther + rifle squad + 100 m Normandy patch under both grades.
  - Run a blind 800p Low A/B with 30–50 genre players. Stylized wins only if it is preferred by more than ~60/40 and is not called "cartoonish".
  - Run the Deck check: units must stay separable from terrain with aids on, and the GPU must stay ≤ 60 % busy (§2.2).

**Per-theater palettes** (`mods/ww2/defs/art/palettes/<theater>.toml`, presentation-only, not in the sim hash, §7.8). Each palette sets anchor albedos (sRGB), a grade LUT, a haze colour and a unit-vs-terrain value band.

| Theater | Terrain anchors | Haze / sky | Grade | Unit vs terrain |
|---|---|---|---|---|
| Belarus summer (Bagration) | forest green #3F4A2C, marsh ochre #8C7A4A, sand road #A89170, birch bark #D6D2C4 | warm grey-green #B9BFAE | muted greens + ochre | units ≥ ΔL* 10 brighter |
| Normandy bocage / coast | hedgerow #4C5A33, pasture #6E7F45, chalk sand #C9BE9F, slate sea #5C6B70 | golden haze #D8CFA8 | golden-green summer | units ≥ ΔL* 10 brighter |
| Ardennes winter (Dec 1944) | snow #DDE2E6 (max L* 90), spruce #2E3A33, beech bark #5B4A3C | overcast #A9B1B8 | blue-grey, low saturation | **inverted:** units ≥ ΔL* 25 darker |
| Siegfried Line autumn | beech rust #8A5A2E, wet field #5E5A3E, concrete #9A968C, mud #4A3F33 | damp grey #9EA39B | desaturated warm | units ≥ ΔL* 10 brighter |

**Readability aids (client options; fairness rules §2.2, §12):**
- **Always on (not an option):**
  - A spotted unit is drawn with a class marker readable at 800p (§2.2).
  - Smoke, gameplay foliage and fog keep minimum opacity on every preset. Cosmetic mods cannot change `vision_blocking` assets (§7.8).
- **Toggleable:**
  - team rim tint (instance custom data)
  - selection/hover outline
  - unit value lift (+0–15 % L*)
  - terrain desaturation (0–30 %)
  - colour-blind team palettes
  - marker size and opacity
- **Hard rules for aids:**
  - Aids apply only to entities already in the Rust-filtered visible set (§5.12.3). No aid can reveal, extend or outline anything the sim has not spotted.
  - No aid lowers smoke or fog below its minimum opacity.
  - Ranked play allows the same aids to every player.

**Fidelity budgets per asset class.** One `.glb` per asset. Presets choose the LOD bias and the KTX2 mip skip, so assets are not authored twice. LOD distances follow §2.2 (Deck: LOD0 ≤ 25 m, LOD1 ≤ 80 m, LOD2 ≤ 200 m, impostor beyond; desktop ≈ 1.5×).

| Class | LOD0 / LOD1 / LOD2 tris (max) | Materials (LOD0 / LOD1+) | Textures desktop → Deck/Low | Texel density desktop (Deck) |
|---|---|---|---|---|
| Tank / SPG / halftrack | 45k / 14k / 4k + octahedral impostor (1K → 512) | 2 / 1 | 2K unique ABD/N/ORM + shared trim & detail normal → 1K | 256 px/m unique, 1024 px/m via trim (128) |
| Truck / gun | 20k / 6k / 1.5k + impostor | 2 / 1 | 1K–2K → 512–1K | 256 (128) |
| Soldier (shared skeleton, modular gear) | 5k / 1.8k / 600 + impostor | 1 / 1 | 1K atlas per faction kit → 512 | 512 (256) |
| Small arms (first person / held) | 6k / 1.5k / — | 1 | 1K → 512 | 1024 (512) |
| Building module / bunker | 15k / 5k / 1k (HLOD clusters) | ≤ 3 / 1 | tiling + trim sheets, 2K → 1K | 512 (256) |
| Props / debris | 3k / 800 / impostor or cull | 1 | shared atlas 1K → 512 | 256–512 (128–256) |
| Trees / foliage | 8k / 2k / cards + impostor | 1–2 | 2K atlas → 1K | n/a (card atlas) |
| Terrain layers | CDLOD (§4.1 rule 6) | ≤ 4 layers/tile Deck, 8 desktop | 2K → 1K tiling, triplanar | 512 (256) tiling |

- **Scene check:** a Battalion late-game scene stays ≤ 1,500 draws and ≤ 2 M triangles across all passes (§2.2, M0/M5 gates).
- **Texture memory (est., set at M0):** texture pool ≤ 1.0 GB at Deck/Low (fits the ≤ 4 GB total on 8 GB PCs), ≤ 3 GB at desktop High.
- **Encoding:** KTX2 (§4); UASTC for normal maps, ETC1S/UASTC-RDO for the rest.

**Materials and shaders (Godot 4.7 Forward+).** Assets use a small set of house shaders, not per-asset `StandardMaterial3D` variety (that keeps the draw budget and the look consistent):
- `fl_vehicle`:
  - camo mask × palette slots
  - ORM and normal maps
  - procedural dust/mud by height, wear from a baked curvature mask
  - damage/wreck parameter and team rim, both from MultiMesh custom data (§4.1)
- `fl_soldier`: bone-texture skinning (§4.1 rule 4) and faction tint masks.
- `fl_terrain`: triplanar CDLOD with splat layers.
- `fl_foliage`: wind; vision-blocking variants enforce minimum opacity.
- `fl_building`, `fl_impostor`.
- **Global uniforms:** snow cover, wetness and theater grade, so every season works without new textures.
- **Lighting:**
  - Physical sky or CC0 HDRI ambient, per-map reflection probes, AO baked into textures.
  - AgX/filmic tone map. Per-theater 3D LUT via `Environment.adjustment_color_correction`.
  - SDFGI/VoxelGI are not required. Desktop-only extras: SSAO, volumetric fog (cosmetic), longer shadow cascades. Deck: §2.2 profile.

**Asset intake pipeline** (`frontline-cli asset import <src> --class vehicle --theater normandy`; agents and modders run the same command):
1. **Licence gate.**
   - Allowed in `mods/core` and `mods/ww2`:
     - CC0-1.0
     - our own work
     - commissioned work with a written assignment (or licence) covering modder reuse inside mods, AI-tool input and training for this game's modding (§7.9); performer contracts (voice, mocap, likeness) are exempt from the AI part
     - CC-BY-4.0 and OGA-BY only as loose files with credits (capped for hero assets)
   - CC-BY-SA is allowed only in third-party mods, as isolated files, never in shared atlases.
2. **Banned licence ids** (validator hard-fail):
   - all NC and ND licences
   - GPL/LGPL art
   - Fab Standard (including Megascans) and UE-only content
   - Sketchfab Free Standard, Standard and Editorial
   - TurboSquid and CGTrader Royalty Free and Editorial
   - Poliigon, Textures.com, BlendKit RF
   - ShareTextures without a written OK
   - Free3D "personal use"
   - Spring:1944, IFA3/APL
   - no licence stated
3. **Manifest** `<asset>.license.toml` next to every file:
   - `source_url`, `platform_id`, `uploader`, `author`, `spdx`
   - `downloaded`, `sha256_original`, `sha256_derived[]`, `parents[]`
   - `noai`, `mod_redistributable = "yes" | "with_credits" | "no"`
   - `reviewer`, `s86a_check`, `period_check`
   - Archived evidence: page snapshot, licence badge, Sketchfab `/v3/models/{uid}` JSON.
4. **Provenance checks.**
   - Hard reject:
     - a game or anime name in the title or tags (warthunder, call_of_duty, wot, pubg and similar)
     - game-style atlases or LOD naming
     - "HD upscaled" textures
     - Telegram/Discord distribution
   - Human review:
     - a third party credited in the title
     - description vs badge mismatch (follow the stricter one)
     - UE4-mannequin derivatives
     - museum scans used as hero assets (needs the scanner's written confirmation)
     - political text in the description
   - Content checks:
     - the §2.3 / §86a symbol validator
     - date check against the 1944–45 roster
     - modern elements and logos (CC0 does not clear trademarks)
5. **House-look processing.**
   - Mesh: decimate/retopo check, LOD chain, impostor bake. Armour mesh and chunk graph are authored separately (§5.7).
   - Texture: delight, clamp albedo to the palette, normalise roughness, resize to class density, KTX2 encode.
6. **Derivatives inherit the strictest parent licence.** Atlases record a manifest per region.
7. **`CREDITS.toml` per mod**, generated from the manifests and fed into the in-game credits.
   - CC BY entries carry title, author, source URI, licence URI and a "modified by" note.
   - Mods reference base assets by dependency where possible. Copying follows the source manifest's `mod_redistributable` (§7.9): `yes` copies freely, `with_credits` copies with its credit entry, `no` is never copied (validator hard-fail).
8. **Agent rules.**
   - Official APIs only: the Poly Haven API with a unique User-Agent, and Sketchfab's download API.
   - No scraping, and no automation on ShareTextures.
   - NoAI-tagged and CGTrader free assets never enter generative or training steps.
   - A human approves each new source domain.

**Style guide and validator (for modders and AI agents).**
- **Reference material:** `docs/art/STYLE_GUIDE.md` plus a `docs/llm/art.md` digest (§8.2).
- **Golden samples in the SDK** (Panther, rifle squad, Normandy patch) as calibration targets.
- **`sdk/godot/art/lookdev.tscn`:** each theater's light and grade, a colour checker, and fixed cameras: RTS 85 m, chase 6 m, optic.
- **`frontline-cli art lookdev --asset <id> --out <png>`:** renders those views so agents can see their work. `art lookdev` and the impostor bake in `asset import` launch the user's Godot 4.7.2 binary on the SDK Godot project with a real renderer (not `--headless`) and need a GPU; the CLI crate itself does not link Godot.
- **`frontline-cli validate` art lint** (`--json`, with suggested fixes):

| Check | Fail | Warn |
|---|---|---|
| Tris per LOD / material count | over the class budget | > 85 % of budget |
| Texel density (sampled UV area vs world area) | outside ±50 % of class target | outside ±25 % |
| Albedo range (sRGB) | < 30 or > 240 (snow up to 245) | ΔE2000 > 12 from the theater palette median |
| Roughness / metallic | metallic > 0 on painted or cloth surfaces; roughness < 0.2 on non-wet surfaces | — |
| Unit vs terrain value band (lookdev histogram) | band from the palette table not met | within 3 L* of the band |
| LOD / impostor / armour mesh present | missing | — |
| Manifest, licence id, hashes, credits | missing, banned, mismatched | CC BY in a hero asset |

- **Base content licence (decided, §7.9):** the "Frontline Base Content License" for our own and commissioned assets (third-party assets keep their own licences, with copying set by `mod_redistributable`) plus Steam App-Specific Terms (SSA §6.B) and mod.io game terms. Lawyer review per §7.9.

**Sources:** <https://en.wikipedia.org/wiki/Unity_of_Command_II>, <https://legacy.prod.metacritic.com/game/pc/unity-of-command-ii>, <https://www.gamepressure.com/newsroom/company-of-heroes-3-with-mixed-reviews-on-steam/z3517f>, <https://steampulse.org/game/1677280>, <https://en.wikipedia.org/wiki/Men_of_War_II>, <https://legacy.prod.metacritic.com/game/pc/men-of-war-ii>, <https://gamebrain.co/game/men-of-war-2>, <https://polyhaven.com/license>, <https://polyhaven.com/our-api>, <https://raw.githubusercontent.com/Poly-Haven/Public-API/master/ToS.md>, <https://docs.ambientcg.com/license/>, <https://texturecan.com/terms/>, <https://3dtextures.me/about>, <https://www.cgbookcase.com/>, <https://www.sharetextures.com/p/license>, <https://quaternius.com/faq.html>, <https://kenney.nl/support>, <https://help.poliigon.com/en/articles/8749749-asset-use-licensing>, <https://www.textures.com/faq-license>, <https://www.blendkit.com/docs/licenses/>, <https://www.blendkit.com/articles/our-position-on-cc0-models/>, <https://www.turbosquid.com/help/en/articles/9937423-royalty-free-license-faq>, <https://help.cgtrader.com/hc/en-us/articles/360015124437>, <https://www.cgtrader.com/forum/selling-buying-3d-models/regarding-royalty-free-licenses>, <https://www.cgtrader.com/pages/subscription-fair-use-policy>, <https://dev.epicgames.com/documentation/en-us/fab/licenses-and-pricing-in-fab>, <https://forums.unrealengine.com/t/fab-license-terms-standard-license/2094243/4>, <https://forums.unrealengine.com/t/fab-ue-only-content-licensing/2082870>, <https://forums.unrealengine.com/t/megascans-license/2081542/9>, <https://forums.unrealengine.com/t/using-free-assets-with-standard-license-in-virt-a-mate/2080480/3>, <https://80.lv/articles/megascans-no-longer-free-after-2024/>, <https://sketchfab.com/blogs/community/kitbash-acquires-sketchfab-and-artstation>, <https://digitalproduction.com/2026/08/12/kitbash-buys-artstation-and-sketchfab/>, <https://sketchfab.com/blogs/community/introducing-the-noai-createdwithai-tags/>, <https://creativecommons.org/licenses/by/4.0/legalcode.en>, <https://store.steampowered.com/subscriber_agreement/>, <https://mod.io/legal/terms>, <https://opengameart.org/forumtopic/oga-and-license-faq-updatechanges>, <https://raw.githubusercontent.com/spring1944/spring1944/master/docs/License.txt>, <https://www.bistudio.com/community/licenses/licensed-data-packages>, <https://www.cyark.org/data-use-policy>, <https://commons.wikimedia.org/wiki/Commons:3D>, <https://dashboard.si.edu/openaccess/faq>, <https://legal.na.wargaming.net/en/player-content-policy>, <https://docs.godotengine.org/en/4.7/community/asset_store/submitting_to_asset_store.html>, <https://www.cgchannel.com/2021/12/get-290-free-materialx-materials-from-amds-online-library/>

---

## 13. Repository Layout (proposed)

```
frontline-rts/      # private repo (proprietary, §7.9)
  crates/
    sim/            # deterministic core; no render/audio deps
    # content crate: see sdk/crates/content/ (whole crate open, §7.9, §17 Q15)
    script/         # Luau host + API bindings + typings generator
    plugin-wasm/    # WASM system host
    ai/             # commander, BT runtime, influence maps
    net/            # lockstep, lobby, relay
    godot-bridge/   # GDExtension (godot 0.5.5, api-4-7): sim thread, input→Commands, snapshot→MultiMesh
    sim-ffi/        # stable C-ABI cdylib over sim+script+ai for SDK tools; prebuilt Win/Linux/macOS (§7.9)
    sim-editor-gdext/ # prebuilt GDExtension that runs the sim inside the editor plugin (§12)
  godot/            # Godot 4.7.2 game project: presenter scenes, UI, VFX, audio
    project.godot
  mods/core/        # era-agnostic rules, materials, UI shell (Base Content License)
  mods/ww2/         # base game content: Jun 1944 – early 1945, Germany / USSR / USA (Base Content License)
  design/           # internal design docs, incl. this file (not published)
  sdk/              # mirrored to the public `frontline-sdk` repo; MIT OR Apache-2.0 unless a file's header or .license.toml says otherwise
    LICENSE-MIT  LICENSE-APACHE  THIRD_PARTY_NOTICES
    crates/content/ # whole crate: def types, schema gen, validators, mod loader, load order, patching, mod-set hashing (the game links this same crate)
    crates/cli/     # frontline-cli front-end (no Godot dependency)
    crates/sim-sys/ # open bindings that locate/load the prebuilt sim-ffi library
    godot/addons/frontline_tools/  # editor plugin: map/mission/fortification editor (loads sim-editor-gdext)
    godot/shaders/  # house shaders fl_* (Base Content License)
    godot/art/lookdev.tscn  # lookdev scene (§12.1)
    schemas/  typings/  # generated JSON Schemas and .d.luau files
    docs/           # MODDING.md, docs/llm/, art/STYLE_GUIDE.md, net/relay-protocol.md
    CONTRIBUTING.md # DCO sign-off, inbound = outbound (§7.9)
    samples/sample_mod/  # code MIT OR Apache-2.0; golden-sample assets under the Base Content License
  bench/            # performance scenarios
  tests/            # determinism, replay, golden ballistics
```

---

## 14. Testing Strategy

- **Unit tests** per system (pure functions over component data).
- **Golden ballistics tests**: table of (ammo, target zone, range, angle) → expected outcome.
- **Determinism tests**: same scenario, N seeds, multi-platform hash comparison in CI.
- **Replay regression**: a library of recorded matches re-simulated each commit; hash drift requires an explicit "sim-changing" label on the PR.
- **Performance gates**: `bench/battalion`, `bench/regiment` and `bench/spike` with the per-stage ms budgets and work-count budgets of §2.2 (CI rules in §6).
- **Mod compatibility CI**: popular community mods (opt-in) validated against each new API version.
- **Fuzzing**: random command streams, malformed def files, mod assets (glTF incl. armor-mesh/chunk-graph extras, KTX2, OGG) and the `.tscn`/`.tres` allowlist parser into the loader and sim, including `MoveInput`/`AimInput`/`Trigger`/`UseAction` streams with out-of-range values, stale `control_gen`, unowned units and re-stamped ticks.
- **Direct-control tests** (§5.12):
  - Scripted input (`simulate --inputs`) gives identical hashes across the platform and thread-count matrix.
  - Parity: the same aim/fire sequence issued by AI and by player Commands gives the same hit distribution (statistical CI, `direct_control_accuracy_mult = 1.0`, AI aim error off).
  - Limits: no `AimInput` stream turns a turret or body faster than its def rate, and no shot leaves outside the reached traverse.
  - Release: death, `SetController`, ownership loss (incl. co-op share revoked), `max_direct_control_time_s` timeout and morale break release control and zero held inputs in the same tick.
  - Prediction (presenter, not hashed): `step_controlled` on a prediction slice matches the sim exactly when nothing else changes; misprediction rate and correction distance are logged in the feel-test scenarios.
- Thread-count fuzz (1/2/3/8 workers) with identical hashes.
- Save → load → continue hash test.
- Statistical lethality CI (fragment weighting, burst bundles).
- Cross-platform hash matrix: SteamOS native, SteamOS + Proton, Windows, macOS ARM.

---

## 15. Milestones

| # | Milestone | Exit criteria |
|---|-----------|---------------|
| M0 | Sim skeleton + Godot spike | Headless ECS, Q16.16 fixed point, tick loop with `tick_hz` match constant, incremental state hash, CLI `simulate` deterministic on SteamOS (native + Proton), Windows and macOS ARM, including thread-count fuzz. The §2.2 M0 kernel benchmarks (LOS ray, ORCA) and a paced composed tick measured on Deck LCD/OLED, Ryzen 5 3600 and M3, passing the §2.2 M0 gates. Godot 4.7.2 renders bone-texture MultiMesh units + 1,500 tracers + smoke from sim snapshots while the sim runs: Battalion caps (700 soldiers + 100 vehicles) at 30 fps on Deck (Deck preset, GPU ≤ 60 % busy), and Regiment caps (2,000 + 300) at 60 fps at 1080p on GTX 1660 class. `cargo-deny` licence allowlist and generated `THIRD_PARTY_NOTICES` in CI; `.license.toml` manifests required from the first imported asset (§7.9, §12.1). |
| M1 | Infantry sandbox + 2-player lockstep | A minimal 2-client lockstep match through a local relay (commands, hash check, desync dump); all later features are tested networked by default. Basic infantry direct control (move, aim, fire, local prediction) and the blind **direct-control feel test** at D = 1, 3 and 5 ticks (latency injected at the local relay), including the `fire_rewind` experiment (§5.12.3) and a 20 Hz arm (§2.2). **Pass (blind A/B, ≥ 8 testers):** local feedback within one presenter frame; at D = 3 and D = 5, hit rate on a crossing runner at 50/150/300 m within 15 % of D = 1; ≥ 70 % of testers rate aiming "responsive". If the uncompensated baseline fails and `fire_rewind` passes, `fire_rewind` becomes the PvP default; if both fail, PvP lobbies cap D at 3 (§10, §17 Q13). Replay size with direct control meets §2.2. Squads move (coarse HPA* + budgeted refinement + ORCA), shoot (60 Hz-substepped projectiles), take cover, suppress. Spotting meets the §2.2 cadence. The Battalion infantry cap (700 soldiers) runs under the Deck budget and the Regiment infantry cap (2,000 soldiers) under the desktop budget (§2.2). |
| M2 | Vehicles & armor | Crew seats, components, armor mesh hit tests, penetration model, `ballistics` CLI + golden tests. Basic vehicle direct control (drive + turret traverse + fire, chase camera). Art style slice: Panther + rifle squad + 100 m Normandy patch, blind A/B at 800p (§12.1). |
| M2.5 | **Terrain spike** (early, de-risks the core pillar) | Two-tier heightfield, `Dig`/`Crater` ops, budgeted nav re-bake; `fortify-test` shows a trench cuts small-arms and HE casualties versus open ground. Re-check the Deck/desktop tier caps with the new costs. |
| M3 | Modding v1 | Mod loader (folders + content-addressed cache, package/sim/presentation hashes, mod set records in saves/replays), patches, JSON Schemas, `validate`/`explain`, Luau missions with hot reload, all base content moved into `mods/core` + `mods/ww2`. SDK boundary in place (§13: `sdk/` tree, `sim-ffi` C ABI, `sim-sys` bindings), with CI building the open tree without the closed crates' source; legal texts drafted (§7.9). |
| M4 | World & fortifications | Streaming terrain, full construction system (profiles, pieces, blueprints, materials), dugouts/bunkers, modular player-built buildings, destruction & collapse, decorators. Re-check the Deck/desktop tier caps with the new costs. |
| M5 | Direct control & UX | Full direct control (§5.12): all unit types and stations, crew actions, inventory, optics with reticles and range setting, top-down/third-person/optic cameras, controller/gyro + aim assist; full RTS UI; presenter quality pass (LOD, impostors, audio). Gunner optic at maximum zoom over a Battalion late-game scene stays inside the Deck preset (≤ 1,500 draws, 2 M triangles, GPU ≤ 60 % busy). |
| M6 | AI opponents | Commander + squad BTs, difficulty defs, `batch` balance runs. Re-check the Deck/desktop tier caps with the new costs; final tier caps set. |
| M7 | Multiplayer at scale | Relay-closed-turn lockstep for 10 players, custom lobbies + server browser + host-run relays (in-client and free standalone relay binary, §7.9), SCS + device table, governor and `SetController`, JIP from peer snapshot, co-op PvE lobbies. Direct-control feel re-checked in 10-player matches at real D, with re-stamp stats and replay audit. Mod distribution (§7.8): Steam Workshop + mod.io adapters, `frontline-cli publish` + id registry service, lobby mod sets with auto-download and match profiles. Lawyer-reviewed legal texts live before the first external publish; mod-terms prompt, Workshop agreement handling and required `ai_assisted` in `publish` (§7.8, §7.9). Apply for Steam Deck Verified afterwards. |
| M8 | Content & editor | Map editor, mission set (East + West, solo + co-op), full 3-faction 1944–45 roster, public modding docs. Public SDK release: `frontline-sdk` repo (MIT OR Apache-2.0) + prebuilt `sim-ffi`/`sim-editor-gdext` for Windows/Linux/macOS (§7.9). |
| Post-launch | Ranked & campaign | Official relays, matchmaking and ratings (Battalion only), abandon/demotion rules, cosmetic-mod allowlist on official relays (§7.8); persistent campaign (§2.5). |
| Later | Era packs | `coldwar` / `modern` mods on the same engine: guided munitions, thermal sensors, ERA, `air_low` layer (§2.4). |

---

## 16. Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Determinism breaks (floats, iteration order, mods) | MP unplayable, replays useless | Fixed-point, banned APIs lint, hash checks in CI, `deterministic` flag enforced for MP mods. |
| Ballistics cost explodes with many projectiles | Frame drops in big fights | Projectile LOD, SoA layout, BVH, hard per-tick budgets with deferred resolution. |
| Deformation cost (nav re-bake, terrain texture uploads) under heavy artillery | Hitches, stale paths | Budgeted work-count queues (§2.2), pessimistic immediate nav invalidation, GPU height-texture uploads capped per frame, sparse detail tiles, M2.5 spike before committing. |
| Heightfield can't express tunnels/overhangs | Some fortifications impossible | Overhead cover via structures over dug pits; sparse voxel islands as a v2 option. |
| Players dig everywhere → memory growth | OOM on long matches | Detail-tile pool cap + demotion of re-flattened tiles + delta storage. |
| Godot node/draw-call overhead with thousands of units | Frame drops | Integration rules §4.1: MultiMesh + `RenderingServer`, Rust-side culling, bone-matrix animation textures, one batched transfer per frame; validated in the M0 spike. |
| Godot or godot-rust breaking changes | Rework | Pinned versions + upgrade policy (§4.2); sim crate has no Godot dependency. |
| Mod API lock-in | Can't evolve without breaking mods | `api_version`, deprecation windows, migration tooling, generated docs. |
| Scope creep (campaign, editor, MP all at once) | Never ships | Milestones gate each feature; headless sim is useful on its own from M0. |
| Malicious mods | Security issues for players | Luau/WASM sandboxes, no FS/network access, no native code, script-free allowlist parsing of mod Godot resources, fuzzed loaders, channel takedowns honoured (§7.8). |
| Mod version drift across channels | Players can't join modded lobbies | Package-hash identity, one `publish` path to both channels, Workshop item versions per game branch + newest-version rule for hosts, mod.io fallback when a Workshop item changes mid-lobby, mod.io file pinning (§7.8). |
| Saves/replays lose their mods (Workshop newest-only, takedowns, cache eviction) | Old saves and replays won't load | Mod set records with package hashes and locators, save-referenced mods pinned in the cache, `replay export --with-mods` (§7.8, §11). |
| mod.io cost/terms, or Steam-community pushback on a second mod channel | Channel dropped or delayed | Confirm mod.io tier and console terms in writing before M7 (§17 Q14); mod.io stays optional on Steam (Workshop is the default there); `ModSource` adapters can be removed without touching the loader (§7.8). |
| Mod id registry outage | Modders can't publish | Stateless service; `publish` fails closed; loading and playing never need it (§7.8). |
| Licence breach in shipped or mod content (mislabelled CC0, NoAI ignored, ripped assets, missing CC-BY credits) | Takedowns, store removal, legal claims | §12.1 intake gate, manifests and banned-licence validator; `mod_redistributable` copy rules; credits validator; a takedown on any channel blocks the package hash everywhere (§7.8, §7.9). |
| Open SDK / closed sim boundary leaks or is legally unclear | Closed code exposed, or SDK licence disputes | Separate public repo built without closed sources; C-ABI prebuilt library under its own licence; audit heuristics kept closed; lawyer review (§7.9, §13). |
| Legal texts late, or mod rules dodged by uploading outside the CLI | Unenforceable mod rules, launch delay | Drafts at M3, review before the first external SDK/asset release; rules in EULA + Workshop/mod.io game terms, not only in `publish` (§7.9, §15). |
| AI use of voice/likeness or commissioned art beyond contract | Performer/artist claims | AI exclusion for voice, mocap and likeness; AI clause in commission contracts (§7.9, §12.1). |
| Art/asset volume | Content bottleneck | Modular kits, glTF pipeline, community-friendly tooling early (M3). |
| Deck sim cost above estimate (±40 %, §2.2) | Deck tier shrinks | M0 real-hardware gates; caps are data; re-check at M2.5/M4/M6. |
| APU power split starves the CPU | Deck sim slows | Deck preset keeps GPU ≤ 60 % busy at 30 fps; M0 clock gate. |
| Capped spotting feels sluggish | Gameplay feel | Invariant cadence + priority lanes + exposure cap. |
| Proton/libm float divergence | Desync | Deterministic Luau math, NaN-canonical WASM, Proton in CI matrix. |
| Lockstep input delay makes direct control feel laggy online (≈ 150–220 ms at D = 3–5) | Core feature disappoints in PvP | Exact prediction of the controlled unit's movement and aim from the sim's own code (§5.12.4), per-lobby D cap (§10), blind M1 feel gate with decided fallbacks (`fire_rewind`, D cap 3; §15). |
| Maphack / aimbot (lockstep clients hold full state) | Unfair PvP, empty lobbies | Rust-side spotting filter before render/audio/UI, sim-side aim limits and input validation (§5.12.3), unpredictable RNG (§5.11), replay audit, official relays for ranked (§10). |
| Player-base fragmentation | Empty lobbies | Custom lobbies + server browser at launch; ranked later and only at Battalion; Regiment as custom/events; co-op PvE keeps players engaged when PvP lobbies are thin. |

---

## 17. Open Questions

1. ~~Engine path~~ **Decided 2026-10-31:** Rust sim core + Godot 4.7.2 presenter via `godot` 0.5.5 (§4).
2. ~~Priority order~~ **Decided 2026-10-31:** multiplayer first, with PvP and co-op PvE as launch priorities; PvP starts with custom lobbies, ranked later; determinism enforced from M0 (§2.5).
3. ~~Setting & scope~~ **Decided:** Jun 1944 – early 1945, Eastern + Western Front, Germany / USSR / USA, authentic; engine kept era-agnostic for future Cold War / modern packs (§2.3, §2.4).
4. ~~Scale target~~ **Decided 2026-10-31:** two lobby tiers, Battalion (700/100, Steam Deck minimum) and Regiment (2,000/300, 6-core desktop), 10 players max (§2.2). Caps are estimates until M0; final caps at M6.
5. **[OPEN]** Persistent campaign design (strategic map, unit roster carried between battles). Timing decided: post-launch; launch ships a mission set (§2.5).
6. ~~Mod distribution~~ **Decided 2026-10-31:** Steam Workshop + mod.io + plain folders from launch; auto-download on lobby join; any mods in custom lobbies, vanilla + approved cosmetic mods in ranked (§7.8).
7. ~~Art direction~~ **Decided 2026-10-31:** palettes, readability aids, budgets, materials and asset intake (§12.1). The visual style (grounded realism vs stylized realism) is chosen by the blind A/B style test at M2 exit (§12.1, §15).
8. ~~Fortification depth~~ **Decided:** dugouts only in v1; tunnels possibly v2 (§5.10.9).
9. ~~Construction pacing~~ **Decided:** compressed realism, e.g. 20 m trench ≈ 3–4 min per engineer squad; one global `construction_time_scale` (§5.10.9).
10. ~~Build placement~~ **Decided:** anywhere a unit can stand, with mode-defined limits; tiered builders (§5.10.9).
11. ~~Licensing~~ **Decided 2026-10-31:** base assets free to use/modify/redistribute in mods for this game only; modders own their mods, mods must be free; proprietary game with open-source (MIT OR Apache-2.0) SDK; AI-made mods welcome with an `ai_assisted` tag (§7.9). Legal texts drafted at M3; lawyer review before the first external SDK/asset release, no later than M7 (§7.9, §15).
12. ~~8 GB PCs~~ **Decided 2026-10-31:** supported at a Low preset (≤ 4 GB total, Battalion only, maps ≤ 2×2 km), verified at M0 (§2.2).
13. ~~PvP direct-control fallback~~ **Decided 2026-10-31:** if the PvP direct-control feel gate fails at M1, make `fire_rewind` the PvP default if it passes, otherwise cap PvP lobbies at D = 3 (§5.12.3, §15).
14. **[OPEN]** mod.io commercial terms: the tier needed for launch (PC) and later for consoles; mod.io publishes no prices. Confirm with mod.io before M7 (§7.8).
15. ~~SDK boundary~~ **Decided 2026-10-31:** the whole `content` crate is open; the public `frontline-sdk` repo accepts outside contributions under DCO sign-off (inbound = outbound, MIT OR Apache-2.0); the relay ships as a free standalone binary with a documented protocol, source closed for now (§7.9, §10, §13).

---

*Next step suggestion: turn §15 M0–M1 into a concrete task breakdown and run the M0 spike to validate the §2.2 tier caps and budgets (the stack is decided, §4).*
