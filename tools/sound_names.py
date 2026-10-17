#!/usr/bin/env python3
"""Rename weapon sound folders by sound_names.txt and the sound names inside the models that
play them (animation events: frame, 5004, "weapons/<folder>/<file>.wav"). Safe to run again.
Usage: sound_names.py <game data dir> [--dry-run]"""
import glob
import os
import struct
import sys


def clean(name):
    return name.lower().replace(" ", "_").replace("'", "")


def rules(path):
    out = []
    for line in open(path):
        line = line.split("#", 1)[0].strip()
        if "->" in line:
            a, b = [x.strip() for x in line.split("->")]
            out.append((a, b))
    return out


def retarget(option, table):
    low = clean(option)
    for old, new in table:
        prefix = f"weapons/{old}/"
        if low.startswith(prefix):
            return f"weapons/{new}/" + low[len(prefix):]
    return option


def patch_models(models, table, dry):
    for path in sorted(glob.glob(os.path.join(models, "**", "*.mdl"), recursive=True)):
        data = bytearray(open(path, "rb").read())
        if data[:4] != b"IDST":
            continue
        changed = 0
        count, offset = struct.unpack("<ii", data[164:172])
        for i in range(count):
            o = offset + i * 176
            events, index = struct.unpack("<ii", data[o + 48:o + 56])
            for e in range(events):
                eo = index + e * 76
                if struct.unpack("<i", data[eo + 4:eo + 8])[0] != 5004:
                    continue
                old = bytes(data[eo + 12:eo + 76]).split(b"\0")[0].decode("latin1")
                new = retarget(old, table)
                if new != old and len(new) < 64:
                    data[eo + 12:eo + 76] = new.encode("latin1").ljust(64, b"\0")
                    changed += 1
        if changed:
            print(f"{os.path.relpath(path, models)}: {changed} sound names")
            if not dry:
                open(path, "wb").write(data)


def main(root, dry):
    table = rules(os.path.join(os.path.dirname(__file__), "sound_names.txt"))
    sounds = os.path.join(root, "sound", "weapons")
    for old, new in table:
        src, dst = os.path.join(sounds, old), os.path.join(sounds, new)
        if os.path.isdir(src) and not os.path.exists(dst):
            print(f"folder {old} -> {new}")
            if not dry:
                os.rename(src, dst)
    patch_models(os.path.join(root, "models"), table, dry)


if __name__ == "__main__":
    main(sys.argv[1], "--dry-run" in sys.argv)
