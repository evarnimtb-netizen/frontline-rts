# S17a site table (raw owner comparisons and sight-bit reads in non-test sim code)

Generated from `tools/owner_sites.sh` after the S17a Build A sweep (`s17-build`). Every Hostile and Sees
site now asks `crate::teams` (`hostile`, `allied`, `sees`, `enemy_seen`, `known_mask`); what remains as a raw
comparison is an Own test (orders, control, services, pop, cards, records of one player: rule T8), a
validation or a report. Classes: **Own**; **Hostile (teams)** / **Sees (teams)** / **Own+ally (teams)**:
converted; **Own bit**: spotting writes and reads the observer's own `seen_by` bit (T2 reads it through
`sees`); **Carry**: the mask copied into a mark or snapshot; **Presenter (P)**: fog of war, Presenter P;
**Foe**: the A1 main foe; **Report**. No row is unclassified.

Converted sites (not listed below when the comparison went away): `combat.rs` targets, rounds and
fragments past allies, blast spares, kill credit; `gunnery.rs` targets and HE clearance; `squad.rs`,
`dig.rs` enemy facing; `orders.rs` attack orders; `spotting.rs` targets; `items.rs` thrown items past allied
vehicles and pieces; `bail.rs` capture (T8) and credit; `damage.rs` blasts and credit; `fort.rs` mines,
wire, pieces, mine knowledge; `sweep.rs` mine knowledge; `fire_mission.rs` intel; `counter_battery.rs`
hearers and markers (T2); `wounds.rs` own-fire credit; `vehicle.rs` friendly-ahead yield;
`structures.rs` static boxes; `rules.rs` capture, drain, victory (T4-T7); `game.rs` load checks; AI:
`ai.rs` known enemies and vehicles, `Rel::Ally` (A2), `ai/fort.rs`, `ai/guns.rs` main foe (A1),
`ai/arty.rs`, `ai/mortar.rs` own-men tests count allies (A5), `ai/armour.rs` captures, `ai/breach.rs`
wire and points, `ai/sweep.rs`, `ai/obstacles.rs`, `ai/supply.rs`, `ai/medic.rs` points.

