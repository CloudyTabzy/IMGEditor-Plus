# HashMap crate: `hashbrown` 0.17.2 with foldhash

Decision record for the internal hash-collection migration (performance patch,
2026-10-19; upgraded from 0.17.1 to 0.17.2 with deeper API adoption on
2026-11-27). Companion to the crate-level A/B in the sibling benchmark project
(`C:\Dev\Hashmap_Bench`, Suites A–M) and to `docs/allocator-rusty-alloc.md`,
which set the A/B discipline used here.

## Decision

Every internal `HashMap`/`HashSet` is `hashbrown`'s SwissTable with the crate
default foldhash, re-exported through `src/collections.rs`. Nothing else was
adopted: no concurrent map (`papaya`/`kovan`/`starshard`), no ordered map
(`indexmap`/`ordermap`/`ringmap`), no research construction (`opthash`).

Why:

- Since Rust 1.36 std's `HashMap` **is** hashbrown (vendored SwissTable). The
  hashbrown-vs-std gap is entirely the hasher: SipHash-1-3 vs foldhash —
  ~1.8–2.5× on the key shapes this app uses (usize indices, `(u32, u32)` edges,
  8–60 char entry names).
- std keeps SipHash for HashDoS resistance. This is a local desktop editor:
  keys are game-asset names, entry indices, and pixels, and foldhash's
  `RandomState` is randomly seeded per process, so the security property buys
  nothing here.
- `hashbrown` was already in the dependency tree via `quick_cache`
  (scene/texture/inspection caches) and `indexmap` (through iced), now unified
  on 0.17.2. Promoting it to a direct dependency added no compile units.
- It adds the APIs this code actually uses: `entry_ref` / `VacantEntryRef::insert_with_key`
  (borrowed-key entry), `Equivalent` (zero-allocation case-insensitive lookup
  via `LowerStr`), and `HashSet::get` / `HashSet::take`.

Kept deliberately:

- `BTreeMap` for report/config output and counters — deterministic ordering is
  a feature there, not an accident.
- `std::collections::hash_map::DefaultHasher` where a stable hasher is wanted
  for non-collection work.
- The test-local median-cut histogram in `compat/palette.rs` (an independent
  reference implementation inside `#[cfg(test)]`).

## What changed

- `src/collections.rs` — re-exports (`HashMap`, `HashSet`, `Entry`); the
  single place the crate choice is made.
- All `std::collections::{HashMap, HashSet}` uses in `src/` switched to it.
- `ArchiveInfo::selected_lookup` — `HashMap<usize, usize>` → dense `Vec<u32>`
  with a `NOT_VISIBLE` sentinel. Entry indices are dense, so the per-keystroke
  rebuild no longer hashes or grows a table.
- `ui/view.rs` — the per-frame compatibility verdict table (row highlighting)
  → `Vec<Option<Verdict>>` indexed by entry index, built by a fill loop.
- `tasks.rs` export name counting and `ui/app.rs` NIF texture resolution —
  `entry_ref` instead of `entry(key.clone())`.

## Method

Harness: `examples/benchmark_hashmaps.rs` (local-only; `examples/benchmark_*.rs`
is gitignored). The harness code was identical for both arms; the "before" arm
was produced by reverting **only `src/`** (`git stash push -- src`) in the same
tree, so both arms share the same lockfile, dep versions, and (warm) page
cache. Archive: `corpus\Gta_3_img\gta3.img` (16,316 entries); the real compat
scan produced 2,740 per-entry verdicts for the highlight-table arm. Release
profile, `cargo -j 2`, Windows, Rust 1.96.0.

