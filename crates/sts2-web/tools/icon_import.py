#!/usr/bin/env python3
"""Writes the home screen icons from the owner's Steam logo into the shared art folder. Nothing this writes is committed."""
import os
from PIL import Image


def cache_dir(name):
    """`~/.cache/spirezero-<name>`, or the old `~/.cache/spirebot-<name>` while only that exists."""
    new, old = os.path.expanduser(f"~/.cache/spirezero-{name}"), os.path.expanduser(f"~/.cache/spirebot-{name}")
    return old if not os.path.exists(new) and os.path.isdir(old) else new

LOGO = os.path.expanduser("~/.local/share/Steam/appcache/librarycache/2868840/logo.png")
OUT = os.path.join(cache_dir("data"), "art/app")
BG = (14, 16, 21, 255)  # the manifest's background_color

logo = Image.open(LOGO).convert("RGBA")
logo = logo.crop(logo.getbbox())
os.makedirs(OUT, exist_ok=True)
for name, size, fill in (("icon-180", 180, 0.90), ("icon-192", 192, 0.90), ("icon-512", 512, 0.90), ("icon-maskable-512", 512, 0.72)):
    im = Image.new("RGBA", (size, size), BG)
    w = round(size * fill)
    h = round(logo.height * w / logo.width)
    im.alpha_composite(logo.resize((w, h), Image.LANCZOS), ((size - w) // 2, (size - h) // 2))
    im.convert("RGB").save(f"{OUT}/{name}.png", optimize=True)
