## Fonts and the Godot theme, made from Tokens (design/system/tokens.json). Screens never pick
## their own fonts, sizes or colours: they ask for a type role here, or use a component.
class_name UiTheme
extends RefCounted

static var _fonts := {}
static var _theme: Theme

## The font of a type role (Tokens.TYPE), with its letter spacing and variable-font axes.
static func font(role: String) -> Font:
	if _fonts.has(role):
		return _fonts[role]
	var t: Dictionary = Tokens.TYPE[role]
	var face: String = t.font
	var v := FontVariation.new()
	v.base_font = load(Tokens.FONT_FILES[face])
	v.spacing_glyph = roundi(t.track * t.size)
	var axes: Dictionary = Tokens.FONT_AXES.get(face, {})
	if not axes.is_empty():
		var ts := TextServerManager.get_primary_interface()
		var coords := {}
		for axis in axes:
			coords[ts.name_to_tag(axis)] = axes[axis]
		v.variation_opentype = coords
	# trim the line box to the token's line height, with capitals centred in it (Godot gives
	# a line the font's whole ascent and descent, far taller than the type scale's lines)
	var ascent := v.get_ascent(t.size)
	var descent := v.get_descent(t.size)
	var cap: float = t.size * (0.8 if face == "kana" else 0.7)
	v.spacing_top = roundi((t.line + cap) / 2.0 - ascent)
	v.spacing_bottom = roundi((t.line - cap) / 2.0 - descent)
	_fonts[role] = v
	return v

static func size(role: String) -> int:
	return Tokens.TYPE[role].size

## A label in a type role. Colour defaults to the main text colour.
static func label(role: String, text := "", color := Tokens.TEXT) -> Label:
	var l := Label.new()
	l.text = text
	l.uppercase = Tokens.TYPE[role].upper
	l.add_theme_font_override("font", font(role))
	l.add_theme_font_size_override("font_size", size(role))
	l.add_theme_color_override("font_color", color)
	return l

## The theme every menu root uses: defaults for plain controls (body text, framed buttons,
## sliders). Components draw their own frames on top of this.
static func theme() -> Theme:
	if _theme:
		return _theme
	var th := Theme.new()
	th.default_font = font("body")
	th.default_font_size = size("body")
	th.set_color("font_color", "Label", Tokens.TEXT)

	for state in ["normal", "hover", "pressed", "disabled", "focus"]:
		th.set_stylebox(state, "Button", StyleBoxEmpty.new())
	th.set_font("font", "Button", font("tab"))
	th.set_font_size("font_size", "Button", size("tab"))
	for c in ["font_color", "font_hover_color", "font_pressed_color", "font_focus_color", "font_hover_pressed_color"]:
		th.set_color(c, "Button", Tokens.TEXT)
	th.set_color("font_disabled_color", "Button", Tokens.TEXT_OFF)

	# sliders: a 4 tall track, amber where it's filled, a square grabber
	var track := StyleBoxFlat.new()
	track.bg_color = Tokens.LINE
	track.content_margin_top = Tokens.STROKE_FRAME / 2.0
	track.content_margin_bottom = Tokens.STROKE_FRAME / 2.0
	var filled := track.duplicate()
	filled.bg_color = Tokens.YOU
	th.set_stylebox("slider", "HSlider", track)
	th.set_stylebox("grabber_area", "HSlider", filled)
	th.set_stylebox("grabber_area_highlight", "HSlider", filled)
	th.set_icon("grabber", "HSlider", _square(Tokens.SPACE_M, Tokens.TEXT))
	th.set_icon("grabber_highlight", "HSlider", _square(Tokens.SPACE_M, Tokens.YOU))
	# scrollbars: a 2 wide line, a dim bar you can grab, brighter under the pointer
	var bar := StyleBoxFlat.new()
	bar.bg_color = Tokens.LINE
	bar.content_margin_left = Tokens.STROKE_HAIR
	var grab := StyleBoxFlat.new()
	grab.bg_color = Tokens.TEXT_DIM
	grab.content_margin_left = Tokens.STROKE_FRAME
	var grab_hi := grab.duplicate()
	grab_hi.bg_color = Tokens.TEXT
	th.set_stylebox("scroll", "VScrollBar", bar)
	th.set_stylebox("grabber", "VScrollBar", grab)
	th.set_stylebox("grabber_highlight", "VScrollBar", grab_hi)
	th.set_stylebox("grabber_pressed", "VScrollBar", grab_hi)
	_theme = th
	return th

static func _square(px: int, color: Color) -> ImageTexture:
	var img := Image.create(px, px, false, Image.FORMAT_RGBA8)
	img.fill(color)
	return ImageTexture.create_from_image(img)
