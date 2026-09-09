# GTA RenderWare DFF/TXD preview

Status: implemented for the embedded viewer, with PC RenderWare as the
compatibility target

Checked: 2026-09-09

This note records the DFF and TXD research behind the GTA model and texture
preview work. It is deliberately separate from the IMG archive notes: an IMG
archive only provides the byte ranges, while DFF and TXD define the model,
material, and raster payloads inside those ranges.

## Scope and corpus caveat

The embedded viewer now recognizes these model entries:

- `.dff` RenderWare Clumps used by the PC editions of GTA III, Vice City, and
  San Andreas.
- `.nif` Gamebryo models used by Bully, through the existing pipeline.

It also recognizes `.txd` RenderWare Texture Dictionaries and `.nft` Bully
texture catalogs. DFF/TXD support is an inspection feature: it reads and
renders the common PC formats, but it is not yet a complete RenderWare editor
or serializer.

The local validation corpus supplied as
`C:\Dev\IMGEditor-master\Gta_3_img` is useful, but its identity should not be
overstated. The image is an IMG v2 archive (`VER2`) and its extracted files
include San Andreas-style names such as `gta_proc_*` and `cuntw_*`. The current
export contained 16,316 entries: 12,964 DFF files, 2,759 TXD files, and no NFT
files. It therefore validates shared PC RenderWare coverage and archive
integration; it does not prove compatibility with a retail GTA III or Vice
City archive. A legally obtained, known-game fixture and a small manifest
should be added before claiming game-specific parity.

## Format findings

### DFF / RenderWare Clump

The parser follows the common section stream used by PC RenderWare models:

1. The top-level section is a `CLUMP` (`0x10`).
2. `FRAME_LIST` (`0x0E`) supplies local right/up/at basis vectors, position,
   parent index, and creation flags. Parent transforms are composed before
   geometry reaches the viewer.
3. `GEOMETRY_LIST` (`0x1A`) contains `GEOMETRY` (`0x0F`) sections.
4. A geometry structure starts with flags, triangle count, vertex count, and
   morph-target count. The flags determine whether prelit colors, UV sets,
   positions, and normals are present.
5. Each triangle record stores the RenderWare order `b, a, material, c`; the
   parser converts it to the viewer's normal `a, b, c` index order while
   preserving winding.
6. The first morph target supplies the position, normal, and first UV set used
   for the preview. Additional UV sets and additional morph targets are
   consumed safely but are not animated.
7. `MATERIAL_LIST`/`MATERIAL`/`TEXTURE` sections provide material indices and
   diffuse texture names. Geometry is split by material so each resulting
   viewer mesh can resolve the correct TXD texture.
8. `ATOMIC` (`0x14`) instances connect geometry to frames. Both the compact
   older PC layout, which conventionally uses geometry 0, and the layout with
   an explicit geometry index are accepted.

The parser intentionally rejects the RenderWare native-geometry flag. Native
PS2/Xbox/other platform streams do not have the same vertex layout as the PC
interleaved stream, so treating them as ordinary positions would produce a
misleading model. Unreferenced geometry is retained as a diagnostic fallback
when an exporter omits or corrupts atomics.

The source frame transforms are applied first. The embedded scene builder then
uses `BaseOrientation::Zup` for DFF and maps the result into the viewer's
Y-up camera convention. This keeps source placement and rotations intact
while making the model upright and consistent with the existing NIF viewer.

### TXD / Texture Dictionary

The parser accepts a top-level `TEXTURE_DICTIONARY` (`0x16`), reads its texture
count, and walks `TEXTURE_NATIVE` (`0x15`) children. The PC native structure
contains:

- platform ID, filter/addressing fields, and fixed 32-byte diffuse/alpha names;
- raster format flags and the D3D format/FourCC;
- width, height, depth, mip level count, raster type, and compression property;
- an optional palette; and
- a length-prefixed sequence of mip levels.

Both the D3D8 platform (`8`, common in older GTA PC assets) and D3D9 platform
(`9`) are recognized. D3D8 compression is selected from the platform property;
D3D9 compression is selected from the DXT FourCC. Older streams that only carry
the legacy compressed raster marker still use the raster-format fallback.

The base mip is decoded into the viewer's RGBA8 layout. The decoder covers:

- DXT1/BC1, DXT2/BC2, DXT3/BC2, DXT4/BC3, and DXT5/BC3;
- 1555, 565, 4444, 8888, 888, and 555 packed/unpacked PC rasters;
- LUM8 and A8L8; and
- PAL4 and PAL8 indexed rasters.

