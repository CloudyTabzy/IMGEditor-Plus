# Rayon use (`rayon` 1.12.0), and the dependency optional-feature audit

Record for the threading/feature pass that followed the hash-collection
migration (2026-10-19) and the `rayon 1.12.0` (`rayon-core 1.13.0`)
architecture pass (2026-11-27). The question it answers: which optional
dependency features (Rayon above all) are worth enabling, and which of the
app's own workloads benefit from work-stealing parallelism without exploding
peak memory or breaking deterministic output.

## Active parallel surfaces

| surface | mechanism & policy |
|---|---|
| Archive export (`tasks.rs`) | `par_chunks(64)` + `flat_map_iter` zero-copy mmap export (batched atomic progress + early break on cancel); chunked `par_chunks` + `flat_map_iter` buffered fallback (`workers` clamped to 8) |
| Compat archive scan (`compat/scan.rs`) | `par_iter().enumerate().try_fold().try_reduce()` at `>= 32` entries; zero-copy mmap slices (`read_entry_bytes`), header-only TXD parsing (`parse_txd_headers`), lock-free `classify_verdict`; left-to-right `try_reduce` keeps `entry_verdicts` and `anomaly_examples` bit-for-bit identical to serial while short-circuiting on cancel |
| Pre-save header normalize (`compat/normalize.rs`) | `par_iter().enumerate().filter_map()` at `>= 32` entries; zero-copy mmap inspection, cloning bytes only for entries that need patching |
| Bulk texture conversion (`compat/convert.rs`, `ui/app.rs`) | On-demand bounded `ThreadPool` (`BULK_CONVERT_MAX_WORKERS = 4`, torn down on return) via `run_in_bounded_convert_pool`, fanning out across selected TXDs and textures; nested `texpresso` / `quantette` / `palette` calls inherit this same pool via `ThreadPool::install`, capping peak scratch memory while keeping multi-core throughput; `convert_bulk_entries` splices in parallel at `>= 4` entries |
| Palette LUT & pixel mapping (`compat/palette.rs`) | Cached constant 32,768-cell Oklab table (`LUT_OKLAB_CELLS`), direct `par_iter_mut` fill into boxed LUT, row-wise `par_chunks_exact_mut` mapping at `>= 65,536` pixels (256×256+) |
| TXD preview catalog decode (`ui/app.rs`) | Short-circuiting `par_iter().map().collect::<Result<_, _>>()` at `>= 8` rasters or `>= 2` rasters with `>= 262,144` total pixels |
| NFT preview catalog decode (`inspector/texture.rs`) | Sequential `FnMut` external resolution followed by `into_par_iter` payload decode at `>= 4` payloads or `>= 2` payloads totalling `>= 128 KiB` |
| Batch import prep & pre-flight checks (`parser/mod.rs`, `compat/scan.rs`, `ui/app.rs`) | `prepare_import_batch` fans out `fs::metadata` at `>= 32` files; `check_import_files` and `refresh_imported_verdicts` fan out at `>= 8` files, preserving exact input order |
| DXT encode (`texpresso`, `rayon` feature) | Internal `par_chunks_mut` over block rows — inherits calling rayon pool |
| Palette quantization (`quantette`, `threads` feature) | Internal parallelism — inherits calling rayon pool |

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

## 1.12.0 architecture pass (2026-11-27)

Manifest floor bumped from `rayon = "1.10"` → `"1.12"` (locking `rayon 1.12.0`
and `rayon-core 1.13.0`). All three previously deferred high-value surfaces
(`compat/scan.rs`, `decode_nft_textures_with_resolver`, and the bulk converter
driver) plus `compat/normalize.rs`, `compat/palette.rs` row mapping, batch
import prep/preflight, and `tasks.rs` export chunking were redesigned around
three architectural rules:

1. **Eliminate copy/lock contention before fanning out.** `read_entry_bytes`
   borrows slices straight from `source_mmap` (`Cow::Borrowed`), `parse_txd_headers`
   validates every palette/mipmap slice without copying 350+ MB of pixels, and
   `classify_verdict` / `classify_nft_verdict` avoid 160k+ Fluent `RwLock` +
   `String` allocations per archive scan.
