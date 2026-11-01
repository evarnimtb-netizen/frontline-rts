#!/usr/bin/env python3
"""HUD mock v2.3, mood "Blend" (map table base + field-manual paper + olive-steel edges).

Layout is the approved v2 layout (S1b-hud.md), with the owner's 2026-10-31 decisions:
north-up rounded-square minimap, stance X/C/Z (no V), orders RMB/Ctrl+RMB/H/R/Y/L,
abilities G/T/F/B.

Materials:
  felt    - console panels, top bar, minimap bezel (visible fibre texture, double
            neatline, light top edge + dark seam)
  canvas  - ability row (woven khaki)
  paper   - minimap sheet, call-in requisition slip, teleprinter feed tape
Marks: grease-pencil yellow for selection only, red rubber stamps for alarm states with an
amber lamp, map-legend objective symbols for A-E (grease-pencil owner arcs, blue/red dashed
ring when contested), black/cream scale-bar ticket bars with a grease value mark.
v2.3 (AD critique): mottled-wool + twill felt with binding and stitching, one ruled orders
board with cream key tabs and an Auto cue, redrawn counters, ink stamps, faction bands
(GER field grey, USA light khaki, USSR earth brown) + survey-mark emblems, 4 cards/row at 1600.

Usage: FONTCONFIG_FILE=fonts-blend.conf PANGOCAIRO_BACKEND=fc \
       gen_hud_blend.py OUTDIR [GER|USA|USSR] [--notext]
Writes ui-mood-blend-{1600,1280}[-usa|-ussr].svg (+ .notext.svg and .items.json with
--notext, for the contrast checker).
Fonts (OFL): Barlow Medium/SemiBold, Stardos Stencil Bold, Courier Prime Bold.
"""
import math, sys, random, json, struct, os

HERE = os.path.dirname(os.path.abspath(__file__))
FONTDIR = os.path.join(HERE, "fonts-blend")

# ---------------- palette ----------------
TXT = "#ece4c8"        # cream: text on felt
DIM = "#d0c8ac"        # secondary text on felt
OFF = "#a29d87"        # disabled text on felt / canvas (>= 3:1, clearly dimmer)
ACC = "#efc94c"        # grease-pencil yellow: selection, costs on felt
FRI = "#72a2db"        # friendly blue
ENE = "#e46e5b"        # enemy red (battlefield, minimap diamonds)
STAMP = "#a3291b"      # rubber-stamp red ink
STAMP_TXT = "#f7eedb"
STAMP_INK = "#8e2414"  # red ink printed on paper / stamp
STAMP_PATCH = "#efe5c6"  # small cream patch under a stamp
LAMP = "#ffa424"       # amber alarm lamp
FELT = "#1d2721"
PANEL = "rgba(29,39,33,0.94)"
BTN = "rgba(29,39,33,0.92)"
CANVAS = "rgba(62,52,30,0.94)"
CR = "236,228,200"
PAPER = "#e2d8b8"      # map sheet, tape
SLIP = "#d8cca6"       # requisition slip (a shade darker: less glare)
INK = "#22251d"
INK2 = "#3c3a29"
INK_OFF = "#67614b"    # disabled ink on paper (>= 3:1)
GREASE_INK = "#5a3205" # cost / progress ink on paper

OWN_G = "#86a6cf"      # grease-pencil blue: own objective arcs, own ticket mark
ENE_G = "#d4806b"      # grease-pencil red: enemy objective arcs, enemy ticket mark
TAB = "#e8dfc2"        # stamped cream key tab
COUNTER = "#121813"    # unit counter square

# faction accent: band + its text colour + rule tint + emblem shape (second, non-colour cue).
# All bands keep >= 20 dE00 from the felt, and USSR keeps >= 20 dE00 from ENE and STAMP.
FACTIONS = {
    "GER": ("GERMANY", "#46535b", TXT, "#9fb0b9", "square"),   # field grey band, cream text
    "USA": ("USA", "#bdb07c", INK, "#d6c98f", "disc"),         # light khaki band, ink text
    "USSR": ("USSR", "#5e4a35", TXT, "#c2a27a", "tri"),        # earth-brown band, cream text
}

B5, B6 = 500, 600      # Barlow Medium / SemiBold
FONTFILES = {("Barlow", 500): "Barlow-Medium.ttf", ("Barlow", 600): "Barlow-SemiBold.ttf",
             ("Stardos Stencil", 700): "StardosStencil-Bold.ttf", ("Courier Prime", 700): "CourierPrime-Bold.ttf"}


class TTF:
    """Minimal TrueType advance-width reader (cmap format 4 + hmtx), no kerning."""
    def __init__(self, path):
        d = open(path, "rb").read()
        n = struct.unpack(">H", d[4:6])[0]
        t = {}
        for i in range(n):
            tag, _, off, _ = struct.unpack(">4sIII", d[12 + 16 * i:28 + 16 * i])
            t[tag.decode("latin-1")] = off
        self.upem = struct.unpack(">H", d[t["head"] + 18:t["head"] + 20])[0]
        nh = struct.unpack(">H", d[t["hhea"] + 34:t["hhea"] + 36])[0]
        hm = t["hmtx"]
        self.adv = [struct.unpack(">H", d[hm + 4 * i:hm + 4 * i + 2])[0] for i in range(nh)]
        c = t["cmap"]; nsub = struct.unpack(">H", d[c + 2:c + 4])[0]
        self.cmap = {}
        for i in range(nsub):
            pid, eid, off = struct.unpack(">HHI", d[c + 4 + 8 * i:c + 12 + 8 * i])
            o = c + off
            if struct.unpack(">H", d[o:o + 2])[0] != 4 or pid not in (0, 3):
                continue
            segx2 = struct.unpack(">H", d[o + 6:o + 8])[0]; seg = segx2 // 2
            ends = struct.unpack(f">{seg}H", d[o + 14:o + 14 + segx2])
            starts = struct.unpack(f">{seg}H", d[o + 16 + segx2:o + 16 + 2 * segx2])
            deltas = struct.unpack(f">{seg}h", d[o + 16 + 2 * segx2:o + 16 + 3 * segx2])
            rp = o + 16 + 3 * segx2
            roffs = struct.unpack(f">{seg}H", d[rp:rp + segx2])
            for k in range(seg):
                for ch in range(starts[k], ends[k] + 1):
                    if ch == 0xFFFF:
                        continue
                    if roffs[k] == 0:
                        g = (ch + deltas[k]) & 0xFFFF
                    else:
                        a = rp + 2 * k + roffs[k] + 2 * (ch - starts[k])
                        g = struct.unpack(">H", d[a:a + 2])[0]
                        if g:
                            g = (g + deltas[k]) & 0xFFFF
                    self.cmap.setdefault(ch, g)

    def width(self, s, size, ls=0.0):
        tot = 0
        for ch in s:
            g = self.cmap.get(ord(ch))
            if g is None:
                print(f"  !! missing glyph {ch!r} U+{ord(ch):04X}", file=sys.stderr)
                g = 0
            tot += self.adv[min(g, len(self.adv) - 1)]
        return tot * size / self.upem + ls * len(s)


_FONTS = {}


def font(fam, wt):
    k = (fam, wt)
    if k not in _FONTS:
        _FONTS[k] = TTF(os.path.join(FONTDIR, FONTFILES[k]))
    return _FONTS[k]


def tw(s, size, wt=B6, fam="Barlow", ls=0.0):
    return font(fam, wt).width(s, size, ls)


def esc(s):
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


class S:
    def __init__(self, W, H, notext=False):
        self.W, self.H = W, H
        self.o = []
        self.rects = {}
        self.items = []
        self.notext = notext

    def add(self, s): self.o.append(s)

    def rect(self, x, y, w, h, fill, rx=0, stroke=None, sw=1, extra=""):
        st = f' stroke="{stroke}" stroke-width="{sw}"' if stroke else ""
        self.add(f'<rect x="{x:.1f}" y="{y:.1f}" width="{w:.1f}" height="{h:.1f}" rx="{rx}" fill="{fill}"{st} {extra}/>')

    def text(self, x, y, s, size=15, fill=TXT, anchor="start", wt=B6, fam="Barlow", ls=0.0,
             dis=False, tag="", extra=""):
        w = tw(s, size, wt, fam, ls)
        x0 = x if anchor == "start" else (x - w / 2 if anchor == "middle" else x - w)
        self.items.append(dict(s=s, x0=round(x0, 1), x1=round(x0 + w, 1), y0=round(y - size * 0.78, 1),
                               y1=round(y + size * 0.22, 1), size=size, dis=dis, tag=tag, fam=fam))
        if self.notext:
            return w
        lsa = f' letter-spacing="{ls}"' if ls else ""
        self.add(f'<text x="{x:.1f}" y="{y:.1f}" font-family="{fam}" font-size="{size}" font-weight="{wt}" '
                 f'fill="{fill}" text-anchor="{anchor}"{lsa} {extra}>{esc(s)}</text>')
        return w

    def sten(self, x, y, s, size=16, fill=TXT, anchor="start", **kw):
        return self.text(x, y, s, size, fill, anchor, 700, "Stardos Stencil", 0.8, **kw)

    def reg(self, name, x, y, w, h): self.rects[name] = (x, y, w, h)


