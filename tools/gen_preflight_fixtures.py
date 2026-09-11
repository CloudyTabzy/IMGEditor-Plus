"""Generate tiny RenderWare TXD fixtures for testing the import validator.

The files exercise every verdict class of the compatibility engine:
native, supported/mismatched, unknown, incompatible, and unparseable.
Pixel data is noise - the validator judges format headers, not pixels.

Expected outcome when imported into an archive whose target is set
(the import check dialog opens for incompatible **or unknown** rasters):

    01_native_pal8_iii.txd     PAL8, platform 8   native  (III/VC)
    02_native_8888_iii.txd     8888, platform 8   native  (all RW)
    03_mismatch_dxt1_sa.txd    DXT1, platform 9   convertible (III/VC), native (SA)
    04_unknown_888_24bit.txd   888 depth 24       unknown  -> opens the dialog
    05_unknown_a8l8.txd        A8L8 format word   unknown  -> opens the dialog
    06_invalid_garbage.txd     noise              skipped  (no dialog, no false flag)
    07_native_565_vc.txd       565, platform 8    native (III/VC), supported (SA)
    08_mismatch_dxt3_sa.txd    DXT3, platform 9   convertible (III/VC), native (SA)

Any RW TXD is incompatible for a Bully target (World.img), so every file
above opens the dialog there - that is the incompatible case.

Usage:
    python tools/gen_preflight_fixtures.py [output_dir]

Default output: ../preflight-fixtures next to the repository root.
"""

import os
import random
import struct
import sys

RW_STRUCT = 1
RW_TEXTURE_NATIVE = 0x15
RW_TEXTURE_DICTIONARY = 0x16
RW_VERSION = 0x1803FFFF

PLATFORM_D3D8 = 8
PLATFORM_D3D9 = 9

# Raster format nibbles (bits 8-11) and palette bits (13-14).
NIBBLE_1555 = 0x0100
NIBBLE_565 = 0x0200
NIBBLE_4444 = 0x0300
NIBBLE_LUM8 = 0x0400
NIBBLE_8888 = 0x0500
NIBBLE_888 = 0x0600
PAL8 = 0x2000

# D3DFORMAT values.
D3DFMT_R8G8B8 = 20
D3DFMT_A8L8 = 51

FOURCC_DXT1 = 0x31545844  # "DXT1"
FOURCC_DXT3 = 0x33545844  # "DXT3"


def u16(value):
    return struct.pack("<H", value)


def u32(value):
    return struct.pack("<I", value)


def section(kind, body):
    return u32(kind) + u32(len(body)) + u32(RW_VERSION) + body


def native_struct(
    platform,
    raster_format,
    d3d_format,
    width,
    height,
    depth,
    raster_type,
    platform_properties,
    data,
    palette=b"",
    name="fixture",
):
    body = u32(platform)
    body += bytes([6, 17, 0, 0])  # filter mode, UV addressing, padding
    body += name.encode("ascii")[:31].ljust(32, b"\0")
    body += bytes(32)  # alpha/mask name
    body += u32(raster_format)
    body += u32(d3d_format)
    body += u16(width) + u16(height)
    body += bytes([depth, 1, raster_type, platform_properties])
    body += palette
    body += u32(len(data)) + data
    return section(RW_STRUCT, body)


def texture_native(native):
    return section(RW_TEXTURE_NATIVE, native)


def name24(name):
    return name.encode("ascii")[:23].ljust(24, b"\0")


