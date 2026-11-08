# Data sourcing log (branch `data-sourcing`)

Web pass over "verify" marks. Rule: a value counts only with a primary or good secondary source, or two independent
general sources. "conf" = confirmed (comment updated), "open" = unresolved (keeps verify), "prop" = proposed by the
research, not accepted for this apply stage. Quality: P primary, S good secondary, G general web.

## Applied

| Figure | Old | New | Source | Q |
|---|---|---|---|---|
| usa_cckw `cap_kg_total` | 2500 | 2270 (2.5 short tons, off-road rating) | en.wikipedia.org/wiki/GMC_CCKW_2%C2%BD-ton_6%C3%976_truck; warhistory.org US Army trucks | G x2 |

Version bumps: none. A data value change needs no SAVE_VERSION or PROTOCOL_VERSION bump. Canonical snapshot
`ww2_canonical.json` blessed (one line). `simbench` BASELINE is a hardware note, not data; unchanged.

## Group 1: StuG III G, M10, 3-inch ammo, skirmish rules

| Figure | Old | Result | Source | Q |
|---|---|---|---|---|
| StuG hull rear | 50 mm / 10 deg | conf (angle from one database) | wwiitanks.co.uk/FORM-Tank_Data.php?I=150 | G |
| StuG casemate rear angle | 30 deg | open (database says 0) | same | G |
| StuG track band, traverse 6, elevate 4, split 27/24/3 | - | open | none | - |
| StuG stowage total | 54 | conf | wiki.warthunder.com/unit/germ_stug_III_ausf_G | G |
| M10 hull rear 19 at 38, roof | 19 | conf (engine deck roof is 9.5) | afvdatabase.com/usa/3ingmcm10.html | G x2 |
| M10 turret rear | 25 at 0 | conf | same | G x2 |
| M10 turret side slope | 20 | prop 15 (one source) | same | G |
| M10 traverse | 5 deg/s | prop 4.5 (80 s per turn) | en.wikipedia.org/wiki/M10_tank_destroyer | G |
| M10 elevate rate, rack split | - | open (total 54 conf) | none | - |
| M42A1 round mass | 10900 g | prop 11300 (hobbyist sheet) | theshermantank.com 3-inch M7 spec sheet | G |
| M42A1 filler 0.86 lb, body 12.9 lb | same | conf | en.wikipedia.org/wiki/76_mm_gun_M1 | G |
| M42A1 `frag_pen_mm` 12 (S8a-13) | 12 | open | none | - |
| M62 round mass 12400 g, MV 792 | same | conf | theshermantank.com sheet; tankarchives.com/2017/02/76-mm-m7-gun-trials.html | G |
| M62 penetration row (S7-6) | 118..92 | open (only the 1000 yd input supported) | en.wikipedia.org/wiki/M10_tank_destroyer | G |
| `face_hardened_pct` 125 (S8a-4) | 125 | open (reports 1.1 to 1.5) | community.battlefront.com/topic/38504 | G |

## Group 2: Sd.Kfz. 251/1, M3A1, trucks, S8a-14

| Figure | Old | Result | Source | Q |
|---|---|---|---|---|
| 251 width/height 2.10/1.75 | same | conf (German Wikipedia differs) | en.wikipedia.org/wiki/Sd.Kfz._251 | G x2 |
| 251 plates 14.5/8/8 | same | conf | same; weaponsystems.net/system/81-Sd.Kfz.%20251/22 | G x2 |
| 251 length 5.98, engine deck 5.5, slopes, MG arc, MG rounds, shield, boarding times | - | open | weaponsystems.net (single site) or none | G/- |
| 251 cross-country 750 cm/s | 750 | open (road 52.5 km/h conf) | same | G |
| M3A1 length/width | 6.18/2.22 | conf | en.wikipedia.org/wiki/M3_half-track | G |
| M3A1 armour 12.7/6.4 | same | conf | afvdatabase.com/usa/halftrackm3.html | G x2 |
| M3A1 .50 rounds 700 | 700 | conf | wiki.lonesentry.com/manuals/tm-9-2800-military-vehicles/m3a1-halftrack.html | P |
| M3A1 road 72 km/h | same | conf; cross-country 850 open | same | P |
| M3A1 height, floor, radiator front, windshield slope, ring rate | - | open (radiator 6.4 and slope 25 are single-source) | afvdatabase.com | G |
| Blitz payload 3000 | same | conf (rating only) | fiddlersgreen.net/models/vehicles/Opel-Blitz-Truck | G |
| Blitz and CCKW turn radius, cross-country speed | - | open | none | - |
| CCKW payload | 2500 | applied, see above | see above | G x2 |
| S8a-14 speeds 750/850 | same | open, road speeds conf | see above | P/G |