Before = 2 runs (`baseline.txt`, `before2-warm.txt`), after = 4 runs
(`after*.txt`), raw output in `fixtures\hashmap-bench\`. Values below are the
median of the per-run medians; each run's own min/median is over its n rounds.

## Results — application paths

| workload (16,316 entries) | before | after | change |
|---|---|---|---|
| visible-list rebuild, no filter | 1.81 ms | 1.71 ms | −6 % |
| visible-list rebuild, filter "a" (10,511 rows) | 1.16 ms | 1.10 ms | −5 % |
| visible-list rebuild, filter "pol" (343 rows) | 0.244 ms | 0.25 ms | noise |
| visible-list rebuild, sort size desc | 1.92 ms | 1.77 ms | −8 % |
| visible-list rebuild, sort type asc | 2.78 ms | 2.83 ms | noise band |
| visible-list rebuild, sort offset desc | 0.38 ms | 0.19 ms | **−50 %** |
| `display_row_of` lookup | 10.3 ns | 2.2 ns | **−79 %** |
| manifest compare, case-sensitive | 2.61 ms | 2.31 ms | −12 % |
| manifest compare, case-insensitive | 3.17 ms | 2.56 ms | −19 % |
| wireframe edge dedup (60 k triangles) | 2.67 ms | 0.87 ms | **−67 %** |

The offset-sort arm isolates the `selected_lookup` win (its sort is nearly
free): the map fill cost ~0.19 ms per rebuild in SipHash hashing plus table
growth; the dense write is ~0.02 ms. The other rebuild arms are dominated by
fuzzy scoring, string comparisons, and the sort itself, so they move less.

## Results — isolated micro arms (identical in both arms)

| 2,740 verdict pairs, per rebuild | time |
|---|---|
| `std::collections::HashMap<usize, Verdict>` collect | 0.023 ms |
| `hashbrown::HashMap<usize, Verdict>` collect | 0.010 ms |
| `Vec<Option<Verdict>>` dense fill | 0.001 ms |

| per-lookup | ns/op |
|---|---|
| std map `get` | 11.4 |
| hashbrown `get` | 6.1 |
| dense `Vec` index | 3.6 |

| 16,316 `CompactString` keys, count map | time |
|---|---|
| std `entry(k.clone())` | 0.35 ms |
| hashbrown `entry(k.clone())` | 0.36 ms |
| hashbrown `entry_ref(&key)` | 0.19 ms |

The owned-clone `entry` variant shows **no** win over std — with all-unique
keys the clone dominates and hashing is a fraction of the cost. The win comes
from `entry_ref`'s borrowed probe; the app uses it where the key is already in
hand (export path, texture resolution).

## Notes and caveats

- Shared, unpinned machine: treat <5 % as noise, as in the sibling benchmark
  record. The type-sort and small-filter arms sit in that band.
- The compat scan itself is unchanged by this work: 277 ms warm before and
  after. An early cold-cache run (898 ms) was page-cache, not code.
- The frame-budget effect of the verdict table is small in absolute terms
  (23 µs → ~1 µs on gta3.img, where 2,740 entries carry verdicts) but it is
  per redraw, and the dense table also removes a per-frame allocation.
- Verification (initial 0.17.1 pass): 750 lib tests passed in release; the
  Bully Stream fixture test (`object_1004_curve_roots_bind_across_matching_rigs_when_available`)
  that failed locally when `C:\Games\Bully - Scholarship Edition\Stream` was
  absent was later fixed in `af0a3ee` to use `test_paths::bully_stream()` and
  skip cleanly.

## 0.17.2 upgrade and API adoption (2026-11-27)

Upgraded `hashbrown` from `0.17.1` → `0.17.2` (released 2026-11-22: fixes an
allocator leak on dropping empty tables #751, switches `ProbeSeq::move_next`
to wrapping arithmetic #736, relaxes default hasher/allocator bounds #732, and
uses `std::hash::RandomState` #747). Default features (`default-hasher` via
`foldhash 0.2.0`, `inline-more`, `allocator-api2`, `equivalent`, `raw-entry`)
already cover everything needed; `serde`, `rayon`, and `nightly` remain off.

Audited every `HashMap`/`HashSet` site in `src/` and adopted `hashbrown`'s
borrowed-lookup and entry APIs where they eliminate real allocations or passes:

- `src/collections.rs` — re-exports `EntryRef` and `Equivalent`, and adds
  `LowerStr<'a>` implementing `Hash` + `Equivalent<String | CompactString | str | &str>`
  matching `str::to_lowercase()` without heap allocation on ASCII inputs.
