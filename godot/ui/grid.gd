## Places things on the 12-column grid (Tokens), so screens say "column 10, three wide"
## instead of typing pixels.
class_name Grid
extends RefCounted

## Left edge of a column (0 is the first).
static func x(col: int) -> float:
	return Tokens.MARGIN_X + col * (Tokens.COLUMN + Tokens.GUTTER)

## Width of `cols` columns with the gutters between them.
static func w(cols: int) -> float:
	return cols * Tokens.COLUMN + (cols - 1) * Tokens.GUTTER

## Puts a control at column `col`, `cols` wide, with its top at `y`.
static func place(c: Control, col: int, cols: int, y: float) -> void:
	c.position = Vector2(x(col), y)
	c.size.x = w(cols)
	c.custom_minimum_size.x = w(cols)

## The bottom margin line: things that sit at the bottom end here.
static func bottom() -> float:
	return Tokens.CANVAS.y - Tokens.MARGIN_Y
