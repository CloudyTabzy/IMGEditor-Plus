# Allocator: rusty_alloc as the process-wide global heap

Adopted 2026-09-30: the entire process allocates through
[`rusty_alloc`](https://crates.io/crates/rusty_alloc) 2.2.1 — a pure-Rust
remake of mimalloc v2.4.5 — via the `RustyAlloc` `GlobalAlloc` veneer in
`rusty_alloc-api`, installed once in `src/lib.rs`:

```rust
#[global_allocator]
static ALLOC: rusty_alloc_api::RustyAlloc = rusty_alloc_api::RustyAlloc;
```

There is no C mimalloc anywhere in the graph; the core crate is dependency-free
on every target this project ships. Binary cost measured at +50 KB per
executable. This record holds the A/B numbers that justified the swap, the
memory-retention characteristic that comes with it, and every other surface of
the crate that was studied and deliberately not wired in.

## Measured A/B (2026-09-30)

Method: two release builds of the local (gitignored) harnesses
`examples/benchmark_table.rs` and `examples/benchmark_export.rs`, one with the
allocator and one with the Windows CRT heap, run **interleaved** (baseline,
rusty, baseline, …) on `corpus/Gta_3_img/gta3.img` (940 MB, 16,316 entries) to
cancel thermal and page-cache drift. Medians of 5 rounds (table) and 3 rounds
(export); the headline metric's full per-round spread is quoted because a
median alone cannot show overlap.

| metric (benchmark_table) | CRT heap | rusty_alloc | delta |
|---|---:|---:|---:|
| open (parse + initial list) | 11.14 ms | 9.41 ms | **−15.5 %** |
| rebuild visible list (×3) | 2.06–2.19 ms | 1.82–1.96 ms | −10…−15 % |
| filter "w" (4,105 hits) | 0.67 ms | 0.61 ms | −9.0 % |
| sort by name | 2.15 ms | 1.85 ms | −14.0 % |
| selected-count scan ×1000 (alloc-free control) | 17.6 ms | 18.4 ms | noise |

`open` per-round spread: CRT 10.17 / 10.39 / 11.14 / 11.21 / 11.30 ms vs
rusty 9.15 / 9.29 / 9.41 / 9.41 / 9.43 ms — **zero overlap across all five
interleaved rounds**. The alloc-free scan is the control: it touches no
allocator path and its two arms overlap completely (15.2–18.6 ms both), which
is what makes the allocation-bound deltas trustworthy.

| metric (benchmark_export, ZeroCopy) | CRT heap | rusty_alloc |
|---|---:|---:|
| export 940 MB / 16,316 files | 21.98 s | 21.98 s |

Export is disk-bound (Windows small-file creation limiter, see
[export-optimization-lessons.md](export-optimization-lessons.md)); the
allocator cannot move it and did not. The win is exactly where the interactive
UI lives: archive open, entry-table rebuilds, search filtering, sorting — the
paths an older CPU feels most.

Peak working set during export, sampled every 100 ms: 912.5 MB (CRT) vs
915.1 MB (rusty), +0.3 %, dominated by the memory-mapped archive view itself.
Read the next section before treating that as the whole memory story.

## Memory retention: what shipping this actually means

rusty_alloc v1/v2 ship `purge_delay = -1`, i.e. **purging is opt-in**: a live
heap never returns freed pages to the OS, it keeps them for reuse. Process RSS
therefore sits at the high-water mark of each thread heap's bursts until the
memory is reallocated. The upstream reason is a known open defect (their
LEDGER M8): on Windows, `MEM_DECOMMIT`-purged spans whose reuse path does not
re-commit them fault with an access violation — a forced purge of small spans
crashed the crate's own test suite with `0xC0000005` (see the notes in
`heap.rs` `collect_inner` and `segment.rs` `purged_any`). Linux's
`MADV_DONTNEED` tolerates what Windows does not.

Consequences for this app, all accepted deliberately:

- **We do not set `purge_delay` in code.** Enabling a vendor-flagged
  Windows access-violation path in a Windows-first product is not a trade
  this project makes for a trim heuristic.
- **Retention is bounded, not growing.** Every large buffer in the app lives
  under a byte-budgeted `quick_cache` (scenes 256/64 MiB, textures 128/32 MiB,
  inspections 16/4 MiB, desktop/mobile), so retained-freed memory is capped by
  the peak burst above current residency — e.g. closing an archive leaves the
  evicted decode buffers parked in the allocator instead of with the OS.
  Steady-state editing does not accumulate.
- **Thread death does purge.** Abandoned segments are purged unconditionally
  (`segment.rs::purge_free_spans`, deliberately not gated on `purge_delay`),
  so churny thread pools give memory back. Our rayon/tokio pools are
  process-lifetime, so this contributes little here — do not rely on it.
- **Escape hatch without a rebuild:** the crate parses `RUSTY_ALLOC_<OPTION>`
  (and `MIMALLOC_<OPTION>` for compat) from the environment once, on first
  allocation. A user who wants OS trimming can set
  `RUSTY_ALLOC_PURGE_DELAY=<ms>` and accept the M8 caveat; nothing in this
  repo sets or requires it.

## Studied and rejected surfaces

- **First-class `Heap` / destroyable heaps.** Attractive for burst scopes
  (decode scratch, export workers): `Drop` releases everything at once. But
  the crate has no stable `Allocator`-trait impl (upstream plans it after
  `allocator_api` stabilizes), so using a `Heap` means hand-plumbing
  `alloc`/`dealloc` through the decode and export paths — new `unsafe` at
  every hotspot against this repo's unsafe policy, for a gain the global
  allocator's per-thread heaps already approximate.
- **`alloc::collect(true)` at burst boundaries.** Sweeps only the *calling*
  thread's heap: drains cross-thread frees, adopts abandoned segments, retires
  empty pages back into their segments — but with purge off it decommits
  nothing, so RSS does not fall. Our worker threads never die, so orphan
  adoption finds almost nothing. No measurable benefit; not wired.
- **`arena` module.** Supplies memory *to* the allocator (bare-metal regions,
  guarded sampling); it is not a consumer-side feature. Irrelevant on Windows
  desktop.
- **`bins::good_size` pre-sizing.** Rounding capacities to bin sizes to avoid
  internal fragmentation is micro-optimization below this app's measurement
  floor; the A/B shows the allocator's own placement already wins.
- **Setting `purge_delay` at startup.** See the retention section: rejected on
  the M8 Windows access-violation defect.

## Re-running the A/B

The harnesses are local-only (gitignored) `examples/`. To reproduce an arm
pair: build the examples, stash the exes, comment out the two
`#[global_allocator]` lines in `src/lib.rs`, rebuild, and interleave the two
binaries over the same archive (5+ rounds; medians; quote per-round spread for
any headline number). The baseline exes behind this record were stashed under
`target/alloc-ab/baseline/` on the measuring machine — uncommitted scratch,
regenerate rather than trust.
