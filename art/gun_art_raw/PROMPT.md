# Prompt for the image model

You get one small game-HUD weapon picture (170x45 px, black background, a text label baked in).
Task: return the same weapon, cleaned up, as a PNG with a TRANSPARENT background.

Rules:
- Remove the text label completely (names like GLOCK, USP, mac10, FLASH, GENADE, SMOKE). Repaint
  whatever the label covered so the weapon looks whole and natural. Do not leave dots or holes.
- Remove the black background and any thin vertical or horizontal edge lines. The background must
  be fully transparent (real alpha, not black or white).
- Keep the weapon exactly as it is: same model, same side view (barrel pointing left or right as
  in the original), same proportions, same colours and shading. Do not redesign it, do not add
  hands, shadows, glow, text, or a frame.
- Upscale cleanly to 4x (680x180 px), crisp edges, no blur, no JPEG artefacts. Keep the same
  aspect ratio and keep the weapon centred with a small even margin.
- One weapon per image. Keep the file name the same (ak47.png, awp.png, ...).

Check each result on both a dark and a light background: no black fringe, no leftover letters.
