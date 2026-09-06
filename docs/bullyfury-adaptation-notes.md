# Bullyfury adaptation notes

This document records useful format behavior found in the local `bullyfury`
0.1.2 reference package so future work does not require re-reading the source.
The Rust implementation remains independent; the package is a format reference,
not a runtime dependency.

Reference snapshot: `C:\Dev\IMGEditor-master\bullyfury-0.1.2`

## Implemented here

### Bully/Gamebryo DXT mapping

`NiPixelData` uses these compressed pixel-format values:

| Value | Format |
| ---: | --- |
| 4 | DXT1 / BC1 |
| 5 | DXT3 / BC2 |
| 6 | DXT5 / BC3 |

The texture inspector previously sent values 5 and 6 through the DXT5 decoder.
The mapping is now centralized in `src/inspector/texture.rs`, and the regression
suite verifies that DXT3's explicit alpha nibble is preserved.

## Current Rust implementation already ahead

- The Rust NIF parser handles Bully 20.3.0.9 geometry, UVs, scene transforms,
  inherited nodes, and big-endian files.
- The embedded viewer resolves NFT textures from loose files and IMG archive
  entries, and can draw UV overlays.
- `NiPixelData` uses its explicit mipmap table before the legacy size guesser.

These paths should remain authoritative instead of being replaced by the
texture-only parser in `bullyfury`.

## Deferred: Bully-specific COL3

Bully's `World.img` collision entries use `COL3` magic but do not use the stock
GTA San Andreas `COL3` layout. The reference parser identifies:

- 96-byte model headers with `object_id`, type words, bounds, and box count;
- AABB box blocks with packed material/flag/brightness/light values;
- compressed mesh vertices stored as signed 16-bit values divided by 128;
- triangle faces with material and light bytes;
- optional tail records and `LIMK` face adjacency;
- raw preservation of unknown tail data;
- Bully surface-material labels for collision inspection and OBJ/MTL export.

IMGEditor currently has a generic GTA collision parser in `src/parser/col.rs`.
Do not silently change its `COL3` behavior. The safe future approach is a
separate Bully parser selected by validated layout/fixture detection, with real
`World.img` entries and round-trip tests before connecting it to the viewer.

Suggested tests:

1. Parse box-only and compressed-mesh Bully entries.
2. Verify object IDs, bounds, material fields, and face indices.
3. Preserve unknown tails and `LIMK` records byte-for-byte.
4. Render collision geometry in a separate debug mode without affecting NIF
   model rendering.

## Deferred: IDB metadata lookup

The reference supports `sdep` and `sjbo` IDB bodies and exposes lookups by
object ID, model name, and texture name. This can improve the current
IDE/basename NFT resolver when an installation has IDB data but incomplete IDE
files.

Recommended Rust shape:

- Parse the 12-byte IDB header and retain declared-size trailing padding.
- Keep unknown body/tail sections as raw bytes.
- Expose indexed `model -> texture` and `object_id -> records` views.
- Use it as an optional resolver layer after explicit NIF texture references,
  before same-basename fallback.
- Add fixture tests before changing resolution precedence or UI labels.

## Deferred: IPB and GRIDS.DAT

`IPB` contains world-instance chunks (`inst`, `prop`, `pont`, `pois`, `spec`,
and `proj`). `GRIDS.DAT` contains pedestrian and vehicle navigation graphs with
positions, links, flags, speed, density, and lane metadata.

These are useful future inspectors or diagnostic exports, but they are not
required for correct NIF/NFT rendering. Keep them separate from the core archive
and 3D viewer paths.

## Reference limitations to avoid copying

- The reference NIF reader is little-endian only; IMGEditor should retain its
  existing endian-aware parser.
- Its NIF geometry types expose metadata but not full vertex/UV payloads.
- Texture transforms are rejected by the reference parser; IMGEditor already
  reads their fields and should eventually apply them rather than rejecting the
  complete file.
- Its DDS linear-size helper uses `width * height` formulas for compressed
  textures. Block dimensions must use `ceil(width / 4) * ceil(height / 4)`;
  retain the Rust implementation's block-based sizing.
- IMG sector archives cannot recover original unpadded file sizes from the
  directory alone. Extraction and preview code must continue treating sector
  padding as an archive-boundary concern.

## Validation reference

The downloaded package contains eight synthetic unit-test modules and no real
game fixtures. Its source-layout test run passed 28 tests, but future Bully COL3,
IDB, and palette work still needs validation against installed game assets.
