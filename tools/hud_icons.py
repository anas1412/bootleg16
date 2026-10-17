#!/usr/bin/env python3
"""Cut the green-screen HUD icon sheet (7x6 grid, see dist/icons_sheet/prompt.txt) into
gfx/hud/<name>.png in the add-on: pure white glyphs with real alpha, tinted by the game.
Usage: hud_icons.py <sheet.png> <out_dir>"""
import os
import sys
import numpy as np
from PIL import Image

# Cells set to None are guns the game does not sell (their icons are not cut).
# Cell number (1-based, row by row) -> icon name used by client/src/hud/icons.rs.
CELLS = {1: "cross", 2: "suit_full", 3: "suithelmet_full", 4: "defuser", 5: "c4", 6: "buyzone", 7: "stopwatch",
         8: "d_headshot", 9: "d_skull", 10: "d_knife", 11: "d_glock18", 12: "d_usp", 13: "d_p228", 14: "d_deagle",
         15: None, 16: None, 17: "d_mac10", 18: "d_tmp", 19: "d_mp5navy", 20: "d_ump45", 21: "d_p90",
         22: None, 23: None, 24: "d_galil", 25: "d_famas", 26: "d_ak47", 27: "d_m4a1",
         28: None, 29: None, 30: "d_scout", 31: "d_awp", 32: None, 33: None,
         34: None, 35: None, 36: "d_flashbang", 37: "d_smoke", 42: "d_grenade"}
COLS, ROWS, CELL_W, CELL_H, GAP, W, H = 7, 6, 400, 240, 40, 3120, 1720


def label(mask, grow=6):
    """Groups of `mask` pixels (gaps up to `grow` px are bridged): a list of (ys, xs)."""
    h, w = mask.shape
    p = np.pad(mask, grow)
    bridged = np.zeros_like(mask)
    for dy in range(2 * grow + 1):
        for dx in range(2 * grow + 1):
            bridged |= p[dy:dy + h, dx:dx + w]
    seen = np.zeros_like(mask)
    groups = []
    for y0, x0 in zip(*np.nonzero(bridged)):
        if seen[y0, x0]:
            continue
        stack, pts = [(y0, x0)], []
        seen[y0, x0] = True
        while stack:
            y, x = stack.pop()
            if mask[y, x]:
                pts.append((y, x))
            for ny, nx in ((y - 1, x), (y + 1, x), (y, x - 1), (y, x + 1)):
                if 0 <= ny < h and 0 <= nx < w and bridged[ny, nx] and not seen[ny, nx]:
                    seen[ny, nx] = True
                    stack.append((ny, nx))
        if pts:
            a = np.array(pts)
            groups.append((a[:, 0], a[:, 1]))
    return groups


def main(src, out):
    im = np.array(Image.open(src).convert("RGB")).astype(float)
    sx, sy = im.shape[1] / W, im.shape[0] / H
    # White over green: red and blue stay low only where the green shows.
    bg = np.median(np.minimum(im[:20, :20, 0], im[:20, :20, 2]))
    alpha = np.clip((np.minimum(im[..., 0], im[..., 2]) - bg) / (255 - bg), 0, 1)
    alpha[alpha < 0.08] = 0  # the green is not perfectly flat: no faint boxes behind icons
    a8 = np.round(alpha * 255).astype(np.uint8)
    # Each icon is found as a whole shape and given to the cell its centre is nearest to, so a
    # weapon that runs past its cell is neither cut off nor mixed with its neighbour.
    cells = {}
    for ys, xs in label(a8 > 20):
        c = min(max(round(((xs.mean() / sx) - GAP - CELL_W / 2) / (CELL_W + GAP)), 0), COLS - 1)
        r = min(max(round(((ys.mean() / sy) - GAP - CELL_H / 2) / (CELL_H + GAP)), 0), ROWS - 1)
        cells.setdefault(r * COLS + c + 1, []).append((ys, xs))
    os.makedirs(out, exist_ok=True)
    for n, name in CELLS.items():
        if not name:
            continue
        if n not in cells:
            print(f"{name}: EMPTY")
            continue
        ys = np.concatenate([g[0] for g in cells[n]])
        xs = np.concatenate([g[1] for g in cells[n]])
        box = (max(xs.min() - 2, 0), max(ys.min() - 2, 0), xs.max() + 3, ys.max() + 3)
        rgba = np.zeros(a8.shape + (4,), np.uint8)
        rgba[..., :3] = 255
        rgba[ys, xs, 3] = a8[ys, xs]
        Image.fromarray(rgba).crop(box).save(f"{out}/{name}.png")
        print(f"{name}: {box[2] - box[0]}x{box[3] - box[1]}")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
