# bootleg16

## Code layout
- No god files. Keep `main.rs` small: app setup + plugin registration only.
- One module (file) per concern: menu, hud, buy, players, effects, sound, viewmodel, map, ...
- Each module exposes a Bevy `Plugin` that registers its own systems; main.rs just adds plugins.
- Split a file once it grows past ~400 lines or holds two unrelated jobs.
- Shared helpers live in one place (e.g. `ui.rs` for UI widgets/colors); reuse, don't duplicate.

## Builds
- Always: `systemd-run --user --scope -q -p MemoryMax=3G -- cargo build -p <crate> -j 2`.

## Docs
- Open items go in docs/TODO.md, lessons in docs/GOTCHAS.md.
