#!/usr/bin/env python3
"""Build the title face: Noto Serif CJK JP Black (OFL, Source Han Serif), compressed to 80 %
across the way NERV's title cards squeeze Matisse EB. The squeeze is baked into the file
(outlines, advances, kerning), so Godot and the browser set it tight with no transform.
Keeps Latin, punctuation, kana and the JIS level 1 kanji (what titles and names need).
Usage: make_title_font.py <NotoSerifCJK-Black.ttc> <out.otf>
(Arch: /usr/share/fonts/noto-cjk/NotoSerifCJK-Black.ttc, package noto-fonts-cjk.)"""
import sys
from fontTools import subset
from fontTools.pens.t2CharStringPen import T2CharStringPen
from fontTools.pens.transformPen import TransformPen
from fontTools.ttLib import TTCollection

SQUEEZE = 0.8
FAMILY = "Noto Serif JP Title"


def wanted():
    """The code points kept: Latin, punctuation, CJK symbols, kana, full-width forms, JIS level 1."""
    cps = set(range(0x20, 0x7F)) | set(range(0xA0, 0x180)) | set(range(0x2000, 0x2070))
    cps |= set(range(0x3000, 0x3100)) | set(range(0xFF00, 0xFFF0))
    for hi in range(0xB0, 0xD0):
        for lo in range(0xA1, 0xFF):
            try:
                cps.add(ord(bytes([hi, lo]).decode("euc_jp")))
            except UnicodeDecodeError:
                pass
    return cps


def squeeze_outlines(font):
    """Every glyph drawn again at SQUEEZE across, with its advance and side bearing to match."""
    top = font["CFF "].cff.topDictIndex[0]
    strings = top.CharStrings
    hmtx = font["hmtx"].metrics
    for name in font.getGlyphOrder():
        old = strings[name]
        width = round(hmtx[name][0] * SQUEEZE)
        pen = T2CharStringPen(width, None)
        old.draw(TransformPen(pen, (SQUEEZE, 0, 0, 1, 0, 0)))
        new = pen.getCharString(private=old.private, globalSubrs=old.globalSubrs)
        strings.charStringsIndex[strings.charStrings[name]] = new
        hmtx[name] = (width, round(hmtx[name][1] * SQUEEZE))
    # the new charstrings carry their width outright (measured from 0), not from the old defaults
    for fd in top.FDArray if hasattr(top, "FDArray") else [top]:
        fd.Private.defaultWidthX = 0
        fd.Private.nominalWidthX = 0


def squeeze_positions(font):
    """Kerning and other horizontal positioning (GPOS) scaled like the outlines."""
    if "GPOS" not in font:
        return

    def fix(v):
        for f in ("XAdvance", "XPlacement"):
            if v is not None and getattr(v, f, None):
                setattr(v, f, round(getattr(v, f) * SQUEEZE))

    for lookup in font["GPOS"].table.LookupList.Lookup:
        for sub in lookup.SubTable:
            sub = getattr(sub, "ExtSubTable", sub)
            value = getattr(sub, "Value", None)  # single adjustment: one record, or one per glyph
            for v in value if isinstance(value, list) else [value]:
                fix(v)
            for ps in getattr(sub, "PairSet", None) or []:
                for rec in ps.PairValueRecord:
                    fix(rec.Value1)
                    fix(rec.Value2)
            for c1 in getattr(sub, "Class1Record", None) or []:
                for c2 in c1.Class2Record:
                    fix(c2.Value1)
                    fix(c2.Value2)


def rename(font):
    for rec in font["name"].names:
        if rec.nameID in (1, 16):
            rec.string = FAMILY
        elif rec.nameID == 4:
            rec.string = FAMILY + " Black"
        elif rec.nameID == 6:
            rec.string = FAMILY.replace(" ", "") + "-Black"


def main(ttc, out):
    font = next(f for f in TTCollection(ttc).fonts if "JP" in f["name"].getDebugName(4))
    opts = subset.Options()
    opts.layout_features = ["*"]
    opts.name_IDs = ["*"]
    opts.notdef_outline = True
    sub = subset.Subsetter(opts)
    sub.populate(unicodes=wanted())
    sub.subset(font)
    squeeze_outlines(font)
    squeeze_positions(font)
    font["hhea"].advanceWidthMax = max(w for w, _ in font["hmtx"].metrics.values())
    font["OS/2"].xAvgCharWidth = round(font["OS/2"].xAvgCharWidth * SQUEEZE)
    rename(font)
    font.save(out)


if __name__ == "__main__":
    main(*sys.argv[1:3])