Two byte-order details are important. PC uncompressed 24/32-bit rasters use
BGR/BGRA source bytes and are converted to RGBA. RenderWare palette entries
are already RGBA, and PAL4 stores the first pixel in the high nibble. DXT2 and
DXT4 color channels are un-premultiplied when the alpha value permits it so
the scene receives straight-alpha pixels.

## IMGEditor Plus integration

The archive-backed path is intentionally asynchronous:

1. The UI identifies a selected DFF or DFF/TXD entry and switches to the
   requested inspector tab.
2. `spawn_blocking` reads the bounded archive entry and parses it away from the
   Iced event loop.
3. For DFF, material diffuse names are collected and resolved against TXDs in
   this order: IDE-derived dictionary, same-basename TXD, then a bounded scan
   of archive TXDs. The scan is capped at 256 MiB of declared archive sectors.
4. The scene builder creates GPU-ready meshes and shares decoded diffuse
   textures with the texture tab and UV overlay.
5. For a directly selected TXD, every decodable native texture is cached and
   the texture tab exposes its name, format, alpha state, dimensions, mip count,
   and preview.

The cache key includes archive identity, archive generation, and entry index.
Mutations such as imports, deletes, renames, and cross-archive moves therefore
cannot reuse a stale model or texture preview. Archive-only operation remains
useful when no IDE files are available, while an IDE map provides a faster and
more accurate dictionary association when present.

## Safety and limits

The parser is bounded before allocating or decoding untrusted data:

- DFF frame, geometry, atomic, morph-target, vertex, and triangle counts have
  explicit viewer limits.
- TXD dimensions must be non-zero and no larger than 8192×8192.
- Decoder pixel counts and output RGBA sizes use checked arithmetic and reject
  unreasonable dimensions before allocation.
- Native mip data is length-prefixed and read through bounded cursors.
- Unsupported native/console streams fail clearly instead of being guessed as
  PC vertex data.
- GPU admission checks remain separate from file parsing and can reject a
  scene whose mesh or texture footprint exceeds the selected device limits.

These limits are preview limits, not claims about the maximum values accepted
by the games. They protect the desktop viewer from malformed files and
accidental multi-hundred-megapixel allocations.

## Validation performed

Focused tests cover:

- empty and non-RenderWare inputs;
- DFF section parsing, actual triangle ordering, material texture names,
  compact atomics, frame transforms, and real local DFF samples;
- TXD headers, PC native texture parsing, DXT decoding, and a real local TXD;
- PAL4 high-nibble/RGBA behavior, bounded PAL8 palettes, legacy D3D8
  compression fallback, and oversized dimensions; and
- scene construction with a DFF mesh and resolved diffuse texture.

The corpus tests intentionally skip when the external local fixture is absent,
so CI does not depend on a user's game installation. When a real archive is
available, run the parser tests from `IMGEditor-rs` and record only an
untracked manifest of filenames, formats, dimensions, and hashes.

At this change point, the full Rust suite reports 305 passing unit tests and one
intentionally ignored doctest.

## Deliberately deferred work

The current preview does not yet promise:

- PS2/Xbox/GameCube/PSP native geometry or native texture decoding;
- skinned DFF bones, HAnim, IFP animation, or morph animation playback;
- MatFX, environment/bump/specular effects, material colors, dual textures,
  or every RenderWare extension section;
- complete Bin Mesh/native geometry variants and all unusual palette/mipmap
  layouts; or
- writing DFF/TXD files back to disk.

Those are tracked in [`TODO.md`](../TODO.md). They should be implemented only
with representative fixtures and per-format tests; falling back to an
approximate PC decode would be worse than reporting an unsupported stream.

## References

The implementation was compared against these public format readers and
references:

- [DragonFF DFF reader](https://github.com/Parik27/DragonFF/blob/master/gtaLib/dff.py)
- [DragonFF TXD reader](https://github.com/Parik27/DragonFF/blob/master/gtaLib/txd.py)
- [librw RenderWare implementation](https://github.com/aap/librw)
- [Noesis RenderWare model reader](https://github.com/leeao/Noesis-Plugins/blob/master/Model/fmt_RenderWare_PS2_PC.py)
- [RenderWare type definitions](https://github.com/DK22Pac/v2saconv/blob/master/ytdydryddyft2txddffcol/dffapi/RwTypes.h)
- [GTAMods `RpGeometry` reference](https://gtamods.com/wiki/RpGeometry)
- [GTAMods RenderWare binary stream reference](https://gtamods.com/wiki/RenderWare_binary_stream_file)
- [GTAMods RenderWare rendering overview](https://gtamods.com/wiki/Rendering_with_RenderWare)
- [MultimediaWiki TXD overview](https://wiki.multimedia.cx/index.php/TXD)
