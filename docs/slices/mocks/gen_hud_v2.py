#!/usr/bin/env python3
"""Generate HUD mock v2.1 (light console, floating radar minimap top-left).

Usage: gen_hud_v2.py OUTDIR
Writes hud-mock-v2.svg, hud-mock-v2-1280.svg (clean, no notes) and
hud-mock-v2-annotated.svg (1600, with designer notes).
"""
import math, sys, random

ACC = "#e0ad4a"       # single accent (amber)
TXT = "#eceee6"
DIM = "#c3c9bb"
FRI = "#7fb0d8"
ENE = "#cf7a6a"
PANEL = "rgba(14,18,20,0.46)"      # roster cards, detail panel
BTN = "rgba(14,18,20,0.40)"        # orders buttons, stance strip
ABIL = "rgba(52,40,16,0.50)"       # ability row: slight warm tint
STRIP = "rgba(14,18,20,0.38)"      # call-in shared strip
FONT = "Helvetica, Arial, sans-serif"
OFF_TXT = "rgba(236,238,230,0.50)"
OFF_ACC = "rgba(224,173,74,0.50)"


def esc(s):
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


class S:
    def __init__(self, W, H):
        self.W, self.H = W, H
        self.o = []
        self.rects = {}

    def add(self, s): self.o.append(s)

    def rect(self, x, y, w, h, fill, rx=0, stroke=None, sw=1, extra=""):
        st = f' stroke="{stroke}" stroke-width="{sw}"' if stroke else ""
        self.add(f'<rect x="{x:.1f}" y="{y:.1f}" width="{w:.1f}" height="{h:.1f}" rx="{rx}" fill="{fill}"{st} {extra}/>')

    def text(self, x, y, s, size=14, fill=TXT, anchor="start", weight="normal", extra=""):
        self.add(f'<text x="{x:.1f}" y="{y:.1f}" font-size="{size}" fill="{fill}" text-anchor="{anchor}" font-weight="{weight}" {extra}>{esc(s)}</text>')

    def reg(self, name, x, y, w, h): self.rects[name] = (x, y, w, h)


def tw(s, size, bold=False):
    return len(s) * size * (0.60 if bold else 0.54)


def pip(s, x, y, size, st):
    """Pip states are shape-coded, not colour-coded."""
    if st == "sel":      # selected: bright solid fill
        s.rect(x, y, size, size, "#f6f6f0", rx=2)
    elif st == "a":      # alive: mid fill
        s.rect(x, y, size, size, "rgba(236,238,230,0.40)", rx=2)
    elif st == "w":      # wounded: outline + lower half filled
        s.rect(x + .5, y + .5, size - 1, size - 1, "rgba(236,238,230,0.08)", rx=2, stroke="rgba(236,238,230,0.85)", sw=1.2)
        s.rect(x + .5, y + size / 2, size - 1, size / 2 - .5, "rgba(236,238,230,0.85)")
    elif st == "pin":    # pinned: hatch + solid outline
        s.rect(x + .5, y + .5, size - 1, size - 1, "url(#hatch)", rx=2, stroke="rgba(236,238,230,0.85)", sw=1.2)
    elif st == "det":    # detached: heavy dashed outline, empty
        s.rect(x + 1, y + 1, size - 2, size - 2, "none", rx=2, stroke="#eceee6", sw=2, extra='stroke-dasharray="4 2.6"')
    elif st == "x":      # dead: X, no box
        s.add(f'<path d="M {x+2} {y+2} L {x+size-2} {y+size-2} M {x+size-2} {y+2} L {x+2} {y+size-2}" stroke="rgba(236,238,230,0.75)" stroke-width="2.2"/>')


def clock(s, x, y, r=5.5, col=TXT):
    s.add(f'<circle cx="{x:.1f}" cy="{y:.1f}" r="{r}" fill="none" stroke="{col}" stroke-width="1.4"/>'
          f'<path d="M {x:.1f} {y-r+2:.1f} L {x:.1f} {y:.1f} L {x+r-2:.1f} {y:.1f}" stroke="{col}" stroke-width="1.4" fill="none"/>')


