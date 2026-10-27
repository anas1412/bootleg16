"""Writes the design system from its one source, design/system/tokens.json:

  godot/ui/tokens.gd           constants the Godot UI is built from
  design/system/index.html     the design system page (from design/system/page.html)

    python3 -I tools/make_design_system.py
"""
import json
from html import escape
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SYSTEM = ROOT / "design/system"
T = json.loads((SYSTEM / "tokens.json").read_text())

CSS_FONT = {  # family var, weight, extra css
    "display": ("var(--f-display)", 400, ""),
    "ui": ("var(--f-ui)", 600, ""),
    "ui_bold": ("var(--f-ui)", 700, ""),
    "mono": ("var(--f-mono)", 500, ""),
    "kana": ("var(--f-kana)", 500, ""),
}
SAMPLES = {
    "takeover": "Operation ready", "name": "Vera", "page": "Locker", "action": "Deploy",
    "tab": "Play", "body": "Nadia joined your squad", "label": "Your squad 2/5", "kana": "ヴェラ", "tab_kana": "出撃",
}
COLOUR_GROUPS = (
    ("Ground", ("bg", "panel", "panel_hi", "line")),
    ("Text", ("text", "text_dim", "text_off")),
    ("Meaning", ("you", "alert", "bureau", "strays")),
)
SPACE_USE = {
    "xs": "Icon to its text.", "s": "Between rows in a list.", "m": "Inside rows and buttons.",
    "l": "Between cards. The grid gutter.", "xl": "Between groups.", "xxl": "Between blocks of a screen.",
    "huge": "Screen margin, top and bottom.", "max": "Screen margin, left and right.",
}
MOTION = (
    ("hover_ms", "ms", "Hover in and out."), ("enter_ms", "ms", "Things arriving."),
    ("exit_ms", "ms", "Things leaving, faster than arriving."), ("takeover_ms", "ms", "Takeover bands sliding in."),
    ("stagger_ms", "ms", "Between rows entering."), ("glitch_ms", "ms", "Channel split, then clean."),
    ("blur_behind", "px", "Blur of the game behind a menu or popup."),
    ("slide", "px", "How far things travel as they arrive."),
)


def css_name(key):
    return key.replace("_", "-")


def tokens_css():
    m = T["motion"]
    lines = [f"  --{css_name(k)}: {v['hex']};" for k, v in T["color"].items()]
    lines += [f"  --s-{k}: {v}px;" for k, v in T["space"].items()]
    lines += [
        f"  --hover: {m['hover_ms']}ms; --enter: {m['enter_ms']}ms; --exit: {m['exit_ms']}ms;",
        f"  --takeover: {m['takeover_ms']}ms; --stagger: {m['stagger_ms']}ms; --glitch: {m['glitch_ms']}ms;",
        f"  --slide: {m['slide']}px; --split: {m['glitch_split']}px; --blur: {m['blur_behind']}px;",
        f"  --ease-curve: {', '.join(str(x) for x in m['ease'])};",
    ]
    return "\n".join(lines)


def swatches():
    out = []
    for group, keys in COLOUR_GROUPS:
        items = "".join(
            f'<div class="sw"><i style="background:var(--{css_name(k)})"></i><div><b>{k.replace("_", " ")}</b>'
            f'<code>{T["color"][k]["hex"]}</code><span>{escape(T["color"][k]["use"])}</span></div></div>'
            for k in keys)
        out.append(f'<div class="swgroup"><span class="lab">{group}</span><div class="swatches">{items}</div></div>')
    return "\n".join(out)


def type_specimens():
    out = []
    for role, t in T["type"].items():
        fam, weight, extra = CSS_FONT[t["font"]]
        family = T["font"][t["font"]]["family"]
        case = "text-transform:uppercase;" if t["upper"] else ""
        colour = "color:var(--text-dim);" if role == "kana" else ""
        style = (f"--size:{t['size']}px;font:{weight} {t['size']}px/{t['line']}px {fam};{extra}"
                 f"letter-spacing:{t['track']}em;{case}{colour}")
        spec = f"{family} {t['size']}/{t['line']}" + (f" · +{round(t['track'] * 100)}%" if t["track"] else "")
        out.append(f'<div class="ty"><div class="spec"><b>{role}</b><code>{spec}</code><span>{escape(t["use"])}</span></div>'
                   f'<div class="sample" style="{style}">{SAMPLES[role]}</div></div>')
    return "\n".join(out)


