#!/bin/sh
# export_faces.sh: renders the assistants' layered face sheets for the game from
# design/mockups/ui/faces.js (the one source). Needs the mockups server (port 8716).
# Output: client/assets/assistant/rig_<face>.png, 11 layers × 9 moods, 256 px cells.
cd "$(dirname "$0")/.."
for w in claudette childgbt sister brother grumpk; do
  chromium --headless=new --disable-gpu --hide-scrollbars --default-background-color=00000000 --window-size=2816,2304 \
    --virtual-time-budget=4000 --screenshot="$PWD/client/assets/assistant/rig_$w.png" "http://localhost:8716/mockups/ui/export.html?w=$w" >/dev/null 2>&1
done
ls -la client/assets/assistant/rig_*.png
