# Memory-mapped archive I/O: `memmap2` 0.9.12

Decision and audit record for the `memmap2 0.9.10` → `0.9.12` upgrade and
zero-copy mmap adoption pass (2026-11-27). Companion to
[allocator-rusty-alloc.md](allocator-rusty-alloc.md),
[hashmap-crate.md](hashmap-crate.md), and
[rayon-parallelism.md](rayon-parallelism.md).

## What changed in `memmap2` 0.9.11–0.9.12

- **`0.9.12` (2026-10-21):**
  - Added `MmapOptions::no_probe_handle()` on Windows: skips the up-to-two
    extra `CreateFileMappingW` + `CloseHandle` capability probes (`PAGE_READWRITE`,
    `PAGE_EXECUTE_READ`) that `Mmap::map` performs by default, mapping straight
    to `PAGE_READONLY` / `FILE_MAP_READ`. This also guarantees a read-only
    archive map can never create a writable section or be widened later.
  - Cached Windows allocation granularity (`GetSystemInfo`) in an `AtomicUsize`
    so repeated maps avoid redundant syscalls.
  - Added `MmapOptions::map_if_safe()` for safe read-only mappings when the
    kernel can guarantee immutability. Verified in `memmap2-0.9.12/src/windows.rs`
    that Windows returns `ErrorKind::Unsupported` unconditionally (the API
    currently covers sealed Linux descriptors such as `memfd`), so Windows
    archive maps remain behind our single documented `unsafe` helper.
- **`0.9.11` (2026-06-22):**
  - Fixed unchecked pointer arithmetic in `advise_range`,
    `unchecked_advise_range`, `flush_range`, and `flush_async_range`.
- **`Advice` / `advise()`:** Unix-only (`madvise`); not exposed on Windows.

## Architecture and safety contract

All four previously scattered `unsafe { Mmap::map(...) }` calls (`pc_v1.rs` ×2,
`pc_v2.rs` ×2) are consolidated into one crate-internal boundary:
`parser::map_archive_file(file: &File) -> io::Result<Arc<Mmap>>` using
`MmapOptions::new().no_probe_handle().map(file)`.

Windows file-locking invariant:

- On Windows, an active `MapViewOfFile` blocks `std::fs::rename` over the same
  path (`ERROR_SHARING_VIOLATION` / `ERROR_ACCESS_DENIED`).
- `ArchiveInfo` holds the sole long-lived `Arc<Mmap>` and drops it
  (`archive.source_mmap = None`) immediately before replacing an archive in
  place during save (`pc_v1.rs`, `pc_v2.rs`).
- Background load tasks (`spawn_blocking`) and `ArchiveTextureIndex` hold
  short-lived `Arc<Mmap>` clones only for the duration of a parse/decode.
  `Arc<Mmap>` is **never** stored in persistent UI dialogs (`ReplaceState`) or
  caches.

## Codebase changes

- **`src/parser/mod.rs`:**
  - Added `map_archive_file` (`MmapOptions::no_probe_handle()` + single
    documented `// SAFETY:` block).
  - Simplified `read_entry_data_from_source` to delegate to
    `read_entry_data_with_source(..., None)`.
  - Switched `export_entry_to_file` to `read_entry_bytes` (zero-copy mmap slice
    straight into `std::fs::write`).
- **`src/parser/pc_v2.rs` & `src/parser/pc_v1.rs`:**
  - Switched all open/reopen sites to `map_archive_file`.
  - `PcV2Parser::open` now maps the file first and validates directory records
    directly off `&source_mmap[..]` via `read_validated_entries_from_slice`,
    eliminating the 1 MiB `BufReader` heap allocation and `read_exact` copies
    on every IMG v2 open.
  - `PcV1Parser::open_with_endian` pre-reserves `archive.entries` to the exact
    record count.