## Group 3: Sd.Kfz. 234/1, M8, 2 cm, 37 mm

| Figure | Old | Result | Source | Q |
|---|---|---|---|---|
| 234/1 gun elevation | [-4, 60] | REVERTED, see below | quartermastersection.com/german/afvs/417/SdKfz2348Rad | G x2 |
| 234/1 top plate 5 mm; MG 42 rounds 2400 | same | conf | wwiitanks.co.uk/FORM-Tank_Data.php?I=281; historyofwar.org | G x2 |
| 234/1 turret side/rear | 10 at 25 | prop 8 at 40 / 8 at 38 (sources split, 14.5 elsewhere) | same | G |
| 234/1 rare_cap, turn, rates, 2 cm split | - | open | none | - |
| M8 turn radius 850, elevation, stowage 80, coax 1500, .50 400 | same | conf | afvdatabase.com/usa/lacm8.html | S |
| M8 turret slopes | 15/15/15 | prop 15/18/17 (FORM says 0) | same | S |
| M8 reverse, hand rates, AP/HE split | - | open | none | - |
| 2 cm KwK 38 rate 120 rpm, M6 24 rpm | - | open | wwiitanks.co.uk/FORM-Gun_Data.php?I=73 | G |
| PzGr 39 MV 780, Sprgr MV 830 | - | open (FORM 899 and 995) | same | G |
| PzGr 40 MV 1050; Sprgr filler | same | conf | en.wikipedia.org/wiki/2_cm_KwK_30; FORM | G |
| 2 cm penetration rows, round masses | - | open (sources differ up to 50 %) | en.wikipedia.org/wiki/20%C3%97138mmB | G |
| 37 mm M51 MV 884, pen 61/53/46 | same | conf | wwiitanks.co.uk/FORM-Gun_Data.php?I=231 | G |
| 37 mm M63 filler 39 g, MV 792 | same | conf (one database) | same | G |
| 37 mm round masses 1590 g | - | open | none | - |

## Rejected or reverted

- 234/1 elevation +70: the sources agree, but the content validator accepts only [-30..=0, 1..=60] for
  `elevation_deg`, so the load fails. Reverted; the comment records the +70 finding. Raising the validator limit is an
  owner or code decision (it is a rules-schema change, outside this pass).
- Not applied by design (outside the accepted list): M10 side slope 15 and traverse 4.5, M42A1 mass 11300, 234/1
  turret 8 mm plates, M8 turret slopes. The research flagged each as single-source or conflicting.

## Counts

Applied 1, reverted 1, unresolved about 49 figures (keep verify), confirmed about 30 (comments updated only where a
"verify" comment sat on the exact figure; the rest of the confirmations are listed here only).

## Balance effect

The one value changed (CCKW `cap_kg_total` 2500 -> 2270) was measured on three 160 sets, seeds 1-80, 101-180, 201-280,
against the S8b tip: Germany 41.9 / 47.5 / 50.0 %, pooled 46.46 %, the same numbers. The AI loads a truck far below either
cap (about 535 kg), so the cap does not bind in AI matches. Full suite: 644 tests, Godot checks, 203 of 207 targets as S8b.
