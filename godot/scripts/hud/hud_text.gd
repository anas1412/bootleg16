## Text and icons over the game: labels in a type role with a dark edge (they read on any
## wall), and the game's own white HUD icons (`ui/icons/hud`, drawn by tools/hud_icons.py),
## tinted by colour like the game tints them.
class_name HudText
extends RefCounted

static var _icons := {}

static func label(role: String, text := "", color := Tokens.TEXT) -> Label:
	var l := UiTheme.label(role, text, color)
	l.add_theme_color_override("font_shadow_color", Color(Tokens.BG, 0.85))
	l.add_theme_constant_override("shadow_offset_x", 0)
	l.add_theme_constant_override("shadow_offset_y", 1)
	l.add_theme_constant_override("shadow_outline_size", 6)
	l.mouse_filter = Control.MOUSE_FILTER_IGNORE
	return l

## Money as the HUD writes it: $3,250.
static func money(n: int) -> String:
	var s := str(absi(n))
	var out := ""
	while s.length() > 3:
		out = "," + s.right(3) + out
		s = s.left(s.length() - 3)
	return ("-" if n < 0 else "") + "$" + s + out

static func icon_texture(name: String) -> Texture2D:
	if not _icons.has(name):
		_icons[name] = load("res://ui/icons/hud/%s.png" % name)
	return _icons[name]

## An icon `height` tall, its width from the picture's shape (at most `max_width`).
static func icon(name: String, height: float, color := Tokens.TEXT, max_width := 0.0) -> TextureRect:
	var t := TextureRect.new()
	t.texture = icon_texture(name)
	t.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
	t.stretch_mode = TextureRect.STRETCH_KEEP_ASPECT_CENTERED
	var w := height * t.texture.get_width() / t.texture.get_height()
	if max_width > 0.0:
		w = minf(w, max_width)
	t.custom_minimum_size = Vector2(w, height)
	t.modulate = color
	t.mouse_filter = Control.MOUSE_FILTER_IGNORE
	return t

## Swaps an icon's picture, keeping its height.
static func set_icon(t: TextureRect, name: String, max_width := 0.0) -> void:
	t.texture = icon_texture(name)
	var h := t.custom_minimum_size.y
	var w := h * t.texture.get_width() / t.texture.get_height()
	t.custom_minimum_size.x = minf(w, max_width) if max_width > 0.0 else w
