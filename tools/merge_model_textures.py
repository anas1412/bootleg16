#!/usr/bin/env python3
"""Put a model's textures inside the model: player models ship them in a second file
(`<name>t.mdl`), which the game does not need to know about. The texture file is appended to
the model, the model's texture header points into it, and the second file is deleted.
Usage: merge_model_textures.py <models dir> [--dry-run]"""
import glob
import os
import struct
import sys


def merge(path, dry):
    data = bytearray(open(path, "rb").read())
    textures = path[:-4] + "t.mdl"
    if data[:4] != b"IDST" or struct.unpack("<i", data[180:184])[0] != 0 or not os.path.exists(textures):
        return False
    tex = open(textures, "rb").read()
    count, index, data_index = struct.unpack("<iii", tex[180:192])
    if count == 0:
        return False
    data += bytes(-len(data) % 4)
    base = len(data)
    data += tex
    for i in range(count):  # each texture's pixel offset moves with the appended file
        o = base + index + i * 80 + 76
        struct.pack_into("<i", data, o, struct.unpack_from("<i", data, o)[0] + base)
    struct.pack_into("<iii", data, 180, count, base + index, base + data_index)
    struct.pack_into("<i", data, 72, len(data))  # the header's length field
    if not dry:
        open(path, "wb").write(data)
        os.remove(textures)
    return True


def main(models, dry):
    for path in sorted(glob.glob(os.path.join(models, "**", "*.mdl"), recursive=True)):
        if not path.endswith("t.mdl") and merge(path, dry):
            print("merged", os.path.relpath(path, models))


if __name__ == "__main__":
    main(sys.argv[1], "--dry-run" in sys.argv)
