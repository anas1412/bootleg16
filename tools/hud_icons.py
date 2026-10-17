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


def main(src, out):
    im = np.array(Image.open(src).convert("RGB")).astype(float)
    sx, sy = im.shape[1] / W, im.shape[0] / H
    # White over green: red and blue stay low only where the green shows.
    bg = np.median(np.minimum(im[:20, :20, 0], im[:20, :20, 2]))
    alpha = np.clip((np.minimum(im[..., 0], im[..., 2]) - bg) / (255 - bg), 0, 1)
    rgba = np.zeros(im.shape[:2] + (4,), np.uint8)
    rgba[..., :3] = 255
    rgba[..., 3] = np.round(alpha * 255)
    os.makedirs(out, exist_ok=True)
    for n, name in CELLS.items():
        if not name:
            continue
        r, c = divmod(n - 1, COLS)
        x0, y0 = round((GAP + c * (CELL_W + GAP)) * sx), round((GAP + r * (CELL_H + GAP)) * sy)
        # A little past the cell: wide weapons may run into the gap (never into another icon).
        m = round(GAP / 2 * sx)
        x0, y0 = max(x0 - m, 0), max(y0 - m, 0)
        cell = rgba[y0:y0 + round(CELL_H * sy) + 2 * m, x0:x0 + round(CELL_W * sx) + 2 * m]
        ys, xs = np.nonzero(cell[..., 3] > 20)
        if len(xs) == 0:
            print(f"{name}: EMPTY")
            continue
        box = (max(xs.min() - 2, 0), max(ys.min() - 2, 0), xs.max() + 3, ys.max() + 3)
        Image.fromarray(cell).crop(box).save(f"{out}/{name}.png")
        print(f"{name}: {box[2] - box[0]}x{box[3] - box[1]}")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