# ---------------- materials ----------------
def texture(s, x, y, w, h, rx=0, kind="felt"):
    if kind == "felt":   # directional twill weave under the mottled-wool filter
        s.rect(x, y, w, h, "url(#twill)", rx)
    s.rect(x, y, w, h, "#000", rx, extra=f'filter="url(#{kind})"')


def edges(s, x, y, w, h, rx=1.5, stitch=True):
    """Felt edge: dark outer seam, dark binding band, stitched line, light top edge."""
    s.rect(x - .5, y - .5, w + 1, h + 1, "none", rx + .5, stroke="rgba(4,6,5,0.9)", sw=1)
    s.rect(x + 1.25, y + 1.25, w - 2.5, h - 2.5, "none", max(0, rx - 1), stroke="rgba(6,9,7,0.55)", sw=2.5)
    if stitch:
        s.rect(x + 4, y + 4, w - 8, h - 8, "none", 0, stroke=f"rgba({CR},0.30)", sw=0.9,
               extra='stroke-dasharray="2.6 2.2"')
    s.add(f'<path d="M {x+1.5:.1f} {y+.6:.1f} H {x+w-1.5:.1f}" stroke="rgba({CR},0.45)" stroke-width="1"/>')
    s.add(f'<path d="M {x+1.5:.1f} {y+h-.6:.1f} H {x+w-1.5:.1f}" stroke="rgba(0,0,0,0.6)" stroke-width="1"/>')


def emblem(s, x, y, kind, size=12, col=TXT, cut=None):
    """Original faction emblem drawn like a survey mark on a map: an outline square / circle /
    triangle with a centre dot. Shape is the non-colour faction cue. No real insignia."""
    cx, cy, r = x + size / 2, y + size / 2, size / 2
    sw = max(1.6, size * 0.16)
    st = f'fill="none" stroke="{col}" stroke-width="{sw:.1f}" stroke-linejoin="miter"'
    if kind == "square":
        s.add(f'<rect x="{x+sw/2:.1f}" y="{y+sw/2:.1f}" width="{size-sw:.1f}" height="{size-sw:.1f}" {st}/>')
    elif kind == "disc":
        s.add(f'<circle cx="{cx:.1f}" cy="{cy:.1f}" r="{r-sw/2:.1f}" {st}/>')
    else:
        cy = y + size * 0.62
        s.add(f'<path d="M {cx:.1f} {y-0.6:.1f} L {x+size+0.6:.1f} {y+size-sw/2:.1f} H {x-0.6:.1f} Z" {st}/>')
    s.add(f'<circle cx="{cx:.1f}" cy="{cy:.1f}" r="{max(1.4, size*0.14):.1f}" fill="{col}"/>')


def keytab(s, x, y, key, dis=False, h=15, tag="", sz=14):
    """Hotkey as a stamped cream letter tab: square corners, ink letter. Disabled: dashed
    outline with a dim letter, no fill."""
    w = max(h - 1, tw(key, sz) + 7)
    if dis:
        s.rect(x + .5, y + .5, w - 1, h - 1, "none", 0, stroke=f"rgba({CR},0.45)", sw=1, extra='stroke-dasharray="2 2"')
        s.text(x + w / 2, y + h - 3.5, key, sz, OFF, "middle", dis=True, tag=tag or f"key {key}")
    else:
        s.rect(x + 1, y + 1.2, w, h, "rgba(0,0,0,0.45)")
        s.rect(x, y, w, h, TAB)
        texture(s, x, y, w, h, 0, "pgrain")
        s.text(x + w / 2, y + h - 3.5, key, sz, INK, "middle", tag=tag or f"key {key}")
    return w


def neatline(s, x, y, w, h, inset=3.5, strong=True):
    """Map neatline: a firm outer line and a hairline inside it."""
    s.rect(x + inset, y + inset, w - 2 * inset, h - 2 * inset, "none", 0,
           stroke=f"rgba({CR},{0.46 if strong else 0.34})", sw=1.3 if strong else 1)
    if strong:
        s.rect(x + inset + 2.5, y + inset + 2.5, w - 2 * inset - 5, h - 2 * inset - 5, "none", 0,
               stroke=f"rgba({CR},0.16)", sw=0.8)


def felt_panel(s, x, y, w, h, fill=PANEL, neat=True, strong=True, rx=1.5, stitch=True):
    s.rect(x, y, w, h, fill, rx)
    texture(s, x, y, w, h, rx, "felt")
    edges(s, x, y, w, h, rx, stitch)
    if neat:
        neatline(s, x, y, w, h, inset=6.5 if stitch else 3.5, strong=strong)


def canvas_panel(s, x, y, w, h, rx=1.5, dis=False):
    s.rect(x, y, w, h, CANVAS, rx)
    s.rect(x, y, w, h, "url(#weave)", rx)
    texture(s, x, y, w, h, rx, "felt")
    edges(s, x, y, w, h, rx)
    if dis:
        s.rect(x + 3.5, y + 3.5, w - 7, h - 7, "none", 0, stroke=f"rgba({CR},0.30)", sw=1, extra='stroke-dasharray="3 3"')
    else:
        neatline(s, x, y, w, h, strong=False)


def paper(s, x, y, w, h, fill=PAPER, shadow=True):
    if shadow:
        s.rect(x + 2, y + 3, w, h, "rgba(0,0,0,0.38)")
    s.rect(x, y, w, h, fill)
    texture(s, x, y, w, h, 0, "pgrain")


def frame_bar(s, x0, x1, y, h=3, seg=24, light=f"rgba({CR},0.62)"):
    """Alternating map-border bar (black / cream graticule segments)."""
    s.rect(x0, y, x1 - x0, h, "rgba(4,6,5,0.85)")
    i = 0; xx = x0
    while xx < x1:
        if i % 2 == 0:
            s.rect(xx, y + .5, min(seg, x1 - xx), h - 1, light)
        xx += seg; i += 1


def grease_path(pts, rnd, j=0.9):
    d = []
    for i, (x, y) in enumerate(pts):
        x += rnd.uniform(-j, j); y += rnd.uniform(-j, j)
        d.append(f'{"M" if i == 0 else "L"} {x:.1f} {y:.1f}')
    return " ".join(d)


def grease_rect(s, x, y, w, h, col=ACC, seed=1, sw=2.4):
    rnd = random.Random(seed)
    for p, (sw_, op) in enumerate([(sw, 0.95), (sw * 0.55, 0.55)]):
        pts = []
        o = p * 0.8
        corners = [(x - 2 + o, y + o), (x + w - o, y - o * .5), (x + w + o * .5, y + h - o), (x + o, y + h + o * .5), (x - o * .5, y + 3)]
        for (ax, ay), (bx, by) in zip(corners, corners[1:]):
            n = max(2, int(math.hypot(bx - ax, by - ay) / 14))
            for k in range(n):
                t = k / n
                pts.append((ax + (bx - ax) * t, ay + (by - ay) * t))
        pts.append(corners[-1])
        s.add(f'<path d="{grease_path(pts, rnd)}" fill="none" stroke="{col}" stroke-width="{sw_:.1f}" '
              f'stroke-linecap="round" stroke-linejoin="round" opacity="{op}"/>')


def grease_line(s, x0, y0, x1, y1, col=ACC, seed=1, sw=3):
    rnd = random.Random(seed)
    n = max(2, int(math.hypot(x1 - x0, y1 - y0) / 10))
    pts = [(x0 + (x1 - x0) * k / n, y0 + (y1 - y0) * k / n) for k in range(n + 1)]
    s.add(f'<path d="{grease_path(pts, rnd, 0.6)}" fill="none" stroke="{col}" stroke-width="{sw}" stroke-linecap="round" opacity="0.95"/>')


def grease_circle(s, cx, cy, r, col=ACC, seed=1, sw=2.2):
    """Hand-drawn loop, slightly overshooting its start."""
    rnd = random.Random(seed)
    pts = []
    for k in range(29):
        a = -0.4 + k * (2 * math.pi + 0.7) / 28
        rr = r + rnd.uniform(-0.6, 0.6) + 0.5 * math.sin(a * 2)
        pts.append((cx + rr * math.cos(a), cy + rr * 0.92 * math.sin(a)))
    s.add(f'<path d="{grease_path(pts, rnd, 0.3)}" fill="none" stroke="{col}" stroke-width="{sw}" '
          f'stroke-linecap="round" stroke-linejoin="round" opacity="0.95"/>')


