"""Draws the bootleg16 identity marks as plain SVG paths (no font needed to show them).

Letters come from OFL fonts in godot/fonts, turned into outlines; layouts are measured from the
real ink bounds so gaps land exactly on the 8px scale. Writes design/identity/marks/*.svg.

    python3 -I tools/make_marks.py
"""
import math
from pathlib import Path

from fontTools.pens.boundsPen import BoundsPen
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen
from fontTools.ttLib import TTFont

ROOT = Path(__file__).resolve().parent.parent
FONTS = ROOT / "godot/fonts"
OUT = ROOT / "design/identity/marks"

AMBER, WHITE, CYAN, PINK, RED = "#ffa000", "#f2f2f0", "#49c6ff", "#ff4f8b", "#ff3b3b"
BLACK_COND = FONTS / "barlow_condensed/BarlowCondensed-Black.ttf"
JP = FONTS / "noto_sans_jp/NotoSansJP[wght].ttf"


class Text:
    """A line of text as outlines: size in px, tracking in px between letters."""

    def __init__(self, font, text, size, tracking=0.0, weight=None):
        f = TTFont(font)
        self.glyphs = f.getGlyphSet(location={"wght": weight} if weight else None)
        cmap = f.getBestCmap()
        self.names = [cmap[ord(c)] for c in text]
        self.scale = size / f["head"].unitsPerEm
        self.tracking = tracking

    def _draw(self, pen, x, baseline):
        for name in self.names:
            self.glyphs[name].draw(TransformPen(pen, (self.scale, 0, 0, -self.scale, x, baseline)))
            x += self.glyphs[name].width * self.scale + self.tracking

    def bounds(self):
        """Ink box (left, top, right, bottom) when drawn at x=0 on baseline 0."""
        pen = BoundsPen(self.glyphs)
        self._draw(pen, 0, 0)
        return pen.bounds  # already y-down because of the flipped transform

    def path(self, left, top, fill):
        """The text placed so its ink box starts at (left, top)."""
        l, t, _, _ = self.bounds()
        pen = SVGPathPen(self.glyphs)
        self._draw(pen, left - l, top - t)
        return f'<path fill="{fill}" d="{pen.getCommands()}"/>'

    @property
    def size(self):
        l, t, r, b = self.bounds()
        return r - l, b - t


def svg(w, h, body):
    return f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w:g} {h:g}" width="{w:g}" height="{h:g}">{body}</svg>\n'


def poly(points, fill):
    return f'<polygon fill="{fill}" points="{" ".join(f"{x:.2f},{y:.2f}" for x, y in points)}"/>'


def wordmark():
    # BOOTLEG white, 16 amber; amber rule 16 under the letters; katakana 16 under the rule.
    boot = Text(BLACK_COND, "BOOTLEG", 192, tracking=4)
    num = Text(BLACK_COND, "16", 192, tracking=4)
    kana = Text(JP, "ブートレグ", 40, tracking=24, weight=700)
    gap_num = 8
    bw, bh = boot.size
    nw, _ = num.size
    kw, kh = kana.size
    w = bw + gap_num + nw
    rule_y = bh + 16
    kana_y = rule_y + 8 + 16
    h = kana_y + kh
    body = boot.path(0, 0, WHITE) + num.path(bw + gap_num, 0, AMBER)
    body += f'<rect fill="{AMBER}" x="0" y="{rule_y:g}" width="{w:g}" height="8"/>'
    body += kana.path((w - kw) / 2, kana_y, AMBER)
    return svg(w, h, body)


def hexagon(cx, cy, r, inner=None):
    """Pointy-top hexagon; with inner, a ring of that inner radius (even-odd)."""
    def pts(rr):
        return [(cx + rr * math.cos(math.radians(a)), cy + rr * math.sin(math.radians(a))) for a in range(-90, 270, 60)]
    d = "M" + " L".join(f"{x:.2f},{y:.2f}" for x, y in pts(r)) + " Z"
    if inner:
        d += " M" + " L".join(f"{x:.2f},{y:.2f}" for x, y in pts(inner)) + " Z"
    return d


def monogram():
    # B16 inside a hexagon ring 16 thick; the letters are sized to sit 24 clear of the ring's
    # straight sides and centred on the hexagon.
    size, r = 256, 128
    inner_width = 2 * (r - 16) * math.cos(math.radians(30))
    probe = Text(BLACK_COND, "B16", 100, tracking=2)
    letters = Text(BLACK_COND, "B16", 100 * (inner_width - 48) / probe.size[0], tracking=2)
    lw, lh = letters.size
    body = f'<path fill="{AMBER}" fill-rule="evenodd" d="{hexagon(r, r, r, r - 16)}"/>'
    body += letters.path((size - lw) / 2, (size - lh) / 2, AMBER)
    w = 2 * r * math.cos(math.radians(30))
    # trim the empty sides of the pointy-top hexagon
    return svg(w, size, f'<g transform="translate({(w - size) / 2:.2f},0)">{body}</g>')


def bureau():
    # Sun on the water: a solid half disc on the horizon (no rays), three water lines below.
    r, cx, horizon = 112, 128, 120
    sun = f'<path fill="{CYAN}" d="M{cx - r},{horizon} A{r},{r} 0 0 1 {cx + r},{horizon} Z"/>'
    water = "".join(f'<rect fill="{CYAN}" x="{cx - half}" y="{y}" width="{2 * half}" height="16" rx="8"/>'
                    for y, half in ((136, 112), (168, 80), (200, 48)))
    return svg(256, 216, sun + water)


def strays():
    # A triangle blown into three shards, each pushed 12 out from where the cracks meet.
    a, b, c = (128, 8), (248, 216), (8, 216)
    p = (132, 140)

    def lerp(u, v, t):
        return (u[0] + (v[0] - u[0]) * t, u[1] + (v[1] - u[1]) * t)

    q1, q2, q3 = lerp(a, b, 0.42), lerp(b, c, 0.4), lerp(c, a, 0.3)
    shards = ([a, q1, p, q3], [q1, b, q2, p], [q3, p, q2, c])
    body = ""
    for s in shards:
        mx, my = sum(x for x, _ in s) / len(s), sum(y for _, y in s) / len(s)
        dx, dy = mx - p[0], my - p[1]
        n = math.hypot(dx, dy)
        body += poly([(x + 12 * dx / n, y + 12 * dy / n) for x, y in s], PINK)
    return svg(272, 248, f'<g transform="translate(8,12)">{body}</g>')


def live_bug():
    # The Channel 16 bug, used only as the live mark: 16 in a rounded square, red dot top right.
    size = 128
    num = Text(BLACK_COND, "16", 72, tracking=2)
    nw, nh = num.size
    body = f'<rect x="4" y="12" width="112" height="112" rx="16" fill="none" stroke="{WHITE}" stroke-width="8"/>'
    body += num.path(4 + (112 - nw) / 2, 12 + (112 - nh) / 2, WHITE)
    body += f'<circle cx="116" cy="12" r="12" fill="{RED}"/>'
    return svg(size, size + 8, f'<g transform="translate(0,4)">{body}</g>')


if __name__ == "__main__":
    OUT.mkdir(parents=True, exist_ok=True)
    for name, make in (("wordmark", wordmark), ("monogram", monogram), ("bureau", bureau),
                       ("strays", strays), ("live16", live_bug)):
        (OUT / f"{name}.svg").write_text(make())
        print(OUT / f"{name}.svg")
