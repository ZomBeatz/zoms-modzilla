"""Build the Zom's Modzilla icons, wordmarks, social preview and Steam artwork, all from
branding/modzilla-icon.svg (text outlined from Open Sans, like the Zomello logo).

Run from the repo root: python3 branding/make-logos.py
Needs fontTools and rsvg-convert (librsvg) for the PNGs.
"""
import re
import subprocess
from fontTools.ttLib import TTFont
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen

FONTS = "/usr/share/fonts/TTF/"
OUT = "branding"

# The icon, without its <svg> wrapper, from the one source of truth.
ICON = re.search(r"<svg[^>]*>(.*)</svg>", open(f"{OUT}/modzilla-icon.svg").read(), re.S).group(1)


def text_path(text, font_file, size, x, baseline, tracking=0.0):
    f = TTFont(FONTS + font_file)
    gs, cmap, hmtx = f.getGlyphSet(), f.getBestCmap(), f["hmtx"]
    scale = size / f["head"].unitsPerEm
    pen = SVGPathPen(gs)
    cx = x
    for ch in text:
        g = cmap[ord(ch)]
        gs[g].draw(TransformPen(pen, (scale, 0, 0, -scale, cx, baseline)))
        cx += hmtx[g][0] * scale + tracking * size
    return pen.getCommands(), cx - tracking * size


def logo(dark):
    ink, blue, sub = ("#ffffff", "#4fb0ec", "#9aa3b5") if dark else ("#14171f", "#2580c3", "#5b6375")
    zom, x1 = text_path("Zom's", "OpenSans-ExtraBold.ttf", 170, 345, 205)
    product, x2 = text_path("Modzilla", "OpenSans-ExtraBold.ttf", 170, x1 + 44, 205)
    tag, x3 = text_path("MOD MANAGER FOR LINUX", "OpenSans-Bold.ttf", 34, 349, 262, tracking=0.16)
    w = round(max(x2, x3) + 35)
    return f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} 300" width="{w}" height="300">
  <g transform="scale(0.29297)">{ICON}</g>
  <path d="{zom}" fill="{ink}"/>
  <path d="{product}" fill="{blue}"/>
  <path d="{tag}" fill="{sub}"/>
</svg>
''', w


def social():
    """1280 x 640: the dark wordmark on the dark background, with the promise under it."""
    inner, w = logo(True)
    body = re.search(r"<svg[^>]*>(.*)</svg>", inner, re.S).group(1)
    line, lw = text_path("Every mod you download, kept in order.", "OpenSans-SemiBold.ttf", 38, 0, 0)
    scale = 900 / w
    return f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1280 640" width="1280" height="640">
  <defs>
    <linearGradient id="page" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#1a1e27"/>
      <stop offset="1" stop-color="#0e1116"/>
    </linearGradient>
  </defs>
  <rect width="1280" height="640" fill="url(#page)"/>
  <g transform="translate({(1280 - 900) / 2} 180) scale({scale})">{body}</g>
  <path transform="translate({(1280 - lw) / 2} 462)" d="{line}" fill="#c9d1e0"/>
</svg>
'''


for name, svg in (("modzilla-logo", logo(False)[0]), ("modzilla-logo-dark", logo(True)[0]), ("social-preview", social())):
    open(f"{OUT}/{name}.svg", "w").write(svg)
    width = "1280" if name == "social-preview" else "920"
    subprocess.run(["rsvg-convert", "-w", width, f"{OUT}/{name}.svg", "-o", f"{OUT}/{name}.png"], check=True)


# The app's icons (the window, the app menu, the .deb/.rpm/AppImage) and the page's icon.
def png(svg_file, out, w, h=None):
    args = ["rsvg-convert", "-w", str(w)] + (["-h", str(h)] if h else []) + [svg_file, "-o", out]
    subprocess.run(args, check=True)


png(f"{OUT}/modzilla-icon.svg", f"{OUT}/modzilla-icon-1024.png", 1024)
for name, size in (("32x32", 32), ("128x128", 128), ("128x128@2x", 256), ("256x256", 256), ("512x512", 512), ("icon", 512)):
    png(f"{OUT}/modzilla-icon.svg", f"src-tauri/icons/{name}.png", size)
open("public/app-icon.svg", "w").write(open(f"{OUT}/modzilla-icon.svg").read())


# Steam's artwork for Add to Steam: library portrait, wide capsule, hero banner, logo, icon.
def steam_art():
    blue, ink, sub = "#4fb0ec", "#ffffff", "#9aa3b5"
    page = '<linearGradient id="page" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#232833"/><stop offset="1" stop-color="#12151b"/></linearGradient>'

    def words(size, x, baseline):
        zom, x1 = text_path("Zom's", "OpenSans-ExtraBold.ttf", size, x, baseline)
        mod, x2 = text_path("Modzilla", "OpenSans-ExtraBold.ttf", size, x1 + size * 0.26, baseline)
        return f'<path d="{zom}" fill="{ink}"/><path d="{mod}" fill="{blue}"/>', x2 - x

    def icon(x, y, size):
        return f'<g transform="translate({x} {y}) scale({size / 1024})">{ICON}</g>'

    def save(name, w, h, body, bg=True):
        back = f'<defs>{page}</defs><rect width="{w}" height="{h}" fill="url(#page)"/>' if bg else ""
        path = f"src-tauri/steam-art/{name}.svg"
        open(path, "w").write(f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}">{back}{body}</svg>')
        png(path, f"src-tauri/steam-art/{name}.png", w, h)

    # Portrait (600 x 900): the icon, then the name on two lines.
    z, zw = text_path("Zom's", "OpenSans-ExtraBold.ttf", 64, 0, 0)
    m, mw = text_path("Modzilla", "OpenSans-ExtraBold.ttf", 64, 0, 0)
    t, tw = text_path("MOD MANAGER", "OpenSans-Bold.ttf", 22, 0, 0, tracking=0.16)
    save("portrait", 600, 900, icon(150, 170, 300)
         + f'<path transform="translate({(600 - zw) / 2} 600)" d="{z}" fill="{ink}"/>'
         + f'<path transform="translate({(600 - mw) / 2} 680)" d="{m}" fill="{blue}"/>'
         + f'<path transform="translate({(600 - tw) / 2} 740)" d="{t}" fill="{sub}"/>')
    # Wide capsule (920 x 430): the icon beside the name, on two lines like the portrait.
    z, zw = text_path("Zom's", "OpenSans-ExtraBold.ttf", 78, 0, 0)
    m, mw = text_path("Modzilla", "OpenSans-ExtraBold.ttf", 78, 0, 0)
    left = 370
    save("wide", 920, 430, icon(70, 85, 260)
         + f'<path transform="translate({left} 200)" d="{z}" fill="{ink}"/>'
         + f'<path transform="translate({left} 292)" d="{m}" fill="{blue}"/>')
    # Hero (1920 x 620): a quiet banner; Steam puts the logo over it.
    save("hero", 1920, 620, f'<g opacity="0.18">{icon(1380, 40, 540)}</g>')
    # Logo (1000 x 260, no background): the name, for over the hero.
    _, nw = words(100, 0, 0)
    size = 100 * 920 / nw  # as big as fits, with a margin
    n, nw = words(size, 0, 0)
    save("logo", 1000, 260, f'<g transform="translate({(1000 - nw) / 2} {130 + size * 0.36})">{n}</g>', bg=False)
    png(f"{OUT}/modzilla-icon.svg", "src-tauri/steam-art/icon.png", 256)


steam_art()