- `src/compare.rs` — collapsed `compare_names` from five `HashSet<String>`
  tables (~80 k `String` allocations on `gta3.img`) into two borrowed-key
  tables (`HashMap<K, bool>` + `HashSet<K>`), using `&str` for case-sensitive
  and `CaseFolded<'_>` for case-insensitive matching, skipping the archive-only
  pass entirely when `archive_only` is empty.
- `src/tasks.rs` — `scan_import_folder` borrows `&entry.file_name_lower`
  (`HashSet<&str>`) and probes with `LowerStr`; `FolderImportTask::run_blocking`
  pre-sizes `HashSet<CompactString>` from `entry.file_name_lower` instead of
  re-lowercasing every entry into a heap `String`.
- `src/inspector/texture.rs` — `IdeMap` (`entry_ref(&LowerStr(_))` +
  `VacantEntryRef::insert_with_key` + allocation-free `txd_name_for`),
  `ArchiveTextureIndex` (inline `CompactString` keys, allocation-free `read`,
  `file_name_lower.ends_with(".txd")` filter, and `wanted.take(&LowerStr(_))`
  in `resolve_txd_entry` to move matched keys with zero clone), and
  allocation-free `NftCatalog` lookups.
- `src/compat/scan.rs` & `src/parser/texture_decoder.rs` — replaced the
  uncapped `unique_color_count` `HashSet` (Finding #11 in
  `docs/memory-pre-profiling-audit.md`) with `texture_decoder::palette_colors`,
  and pre-sized `unique_color_count_capped`'s `HashSet`.
- `src/inspector/animation/binding.rs`, `src/ui/texture_preview.rs`, and
  `src/ui/app.rs` — switched `gta_ifp_name_calibration` and
  `decoded_textures_from_scene` to `LowerStr` / `entry_ref` / `with_capacity`,
  and removed a leftover `.clone()` before `entry_ref` in NIF texture
  resolution (`app.rs`).

### Measured impact (release, `gta3.img`, 3 warm runs)

Same harness (`examples/benchmark_hashmaps.rs`) and archive (`gta3.img`,
16,316 entries), Windows, Rust 1.99.1, `rusty_alloc` 2.6.0:

| arm (16,316 entries) | std baseline | 0.17.1 | 0.17.2 + API pass | vs 0.17.1 |
|---|---|---|---|---|
| `compare_names`: case-sensitive | 2.61 ms | 2.31 ms | **0.416 ms** | **−82 % (5.6×)** |
| `compare_names`: case-insensitive | 3.17 ms | 2.56 ms | **0.853 ms** | **−67 % (3.0×)** |
| `display_row_of` lookup | 10.3 ns | 2.2 ns | 2.15 ns | parity |
| `verdict map: hashbrown collect` | 0.023 ms | 0.010 ms | 0.011 ms | parity |
| `name_counts: entry_ref(&key)` | 0.35 ms | 0.19 ms | 0.219 ms | parity |
| `build_wire_indices` (60 k tris) | 2.67 ms | 0.87 ms | 0.957 ms | parity |

Raw medians across the 3 warm runs (ms unless noted):

