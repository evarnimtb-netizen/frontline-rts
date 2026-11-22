# S15 USSR (Red Army, 1944): historical data (research only)

Working note for the S15 plan (2026-11-22, research agent). Not a spec: values here are inputs; the plan decides.

Legend, as in `S13-research-history.md`: **(W)** = value from a fetched web page, source named. **(M)** = from my own memory of Zaloga / Rottman / Shirokorad, NOT confirmed by a fetched page: treat as **(verify)**. Slopes are from vertical (0 = vertical plate) unless the row says otherwise. ONWAR tables measure from horizontal; converted here (90 − angle).

**Penetration criteria (read first).** Soviet tables give **75 % success** ("Soviet method") at meet angles 90° (= 0° obliquity) and 60° (= 30° obliquity); Wikipedia warns they are "not directly comparable with western data" (WP-ZiS3, WP-ZiS2). B&L (Bird & Livingston 2001) has Soviet rows, but the book is not online. One B&L row was found second-hand (45 mm, via WP-M42-ipfs, which cites B&L 2001; page not given). Other rows are labelled: **Sov75-0°** (Soviet 75 %, normal), **US50-calc** (Wikipedia's "calculated, 90°, US 240 BHN plate, 50 %": the same substitute S13 used), **WWT-calc** (wwiitanks calculation from calibre, mass and MV; it runs high). The repo's ammo rows use B&L 50 % at 0°, so a Sov75 row **under-rates** Soviet guns against German and US rows. A 75 % figure is lower than the 50 % one for the same gun, by a few mm (verify the size of the gap).

URLs (short names in tables):
- WP-ZiS3 https://en.wikipedia.org/wiki/76_mm_divisional_gun_M1942_(ZiS-3) ; WP-ZiS2 https://en.wikipedia.org/wiki/57_mm_anti-tank_gun_M1943_(ZiS-2) ; WP-M42 https://en.wikipedia.org/wiki/45_mm_anti-tank_gun_M1942_(M-42) ; WP-M42-ipfs https://en.wikipedia-on-ipfs.org/wiki/45_mm_anti-tank_gun_M1942_(M-42) (older revision with the B&L table)
- WP-52K https://en.wikipedia.org/wiki/85_mm_air_defense_gun_M1939_(52-K) (D-5T rows) ; WP-F34 https://en.wikipedia.org/wiki/76_mm_tank_gun_M1940_F-34 ; WP-BS3 https://en.wikipedia.org/wiki/100_mm_field_gun_M1944_(BS-3) ; WP-A19 https://en.wikipedia.org/wiki/122_mm_gun_M1931/37_(A-19)
- WP-M30 https://en.wikipedia.org/wiki/122_mm_howitzer_M1938_(M-30) ; WP-RG43 https://en.wikipedia.org/wiki/76_mm_regimental_gun_M1943 ; WP-BM37 https://en.wikipedia.org/wiki/82-BM-37 ; WP-PM38 https://en.wikipedia.org/wiki/M1938_mortar ; JP-M https://jaegerplatoon.net/MORTARS5.htm (Finnish data on captured Soviet 82 mm)
- WWT-ZiS2 https://wwiitanks.co.uk/FORM-Gun_Data.php?I=279 ; WWT-85 .../FORM-Gun_Data.php?I=299 ; WWT-ZiS3 .../I=292 ; WWT-122 .../I=314 ; WWT-45 .../I=275
- WT-M30 https://old-wiki.warthunder.com/M-30_(122_mm)/Ammunition (game wiki: weak; used only where nothing better was fetched)
- WP-T34 https://en.wikipedia.org/wiki/T-34 ; TE-T3485 https://www.tanks-encyclopedia.com/ww2/soviet/soviet_t34-85 ; ONW-T3485 https://mail.onwar.com/wwii/tanks/ussr/su047t3485m44.html ; WP-IS2 https://en.wikipedia.org/wiki/IS-2 ; WP-SU100 https://en.wikipedia.org/wiki/SU-100 ; WP-SU85 https://en.wikipedia.org/wiki/SU-85 ; TE-SU85 https://www.tanks-encyclopedia.com/ww2/soviet/soviet_SU-85.php ; WP-SU76 https://en.wikipedia.org/wiki/SU-76 ; OSP-SU76M https://www.ospreypublishing.com/us/discover/osprey-publishing/machine-of-the-month/su-76m ; WP-BA64 https://en.wikipedia.org/wiki/BA-64 ; WP-M3SC https://en.wikipedia.org/wiki/M3_Scout_Car ; WP-US6 https://en.wikipedia.org/wiki/Studebaker_US6_2%C2%BD-ton_6%C3%976_truck
- BO-RC44 https://battleorder.org/ussr-rifle-co-1944 (cites Kennedy "Bayonet Strength"; Zaloga & Ness, *Red Army Handbook 1939-45*, Sutton 1998; Rottman, *Soviet Rifleman 1941-45*, Osprey 2007)
- WP-SN https://en.wikipedia.org/wiki/Stalnoi_Nagrudnik ; WP-ShISBr https://en.wikipedia.org/wiki/Assault_Engineering_Brigades ; WP-ROKS https://en.wikipedia.org/wiki/ROKS_flamethrowers ; WP-Desant https://en.wikipedia.org/wiki/Tank_desant ; TA-riders https://www.tankarchives.com/2013/09/tank-riders.html
- WP-Mosin https://en.wikipedia.org/wiki/Mosin%E2%80%93Nagant ; WP-PPSh https://en.wikipedia.org/wiki/PPSh-41 ; WP-PPS https://en.wikipedia.org/wiki/PPS_submachine_gun ; WP-DP https://en.wikipedia.org/wiki/Degtyaryov_machine_gun ; WP-Maxim https://en.wikipedia.org/wiki/PM_M1910 ; WP-SG43 https://en.wikipedia.org/wiki/SG-43_Goryunov ; WP-PTRD https://en.wikipedia.org/wiki/PTRD-41 ; MF-PTRS https://modernfirearms.net/en/anti-tank-rifles/russia-anti-tank-rifles/simonov-ptrs-eng/ ; WP-145 https://en.wikipedia.org/wiki/14.5_%C3%97_114_mm ; WP-TT https://en.wikipedia.org/wiki/7.62%C3%9725mm_Tokarev ; WP-F1 https://en.wikipedia.org/wiki/F1_grenade_(Russia) ; WP-RPG43 https://en.wikipedia.org/wiki/RPG-43 ; WP-RPG6 https://en.wikipedia.org/wiki/RPG-6
- LS-TTT34 https://www.lonesentry.com/articles/ttt34/russian-artillery-support.html (Tactical and Technical Trends No. 34, 23 Sep 1943) ; BF-delay https://community.battlefront.com/topic/27354-delay-in-called-soviet-artillery-fire/ (quotes *The Battle of Kursk: The Soviet General Staff Study*, 1944)
- Not obtained online: TM 30-430 *Handbook on USSR Military Forces* (1945), the 04/550 shtat itself, Shirokorad *Encyclopedia of Soviet Artillery*, B&L. Each is the book to check before shipping.

---
## 1. Infantry

### 1.1 Rifle section (strelkovoe otdelenie): establishment and field strength
| Item | Value | Source |
|---|---|---|
| Shtat | Rifle division **04/550, dated 10 Dec 1942**, with early-1944 changes (one rifle platoon per company became an SMG platoon); superseded by **05/40 in Dec 1944** (in force early 1945) | BO-RC44 (W) |
| Section, paper | **9**: commander (PPSh-41 / PPS-43), deputy (yefreytor; Mosin or SVT-40), DP-27 gunner, assistant gunner (Mosin or SMG), 1 rifleman with SMG, 4 riflemen (Mosin). "Heavy" section: 2 DP teams, 2 riflemen | BO-RC44 (W) |
| Platoon | 1 officer + 39: HQ 2, 2 PU snipers (a pair), 2 light + 2 heavy sections | BO-RC44 (W) |
| Company | 4 officers + 137; medical section 1 instructor + 4 medics (rifles or carbines); MG section 6 (Maxim, 3 bearers, horse driver); 50 mm mortar platoon (7) | BO-RC44 (W) |
| Field strength 1944 | "By 1944 platoons were reduced to 4 sections of 6 men"; the 4th section often absent | BO-RC44 (W) |
| Division strength 1944 | Typically 3,000-5,000 of a ~9,400 paper strength (Glantz) | (M) **(verify)** |
| Ammo per man | Mosin: 40 ready + 30 reserve + 70 bandolier (140). SVT: same 140. DP: gunner 1 pan (47), assistant 3 pans in a can (141), "about 188 per gun in practice". PPS-43: 3 x 35 = 105. PPSh: 71 in gun + 71 or 176 ready (pre-1944 load) | BO-RC44 (W) |
| Grenades per man | one of: 1 F-1, or 3 RG-42, or 2 RGD-33 | BO-RC44 (W) |
| Bayonets | M91/30 spike bayonet; Soviet practice: zeroed and carried fixed. M1944 carbine: bayonet **permanently attached**, side-folding | WP-Mosin (W for M44); "carried fixed" is (M) **(verify)** |

Recommendation: full card 9 (light section layout), field-strength card 6 (`extends`; commander, DP team, 3 riflemen, of whom 1 SMG). Guards: the honour went to whole divisions, and Guards shtat gave more SMGs and MGs (M, **verify** against shtat 04/500-series); a training variant is fair, a new weapon set is not.

### 1.2 Small arms (weapon defs)
| Weapon | Mass | MV | Rate | Feed | Window / notes | Source |
|---|---|---|---|---|---|---|
| Mosin M91/30 | 4.0 kg | ~865 m/s (light ball) | bolt | 5-rd stripper clip | Standard 1930-45 | WP-Mosin (W) |
| M91/30 + PU (3.5x), sniper | ~4.7 kg (M) | ~865 | bolt | single rounds (bent bolt, no clips) | PU from 1942 | WP-Mosin (W) |
| M1944 carbine | ~4.1 kg (M) | ~800 (M) **(verify)** | bolt | 5-rd clip | Issued "from Feb 1944" per one passage, "late 1944" per another (page conflicts). Use 1944-02 **(verify)** | WP-Mosin (W) |
| SVT-40 | 3.85 kg (M) | ~840 (M) | semi | 10-rd box | Scarce in line units by 1944 (production ended early 1945) (M) **(verify)** | (M) |
| PPSh-41 | 3.63 kg empty; 5.45 with drum; 4.32 with box | 488 m/s | ~1,000-1,250 rpm (page 1,250) | 71-rd drum; 35-rd box from 1942 | ~5 million by 1945 | WP-PPSh (W) |
| PPS-43 | 3.04 kg | ~500 m/s | 500-600 rpm | 35-rd box | Mass production mid-1943; ~2 million by 1945 | WP-PPS (W) |
| DP-27 / DPM | 9.12 kg empty, 11.5 loaded | 840 m/s | 550 rpm (combat ~80 rpm) | 47-rd pan | DPM adopted 1943-44 (rear pistol grip, spring moved) | WP-DP (W) |
| Maxim M1910/30 | gun 23.8 kg; Sokolov mount ~31 kg (inferred: Vladimirov mount 39 kg = 8 kg heavier) | 740 m/s | 600 rpm | 250 belt | Water-cooled; shield 6 mm (M) **(verify)** | WP-Maxim (W) |
| SG-43 Goryunov | 13.8 kg gun; 41 kg on wheeled mount | 800 m/s | 500-700 rpm | 200 / 250 belt | Adopted May 1943; both in service in 1944 | WP-SG43 (W) |
| PTRD-41 | 16 kg | 1,012 m/s | 8-10 rpm | single shot | 293,164 built 1941-44 | WP-PTRD (W) |
| PTRS-41 | 20.9 kg | ~1,010 (as PTRD) | semi | 5-rd en-bloc | ~60,000 (1941-42 figure on page); effective 300-400 m | MF-PTRS (W) |
| TT-33 pistol | 0.85 kg (M) | ~420 (M) | semi | 8-rd | officers, crews | (M) |

### 1.3 Cartridges (defs/cartridges)
| Id | Bullet | MV | Packed g/round | Source |
|---|---|---|---|---|
| 7.62x54R, clip (light ball L 9.6 g; heavy ball D 11.8 g for MGs) | 9.6 g (1908 light bullet 9.61 g) | 865 (Mosin) | ~25-27 g incl. clip (M) **(verify)**; B-32 AP-I cartridge 22.6 g (modern 7BZ3 figure) | WP-Mosin (W); 7.62x54R page (W for 7BZ3 only) |
| 7.62x54R, DP pan / Maxim belt | as above | 740-840 | per-id packaging as S5a rule | (M) |
| 7.62x25 Tokarev | 5.5 g FMJ | ~490 from PPSh | ~11 g (M) **(verify)** | WP-TT (W) |
| 14.5x114, B-32 (API, steel core) | 64 g | ~1,000-1,006 | ~200 g (BS figure) | WP-145 (W) |
| 14.5x114, BS-41 (tungsten carbide core 38.7 g) | 64.4 g | ~1,006 | ~200 g | WP-145 (W) |

PTR penetration (0° obliquity): **B-32: 32 mm at 500 m** (WP-145); **BS-41: ~40 mm at 100 m, 30-32 mm at 500 m** (WP-145), "40 mm at 100 m and 35 mm at 300 m (90°)" (MF-PTRS); PTRD page: ~40 at 100, ~30 at 500 (WP-PTRD). Proposed row (both rounds, flag): `[[100, 40], [300, 35], [500, 30]]`. BS-41 was scarce (tungsten) (M). Enough for half-tracks, armoured cars, Panzer IV side skirts and 30 mm sides close in; nothing on a tank front.

### 1.4 Grenades and AT grenades (defs/items)
| Item | Mass | Filler | Other | Source |
|---|---|---|---|---|
| F-1 (defensive, cast iron) | ~600 g | 60 g TNT | fuze 3.2-4.2 s (UZRG); effective radius ~30 m, fragments to 200 m; throw 30-45 m | WP-F1 (W) |
| RGD-33 (stick, optional frag sleeve) | ~500 g; ~750 g with sleeve | ~85 g TNT | 3.5-4 s | (M) **(verify)** |
| RG-42 (tin can, offensive) | ~420 g | ~110-120 g TNT | 3.2-4 s; adopted 1942 | (M) **(verify)** |
| RPG-43 (HEAT, drogue) | 1.2 kg | 612 g TNT | ~75 mm RHA at 90°; throw 14-18 m (US manuals); service Apr 1943 | WP-RPG43 (W) |
| RPG-6 (HEAT) | 1.13 kg | 562 g TNT | ~100 mm; service Oct 1943 | WP-RPG6 (W) |

### 1.5 Assault sappers (ShISBr) and SN-42
| Item | Value | Source |
|---|---|---|
| ShISBr | 15 brigades formed by 30 May 1943; at the front from Aug 1943; brigade = HQ 40, HQ coy 87, motor engineer-scout coy 101, **5 battalions of 388**, bridging 36. Withdrawn after each breakthrough (no heavy weapons of their own). Procedure for their use dated Dec 1943 | WP-ShISBr (W) |
| SN-42 | **Two pressed plates of 2 mm 36SGN steel (tolerance 1.8-2.2 mm), ~3.5 kg**, front torso and groin | WP-SN (W) |
| SN-42 protects | bayonet, small fragments, 9 mm lead-core pistol bullets; "an MP 40 at close range" (page wording; the US/UK figure usually quoted is 100 m (M)); **useless against 7.92x57 rifle rounds** | WP-SN (W) |
| ROKS-3 | ~9 L fuel, range up to ~45 m ideal; issued to ShISBr from **spring 1944**; mass ~23 kg filled, 6-8 bursts (M) **(verify)** | WP-ROKS (W), WP-ShISBr (W) |
| Assault group (shturmovaya gruppa) | No establishment found online. Practice (Rottman, M): 20-50 men in town fighting, split into assault, support (MG, PTR) and reserve teams, with sappers carrying charges. An 8-man card is a game abstraction (M) **(verify)** | (M) |

SN-42 game plate: `torso_front` 2 mm steel at 0° (curved; no slope data). With the repo's cosine model, the 9 mm and 7.62x25 rows must fail on 2 mm at mid range and 7.92x57 must pass at all ranges; tune the small-arms penetration rows to match the WP statement, not the other way round **(verify)** with any pistol-vs-2 mm data the S12 hit-zone work uses.

### 1.6 Other infantry cards
| Card | Men | Weapons | Source |
|---|---|---|---|
| Sapper section (sapyornoe otdelenie) | 7-9 (M) **(verify)** | Mosin/PPSh, TM-41 AT mine, PMD-6 box mine | infantry-structures.md 2.3; (M) |
| HMG crew (pulemyotny raschyot) | company MG section = 6 incl. horse driver (BO-RC44 W); a card of 4-5 (commander, gunner, 2-3 bearers) | Maxim or SG-43 | BO-RC44 (W) |
| PTR team | 2 (gunner, assistant) | PTRD or PTRS; assistant Mosin (M) | WP-PTRD (W: crew 2) |
| Sniper pair | 2 (platoons held 2 PU snipers) | M91/30 PU | BO-RC44 (W) |
| Medics (sanitar) | company medical section 5, all with rifles or carbines | carbines (so not unarmed, unlike US aid men) | BO-RC44 (W) |
| Platoon HQ | 2 (commander SMG + pistol, deputy rifle); a 3rd as runner | | BO-RC44 (W) |

---
## 2. Mortars and guns

### 2.1 Mortars
| Item | 82 mm BM-37 (BM-41 / BM-43) | 120 mm PM-38 (PM-43) | Source |
|---|---|---|---|
| Mass in action | BM-37 56-57.3 kg (barrel 18.5, plate 19.4, bipod 18.1); BM-41 47.5 kg (wheels replace bipod, poor) ; BM-43 wheeled axle, "highly successful" | 280 kg firing; 477 kg on travel carriage | JP-M (W), WP-BM37 (W); WP-PM38 (W) |
| Range | 73-100 m to **3,040-3,100 m** with 82 mm bombs; **2,600 m with captured 81 mm bombs** | 5.7 km (OF-843 HE-frag); OF-843A 5.52 km | WP-BM37, JP-M (W); WP-PM38 (W) |
| MV max | 202-211 m/s | 272 m/s | JP-M, WP-BM37 (W); WP-PM38 (W) |
| Elevation / traverse | +45 to +85°; 6° (BM-41 6-10°) | (M) +45 to +80° | JP-M (W) |
| Rate | 15-25 sustained; 25-30 max (a wartime Soviet manual says 20) | 10 rpm (WP); 15 max (M) | JP-M (W); WP-PM38 (W) |
| Crew | 4-5 (commander, gunner, loader, 1-2 bearers) (M) **(verify)** | **6** | (M); WP-PM38 (W) |
| Bomb | **O-832D (frag), ~3.1-3.3 kg** (WP 3.05; JP-M 3.1-3.2; "3.31" is the usual memory value (M)); filler ~0.4 kg TNT/ammatol (M) **(verify)** | **OF-843A 15.9 kg, 1.58 kg filler**; OF-843 15.9 kg, 3.0 kg; OF-843B 16 kg, 1.4 kg; F-843 HE 16.2 kg, 3.93 kg | JP-M (W), WP-BM37 (W); WP-PM38 (W) |
| Smoke | D-832 (M) | D-843A 16.44 kg | WP-PM38 (W) |
| Tow | carried by crew (3 loads) | own limber; with 20-round caisson behind a horse team or vehicle (GAZ-AA / Studebaker) | WP-PM38 (W) |

Captured ammo: **confirmed one way**. WP-BM37: the 82 mm "was designed to fire western 81 mm captured ammunition, while denying an enemy the same advantage". JP-M: all non-British 81 mm bombs fire from 82 mm tubes, max 2,600 m, MV ~193 m/s, accuracy "poor enough to endanger your own troops unless precautions are taken". An 82 mm bomb does not fit an 81.4 mm tube (calibre; the design intent above). Game: `accepts = ["mortar_82_sov", "mortar_8cm_ger"]` with range cap 2,600 m and a larger dispersion multiplier (data, start x2 **(verify)**). Note the Germans used captured 82 mm tubes as 8.2 cm GrW 274/2(r) (WP-BM37) with Soviet bombs only.

### 2.2 AT and field guns
| Item | 45 mm M-42 | 57 mm ZiS-2 | 76 mm ZiS-3 | 76 mm M1943 regt. | 122 mm M-30 |
|---|---|---|---|---|---|
| Mass action / travel | 625 / 1,250 kg | 1,250 kg (action) | 1,116 / 2,150 kg | 600 / 1,300 kg | 2,450 / 3,100 kg |
| Crew | 6 | 7 | 7 | 6 (M) **(verify)** | 8 |
| Rate | 15-20 rpm | 25 cyclic, **10 practical** | up to 25 | 10-12 | 5-6 |
| Max range | 4.55 km | 8.4 km | 13.29 km | 4.2 km | 11.8 km |
| Elevation / traverse | -8 to +25° / 60° | -5 to +25° / 56° | -5 to +37° / 54° (M) | -8 to +25° / 60° | -3 to +63.5° / 49° |
| Shield | **7 mm** | (M) 6 mm | (M) 6 mm **(verify)** | (M) 4.5-6 | (M) 4-6 |
| Built | 10,843 (1942-45) | 371 in 1941; **9,645 from 15 Jun 1943 to 1945** | 103,000+ (from Dec 1941) | 5,122 (1943-45) | 16,887 in 1941-45 |
| Tow | horses, GAZ-AA, Willys/Dodge (M) | Studebaker / Dodge WC (M) | Studebaker US6 (M); horses | horses, Willys (M) | 6-horse limber, STZ-5, Ya-12, Studebaker, Dodge WC |
| Source | WP-M42 (W) | WP-ZiS2 (W) | WP-ZiS3 (W) | WP-RG43 (W) | WP-M30 (W) |

### 2.3 Rounds (defs/ammo)
| Round | Type | Projectile kg | MV m/s | Filler | Source |
|---|---|---|---|---|---|
| 45 mm BR-240 | APHE (APBC-HE) | 1.43 | 870 | ~18-20 g (M) | WP-M42 (W mass, gun MV) |
| 45 mm BR-240P | APCR | 0.85 | ~1,070 (M) **(verify)** | none | WP-M42 (W mass) |
| 45 mm O-240 | frag | 2.14 | ~335 (M) | ~0.12 kg (M) | WP-M42 (W mass) |
| 57 mm BR-271 | APCBC | 3.14 | up to 990 | 14 g | WP-ZiS2 (W) |
| 57 mm BR-271K | APHE | 3.14 | 990 | 18 g | WP-ZiS2 (W) |
| 57 mm BR-271P | APCR ("reel") | 1.79 | up to 1,250 | none | WP-ZiS2 (W) |
| 57 mm O-271U | frag | 3.75 | up to 700 | 204-220 g | WP-ZiS2 (W) |
| 76 mm BR-350A | APHE, sharp nose | 6.3 | 700 (ZiS-3; F-34 655) | 155 g | WP-ZiS3, WP-F34 (W) |
| 76 mm BR-350B | APHE, blunt + ballistic cap | ~6.5 (M) | ~680 (M) | ~120 g (M) | **not on any fetched page; (verify)**; date 1943 (M) |
| 76 mm BR-350P | APCR, from Apr 1943 | 3.02 | up to 950 (F-34 965) | none | WP-ZiS3, WP-F34 (W) |
| 76 mm OF-350 | HE-frag, steel | 6.2 | 680 | 710 g | WP-ZiS3 (W) |
| 76 mm BP-350M | HEAT, from May 1943 | 3.94 | ≤680 (regt. gun: low charge) | 623 g | WP-ZiS3 (W) |
| 76 mm regt. HE (OF-350) | HE-frag | 6.2 | 262-311 | 710 g | WP-RG43 (W) |
| 122 mm OF-462 | HE-frag | 21.76 | 515 | 3.67 kg (TNT eq.) | WT-M30 (W, game wiki; **verify** vs Shirokorad) |
| 122 mm BP-460A | HEAT | 13.4 | 335 | 2.73 kg | WT-M30 (W); 100-160 mm at 90° (WP-M30 W) |
| 85 mm BR-365 / BR-365K | APHE (sharp / blunt) | 9.2-9.36 (M) | 792 | ~48 g (M) | WP-52K (W MV) |
| 85 mm BR-365P | APCR | ~5.0 (M) | 1,050 | none | WP-52K (W MV) |
| 85 mm O-365K | HE-frag | 9.54 | 793 | 741 g | WWT-85 (W) |
| 122 mm BR-471 (D-25T) | APHE, sharp | 25 | ~795-800 | ~156 g (M) | IS-2 notes 2.4 |
| 122 mm OF-471 (D-25T) | HE-frag | 25 | 800 | 3.8 kg TNT | WP-IS2 (W) |

### 2.4 Penetration rows (mm, 0° obliquity unless stated)
| Round | 100 | 500 | 1000 | 1500 | 2000 | Criterion / source |
|---|---|---|---|---|---|---|
| 45 mm BR-240 | **71** | **54** | **40** | **32** | **28** | **B&L 2001 (via WP-M42-ipfs; page not given)** (W) |
| 45 mm BR-240P | **108** | **74** | **46** | – | – | B&L 2001 (via WP-M42-ipfs) (W) |
| 57 mm BR-271 | 114 | 103 | 91 | 79 | 69 | Sov75-0° (WP-ZiS2) |
| 57 mm BR-271 (alt.) | 143 | ~127 | ~110 | ~94 | 81 | WWT-calc 0° (runs high) |
| 57 mm BR-271K | 112 | 94 | 74 | 57 | 44 | Sov75-0° (WP-ZiS2) |
| 57 mm BR-271P | 190 | 147 | 101 | – | – | Sov75-0° (WP-ZiS2) |
| 76 mm BR-350A (ZiS-3) | 82 | 75 | 67 | 60 | 53 | Sov75-0° (WP-ZiS3) |
| 76 mm BR-350A (F-34, 655 m/s) | – | 69 | 61 | – | – | WP-F34, criterion not stated |
| 76 mm BR-350A (alt.) | 91 | ~80 | ~68 | ~57 | 48 | WWT-calc 0° |
| 76 mm BR-350P | ~(verify) | 92 | 60 | – | – | WP-F34 (965 m/s; criterion not stated) |
| 85 mm BR-365 (D-5T / ZiS-S-53) | 142 | 125 | 107 | 92 | 78 | US50-calc (WP-52K) |
| 85 mm BR-365K (APBC) | 139 | 123 | 105 | 91 | 81 | US50-calc (WP-52K) |
| 85 mm BR-365P | 175 | 136 | 100 | 73 | 54 | US50-calc (WP-52K) |
| 122 mm BR-471 (D-25T) | 178 | ~166 | ~152 | ~140 | 128 | WWT-calc 0° (page labels it "OF-471 APHE": muddled; **verify**) |
| 100 mm BR-412B (SU-100) | – | 190 @30° | 170 @30° | – | – | WP-BS3 (30° obliquity, criterion not stated) |
| 76 mm BP-350M HEAT | flat ~70-75 (M) **(verify)** | | | | | (M) |
| 122 mm BP-460A HEAT | flat 100-160 | | | | | WP-M30 (W, range of values) |

Recommendation: use the B&L 45 mm row as given. For 57, 76 and 85 mm, the 0° rows marked Sov75 should be raised to a 50 % row before they are compared with the German B&L rows. Wikipedia's US50-calc rows (85 mm here, S13's German rows) are the consistent open substitute. For the 57 and 76 mm a US50-calc row was not found: S13-research-obliquity.py style tooling could compute one (DeMarre from the 85 mm pair) **(verify)**.