2. **Preserve exact archive order deterministically.** Rayon's
   `IndexedParallelIterator::try_fold` + `try_reduce` combines `left` before
   `right`, keeping `entry_verdicts` order and the 5-example cap in
   `anomaly_examples` bit-for-bit identical to the serial loop (verified by
   unit tests on synthetic archives and retail `gta3.img`).
3. **Bound memory-heavy nested parallelism with an on-demand `ThreadPool`.**
   `texpresso 2.0.2`, `quantette 0.6.0`, and `palette.rs` all schedule onto
   Rayon's current pool. Running `plan_bulk_convert` inside a 4-worker
   `ThreadPool::install` caps concurrent in-flight textures (and therefore peak
   scratch memory) while letting nested DXT/palette work share those same 4
   workers with zero deadlocks, zero oversubscription, and zero lingering idle
   threads once conversion finishes.

### Measured impact (`examples/benchmark_rayon_pass.rs`, release, 16 threads)

Archive: `corpus\Gta_3_img\gta3.img` (16,316 entries, 2,740 TXDs, 26,004
textures) and `corpus\Gta_3_img\Export\`. Windows, Rust 1.99.1, `rusty_alloc`
2.6.0. "After" is the median across 3 warm runs (each run's own median over
its rounds).

| workload | before (med) | after (med of 3 runs) | speedup |
|---|---|---|---|
| `scan_archive (gta3.img, sa)` (incl. open) | 263.750 ms | **25.715 ms** | **10.3× (−90 %)** |
| `normalize::plan_fixes` (229 fixes) | 32.339 ms | **0.250 ms** | **129× (−99 %)** |
| `normalize::collect_fixes` (229 patches) | 40.348 ms | **3.550 ms** | **11.4× (−91 %)** |
| `prepare_import_batch` (500 files) | 5.607 ms | **1.277 ms** | **4.4× (−77 %)** |
| `check_import_files` (64 TXDs) | 2.843 ms | **0.767 ms** | **3.7× (−73 %)** |
| `bulk convert` (337 tex → Rgb888, 4 workers) | 18.244 ms | **5.440 ms** | **3.4× (−70 %)** |
| `palette::map_level` (1024² dither, LUT + map) | 4.092 ms | **1.307 ms** | **3.1× (−68 %)** |
| `bulk convert` (337 tex → DXT1, 4 workers) | 89.879 ms | **37.505 ms** | **2.4× (−58 %)** |

Raw medians across the 3 warm after runs (ms):

| arm | run 1 | run 2 | run 3 | median |
|---|---|---|---|---|
| `scan_archive (gta3.img, sa)` | 25.867 | 25.640 | 25.715 | 25.715 |
| `normalize::plan_fixes` | 0.167 | 0.267 | 0.250 | 0.250 |
| `normalize::collect_fixes` | 3.648 | 3.527 | 3.550 | 3.550 |
| `palette::map_level` (1024² dither) | 1.411 | 1.264 | 1.307 | 1.307 |
| `bulk convert` bounded pool (DXT1) | 38.976 | 36.495 | 37.505 | 37.505 |
| `bulk convert` bounded pool (Rgb888) | 5.264 | 5.440 | 5.746 | 5.440 |
| `prepare_import_batch` (500 files) | 1.277 | 1.237 | 1.296 | 1.277 |
| `check_import_files` (64 TXDs) | 0.769 | 0.680 | 0.767 | 0.767 |

## Rejected app-side candidates

- **Search scoring** (16 k fuzzy scores per keystroke, inside the measured
  1.1–1.8 ms rebuild): the rebuild is sort-dominated; fork/join overhead
  would eat most of the gain at this size. Kept serial.

## Notes

- Machine: 16 logical CPUs, shared/unpinned; treat <5 % as noise, as in the
  other performance records.
- Verification: `cargo fmt --check`, `cargo clippy --all-targets -j 2 -- -D warnings`,
  and `cargo test -j 2` all pass with zero warnings (the Bully Stream fixture
  test noted in earlier records was fixed in `af0a3ee` to skip cleanly via
  `test_paths::bully_stream()`).