def space_scale():
    return "\n".join(
        f'<div class="sp"><code>{k}</code><code>{v}</code><span style="display:flex;gap:16px;align-items:center">'
        f'<i style="width:{v * 2}px;flex:none"></i>{SPACE_USE[k]}</span></div>'
        for k, v in T["space"].items())


def motion_list():
    m = T["motion"]
    items = "".join(f'<div class="mo"><b>{m[k]}<small style="font-size:24px"> {unit}</small></b><span>{use}</span></div>'
                    for k, unit, use in MOTION)
    return f'<div class="motion">{items}</div>'


def page():
    html = (SYSTEM / "page.html").read_text()
    for name in ("wordmark", "monogram", "bureau", "strays", "live16"):
        svg = (ROOT / f"design/identity/marks/{name}.svg").read_text().strip()
        html = html.replace(f"{{{{mark:{name}}}}}", svg)
    fills = {"/*{{tokens_css}}*/": tokens_css(), "{{swatches}}": swatches(), "{{type_specimens}}": type_specimens(),
             "{{cols}}": "<i></i>" * T["grid"]["columns"], "{{space_scale}}": space_scale(),
             "{{motion_list}}": motion_list()}
    for slot, value in fills.items():
        html = html.replace(slot, value)
    html = html.replace("<!-- Template: tools/make_design_system.py fills the {{...}} slots from tokens.json and writes index.html. -->",
                        "<!-- Generated by tools/make_design_system.py from page.html and tokens.json. Don't edit by hand. -->")
    assert "{{" not in html, "unfilled slot in page.html"
    assert "—" not in html, "em dash in the page"
    return html


def gd_value(v):
    if isinstance(v, bool):
        return "true" if v else "false"
    if isinstance(v, str):
        return json.dumps(v)
    if isinstance(v, dict):
        return "{" + ", ".join(f"{json.dumps(k)}: {gd_value(x)}" for k, x in v.items() if not k.startswith("_")) + "}"
    if isinstance(v, list):
        return "[" + ", ".join(gd_value(x) for x in v) + "]"
    return repr(v)


def tokens_gd():
    g, m = T["grid"], T["motion"]
    out = ["## Generated by tools/make_design_system.py from design/system/tokens.json. Don't edit by hand.",
           "class_name Tokens", "",
           f"const CANVAS := Vector2i({g['canvas'][0]}, {g['canvas'][1]})",
           f"const MARGIN_X := {g['margin_x']}", f"const MARGIN_Y := {g['margin_y']}",
           f"const COLUMNS := {g['columns']}", f"const GUTTER := {g['gutter']}",
           f"const COLUMN := {(g['canvas'][0] - 2 * g['margin_x'] - (g['columns'] - 1) * g['gutter']) // g['columns']}", ""]
    out += [f"const SPACE_{k.upper()} := {v}" for k, v in T["space"].items()]
    out += [""] + [f"const STROKE_{k.upper()} := {v}" for k, v in T["stroke"].items()]
    out += [""] + [f'const {k.upper()} := Color("{v["hex"]}")  # {v["use"]}' for k, v in T["color"].items()]
    out += ["", "const FONT_FILES := {" + ", ".join(f'"{k}": "res://fonts/{v["file"]}"' for k, v in T["font"].items()) + "}"]
    axes = {k: {a: v[a] for a in ("wdth", "wght") if a in v} for k, v in T["font"].items()}
    out += [f"const FONT_AXES := {gd_value({k: v for k, v in axes.items() if v})}"]
    out += [f"const FONT_CAP := {gd_value({k: v['cap'] for k, v in T['font'].items()})}"]
    out += ["", f"const TYPE := {gd_value(T['type'])}", ""]
    out += [f"const {k.upper().replace('_MS', '')} := {v / 1000:g}  # seconds" for k, v in m.items() if k.endswith("_ms")]
    out += [f"const SLIDE := {m['slide']}", f"const GLITCH_SPLIT := {m['glitch_split']}", f"const BLUR_BEHIND := {m['blur_behind']}",
            "const EASE_TRANS := Tween.TRANS_EXPO", "const EASE_TYPE := Tween.EASE_OUT", ""]
    return "\n".join(out)


if __name__ == "__main__":
    (SYSTEM / "index.html").write_text(page())
    gd = ROOT / "godot/ui/tokens.gd"
    gd.parent.mkdir(exist_ok=True)
    gd.write_text(tokens_gd())
    print(SYSTEM / "index.html")
    print(gd)
