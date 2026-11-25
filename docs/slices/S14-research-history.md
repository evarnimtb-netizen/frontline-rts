# S14 "assault kit": history and figures (working note)

Working note for `S14-assault-kit.md` (2026-11-22, plan agent). Values here are inputs; the plan decides. "(verify)" marks a
figure that two sources give differently or that only one secondary source gives; Build stage "data" confirms or keeps it.

## 1. Flammenwerfer 41 (Germany)

| Figure | Value | Source |
|---|---|---|
| Fuel | **11.8 L** Flammöl 19 (tar and petrol) | Wikipedia "Flammenwerfer 41"; dday-overlord.com FmW 41 page |
| | 1 1/2 US gal (about 5.7 L) | US *Intelligence Bulletin*, April 1944, "Portable Flame Throwers (German)" (lonesentry.com) (verify: early model or a misprint) |
| | 7 L | `docs/design/roster/infantry-structures.md` 3.1 excerpt (marked verify there) |
| Duration of fire | **about 10 s** in all | dday-overlord.com |
| Range | **32 m** effective (Wikipedia, dday-overlord); 25 yd maximum (US IB 1944) | |
| Weight | 28.7 kg loaded (dday-overlord, Wikipedia text); 47 lb charged / 32 lb empty (US IB 1944); 18 kg (Wikipedia infobox) (verify) | |
| Ignition | hydrogen torch and battery spark at first; failed in the winter of 1941-42; later a cartridge igniter (Strahlpatrone 41; about 10 ignitions a load, verify) | Wikipedia; US IB 1944 (hydrogen system described) |
| Production | 64,284, 1942 to April 1945 | Wikipedia |
| Who | engineers only; a pioneer platoon of about 60 men held 2 flamethrowers; attached down to two men for a raid | US IB 1944 |
| Use | static targets in enclosed spaces, most often pillboxes: wire cut first, then the team closes to effective range under smoke or MG / AT gun / tank cover and engages the embrasures; the effect is mainly psychological; carriers are easy targets once seen and their losses were high | US IB 1944 |

Game reading: 11.8 L over 10 s = 1.18 L/s; at 12 particles a second, 120 particles of about 0.1 L (`g = 100`, density about 0.9).
Range 32 m with the US bulletin's 25 yd as the low end: `range_m` 30. Ten ignitions: bursts of 12 particles (1 s).

## 2. US portable flamethrowers

| Figure | M1A1 | M2-2 | Source |
|---|---|---|---|
| Fuel | 4.7-5 US gal (17.8-18.9 L), thickened (napalm) | 4 US gal (14.8-15.1 L) in two tanks | Wikipedia "M1 flamethrower", "M2 flamethrower"; dday-overlord.com M2 |
| Rate | about 0.5 gal/s | 0.5 gal/s (1.85 L/s) | same |
| Duration | about 9 s (from fuel and rate) | about 7 s | same |
| Range | about 45 m thickened (text); 20 m effective / 43 m max (infobox) | 20 m effective, 40 m max | same |
| Weight | 29-32 kg | 30.8 kg loaded, 19.5 kg empty | same |
| Ignition | hydrogen igniter, battery spark | cartridge igniter (5 cartridges a load, from TM 3-376A memory: verify) | Wikipedia; TM 3-376A (Gutenberg #53669, not fetched: robots) |
| In service | from 1943; about 150 landed on D-Day, about 100 found abandoned on the shore | combat from 1943-44 (Pacific); "used through the end of the war, especially in Normandy" (dday-overlord, verify) | |

Finding: the roster gives the Assault Section (1944-06, West) an M2-2. The weapon the June 1944 assault teams landed with is the M1A1
by the sources above; the M2-2's ETO date is not confirmed. The two are near identical in game terms (fuel 15-18 L, 7-9 s, 20/40 m).
Plan 4.1 settles on the M1A1 for the 1944-06 card and keeps the M2-2 as a later `extends` variant (no card), unless the data stage
finds the M2-2 with US units in France from June 1944 (then the roster's M2-2).

Game reading (M1A1): 9 s x 12 = 108 particles of about 0.165 L (`g = 130`, density about 0.78); `range_m` 35; no ignition limit,
bursts of 12 (1 s) as the German.

## 3. Melee

- `infantry-structures.md` 4: trenches were cleared with grenades, SMGs and pistols along the trench; bayonets and sharpened spades
  when weapons were empty or jammed. Roster question 9 (A): a minimal physical strike, last, no knife squads.
- Strike reach from the design: bayonet thrust 1.8 m, rifle butt 1.0 m, spade or knife 0.6 m; wind-up 0.4-0.8 s, recovery 1 s.
- Both armies carried an entrenching tool per man (German Kleines Schanzzeug spade; US M1943 folding shovel); the German spade
  sharpened as a weapon is a common memoir detail (verify, not used for numbers).
- Bayonets: German S84/98 and US M1 / M1905 were issued but fixed on order in the West; Soviet sections kept them fixed (S15).

## 4. Trench clearing

- US and German doctrine alike (design 4.1): grenade over the traverse or into the bay, then the first man in with an SMG, the next
  covering; repeat bay by bay. The `ClearTrench` order (plan 6) follows that loop: throw ahead, wait for the burst, move in.

## 5. Not used, recorded

- Fuel tank hits: post-war accounts and modern tests agree that a bullet rarely ignites a pressurised tank (fuel leaks; a tracer may
  light it); a popular myth in films and games. Plan question S14-2 (A): no rupture rule.
- Flame against tanks: crews behind closed plates were rarely hurt; open-topped vehicles were vulnerable. Plan S14-6 (A): no vehicle
  effect in S14.

Sources: en.wikipedia.org/wiki/Flammenwerfer_41; en.wikipedia.org/wiki/M1_flamethrower; en.wikipedia.org/wiki/M2_flamethrower;
www.dday-overlord.com (FmW 41 and M2 pages); www.lonesentry.com/articles/portft/ (US *Intelligence Bulletin*, April 1944);
TM 3-376A *Portable Flame Thrower M2-2* (Project Gutenberg #53669, to read in the data stage).