def pack_v1(archive_dir, base, entries):
    """Write a classic IMG v1 pair (`.dir` + `.img`)."""
    dir_bytes = bytearray()
    img = bytearray()
    sector = 1  # sector 0 left blank, matching common archives
    for name, data in entries:
        sectors = max(1, (len(data) + 2047) // 2048)
        dir_bytes += u32(sector) + u32(sectors) + name24(name)
        img.extend(bytes(sector * 2048 - len(img)))
        img.extend(data)
        img.extend(bytes(sectors * 2048 - len(data)))
        sector += sectors
    os.makedirs(archive_dir, exist_ok=True)
    with open(os.path.join(archive_dir, f"{base}.dir"), "wb") as handle:
        handle.write(dir_bytes)
    with open(os.path.join(archive_dir, f"{base}.img"), "wb") as handle:
        handle.write(img)
    return len(entries), len(img)


def pack_v2(archive_dir, base, entries):
    """Write an IMG v2 archive (`VER2`, embedded directory)."""
    dir_size = 8 + len(entries) * 32
    data_start = ((dir_size + 2047) // 2048) * 2048
    sector = data_start // 2048
    body = bytearray()
    index = bytearray()
    for name, data in entries:
        sectors = max(1, (len(data) + 2047) // 2048)
        index += u32(sector) + u32(sectors) + name24(name)
        body.extend(data)
        body.extend(bytes(sectors * 2048 - len(data)))
        sector += sectors
    blob = bytearray(b"VER2")
    blob += u32(len(entries))
    blob += index
    blob += bytes(data_start - len(blob))
    blob += body
    os.makedirs(archive_dir, exist_ok=True)
    path = os.path.join(archive_dir, f"{base}.img")
    with open(path, "wb") as handle:
        handle.write(blob)
    return len(entries), len(blob)


def txd(*natives):
    dict_body = section(RW_STRUCT, u16(len(natives)) + u16(2))
    for native in natives:
        dict_body += section(RW_TEXTURE_NATIVE, native)
    return section(RW_TEXTURE_DICTIONARY, dict_body)


def noise(count, seed):
    rng = random.Random(seed)
    return bytes(rng.randrange(256) for _ in range(count))


def rgba_palette(seed):
    rng = random.Random(seed)
    return bytes(rng.randrange(256) for _ in range(1024))


def main():
    default_dir = os.path.join(os.path.dirname(__file__), "..", "..", "preflight-fixtures")
    out_dir = os.path.abspath(sys.argv[1] if len(sys.argv) > 1 else default_dir)
    os.makedirs(out_dir, exist_ok=True)

    size = 8
    files = {}

    # 1. Native for III/VC: PAL8, platform 8, 1 KB palette + byte indices.
    files["01_native_pal8_iii.txd"] = txd(
        native_struct(
            PLATFORM_D3D8, PAL8, 0, size, size, 8, 4, 0,
            noise(size * size, 1), palette=rgba_palette(2), name="native_pal8",
        )
    )

    # 2. Native for every RW game: uncompressed 8888, platform 8.
    files["02_native_8888_iii.txd"] = txd(
        native_struct(
            PLATFORM_D3D8, NIBBLE_8888, 0, size, size, 32, 4, 0,
            noise(size * size * 4, 3), name="native_8888",
        )
    )

    # 3. Mismatched for III/VC, native for SA: DXT1 on platform 9.
    files["03_mismatch_dxt1_sa.txd"] = txd(
        native_struct(
            PLATFORM_D3D9, NIBBLE_565, FOURCC_DXT1, size, size, 16, 4, 0,
            noise((size // 4) * (size // 4) * 8, 4), name="mismatch_dxt1",
        )
    )

    # 4. Unknown everywhere: 888 stored true 24-bit (depth 24).
    files["04_unknown_888_24bit.txd"] = txd(
        native_struct(
            PLATFORM_D3D8, NIBBLE_888, 0, size, size, 24, 4, 0,
            noise(size * size * 3, 5), name="unknown_888_24",
        )
    )

    # 5. Incompatible for every RW game: A8L8 format word -> opens the
    #    Import check dialog on a target that is set.
    files["05_unknown_a8l8.txd"] = txd(
        native_struct(
            PLATFORM_D3D9, NIBBLE_LUM8, D3DFMT_A8L8, size, size, 16, 4, 0,
            noise(size * size * 2, 6), name="unknown_a8l8",
        )
    )

    # 6. Unparseable: noise behind a .txd name -> reported as skipped.
    files["06_invalid_garbage.txd"] = noise(512, 7)

    # 7. Native VC form (and driver-mapped for III): 565, platform 8.
    files["07_native_565_vc.txd"] = txd(
        native_struct(
            PLATFORM_D3D8, NIBBLE_565, 0, size, size, 16, 4, 0,
            noise(size * size * 2, 8), name="native_565",
        )
    )

    # 8. Mismatched for III, native for SA: DXT3 on platform 9.
    files["08_mismatch_dxt3_sa.txd"] = txd(
        native_struct(
            PLATFORM_D3D9, NIBBLE_4444, FOURCC_DXT3, size, size, 16, 4, 0,
            noise((size // 4) * (size // 4) * 16, 9), name="mismatch_dxt3",
        )
    )

    for name, data in files.items():
        path = os.path.join(out_dir, name)
        with open(path, "wb") as handle:
            handle.write(data)
        print(f"{name:34} {len(data):6} bytes")

    print(f"\nWrote {len(files)} fixtures to {out_dir}")

    # ---- Ready-to-open IMG archives for the pre-save report ----------
    # Each archive is built from the same TXD builders, so opening one in
    # the app and pressing Ctrl+S exercises a specific save path.
    archive_root = os.path.join(os.path.dirname(out_dir), "save-test-fixtures")
    f = files

    # A DXT3 fourcc on an 8888/32-bit header: the gating ERROR anomaly.
    broken_dxt = txd(
        native_struct(
            PLATFORM_D3D9, NIBBLE_8888, FOURCC_DXT3, size, size, 32, 4, 0,
            noise(64, 42), name="broken_dxt",
        )
    )
    # A8L8 on platform 8: unknown for III without a platform rewrite, so
    # the dialog shows the unknown category on its own.
    a8l8_p8 = txd(
        native_struct(
            PLATFORM_D3D8, NIBBLE_LUM8, D3DFMT_A8L8, size, size, 16, 4, 0,
            noise(size * size * 2, 43), name="unknown_a8l8",
        )
    )

    archives = [
        # Clean for a GTA III target: saves silently.
        ("clean_iii", pack_v1, "gta3", [
            ("pal8_native.txd", f["01_native_pal8_iii.txd"]),
            ("8888_native.txd", f["02_native_8888_iii.txd"]),
            ("565_native.txd", f["07_native_565_vc.txd"]),
        ]),
        # Unknown formats + a broken header: opens the Save check dialog.
        ("issues_iii", pack_v1, "gta3", [
            ("unknown_888_24.txd", f["04_unknown_888_24bit.txd"]),
            ("unknown_a8l8.txd", a8l8_p8),
            ("broken_dxt_header.txd", broken_dxt),
        ]),
        # Native content in the wrong container for III: container note.
        ("container_mismatch", pack_v2, "gta3", [
            ("pal8_native.txd", f["01_native_pal8_iii.txd"]),
            ("8888_native.txd", f["02_native_8888_iii.txd"]),
        ]),
        # Native content for SA in the SA container: saves silently.
        ("clean_sa", pack_v2, "gta3", [
            ("dxt1_sa.txd", f["03_mismatch_dxt1_sa.txd"]),
            ("dxt3_sa.txd", f["08_mismatch_dxt3_sa.txd"]),
        ]),
        # SA-dialect content in a v1 container for a III target:
        # convertible only, so it still saves silently.
        ("cross_target", pack_v1, "gta3", [
            ("dxt1_sa.txd", f["03_mismatch_dxt1_sa.txd"]),
        ]),
    ]
    print(f"\nSave-test archives in {archive_root}:")
    for name, packer, base, entries in archives:
        target_dir = os.path.join(archive_root, name)
        count, size = packer(target_dir, base, entries)
        print(f"  {name:20} {count} entries, {size} bytes  ->  {target_dir}")


if __name__ == "__main__":
    main()
