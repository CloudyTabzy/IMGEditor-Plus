# HashMap crate: `hashbrown` 0.17.1 with foldhash

Decision record for the internal hash-collection migration (performance patch,
2026-10-19). Companion to the crate-level A/B in the sibling benchmark project
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
- `hashbrown` was already in the dependency tree at exactly 0.17.1 via
  `quick_cache` (scene/texture/inspection caches) and `indexmap` (through
  iced). Promoting it to a direct dependency added no compile units.
- It adds the APIs this code actually uses: `entry_ref` (borrowed-key entry)
  and `HashSet::get`.

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
- Verification: 750 lib tests pass in release. One fixture-gated test is
  skipped locally — `object_1004_curve_roots_bind_across_matching_rigs_when_available`
  requires `IMGEDITOR_BULLY_STREAM`, whose configured path
  (`C:\Games\Bully - Scholarship Edition\Stream`) no longer exists on this
  machine; its `world_entry` helper returns `None` at `std::fs::read(World.dir)`
  before any collection code runs, so all four fixture cases are skipped and
  its final coverage assert fires.