---
## 3. Vehicles

### 3.1 T-34-85 (1944)
| Face | Value | Source |
|---|---|---|
| Upper glacis | 45 @ 60° | ONW-T3485 (W: 45 @ 30 from horizontal); TE-T3485 (W) |
| Lower nose | 45 @ 53° (M) ; ONWAR gives 45 @ 60 for "hull front" as a whole | (M) **(verify)** |
| Hull side, upper (sponson) | 45 @ 40° | ONW-T3485 (W: 45 @ 50 from horiz.) |
| Hull side, lower | 45 @ 0° | ONW-T3485, TE-T3485 (W) |
| Hull rear | 45 @ 45° (upper ~48° per ONW: 45 @ 42 horiz.) | ONW-T3485 (W) |
| Hull top / floor | 20 / 20 | ONW-T3485 (W) |
| Turret front, mantlet | 90 cast, rounded; mantlet 90 | TE-T3485, ONW (W). TE's text also says 60 (conflict); use 90 |
| Turret side | 75 @ 20° | ONW-T3485 (W: 75 @ 70 horiz.); TE (W: 75) |
| Turret rear | 52 @ 10° | ONW-T3485 (W) |
| Turret roof | 20 @ 76-90° | ONW-T3485 (W) |

| Item | Value | Source |
|---|---|---|
| Gun | **D-5T: production Jan 1944** (Plant 112, the first T-34-85s from Dec 1943 per TE). **S-53 (later ZiS-S-53): from Feb-Mar 1944 at Plant 112, in parallel**; all "model 1944" tanks have ZiS-S-53. Same ballistics (85 mm L/52-L/55, 792 m/s). D-5T tanks: a few hundred (TE: ~283 early + 600 model 1943) | WP-T34, TE-T3485 (W) |
| Rounds | 55-60 (WP 60; ONW 56-60; TE says 35, an error). Split (M): ~36 HE, ~19 AP, ~5 APCR when issued **(verify)** | WP-T34, ONW (W) |
| MG | 2 DTM (coax, bow), 1,890 rds (ONW) or 1,900-2,700 (TE) | ONW (W) |
| Elevation / traverse | -5 to +22°; electric, **360° in 21 s (~17°/s)**; hand backup | ONW-T3485 (W) |
| Weight, engine | 32 t; V-2-34(M) diesel 520 hp | ONW, TE (W) |
| Speed | **55 km/h road, 30 off-road**; cruise 47-50 | ONW, TE (W) |
| Ground pressure | 0.78 kg/cm2; 55 cm tracks | ONW (W) |
| Crew | 5 (commander, gunner, loader, driver, bow MG) | ONW (W) |
| Ammo rack | floor bins under the fighting compartment and racks on turret sides/bustle (M) **(verify)** | (M) |
| Built | ~17,700 model 1944 from Mar 1944 to May 1945 (ONW's "176,801" is a typo) | TE, ONW (W) |

### 3.2 Tank riders (tankovy desant)
| Item | Value | Source |
|---|---|---|
| Riders per tank, official | **T-34: 12**; KV: 15; T-70, Valentine, M4A2: 6 ("Short tactical-technical manual", NKO; undated) | TA-riders (W) |
| Riders per tank, usual | medium 8-10, heavy 10-12, light 5-6 | WP-Desant (W) |
| Hand-holds | ropes or welded handrails; reports complain of too few hand-holds | WP-Desant, TA-riders (W) |
| Organisation | each tank brigade had a motor rifle-machine gun battalion; riders were assigned to specific tanks before the operation and trained with them | WP-Desant (W); battalion (M) |
| Dismount | "in the final phase of the assault"; in practice 200-300 m from the enemy or at first fire (M) **(verify)** | WP-Desant (W) |
| Losses | very high; no figure fetched (M) | – |

Game: 8 riders on the T-34-85 deck (one SMG section of 7-9), official cap 12. Riders ride outside every armour face.

### 3.3 Other vehicles (1944)
| Vehicle | Armour (mm @ °) | Weapon, rounds | Mobility | Crew | Window | Source |
|---|---|---|---|---|---|---|
| SU-85 | glacis 45 @ 50 (M; vehicles.md); sides 45 upper @ 40, rear and side slopes "30" per TE **(verify)** | 85 mm D-5S, ~48 rounds (M); traverse ±10 (M) | 29.6 t, 55 km/h | 4 | service Aug 1943; production cancelled late 1944; 2,050-2,650 built; SU-85M (SU-100 hull) 315 from Sep 1944 | TE-SU85, WP-SU85 (W) |
| SU-100 | front 75 @ 55; sides 45; rear 45; roof 20 | 100 mm D-10S, 33 (vehicles.md; M) | 31.6 t, 48 km/h | 4 | production **Sep 1944**, service **Oct 1944**, first mass use Jan-Mar 1945 (Hungary) | WP-SU100 (W) |
| SU-76M | upper front 25 @ 60; lower 30 @ 30; hull side 15 @ 0; rear 15; casemate front 25 @ 25; casemate side 10 @ 20; floor/roof 7; **open top, part-open rear** | ZiS-3Sh, **60 rounds**; ±15°, -5 to +15°; DT 945 rds; PPSh 426 rds; 10 F-1 | 10.5-10.6 t; 43 km/h (governed 30) | 4 | adopted Jul 1943, 13,732 built to Oct 1945 | OSP-SU76M, WP-SU76 (W) |
| BA-64B | front 15, sides 6-9, most plates ~30°; open-top turret | DT, 1,070 rds | 2.4 t; 80 km/h (40 on GK tyres) | 2 | B model 1943-46; ~9,110 BA-64 of all types | WP-BA64 (W); side 9 (M) |
| M3A1 Scout Car | max 13 (12.7 front, 6.35 sides) | .50 M2 + 1-2 .30 on skate rail | 80 km/h | 2 + 6 | **3,310 allocated to the USSR** (plus diesel M3A1E1) | WP-M3SC (W) |
| Studebaker US6 | none | – | 72 km/h; 2.3 t off-road, 3.6 t (4 sh t) road after Red Army tests | 1-2 + cargo or ~16 men (M) | most US6 went to the USSR (~150,000 (M) **(verify)**); towed AT and field guns, carried BM-13 | WP-US6 (W) |
| IS-2 (1944) | upper glacis 100 @ 60 (stepped nose; straight 120 @ 60 late 1944); lower 100 @ 30; sides 90-130 @ 9-25; turret front 100, mantlet 120; turret side 90 @ 20; rear, roof (M) 60 / 30 | D-25T, **28 rounds (20 HE, 8 AP)**; 1-1.5 rpm, 2-3 rpm with the drop breech; BR-471B only post-war | 46 t; 37 km/h | 4 | from spring 1944; 3,854 of all IS-2 | WP-IS2 (W) |

---
## 4. Doctrine notes (AI and rules)

| Topic | Note | Source |
|---|---|---|
| Fire direction | Kursk study (1944): **prepared fire 1.5-3 min after the call, unplanned fire 4-7 min, best units 40-90 s**. The repo's 40 s (USSR) vs 15 s (USA) vs 25 s (GER) is a compressed scale; the slower Soviet value fits. Keep it data | BF-delay quoting the General Staff Study (W, second-hand) |
| Observers | Observers rode with the first tank wave (two, so one loss did not stop fire), radio code set in advance; massed batteries preferred: "several dozens of guns are more effective than any one tank" | LS-TTT34 (W) |
| Radios | scarce below battalion; wire telephone at company (M); infantry-structures.md already says so | (M) |
| Mortars | 82 mm in the battalion mortar company, 120 mm in the regiment; Red Army massed mortars more than any other army (M); 50 mm withdrawn by 1944 (guns-artillery.md P1) | (M) |
| ZiS-3 direct fire | divisional guns and regimental 76 mm fired over open sights at infantry and tanks; ZiS-3 was the IPTAP gun too (M) | (M) |
| Captured weapons | 82 mm fires German 8 cm bombs (2.1). MG 34/42 and, in 1945, Panzerfausts were widely reused (trofeinye) (M) | WP-BM37, JP-M (W); rest (M) |
| Penal units | exclude (infantry-structures.md 2.3) | – |
| Guards | honour for a division; more automatic weapons on paper (M) | (M) |
| Medics | company medics carried rifles or carbines (BO-RC44); many were women (infantry-structures.md) | BO-RC44 (W) |

---
## 5. Date windows (available_from / available_to, for a 1944 game)

| Item | From | To | Source |
|---|---|---|---|
| Rifle, SMG, sapper, MG, PTR, sniper, medic cards | 1944-01 | 1945-05 | all in service (above) |
| M1944 carbine | 1944-02 **(verify; WP conflicts: "late 1944")** | – | WP-Mosin |
| PPS-43 | mid-1943 | – | WP-PPS |
| SG-43 | 1943-05 | – | WP-SG43 |
| DPM | 1944 (1943-44 adoption) | – | WP-DP |
| RPG-6 | 1943-10 | – | WP-RPG6 |
| SN-42 / ShISBr | 1943-08 | – | WP-ShISBr |
| ROKS-3 with ShISBr | 1944-03 (spring) | – | WP-ShISBr |
| PTRD | to 1944 production; in service to 1945 | – | WP-PTRD |
| BR-350P, BP-350M | 1943-04 / 1943-05 | – | WP-ZiS3 |
| ZiS-2 | 1943-06 | – | WP-ZiS2 |
| 76 mm M1943 | 1943 | – | WP-RG43 |
| T-34-85 D-5T / ZiS-S-53 | 1944-01 / 1944-03 | – | WP-T34 |
| SU-85 | 1943-08 | late 1944 (production; in units to 1945) | TE-SU85 |
| SU-100 | 1944-10 (service) ; combat 1945-01 | – | WP-SU100 |
| SU-76M, BA-64B, US6, M3A1 | all of 1944 | – | above |
| IS-2 (stepped / straight nose) | 1944-02 (M) / late 1944 | – | WP-IS2 |

---
## 6. Core first wave and deferrals

Role map against `factions/germany.toml` and `usa.toml` (today's rosters):

| Role (GER / USA card today) | USSR core card |
|---|---|
| Line squad, full and field strength (Grenadiergruppe, Rifle Squad ± 9) | Strelkovoe otdelenie 9 + field strength 6 |
| MG card (sMG-Gruppe / LMG squad) | Pulemyotny raschyot (Maxim M1910/30; SG-43 as a later `extends`) |
| Assault infantry (VG-Sturmgruppe) | Otdelenie avtomatchikov (PPSh; also the tank-rider section) |
| Engineers (Pioniere / Engineer squad) | Sapyornoe otdelenie |
| Assault engineers (Sturmpioniere / Assault section) | Shturmovaya gruppa (SN-42); ROKS-3 pair only if S14 flame has shipped (S15 depends on "all above", so it should have) |
| Construction engineers (Baupioniere) | Sapper card with `build_heavy` (no separate card) |
| Medium mortar (GrW 34 / M1) | 82 mm BM-37 crew |
| Light mortar (kz. GrW 42 / M2) | **none (authentic gap: 50 mm withdrawn)**; the 82 mm covers it |
| Infantry AT (Panzerschreck / Bazooka) | PTRD team + RPG-43 / RPG-6 as items (weaker vs tanks: authentic) |
| Tank | T-34-85 (rider seats) |
| Assault gun / TD (StuG / M10) | SU-85 (1944); SU-76M as cheap support |
| Half-track (251 / M3A1) | M3A1 Scout Car (2 + 6) + tank riders |
| Armoured car (234/1 / M8) | BA-64B |
| Truck and tractor (Opel, SdKfz 11 / CCKW) | Studebaker US6 (truck and tow) |
| AT gun (Pak 40, Pak 38 / 57 mm M1) | ZiS-3 (dual role) and 45 mm M-42 |
| Infantry gun (leIG 18 / 105 mm M3) | 76 mm M1943 regimental gun |
| Observer (VB-Trupp / FO party) | Artillery razvedchiki (2-3) |
| Field howitzer (leFH 18 / M2A1) | 122 mm M-30 section |
| Platoon HQ, ammo bearers, medics | Upravlenie vzvoda; podnoschiki (bearers); sanitary (2, carbines) |

**Defer:** IS-2 and SU-100 (heavy-wave peers of S13's Panther, Tiger, Jumbo, M36; SU-100 is late 1944 / 1945); 120 mm PM-38 (heavy-mortar peer of S13's 12 cm GrW 42 and 4.2 in; ship with or after them); ZiS-2 (third AT gun, a Pak 40 peer by penetration; small data job once rows are agreed); Guards section, sniper pair, SG-43 variant (variants with no peer card today); ROKS-3 team (if S14 is late); BM-13 (open owner question Q23); T-34/76, M4A2 (76) W, ISU-152 (vehicles.md later list).

---
## 7. Contradictions with the repo's design docs

1. **45 mm M-42 row.** guns-artillery.md §6 has BR-240 at 0° as 100→70, 500→61, 1000→51 and BR-240P 100→105, 500→84. The B&L row (via Wikipedia) is **71 / 54 / 40 / 32 / 28** and APCR **108 / 74 / 46**: the repo's 500 and 1000 m values are 7-11 mm high.
2. **ZiS-3 BR-350A row.** guns-artillery.md §6 has 82 / 70 / 61 / 53 (at 100/500/1000/1500); the Soviet 75 % table on Wikipedia is 82 / 75 / 67 / 60 / 53 at 90° (the repo's 500-1500 m values are 5-7 mm low; 61 and 53 look taken from other columns). vehicles.md §4.5 gives MV 662 and "about 65 / 58 / 50" at 30° (the Sov 60° row is 67 / 61 / 55). The repo's MV 660 is the F-34 figure; the ZiS-3's is 680-700.
3. **Soviet table criterion.** guns-artillery.md §6 calls the Soviet tables "0°" without saying they are 75 % success. Mixed with B&L 50 % rows they under-rate the Soviet guns (S3b review H2 hit the same bug with the US M61).
4. **Crews.** infantry-structures.md 2.3 gives the 120 mm crew 5; WP-PM38 says 6 (guns-artillery.md already says 6). ZiS-3 crew 7 and ZiS-2 crew 7 (WP) vs "6-7" and "6" in guns-artillery.md.
5. **Medics.** infantry-structures.md: "unarmed or pistol"; the 1944 company establishment arms the medical section with rifles or carbines (BO-RC44).
6. **T-34-85 ammo.** TE-T3485 says 35 rounds; WP and ONWAR 55-60. vehicles.md does not state it; use 55-60.
7. **SU-100 window.** vehicles.md "front from about Jan 1945 (verify)": service from Oct 1944, mass combat early 1945 (WP-SU100). Close enough; date it 1944-10 for availability.
8. **Shtat numbers.** 04/550 is dated **10 Dec 1942** (with 1944 amendments), not a 1944 shtat; the Dec 1944 shtat is **05/40**. "04/750" was not found.
9. **BA-64B sides.** vehicles.md "9-10 mm"; WP-BA64 says 6 mm (front 15). Verify on a primary source.
