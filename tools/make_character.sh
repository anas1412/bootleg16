#!/usr/bin/env bash
# Builds the placeholder mannequin into godot/characters/ with Blender (Steam install).
# MPFB and its assets live in a project-local Blender profile, not the user's.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
blender="${BLENDER:-$HOME/.local/share/Steam/steamapps/common/Blender/blender}"
dl="$root/design/assets/blender/dl"
export BLENDER_USER_RESOURCES="$root/design/assets/blender/profile"

if [ ! -d "$BLENDER_USER_RESOURCES/extensions/user_default/mpfb" ]; then
    "$blender" --background --command extension install-file -r user_default -e "$dl/mpfb.zip"
fi
"$blender" --background --python-exit-code 1 --python "$root/design/characters/make_mannequin.py" \
    -- "$dl" "$root/godot/characters/mannequin.glb"
