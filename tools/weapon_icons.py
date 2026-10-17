#!/usr/bin/env python3
"""Cut the cleaned 3x6 weapon sheet (see docs: dist/weapons_sheet/prompt.txt) into one PNG per
weapon for the buy menu: gfx/guns/<name>.png in the add-on. Removes checkerboard patches that
the image model left inside holes (trigger guards, handles): flat white / light-grey areas.
Usage: weapon_icons.py <sheet.png> <out_dir> [--checker]  (checker removal is off by default:
it can nibble light metal; a sheet with real transparency does not need it)."""
import sys
import numpy as np
from PIL import Image

NAMES = ["glock18", "usp", "p228", "deagle", "mac10", "tmp", "mp5navy", "ump45", "p90",
         "galil", "famas", "ak47", "m4a1", "scout", "awp", "flashbang", "hegrenade", "smokegrenade"]
COLS, ROWS = 3, 6
CHECKER = len(sys.argv) > 3 and sys.argv[3] == "--checker"  # only for sheets with baked-in chessboards
# Sheet layout (relative to a 2200x1360 sheet): 680x180 cells, 40 px gaps.
CELL_W, CELL_H, GAP, SHEET_W = 680, 180, 40, 2200


def flat(mask, r=3):
    """Pixels whose whole (2r+1)^2 neighbourhood is in `mask`."""
    h, w = mask.shape
    p = np.pad(mask, r, constant_values=False)
    out = np.ones_like(mask)
    for dy in range(2 * r + 1):
        for dx in range(2 * r + 1):
            out &= p[dy:dy + h, dx:dx + w]
    return out


def grow(seed, mask, steps):
    """`seed` spread over `mask` by `steps` pixels."""
    h, w = mask.shape
    cur = seed.copy()
    for _ in range(steps):
        p = np.pad(cur, 1, constant_values=False)
        n = cur.copy()
        for dy in (0, 1, 2):
            for dx in (0, 1, 2):
                n |= p[dy:dy + h, dx:dx + w]
        cur = n & mask
    return cur


def components(mask):
    """4-connected components of `mask`: a list of (ys, xs) arrays."""
    h, w = mask.shape
    seen = np.zeros_like(mask)
    out = []
    for y0, x0 in zip(*np.nonzero(mask)):
        if seen[y0, x0]:
            continue
        stack, ys, xs = [(y0, x0)], [], []
        seen[y0, x0] = True
        while stack:
            y, x = stack.pop()
            ys.append(y)
            xs.append(x)
            for ny, nx in ((y - 1, x), (y + 1, x), (y, x - 1), (y, x + 1)):
                if 0 <= ny < h and 0 <= nx < w and mask[ny, nx] and not seen[ny, nx]:
                    seen[ny, nx] = True
                    stack.append((ny, nx))
        out.append((np.array(ys), np.array(xs)))
    return out


def clean(cell):
    """The patches are a white / light-grey chessboard. A patch is a small blob of those two
    flat colours that contains real white (the weapons' own art has little pure white)."""
    a = cell[..., 3] > 128
    rgb = cell[..., :3].astype(int)
    grey = (rgb.max(-1) - rgb.min(-1)) <= 8
    v = rgb.mean(-1)
    checker = a & grey & ((v >= 246) | ((v >= 218) & (v <= 232)))
    kill = np.zeros_like(a)
    for ys, xs in components(checker):
        area = len(ys)
        box = (ys.max() - ys.min() + 1) * (xs.max() - xs.min() + 1)
        white = int((v[ys, xs] >= 251).sum())
        if 25 <= area <= 9000 and white >= 12 and area / box > 0.3:
            kill[ys, xs] = True
    # the soft edge of a removed patch
    kill |= grow(kill, a & grey & (v >= 150), 2)
    cell = cell.copy()
    cell[kill, 3] = 0
    return cell, int(kill.sum())


def main(src, out):
    import os
    im = np.array(Image.open(src).convert("RGBA"))
    s = im.shape[1] / SHEET_W
    os.makedirs(out, exist_ok=True)
    for i, name in enumerate(NAMES):
        r, c = divmod(i, COLS)
        x0, y0 = round((GAP + c * (CELL_W + GAP)) * s), round((GAP + r * (CELL_H + GAP)) * s)
        cell = im[y0:y0 + round(CELL_H * s), x0:x0 + round(CELL_W * s)]
        cell, n = clean(cell) if CHECKER else (cell, 0)
        ys, xs = np.nonzero(cell[..., 3] > 8)
        pad = 4
        box = (max(xs.min() - pad, 0), max(ys.min() - pad, 0), xs.max() + pad + 1, ys.max() + pad + 1)
        Image.fromarray(cell).crop(box).save(f"{out}/{name}.png")
        print(f"{name}: {box[2] - box[0]}x{box[3] - box[1]}, {n} checker px removed")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
