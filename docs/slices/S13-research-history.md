# S13 heavy wave: historical data (research only)

Working note for `S13-heavy-wave.md` (2026-11-19, research agent). Not a spec: values here are inputs; the plan decides.

Status legend: **(W)** = web-page value, source named. **(M)** = from my own memory of Jentz / Hunnicutt / Zaloga, NOT confirmed by a fetched page, treat as **(verify)**. "from vert." = slope from vertical (0 = vertical plate, 90 = horizontal).
Honest scope note: I could NOT obtain Bird & Livingston (B&L) tables online (book is not on the open web). Every penetration row below is therefore either (a) a Wikipedia/wwiitanks table that states its criterion, or (b) my interpolation, labelled. Rows marked "B&L-style 0 deg" are the closest open substitute: they are stated as "American and British 50 % success criteria" at 90 deg impact (= 0 deg obliquity, normal). Check against the book before shipping.

URLs used (short names used in tables):
- WP-KwK42 https://en.wikipedia.org/wiki/7.5_cm_KwK_42 ; WP-KwK36 https://en.wikipedia.org/wiki/8.8_cm_KwK_36 ; WP-76 https://en.wikipedia.org/wiki/76_mm_gun_M1 ; WP-90 https://en.wikipedia.org/wiki/90_mm_gun_M1/M2/M3 ; WP-75 https://en.wikipedia.org/wiki/75_mm_gun_M2/M3/M6
- WP-Panther https://en.wikipedia.org/wiki/Panther_tank ; WP-Tiger https://en.wikipedia.org/wiki/Tiger_I ; WP-M36 https://en.wikipedia.org/wiki/M36_tank_destroyer ; WP-M4 https://en.wikipedia.org/wiki/M4_Sherman
- WWT-174 https://wwiitanks.co.uk/FORM-Tank_Data.php?I=174 (Panther G) ; WWT-G148 .../FORM-Gun_Data.php?I=148 (KwK 42) ; WWT-G256 .../FORM-Gun_Data.php?I=256 (90 mm)
- AFVDB-M36 https://afvdatabase.com/usa/90mmgmcm36.html ; ONWAR-76W https://onwar.com/wwii/tanks/usa/us020m4a376w.html ; TE-Jumbo https://tanks-encyclopedia.com/ww2/us/m4a3e2-jumbo-assault-tank ; PW-Tiger https://panzerworld.com/pz-kpfw-tiger-ausf-e ; HOW-PantherG https://www.historyofwar.org/articles/weapons_panther_ausf_G.html
- Chieftain1/2 https://worldoftanks.com/en/news/history/chieftains-hatch-us-guns-vs-german-armour-part-1/ and .../us-guns-german-armor-part-2/ (quote Isigny reports, Jan 1945 Ordnance pamphlet)
- ARMET https://www.generalstaff.org/BBOW/BalExt/ARMET-TR-17002_76mm_Analysis.pdf (US Army ARDEC report on 76 mm vs Panther)
- LS-90 https://lonesentry.com/manuals/90-mm-ammunition (Jan 1945 Ordnance pamphlet on 90 mm ammo)
- LS-TM30-451 https://www.lonesentry.com/manuals/tme30/ch7sec3.html (TM-E 30-451, German mortars)
- 4P2 https://4point2.org/mortar42.htm ; WP-M2mortar https://en.wikipedia.org/wiki/M2_4.2-inch_mortar ; WP-GrW42 https://en.wikipedia.org/wiki/Granatwerfer_42
- LS-Tiger https://m.lonesentry.com/panzer/tiger-tank-normandy.html

---
## 1. Armour tables (mm @ slope from vertical)

### 1.1 Panther Ausf. G
| Face | Value | Source |
|---|---|---|
| Upper glacis (superstructure front plate) | 80 @ 55 deg (LOS ~139) | WWT-174 (W); WP-Panther (W); US Isigny measured 57 deg ground-adjusted (Chieftain1) |
| Lower nose plate | 60 @ 55 deg (LOS ~105) | WWT-174 (W). Chieftain1: US measured nose 65 mm at 55 deg (66.7 on one) |
| Superstructure side (upper) | 50 @ 29-30 deg (LOS ~58). Ausf. A/early D: 40 @ 40 deg (G thickened, slope reduced by 11 deg) | WWT-174 (W: 50 @ 30); HOW-PantherG (W: slope reduced 11 deg, 40->50) |
| Lower hull side | 40 @ 0 | WWT-174 (W) |
| Hull rear | 40 @ 30 deg upper; superstructure rear 50 @ 30 | WWT-174 (W) |
| Hull roof | 16 (over engine) ; 30-40 over crew / superstructure top 40 | WWT-174 (W: 40 sup. top, 30 hull top); HOW (W: 16 / 40) |
| Hull floor | 16-30 | HOW (W) |
| Turret front (cast) | 100 @ 11-12 deg (LOS ~102) | WWT-174 (W: 110 @ 11, page typo?) ; WP/HOW 100. Use 100 (verify) |
| Gun mantlet | Ausf. D/A: 100 mm cast, curved "Saukopf" with lower shot trap (deflects into 16-30 mm roof). G: flatter chin ("Kinnblende") mantlet, introduced from ~Sep/Nov 1944 only on part of production; old mantlet still built to the end | WP-Panther (W) |
| Turret side / rear | 45 @ 25 deg (LOS ~50) | WWT-174 (W) |
| Turret roof | 16 @ 84-90 deg | WWT-174 (W: 16 @ 87) |
| Schuerzen | 5 mm side skirts (not vs AP) | WP-Panther (W) |

