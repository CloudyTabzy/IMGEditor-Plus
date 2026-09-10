# RenderWare texture format mismatches — anomaly patterns

Case studies and detection rules for texture rasters whose *declared*
format disagrees with their *stored* layout. Modded archives routinely mix
rasters produced by different tools and platforms, so the decoder must
cross-check every format claim against the fields that cannot lie cheaply:
the D3D format word, the depth byte, and the mip data length.

## Case study: `dwayne.txd` in a modded `gta3.img`

Archive: an IMG **v2** (`VER2`) archive named `gta3.img` (GTA SA-style
container holding GTA III assets, rebuilt by a mod tool — no `.dir` file,
16,316 entries, D3D9 platform rasters throughout).

`dwayne.txd` previewed as a progressive-misalignment checkerboard: the
model geometry was fine, the texture was not. The raster native header:

```
platform          = 9        (D3D9 — original GTA III PC rasters are 8)
filter            = 0x1101
diffuse name      = "dwayne" (32-byte field; mask name field empty)
raster format     = 0x0600   (RW raster format code: 888 = 24-bit RGB)
D3D format word   = 22       (D3DFMT_X8R8G8B8 — 32-bit RGBX, NOT 24-bit)
width / height    = 256 / 256
depth             = 32       (bytes, not bits, per pixel)
mip level 0 size  = 262144   (256*256*4 — internally consistent)
```

RW raster-format code `0x600` ("888") describes the *logical* format:
24 bits of RGB, no alpha. On the D3D9 platform RenderWare stores such
rasters as **`D3DFMT_X8R8G8B8` (32-bit RGBX)** because D3D9 has no
practical 24-bit texture format. The `X` byte is undefined by the format;
real assets fill it with `0xFF`.

The decoder previously mapped D3D format 22 to a 24-bit ("888") decode —
reading 3 bytes/px from a 4-byte/px buffer, which garbles every pixel with
a progressive 1-byte shift. Hand-decoding the same buffer as BGRA32
produced a perfect texture atlas. The file was never corrupt.

**Corpus survey** (first texture of all 2,759 TXDs in that archive):

| raster format | D3D format word | depth | count | app status |
|---|---|---|---|---|
| 0x200 / 0x8200 / 0x100 | `'DXT1'` fourcc | 16 | 1,974 | decoded fine |
| 0x300 | `'DXT3'` fourcc | 16 | 378 | decoded fine |
| **0x600** | **22 (X8R8G8B8)** | **32** | **355** | **was garbled — the dwayne class** |
| 0x500 | 21 (A8R8G8B8) | 32 | 33 | decoded fine |

## The D3D9 format word is the authority (not the raster nibble)

The D3D format word after the raster-format flags is either a DXT FourCC
(`'DXT1'`…) or a raw `D3DFMT_*` code. The full uncompressed table, with the
two historically confused entries first:

| D3DFMT | code | bits | channels | RW raster code |
|---|---|---|---|---|
| `R8G8B8` | **20** | 24 | RGB | 888 (0x600) |
| `A8R8G8B8` | **21** | 32 | BGRA | 8888 (0x500) |
| `X8R8G8B8` | **22** | 32 | BGRX | 888 (0x600) — **the D3D9 storage for 888** |
| `R5G6B5` | 23 | 16 | RGB | 565 (0x200) |
| `A1R5G5B5` | 24 | 16 | BGRA | 1555 (0x100) |
| `A4R4G4B4` | 25/26 | 16 | BGRA | 4444 (0x300) |
| `L8` | 50 | 8 | L | LUM8 (0x400) |
| `A8L8` | 51 | 16 | LA | — |

The trap: `22` looks like "a bigger 24-bit format", and the RW nibble says
888, so "888 = 24-bit" feels confirmed from two directions. The truth runs
the other way: **on D3D9, raster code 888 is stored 32-bit**. Same logic
gives a related hazard in the opposite direction: D3D8 rasters (platform 8)
*do* store 888 as 3 bytes/px, so a tool converting D3D8 → D3D9 without
rewriting the raster nibble produces exactly this file shape.

## Detection rules implemented

`src/parser/texture_decoder.rs`:

1. **D3D9 dispatch trusts the format word.** `(D3D9, 22)` decodes 4 bytes/px
   (`decode_x8r8g8b8`, alpha forced opaque); `(D3D9, 20)` decodes true
   24-bit (`decode_888`). `(D3D9, 21)` is 8888 ARGB.
2. **Depth byte disambiguates the fallback path.** A raster flagged 888
   whose `depth` byte says 32 decodes as X8R8G8B8 even when the D3D format
   word is zero/stale (mixed-import files).
3. **Legacy `0x23` dictionary path has no depth byte**, so `decode_raster`
   falls back to a data-length check: tight 888 rows are exactly 3 B/px;
   ≥ 4 B/px means 32-bit X8R8G8B8-style storage.

Format labels now distinguish "X8R8G8B8 (888 RGB, 32bpp)" from
"R8G8B8 (888 RGB, 24bpp)" so the UI reports what is actually stored.

## Cross-check discipline for future format bugs

When a texture decodes to noise, compare these four independent claims in
order of reliability:

1. **Mip data length** (`len == w * h * bytes_per_px`) — the strongest
   signal; a wrong-bytes/px hypothesis is falsified arithmetically.
2. **Depth byte** (8/16/24/32) — stored per raster.
3. **D3D format word / FourCC** — authoritative on D3D9.
4. **RW raster-format nibble** — describes the logical format, which
   platforms reinterpret for storage (888 → X8R8G8B8 on D3D9).

Two independent fields agreeing is not proof (888 nibble + "22 = bigger
888" assumption both said 24-bit); a field that *contradicts* the others
(depth 32, or the data length) is. For palette rasters, also verify the
palette-size derivation from the extension bits matches the depth
(4-bit vs 8-bit palettes).

## Related observations from the same archive

- `player.txd` (`torso8bit`) carries ~20 bytes of stale, pixel-like junk in
  its 32-byte alpha/mask-name field — harmless for decoding (alpha texture
  is optional), presumably leftover padding from the rebuild tool. Expect
  occasional garbage in nominally-ASCII fields of tool-converted archives.
- The archive being VER2 while holding GTA III assets means `.dir`-based
  (IMG v1) tooling assumptions do not apply; entry names live in the IMG
  header itself.

## Validation

- 4 decoder unit tests cover X8R8G8B8 opaque-alpha, D3D9 format 22 as
  32-bit, format 20 as true 24-bit, and the no-format-word depth fallback.
- End-to-end: hand-decoding `dwayne.txd`'s mip as BGRA32 yields a correct
  texture atlas (blue coveralls character); a reference PNG is preserved
  in the archive's export folder from the investigation session.