def build(W, H, P, notes=False):
    s = S(W, H)
    s.add(f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" width="{W}" height="{H}" font-family="{FONT}">')
    s.add(f'<title>Frontline RTS HUD mock v2.1 {W}x{H}</title>')
    s.add('<defs>'
          '<pattern id="hatch" width="4" height="4" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">'
          f'<rect width="4" height="4" fill="rgba(0,0,0,0)"/><line x1="0" y1="0" x2="0" y2="4" stroke="{TXT}" stroke-width="1.6"/></pattern>'
          '<pattern id="offmap" width="7" height="7" patternUnits="userSpaceOnUse" patternTransform="rotate(-45)">'
          '<rect width="7" height="7" fill="#2a3127"/><line x1="0" y1="0" x2="0" y2="7" stroke="#55604c" stroke-width="1.5"/></pattern>'
          '<linearGradient id="botfade" x1="0" y1="0" x2="0" y2="1">'
          '<stop offset="0" stop-color="#0c1010" stop-opacity="0"/><stop offset="1" stop-color="#0c1010" stop-opacity="0.26"/></linearGradient>'
          '<radialGradient id="mmfade"><stop offset="0.82" stop-color="#fff" stop-opacity="1"/><stop offset="1" stop-color="#fff" stop-opacity="0.45"/></radialGradient>'
          '</defs>')
    k = W / 1600; kv = H / 900

    # ---------------- terrain ----------------
    s.rect(0, 0, W, H, "#7f8c69")
    for (cx, cy, rx, ry, c, op) in [(380, 300, 260, 140, "#76835f", .7), (1060, 470, 300, 150, "#748160", .65),
                                    (260, 600, 220, 110, "#86926f", .6), (1300, 230, 240, 100, "#76835f", .6),
                                    (760, 180, 200, 70, "#8a9472", .5), (820, 700, 300, 90, "#76835f", .5)]:
        s.add(f'<ellipse cx="{cx*k:.0f}" cy="{cy*kv:.0f}" rx="{rx*k:.0f}" ry="{ry*kv:.0f}" fill="{c}" opacity="{op}"/>')
    s.add(f'<path d="M -20 {640*kv:.0f} C {400*k:.0f} {560*kv:.0f}, {700*k:.0f} {430*kv:.0f}, {1000*k:.0f} {380*kv:.0f} S {1450*k:.0f} {160*kv:.0f}, {W+20} {90*kv:.0f}" '
          'fill="none" stroke="#958f72" stroke-width="18" opacity="0.55"/>')
    rnd = random.Random(4)
    for (gx, gy) in [(470, 240), (1180, 560), (300, 470), (1400, 380), (650, 120)]:
        for _ in range(6):
            x = (gx + rnd.uniform(-45, 45)) * k; y = (gy + rnd.uniform(-28, 28)) * kv
            s.add(f'<circle cx="{x:.0f}" cy="{y:.0f}" r="{rnd.uniform(9,15)*k:.0f}" fill="#5f6f4d" opacity="0.75"/>')
    s.rect(900*k, 300*kv, 46*k, 30*k, "#6f6a58", extra=f'transform="rotate(-18 {923*k:.0f} {315*kv:.0f})"')
    # friendly squads on the battlefield: Rifle 1 (selected, 8 men), Rifle 2 (6 alive, 1 detached), Engineers (5)
    field = [((600, 560), 8, True, 0), ((870, 470), 5, False, 1), ((1100, 385), 5, False, 2)]
    for (cx, cy), n, sel, i in field:
        cx *= k; cy *= kv
        s.add(f'<ellipse cx="{cx:.0f}" cy="{cy:.0f}" rx="{44*k:.0f}" ry="{30*k:.0f}" fill="none" stroke="{ACC if sel else "rgba(240,240,230,0.55)"}" stroke-width="{2 if sel else 1.5}"/>')
        for j in range(n):
            a = j * (2 * math.pi / n) + i
            s.add(f'<circle cx="{cx+math.cos(a)*26*k:.0f}" cy="{cy+math.sin(a)*15*k:.0f}" r="4" fill="{FRI}" stroke="#1d2a33" stroke-width="1"/>')
    # Rifle 2's detached man, away from his squad ring
    s.add(f'<circle cx="{955*k:.0f}" cy="{512*kv:.0f}" r="4" fill="{FRI}" stroke="#1d2a33" stroke-width="1"/>')
    for (cx, cy) in [(980, 128), (1130, 96), (1275, 70), (1210, 150)]:
        cx *= k; cy *= kv
        for j in range(4):
            s.add(f'<circle cx="{cx+j*9-14:.0f}" cy="{cy+(j%2)*6:.0f}" r="4" fill="{ENE}" stroke="#3a1f1a" stroke-width="1"/>')
    s.add(f'<path d="M {600*k:.0f} {560*kv:.0f} L {700*k:.0f} {505*kv:.0f}" stroke="{ACC}" stroke-width="2" stroke-dasharray="6 5" fill="none"/>')
    s.add(f'<circle cx="{700*k:.0f}" cy="{505*kv:.0f}" r="5" fill="none" stroke="{ACC}" stroke-width="2"/>')
    s.rect(0, H - P["fade"], W, P["fade"], "url(#botfade)")

    # ---------------- top bar (one strip, chips inline) ----------------
    TB = P["tb"]
    s.rect(0, 0, W, TB, "rgba(12,16,18,0.55)")
    s.reg("Top bar", 0, 0, W, TB)
    by = TB / 2 + 5
    x = 12
    s.text(x, by, P["faction"], 14, TXT, weight="bold"); x += len(P["faction"]) * 11 + 16
    s.text(x, by, "MP", 13, DIM); x += 26
    s.text(x, by, "235", 14, TXT, weight="bold"); x += 34
    s.text(x, by, "+3.3/s", 14, DIM); x += 58
    s.text(x, by, "POP", 13, DIM); x += 34
    s.text(x, by, "31/120", 14, TXT, weight="bold")
    cw, cg, n = P["chip_w"], 4, 5
    chips_w = n * cw + (n - 1) * cg
    c0 = W / 2 - chips_w / 2
    bw = P["ticket_w"]
    cy0, ch = 3, TB - 6
    s.rect(c0 - 10 - bw, by - 9, bw, 8, "rgba(0,0,0,0.35)", rx=2)
    s.rect(c0 - 10 - bw, by - 9, bw * 0.92, 8, FRI, rx=2)
    s.text(c0 - 18 - bw, by, "460", 14, TXT, "end", "bold")
    s.rect(c0 + chips_w + 10, by - 9, bw, 8, "rgba(0,0,0,0.35)", rx=2)
    s.rect(c0 + chips_w + 10, by - 9, bw, 8, ENE, rx=2)
    s.text(c0 + chips_w + 18 + bw, by, "500", 14, TXT, "start", "bold")
    chips = [("A", FRI, 1.0), ("B", None, 0.0), ("C", FRI, 0.6), ("D", ENE, 1.0), ("E", ENE, 0.35)]
    for i, (L, col, p) in enumerate(chips):
        x = c0 + i * (cw + cg)
        s.rect(x, cy0, cw, ch, "rgba(255,255,255,0.10)", rx=3)
        if col and p > 0:
            s.rect(x, cy0, cw * p, ch, col, rx=3, extra='opacity="0.75"')
        if L == "C":
            s.rect(x + cw * p, cy0, cw * (1 - p), ch, "url(#hatch)", extra='opacity="0.5"')
            s.rect(x + .5, cy0 + .5, cw - 1, ch - 1, "none", rx=3, stroke=ACC, sw=1.5)
        s.text(x + cw / 2, by, L, 14, "#ffffff", "middle", "bold")
    s.text(W - 112, by, "1:14", 14, TXT, "end", "bold")
    s.rect(W - 100, 3, 88, TB - 6, "rgba(255,255,255,0.10)", rx=3)
    s.text(W - 56, by, "Menu  Esc", 13, TXT, "middle")

    # ---------------- minimap: radar disc, top-left ----------------
    D = P["mm_d"]; r = D / 2
    mx, my = 12, TB + 8
    cx, cy = mx + r, my + r
    s.reg("Minimap (radar disc)", mx, my, D, D)
    yaw = 30                         # camera yaw, degrees clockwise from north
    R_m = 0.56                       # disc radius in map units (map = 1 x 1)
    S_px = r / R_m                   # map side in px at this zoom
    cam_u, cam_v = -0.06, 0.10       # camera look-at point, map units from map centre
    s.add(f'<clipPath id="mmclip"><circle cx="{cx}" cy="{cy}" r="{r}"/></clipPath>')
    s.add(f'<mask id="mmmask"><circle cx="{cx}" cy="{cy}" r="{r}" fill="url(#mmfade)"/></mask>')
    s.add(f'<g clip-path="url(#mmclip)" mask="url(#mmmask)">')
    s.add(f'<circle cx="{cx}" cy="{cy}" r="{r}" fill="url(#offmap)" opacity="0.85"/>')   # deliberate off-map zone
    s.add(f'<g transform="rotate({-yaw} {cx} {cy})">')
    m0x = cx - (0.5 + cam_u) * S_px; m0y = cy - (0.5 + cam_v) * S_px
    def W2(u, v): return m0x + u * S_px, m0y + v * S_px     # map (0..1) -> rotated-group px
    s.rect(m0x, m0y, S_px, S_px, "rgba(124,138,100,0.95)")
    for (u, v, a, b) in [(.3, .7, .25, .14), (.7, .35, .22, .12), (.55, .82, .2, .08), (.25, .2, .15, .1), (.8, .8, .14, .1)]:
        x, y = W2(u, v)
        s.add(f'<ellipse cx="{x:.1f}" cy="{y:.1f}" rx="{a*S_px:.1f}" ry="{b*S_px:.1f}" fill="#69785a" opacity="0.8"/>')
    x1, y1 = W2(0, .72); x2, y2 = W2(.4, .6); x3, y3 = W2(.6, .45); x4, y4 = W2(1, .18)
    s.add(f'<path d="M {x1:.1f} {y1:.1f} C {x2:.1f} {y2:.1f}, {x3:.1f} {y3:.1f}, {x4:.1f} {y4:.1f}" stroke="#a49c7c" stroke-width="3" fill="none"/>')
    s.rect(m0x, m0y, S_px, S_px, "none", stroke="rgba(236,238,230,0.55)", sw=1.5)          # map edge
    s.add('</g></g>')
    s.add(f'<circle cx="{cx}" cy="{cy}" r="{r-0.5}" fill="none" stroke="rgba(236,238,230,0.25)" stroke-width="1"/>')
    # Units and points are placed in view space (already rotated): up = camera forward.
    def V(fx, fy): return cx + fx * r, cy + fy * r
    for (fx, fy) in [(-.10, .16), (.02, .05), (.14, -.05), (-.40, .42), (-.30, .48)]:
        x, y = V(fx, fy); s.add(f'<rect x="{x-3:.1f}" y="{y-3:.1f}" width="6" height="6" fill="{FRI}" stroke="#13202a" stroke-width="1"/>')
    for (fx, fy) in [(-.02, -.36), (.10, -.42), (.24, -.47), (.18, -.32)]:
        x, y = V(fx, fy); s.add(f'<rect x="{x-3:.1f}" y="{y-3:.1f}" width="6" height="6" fill="{ENE}" stroke="#2c1612" stroke-width="1"/>')
    for (L, fx, fy, col) in [("A", -.48, .30, FRI), ("B", -.56, -.24, None), ("C", .30, -.14, ACC), ("D", .30, -.70, ENE), ("E", .68, -.18, ENE)]:
        x, y = V(fx, fy)
        s.add(f'<circle cx="{x:.1f}" cy="{y:.1f}" r="{P["mm_pt"]}" fill="rgba(14,18,20,0.75)" stroke="{col or DIM}" stroke-width="2"/>')
        s.text(x, y + 5, L, 13, TXT, "middle", "bold")
    # camera frustum: centred on the disc, always pointing up
    nw, fw, fl = P["fr"]
    ny0 = cy + fl * 0.45; fy0 = cy - fl * 0.55
    s.add(f'<path d="M {cx-nw:.1f} {ny0:.1f} L {cx+nw:.1f} {ny0:.1f} L {cx+fw:.1f} {fy0:.1f} L {cx-fw:.1f} {fy0:.1f} Z" fill="rgba(255,255,255,0.10)" stroke="#f2f2ea" stroke-width="1.5"/>')
    # north marker on the rim (world north rotated by -yaw)
    th = math.radians(-yaw)
    nx, ny = cx + r * math.sin(th) * 0.90, cy - r * math.cos(th) * 0.90
    s.add(f'<circle cx="{nx:.1f}" cy="{ny:.1f}" r="11" fill="rgba(14,18,20,0.85)" stroke="{ACC}" stroke-width="1.5"/>')
    s.text(nx, ny + 5, "N", 13, ACC, "middle", "bold")
    # north-up lock toggle on the rim at ~4:30
    a = math.radians(135)
    lx, ly = cx + r * math.sin(a) * 0.93, cy - r * math.cos(a) * 0.93
    s.add(f'<circle cx="{lx:.1f}" cy="{ly:.1f}" r="12" fill="rgba(14,18,20,0.80)" stroke="rgba(236,238,230,0.55)" stroke-width="1"/>')
    s.add(f'<path d="M {lx-3.5:.1f} {ly-1:.1f} v -3 a 3.5 3.5 0 0 1 7 0 v 3" fill="none" stroke="{TXT}" stroke-width="1.6"/>')
    s.rect(lx - 5.5, ly - 1, 11, 7, TXT, rx=1.5)

    # ---------------- event feed (top-right, click-through, max 3, fades) ----------------
    fe = [("1:12", "Rifle 2 under fire  ×3", 1.0), ("1:09", "Engineers under fire", .75), ("1:07", "Point C contested", .5)]
    fwid = P["feed_w"]; fx0 = W - 12 - fwid; fy0 = TB + 8; lh = 24
    s.reg("Event feed (3 lines)", fx0, fy0, fwid, lh * len(fe))
    for i, (t, m, op) in enumerate(fe):
        y = fy0 + i * lh
        wline = tw(t + "  " + m, 14) + 20
        s.add(f'<g opacity="{op}" pointer-events="none">')
        s.rect(W - 12 - wline, y, wline, lh - 3, "rgba(14,18,20,0.38)", rx=3)
        s.text(W - 22, y + 16, m, 14, TXT, "end")
        s.text(W - 22 - tw(m, 14) - 10, y + 16, t, 13, DIM, "end")
        s.add('</g>')

    # ---------------- bottom console: one top, one baseline ----------------
    base = H - 10
    HC = P["hc"]
    top = base - HC

    # orders + abilities + stance (5 columns x 3 rows)
    bw_, g = P["btn_w"], 4
    ow = 5 * bw_ + 4 * g
    bh_ = (HC - 2 * g) / 3
    ox, oy = W - 12 - ow, top
    s.reg("Orders / abilities / stance", ox, oy, ow, HC)
    # row 1: abilities, own row, warm tint, 4 wide buttons
    abw = (ow - 3 * g) / 4
    abil = [("G", "Frag", ("cnt", "×2", 0)), ("T", "Smoke", ("cd", "8s", 0.45)), ("F", "Satchel", ("off", "Eng", 0)), ("B", "Barrage", None)]
    for i, (key, lab, st) in enumerate(abil):
        x = ox + i * (abw + g); y = oy
        s.rect(x, y, abw, bh_, ABIL, rx=4)
        off = st and st[0] == "off"
        if st and st[0] == "cd":     # recharge fills from the bottom, under the text
            s.rect(x, y + bh_ * (1 - st[2]), abw, bh_ * st[2], "rgba(224,173,74,0.18)")
        s.text(x + 7, y + 16, key, 13, OFF_ACC if off else ACC, weight="bold")
        if st:
            s.text(x + abw - 6, y + 16, st[1], 13, OFF_TXT if off else TXT, "end", "bold")
        s.text(x + abw / 2, y + bh_ - 8, lab, 14, OFF_TXT if off else TXT, "middle")
    # row 2: orders
    orders = [("RMB", "Move"), ("Ctrl+RMB", "Attack"), ("H", "Stop"), ("R", "Retreat"), ("Y", "Dig in")]
    for i, (key, lab) in enumerate(orders):
        x = ox + i * (bw_ + g); y = oy + bh_ + g
        s.rect(x, y, bw_, bh_, BTN, rx=4)
        if key.endswith("RMB"):          # mouse glyph: right button filled
            kx = x + 6
            if key.startswith("Ctrl"):
                s.text(kx, y + 16, "Ctrl", 13, ACC, weight="bold"); kx += 30
            s.rect(kx, y + 5, 9, 13, "none", rx=4, stroke=ACC, sw=1.4)
            s.add(f'<path d="M {kx+4.5:.1f} {y+5.7:.1f} h 2 a 3.3 3.3 0 0 1 2 3 v 2.6 h -4 Z" fill="{ACC}"/>')
        else:
            s.text(x + 6, y + 16, key, 13, ACC, weight="bold")
        s.text(x + bw_ / 2, y + bh_ - 8, lab, 14, TXT, "middle")
    # row 3: stance as one segmented strip (4 cols) + Regroup
    y = oy + 2 * (bh_ + g)
    sw_ = 4 * bw_ + 3 * g
    s.rect(ox, y, sw_, bh_, BTN, rx=4)
    segw = sw_ / 4
    for i, (key, lab) in enumerate([("Z", "Stand"), ("X", "Crouch"), ("C", "Prone"), ("V", "Auto")]):
        x = ox + i * segw
        if i:
            s.rect(x - .5, y + 7, 1, bh_ - 14, "rgba(236,238,230,0.20)")
        if lab == "Crouch":
            s.rect(x + 1, y + 1, segw - 2, bh_ - 2, "rgba(224,173,74,0.16)", rx=4, stroke=ACC, sw=2)
        s.text(x + 6, y + 16, key, 13, ACC, weight="bold")
        s.text(x + segw / 2, y + bh_ - 8, lab, 14, TXT, "middle")
    x = ox + 4 * (bw_ + g)
    s.rect(x, y, bw_, bh_, BTN, rx=4)
    s.text(x + 6, y + 16, "L", 13, ACC, weight="bold")
    s.text(x + bw_ / 2, y + bh_ - 8, "Regroup", 14, TXT, "middle")

    # call-in column above the orders: one shared strip, no per-tile boxes
    ctw = 120; tile_h = P["tile_h"]; head = P["ci_head"]; q_h = P["ci_q"]
    tiles = [("Rifle squad", "120", "ok"), ("MG team", "150", "cd"), ("Engineers", "140", "ok"), ("Mortar", "260", "poor")]
    strip_h = head + len(tiles) * tile_h
    col_h = strip_h + 4 + q_h
    cx0 = W - 12 - ctw; cy_top = oy - P["ci_gap"] - col_h
    s.reg("Call-in column", cx0, cy_top, ctw, col_h)
    s.rect(cx0, cy_top, ctw, strip_h, STRIP, rx=5)
    s.text(cx0 + 8, cy_top + 14, "CALL-IN", 13, DIM, weight="bold")
    s.text(cx0 + ctw - 8, cy_top + 14, "Tab", 13, DIM, "end")
    for i, (nm, cost, st) in enumerate(tiles):
        y = cy_top + head + i * tile_h
        s.rect(cx0 + 8, y, ctw - 16, 1, "rgba(236,238,230,0.14)")
        poor = st == "poor"
        if st == "cd":   # re-call cooldown: fill from the bottom, under the text
            s.rect(cx0 + 1, y + tile_h * 0.45, ctw - 2, tile_h * 0.55 - 1, "rgba(236,238,230,0.12)")
        s.text(cx0 + 8, y + 16, nm, 14, OFF_TXT if poor else TXT, weight="bold")
        s.text(cx0 + 8, y + tile_h - 5, f"{cost} MP", 14, OFF_ACC if poor else ACC, weight="bold")
        if st == "cd":
            s.text(cx0 + ctw - 8, y + tile_h - 5, "9s", 13, TXT, "end", "bold")
            clock(s, cx0 + ctw - 33, y + tile_h - 10)
        if poor:
            s.text(cx0 + ctw - 8, y + tile_h - 5, "need 25", 14, "rgba(236,238,230,0.75)", "end")
    qy = cy_top + strip_h + 4
    s.rect(cx0, qy, ctw, q_h, STRIP, rx=5)
    s.rect(cx0 + 6, qy + q_h - 6, ctw - 12, 3, "rgba(255,255,255,0.18)")
    s.rect(cx0 + 6, qy + q_h - 6, (ctw - 12) * 0.62, 3, ACC)
    s.text(cx0 + 6, qy + 15, "Next: Rifle 7s", 14, TXT)
    s.text(cx0 + ctw - 6, qy + 15, "2/5", 13, TXT, "end", "bold")

    # detail panel: focused squad, men as clickable plain rows, row-first = pip order
    dw = P["detail_w"]
    dx = ox - 12 - dw; dy = top
    s.reg("Squad detail (men rows)", dx, dy, dw, HC)
    s.rect(dx, dy, dw, HC, PANEL, rx=6)
    s.text(dx + 10, dy + 19, "Rifle squad 1", 14, TXT, weight="bold")
    s.text(dx + dw - 10, dy + 19, "Holding · crouch", 14, DIM, "end")
    men = [("Sgt (lead)", "45", "a"), ("Rifle 2", "40", "a"), ("Rifle 3", "45", "sel"), ("Rifle 4", "38", "a"),
           ("LMG", "180", "a"), ("Asst LMG", "30", "w"), ("Rifle 7", "42", "a"), ("Rifle 8", "44", "a")]
    colw = (dw - 20 - 8) / 2; rh = (HC - 28 - 6) / 4
    for i, (nm, am, st) in enumerate(men):
        rr = i // 2; c = i % 2
        x = dx + 10 + c * (colw + 8); y = dy + 27 + rr * rh
        if st == "sel":
            s.rect(x - 4, y, colw + 4, rh, "rgba(255,255,255,0.15)", rx=3)
            s.rect(x - 4, y, 3, rh, ACC)
        pip(s, x + 4, y + (rh - 12) / 2, 12, st)
        s.text(x + 24, y + rh / 2 + 5, nm, 14, TXT, weight="bold" if st == "sel" else "normal")
        s.text(x + colw - 4, y + rh / 2 + 5, am, 14, DIM, "end")

    # roster: group tabs + cards, same top as the rest
    rx0 = 12; rx1 = dx - 12
    tab_h = 22
    for i, g_ in enumerate(["1", "2", "3"]):
        s.rect(rx0 + i * 32, top, 28, tab_h, "rgba(14,18,20,0.70)" if i == 0 else PANEL, rx=4)
        if i == 0:
            s.rect(rx0 + i * 32, top + tab_h - 2, 28, 2, ACC)
        s.text(rx0 + i * 32 + 14, top + 16, g_, 14, TXT, "middle", "bold")
    cg2 = 6
    ncol = P["cards_per_row"]
    cw2 = (rx1 - rx0 - (ncol - 1) * cg2) / ncol
    squads = [("Rifle 1", "8/8", "Holding", ["a", "a", "sel", "a", "a", "w", "a", "a"], True),
              ("Rifle 2", "6/8", "Under fire", ["a", "a", "w", "pin", "a", "det", "x", "x"], False),
              ("MG team", "2/3", "Pinned", ["pin", "pin", "x"], False),
              ("Engineers", "5/5", "Moving", ["a"] * 5, False),
              ("Mortar", "3/3", "Ready", ["a"] * 3, False),
              ("Sniper", "2/2", "Hidden", ["a", "a"], False)]
    nrows = math.ceil(len(squads) / ncol)
    cards_top = top + tab_h + 6
    chh = (base - cards_top - (nrows - 1) * cg2) / nrows
    s.reg("Roster (tabs + cards)", rx0, top, ncol * cw2 + (ncol - 1) * cg2, HC)
    ps = P["pip"]
    for i, (nm, cnt, stt, pips, foc) in enumerate(squads):
        rr = i // ncol; c = i % ncol
        x = rx0 + c * (cw2 + cg2); y = cards_top + rr * (chh + cg2)
        s.rect(x, y, cw2, chh, PANEL, rx=5)
        if foc:
            s.rect(x, y, 3, chh, ACC)
            s.rect(x + .5, y + .5, cw2 - 1, chh - 1, "none", rx=5, stroke="rgba(224,173,74,0.65)", sw=1)
        s.text(x + 10, y + 18, nm, 14, TXT, weight="bold")
        s.text(x + cw2 - 8, y + 18, stt, 14, ACC if stt in ("Under fire", "Pinned") else DIM, "end")
        py = y + chh - ps - 7
        for j, p in enumerate(pips):
            pip(s, x + 10 + j * (ps + 3), py, ps, p)
        s.text(x + cw2 - 8, py + ps - 1, cnt, 14, TXT, "end")

    band = top - TB
    if notes:
        cc = "#1b2117"
        for t, x, y, a in P["callouts"](s, band):
            s.text(x, y, t, 13, cc, a, "bold", 'font-style="italic"')
    s.add('</svg>')
    return s, band


def callouts_1600(s, band):
    return [
        ("Minimap: radar disc centred on camera, rotates with yaw; hatched = off-map", 276, 140, "start"),
        ("Rim: N = world north, lock (4:30) = north-up; wheel zooms to whole map", 276, 158, "start"),
        ("Feed: 3 lines max, click-through, each line fades after 8 s", 1588, 128, "end"),
        ("Call-in: one strip, clock = re-call cooldown; queue = next arrival + count", 1458, 520, "end"),
        (f"Clear band 1600 x {band:.0f} ({band/900*100:.0f}% of height)", 800, 300, "middle"),
        ("Pips: fill = selected, half = wounded, hatch = pinned, dashed = detached, X = dead", 12, 748, "start"),
    ]


def callouts_1280(s, band):
    return []


P1600 = dict(tb=28, faction="GERMANY", chip_w=44, ticket_w=100, mm_d=250, mm_pt=10, fr=(14, 34, 46),
             feed_w=300, btn_w=68, tile_h=40, ci_gap=10, ci_head=20, ci_q=26, hc=130, detail_w=300,
             cards_per_row=5, pip=13, fade=170, callouts=callouts_1600)
P1280 = dict(tb=28, faction="GER", chip_w=40, ticket_w=60, mm_d=200, mm_pt=10, fr=(11, 27, 37),
             feed_w=260, btn_w=62, tile_h=38, ci_gap=8, ci_head=18, ci_q=24, hc=124, detail_w=262,
             cards_per_row=3, pip=12, fade=160, callouts=callouts_1280)


def report(s, W, H, band):
    print(f"== {W}x{H}  clear band={band:.0f} px ({band/H*100:.1f}% of height)")
    area = 0
    for n_, (x, y, w, h) in s.rects.items():
        print(f"  {n_:30s} x{x:.0f}-{x+w:.0f}  y{y:.0f}-{y+h:.0f}  ({w:.0f}x{h:.0f})")
        area += w * h
    print(f"  HUD rect area {area/(W*H)*100:.1f}%  -> area-based clear {100-area/(W*H)*100:.1f}%")


if __name__ == "__main__":
    out = sys.argv[1]
    for (W, H, P, fn) in [(1600, 900, P1600, "hud-mock-v2.svg"), (1280, 800, P1280, "hud-mock-v2-1280.svg")]:
        s, band = build(W, H, P)
        open(f"{out}/{fn}", "w").write("\n".join(s.o))
        report(s, W, H, band)
    s, band = build(1600, 900, P1600, notes=True)
    open(f"{out}/hud-mock-v2-annotated.svg", "w").write("\n".join(s.o))
