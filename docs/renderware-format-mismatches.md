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

When a texture decodes to noise, compare these five independent claims in
order of reliability:

1. **Mip data length** (`len == w * h * bytes_per_px`) — the strongest
   signal; a wrong-bytes/px hypothesis is falsified arithmetically.
2. **Depth byte** (8/16/24/32) — stored per raster.
3. **D3D format word / FourCC** — authoritative on D3D9.
4. **RW raster-format nibble** — describes the logical format, which
   platforms reinterpret for storage (888 → X8R8G8B8 on D3D9).
5. **Content fingerprints** — unique-color count (≤ 256 ⇒ decoded
   palette), alpha-channel usage, intact mip chains. Header fields can
   all be stale from a tool pass; the pixels are what remains.

Two independent fields agreeing is not proof (888 nibble + "22 = bigger
888" assumption both said 24-bit); a field that *contradicts* the others
(depth 32, or the data length) is. For palette rasters, also verify the
palette-size derivation from the extension bits matches the depth
(4-bit vs 8-bit palettes).

## Corpus provenance forensics — what this archive is made of

The unique-color test turns the modded `gta3.img` survey from a format
table into a provenance reconstruction. A lossless re-encode preserves the
original color count, so **an uncompressed raster with ≤ 256 unique colors
is provably a decoded palette** — no header claim required.

Full-corpus results (first texture per TXD, all 2,759 files classified):

| Class | Count | Content | Provenance evidence |
|---|---|---|---|
| DXT1, no mips | 1,508 | world props (a51_*, ammo*, arch_plx) | SA ships these compressed; sizes 64²–256² |
| DXT1 + mip ext (`0x8200`) | 413 | big world, 512²/256², intact 8–10 level chains | SA's own mip convention — untouched originals |
| DXT3 | 378 | alpha props (kmb_chute, law_coffinfl) | SA alpha-compressed originals |
| "1555" | 53 | small alpha bits | **actually DXT1** — stale 1555 raster nibble, DXT1 FourCC wins |
| 888 → X8R8G8B8 | 355 | interior/building tiles (bistro, hospital2, liberty*), skins (dwayne, player) | 217 files ≤ 256 unique colors → decoded palettes |
| 8888 (A8R8G8B8) | 33 | alpha tiles (bistro_alpha, trees2) + alpha peds (bmycr, bmydrug) | 24 of 26 parsed ≤ 256 colors; 26/26 use real palette alpha |
| no raster (child 0x3) | 19 | SA generic dictionaries (gb_la, gb_sf, gb_vegas) | different structure, unexamined |

Interpretation: the conversion pipeline that produced this archive
(1) copied DXT-compressed content verbatim — SA ships its world compressed,
so there was nothing to do; (2) re-encoded SA's **paletted** rasters
(ped skins, player parts, interior tiles — famously PAL8/PAL4 in SA) into
uncompressed RGB(A), losslessly; and (3) stored author-imported RGBA art
as-is. It split by alpha: paletted-no-alpha → 888 (→ X8R8G8B8 storage,
the dwayne bug), paletted-with-alpha → 8888. **Zero paletted rasters
survive** in the archive; `player.txd`'s texture name `torso8bit` is a
fossil of the original 8-bit form.

Two forensic fingerprints worth remembering:

- **The 257/258/259-color cluster** (84/35/18 files in the 888 class):
  a decoded 256-entry palette plus 1–3 stray pixels — the converter's
  own signature (likely colorkey or padding artifacts written outside
  the palette). A pile of files at exactly N and N+1..N+3 colors is a
  re-encode tell, not organic art.
- **Perceptual non-uniqueness**: 888 and 8888 files pair naturally
  (`bistro` / `bistro_alpha`) — one paletted raster family split by the
  converter's alpha check.

Provenance table for this archive:

| Content | Original form | Converter action | Result |
|---|---|---|---|
| SA world/props (already DXT) | DXT1/DXT3 + mips | verbatim passthrough | decodes fine |
| SA paletted, no alpha (skins, tiles) | PAL8, ~4–256 colors | palette → 888 RGB | X8R8G8B8 32-bit storage |
| SA paletted, with alpha (trees, fences, peds) | PAL8 + RGBA palette | → 8888 | fine |
| Custom imported art (dwayne, etc.) | authored RGBA | stored as 888/8888 | >256 colors, many colors |

Practical consequence: **~60%+ of this archive's uncompressed texture
content is palette-reconstructible** — the original palette can be
recovered from the decoded pixels, which matters for any future
edit/replace feature (see discussion in the repo history; the decoder's
unique-color analysis is the detection primitive).

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