Ausf. A vs G (brief): A turret/hull armour same thickness; A has the commander's cupola with vision slits (G drum cupola with periscopes, better), A hull side 40 @ 40 deg (G 50 @ 30, deeper sponsons), A has a hull MG ball mount in both. Mantlet curved in both A and most G. Production A 2,200 (Aug 43-Aug 44), G ~2,961 (Mar 44-Apr 45) WP-Panther (W).

### 1.2 Tiger I (late 1944 production)
| Face | Value | Source |
|---|---|---|
| Hull front | 100 mm (page gives no slope). Detail: nose plate 100 @ ~24 deg, upper front plate (glacis, under superstructure) 63 @ 80 deg, superstructure front 100 @ ~8-9 deg | WP-Tiger (W for 100); slopes (M, Jentz) **(verify)** |
| Hull side | lower 60 @ 0; upper/sponson 80 @ 0 | WP-Tiger (W: 60, 80), slope 0 (M) |
| Hull rear | 80 @ ~8 deg (82 in some tables) | (M) **(verify)** |
| Hull top / bottom | 25 / 25 (hull top 25; crew deck) | WP-Tiger (W) |
| Turret front | 100 mm @ ~8 deg (flat cast front plate) | WP-Tiger (W: 100) ; PW-Tiger 70-145 (page lists the spread; verify) |
| Mantlet | 110 at edges, up to 200 around gun (cylindrical, rotating cast) | WP-Tiger (W) |
| Turret side / rear | 80 @ 0 (rear 80) | WP-Tiger (W) |
| Turret roof | 25, thickened to 40 from Mar 1944 | WP-Tiger (W) |
Late production (from Feb 1944) Tiger E uses steel-roadwheel and the Tiger I(E) "Kommandokuppel" cupola; both unchanged in armour.

### 1.3 M4A3(76)W (HVSS later called E8)
| Face | Value | Source |
|---|---|---|
| Upper glacis | 63.5 mm (2.5 in) @ 47 deg (LOS ~93) on the late "large hatch" welded hull; earlier hull 50.8 mm @ 56 deg. Cast-hull variants: 64 mm @ 56. The M4A3 (all-welded hull) uses 63.5 @ 47 plate | WP-M4 (W) |
| Lower nose / transmission | 50.8-108 (cast final-drive cover 108) @ 0-56 | ONWAR-76W (W: "front hull 50-108 @ 34-90"; its angles are measured from horizontal in the table, i.e. 90 = vertical, confirmed by sides 38 @ 90) |
| Hull side | 38 mm vertical (sponson 76 mm on the superstructure/sponson 38 hull plus 25 ... not verified) | WP-M4 (W) 38 @ 0 |
| Hull rear | 38 @ 0-22 deg | WP-M4 (W) |
| Hull roof / floor | 25.4 roof; floor 25.4 front -> 12.7 rear | WP-M4 (W) |
| Turret (T23 cast, "T23-type") front gun shield | 88.9 mm (3.5 in) cast mantlet, effectively ~ 0 deg | WP-M4 (W: 89 @ 90 in ONWAR convention = vertical) |
| Turret front (casting) | 63.5 @ ~45 deg (turret face) | ONWAR-76W (W) |
| Turret side/rear | 63.5 @ 0-13 deg | WP-M4 (W) |
| Turret roof | 25.4 | WP-M4 (W) |
Note: ONWAR table lists turret "front 63 @ 45-50" in horizontal-angle convention, so approx 40-45 deg from vertical, I treat that as (verify).

### 1.4 M36 Jackson (M36 on M10A1 hull; the 90 mm GMC)
| Face | Value | Source |
|---|---|---|
| Upper glacis | 38 mm (1.5 in) @ 55 deg (LOS 66) | AFVDB-M36 (W); WP-M36 (W) |
| Lower nose | 51 mm (2 in) @ 0-56 deg (curved casting/plate) | AFVDB-M36 (W) |
| Upper sides | 19 mm @ 38 deg | AFVDB-M36 (W) |
| Lower sides | 25 mm @ 0 | AFVDB-M36 (W) |
| Rear | 19 @ 38 (upper), 25 @ 0 (lower) | AFVDB-M36 (W) |
| Hull top | 19 mm (front), 9.5 mm (rear) | AFVDB-M36 (W) |
| Turret front (gun shield) | 76 mm (3 in) cast, @ 0 | AFVDB-M36 (W) |
| Turret side | 32 mm (1.25 in) @ 5 deg | AFVDB-M36 (W) |
| Turret rear | 44-127 mm; the counterweight bustle rear 127 mm (5 in) | AFVDB-M36 (W) |
| Turret roof | OPEN TOP (only 9.5-25 mm partial plates; canvas cover). Vulnerable to HE airburst / mortars / infantry | AFVDB (W), WP-M36 (W) |
| M36B1 (Sherman hull) | glacis 64 @ 47 deg etc. (different; only 187 built, used Feb 1945+) | AFVDB-M36 (W) |