# ---------------- unit counters (original, era-neutral pictograms) ----------------
def glyph(s, cx, cy, kind, col=TXT):
    """Cream-ink pictogram centred on (cx, cy), about 20 x 14 px. Not APP-6, no insignia."""
    L = f'stroke="{col}" stroke-width="1.6" fill="none" stroke-linecap="round" stroke-linejoin="round"'
    F = f'fill="{col}"'
    if kind in ("rifle", "sniper"):   # rifle side-on, tilted up: stock, barrel, magazine
        scope = ""
        if kind == "sniper":            # telescope on top of the receiver, with bell ends
            scope = (f'<rect x="{cx-3.5:.1f}" y="{cy-3.6:.1f}" width="8" height="2.4" rx="0.8" {F}/>'
                     f'<path d="M {cx-4.5:.1f} {cy-4.4:.1f} V {cy-0.8:.1f} M {cx+5.5:.1f} {cy-4.4:.1f} V {cy-0.8:.1f}" {L}/>'
                     f'<path d="M {cx-1:.1f} {cy-1.4:.1f} V {cy+0.2:.1f} M {cx+2:.1f} {cy-1.4:.1f} V {cy+0.2:.1f}" stroke="{col}" stroke-width="1.2"/>')
        mag = "" if kind == "sniper" else f'<path d="M {cx-1:.1f} {cy+1.2:.1f} V {cy+4.6:.1f}" {L}/>'
        s.add(f'<g transform="rotate(-12 {cx:.1f} {cy:.1f})">'
              f'<path d="M {cx-10:.1f} {cy+1.4:.1f} L {cx-4:.1f} {cy-0.4:.1f} L {cx-4:.1f} {cy+2.4:.1f} L {cx-10:.1f} {cy+4.8:.1f} Z" {F}/>'
              f'<path d="M {cx-4:.1f} {cy+0.6:.1f} H {cx+10.5:.1f}" {L}/>{mag}{scope}</g>')
    elif kind == "mg":       # heavy barrel on a bipod, short stock, ammo box
        s.add(f'<path d="M {cx-10:.1f} {cy-3.5:.1f} L {cx-6:.1f} {cy-2.6:.1f} L {cx-6:.1f} {cy+0.6:.1f} L {cx-10:.1f} {cy+1.6:.1f} Z" {F}/>'
              f'<path d="M {cx-6:.1f} {cy-1.2:.1f} H {cx+10.5:.1f}" stroke="{col}" stroke-width="2.6" stroke-linecap="round"/>'
              f'<rect x="{cx-4.5:.1f}" y="{cy+0.2:.1f}" width="4" height="3.6" {F}/>'
              f'<path d="M {cx+5:.1f} {cy-0.6:.1f} L {cx+2:.1f} {cy+6:.1f} M {cx+5:.1f} {cy-0.6:.1f} L {cx+8:.1f} {cy+6:.1f}" {L}/>')
    elif kind == "eng":      # entrenching spade: D-handle, shaft, pointed blade; leaning 25 deg
        s.add(f'<g transform="rotate(25 {cx:.1f} {cy:.1f})">'
              f'<path d="M {cx-3:.1f} {cy-8.2:.1f} H {cx+3:.1f} M {cx-3:.1f} {cy-8.2:.1f} L {cx-1:.1f} {cy-5.2:.1f} M {cx+3:.1f} {cy-8.2:.1f} L {cx+1:.1f} {cy-5.2:.1f}" {L}/>'
              f'<path d="M {cx:.1f} {cy-5.4:.1f} V {cy+0.6:.1f}" stroke="{col}" stroke-width="1.8"/>'
              f'<path d="M {cx-3.6:.1f} {cy+0.4:.1f} H {cx+3.6:.1f} V {cy+4.6:.1f} Q {cx+3.2:.1f} {cy+7:.1f} {cx:.1f} {cy+8.6:.1f} Q {cx-3.2:.1f} {cy+7:.1f} {cx-3.6:.1f} {cy+4.6:.1f} Z" {F}/></g>')
    elif kind == "mortar":   # tube at 45 deg on a baseplate, bipod from mid-tube to the ground
        s.add(f'<path d="M {cx-10:.1f} {cy+6.6:.1f} H {cx-1:.1f}" stroke="{col}" stroke-width="2.4" stroke-linecap="round"/>'
              f'<path d="M {cx-5.5:.1f} {cy+5:.1f} L {cx+5.5:.1f} {cy-6:.1f}" stroke="{col}" stroke-width="3" stroke-linecap="butt"/>'
              f'<path d="M {cx+1.5:.1f} {cy-2:.1f} L {cx+4:.1f} {cy+6.6:.1f} M {cx+1.5:.1f} {cy-2:.1f} L {cx+8.5:.1f} {cy+6.6:.1f}" {L}/>')


def counter(s, x, y, kind, w=26, h=18, col=FRI):
    """Board-game counter: dark square, cream ink pictogram, small friendly-colour corner."""
    s.rect(x, y, w, h, COUNTER, 0, stroke=f"rgba({CR},0.55)", sw=1)
    s.add(f'<path d="M {x+.5:.1f} {y+.5:.1f} h 7 L {x+.5:.1f} {y+7.5:.1f} Z" fill="{col}"/>')
    glyph(s, x + w / 2 + 0.5, y + h / 2, kind)


# ---------------- marks ----------------
def pip(s, x, y, size, st):
    if st == "sel":
        s.rect(x, y, size, size, "#fbf6e6", rx=1)
    elif st == "a":
        s.rect(x, y, size, size, f"rgba({CR},0.42)", rx=1)
    elif st == "w":
        s.rect(x + .5, y + .5, size - 1, size - 1, f"rgba({CR},0.08)", rx=1, stroke=f"rgba({CR},0.9)", sw=1.2)
        s.rect(x + .5, y + size / 2, size - 1, size / 2 - .5, f"rgba({CR},0.9)")
    elif st == "pin":
        s.rect(x + .5, y + .5, size - 1, size - 1, "url(#hatch)", rx=1, stroke=f"rgba({CR},0.9)", sw=1.2)
    elif st == "det":
        s.rect(x + 1, y + 1, size - 2, size - 2, "none", rx=1, stroke=TXT, sw=2, extra='stroke-dasharray="4 2.6"')
    elif st == "x":
        s.add(f'<path d="M {x+2} {y+2} L {x+size-2} {y+size-2} M {x+size-2} {y+2} L {x+2} {y+size-2}" stroke="rgba({CR},0.8)" stroke-width="2.2"/>')


def clock(s, x, y, r=5.5, col=TXT):
    s.add(f'<circle cx="{x:.1f}" cy="{y:.1f}" r="{r}" fill="none" stroke="{col}" stroke-width="1.4"/>'
          f'<path d="M {x:.1f} {y-r+2:.1f} L {x:.1f} {y:.1f} L {x+r-2:.1f} {y:.1f}" stroke="{col}" stroke-width="1.4" fill="none"/>')


def keycap(s, x, y, key, dis=False, h=17, tag=""):
    """Hotkey as a small worn key cap: cream legend, cream outline."""
    w = max(h - 1, tw(key, 14) + 8)
    s.rect(x, y, w, h, "rgba(0,0,0,0.30)", 2.5, stroke=f"rgba({CR},{0.28 if dis else 0.55})", sw=1)
    s.text(x + w / 2, y + h - 4.5, key, 14, OFF if dis else TXT, "middle", dis=dis, tag=tag or f"key {key}")
    return w


def mouse(s, x, y, col=TXT):
    s.rect(x, y, 10, 14, "none", rx=4.5, stroke=col, sw=1.4)
    s.add(f'<path d="M {x+5:.1f} {y+.7:.1f} h 1.8 a 3.6 3.6 0 0 1 2.4 3.4 v 2.6 h -4.2 Z" fill="{col}"/>')


def lamp(s, x, y, r=3.8):
    """Amber alarm lamp: glow, dark bezel ring with a light rim, lens highlight. ~8 px lens."""
    s.add(f'<circle cx="{x:.1f}" cy="{y:.1f}" r="{r*2.6:.1f}" fill="url(#lampglow)"/>'
          f'<circle cx="{x:.1f}" cy="{y:.1f}" r="{r+1.9:.1f}" fill="#0b0d0b" stroke="rgba({CR},0.55)" stroke-width="0.8"/>'
          f'<circle cx="{x:.1f}" cy="{y:.1f}" r="{r}" fill="{LAMP}"/>'
          f'<circle cx="{x-r*0.35:.1f}" cy="{y-r*0.35:.1f}" r="{r*0.36:.1f}" fill="#fff3d0" opacity="0.9"/>')


def stamp(s, x1, ytop, txt, seed=1, tag="", sz=14):
    """Red rubber stamp, right edge at x1: small cream patch, red double-ruled frame, red
    lettering, ink break-up; upright within 1.5 degrees. Returns its left edge."""
    w = tw(txt, sz) + 13; h = 21
    x0 = x1 - w
    cx, cy = x0 + w / 2, ytop + h / 2
    ang = (-1.5, 1.2, -1.0)[seed % 3]
    s.add(f'<g transform="rotate({ang} {cx:.1f} {cy:.1f})">')
    s.rect(x0, ytop, w, h, STAMP_PATCH)
    s.add('<g filter="url(#stampink)">')
    s.rect(x0 + .8, ytop + .8, w - 1.6, h - 1.6, "none", 0, stroke=STAMP_INK, sw=1.6)
    s.rect(x0 + 3, ytop + 3, w - 6, h - 6, "none", 0, stroke=STAMP_INK, sw=0.8)
    s.add('</g>')
    s.text(cx, ytop + 15, txt, sz, STAMP_INK, "middle", tag=tag or f"stamp {txt}",
           extra='filter="url(#stampink2)"')
    s.add('</g>')
    return x0


