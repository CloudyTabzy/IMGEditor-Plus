# IMGEditor-RS Performance Optimization Pass

Date: 2026-09-13

## Executive Summary

A full memory and throughput audit of the Rust port found the architecture
already near-optimal for a desktop archive editor (mmap-first I/O, byte-budgeted
LRU caches, single-flight decoding, disciplined `spawn_blocking` usage), but
identified a set of localized hotspots. All of them were fixed with scalar Rust
— no new dependencies, no `unsafe`, no inline assembly — and every change is
verified by tests. The headline results:

- **Archive open**: the IMG directory was parsed three times per open (format
  pre-check + detection + the real parse). It is now parsed exactly once, and
  IMG v2 directory records are read through a buffered reader instead of one
  syscall per 32-byte record.
- **Task-boundary clones**: full entry-list clones (~18 MB per operation for a
  100k-entry archive, taken on the UI thread) were removed from export, texture
  preview, the validator probe, and bulk conversion; the buffered export path
  now streams instead of materializing a per-entry buffer of up to 128 MiB.
- **Texture decode**: the DXT decoders got 3.8x-6.8x faster through two passes
  of data-flow restructuring (see the benchmark section below).

Everything below was driven by measurement; the numbers were captured with a
one-off benchmark harness that has since been removed (its essentials are
documented here for reproduction).

---

## 1. Archive open: three directory parses -> one

`detect_version` validated the whole directory through `is_valid`, then the
parser's `open()` parsed it again, and the app's open task ran a detection
pre-check on top — three full directory parses per open of an IMG v2 archive,
with IMG v2 records read 32 bytes per syscall off an unbuffered `File`.

`parser::open_in_detect_order` now dispatches on cheap format probes (the
`VER2` magic for v2, sibling `.dir` presence for v1/Xbox 360) and parses the
winning directory exactly once through a 1 MiB `BufReader`. `ArchiveInfo::open`,
`Editor::open_archive`, `compat::scan::scan_archive`, and the app open task all
share it.

One deliberate behavior change: a file that is *recognized* but structurally
invalid (a `VER2` header with a truncated directory, say) now surfaces the
parser's specific error ("entry 5 ends beyond IMG size") instead of silently
degrading to an unknown-format archive. Unrecognized files still open as
unknown and are reported as unsupported.

## 2. Task-boundary clones

Blocking tasks run on `spawn_blocking` and need owned data, but several paths
were cloning far more than the task actually reads:

- **Export** borrowed entry handles (`Vec<&EntryInfo>`) instead of cloning the
  entry list, and `export_entries_batched` walks `par_chunks` over the slice
  instead of cloning per-worker chunks. The buffered export engine streams
  through `io::copy` into a buffered writer (bounded by a 4 MiB reader) instead
  of materializing a full per-entry buffer — up to 128 MiB for a maximum-size
  IMG v2 entry — and verifies the full sector range was written. A worker whose
  shared reader fails to open degrades to per-entry opens instead of panicking.
- **Texture preview** cloned the entire entry list on every preview click,
  although only NFT catalogs need random access to sibling entries. TXD
  previews (the common case) no longer clone it at all; NFT indexes consume an
  owned snapshot via `ArchiveTextureIndex::from_entries_owned` instead of
  re-cloning every entry into a map.
- **Validator probe** (`compat::hint`) is split in two: a metadata-only name
  scan (`scan_probe_sample`, safe on the UI thread, cloning at most
  `PROBE_SAMPLE_LIMIT` = 48 entries) and a blocking content phase
  (`probe_from_scan`) that reads the sampled TXD headers. `probe_target`
  remains as a one-call wrapper for tests and headless use.
- **Bulk conversion** plans and executes from a bounded snapshot of the
  selected entries (each `BulkEntryPlan` carries its own `EntryInfo`) plus the
  archive path and mmap handle, so neither phase clones the archive. The plan
  also carries the archive generation and source identity; planning,
  confirmation, and completion all discard stale results if entries changed
  or the tab at that index was replaced while either background task ran.

## 3. NIF pixel payload copy

`read_ni_pixel_data` copied up to 64 MiB of pixel data one byte per
`read_u8` call. It now takes the remaining block bytes in a single
`copy_from_slice` via `Reader::remaining_slice`.

## 4. Texture decode: DXT 3.8x-6.8x

The hand-rolled DXT decoders in `parser/texture_decoder.rs` were the heaviest
per-pixel code in the app. Two passes of restructuring, both scalar:

### Pass 1 — premultiplied restore and block writes