| arm | run 1 | run 2 | run 3 | median |
|---|---|---|---|---|
| `compare_names`: case-sensitive | 0.416 | 0.406 | 0.419 | 0.416 |
| `compare_names`: case-insensitive | 0.853 | 0.843 | 0.888 | 0.853 |
| `display_row_of` (ns/lookup) | 2.17 | 2.14 | 2.15 | 2.15 |
| `rebuild`: no filter, name asc | 1.776 | 1.853 | 1.866 | 1.853 |
| `rebuild`: sort offset desc | 0.227 | 0.187 | 0.218 | 0.218 |
| `name_counts: entry_ref(&key)` | 0.219 | 0.207 | 0.349 | 0.219 |
| `build_wire_indices` | 0.940 | 0.957 | 1.623 | 0.957 |

## Appendix: raw runs

Commands (from `IMGEditor-rs`):

```
cargo build -j 2 --release --example benchmark_hashmaps
target\release\examples\benchmark_hashmaps.exe ^
    C:\Dev\IMGEditor-master\corpus\Gta_3_img\gta3.img sa --real-verdicts
```

Raw outputs live outside the repo:
`C:\Dev\IMGEditor-master\fixtures\hashmap-bench\{baseline,before2-warm,after,after2,after3,after4}.txt`.
`baseline` is the very first (cold page cache) run; `before2-warm` is the warm
before arm rebuilt from the same tree with only `src/` reverted.

### List rebuilds — per-run medians (ms)

| arm | baseline | before2-warm | after | after2 | after3 | after4 |
|---|---|---|---|---|---|---|
| no filter, name asc | 1.805 | 1.808 | 1.637 | 1.772 | 1.711 | 1.713 |
| filter "a" | 1.188 | 1.141 | 1.083 | 1.080 | 1.102 | 1.121 |
| filter "pol" | 0.245 | 0.243 | 0.243 | 0.253 | 0.247 | 0.260 |
| sort size desc | 1.918 | 1.927 | 1.713 | 1.720 | 1.804 | 1.845 |
| sort type asc | 2.690 | 2.873 | 2.822 | 2.929 | 2.836 | 2.762 |
| sort offset desc | 0.396 | 0.367 | 0.190 | 0.217 | 0.189 | 0.188 |

### Other arms — per-run medians

| arm | baseline | before2-warm | after | after2 | after3 | after4 |
|---|---|---|---|---|---|---|
| `display_row_of` (ns) | 10.28 | 10.25 | 2.15 | 2.14 | 2.22 | 2.23 |
| compare, case-sensitive (ms) | 2.665 | 2.564 | 2.060 | 2.364 | 2.250 | 2.389 |
| compare, case-insensitive (ms) | 3.237 | 3.097 | 2.477 | 2.729 | 2.539 | 2.571 |
| wire dedup (ms) | 2.681 | 2.651 | 0.845 | 1.155 | 0.889 | 0.858 |
| open #2 / #3 (ms) | 8.28 / 8.21 | 8.13 / 7.76 | 7.77 / 7.02 | 7.87 / 7.26 | 8.08 / 7.26 | 7.88 / 7.72 |
| compat scan (ms) | 898.2 * | 277.5 | 277.2 | 279.4 | 289.8 | 296.0 |

\* first read of the 897 MB archive, page-cache cold; the five warm runs
agree within noise, i.e. the scan is unchanged by the migration.

### Isolated micro arms (identical code in both arms; ranges over six runs)

| arm | range |
|---|---|
| verdict table, std `HashMap` collect (2,740 pairs) | 0.022–0.023 ms |
| verdict table, hashbrown collect | 0.008–0.010 ms |
| verdict table, dense `Vec` build | 0.001 ms |
| verdict `get`: std / hashbrown / `Vec` | 11.30–11.86 / 5.87–6.88 / 3.59–3.75 ns |
| name counts, std `entry(k.clone())` | 0.337–0.355 ms |
| name counts, hashbrown `entry(k.clone())` | 0.348–0.373 ms |
| name counts, hashbrown `entry_ref(&key)` | 0.179–0.192 ms |

The lookup-phase checksums (`635668774` for `display_row_of`, `840089` for
the verdict gets) are identical in every run, confirming the arms do the same
work.


