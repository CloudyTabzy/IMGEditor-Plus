"""Generate example images for manually testing the Phase B converter.

Writes `converter-fixtures/` next to the repository root:
PNG / BMP / TGA via Pillow, plus hand-written DXT1/DXT5 DDS files (a
tiny range-fit block encoder, no external DDS writer needed).

Usage:  python tools/gen_converter_fixtures.py
"""

import os
import random
import struct

from PIL import Image, ImageDraw

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
OUT = os.path.join(ROOT, "converter-fixtures")


def gradient(width: int, height: int) -> Image.Image:
    img = Image.new("RGBA", (width, height))
    px = img.load()
    for y in range(height):
        for x in range(width):
            px[x, y] = (
                (x * 255) // max(width - 1, 1),
                (y * 255) // max(height - 1, 1),
                ((x + y) * 255) // max(width + height - 2, 1),
                255,
            )
    return img


def alpha_rings(width: int, height: int) -> Image.Image:
    img = Image.new("RGBA", (width, height), (20, 20, 30, 255))
    draw = ImageDraw.Draw(img)
    cx, cy = width // 2, height // 2
    for radius, alpha in ((56, 255), (42, 200), (28, 140), (14, 60)):
        draw.ellipse(
            (cx - radius, cy - radius, cx + radius, cy + radius),
            fill=(90, 200, 255, alpha),
        )
    return img