- **`src/inspector/texture.rs`:**
  - Added `ArchiveTextureIndex::with_source_mmap(Option<Arc<Mmap>>)` and
    zero-copy `read_bytes` (borrowing `&mmap[range]` directly for `parse_txd`
    and `parse_nft_catalog_bytes` without per-entry `File::open` syscalls).
  - Added a `parse_txd_headers` pre-check in `resolve_txd_entry` so fallback
    scans over hundreds of TXDs skip full pixel copies on non-matching entries.
- **`src/ui/app.rs` (resolving Findings #8 and #9 in `memory-pre-profiling-audit.md`):**
  - Routed all remaining mmap-bypassing load paths through
    `read_entry_bytes_with_source` + `archive.source_mmap`: GTA IFP/DFF
    playback, Bully AGR/NIF playback, 3D scene load (DFF/NIF/COL), texture
    preview tab load (TXD/NFT), `plan_replace`, `ReplaceConfirmed`, and
    external 3D viewer launch.
  - Switched both `ArchiveTextureIndex::from_entries(&archive_entries, ...)`
    call sites (3D scene load and AGR playback) to `from_entries_owned` +
    `.with_source_mmap(...)`, eliminating a 3 MB duplicate `Vec<EntryInfo>`
    clone on large archives.

## Measured impact (`examples/benchmark_memmap.rs`, release, `gta3.img`)

Archive: `corpus\Gta_3_img\gta3.img` (897 MB, 16,316 entries). Windows, Rust
1.99.1, `rusty_alloc 2.6.0`, `memmap2 0.9.12`, 3 warm runs:

| workload | before / File::open arm (med) | after / mmap arm (med of 3 runs) | speedup |
|---|---|---|---|
| `read_entry_data_from_source` vs `read_entry_data` (400 DFF/TXDs) | 10.692 ms | **1.702 ms** | **6.3× (−84 %)** |
| `ArchiveTextureIndex::resolve_textures_for_dff` (15 DFFs, no IDE, pre-check off) | 1,249.156 ms | 296.313 ms | 4.2× (−76 %) |
| `ArchiveTextureIndex::resolve_textures_for_dff` (15 DFFs, no IDE, `source_mmap` + header pre-check) | 1,249.156 ms | **65.739 ms** (min 46.99 ms) | **19.0× (−95 %)** |
| `Mmap::map` vs `MmapOptions::no_probe_handle().map` (200 maps) | 3.590 ms (min 3.341) | 4.264 ms (min 3.352) | within noise; skips 2 Windows kernel probes + enforces `PAGE_READONLY` |
| `ArchiveInfo::open` (16,316 entries, warm) | ~8–9 ms | 9.020 ms (min 8.608) | eliminates 1 MiB `BufReader` alloc |

Raw per-run medians (ms) with `source_mmap` + header pre-check enabled:

| arm | run 1 | run 2 | run 3 | median |
|---|---|---|---|---|
| `Mmap::map` (200 maps) | 5.207 | 3.341 | 3.590 | 3.590 |
| `MmapOptions::no_probe_handle().map` (200 maps) | 4.264 | 5.600 | 3.352 | 4.264 |
| `ArchiveInfo::open` (warm) | 8.608 | 9.020 | 11.563 | 9.020 |
| `read_entry_data_from_source` (400 DFF/TXD) | 10.292 | 15.560 | 10.692 | 10.692 |
| `read_entry_data` mmap-backed (400 DFF/TXD) | 1.778 | 1.522 | 1.702 | 1.702 |
| `ArchiveTextureIndex::resolve` (`File::open`, with header pre-check) | 1040.150 | 1016.224 | 1073.992 | 1040.150 |
| `ArchiveTextureIndex::resolve` (`source_mmap`, with header pre-check) | 65.739 | 81.879 | 48.030 | 65.739 |

Note: `read_entry_data` in the 400-entry arm still copies into a `Vec<u8>`
(`Cow::into_owned`), whereas the UI load paths (`read_entry_bytes_with_source`)
pass the borrowed `&[u8]` mmap slice directly into `parse_dff`, `parse_txd`,
`parse_col`, `NifFile::parse`, `parse_ifp`, and `parse_agr` with zero copy.
