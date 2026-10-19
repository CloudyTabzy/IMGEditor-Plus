# Rayon use, and the dependency optional-feature audit

Record for the threading/feature pass that followed the hash-collection
migration (performance patch, 2026-10-19). The question it answers: which
optional dependency features (Rayon above all) are worth enabling, and which
of the app's own loops are worth fanning out.

## Existing parallel surfaces (unchanged)

| surface | mechanism |
|---|---|
| Archive export (`tasks.rs`) | `par_iter` over entries, chunked, workers clamped to 8 |
| Palette LUT build (`compat/palette.rs`) | `par_iter` |
| DXT encode (`texpresso`, `rayon` feature) | internal `par_chunks_mut` over block rows — on |
| Palette quantization (`quantette`, `threads` feature) | internal parallelism — on |

## New: TXD preview catalog decode fans out

Selecting a TXD decodes every raster for the texture-preview tab
(`app.rs`, inside `spawn_blocking`). The decode is the app's own format
dispatcher — **not** texpresso — so each raster decoded serially, and retail
catalogs commonly carry 18–44 rasters.

Harness: `examples/benchmark_rayon_decode.rs` (local-only). Top-3 TXDs by
size plus the smallest, gta3.img, 16 rayon threads, release. Raw output:
`fixtures\rayon-bench\decode.txt`. Pixel checksums are asserted equal
between the sequential and parallel arms.

| catalog | rasters | seq (med) | par (med) | speedup |
|---|---|---|---|---|
| `vgwsthiway1.txd` | 18 | 4.02 ms | 1.07 ms | 3.8× |
| `des_ne.txd` | 44 | 3.98 ms | 1.17 ms | 3.4× |
| `des_nw.txd` | 36 | 3.79 ms | 1.14 ms | 3.3× |
| `billbrdlod_lan.txd` | 4 | 0.002 ms | 0.015 ms | 0.16× (overhead only) |

The last row is why the shipped policy is a **threshold**: fan out only at
`TXD_PARALLEL_DECODE_MIN = 8` rasters. The hybrid arm measures 0.002 ms on
the small catalog (no fork/join tax) and matches the parallel arm elsewhere.
Effect: a large TXD's preview catalog decodes in ~1 ms instead of ~4 ms.

## Dependency optional features evaluated — no change

| crate | feature | verdict |
|---|---|---|
| `image` | `rayon` | **No.** In 0.25.10 the feature only reaches `images/buffer_par.rs` (parallel buffer helpers) and external `ravif`/`exr` codecs. The PNG/DDS/BMP/TGA decoders we enable contain no rayon code, so the feature touches no path we run. |
| `quick_cache` | `sharded-lock` | **No.** An alternative to the crate's default `parking_lot`, mutually exclusive with it; nothing here shows lock contention (cache hits are single-digit µs at desktop rates). |
| `quick_cache` | `custom-hasher` | Already on by default (foldhash) — consistent with `docs/hashmap-crate.md`. |
| `hashbrown` | `rayon`, `serde` | **No.** No `par_iter` use, no map serialization; our maps are ≤16 k entries, and the sibling benchmark project measured parallel map *construction* losing (0.71–0.82×). |
| `fuzzt` | extra distance algorithms | **No.** `search.rs` uses `jaro_winkler` only; the crate is already pinned to `default-features = false, features = ["jaro"]`. |
| `serde_json` | `preserve_order` | **No.** Settings are INI; JSON is not round-tripped through ordered maps. |
| `glam` | `core-simd`, `fast-math` | **No.** `core-simd` is nightly-only; `fast-math` would break the deterministic headless pixel-diff tests. |
| `rusty_alloc` | `secure`, `profile`, `debug_checks` | **No.** Hardening/profiling hooks, not throughput; the allocator record (`docs/allocator-rusty-alloc.md`) covers the settings we ship. |
| `tokio` | `fs` | **No.** File IO already lives in `spawn_blocking`; async file IO would add a scheduler layer without removing the blocking work. |
| `wgpu` | `counters` | **No.** GPU-side profiling aid, not a user-facing speed feature; could be a local dev aid later. |
| `ureq` | `charset`, `cookies`, `native-tls`, … | **No.** The updater uses plain TLS + gzip only. |

## App-side rayon candidates measured and deferred

- **Compat scan per-entry profiling** (`compat/scan.rs`, 277 ms warm for
  16,316 entries): parallelizable, but the report is accumulated into
  `BTreeMap`s and counters, so it needs a per-thread report + merge
  refactor. The scan runs off the UI thread; deferred until it shows up in
  a profile.
- **NFT catalog decode** (`decode_nft_textures_with_resolver`): takes an
  `FnMut` resolver, so fanning out needs an `Fn + Sync` signature change;
  payload counts per catalog are small. Deferred.
- **Search scoring** (16 k fuzzy scores per keystroke, inside the measured
  1.1–1.8 ms rebuild): the rebuild is sort-dominated; fork/join overhead
  would eat most of the gain at this size. Rejected.
- **Converter driver** (serial by design — see `docs/memory-pre-profiling-audit.md`):
  the biggest untapped CPU fan-out, but the audit flagged per-texture peak
  memory (~19 MB at 1024²: rgba + visible + Oklab), so parallel workers
  multiply peak. Needs a worker-budget design first; deferred.

## Notes

- Machine: 16 logical CPUs, shared/unpinned; treat <5 % as noise, as in the
  other performance records.
- Nested-rayon interaction is not a factor for the new fan-out: the preview
  path uses the app's serial DXT decoder, not texpresso; texpresso's rayon
  path is only used by encode/convert.
- Verification: 750 lib tests pass in release (one stale-fixture test
  filtered — see `docs/hashmap-crate.md`), `cargo fmt --check` clean,
  `cargo clippy --release --all-targets` clean.
