#!/usr/bin/env python3
"""Rename the animations inside view models (v_*.mdl) to the names the game asks for, by the
rules in sequence_names.txt. Works in place, only ever shortens or keeps a name (the label
field is 32 bytes) and never makes two animations share a new name. Safe to run again.
Usage: sequence_names.py <models dir> [--dry-run]"""
import glob
import os
import struct
import sys


def rules(path):
    prefixes, words, names, fills = [], [], {}, []
    for line in open(path):
        line = line.split("#", 1)[0].strip()
        if line.startswith("prefix:"):
            prefixes += line[7:].split()
        elif line.startswith("word:"):
            a, b = [x.strip() for x in line[5:].split("->")]
            words.append((a, b))
        elif line.startswith("name:"):
            a, b = [x.strip() for x in line[5:].split("->")]
            names[a] = b
        elif line.startswith("fill:"):
            target, sources = [x.strip() for x in line[5:].split("<-")]
            fills.append((target, sources.split()))
    return prefixes, words, names, fills


def renamed(old, prefixes, words, names):
    n = old.lower()
    for p in prefixes:
        if n.startswith(p):
            n = n[len(p):]
    for a, b in words:
        n = n.replace(a, b)
    return names.get(n, n)


def main(models, dry):
    prefixes, words, names, fills = rules(os.path.join(os.path.dirname(__file__), "sequence_names.txt"))
    for path in sorted(glob.glob(os.path.join(models, "v_*.mdl"))):
        data = bytearray(open(path, "rb").read())
        if data[:4] != b"IDST":
            continue
        count, offset = struct.unpack("<ii", data[164:172])
        old = [bytes(data[offset + i * 176: offset + i * 176 + 32]).split(b"\0")[0].decode("latin1") for i in range(count)]
        now = list(old)
        for i, label in enumerate(old):
            new = renamed(label, prefixes, words, names)
            if new != label and new not in now:
                now[i] = new
        for target, sources in fills:
            if target not in [n.lower() for n in now]:
                for src in sources:
                    hit = next((i for i, n in enumerate(now) if n.lower() == src), None)
                    if hit is not None:
                        now[hit] = target
                        break
        changes = [(a, b) for a, b in zip(old, now) if a != b]
        if changes:
            print(f"{os.path.basename(path)}: " + ", ".join(f"{a}->{b}" for a, b in changes))
            if not dry:
                for i, label in enumerate(now):
                    data[offset + i * 176: offset + i * 176 + 32] = label.encode("latin1").ljust(32, b"\0")
                open(path, "wb").write(data)


if __name__ == "__main__":
    main(sys.argv[1], "--dry-run" in sys.argv)