def palette_art(width: int, height: int) -> Image.Image:
    colors = [
        (0, 0, 0, 255),
        (255, 0, 0, 255),
        (0, 255, 0, 255),
        (0, 0, 255, 255),
        (255, 255, 0, 255),
        (255, 0, 255, 255),
    ]
    img = Image.new("RGBA", (width, height))
    px = img.load()
    for y in range(height):
        for x in range(width):
            px[x, y] = colors[((x // 8) + (y // 8)) % len(colors)]
    return img


def noise(width: int, height: int, seed: int = 7) -> Image.Image:
    rng = random.Random(seed)
    img = Image.new("RGB", (width, height))
    px = img.load()
    for y in range(height):
        for x in range(width):
            px[x, y] = (rng.randrange(256), rng.randrange(256), rng.randrange(256))
    return img


def checker(width: int, height: int) -> Image.Image:
    img = Image.new("RGBA", (width, height))
    px = img.load()
    for y in range(height):
        for x in range(width):
            on = ((x // 8) + (y // 8)) % 2 == 0
            px[x, y] = (230, 230, 230, 255) if on else (40, 40, 40, 60)
    return img


# ---- Minimal DXT encoder (range fit) ---------------------------------


def to_565(rgb):
    r, g, b = rgb
    return ((r >> 3) << 11) | ((g >> 2) << 5) | (b >> 3)


def from_565(value):
    r5 = (value >> 11) & 0x1F
    g6 = (value >> 5) & 0x3F
    b5 = value & 0x1F
    return ((r5 << 3) | (r5 >> 2), (g6 << 2) | (g6 >> 4), (b5 << 3) | (b5 >> 2))


def nearest_index(color, palette):
    best, best_d = 0, 1 << 30
    for i, entry in enumerate(palette):
        d = sum((a - b) ** 2 for a, b in zip(color, entry))
        if d < best_d:
            best, best_d = i, d
    return best


def nearest_scalar(value, palette):
    best, best_d = 0, 1 << 30
    for i, entry in enumerate(palette):
        d = (value - entry) ** 2
        if d < best_d:
            best, best_d = i, d
    return best


def color_block(pixels):
    cols = [p[:3] for p in pixels]
    lo = tuple(min(c[i] for c in cols) for i in range(3))
    hi = tuple(max(c[i] for c in cols) for i in range(3))
    c0, c1 = to_565(hi), to_565(lo)
    if c0 < c1:
        c0, c1 = c1, c0
    if c0 == c1:
        c1 = max(c0 - 1, 0)
    p0, p1 = from_565(c0), from_565(c1)
    palette = [
        p0,
        p1,
        tuple((2 * a + b) // 3 for a, b in zip(p0, p1)),
        tuple((a + 2 * b) // 3 for a, b in zip(p0, p1)),
    ]
    codes = 0
    for i, color in enumerate(cols):
        codes |= nearest_index(color, palette) << (2 * i)
    return struct.pack("<HHI", c0, c1, codes)


def alpha_block(pixels):
    alphas = [p[3] for p in pixels]
    lo, hi = min(alphas), max(alphas)
    if lo == hi:
        hi = min(lo + 1, 255)
    a0, a1 = hi, lo
    if a0 == a1:
        a1 = max(a0 - 1, 0)
    if a0 > a1:
        palette = [a0, a1] + [
            ((7 - i) * a0 + i * a1) // 7 for i in range(1, 7)
        ]
    else:
        palette = [a0, a1] + [
            ((5 - i) * a0 + i * a1) // 5 for i in range(1, 5)
        ] + [0, 255]
    codes = 0
    for i, alpha in enumerate(alphas):
        codes |= nearest_scalar(alpha, palette) << (3 * i)
    return bytes([a0, a1]) + codes.to_bytes(6, "little")


def encode_blocks(img, bpp):
    rgba = img.convert("RGBA")
    width, height = rgba.size
    px = rgba.load()
    out = bytearray()
    for by in range((height + 3) // 4):
        for bx in range((width + 3) // 4):
            block = [
                px[min(bx * 4 + x, width - 1), min(by * 4 + y, height - 1)]
                for y in range(4)
                for x in range(4)
            ]
            if bpp == 16:
                out += alpha_block(block)
            out += color_block(block)
    return bytes(out)


def write_dds(path, img, fourcc):
    width, height = img.size
    bpb = 8 if fourcc == b"DXT1" else 16
    data = encode_blocks(img, bpb)
    header = bytearray(128)
    header[0:4] = b"DDS "
    header[4:8] = struct.pack("<I", 124)
    header[8:12] = struct.pack("<I", 0x1 | 0x2 | 0x4 | 0x1000 | 0x80000)
    header[12:16] = struct.pack("<I", height)
    header[16:20] = struct.pack("<I", width)
    header[20:24] = struct.pack("<I", len(data))
    header[28:32] = struct.pack("<I", 1)
    header[76:80] = struct.pack("<I", 32)
    header[80:84] = struct.pack("<I", 0x4)
    header[84:88] = fourcc
    header[108:112] = struct.pack("<I", 0x1000)
    with open(path, "wb") as fh:
        fh.write(header + data)


def main():
    os.makedirs(OUT, exist_ok=True)
    gradient(128, 128).save(os.path.join(OUT, "opaque_gradient_128.png"))
    alpha_rings(128, 128).save(os.path.join(OUT, "alpha_rings_128.png"))
    palette_art(64, 64).save(os.path.join(OUT, "palette_art_64.bmp"))
    noise(128, 128).save(os.path.join(OUT, "noise_128.tga"))
    gradient(96, 64).save(os.path.join(OUT, "odd_size_96x64.png"))
    write_dds(os.path.join(OUT, "dxt1_gradient_128.dds"), gradient(128, 128), b"DXT1")
    write_dds(os.path.join(OUT, "dxt5_checker_128.dds"), checker(128, 128), b"DXT5")

    readme = """Converter test fixtures
========================

opaque_gradient_128.png  smooth true-color gradient, no alpha
                         (III world -> 888 default; PAL8 opt-in quantizes)
alpha_rings_128.png      soft alpha rings
                         (alpha formats: 8888 for III/player.img, DXT3 for VC/SA)
palette_art_64.bmp       6 flat colors
                         (PAL8/PAL4 store this exactly; watch the dialog say so)
noise_128.tga            random RGB - worst case for DXT and palettes
odd_size_96x64.png       non-square, 4-aligned
dxt1_gradient_128.dds    already-DXT1 source (re-encode path)
dxt5_checker_128.dds     DXT5 with soft alpha

Suggested checks
----------------
1. Open an archive, set a target with the shield button, open a TXD in the
   Texture tab, click "Replace texture..." and pick a fixture.
2. Watch the format default and the warnings; try a non-native choice in the
   format picker (preview + warnings update).
3. Replace, then save the archive; reopen it and confirm the texture renders.
4. Toolbar "Import image as TXD" adds a new entry; save and reopen.
5. Toolbar "Convert selection to target dialect" after selecting TXDs.
"""
    with open(os.path.join(OUT, "README.txt"), "w", encoding="utf-8") as fh:
        fh.write(readme)
    print(f"wrote fixtures to {os.path.normpath(OUT)}")


if __name__ == "__main__":
    main()