| Site | Class | Text |
|---|---|---|
| `ai.rs:854` | Own | `sq.owner != player` |
| `ai.rs:1025` | Own | `id.0 < g.next_squad && g.squads.get(id).is_none_or(\|q\| q.owner == player)` |
| `ai.rs:1461` | Own | `.retain(\|id, _\| g.squads.get(id).is_some_and(\|s\| s.owner == me));` |
| `ai.rs:1464` | Own | `.retain(\|id, _\| g.squads.get(id).is_some_and(\|s\| s.owner == me));` |
| `ai.rs:1488` | Own | `if sq.owner != me \|\| busy.contains(&id) {` |
| `ai.rs:1712` | Own | `\|\| !g.points.iter().any(\|c\| c.owner == Some(self.me))` |
| `ai.rs:1727` | Own | `q.owner == self.me` |
| `ai.rs:1832` | Own | `for sq in g.squads.values().filter(\|sq\| sq.owner == self.me) {` |
| `ai.rs:1849` | Own | `sq.owner == self.me` |
| `bail.rs:238` | Own | `let Some(sq) = self.squads.get(&squad).filter(\|q\| q.owner == p) else {` |
| `bail.rs:256` | Own | `let enemy = ents.owner[vehicle.index()] != p;` |
| `combat.rs:527` | Sees (teams) | `if Some(m.id) == focus \|\| !ctx.game.teams.enemy_seen(owner, m.owner, m.seen_by) {` |
| `combat.rs:2343` | Own+ally (teams) | `if ctx.teams.allied(player, ents.owner[i].0) \|\| id == shooter {` |
| `demolish.rs:114` | Own | `.filter(\|q\| q.owner == p)` |
| `demolish.rs:124` | Own | `q.owner == p \|\| own_men.iter().any(\|&m\| (m - q.pos).length_sq_raw() <= s2)` |
| `demolish.rs:334` | Own | `if owner.0 == he.0 {` |
| `ecs.rs:425` | Own / validation | `&& (self.owner[i] != PlayerId::default()` |
| `fire_mission.rs:538` | Own | `.filter(\|q\| self.squads.get(q).is_some_and(\|c\| c.owner == sq.owner));` |
| `fire_mission.rs:593` | Own (checker after A: an ally's observers do not direct my missions; no shared fire support in S17, S17-5) | `if q.owner != sq.owner {` |
| `fire_mission.rs:810` | Own | `for q in self.squads.values().filter(\|q\| q.owner == p) {` |
| `fire_mission.rs:979` | Own | `q.owner.0 as usize == owner` |
| `fort.rs:456` | Own | `if squads.get(&w.builder).is_none_or(\|q\| q.owner != w.owner) {` |
| `fort.rs:637` | Hostile (teams) | `.is_none_or(\|m\| m.known & teams.known_mask(q.owner.0) == 0)` |
| `fort.rs:685` | Own | `let mine = \|m: &&Mine\| m.owner == owner && m.fort == fort;` |
| `fort.rs:786` | Own | `if w.owner == p && w.fort == f {` |
| `fort.rs:800` | Own | `.filter(\|s\| s.owner == p && s.fort == f)` |
| `fort.rs:806` | Own | `.filter(\|q\| q.owner == p && q.fort == f && q.part == 0)` |
| `fort.rs:816` | Own | `.filter(\|m\| m.owner == p && m.fort == f)` |
| `fort.rs:968` | Own | `s.owner == p` |
| `fort.rs:988` | Own | `.any(\|w\| w.owner == p && w.fort == fort && near(w.line[0]))` |
| `fort.rs:993` | Own | `.any(\|q\| q.owner == p && q.fort == fort && q.part == 0 && near(q.pos))` |
| `fort.rs:1224` | Own | `.is_some_and(\|q\| q.owner == w.owner && q.members.iter().any(\|m\| ents.is_alive(*m)));` |
| `fort.rs:1369` | Own | `c.owner == p` |
| `fort.rs:1399` | Own | `\|\| ents.owner[v.index()] != p` |
| `fort.rs:1573` | Own | `s.owner == owner && s.built && (s.pos - line[0]).length() <= Fx::from_int(5)` |
| `fort.rs:2622` | Own | `if vp == m.owner {` |
| `game.rs:1054` | Own | `.filter(\|id\| ents.owner[id.index()] == player)` |
| `game.rs:1061` | Own | `.filter(\|id\| ents.owner[id.index()] == player)` |
| `game.rs:1068` | Own | `ents.kind[id.index()] == EntityKind::Vehicle && ents.owner[id.index()] == player` |
| `game.rs:1087` | Own | `.filter(\|sq\| sq.owner == player && sq.type_idx == squad_type)` |
| `game.rs:1097` | Own | `sq.owner == player && s.squad_types[sq.type_idx as usize].limit_group == Some(g)` |
| `game.rs:1353` | Own | `&& ents.owner[vehicle.index()] == player` |
| `game.rs:1389` | Own | `.is_some_and(\|s\| s.owner == player && !s.members.is_empty())` |
| `game.rs:2522` | Own / validation | `if self.players.len() != np` |
| `game.rs:2545` | Own | `\|\| (c.capturer.is_some() && c.capturer == c.owner)` |
| `game.rs:2612` | Own | `if self.ai[p].is_some() != s.players[p].is_ai {` |
| `game.rs:2658` | Own | `&& ents.owner[v.index()] == sq.owner` |
| `game.rs:2688` | Own | `\|\| ents.owner[m.index()] != sq.owner` |
| `game.rs:2849` | Own | `.is_none_or(\|d\| d.owner != ents.owner[i]))` |
| `game.rs:3086` | Own | `kind(f) == Some(EntityKind::Soldier) && ents.owner[f.index()] == ents.owner[i]` |
| `gunnery.rs:921` | Own | `if rounds == 0 \|\| !emplaced \|\| ents.owner[i] != sq_owner \|\| !he_left {` |
| `guns.rs:172` | Own | `.is_some_and(\|s\| s.owner != ents.owner[id.index()])` |
| `guns.rs:180` | Own | `\|\| ents.owner[i] != ents.owner[id.index()]` |
| `guns.rs:593` | Own | `&& ents.owner[id.index()] == p` |
| `guns.rs:637` | Own | `&& ents.owner[id.index()] == p` |
| `guns.rs:821` | Own | `ctx.ents.kind.get(ti) == Some(&EntityKind::Vehicle) && ctx.ents.owner[ti] == p` |
| `guns.rs:1066` | Own | `&& self.squads.get(&q).is_some_and(\|s\| s.owner == p)` |
| `guns.rs:1082` | Own | `\|\| ctx.ents.owner[i] != p` |
| `guns.rs:1173` | Own | `&& ctx.ents.owner[i] == ctx.ents.owner[id.index()]` |
| `guns.rs:1488` | Own | `ctx.ents.owner[m.index()] == owner` |
| `guns.rs:1667` | Own | `if !gun_ok \|\| !tow_ok \|\| ctx.ents.owner[gun.index()] != ctx.ents.owner[tow.index()] {` |
| `medics.rs:115` | Own | `\|\| (owners[id.index()] == owner && near(tf[id.index()].pos.xy()))` |
| `medics.rs:119` | Own | `&& owners[j] == owner` |
| `medics.rs:164` | Own | `&& ents.owner[j] == owner` |
| `medics.rs:185` | Own | `&& ents.owner[j] == owner` |
| `medics.rs:250` | Own | `&& ents.owner[k] == owner` |
| `medics.rs:263` | Own | `ents.owner[id.index()] == owner` |
| `medics.rs:273` | Own | `&& st.owner == owner` |
| `medics.rs:303` | Own | `\|\| ents.owner[j] != owner` |
| `medics.rs:476` | Own | `\|\| ctx.ents.owner[id.index()] != owner` |
| `medics.rs:489` | Own | `&& ctx.ents.owner[j] == owner` |
| `medics.rs:536` | Own | `\|\| ctx.ents.owner[j] != owner` |
| `medics.rs:641` | Own | `if t >= ctx.ents.len() \|\| ctx.ents.owner[t] != p {` |
| `medics.rs:703` | Own | `if t >= ctx.ents.len() \|\| ctx.ents.kind[t] != EntityKind::Downed \|\| ctx.ents.owner[...` |
| `medics.rs:766` | Own | `ents.owner[j] == owner` |
| `medics.rs:832` | Own | `&& ents.owner[m.index()] == owner` |
| `medics.rs:1091` | Own | `st.owner == p` |
| `orders.rs:419` | Own | `if stop_squads.last() != Some(&(owner, sid)) {` |
| `projectile.rs:152` | Own / validation | `.any(\|p\| p.weapon == NO_WEAPON && (p.player != 0 \|\| p.ammo != NO_AMMO))` |
| `rules.rs:79` | Own | `.filter(\|(c, _)\| c.owner == Some(player))` |
| `rules.rs:175` | Own | `let held = self.points.iter().filter(\|c\| c.owner == Some(id)).count();` |
| `rules.rs:304` | Own / validation | `*a == 0 && self.players[p].manpower < cheapest` |
| `snapshot.rs:1063` | Presenter (P) | `points_held: g.points().iter().filter(\|c\| c.owner == Some(id)).count() as u8,` |
| `spotting.rs:101` | Own (observers are own men; allies skipped below) | `if ents.owner[i] != p {` |
| `squad.rs:281` | Own | `ents.owner[i].0 == owner` |
| `structures.rs:303` | Own | `for c in self.crates.values().filter(\|c\| c.owner == p) {` |
| `structures.rs:316` | Own | `.filter(\|s\| s.owner == p && setup.stocks[s.def as usize].kind == kind)` |
| `structures.rs:326` | Own | `s.owner == p` |
| `structures.rs:348` | Own | `s.owner == p` |
| `structures.rs:401` | Own / validation | `if d.kind == StockKind::Crate \|\| s.owner.0 as usize >= np {` |
| `structures.rs:423` | Own | `if d.faction != setup.players[s.owner.0 as usize].faction {` |
| `structures.rs:429` | Own | `.is_some_and(\|b\| squads.get(&b).is_none_or(\|q\| q.owner != s.owner))` |
| `structures.rs:444` | Own / validation | `if d.kind != StockKind::Crate \|\| c.owner.0 as usize >= np {` |
| `structures.rs:459` | Own | `\|\| ents.owner[i] != c.owner` |
| `structures.rs:847` | Own | `c.carrier.is_none() && (c.owner == p \|\| c.claim.is_none())` |
| `structures.rs:973` | Own | `&& ents.owner[i] == c.owner` |
| `structures.rs:1115` | Own | `s.owner == p` |
| `structures.rs:1129` | Own | `t.owner == p` |
| `structures.rs:1158` | Own | `c.owner == p` |
| `structures.rs:1326` | Own | `if ents.owner[i] != owner` |
| `structures.rs:1679` | Own | `if ctx.ents.kind.get(i) != Some(&EntityKind::Vehicle) \|\| ctx.ents.owner[i] != p {` |
| `structures.rs:2009` | Own | `let (def, owner, is_hq) = (s.def, s.owner, d.kind == StockKind::Hq);` |
| `structures.rs:2145` | Hostile (teams) | `if allies & (1 << (b.owner % 16)) != 0 \|\| b.reach < Fx::ZERO {` |
| `transport.rs:153` | Own | `&& ents.owner[i] == p` |
| `transport.rs:226` | Own | `if ctx.ents.kind.get(i) != Some(&EntityKind::Vehicle) \|\| ctx.ents.owner[i] != p {` |
| `transport.rs:417` | Own | `if sq.owner != owner \|\| sq.order != (Order::Embark { vehicle: id }) {` |
| `transport.rs:806` | Own | `\|\| ents.owner[i] != ents.owner[id.index()]` |
| `unit.rs:241` | Own | `&& ents.owner[i] == p` |
| `unit.rs:261` | Own | `(i < ents.len() && ents.kind[i] == EntityKind::Vehicle && ents.owner[i] == p)` |
| `vehicle.rs:2206` | Own | `&& ents.owner[id.index()] == player` |
| `vehicle.rs:3044` | Hostile (teams) | `if c.teams.hostile_id(owner, c.ents.owner[si]) \|\| c.ents.kind[si] != EntityKind::Sold...` |
| `world.rs:209` | Own / validation | `if self.players == 0 {` |
| `world.rs:844` | Own / validation | `.any(\|(o, k)\| *k != EntityKind::Vacant && o.0 >= max_owner)` |
| `world.rs:2866` | Own / validation | `self.state.entities.is_alive(unit) && self.state.entities.owner[unit.index()] == player` |
| `wounds.rs:272` | Own | `&& ents.owner()[m.index()] == owner` |
| `wounds.rs:329` | Own | `\|\| ents.owner()[b.man.index()] != st.owner` |
| `ai/ammo.rs:92` | Own | `g.squads.get(id).is_some_and(\|s\| s.owner == me)` |
| `ai/ammo.rs:243` | Own | `.is_none_or(\|sq\| sq.owner != me \|\| sq.order == Order::Retreat);` |
| `ai/armour.rs:83` | Own | `x.owner == self.me` |
| `ai/armour.rs:99` | Own | `x.owner == self.me` |
| `ai/armour.rs:248` | Own | `c.owner == self.me` |
| `ai/armour.rs:313` | Own | `&& self.ents.owner[v.index()] == self.me` |
| `ai/armour.rs:376` | Own | `if self.ents.owner[v.index()] == self.me {` |
| `ai/arty.rs:87` | Own | `\|id: &SquadId\| id.0 < g.next_squad && g.squads.get(id).is_none_or(\|sq\| sq.owner == ...` |
| `ai/arty.rs:206` | Hostile (teams) | `if g.teams.hostile_id(me, q.owner) \|\| Some(id) == firing \|\| q.members.is_empty() {` |
| `ai/arty.rs:241` | Own | `let sq = g.squads.get(&id).filter(\|q\| q.owner == self.me)?;` |
| `ai/arty.rs:491` | Own | `.is_some_and(\|q\| q.owner == self.me && !q.members.is_empty())` |
| `ai/arty.rs:501` | Own | `if q.owner != self.me \|\| q.members.is_empty() {` |
| `ai/arty.rs:535` | Own | `.filter(\|id\| self.ents.owner[id.index()] == self.me)` |
| `ai/arty.rs:982` | Own | `.filter(\|(id, q)\| q.owner == self.me && !self.arty_busy(**id))` |
| `ai/arty.rs:1015` | Own | `q.owner == self.me && !self.ai.arty.contains_key(id) && !self.arty_busy(**id)` |
| `ai/arty.rs:1048` | Own | `.filter(\|q\| q.owner == self.me && !q.members.is_empty() && self.is_fo_type(q.type_idx))` |
| `ai/assault.rs:466` | Own | `&& g.setup.players.iter().any(\|p\| p.faction == t.faction)` |
| `ai/assault.rs:606` | Own | `.is_none_or(\|sq\| sq.owner != self.me \|\| sq.order == Order::Retreat);` |
| `ai/breach.rs:161` | Own | `ents.owner[i] == me && ents.unit[i].riding.is_none()` |
| `ai/breach.rs:237` | Own | `.any(\|q\| q.owner == me && !q.members.is_empty() && opens_wire(s, q.type_idx))` |
| `ai/breach.rs:456` | Own | `if busy.contains(&id) && g.squads.get(&id).is_some_and(\|q\| q.owner == me) {` |
| `ai/breach.rs:514` | Own | `let taken = g.points[job.point as usize].owner == Some(me);` |
| `ai/breach.rs:617` | Own | `.any(\|c\| c.owner == me && (c.to - job.crossing).length_sq_raw() <= near);` |
| `ai/breach.rs:673` | Own | `.filter(\|q\| q.owner == self.me)` |
| `ai/breach.rs:936` | Own | `let sq = g.squads.get(&id).filter(\|q\| q.owner == me)?;` |
| `ai/breach.rs:1034` | Own | `q.owner == me` |
| `ai/demolish.rs:145` | Own | `if q.owner != self.me \|\| q.members.is_empty() {` |
| `ai/demolish.rs:384` | Own | `(q.owner == self.me` |
| `ai/demolish.rs:409` | Own | `.is_some_and(\|q\| q.owner == me && !q.members.is_empty());` |
| `ai/demolish.rs:666` | Own | `.filter(\|q\| q.owner == me && !q.members.is_empty() && !busy.contains(&o))` |
| `ai/fort.rs:475` | Own | `.filter(\|(_, (c, _))\| c.owner == Some(me))` |
| `ai/fort.rs:775` | Own | `.any(\|m\| m.owner == me && m.fort == f && near(m.pos, p, r))` |
| `ai/fort.rs:780` | Own | `.any(\|w\| w.owner == me && w.fort == f && near((w.a + w.b).div_int(2), p, r))` |
| `ai/fort.rs:1042` | Foe (A1/A6: the main foe only) | `.filter(\|&j\| j != i && g.points.get(j).is_some_and(\|c\| c.owner == Some(enemy)))` |
| `ai/fort.rs:1281` | Own | `.filter(\|m\| m.owner == me && m.fort == f && near(m.pos, pos, r))` |
| `ai/fort.rs:1404` | Own | `.any(\|q\| q.owner == me && q.fort == lf && q.part == 0 && near(q.pos, p, r))` |
| `ai/fort.rs:1692` | Own | `q.owner == me` |
| `ai/fort.rs:1743` | Own | `g.points.get(j).is_some_and(\|c\| c.owner == Some(self.me))` |
| `ai/fort.rs:1890` | Own | `w.owner == me && w.fort == f && {` |
| `ai/fort.rs:1897` | Own | `.any(\|q\| q.owner == me && q.fort == f && q.part == 0 && near(q.pos, at, r))` |
| `ai/fort.rs:1899` | Own | `s.owner == me && s.fort == f && {` |
| `ai/fort.rs:1907` | Own | `.any(\|m\| m.owner == me && m.fort == f && near(m.pos, at, r) && !on_lane(m.pos))` |
| `ai/fort.rs:1923` | Own | `.any(\|q\| q.owner == me && q.fort == f && q.part == 0 && near(q.pos, at, r))` |
| `ai/fort.rs:1996` | Own | `.filter(\|m\| m.owner == me && m.fort == f)` |
| `ai/fort.rs:2002` | Own | `.filter(\|w\| w.owner == me && w.fort == f)` |
| `ai/fort.rs:2062` | Own | `.any(\|m\| m.owner == me && m.fort == f && near(m.pos, at, r))` |
| `ai/fort.rs:2067` | Own | `.any(\|w\| w.owner == me && w.fort == f && near(mid_of(d.shape, &w.line), at, r))` |
| `ai/fort.rs:2087` | Own | `m.owner == me && m.fort == f && crate::fort::near_segment(m.pos, a, b, step.div_int(2))` |
| `ai/fort.rs:2092` | Own | `.any(\|w\| w.owner == me && w.fort == f && near(mid_of(d.shape, &w.line), mid, r))` |
| `ai/fort.rs:2119` | Own | `.filter(\|m\| m.owner == me && m.fort == f && near(m.pos, at, r))` |
| `ai/fort.rs:2125` | Own | `.filter(\|w\| w.owner == me && w.fort == f && near(mid_of(d.shape, &w.line), at, r))` |
| `ai/fort.rs:2323` | Own | `q.owner == self.me` |
| `ai/fort.rs:2443` | Own | `c.owner == me` |
| `ai/fort.rs:2456` | Own | `&& self.ents.owner[v.index()] == me` |
| `ai/fort.rs:2608` | Own | `if c.owner != Some(self.me) {` |
| `ai/fort.rs:2708` | Own | `.filter(\|q\| q.owner == me && q.fort == d && q.part == 0 && near(q.pos, site, r))` |
| `ai/fort.rs:2857` | Own | `for m in fs.mines.values().filter(\|m\| m.owner == me) {` |
| `ai/fort.rs:2877` | Own | `.filter(\|w\| w.owner == me && s.forts.get(w.fort as usize).is_some_and(s10b_fort))` |
| `ai/fort.rs:3108` | Own | `let work = g.fort.works.get(&wid).filter(\|w\| w.owner == me);` |
| `ai/fort.rs:3212` | Own | `(w.owner == me` |
| `ai/fort.rs:3255` | Own | `.is_some_and(\|(sq, _)\| g.squads.get(&sq).is_none_or(\|q\| q.owner != me))` |
| `ai/fort.rs:3275` | Own | `let owned = g.squads.get(&id).is_some_and(\|q\| q.owner == me);` |
| `ai/fort.rs:3281` | Own | `if busy.contains(&id) && g.squads.get(&id).is_some_and(\|q\| q.owner == me) {` |
| `ai/fort.rs:3316` | Own | `let owner_lost = job.point != NO_POINT` |
| `ai/fort.rs:3319` | Own | `.is_none_or(\|c\| c.owner != Some(me));` |
| `ai/fort.rs:3404` | Own | `let gone = g.fort.works.get(&wid).is_none_or(\|w\| w.owner != me);` |
| `ai/fort.rs:3870` | Own | `if q.owner != me` |
| `ai/fort.rs:3892` | Own | `if p.owner == me` |
| `ai/fort.rs:3952` | Own | `.filter(\|q\| q.owner == me && !q.members.is_empty())` |
| `ai/fort.rs:4026` | Own | `.filter(\|q\| q.owner == me && !q.members.is_empty())?;` |
| `ai/fort.rs:4556` | Own | `w.owner == me` |
| `ai/fort.rs:4638` | Own | `w.owner == me && w.fort == job.fort && near((w.a + w.b).div_int(2), end, r)` |
| `ai/fort.rs:4683` | Own | `.any(\|m\| m.owner == me && m.fort == job.fort && near(m.pos, end, r))` |
| `ai/fort.rs:4687` | Own | `.any(\|q\| q.owner == me && q.fort == job.fort && near(q.pos, end, r)))` |
| `ai/fort.rs:4708` | Own | `st.owner == me` |
| `ai/fort.rs:4947` | Own | `.find(\|(_, w)\| w.owner == me && w.builder == id && w.fort == job.fort);` |
| `ai/fort.rs:4985` | Own | `.filter(\|w\| w.owner == me && w.builder == id)` |
| `ai/guns.rs:425` | Own | `&& self.ents.owner[id.index()] == me` |
| `ai/guns.rs:455` | Own | `s.owner == me` |
| `ai/guns.rs:836` | Own | `&& self.ents.owner[id.index()] == me` |
| `ai/guns.rs:881` | Own | `.filter(\|x\| x.owner == self.me)` |
| `ai/medic.rs:111` | Own | `.filter(\|(_, q)\| q.owner == me && s.type_has_tag(q.type_idx, "medic"))` |
| `ai/medic.rs:200` | Own | `.find(\|(_, st)\| st.owner == self.me && s.stocks[st.def as usize].kind == StockKind::Aid)` |
| `ai/medic.rs:245` | Own | `c.bleed_out_at != 0 && c.carriers[0].is_none() && self.ents.owner[id.index()] == self.me` |
| `ai/medic.rs:319` | Own | `q.owner == me` |
| `ai/medic.rs:333` | Own | `.filter(\|id\| self.ents.owner[id.index()] == me)` |
| `ai/medic.rs:418` | Own | `st.owner == self.me && st.built && s.stocks[st.def as usize].kind == StockKind::Hq` |
| `ai/medic.rs:500` | Own | `self.ents.owner[id.index()] == self.me` |
| `ai/medic.rs:508` | Own | `.any(\|m\| self.ents.owner[m.index()] == self.me && self.patient_at(m, p, r))` |
| `ai/medic.rs:517` | Own | `if self.ents.owner[i] != self.me` |
| `ai/medic.rs:552` | Own | `if sq.owner != me \|\| busy.contains(&id) {` |
| `ai/medic.rs:638` | Own | `self.ents.owner[m.index()] == me` |
| `ai/medic.rs:651` | Own | `self.ents.owner[id.index()] == me` |
| `ai/medic.rs:722` | Own | `if self.ents.owner[id.index()] != me` |
| `ai/medic.rs:791` | Own | `&& g.squads.get(id).is_some_and(\|q\| q.owner == self.me)` |
| `ai/medic.rs:828` | Own | `if sq.owner != me` |
| `ai/mortar.rs:189` | Own | `.retain(\|id, _\| g.squads.get(id).is_some_and(\|q\| q.owner == self.me));` |
| `ai/obstacles.rs:235` | Own | `q.owner == player` |
| `ai/obstacles.rs:347` | Own | `matches!(k, EntityKind::Soldier \| EntityKind::Vehicle) && ents.owner()[i] == me` |
| `ai/supply.rs:76` | Own | `.filter(\|(id, sq)\| sq.owner == me && is(**id, hq_tag))` |
| `ai/supply.rs:82` | Own | `.filter(\|(id, sq)\| sq.owner == me && is(**id, bearer_tag))` |
| `ai/supply.rs:91` | Own | `.find(\|(_, x)\| x.owner == me && s.stocks[x.def as usize].kind == StockKind::Hq)` |
| `ai/supply.rs:221` | Own | `.filter(\|&k\| g.points[k].owner == Some(me))` |
| `ai/supply.rs:508` | Own | `q.owner == me` |
| `ai/supply.rs:575` | Own | `x.owner == self.me` |
| `ai/supply.rs:592` | Own | `q.owner == self.me && s.squad_types[q.type_idx as usize].has_tag(Some(t)) && {` |
| `ai/supply.rs:629` | Own | `q.owner == me` |
| `ai/supply.rs:642` | Own | `if c.owner != me` |
| `ai/sweep.rs:135` | Hostile (teams) | `.filter(\|m\| self.g.teams.hostile_id(self.me, m.owner) && m.known & kmask != 0)` |
| `ai/sweep.rs:196` | Hostile (teams) | `self.g.teams.hostile_id(self.me, x.owner) && x.pos == q && x.tick > since` |
| `ai/sweep.rs:565` | Own | `.is_some_and(\|q\| q.owner == me && !q.members.is_empty());` |
| `ai/sweep.rs:735` | Own | `.filter(\|q\| q.owner == me && !q.members.is_empty())?;` |
| `ai/sweep.rs:776` | Own | `let Some(q) = g.squads.get(&o).filter(\|q\| q.owner == me) else {` |
| `ai/sweep.rs:821` | Own | `.is_some_and(\|q\| q.owner == me && !q.members.is_empty())` |
| `ai/transport.rs:136` | Own | `s.owner == me` |
| `ai/transport.rs:716` | Own | `sq.owner == self.me` |
| `ai/truck.rs:139` | Own | `s.owner == me` |
| `ai/truck.rs:187` | Own | `.filter(\|q\| q.owner == self.me && !q.members.is_empty())?;` |
| `ai/truck.rs:436` | Own | `&& self.ents.owner[c.index()] == self.me` |
| `ai/truck.rs:558` | Own | `if sq.owner != self.me` |
| `ai/truck.rs:647` | Own | `c.owner == me` |
| `ai/truck.rs:661` | Own | `x.owner == me && x.built && s.stocks[x.def as usize].kind == StockKind::Hq` |
| `ai/truck.rs:668` | Own | `q.owner == me` |
| `ai/truck.rs:698` | Own | `x.owner == me && x.built && s.stocks[x.def as usize].kind == StockKind::Hq` |
| `ai/truck.rs:753` | Own | `for sq in g.squads.values().filter(\|q\| q.owner == self.me) {` |
| `ai/truck.rs:844` | Own | `(self.ents.owner[i] == self.me` |
| `ai/truck.rs:882` | Own | `x.owner == self.me && x.built && s.stocks[x.def as usize].kind == StockKind::Hq` |
| `ai/truck.rs:910` | Own | `.filter(\|q\| q.owner == self.me && is_role(q.type_idx) && !q.members.is_empty())` |
| `ai.rs:1630` | Sees (teams) | `teams.enemy_seen(me, self.ents.owner[i].0, self.ents.combat[i].seen_by)` |
| `ai.rs:2763` | Sees (teams) | `&& teams.enemy_seen(me, self.ents.owner[i].0, self.ents.combat[i].seen_by)` |
| `bail.rs:469` | Own bit (spotting writes / reads its own bit) | `ctx.ents.combat[i].seen_by = ((1u32 << self.players.len()) - 1) as u16;` |
| `bail.rs:982` | Own bit (spotting writes / reads its own bit) | `ctx.ents.combat[i].seen_by = 1 << p.0;` |
| `combat.rs:254` | Carry (copy of the mask) | `pub seen_by: u16,` |
| `combat.rs:279` | Carry (copy of the mask) | `pub seen_by: u16,` |
| `combat.rs:290` | Carry (copy of the mask) | `seen_by: ents.combat[i].seen_by,` |
| `combat.rs:313` | Sees (teams) | `if reach_tab.is_empty() \|\| !game.teams.enemy_seen(owner, m.owner, m.seen_by) {` |
| `combat.rs:398` | Carry (copy of the mask) | `seen_by: c.seen_by,` |
| `combat.rs:422` | Sees (teams) | `(teams.enemy_seen(owner, m.owner, m.seen_by) && (m.pos.xy() - from).length_sq_raw() <= ...` |
| `combat.rs:456` | Sees (teams) | `.filter(\|m\| teams.enemy_seen(owner, m.owner, m.seen_by))` |
| `combat.rs:527` | Sees (teams) | `if Some(m.id) == focus \|\| !ctx.game.teams.enemy_seen(owner, m.owner, m.seen_by) {` |
| `combat.rs:1419` | Sees (teams) | `&& self.teams.sees(owner.0, ents.combat[ti].seen_by)` |
| `combat.rs:1844` | Sees (teams) | `\|\| !self.teams.sees(owner.0, ents.combat[ti].seen_by)` |
| `dig.rs:161` | Sees (teams) | `teams.enemy_seen(owner.0, ents.owner[i].0, ents.combat[i].seen_by)` |
| `ecs.rs:236` | Carry (copy of the mask) | `pub seen_by: u16,` |
| `fire_mission.rs:798` | Own bit | `.enemy_seen(p.0, ents.owner[i].0, ents.combat[i].seen_by)` |
| `fire_mission.rs:849` | Sees (teams) | `\|\| !teams.enemy_seen(p as u8, ents.owner[i].0, ents.combat[i].seen_by)` |
| `game.rs:2868` | Validation | `let bad = c.seen_by >> np != 0` |
| `game.rs:2881` | Carry (copy of the mask) | `seen_by: c.seen_by,` |
| `gunnery.rs:936` | Sees (teams) | `&& teams.enemy_seen(sq_owner.0, ents.owner[ti].0, ents.combat[ti].seen_by)` |
| `gunnery.rs:944` | Sees (teams) | `&& teams.enemy_seen(sq_owner.0, ents.owner[ti].0, ents.combat[ti].seen_by)` |
| `gunnery.rs:1293` | Sees (teams) | `let seen = self.teams.sees(owner.0, ents.combat[t.index()].seen_by);` |
| `gunnery.rs:1399` | Own bit | `ents.combat[ti].seen_by,` |
| `gunnery.rs:1444` | Sees (teams) | `if !self.teams.enemy_seen(owner.0, m.owner, m.seen_by) {` |
| `gunnery.rs:1526` | Sees (teams) | `self.teams.enemy_seen(owner.0, m.owner, m.seen_by)` |
| `guns.rs:1571` | Own bit (spotting writes / reads its own bit) | `ctx.ents.combat[i].seen_by = 1 << p.0;` |
| `hash.rs:130` | Hash | `self.seen_by.put(out);` |
| `orders.rs:129` | Own bit | `ctx.ents.combat[ti].seen_by,` |
| `orders.rs:350` | Sees (teams) | `if !self.teams.sees(owner.0, ctx.ents.combat[ti].seen_by) {` |
| `orders.rs:463` | Own bit | `ctx.ents.combat[j].seen_by,` |
| `snapshot.rs:149` | Presenter (P) | `pub seen_by: u16,` |
| `snapshot.rs:161` | Presenter (P) | `pub seen_by: u16,` |
| `snapshot.rs:349` | Presenter (P) | `pub seen_by: u16,` |
| `snapshot.rs:848` | Presenter (P) | `self.soldier_visible.push(c.seen_by \| (1 << owner));` |
| `snapshot.rs:933` | Presenter (P) | `seen_by: c.seen_by,` |
| `snapshot.rs:950` | Presenter (P) | `seen_by: c.seen_by,` |
| `snapshot.rs:1402` | Presenter (P) | `seen_by: if wreck \|\| abandoned {` |
| `snapshot.rs:1405` | Presenter (P) | `ents.combat()[i].seen_by \| (1 << owner)` |
| `spotting.rs:220` | Own bit (spotting writes / reads its own bit) | `if seen != (ents.combat[i].seen_by & (1 << p.0) != 0) {` |
| `spotting.rs:230` | Own bit (spotting writes / reads its own bit) | `ents.combat[r.vehicle.index()].seen_by & (1 << p.0) != 0 && game.rider_exposed(r);` |
| `spotting.rs:231` | Own bit (spotting writes / reads its own bit) | `if seen != (c.seen_by & (1 << p.0) != 0) {` |
| `spotting.rs:291` | Own bit (spotting writes / reads its own bit) | `if seen != (c.seen_by & (1 << p.0) != 0) {` |
| `squad.rs:293` | Sees (teams) | `teams.enemy_seen(owner, ents.owner[i].0, ents.combat[i].seen_by)` |
| `vehicle.rs:2436` | Own bit (spotting writes / reads its own bit) | `ents.combat[i].seen_by = ((1u32 << self.players.len()) - 1) as u16;` |
| `world.rs:1088` | Own bit (spotting writes / reads its own bit) | `c.seen_by \|= 1 << p;` |
| `world.rs:1090` | Own bit (spotting writes / reads its own bit) | `c.seen_by &= !(1u16 << p);` |
| `ai.rs:2400` | Own+ally (teams) (S17b A3: ally squads at a point) | `.filter(\|sq\| sq.owner != self.me && g.teams.allied(self.me.0, sq.owner.0))` |
| `teams.rs:146` | Teams API | `pub fn sees(&self, p: u8, seen_by: u16) -> bool {` |
| `teams.rs:147` | Teams API | `seen_by & self.sight[ix(p)] != 0` |
| `teams.rs:153` | Teams API | `pub fn enemy_seen(&self, p: u8, t_owner: u8, seen_by: u16) -> bool {` |
| `teams.rs:154` | Teams API | `self.hostile(p, t_owner) && self.sees(p, seen_by)` |