def wobbly_arc(s, cx, cy, r, a0, a1, col, sw=3.2, seed=1, op=0.85, dash=""):
    """Grease-pencil arc from angle a0 to a1 (radians, 0 = north, clockwise)."""
    rnd = random.Random(seed)
    n = max(4, int(abs(a1 - a0) * r / 3))
    pts = []
    for k in range(n + 1):
        a = a0 + (a1 - a0) * k / n
        rr = r + rnd.uniform(-0.45, 0.45)
        pts.append((cx + rr * math.sin(a), cy - rr * math.cos(a)))
    s.add(f'<path d="{grease_path(pts, rnd, 0.25)}" fill="none" stroke="{col}" stroke-width="{sw}" '
          f'stroke-linecap="round" stroke-linejoin="round" opacity="{op}" {dash}/>')


def objective(s, cx, cy, L, col, p, r=10.5, contested=False, seed=1, size=14):
    """Map-legend objective: paper disc with the letter; owner arc in grease pencil = capture
    progress (clockwise from north). Contested: a dashed blue/red ring outside it."""
    if contested:
        rc = r + 3.4
        s.add(f'<circle cx="{cx:.1f}" cy="{cy:.1f}" r="{rc:.1f}" fill="none" stroke="rgba(4,6,5,0.85)" stroke-width="3.6"/>')
        circ = 2 * math.pi * rc; d = circ / 12
        s.add(f'<circle cx="{cx:.1f}" cy="{cy:.1f}" r="{rc:.1f}" fill="none" stroke="{OWN_G}" stroke-width="2.2" stroke-dasharray="{d:.2f} {d:.2f}"/>'
              f'<circle cx="{cx:.1f}" cy="{cy:.1f}" r="{rc:.1f}" fill="none" stroke="{ENE_G}" stroke-width="2.2" stroke-dasharray="{d:.2f} {d:.2f}" stroke-dashoffset="{d:.2f}"/>')
    else:
        for a in (0, 90, 180, 270):   # legend ticks: an objective, not a button
            ca, sa = math.cos(math.radians(a)), math.sin(math.radians(a))
            s.add(f'<path d="M {cx+ca*(r+1.5):.1f} {cy+sa*(r+1.5):.1f} L {cx+ca*(r+4.2):.1f} {cy+sa*(r+4.2):.1f}" stroke="rgba(4,6,5,0.9)" stroke-width="3.2"/>'
                  f'<path d="M {cx+ca*(r+1.5):.1f} {cy+sa*(r+1.5):.1f} L {cx+ca*(r+4):.1f} {cy+sa*(r+4):.1f}" stroke="{TXT}" stroke-width="1.5"/>')
    s.add(f'<circle cx="{cx:.1f}" cy="{cy:.1f}" r="{r:.1f}" fill="none" stroke="rgba(4,6,5,0.78)" stroke-width="4.4"/>')
    if col and p > 0:
        a = 2 * math.pi * min(p, 1.0)
        if p < 0.999:
            s.add(f'<circle cx="{cx:.1f}" cy="{cy:.1f}" r="{r:.1f}" fill="none" stroke="rgba({CR},0.30)" stroke-width="1.2" stroke-dasharray="1.6 1.6"/>')
        wobbly_arc(s, cx, cy, r, -0.12 if p >= 0.999 else 0, a + (0.1 if p >= 0.999 else 0), col, 3.3, seed)
        if p < 0.999:   # progress tick at the arc head
            s.add(f'<path d="M {cx+(r-3.2)*math.sin(a):.1f} {cy-(r-3.2)*math.cos(a):.1f} L {cx+(r+3.2)*math.sin(a):.1f} {cy-(r+3.2)*math.cos(a):.1f}" '
                  f'stroke="{TXT}" stroke-width="1.5"/>')
    else:
        s.add(f'<circle cx="{cx:.1f}" cy="{cy:.1f}" r="{r:.1f}" fill="none" stroke="rgba({CR},0.45)" stroke-width="1.6" stroke-dasharray="2.5 2"/>')
    s.add(f'<circle cx="{cx:.1f}" cy="{cy:.1f}" r="{r-2.3:.1f}" fill="#efe7cd" stroke="{INK}" stroke-width="0.8"/>')
    s.text(cx, cy + size * 0.36, L, size, INK, "middle", tag=f"obj {L}")


def scale_bar(s, x0, y, w, val, vmax, col, seed=1, h=6):
    """Ticket bar as a map scale bar: black/cream alternating segments that are always
    visible, end ticks labelled 0 and max underneath, and the current value as a grease mark
    (a grease underline from 0 to the value plus an upright stroke at the value)."""
    n = 10; sw_ = w / n
    s.rect(x0 - .8, y - .8, w + 1.6, h + 1.6, "#050706")
    for j in range(n):
        s.rect(x0 + j * sw_, y, sw_, h / 2, TXT if j % 2 == 0 else "#0b0e0c")
        s.rect(x0 + j * sw_, y + h / 2, sw_, h / 2, "#0b0e0c" if j % 2 == 0 else TXT)
    for xx in (x0, x0 + w / 2, x0 + w):
        s.rect(xx - .6, y - 2.5, 1.2, h + 4.5, TXT)
    vx = x0 + w * val / vmax
    grease_line(s, x0 + 1, y + h + 3.2, vx, y + h + 3.2, col, seed=seed, sw=2.6)
    grease_line(s, vx, y - 3.5, vx, y + h + 4, col, seed=seed + 1, sw=3)
    s.text(x0, y + h + 15.5, "0", 14, DIM, "middle", wt=B5, tag="ticket 0")
    s.text(x0 + w, y + h + 15.5, str(vmax), 14, DIM, "middle", wt=B5, tag="ticket max")


def mix(c, k, to=(0, 0, 0)):
    c = c.lstrip("#"); r, g, b = int(c[0:2], 16), int(c[2:4], 16), int(c[4:6], 16)
    return "#%02x%02x%02x" % tuple(int(a + (t - a) * k) for a, t in zip((r, g, b), to))


# ---------------- build ----------------
def defs():
    return ('<defs>'
            '<pattern id="hatch" width="4" height="4" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">'
            f'<rect width="4" height="4" fill="rgba(0,0,0,0)"/><line x1="0" y1="0" x2="0" y2="4" stroke="{TXT}" stroke-width="1.6"/></pattern>'
            '<pattern id="hatchdim" width="6" height="6" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">'
            '<line x1="0" y1="0" x2="0" y2="6" stroke="rgba(0,0,0,0.30)" stroke-width="2.2"/></pattern>'
            '<pattern id="pencil" width="5" height="5" patternUnits="userSpaceOnUse" patternTransform="rotate(-35)">'
            '<rect width="5" height="5" fill="rgba(224,173,74,0.20)"/><line x1="0" y1="0" x2="0" y2="5" stroke="#8a5a14" stroke-width="1" opacity="0.25"/></pattern>'
            # woven canvas
            '<pattern id="weave" width="4" height="4" patternUnits="userSpaceOnUse">'
            '<rect x="0" y="0" width="2" height="2" fill="rgba(255,236,190,0.05)"/><rect x="2" y="2" width="2" height="2" fill="rgba(255,236,190,0.05)"/>'
            '<rect x="0" y="1.5" width="4" height="0.6" fill="rgba(0,0,0,0.22)"/><rect x="1.5" y="0" width="0.6" height="4" fill="rgba(0,0,0,0.16)"/></pattern>'
            '<linearGradient id="botfade" x1="0" y1="0" x2="0" y2="1">'
            '<stop offset="0" stop-color="#0c1010" stop-opacity="0"/><stop offset="1" stop-color="#0c1010" stop-opacity="0.26"/></linearGradient>'
            '<radialGradient id="lampglow"><stop offset="0" stop-color="#ffb43c" stop-opacity="0.65"/>'
            '<stop offset="1" stop-color="#ffb43c" stop-opacity="0"/></radialGradient>'
            # felt: low-frequency mottled wool (dark clouds + a few pale ones) and short
            # horizontal fibres; pale parts are capped so text contrast holds over them
            '<filter id="felt" x="0" y="0" width="1" height="1" color-interpolation-filters="sRGB">'
            '<feTurbulence type="fractalNoise" baseFrequency="0.045 0.06" numOctaves="3" seed="4" result="m"/>'
            '<feColorMatrix in="m" type="matrix" values="0 0 0 0 0.02  0 0 0 0 0.03  0 0 0 0 0.02  2.6 0 0 0 -1.05" result="md0"/>'
            '<feComponentTransfer in="md0" result="mdark"><feFuncA type="table" tableValues="0 0.16 0.30 0.40"/></feComponentTransfer>'
            '<feColorMatrix in="m" type="matrix" values="0 0 0 0 0.80  0 0 0 0 0.82  0 0 0 0 0.68  0 -2.6 0 0 1.05" result="ml0"/>'
            '<feComponentTransfer in="ml0" result="mlite"><feFuncA type="table" tableValues="0 0.05 0.08 0.09"/></feComponentTransfer>'
            '<feTurbulence type="fractalNoise" baseFrequency="0.16 0.9" numOctaves="2" seed="21" result="f"/>'
            '<feColorMatrix in="f" type="matrix" values="0 0 0 0 0.02  0 0 0 0 0.03  0 0 0 0 0.02  2.8 0 0 0 -1.3" result="fd0"/>'
            '<feComponentTransfer in="fd0" result="fdark"><feFuncA type="table" tableValues="0 0.22 0.34"/></feComponentTransfer>'
            '<feColorMatrix in="f" type="matrix" values="0 0 0 0 0.84  0 0 0 0 0.86  0 0 0 0 0.74  0 2.8 0 0 -1.45" result="fl0"/>'
            '<feComponentTransfer in="fl0" result="flite"><feFuncA type="table" tableValues="0 0.07 0.10"/></feComponentTransfer>'
            '<feMerge result="all"><feMergeNode in="mdark"/><feMergeNode in="mlite"/><feMergeNode in="fdark"/><feMergeNode in="flite"/></feMerge>'
            '<feComposite in="all" in2="SourceAlpha" operator="in"/></filter>'
            # twill weave: a 3 px diagonal rib, the direction of the cloth
            '<pattern id="twill" width="3" height="3" patternUnits="userSpaceOnUse" patternTransform="rotate(-38)">'
            '<rect width="3" height="1.2" fill="rgba(0,0,0,0.26)"/><rect y="1.7" width="3" height="0.6" fill="rgba(236,228,200,0.045)"/></pattern>'
            # paper grain: faint dark fibres
            '<filter id="pgrain" x="0" y="0" width="1" height="1" color-interpolation-filters="sRGB">'
            '<feTurbulence type="fractalNoise" baseFrequency="0.7 0.25" numOctaves="3" seed="3" result="n"/>'
            '<feColorMatrix in="n" type="matrix" values="0 0 0 0 0.35  0 0 0 0 0.30  0 0 0 0 0.20  0.30 0 0 0 -0.11" result="c"/>'
            '<feTurbulence type="fractalNoise" baseFrequency="0.02" numOctaves="2" seed="12" result="m"/>'
            '<feColorMatrix in="m" type="matrix" values="0 0 0 0 0.55  0 0 0 0 0.45  0 0 0 0 0.25  0.5 0 0 0 -0.2" result="mo"/>'
            '<feMerge result="all"><feMergeNode in="mo"/><feMergeNode in="c"/></feMerge>'
            '<feComposite in="all" in2="SourceAlpha" operator="in"/></filter>'
            # stamp ink: knock small gaps out of the inked block
            '<filter id="stampink" x="-5%" y="-10%" width="110%" height="120%" color-interpolation-filters="sRGB">'
            '<feTurbulence type="fractalNoise" baseFrequency="0.55" numOctaves="2" seed="5" result="n"/>'
            '<feColorMatrix in="n" type="matrix" values="0 0 0 0 0  0 0 0 0 0  0 0 0 0 0  -3.2 0 0 0 2.75" result="mask"/>'
            '<feComposite in="SourceGraphic" in2="mask" operator="in"/></filter>'
            # lighter break-up for stamp lettering (keeps the letters legible)
            '<filter id="stampink2" x="-5%" y="-10%" width="110%" height="120%" color-interpolation-filters="sRGB">'
            '<feTurbulence type="fractalNoise" baseFrequency="0.9" numOctaves="1" seed="9" result="n"/>'
            '<feColorMatrix in="n" type="matrix" values="0 0 0 0 0  0 0 0 0 0  0 0 0 0 0  -3.0 0 0 0 2.95" result="mask"/>'
            '<feComposite in="SourceGraphic" in2="mask" operator="in"/></filter>'
            '</defs>')