### 1.5 M4A3E2 "Jumbo"
| Face | Value | Source |
|---|---|---|
| Upper glacis | 101.6 mm (4 in) @ 47 deg (appliqué: 64 base plate + 38 applied, i.e. cast-in-place armour weld) (LOS ~149) | TE-Jumbo (W: 4 in; slope not stated). 47 deg from M4A3 hull: (M) **(verify)** |
| Lower nose / final drive cover | 101-139 mm (4 to 5.5 in) | TE-Jumbo (W) |
| Hull side | 76 mm (3 in) upper (the thicker plate applied to sponsons) | TE-Jumbo (W); slope 0 (M) |
| Hull rear / top / bottom | unchanged M4A3: rear 38 @ 0-22; top 25; floor 13-25 | TE-Jumbo (W) |
| Turret (T23-like new casting) | ~152 mm (6 in) all around; rear below bulge ~63 mm | TE-Jumbo (W) |
| Mantlet | ~177 mm (7 in: 2 in cast + 5 in added) | TE-Jumbo (W). The commonly quoted "178 mm" |
Slopes of turret faces were not in any fetched page **(verify)**. Weak points: sides (76 mm) and rear, final-drive cover (TE-Jumbo W).

---
## 2. Armament, ammunition, stowage

### 2.1 Gun and ammo data
| Gun / round | Projectile kg | Complete round kg | MV m/s | HE filler | Source |
|---|---|---|---|---|---|
| 7.5 cm KwK 42 L/70 PzGr 39/42 (APCBC-HE) | 6.8-7.2 (disagree: WP lists 7.2 in table, 6.8 in a list and WWT-G148 6.8) | 14.3 (WP) | 935 | 18 g RDX (phlegmatized) | WP-KwK42, WWT-G148 |
| PzGr 40/42 (APCR, tungsten core) | 4.75-4.8 | 11.55 (WP) | 1,120-1,130 | none | WP-KwK42, WWT-G148 |
| SprGr 42 (HE) | n/a (~6.4 kg **(verify)**) | n/a | 700 | 0.65 kg explosive | WP-KwK42 (W) |
| 8.8 cm KwK 36 L/56 PzGr 39 (APCBC-HE) | 10.2 | ~15.3 (M) (verify) | 773-780 (WP gives both) | 0.059 kg (59 g) | WP-KwK36 (W; cites H.Dv. 481/60) |
| PzGr 40 (APCR) | 7.3 | ~ (verify) | 930 | none | WP-KwK36 (W) |
| Hl.Gr 39 (HEAT) | 7.65 | | 600 | | WP-KwK36 (W) |
| SprGr (HE) | ~9.4 (M) | | ~ 820 (M) | ~0.87 kg (M) | **(verify)** Not on a fetched page |
| 76 mm M62A1 APC-T (APCBC) | 7.00 (15.43 lb) | 11.14 (24.55 lb) | 790 (2,600 ft/s) | 0.065 kg (0.144 lb) Expl D | WP-76 (W) |
| 76 mm M93 (T4) HVAP-T | 3.4 (7.6 lb), core 1.8 kg tungsten | ~ 8.4 (M) (verify) | 1,000 (3,400 ft/s) | none | WP-76 (W) |
| 76 mm M42A1 HE | 5.84 (12.87 lb) | 10.03 (22.11 lb) | 820 (2,700 ft/s) | 0.39 kg (0.86 lb) TNT or TNT/amatol | WP-76 (W; cites TM 9-1901) |
| 90 mm M82 APC-T (APCBC) | 10.94 (24.11 lb) | 19.39 (42.75 lb) | 808-810 (2,650-2,670 ft/s) per WP-90; LS-90 pamphlet gives 2,800 ft/s (853 m/s) | 0.14 kg Expl D (WP-90) | WP-90, LS-90 |
| 90 mm M77 AP | 10.61 (23.40 lb) | 19.07 (42.04 lb) | 820 (2,700) | none | WP-90 (W) |
| 90 mm T33 AP (M77 reheat-treated + windshield) | 24.06 lb (10.9) | | 853 (2,800 ft/s) | none | LS-90 (W) |
| 90 mm T30E16 (M304) HVAP | 16.70 lb shot (7.57 kg) | ~ 13.8 (M) (verify) | 1,020 (3,350 ft/s) | none; tungsten carbide | LS-90, WP-90 |
| 90 mm M71 HE | 10.56 (23.29 lb) | 19.02 (41.93 lb) | 820 | ~0.9 kg TNT (M) **(verify)** | WP-90 (weights only) |
| 75 mm M48 HE (for comparison) | 6.76 kg (14.9 lb) | ~ 9.9 (M) | 594 (M3 supercharge) / 450 standard | 0.68 kg (1.5 lb) TNT | WP-75 (W) |
| 75 mm M61 APC (for comparison) | ~6.3 | | 619 | | WP-75: pen 81 mm at 0 deg @ 500 yd (W) |

