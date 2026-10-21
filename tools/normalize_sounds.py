#!/usr/bin/env python3
"""One loudness for the weapon sounds, which come from many packs at very different levels:
every file's peak is brought to a target by kind: shots (the `<weapon>-1.wav` files straight
under sound/weapons/) loud, everything else (reloads, bolts, draws) quieter. 16 and 8 bit mono
PCM. Safe to run again (a file already at its target is left alone).
Usage: normalize_sounds.py <game data dir> [--dry-run]"""
import glob
import os
import re
import sys
import wave

import numpy as np

SHOT_PEAK = 0.80    # of full scale (about -2 dBFS)
FOLEY_PEAK = 0.35   # about -9 dBFS
# shots: `<weapon>-1.wav`, and the two named without the dash (awp1, usp1)
SHOT = re.compile(r"(-\d+|^awp\d|^usp\d)\.wav$")


def normalize(path, dry):
    with wave.open(path) as w:
        params, frames = w.getparams(), w.readframes(w.getnframes())
    if params.nchannels != 1 or params.sampwidth not in (1, 2):
        return None
    if params.sampwidth == 2:
        x = np.frombuffer(frames, dtype="<i2").astype(np.float32) / 32768.0
    else:
        x = (np.frombuffer(frames, dtype=np.uint8).astype(np.float32) - 128.0) / 128.0
    peak = float(np.abs(x).max()) if len(x) else 0.0
    if peak < 1e-4:
        return None
    rel = os.path.basename(os.path.dirname(path)) == "weapons"
    target = SHOT_PEAK if rel and SHOT.search(os.path.basename(path)) else FOLEY_PEAK
    gain = target / peak
    if abs(gain - 1.0) < 0.03:
        return None
    y = np.clip(x * gain, -1.0, 1.0)
    out = (y * 32767.0).astype("<i2").tobytes() if params.sampwidth == 2 else (y * 127.0 + 128.0).astype(np.uint8).tobytes()
    if not dry:
        with wave.open(path, "wb") as w:
            w.setparams(params)
            w.writeframes(out)
    return peak, target


def main(root, dry):
    files = sorted(glob.glob(os.path.join(root, "sound", "weapons", "**", "*.wav"), recursive=True))
    done = 0
    for p in files:
        if normalize(p, dry):
            done += 1
    print(f"{done} of {len(files)} weapon sounds adjusted")


if __name__ == "__main__":
    main(sys.argv[1], "--dry-run" in sys.argv)