def build(W, H, P, fac="GER", notext=False):
    fname, FAC, FAC_T, FAC_L, EMB = FACTIONS[fac]
    BTEX = "pgrain" if FAC_T == INK else "felt"   # light band: paper-like grain, not dark felt
    s = S(W, H, notext)
    s.add(f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" width="{W}" height="{H}" font-family="Barlow">')
    s.add(f'<title>Frontline RTS HUD mock v2.3, blend mood, {fac}, {W}x{H}</title>')
    s.add(defs())
    k = W / 1600; kv = H / 900

    # ---------------- terrain (world placeholder, unchanged) ----------------
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
    field = [((600, 560), 8, True, 0), ((870, 470), 5, False, 1), ((1100, 385), 5, False, 2)]
    for (cx, cy), n, sel, i in field:
        cx *= k; cy *= kv
        s.add(f'<ellipse cx="{cx:.0f}" cy="{cy:.0f}" rx="{44*k:.0f}" ry="{30*k:.0f}" fill="none" stroke="{ACC if sel else "rgba(240,236,220,0.6)"}" stroke-width="{2.2 if sel else 1.5}"/>')
        for j in range(n):
            a = j * (2 * math.pi / n) + i
            s.add(f'<circle cx="{cx+math.cos(a)*26*k:.0f}" cy="{cy+math.sin(a)*15*k:.0f}" r="4" fill="{FRI}" stroke="#1d2a33" stroke-width="1"/>')
    s.add(f'<circle cx="{955*k:.0f}" cy="{512*kv:.0f}" r="4" fill="{FRI}" stroke="#1d2a33" stroke-width="1"/>')
    for (cx, cy) in [(980, 128), (1130, 96), (1275, 70), (1210, 150)]:
        cx *= k; cy *= kv
        for j in range(4):
            s.add(f'<circle cx="{cx+j*9-14:.0f}" cy="{cy+(j%2)*6:.0f}" r="4" fill="{ENE}" stroke="#3a1f1a" stroke-width="1"/>')
    s.add(f'<path d="M {600*k:.0f} {560*kv:.0f} L {700*k:.0f} {505*kv:.0f}" stroke="{ACC}" stroke-width="2.2" stroke-dasharray="6 5" fill="none"/>')
    s.add(f'<circle cx="{700*k:.0f}" cy="{505*kv:.0f}" r="5" fill="none" stroke="{ACC}" stroke-width="2"/>')
    s.rect(0, H - P["fade"], W, P["fade"], "url(#botfade)")

    # ---------------- top bar: felt map-margin strip ----------------
    TB = P["tb"]
    s.rect(0, 0, W, TB, "rgba(24,32,27,0.95)")
    texture(s, 0, 0, W, TB)
    s.add(f'<path d="M 0 0.5 H {W}" stroke="rgba({CR},0.30)" stroke-width="1"/>')
    frame_bar(s, 0, W, TB - 4, 4, 24)
    s.rect(0, TB, W, 2, FAC_L, extra='opacity="0.9"')     # faction rule under the map border
    s.reg("Top bar", 0, 0, W, TB)
    by = (TB - 4) / 2 + 5.5
    # faction plate: band + emblem + stencil name
    pw = max(118, tw(fname, 17, 700, "Stardos Stencil", 0.8) + 40)
    s.rect(0, 0, pw, TB - 4, FAC)
    texture(s, 0, 0, pw, TB - 4, 0, BTEX)
    s.rect(0, 0, pw, 2, FAC_L)
    s.add(f'<path d="M {pw-.5:.1f} 0 V {TB-4}" stroke="rgba(4,6,5,0.85)" stroke-width="1"/>')
    emblem(s, 10, (TB - 4) / 2 - 6, EMB, 12, FAC_T, FAC)
    s.sten(28, by + 1, fname, 17, FAC_T, tag="faction")
    x = pw + 14
    x += s.text(x, by, "MP", 14, DIM, wt=B5) + 6
    x += s.text(x, by, "235", 15, TXT) + 7
    x += s.text(x, by, "+3.3/s", 15, DIM, wt=B5) + 18
    x += s.text(x, by, "POP", 14, DIM, wt=B5) + 6
    s.text(x, by, "31/120", 15, TXT)
    # objectives A-E (map-legend symbols, grease-pencil ownership arcs) and scale-bar tickets
    cw, n = P["chip_w"], 5
    chips_w = n * cw
    c0 = W / 2 - chips_w / 2
    ocy = (TB - 4) / 2
    chips = [("A", OWN_G, 1.0, False), ("B", None, 0.0, False), ("C", OWN_G, 0.6, True), ("D", ENE_G, 1.0, False), ("E", ENE_G, 0.35, False)]
    for i, (L, col, p, con) in enumerate(chips):
        objective(s, c0 + cw * (i + .5), ocy, L, col, p, P["obj_r"], con, seed=11 + i)
    bw = P["ticket_w"]
    yb = 5
    lx0 = c0 - 16 - bw; rx0 = c0 + chips_w + 16
    scale_bar(s, lx0, yb, bw, 460, 500, OWN_G, seed=61)
    scale_bar(s, rx0, yb, bw, 500, 500, ENE_G, seed=71)
    s.text(lx0 - 18, by + 0.5, "460", 16, TXT, "end", tag="tickets own")
    s.text(rx0 + bw + 20, by + 0.5, "500", 16, TXT, "start", tag="tickets enemy")
    # clock + menu
    mw = 94
    s.rect(W - 12 - mw, 3, mw, TB - 10, "rgba(0,0,0,0.25)", 0, stroke=f"rgba({CR},0.40)", sw=1)
    s.text(W - 12 - mw + 9, by, "Menu", 15, TXT)
    keytab(s, W - 12 - mw + 9 + tw("Menu", 15) + 8, by - 13, "Esc", h=16)
    s.text(W - 12 - mw - 12, by, "1:14", 15, TXT, "end")
    clock(s, W - 12 - mw - 12 - tw("1:14", 15) - 12, ocy, 5.5, DIM)

    # ---------------- minimap: north-up paper map on a felt protractor bezel ----------------
    D = P["mm_d"]; band = P["mm_band"]
    mx, my = 12, TB + 10
    cx, cy = mx + D / 2, my + D / 2
    s.reg("Minimap (north-up square)", mx, my, D, D)
    rxo = 12
    s.rect(mx + 2, my + 3, D, D, "rgba(0,0,0,0.35)", rxo)
    s.rect(mx, my, D, D, PANEL, rxo)
    texture(s, mx, my, D, D, rxo)
    edges(s, mx, my, D, D, rxo)
    s.add(f'<clipPath id="bez"><rect x="{mx+1}" y="{my+1}" width="{D-2}" height="{D-2}" rx="{rxo-1}"/></clipPath>')
    s.add('<g clip-path="url(#bez)">')
    hb = D / 2 - 2.5
    for deg in range(0, 360, 5):
        a = math.radians(deg)
        dx_, dy_ = math.sin(a), -math.cos(a)
        t = hb / max(abs(dx_), abs(dy_))
        L = 8 if deg % 30 == 0 else (5 if deg % 10 == 0 else 2.6)
        x0_, y0_ = cx + dx_ * t, cy + dy_ * t
        x1_, y1_ = cx + dx_ * (t - L), cy + dy_ * (t - L)
        s.add(f'<path d="M {x0_:.1f} {y0_:.1f} L {x1_:.1f} {y1_:.1f}" stroke="rgba({CR},{0.92 if deg % 30 == 0 else 0.62})" stroke-width="{1.5 if deg % 30 == 0 else 1}"/>')
    s.add('</g>')
    px0, py0 = mx + band, my + band; Sm = D - 2 * band
    s.rect(px0 + 2, py0 + 3, Sm, Sm, "rgba(0,0,0,0.45)")
    s.rect(px0, py0, Sm, Sm, PAPER)
    texture(s, px0, py0, Sm, Sm, 0, "pgrain")
    s.add(f'<clipPath id="mmap"><rect x="{px0}" y="{py0}" width="{Sm}" height="{Sm}"/></clipPath><g clip-path="url(#mmap)">')

    def M(u, v): return px0 + u * Sm, py0 + v * Sm
    for (u, v, a, b) in [(.30, .72, .22, .12), (.74, .32, .2, .11), (.56, .86, .18, .07), (.22, .22, .14, .1), (.86, .78, .12, .1), (.48, .12, .12, .06)]:
        x, y = M(u, v)
        s.add(f'<ellipse cx="{x:.1f}" cy="{y:.1f}" rx="{a*Sm:.1f}" ry="{b*Sm:.1f}" fill="#adb98c" stroke="#7d8c5e" stroke-width="1" opacity="0.9"/>')
    for (u, v, a, b) in [(.55, .46, .30, .19), (.55, .46, .21, .13), (.55, .46, .12, .07), (.15, .5, .10, .18), (.15, .5, .05, .09)]:
        x, y = M(u, v)
        s.add(f'<ellipse cx="{x:.1f}" cy="{y:.1f}" rx="{a*Sm:.1f}" ry="{b*Sm:.1f}" fill="none" stroke="#a2774a" stroke-width="1" opacity="0.75" transform="rotate(-15 {x:.1f} {y:.1f})"/>')
    x1, y1 = M(0, .86); x2, y2 = M(.38, .70); x3, y3 = M(.62, .45); x4, y4 = M(1, .16)
    road = f'M {x1:.1f} {y1:.1f} C {x2:.1f} {y2:.1f}, {x3:.1f} {y3:.1f}, {x4:.1f} {y4:.1f}'
    s.add(f'<path d="{road}" stroke="#4a3520" stroke-width="3.8" fill="none"/><path d="{road}" stroke="#9c7b52" stroke-width="1.8" fill="none"/>')
    g8 = Sm / 8
    for i in range(1, 8):
        s.add(f'<path d="M {px0+i*g8:.1f} {py0:.1f} v {Sm:.1f} M {px0:.1f} {py0+i*g8:.1f} h {Sm:.1f}" stroke="rgba(35,38,31,0.38)" stroke-width="0.9"/>')
    # camera frustum (north-up map: it turns with the camera): a wide, shallow trapezoid
    cu, cv, yaw = .40, .66, 12
    ccx, ccy = M(cu, cv)
    nw, fw, fl = P["fr"]
    s.add(f'<g transform="rotate({yaw} {ccx:.1f} {ccy:.1f})"><path d="M {ccx-nw:.1f} {ccy+fl*.5:.1f} L {ccx+nw:.1f} {ccy+fl*.5:.1f} '
          f'L {ccx+fw:.1f} {ccy-fl*.5:.1f} L {ccx-fw:.1f} {ccy-fl*.5:.1f} Z" fill="rgba(255,252,238,0.45)" stroke="{INK}" stroke-width="1.5" stroke-linejoin="round"/></g>')
    for (u, v) in [(.40, .70), (.47, .62), (.55, .55), (.30, .80), (.36, .84)]:
        x, y = M(u, v); s.add(f'<rect x="{x-3.5:.1f}" y="{y-3.5:.1f}" width="7" height="7" fill="{FRI}" stroke="{INK}" stroke-width="1.2"/>')
    for (u, v) in [(.58, .24), (.68, .19), (.80, .14), (.74, .28)]:
        x, y = M(u, v); s.add(f'<path d="M {x:.1f} {y-5:.1f} L {x+5:.1f} {y:.1f} L {x:.1f} {y+5:.1f} L {x-5:.1f} {y:.1f} Z" fill="{ENE}" stroke="{INK}" stroke-width="1.2"/>')
    s.add('</g>')
    # classic map border: alternating black / paper segments inside the sheet edge
    seg = Sm / 16
    s.rect(px0, py0, Sm, Sm, "none", stroke=INK, sw=1.4)
    for i in range(16):
        if i % 2 == 0:
            for (xx, yy, ww, hh) in [(px0 + i * seg, py0 + .7, seg, 2.6), (px0 + i * seg, py0 + Sm - 3.3, seg, 2.6),
                                     (px0 + .7, py0 + i * seg, 2.6, seg), (px0 + Sm - 3.3, py0 + i * seg, 2.6, seg)]:
                s.rect(xx, yy, ww, hh, INK)
    s.rect(px0 + 3.8, py0 + 3.8, Sm - 7.6, Sm - 7.6, "none", stroke=INK, sw=0.7)
    # objectives on the map use the same legend symbol as the top bar
    for (L, u, v, col, p, con) in [("A", .20, .80, OWN_G, 1, False), ("B", .14, .30, None, 0, False), ("C", .52, .50, OWN_G, .6, True),
                                   ("D", .64, .11, ENE_G, 1, False), ("E", .88, .42, ENE_G, .35, False)]:
        x, y = M(u, v)
        objective(s, x, y, L, col, p, P["mm_pt"], con, seed=31)
    # north tag (top centre) and rotate-lock (bottom-right corner)
    s.rect(cx - 12, my - 1, 24, 19, FELT, 0, stroke=TXT, sw=1.3)
    s.add(f'<path d="M {cx:.1f} {my-6:.1f} l 4 5 h -8 Z" fill="{TXT}"/>')
    s.text(cx, my + 13.5, "N", 14, TXT, "middle", tag="north")
    lx, ly = mx + D - 13, my + D - 13
    s.add(f'<circle cx="{lx:.1f}" cy="{ly:.1f}" r="11.5" fill="{FELT}" stroke="rgba({CR},0.6)" stroke-width="1"/>')
    s.add(f'<path d="M {lx-3.5:.1f} {ly-1:.1f} v -3 a 3.5 3.5 0 0 1 7 0 v 3" fill="none" stroke="{TXT}" stroke-width="1.6"/>')
    s.rect(lx - 5.5, ly - 1, 11, 7, TXT, rx=1)

    # ---------------- event feed: teleprinter tape, torn ends ----------------
    fe = [("1:12", "Rifle 2 under fire ×3", 1.0), ("1:09", "Engineers under fire", .9), ("1:07", "Point C contested", .86)]
    fy0 = TB + 10; lh = 27; th = 22
    fwid = max(tw(m, 15) + tw(t, 14, 700, "Courier Prime") for t, m, _ in fe) + 40
    s.reg("Event feed (3 lines)", W - 12 - fwid, fy0, fwid, lh * len(fe))
    for i, (t, m, op) in enumerate(fe):
        y = fy0 + i * lh
        wt_ = tw(t, 14, 700, "Courier Prime"); wm = tw(m, 15)
        wline = 12 + wt_ + 10 + wm + 14
        x1_ = W - 12; x0_ = x1_ - wline
        rr = random.Random(40 + i)
        left = " ".join(f"L {x0_ + rr.uniform(0, 3.2):.1f} {y + th - j * th / 7:.1f}" for j in range(8))
        right = " ".join(f"L {x1_ - rr.uniform(0, 3.2):.1f} {y + j * th / 7:.1f}" for j in range(8))
        tape = f"M {x0_+1:.1f} {y:.1f} {right} {left} Z"
        s.add(f'<g opacity="{op}" pointer-events="none">')
        s.add(f'<path d="{tape}" fill="rgba(0,0,0,0.38)" transform="translate(2 3)"/>')
        s.add(f'<path d="{tape}" fill="{PAPER}"/>')
        s.add(f'<clipPath id="tp{i}{W}"><path d="{tape}"/></clipPath><g clip-path="url(#tp{i}{W})">')
        texture(s, x0_, y, wline, th, 0, "pgrain")
        s.add(f'<path d="M {x0_:.1f} {y+2.5:.1f} H {x1_:.1f} M {x0_:.1f} {y+th-2.5:.1f} H {x1_:.1f}" stroke="rgba(34,37,29,0.18)" stroke-width="0.8"/>')
        s.add('</g>')
        s.text(x0_ + 12, y + 16, t, 14, INK2, wt=700, fam="Courier Prime", tag=f"feed time {t}")
        s.text(x0_ + 12 + wt_ + 10, y + 16, m, 15, INK, tag=f"feed {m}")
        s.add('</g>')

    # ---------------- bottom console ----------------
    base = H - 10
    HC = P["hc"]
    top = base - HC
    bw_, g = P["btn_w"], 4
    ow = 5 * bw_ + 4 * g
    ox, oy = W - 12 - ow, top
    s.reg("Orders / abilities / stance", ox, oy, ow, HC)
    # one felt board, ruled into square cells: abilities (canvas row), orders, stance + regroup
    felt_panel(s, ox, oy, ow, HC, PANEL, neat=False)
    bi = 6                                    # board inset (binding + stitch)
    ix0, iy0, iw, ih = ox + bi, oy + bi, ow - 2 * bi, HC - 2 * bi
    rh_ = ih / 3
    colw_ = iw / 5
    RULE = f"rgba({CR},0.42)"
    # ability row: woven canvas under the felt nap
    s.rect(ix0, iy0, iw, rh_, CANVAS)
    s.rect(ix0, iy0, iw, rh_, "url(#weave)")
    texture(s, ix0, iy0, iw, rh_)
    for r_ in (1, 2):
        s.add(f'<path d="M {ix0:.1f} {iy0+r_*rh_:.1f} H {ix0+iw:.1f}" stroke="{RULE}" stroke-width="1"/>')
    s.rect(ix0, iy0, iw, ih, "none", 0, stroke=f"rgba({CR},0.55)", sw=1.2)
    abw = iw / 4
    lab_dy = rh_ - 6

    def vrule(x, y):
        s.add(f'<path d="M {x:.1f} {y:.1f} v {rh_:.1f}" stroke="{RULE}" stroke-width="1"/>')

    abil = [("G", "Frag", ("cnt", "×2", 0)), ("T", "Smoke", ("cd", "8s", 0.45)), ("F", "Satchel", ("off", "Eng", 0)), ("B", "Barrage", None)]
    for i, (key, lab, st) in enumerate(abil):
        x = ix0 + i * abw; y = iy0
        if i:
            vrule(x, y)
        off = bool(st and st[0] == "off")
        if off:
            s.rect(x + 1, y + 1, abw - 2, rh_ - 2, "url(#hatchdim)")
        if st and st[0] == "cd":
            s.rect(x + .5, y + rh_ * (1 - st[2]), abw - 1, rh_ * st[2] - .5, f"rgba({CR},0.13)")
            s.add(f'<path d="M {x+.5:.1f} {y+rh_*(1-st[2]):.1f} h {abw-1:.1f}" stroke="rgba({CR},0.5)" stroke-width="1" stroke-dasharray="3 2"/>')
        keytab(s, x + 3, y + 3, key, dis=off)
        if st:
            s.text(x + abw - 6, y + 16, st[1], 14, OFF if off else TXT, "end", dis=off, tag=f"abil {lab} {st[1]}")
        s.text(x + abw / 2, y + lab_dy, lab, 15, OFF if off else TXT, "middle", dis=off, tag=f"abil {lab}")
    # row 2: orders
    orders = [("RMB", "Move"), ("Ctrl+RMB", "Attack"), ("H", "Stop"), ("R", "Retreat"), ("Y", "Dig in")]
    for i, (key, lab) in enumerate(orders):
        x = ix0 + i * colw_; y = iy0 + rh_
        if i:
            vrule(x, y)
        if key.endswith("RMB"):
            kx = x + 3
            if key.startswith("Ctrl"):
                kx += keytab(s, kx, y + 3, "Ctrl") + 4
            mouse(s, kx + 1, y + 3)
        else:
            keytab(s, x + 3, y + 3, key)
        s.text(x + colw_ / 2, y + lab_dy, lab, 15, TXT, "middle", tag=f"order {lab}")
    # row 3: stance (X stand, C crouch, Z prone; press the active one again = Auto) + regroup
    y = iy0 + 2 * rh_
    segw = 4 * colw_ / 3
    for i, (key, lab) in enumerate([("X", "Stand"), ("C", "Crouch"), ("Z", "Prone")]):
        x = ix0 + i * segw
        if i:
            vrule(x, y)
        keytab(s, x + 3, y + 3, key)
        if lab == "Crouch":
            s.rect(x + 2, y + 2, segw - 4, rh_ - 4, "rgba(239,201,76,0.12)")
            grease_rect(s, x + 3, y + 3, segw - 6, rh_ - 6, ACC, seed=5)
            # Auto cue: pressing the active stance again returns to Auto
            ax = x + segw - 7
            aw = tw("Auto", 14)
            s.text(ax, y + 16, "Auto", 14, ACC, "end", tag="stance auto")
            rcx, rcy = ax - aw - 8, y + 11
            s.add(f'<path d="M {rcx+4:.1f} {rcy+1.6:.1f} A 4.2 4.2 0 1 1 {rcx+1.2:.1f} {rcy-3.9:.1f}" fill="none" stroke="{ACC}" stroke-width="1.6" stroke-linecap="round"/>'
                  f'<path d="M {rcx-1.4:.1f} {rcy-6.4:.1f} L {rcx+2.6:.1f} {rcy-4.2:.1f} L {rcx-0.6:.1f} {rcy-1.4:.1f}" fill="none" stroke="{ACC}" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>')
        s.text(x + segw / 2, y + lab_dy, lab, 15, TXT, "middle", tag=f"stance {lab}")
    x = ix0 + 4 * colw_
    vrule(x, y)
    keytab(s, x + 3, y + 3, "L")
    s.text(x + colw_ / 2, y + lab_dy, "Regroup", 15, TXT, "middle", tag="order Regroup")

    # call-in column: requisition slip
    ctw = 120; tile_h = P["tile_h"]; head = P["ci_head"]; q_h = P["ci_q"]
    tiles = [("Rifle squad", "120", "ok"), ("MG team", "150", "cd"), ("Engineers", "140", "ok"), ("Mortar", "260", "poor")]
    strip_h = head + len(tiles) * tile_h
    col_h = strip_h + 4 + q_h
    cx0 = W - 12 - ctw; cy_top = oy - P["ci_gap"] - col_h
    s.reg("Call-in column", cx0, cy_top, ctw, col_h)
    paper(s, cx0, cy_top, ctw, strip_h, SLIP)
    s.rect(cx0, cy_top, ctw, head, FAC)
    texture(s, cx0, cy_top, ctw, head, 0, BTEX)
    s.rect(cx0, cy_top, ctw, 2, FAC_L)
    emblem(s, cx0 + 5, cy_top + head / 2 - 4, EMB, 10, FAC_T, FAC)
    s.sten(cx0 + 19, cy_top + head - 4.5, "CALL-IN", 15, FAC_T, tag="callin head")
    keytab(s, cx0 + ctw - 4 - (tw("Tab", 14) + 7), cy_top + (head - 15) / 2 + 0.5, "Tab", tag="callin Tab")
    s.rect(cx0, cy_top + head - 1, ctw, 1.5, INK)   # form header rule
    s.add(f'<path d="M {cx0+6.5:.1f} {cy_top+head:.1f} v {strip_h-head:.1f}" stroke="rgba(154,42,26,0.35)" stroke-width="1"/>')  # form margin rule
    for i, (nm, cost, st) in enumerate(tiles):
        y = cy_top + head + i * tile_h
        if i:
            s.add(f'<path d="M {cx0+4} {y+.5:.1f} h {ctw-8}" stroke="rgba(34,37,29,0.40)" stroke-width="1" stroke-dasharray="3 2"/>')
        poor = st == "poor"
        if st == "cd":
            s.rect(cx0 + 1, y + tile_h * 0.5, ctw - 2, tile_h * 0.5 - 1, "url(#pencil)")
        s.text(cx0 + 12, y + 16, nm, 15, INK_OFF if poor else INK, dis=poor, tag=f"callin {nm}")
        s.text(cx0 + 12, y + tile_h - 5, f"{cost} MP", 15, INK_OFF if poor else GREASE_INK, dis=poor, tag=f"callin {nm} cost")
        if st == "cd":
            s.text(cx0 + ctw - 7, y + tile_h - 5, "9s", 14, INK, "end", tag="callin cd")
            clock(s, cx0 + ctw - 7 - tw("9s", 14) - 9, y + tile_h - 10, 5.2, INK)
        if poor:
            s.text(cx0 + ctw - 6, y + 16, "need 25", 14, STAMP_INK, "end", tag="callin need")
    # perforated tear line at the bottom of the slip
    s.add(f'<path d="M {cx0+3} {cy_top+strip_h-1.5:.1f} h {ctw-6}" stroke="rgba(34,37,29,0.5)" stroke-width="1.2" stroke-dasharray="1.5 2.5"/>')
    qy = cy_top + strip_h + 4
    paper(s, cx0, qy, ctw, q_h, SLIP)
    s.rect(cx0 + 6, qy + q_h - 6, ctw - 12, 3, "rgba(34,37,29,0.20)")
    s.rect(cx0 + 6, qy + q_h - 6, (ctw - 12) * 0.62, 3, GREASE_INK)
    s.text(cx0 + 6, qy + 15, "Next: Rifle 7s", 14, INK, wt=B5, tag="queue")
    s.text(cx0 + ctw - 6, qy + 15, "2/5", 14, INK, "end", tag="queue n")

    # detail panel
    dw = P["detail_w"]
    dx = ox - 12 - dw; dy = top
    s.reg("Squad detail (men rows)", dx, dy, dw, HC)
    felt_panel(s, dx, dy, dw, HC, PANEL, neat=False)
    hh = 25
    s.rect(dx + 1, dy + 1, dw - 2, hh, FAC)
    texture(s, dx + 1, dy + 1, dw - 2, hh, 0, BTEX)
    s.rect(dx + 1, dy + 1, dw - 2, 2, FAC_L)
    frame_bar(s, dx + 1, dx + dw - 1, dy + hh + 1, 3, 20)
    neatline(s, dx, dy + hh + 1, dw, HC - hh - 1, inset=4)
    emblem(s, dx + 8, dy + 1 + hh / 2 - 6, EMB, 12, FAC_T, FAC)
    tx = dx + 26
    if P["detail_sym"]:
        counter(s, dx + 25, dy + 5, "rifle", 24, 17)
        tx = dx + 55
    s.sten(tx, dy + 19, P["detail_title"], P["detail_tsz"], FAC_T, tag="detail title")
    s.text(dx + dw - 9, dy + 19, "Holding · crouch", 14, FAC_T, "end", wt=B6, tag="detail status")
    men = [("Sgt (lead)", "45", "a"), ("Rifle 2", "40", "a"), ("Rifle 3", "45", "sel"), ("Rifle 4", "38", "a"),
           ("LMG", "180", "a"), ("Asst LMG", "30", "w"), ("Rifle 7", "42", "a"), ("Rifle 8", "44", "a")]
    r0 = dy + hh + 8
    colw = (dw - 20 - 8) / 2; rh = (base - 6 - r0) / 4
    for i, (nm, am, st) in enumerate(men):
        rr = i // 2; c = i % 2
        x = dx + 10 + c * (colw + 8); y = r0 + rr * rh
        if st == "sel":
            s.rect(x - 3, y + 1, colw + 3, rh - 2, f"rgba({CR},0.12)")
            grease_line(s, x - 2, y + 3, x - 2, y + rh - 3, ACC, seed=3, sw=3)
        pip(s, x + 4, y + (rh - 12) / 2, 12, st)
        s.text(x + 24, y + rh / 2 + 5, nm, 15, TXT, tag=f"man {nm}")
        s.text(x + colw - 4, y + rh / 2 + 5, am, 15, TXT if st == "sel" else DIM, "end", wt=B5, tag=f"man {nm} ammo")
    s.add(f'<path d="M {dx+10+colw+4:.1f} {r0+2:.1f} v {4*rh-4:.1f}" stroke="rgba({CR},0.18)" stroke-width="1"/>')

    # roster: group tabs + counter cards
    rx0 = 12; rx1 = dx - 12
    tab_h = 22
    for i, g_ in enumerate(["1", "2", "3"]):
        tx_ = rx0 + i * 32
        felt_panel(s, tx_, top, 28, tab_h, "rgba(20,28,23,0.95)" if i == 0 else PANEL, neat=False, stitch=False)
        if i == 0:
            grease_line(s, tx_ + 3, top + tab_h - 3, tx_ + 25, top + tab_h - 3, ACC, seed=9, sw=2.6)
        s.text(tx_ + 14, top + 16, g_, 15, TXT, "middle", tag=f"group {g_}")
    cg2 = 6
    ncol = P["cards_per_row"]
    cw2 = (rx1 - rx0 - (ncol - 1) * cg2) / ncol
    squads = [("Rifle 1", "8/8", "Holding", ["a", "a", "sel", "a", "a", "w", "a", "a"], True, "rifle"),
              ("Rifle 2", "6/8", "Under fire", ["a", "a", "w", "pin", "a", "det", "x", "x"], False, "rifle"),
              ("MG team", "2/3", "Pinned", ["pin", "pin", "x"], False, "mg"),
              ("Engineers", "5/5", "Moving", ["a"] * 5, False, "eng"),
              ("Mortar", "3/3", "Ready", ["a"] * 3, False, "mortar"),
              ("Sniper", "2/2", "Hidden", ["a", "a"], False, "sniper")]
    nrows = math.ceil(len(squads) / ncol)
    cards_top = top + tab_h + 6
    chh = (base - cards_top - (nrows - 1) * cg2) / nrows
    s.reg("Roster (tabs + cards)", rx0, top, ncol * cw2 + (ncol - 1) * cg2, HC)
    ps = P["pip"]
    for i, (nm, cnt, stt, pips, foc, kind) in enumerate(squads):
        rr = i // ncol; c = i % ncol
        x = rx0 + c * (cw2 + cg2); y = cards_top + rr * (chh + cg2)
        felt_panel(s, x, y, cw2, chh, PANEL, strong=False)
        counter(s, x + 8, y + 7, kind, 24, 17)
        nx = x + 39
        s.text(nx, y + 20, nm, 15, TXT, tag=f"card {nm}")
        if stt in ("Under fire", "Pinned"):
            sx = stamp(s, x + cw2 - 6, y + 5, stt, seed=i)
            lamp(s, sx - 10, y + 15.5)
            room = sx - 16 - (nx + tw(nm, 15))
            print(f"  card {nm}: {room:.0f} px between name and lamp at {W}", file=sys.stderr)
            if room < 12:
                print(f"  !! card {nm}: stamp overlaps name at {W}", file=sys.stderr)
        else:
            s.text(x + cw2 - 8, y + 20, stt, 15, DIM, "end", wt=B5, tag=f"card {nm} status")
        py = y + chh - ps - 7
        for j, p in enumerate(pips):
            pip(s, x + 10 + j * (ps + 3), py, ps, p)
        s.text(x + cw2 - 8, py + ps - 1, cnt, 15, TXT, "end", tag=f"card {nm} count")
        if foc:
            grease_rect(s, x + 2, y + 2, cw2 - 4, chh - 4, ACC, seed=21 + i)

    band = top - TB
    s.add('</svg>')
    return s, band


P1600 = dict(tb=32, chip_w=46, obj_r=10.2, ticket_w=130, mm_d=250, mm_band=14, mm_pt=9.5, fr=(14, 30, 24),
             btn_w=68, tile_h=40, ci_gap=10, ci_head=21, ci_q=26, hc=130, detail_w=300, detail_sym=True,
             cards_per_row=4, pip=13, fade=170, detail_title="RIFLE SQUAD 1", detail_tsz=16)
P1280 = dict(tb=32, chip_w=42, obj_r=10.0, ticket_w=100, mm_d=200, mm_band=12, mm_pt=9.0, fr=(11, 24, 19),
             btn_w=62, tile_h=38, ci_gap=8, ci_head=19, ci_q=24, hc=124, detail_w=262, detail_sym=False,
             cards_per_row=3, pip=12, fade=160, detail_title="RIFLE SQUAD 1", detail_tsz=15)


def overlaps(items):
    bad = []
    for i, a in enumerate(items):
        for b in items[i + 1:]:
            if a["x0"] < b["x1"] + 3 and b["x0"] < a["x1"] + 3 and a["y0"] < b["y1"] - 1 and b["y0"] < a["y1"] - 1:
                bad.append((a["s"], b["s"]))
    return bad


if __name__ == "__main__":
    out = sys.argv[1]
    fac = sys.argv[2] if len(sys.argv) > 2 and not sys.argv[2].startswith("-") else "GER"
    notext = "--notext" in sys.argv
    suf = "" if fac == "GER" else "-" + fac.lower()
    for (W, H, P) in [(1600, 900, P1600), (1280, 800, P1280)]:
        fn = f"{out}/ui-mood-blend-{W}{suf}"
        s, band = build(W, H, P, fac)
        open(fn + ".svg", "w").write("\n".join(s.o))
        print(f"== {fac} {W}x{H}  clear band={band:.0f} px ({band/H*100:.1f}%)  call-in top y={s.rects['Call-in column'][1]:.0f}")
        for a, b in overlaps(s.items):
            print(f"  !! text overlap: {a!r} / {b!r}")
        small = [it["s"] for it in s.items if it["size"] < 14]
        if small:
            print("  !! text under 14 px:", small)
        if notext:
            s2, _ = build(W, H, P, fac, notext=True)
            open(fn + ".notext.svg", "w").write("\n".join(s2.o))
            json.dump(s.items, open(fn + ".items.json", "w"))