### 2.2 Ammo stowage, MG, mounts
| Vehicle | Main gun rounds | MG rounds and mounts | Source |
|---|---|---|---|
| Panther G | 82 (24+24 turret sides, 27 hull [18 fwd], 4 sponson, 3 under turntable). Ausf. A/D: 79 | 4,800-5,100 rounds 7.92 mm; hull ball MG34 (bow, radio operator) + coax MG34; AA MG mount optional | WP-Panther (W: 82/79, 5,100); panzerworld (W: 82, 4,800) |
| Tiger I | 92 (PW-Tiger W). Typical split in handbooks ~ 2/3 PzGr 39 : 1/3 SprGr, a few PzGr 40 **(verify)** | 4,500 rounds (4,800 Ausf. E); coax MG34 + bow MG34 | PW-Tiger (W), WP-Tiger (W: 4,500) |
| M4A3(76)W | 71 (wet racks in floor/ sponsons). Split TM: ~ 60 % APC/HE mix; no figure found | 6,250 .30 (coax M1919A4 + bow) + 600 .50 (M2HB roof) | ONWAR-76W (W); WP-M4 |
| M36 | 47 (11 ready in turret, 36 sponson stowage; M36B1 same). Split doctrine: ~ 6-10 AP, rest HE; HVAP only 1-5 per vehicle when available **(verify)** | 1,000 .50 cal M2HB (open turret ring mount, flexible AA) + .30 bow only on M36B1 (2,000) | WP-M36 (W), AFVDB-M36 (W) |
| M4A3E2 | 104 x 75 mm (floor racks, ~100) in the standard 75 mm build; ~100 vehicles were re-gunned with the 76 mm in Feb 1945 (and 45 spare 76 guns used in 3rd Army); then ~71 rounds | 600 .50, 6,250 .30, 900 .45 (SMG), grenades | TE-Jumbo (W); tankarchives (W) |
IMPORTANT for the game: the Jumbo as delivered Sep-Nov 1944 was a 75 mm M3 tank (HE/APC M61 only), not a 76 mm.

### 2.3 Rare-round supply (US HVAP; German PzGr 40)
- US 76 mm HVAP: first 2,000 rounds air-delivered Sep 1944, distributed 11 Sep 1944; of 18,000 rounds received, over half were incompatible with the 76 mm M1 (WP-M4 W). Initial order 20,000; the target 43,000/month was abandoned; 10,000/month not reached until Nov 1944; "enough for about 2 or 3 rounds per tank in 1945" (Chieftain2 W). TDs got priority.
- US 90 mm HVAP (T30E16): fielded Jan-Feb 1945 in tiny numbers (a few rounds per M36; Chieftain2 mentions Jan 1945 pamphlets).
- German PzGr 40 (tungsten): scarce from 1943; practically unavailable to panzer units in 1944-45; tungsten stocks stopped in mid 1944. No count found in fetched pages **(verify)**; typical game convention 0-5 rounds per vehicle.

---
## 3. Penetration data

