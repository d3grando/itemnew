"""Draws icon.png, the registry icon: an inventory slot lit up and glowing round its edge, a new item in it and a
sparkle on its corner, in a vanilla tooltip's frame (the frame LookAt's icon uses, so the mods look like one
family). 32 x 32 pixel art scaled 8 times to 256 x 256. Needs Pillow.

    python tools/icon.py [icon.png]
"""
import sys

from PIL import Image

SIZE, SCALE = 32, 8
BACKGROUND = (0x10, 0x00, 0x10, 0xF0)
FRAME_TOP = (0x50, 0x00, 0xFF, 0xFF)
FRAME_BOTTOM = (0x28, 0x00, 0x7F, 0xFF)


def lerp(a, b, t):
    return tuple(round(ca + (cb - ca) * t) for ca, cb in zip(a, b))


class Canvas:
    """A 32 x 32 tooltip; drawing stays inside its frame."""

    def __init__(self):
        self.img = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
        self.px = self.img.load()
        for y in range(SIZE):
            for x in range(SIZE):
                if x in (0, SIZE - 1) and y in (0, SIZE - 1):
                    continue
                edge = x in (0, SIZE - 1) or y in (0, SIZE - 1)
                frame = x in (1, SIZE - 2) or y in (1, SIZE - 2)
                self.px[x, y] = lerp(FRAME_TOP, FRAME_BOTTOM, (y - 1) / (SIZE - 3)) if frame and not edge else BACKGROUND

    def set(self, x, y, c):
        if not (2 <= x < SIZE - 2 and 2 <= y < SIZE - 2) or c is None:
            return
        if len(c) == 4 and c[3] < 255:
            under, a = self.px[x, y], c[3] / 255
            c = tuple(round(u + (v - u) * a) for u, v in zip(under[:3], c[:3])) + (under[3],)
        self.px[x, y] = c if len(c) == 4 else c + (255,)

    def sprite(self, x0, y0, art, palette):
        for dy, row in enumerate(art):
            for dx, ch in enumerate(row):
                if ch not in ". ":
                    self.set(x0 + dx, y0 + dy, palette[ch])

    def save(self, path):
        self.img.resize((SIZE * SCALE, SIZE * SCALE), Image.NEAREST).save(path, optimize=True)


PALETTE = {
    # the slot: its lit fill, the shadow on its top left, the light on its bottom right
    "F": (0xB4, 0xAC, 0x8C), "D": (0x3C, 0x38, 0x30), "B": (0xFF, 0xFC, 0xE8),
    # the item, a cut gem: outline, body, light, shadow, glint
    "K": (0x0C, 0x2C, 0x2C), "C": (0x2E, 0xC8, 0xB0), "c": (0x7C, 0xF0, 0xDC), "d": (0x16, 0x88, 0x7C),
    "W": (0xFF, 0xFF, 0xFF), "Y": (0xFF, 0xE0, 0x60),
}
GLOW = (0xFF, 0xD0, 0x40)

GEM = [
    "...KKKK...",
    "..KcccCK..",
    ".KcWcCCCK.",
    "KcWcCCCCdK",
    "KcCCCCCCdK",
    "KCCCCCCddK",
    ".KCCCCddK.",
    "..KCCddK..",
    "...KddK...",
    "....KK....",
]

SPARKLE = [
    "....W....",
    "....W....",
    "...WYW...",
    "...YWY...",
    "WWYWWWYWW",
    "...YWY...",
    "...WYW...",
    "....W....",
    "....W....",
]

SMALL_SPARKLE = [
    ".Y.",
    "YWY",
    ".Y.",
]


def draw():
    c = Canvas()
    x0, y0, x1, y1 = 7, 9, 24, 26  # the slot, inclusive
    # the glow round it, fading out
    for y in range(3, 29):
        for x in range(3, 29):
            d = max(x0 - x, x - x1, y0 - y, y - y1)
            if 1 <= d <= 3:
                c.set(x, y, GLOW + ((0xD0, 0x80, 0x38)[d - 1],))
    for y in range(y0, y1 + 1):
        for x in range(x0, x1 + 1):
            top_left = x == x0 or y == y0
            bottom_right = x == x1 or y == y1
            c.set(x, y, PALETTE["D" if top_left and not (x == x1 or y == y1) else "B" if bottom_right else "F"])
    c.set(x0, y1, PALETTE["F"])
    c.set(x1, y0, PALETTE["F"])
    c.sprite(11, 13, GEM, PALETTE)
    c.sprite(20, 3, SPARKLE, PALETTE)
    c.sprite(4, 23, SMALL_SPARKLE, PALETTE)
    return c


if __name__ == "__main__":
    draw().save(sys.argv[1] if len(sys.argv) > 1 else "icon.png")
