"""Convert a TTF/OTF into a three.js `typeface.json` payload.

Emits the same shape FontLoader.js consumes:
  m x y                       moveTo
  l x y                       lineTo
  q x y cpx cpy               quadratic, endpoint FIRST then control
  b x y cp1x cp1y cp2x cp2y   cubic, endpoint FIRST then two controls

BasePen is doing the load-bearing work: it decomposes multi-point TrueType
qCurveTo runs (including the all-off-curve `None`-terminated contour) into
single-control quadratics, and resolves composite glyphs against glyphSet.
"""

import json
import sys

from fontTools.pens.basePen import BasePen
from fontTools.ttLib import TTFont


class TypefacePen(BasePen):
    def __init__(self, glyph_set):
        super().__init__(glyph_set)
        self.ops = []

    def _pt(self, pt):
        return [round(pt[0]), round(pt[1])]

    def _moveTo(self, pt):
        self.ops += ["m", *self._pt(pt)]

    def _lineTo(self, pt):
        self.ops += ["l", *self._pt(pt)]

    def _qCurveToOne(self, cp, pt):
        self.ops += ["q", *self._pt(pt), *self._pt(cp)]

    def _curveToOne(self, cp1, cp2, pt):
        self.ops += ["b", *self._pt(pt), *self._pt(cp1), *self._pt(cp2)]

    def _closePath(self):
        pass


def convert(path, family_name):
    font = TTFont(path)
    head, hhea, post, os2 = font["head"], font["hhea"], font["post"], font.get("OS/2")
    glyph_set = font.getGlyphSet()
    hmtx = font["hmtx"]
    cmap = font.getBestCmap()

    glyphs = {}
    for code, name in sorted(cmap.items()):
        if code < 0x20 or 0x7F <= code < 0xA0:
            continue
        char = chr(code)
        pen = TypefacePen(glyph_set)
        glyph_set[name].draw(pen)
        advance, _lsb = hmtx[name]
        # every command is followed by an even run of ints in x,y order, so the
        # even-indexed ints are exactly the x coordinates
        xs = [v for v in pen.ops if isinstance(v, int)][0::2]
        entry = {
            "x_min": min(xs) if xs else 0,
            "x_max": max(xs) if xs else 0,
            "ha": advance,
        }
        if pen.ops:
            entry["o"] = " ".join(str(v) for v in pen.ops)
        glyphs[char] = entry

    return {
        "glyphs": glyphs,
        "familyName": family_name,
        "ascender": hhea.ascent,
        "descender": hhea.descent,
        "underlinePosition": post.underlinePosition,
        "underlineThickness": post.underlineThickness,
        "boundingBox": {
            "yMin": head.yMin,
            "xMin": head.xMin,
            "yMax": head.yMax,
            "xMax": head.xMax,
        },
        "resolution": head.unitsPerEm,
        "original_font_information": {
            "format": 0,
            "fontFamily": family_name,
            "fontSubfamily": "Regular",
        },
        "cssFontWeight": "normal" if not os2 else ("bold" if os2.usWeightClass >= 600 else "normal"),
        "cssFontStyle": "normal",
    }


if __name__ == "__main__":
    src, dst, family = sys.argv[1], sys.argv[2], sys.argv[3]
    data = convert(src, family)
    payload = json.dumps(data, ensure_ascii=False, separators=(",", ":"))
    with open(dst, "w", encoding="utf-8") as fh:
        fh.write("// Generated from %s by scripts/ttf2typeface.py — do not edit by hand.\n" % family)
        fh.write("export const data = %s\n" % payload)
    print("glyphs:", len(data["glyphs"]), "resolution:", data["resolution"])