Criteria: WP "30 deg" tables = German-style tables, plate inclined 30 deg from vertical (i.e. 30 deg obliquity), source not named on page (probably Jentz / H.Dv.481). WP "90 deg" tables = "American and British 50 % success criteria" (page's own calculation, 0 deg obliquity). WWT = "calculation that seems to fit actual test results".

### 3.1 Normal-incidence (0 deg obliquity) rows: B&L-style substitutes
Ranges in metres. Bold = direct from table; italic = interpolated by me (linear between tabulated ranges, convexity ignored so +-5 mm).
| Round | 100 | 500 | 1000 | 1500 | 2000 | Source / criterion |
|---|---|---|---|---|---|---|
| 7.5 cm PzGr 39/42 | 185 | 168 | 149 | 132 | 116 | WP-KwK42 "90 deg, US/UK 50 %" |
| 7.5 cm PzGr 39/42 (alt.) | 200 | ~182 | ~164 | ~146 | 126 | WWT-G148 0 deg calc. (higher; German-style) |
| 7.5 cm PzGr 40/42 | 265 | 234 | 199 | 170 | 145 | WP-KwK42 "90 deg, US/UK 50 %" |
| 7.5 cm PzGr 40/42 (alt.) | 247 | ~225 | ~206 | ~189 | 168 | WWT-G148 0 deg calc |
| 8.8 cm PzGr 39 | 162 | 151 | 138 | 126 | 116 | WP-KwK36 "90 deg, US/UK 50 %" |
| 8.8 cm PzGr 40 | 219 | 200 | 179 | 160 | 143 | WP-KwK36 same |
| 76 mm M62 APC (RHA) | 125 | ~116 | 106 | ~98 | 89 | WP-76 "90 deg" calculated (FHA row: 124/ /116/ /101) |
| 76 mm M93 HVAP | 239 | ~208 | 175 | ~148 | 124 | WP-76 calculated |
| 90 mm M82 APC (RHA) | 164 | ~152 | 137 | ~126 | 114 | WP-90 calculated "50 % criteria" (FHA: 151/ /140/ /123) |
| 90 mm M77 AP (RHA) | 188 | ~165 | 137 | ~116 | 96 | WP-90 calculated |
| 90 mm T30E16 HVAP | 306 | ~279 | 246 | ~219 | 193 | WP-90 calculated |
| (90 mm AP-alt) | 157 | ~144 | 134(800)/123(1200) | 112(1600) | 103 | WWT-G256 0 deg; labelled "AP, 11 kg, 853 m/s" **(verify which round)** |

### 3.2 Table at 30 deg obliquity (use these if the game wants a national 30 deg table and converts)
| Round | 100 | 500 | 1000 | 1500 | 2000 | Source |
|---|---|---|---|---|---|---|
| 7.5 cm PzGr 39/42 | 138 | 124 | 112 | 99 | 89 | WP-KwK42 (30 deg) |
| 7.5 cm PzGr 39/42 (WWT) | 173 | ~154 | ~138 | ~122 | 108 | WWT-G148 (calc; higher, "141 mm @400 m quoted") |
| 7.5 cm PzGr 40/42 | 194 | 174 | 149 | 127 | 106 | WP-KwK42 |
| 7.5 cm PzGr 40/42 (WWT) | 213 | ~194 | ~178 | ~162 | 145 | WWT-G148 |
| 8.8 cm PzGr 39 | 132 | 110 | 99 | 91 | 83 | WP-KwK36 (30 deg) |
| 8.8 cm PzGr 40 | 171 | 156 | 138 | 123 | 110 | WP-KwK36 |
| 76 mm M62 APC | n/a | 98 (500 m)/93-94 (500 yd) | 90 (1000 m)/88-89 (1000 yd) | 81-82 (1500 yd) | 75-76 (2000 yd) | WP-76 (yards and metres rows) |
| 76 mm M93 HVAP | n/a | 150-158 | 132-135 | 116-117 (yd) | 98-99 (yd) | WP-76 |
| 90 mm AP-alt | 136 | ~128 | ~118 | ~102 | 89 | WWT-G256 30 deg |
Ratio 0 deg / 30 deg is NOT constant between sources: WP-KwK42 1.34 (185/138) vs WWT 1.16 (200/173). A conversion "pen_0 = pen_30 x 1.15" matches WWT; "x 1.34" matches the Wikipedia pair. If you must convert, pick the same source for both rows. For M62 and M93: 0/30 ratio from WP-76 is 106/90 = 1.18 (APC) and 175/132 = 1.33 (HVAP) at 1000 m, so ratio varies with projectile (HVAP loses less to... more to obliquity; APCBC loses less).

### 3.3 Sloped-plate effects and trial data
- **Overmatch (calibre / plate)**: US APC M62 is 76 mm vs 80 mm Panther glacis (ratio ~0.95): below the overmatch threshold of about 1.0-1.2, so the round does not normalise; M82 at 90 mm vs 80 mm = 1.1 and 8.8 cm vs 80 mm = 1.1 normalise better; 7.5 cm L/70 (75 mm) vs 63.5 mm Sherman = 1.18, vs 76 mm Jumbo sides OK. Rules of thumb, not figures from a source: (verify).
- **Shatter gap**: uncapped/capped US AP, hardness issue: Panther plate was measured 262-269 BHN in Jul 1944 (ARMET W); that is moderately hard, but properly tempered plate was tougher; US M62 APC shattering at velocities near the ballistic limit on 80 mm @ 55 deg; M93 HVAP ricochets and breaks up on sloped plate (Chieftain2 W: "T4 ricocheted more than other projectiles on sloped plate").
- **Panther plate quality**: ARMET (W): Jan 1945 metallurgical analysis of captured Panther front plates found large variation; improper temper made some plates shatter on impact; molybdenum shortage cited. Chieftain1 (W): of three Panthers tested 20-21 Aug 1944, one had "best plate" (30 hits 600-200 yd without cracking), two "average plate" cracked after relatively few hits.
- **Isigny Test No. 2 (First Army board, 12-30 Jul 1944)** Chieftain1 (W): 75 mm M3 APC fails front at 200 yd, penetrates sides/rear at 1,500 yd. 3-inch (76 mm) M62: fails glacis at 200 yd, penetrates mantlet at 200 yd, sides and rear at 1,500 yd. 90 mm M77 AP: penetrates glacis at 600 yd, mantlet at 1,000 yd, turret at 1,500 yd. 90 mm M71 HE did not penetrate (welds cracked ~12 in). 105 mm HEAT M67 penetrates glacis and mantlet at 500 yd. 75 mm M66 HEAT fails glacis at 500 yd.
- **12th Army Group tests 20-21 Aug 1944** (Chieftain1 W, 2 W): Panther glacis 85 mm @ 55 deg (US vertical-equivalent 187 mm on the US armour basis curve), nose 65 mm @ 55 deg (139 mm eq.). 76 mm M62 unsatisfactory vs heavy armour; 76 mm HVAP T4 made the lower nose vulnerable (one of four penetrated at 200 yd; one of two penetrated 67 of 80 mm at 400 yd; none at 600 yd) but glacis not reliably penetrated (ARMET W, Chieftain2 W); 17-pdr APDS the only reliable round. M62 could penetrate the mantlet at ~200 yd or less; the lower rounded mantlet shot trap target is "about 6 inches high" (Chieftain2 W).
- **ARMET (calculated)**: APC V50 on Panther front hull 3,291-3,536 fps vs M62 MV 2,600 fps, so cannot defeat at any range; front turret APC 1,895-2,932 fps (BAL66 2,808 fps) also not penetrating at any range; HVAP-T front hull V50 3,861-5,061 fps vs MV 3,400: cannot; HVAP front turret penetrates within ~543 yd (BAL66). APG Mar 1945 test: M93 ballistic limit on 3.25 in plate at 55 deg ~3,400 fps = MV: fail at point-blank.
- **90 mm vs Panther (Jan 1945 Ordnance pamphlet, Chieftain2 W, LS-90 W)**: HVAP penetrates all Panther and King Tiger plates; Panther glacis to 450 yd, King Tiger glacis to 100 yd; Panther mantlet and turret front to 800 yd. T33 AP defeats all Panther plates except mantlet; glacis up to 1,100 yd. M82 APC-T penetrates all Panther plates except glacis and gun mantlet (LS-90 W). 703rd TD Bn Dec 1944: 90 mm could not penetrate King Tiger front at any range with available ammo (WP-M36 W). M36 gun on Panther glacis at ~150 yd deflected "at certain angles" (WP-M36 W).
- **Normalisation**: 8.8 cm PzGr 39 at 100-500 m: 162-151 mm normal vs Sherman glacis 63.5 @ 47 deg (LOS 93): defeats at all ranges; vs Jumbo glacis (101.6 @ 47, LOS 149) borderline beyond ~700 m by the nominal 0-deg numbers, and sloped penalty means it needs closer (verify; no direct trial data found).

---
## 4. Mobility, crew, weight

| Item | Panther G | Tiger I (1944) | M4A3(76)W | M36 | M4A3E2 |
|---|---|---|---|---|---|
| Weight | 45.5 t (HOW W) / 44.8 (WP) | 54-57 t (WP W; PW W 57) | 33.65 t / 66,800-70,000 lb (ONWAR W) | 29 t (63,000 lb; WP W) M36B1 31 t, B2 30 t | 38.1 t (84,000 lb) TE-Jumbo W |
| Engine | Maybach HL230 P30, 700 PS (690 hp) | Maybach HL230 P45, 700 PS | Ford GAA V8 450 hp (500 hp in later) | Ford GAA 450 hp (M36B2 GM 6046 diesel 375 hp) | Ford GAA 500 hp @ 2,600 rpm (TE W; some say 450) |
| Max road speed | 46 km/h (HOW W); governor limits ~ 40 km/h practical | 45.4 km/h, governor ~38; march 30 (WP W) | 42 km/h (26 mph) | 42 km/h (26 mph) | 35 km/h (22 mph; final drive 3.36:1) |
| Cross-country | 20-25 km/h (panzerworld W) ~ 15-20 | 20-25 prescribed; ~10 typical (WP W) | 24-32 km/h (15-20 mph) | ~ 24 | ~ 20 (M) |
| Ground pressure | 0.88 kg/cm2 (panzerworld W; Ausf. D col.) ; Ausf. G ~0.9-0.93 (M) | 1.09 kg/cm2 (PW-Tiger W) | 11 psi = 0.77 kg/cm2 (ONWAR W) | 12.9 psi (0.906 kg/cm2) AFVDB W | 14.2 psi (1.0 kg/cm2) TE-Jumbo W |
| Crew | 5 | 5 | 5 | 5 (M36 AFVDB W) | 5 |
| L x W x H | 6.87 (hull) / 8.86 incl. gun x 3.27 (3.42 w/o skirts) x 2.99 (WP, panzerworld W) | 6.32 hull / 8.45 gun fwd x 3.56 (3.705 with wide tracks) x 3.00 (WP, PW W) | 7.54 incl. gun x 3.00 x 2.97 (ONWAR W) | 5.97 hull / 7.47 w gun x 3.05 x 3.28 (WP-M36 W) | 6.3 x 2.9 x 2.9 (TE W) |
| Range road | 200 km (HOW W) | 195 / 110 x-country (WP W) | ~160-190 (M) | 240 km (WP W; 192 US gal) | 160 km (TE W) |
Bogging / reliability notes: Panther final drives (double spur from mass production) failed more often than engines (Guderian Jun 1944, WP-Panther W); Tiger readiness on the Western Front 87 % (31 May 44), 98 % (14 Sep), 67 % (30 Sep), 88 % (31 Oct), 81 % (15 Nov), 45 % (30 Nov), 64 % (15 Dec), 36 % (15 Mar 45) (WP-Tiger W). Tiger weight 57 t with 0.47 m ground clearance, interleaved roadwheels freeze with mud/ice (M).

### Turret and gun handling
| Vehicle | Traverse | Elevation | Source |
|---|---|---|---|
| Panther G | Hydraulic from engine: low gear ~6 deg/s idle-independent per WP; high-speed gear up to 19 deg/s at 2,000 rpm, >36 deg/s at 3,000 rpm; hand traverse slow (~ 1 deg/handwheel-turn class; no number found) **(verify)** | -8 to +18 deg (M) **(verify)** | WP-Panther (W) |
| Tiger I | 6 deg/s low gear; up to 19 deg/s high at 2,000 rpm; >36 deg/s at 3,000 rpm; min 0.1 deg/s (WP-Tiger W; same wording as Panther page, treat as shared claim; Tiger commonly quoted 360 deg in ~60 s on idle, M) | -8 to +15 (M) | WP-Tiger (W) |
| M4A3(76)W | 360 deg in 15 s (24 deg/s) powered (ONWAR W); hand slower (~ 1/10) (M); no turret-slewing stabiliser (WP-M4 W vertical only) | -10 to +25 deg (ONWAR W) | ONWAR-76W |
| M36 | 360 deg manual and hydraulic; max 24 deg/s (AFVDB W; the M10 original used hydraulic 360 deg/ ~ 17 s) ; hand-cranked secondary. Elevation +20 to -10 (M36), M36B1 +30/-10 | | AFVDB-M36 (W) |
| M4A3E2 | Powered traverse; extra weight makes it slower (e.g. ~ 20 s per 360 (M)) | -10 to +25 | (verify) |
Loader and rate of fire (no page gave numbers; from memory): 8.8 cm round 15.3 kg complete is the heaviest manual-loaded German tank round; practical 6-8 rpm; 7.5 L/70 14.3 kg round 6-8 rpm; 76 mm round 10 kg ~ 8-10 rpm; 90 mm round 19 kg ~ 6-8 rpm (loader had to lift 19 kg in a cramped open turret) **(verify all)**.

---
## 5. Production and ETO window
| Vehicle | Built | ETO first combat | Share / numbers | Source |
|---|---|---|---|---|
| Panther (all) | ~6,000; A 2,200 (Aug 43-Aug 44), G ~2,961 (Mar 44-Apr 45), D 842 | June 1944 (2 regiments, 156 tanks, at D-Day); up to 432 in France by 30 Jul 1944; ~38 % of German tanks in Normandy | WP-Panther (W) |
| Panther G in ETO | G reached the West from ~Aug 1944 (M); most Ardennes Panthers were A and G | Ardennes Dec 1944 | (M) |
| Tiger I | 1,347-1,355 (Aug 42-Aug 44); peak 104/month Apr 44; 1942: 79, 1943: 647, 1944: 623 | s.SS-Pz.Abt. 101 (45 Tiger I, Normandy, Villers-Bocage 13 Jun) , s.SS-Pz.Abt. 102 (45, arrived Jul 44), s.Pz.Abt. 503 (33 Tiger I + 12 Tiger II in Normandy, from Jun 44). Total ~126-132 Tigers committed to Normandy; 132 per Chieftain2 (W); none remained operational at the end | LS-Tiger (W), PW-Tiger (W), WP-Tiger (W) |
| M4A3(76)W | 1,925 (Detroit 1,400 Feb-Jul 44; Fisher 500 Sep-Dec 44) | ETO: first 76 mm combat Cobra (late Jul 1944), 2nd/3rd Armored in Aug; by end of war ~ half of US Shermans in Europe had the 76 | WP-M4 (W) |
| M36 | 2,324 total (M36 1,413; B1 187; B2 237 or 724 (WP inconsistent)) | first 40 shipped Sep 1944, in combat Oct 1944 (WP-M36 W) | WP-M36 (W) |
| M4A3E2 | 254 (Fisher Body; 4 stayed in US; 250 shipped) completed Jul 1944 | New York 14 Aug 44; 128 into France via Cherbourg 22 Sep 1944; first 36 issued to First Army 14 Oct; last delivery 9 Nov 1944. 4th Armored recorded 24 lost. Cobra King in Bastogne 26 Dec 1944 | TE-Jumbo (W) |

Combat notes (W unless marked):
- Panther: side/rear penetrated at 1,500 yd by 75 mm M3 APC and 76 mm M62 (Isigny, Chieftain1); frontal glacis immune to 75/76 (US) except HVAP at <=400 yd; mantlet shot trap (WP-Panther); chin mantlet fixed it (from Sep 1944, partial). Lorraine 19 Sep 1944: 43 German AFVs destroyed, mostly Panthers, vs 3 M10s and 5 M4s lost (Chieftain2).
- Tiger: ~ 1:3 reliability ratio as listed; very few in West; reliability list above.
- Jumbo: used as lead/point tank; mines and AT fire, weak sides/rear (TE W).
- M36: open top; Lt Rose of 814th killed a Panther at 4,600 yd (WP-M36 W).

---
## 6. Secondary vehicle candidates (short rows; mostly (M) or (verify))
| Vehicle | Armour (mm @ deg from vert.) | Gun / rounds | Weight, speed | Notes |
|---|---|---|---|---|
| Jagdpanzer IV (L/48) | glacis 60 @ 50; nose 50 @ 45; side 30 @ 0 (M) | 7.5 cm StuK 40 L/48; 79 rounds (M) | 24 t, 35 km/h (M) | built Dec 43-; ~ 770 L/48 (M) |
| Panzer IV/70 (V) | glacis 80 @ 50; nose 50; side 40 @ 30 (M) | 7.5 cm StuK 42 L/70; 55-60 rounds (M) | 28 t, 35 km/h | nose-heavy, 930 built (M) |
| M18 Hellcat | hull 12.7 mm sloped; turret 12.7-25; open top | 76 mm M1A1 (same ammo as 3.1); 45 rounds (M) | 17-18 t, 88 km/h (55 mph) | 2,507 built (M); ETO from Jul-Aug 1944 |
| Tiger II (Pz.VI B) | hull front 150 @ 50 (upper), 100 @ 50 (lower); side 80 @ 25; turret front 180 @ 9 (Henschel), mantlet 100-150 | 8.8 cm KwK 43 L/71 | 69.8 t, 41 km/h | s.Pz.Abt. 503 (12 in Normandy; WP/LS-Tiger W); ~489 built (M); US 90 mm could not penetrate front (WP-M36 W) |
| M4A3E8 (76)W HVSS | same armour as M4A3(76)W (63.5 @ 47) | 76 mm M1A2; 71 rounds | 33.7 t | HVSS first combat Dec 1944 (WP-M4 W); 76 mm Easy Eights ~ 2,617 built (M) |
| Jagdpanzer 38(t) Hetzer | front 60 @ 60; side 20 @ 40 (M) | 7.5 cm Pak 39 L/48; 41 rounds | 15.75 t | 2,584 built (M); ETO from Aug 1944 |
All **(verify)** except where stated.

---
## 7. Heavy mortars
| Item | 12 cm GrW 42 | 4.2 in (107 mm) M2 chemical mortar |
|---|---|---|
| Bomb weight | 15.6 kg (34 lb 6 oz) WP-GrW42, imfdb, ww2db; TM-E 30-451 says 35 lb | HE M3: 11.11 kg (24.5 lb) WP-M2mortar; 4P2 says 22 lb w/ fuze; WP M2 11.57 kg; chem 13 kg |
| Filler | not found on pages; TM-E: "four types of HE", ~ 20 % filler class (M) **(verify)** | M3 HE: 3.64 kg (8.0 lb) TNT (WP; 4P2 agrees 8 lb); M4: 6.6 lb |
| Max range | 6,000 m (WP), 6,050 m (ww2db), 6,600 yd = 6,035 m (TM-E 30-451) | 4,000 m HE (WP, "about 4 km"); 4.4 km chem; 3,200 yd standard M2 in 4P2 and 4,397 yd with extra increments (4P2) (disagree; use ~ 3,650-4,000 m) |
| Min range | not found. Elevation 45-85 deg per TM-E (W) ; gives min ~ 400-500 m (M) **(verify)** | 340 yd (310 m) 4P2 (W) |
| Rate of fire | 8-10 rpm (WP, imfdb) | 5 rpm for 20 min sustained, then 1 rpm (WP-M2mortar W); bursts up to 20 rpm (M) |
| Crew | 5-6 (M) **(verify)** | 4-5 (M) **(verify)**; 4P2 and WP give none |
| Weight | 280 kg firing (WP, ww2db, imfdb); 616 lb = 279 kg total (TM-E); two-wheeled axle quickly attached, bipod clamped to the mortar for towing (TM-E) | 151 kg (tube 48, baseplate 79, bipod/monopod 24) WP-M2mortar |
| Design | smoothbore, muzzle-loading, copy of Soviet 120-PM 38 (TM-E: "near-exact copy of Russian"); heavy bomb gives fire "comparable with 105 mm howitzer" | rifled barrel, spin stabilised, muzzle-loaded; later bomb has pre-engraved driving band (M3) |
| Charges | not on pages (M: primary + 6 increments) | adjustable propellant disks (4P2 W) |
| Organisation | Heavy mortar companies in Gr.W battalions / Werfer units; infantry regiment heavy company (M) | Chemical mortar battalions: 48 M2 mortars each, 4 companies x 3 platoons x 4 tubes; Dec 1944-Feb 1945 D companies inactivated to form new battalions; typically 1-2 companies (12 mortars each) support a division (WP-M2mortar W). Combat debut Sicily 1943 (35,000 rounds) |
| ETO use | | HE for infantry support more than chemical (US never used gas); smoke WP, 75-90 % of fire was HE (M) |

---
## 8. Key takeaways for game balance
1. Panther glacis 80 @ 55 (LOS 139): 76 mm M62 (0 deg ~125 mm at 100 m, 106 at 1,000) cannot penetrate at any range (ARMET, Isigny); M93 HVAP marginal only at <= 400 yd and often ricochets; the game should make the glacis effectively immune to 75/76 AP, but not to HVAP at short range.
2. 90 mm M82 (0 deg 164/152/137/126/114 at 100/500/1000/1500/2000) defeats Panther sides/turret at all ranges but NOT the glacis (Jan 1945 pamphlet); T33/M77 AP kills glacis to ~1,100 yd (600 yd in Isigny); T30E16 HVAP kills glacis to ~450 yd and mantlet/turret to 800 yd.
3. German 7.5 L/70 PzGr 39/42 (0 deg 185/168/149/132/116) and 8.8 cm PzGr 39 (162/151/138/126/116) both defeat any Sherman/M36 front at all ranges; Jumbo glacis (LOS 149) is where PzGr 40/42 (265/234/199) or 8.8 PzGr 40 (219/200/179) are needed: 7.5 PzGr 39/42 reaches only to ~1,000 m nominal, 8.8 PzGr 39 ~ 700 m.
4. Panther G 75 mm L/70 is only marginally less penetrating than the 8.8 cm L/56 beyond 500 m at normal impact; the Tiger's advantage is armour (100/80 mm flat) and 8.8 HE (0.87 kg (verify) vs 0.65 kg).
5. Tiger I is vulnerable to the 90 mm HVAP/ M82 only at < 1,000 m on its 100 mm face (nominal 0 deg figures; flat plate so no slope benefit); Panther's strength is slope; do not let slope-effectiveness scale linearly in the game: calibre/thickness overmatch matters.
6. Sides: Panther 40-50 mm side and Tiger 60-80 mm are open to M62 at 1,000+ m (Isigny: sides/rear at 1,500 yd): flanking should be decisive for the Panther.
7. HVAP scarcity: US 76 mm ~ 2-3 rounds per tank from Sep 1944; 90 mm HVAP nearly nil until Feb 1945; German PzGr 40 nearly nil: model as stowage 2-4 rounds, not a free ammo type.
8. Jumbo should be a 75 mm vehicle with 4 in glacis, 254 built, arrival from Sep 1944; add 76 mm re-gunning (~100 in Feb 1945) as a variant if wanted.
9. M36: open turret (HE/airburst/mortar susceptible), 47 rounds, hand/hydraulic traverse at 24 deg/s, thin 38 mm @ 55 deg hull and 76 mm mantlet: glass cannon, from Oct 1944.
10. Mortars: 12 cm GrW 42 (15.6 kg bomb, 6 km, 8-10 rpm, 280 kg) is a long-range indirect gun comparable to a 105; M2 4.2 in (11 kg, 8 lb TNT, ~4 km, 5 rpm sustained, 151 kg) is lighter and shorter; chemical battalions were typically attached to divisions.

## 9. Open items needing verification
B&L tables (none obtained); slopes of Tiger I and Jumbo faces; 8.8 SprGr and 90 mm M71 filler masses; Tiger and Panther hand-traverse rates; GrW 42 crew, min range and filler; M36 per-type split; 12 cm GrW 42 charge increments.
