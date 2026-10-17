#!/usr/bin/env python3
"""One naming rule for all game data: file and folder names are lowercase, with spaces and
quotes turned into underscores / dropped. The game asks for lowercase names only.
Skips (and reports) a rename that would overwrite another file.
Usage: normalize_names.py <game data dir> [--dry-run]"""
import os
import sys


def clean(name):
    return name.lower().replace(" ", "_").replace("'", "")


def main(root, dry):
    # Deepest first, so a folder is renamed after what is inside it.
    for dirpath, dirs, files in os.walk(root, topdown=False):
        for name in files + dirs:
            new = clean(name)
            if new == name:
                continue
            src, dst = os.path.join(dirpath, name), os.path.join(dirpath, new)
            if os.path.exists(dst) and not os.path.samefile(src, dst):
                print(f"SKIP {src}: {new} already exists")
                continue
            print(f"{os.path.relpath(src, root)} -> {new}")
            if not dry:
                tmp = dst + ".renaming"  # (case-only changes on any filesystem)
                os.rename(src, tmp)
                os.rename(tmp, dst)


if __name__ == "__main__":
    main(sys.argv[1], "--dry-run" in sys.argv)