- DXT2/DXT4 un-premultiplication divided three times per pixel (a u16 `idiv`
  is roughly 40% of those paths). Restore is now an exact compile-time 64 KiB
  lookup table (`min(255, (channel * 255) / alpha)` for every channel/alpha
  pair, indexed `(channel << 8) | alpha`), applied once per block.
- The 4x4 write-out copies whole 16-byte pixel rows for fully-contained blocks
  instead of scattering bounds-checked single pixels.

### Pass 2 — per-block palettes

`dxt_color_block` re-ran its branchy blend match for every texel, and
`dxt5_block` re-ran the alpha interpolation formula per texel. Both outputs
depend only on per-block constants, so the 4-entry color palette and 8-entry
alpha palette are derived once per block and each texel becomes an
unconditional table lookup. This removed the dominant cost — branch
mispredictions over data-random selectors, not the arithmetic.

### Measured results

1024x1024 (1 Mpixel) surfaces, release profile, minimum of 40 iterations,
deterministic xorshift block data exercising every selector:

| Decoder | Before | After | Speedup |
|---|---:|---:|---:|
| dxt1 | 5.62 ms | 1.48 ms | 3.8x |
| dxt2 (premultiplied) | 11.40 ms | 2.39 ms | 4.8x |
| dxt3 | 6.93 ms | 1.65 ms | 4.2x |
| dxt4 (premultiplied) | 18.67 ms | 3.11 ms | 6.0x |
| dxt5 | 13.75 ms | 2.03 ms | 6.8x |
| dxt5 @2048x2048 | 55.77 ms | 8.34 ms | 6.7x |
| 1555 / 565 / 4444 / 8888 / pal8 | ~0.8-1.5 ms | unchanged | already ~1 Gpx/s |

The 16-bit expanders and palette decoders were already near memory-bandwidth
and were left untouched. Practical meaning: a first-view decode of a 1-megapixel
DXT5 texture went from ~14 ms to ~2 ms of CPU work on top of I/O.

### Exactness

Every rewritten path produces bit-identical output to the formulas it
replaced, pinned by tests in `texture_decoder.rs`:

- `unpremultiply_matches_integer_division` sweeps all 65,535 channel/alpha
  pairs against the original integer division.
- `dxt5_alpha_palette_matches_the_reference_formula` decodes 512 random blocks
  and compares every texel against the per-texel formula.
- `dxt_color_block_selectors_follow_both_endpoint_orders` pins all four color
  selectors in both endpoint orders and both transparency modes.

### Lessons

1. **Measure first.** The first fast implementation (u32 fixed-point
   reciprocal) silently overflowed for most alpha values; only the exhaustive
   sweep test caught it. The correct u64 multiply was *slower than the
   original division*, which justified the lookup table.
2. **Branch misprediction beats arithmetic.** The divisions looked like the
   hotspot; the data-dependent per-texel match around them cost more.
3. **Exactness can be free.** The 64 KiB const table beat both the division
   and the wide multiply while keeping integer-division semantics exactly.
4. **Scalar Rust sufficed.** The wins came from restructuring data flow, not
   from instruction selection; intrinsics or `asm!` would not have fixed a
   branch-misprediction problem and the remaining headroom is single-digit
   percent.

The benchmark harness itself was a gitignored local example
(`examples/benchmark_*.rs` are deliberately kept out of the repo) and was
removed after these numbers were recorded. Recreating it needs only: the
internal decoders exposed behind `#[cfg(feature = "bench")]`, deterministic
xorshift block data, and min/median timing over ~40 release-profile
iterations.

---

## Deliberately not pursued

- **Skin-weight row flattening** (`Vec<Vec<f32>>` in `inspector/nif.rs`) — the
  skinning code is still work-in-progress; revisit once it settles.
- **Save/pack full-archive clone into its task** — required by the iced
  ownership model, where the task hands back the updated `ArchiveInfo` that
  replaces the editor's state. Restructuring the state model is not justified
  by a one-time ~2-5 ms clone per save.
- **SIMD intrinsics** — after the palette rework, DXT decode sits within
  ~1.5-3x of the memory-bound 16-bit expanders; the remaining headroom does
  not justify platform-gated code paths.

## Related commits

- `ed08542` Copy NIF pixel data with slice copies
- `417caae` Parse archive directories once on open
- `f5b4ab3` Cut archive clones from export, preview, and convert tasks
- `b02bc5b` Restore premultiplied DXT color without per-pixel division
- `98667e6` Derive DXT block palettes once instead of per texel
- `c7e5a8a` Remove the texture decode benchmark harness
