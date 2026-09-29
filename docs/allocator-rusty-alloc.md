# Allocator: rusty_alloc as the process-wide global heap

Adopted 2026-09-30: the entire process allocates through
[`rusty_alloc`](https://crates.io/crates/rusty_alloc) 2.2.1 — a pure-Rust
remake of mimalloc v2.4.5 — via the `RustyAlloc` `GlobalAlloc` veneer in
`rusty_alloc-api`, installed once in `src/lib.rs`:

```rust
#[global_allocator]
static ALLOC: rusty_alloc_api::RustyAlloc = rusty_alloc_api::RustyAlloc;
```

`main` calls `imgeditor::init_allocator()` first, which sets the crate's
`purge_delay` option to 0 — see "Memory retention and the purge decision"
below for why that matters and the evidence behind it.

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

## Memory retention and the purge decision (2026-09-30 follow-up)

The crate ships `purge_delay = -1` — purging opt-in — because of an open
defect in its ledger (M8): on Windows, `MEM_DECOMMIT`-purged spans whose
reuse path does not re-commit them fault with an access violation
(`0xC0000005`). The initial adoption accepted the shipped default and
claimed retention was bounded by the byte-budgeted caches. Post-release
profiling disproved that framing.

Method: the local-only harness `examples/benchmark_memory.rs` runs a
scripted session — open `gta3.img`, decode 468 textures (23 MiB RGBA),
build 2000 DFF scenes (194 MiB), drop everything, idle 20 s — while an
external sampler records working set and private commit at 200 ms, with
allocator stats at each phase boundary. Three arms:

| arm | peak working set | resident after teardown | commit after teardown |
|---|---:|---:|---:|
| CRT heap | 255 MiB | 13 MiB | 6 MiB |
| rusty_alloc, purge off (shipped default) | 289 MiB | **236 MiB** | **1030 MiB** |
| rusty_alloc, purge on | 286 MiB | **16 MiB** | 896 MiB |

The memory-heavy phase is scene building (raComm 256 MiB vs 64 for the
texture burst) — matching the app's 256/64 MiB scene-cache budget as the
dominant live consumer. The archive mmap contributes ~0 to private commit
(the CRT arm maps the same archive and shows 6 MiB), so the tail is the
allocator: with purge off, a live heap never returns freed pages, and
after a session whose live peak was ~220 MiB it parked 236 MiB resident
and ~1 GiB of commit (32 segments × 32 MiB reserved). The caches bound
*live* memory; they do not bound *parked* memory. That distinction was
the original adoption's error.

**Decision: purge on at startup** — `init_allocator()` sets
`purge_delay = 0` via `options::set_default`, so an explicit
`RUSTY_ALLOC_PURGE_DELAY`/`MIMALLOC_PURGE_DELAY` environment value still
wins. Evidence:

- **Resident parity restored:** 16 MiB vs the CRT's 13 MiB after teardown.
- **No speed cost.** `benchmark_table` interleaved 5× each: open median
  9.93 ms (purge on) vs 9.87 ms (off), fully overlapping spreads.
  `benchmark_export` ZeroCopy: 25.5 / 25.6 s on vs 26.2 / 28.0 s off.
  The purge gate only fires for coalesced free spans ≥ 128 KiB, so the
  small-allocation UI paths never pay a syscall.
- **Stability.** The full 751-test suite passes with purge on. The
  activated path is `span_free`'s purge of medium+ spans, which marks each
  purged span so reuse re-commits it — the defect's own fix (2026-08-05).
  It is *not* the "purge every free span" path that crashed the crate's
  test suite (that one reaches un-recommitted small spans). The M8
  arena-recycle variant is unreachable here: rusty_alloc arenas require a
  registered memory region and this app registers none.
- **Honest remainder:** private commit stays ~900 MiB after teardown —
  decommitted spans release their pages but the segment reservations
  remain for reuse. Commit is pagefile headroom, not RAM; the resident
  number — the one that mattered for low-spec machines — is fixed.

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
  empty pages back into their segments — but it deliberately does not purge
  (the forced-everything-purge variant is the one that faults on un-recommitted
  small spans), so it does not lower RSS. Our worker threads never die, so
  orphan adoption finds almost nothing. No measurable benefit; not wired.
  The startup `purge_delay = 0` setting is the lever that works.
- **`arena` module.** Supplies memory *to* the allocator (bare-metal regions,
  guarded sampling); it is not a consumer-side feature. Irrelevant on Windows
  desktop.
- **`bins::good_size` pre-sizing.** Rounding capacities to bin sizes to avoid
  internal fragmentation is micro-optimization below this app's measurement
  floor; the A/B shows the allocator's own placement already wins.

## Re-running the A/B

The harnesses are local-only (gitignored) `examples/`. To reproduce an arm
pair: build the examples, stash the exes, comment out the two
`#[global_allocator]` lines in `src/lib.rs`, rebuild, and interleave the two
binaries over the same archive (5+ rounds; medians; quote per-round spread for
any headline number).

## Profiling method — how the retention numbers were made (2026-09-30)

Preserved because the harness itself is gitignored and the measurement had
non-obvious traps.

**Harness:** `examples/benchmark_memory.rs` (local-only). Scripted session on
`corpus/Gta_3_img/gta3.img`: open → decode N TXDs to RGBA (hold) → drop →
build M DFF scenes via `parse_dff` + `build_scene_from_dff` (hold) → drop →
close archive → idle 20 s. Each phase holds 3 s so a 200 ms sampler captures
its steady state; phase boundaries append wall-clock markers +
`stats::merged()` to a CSV.

**Sampler:** a fresh `Get-Process -Id` object per 200 ms sample
(`WorkingSet64`, `PrivateMemorySize64`). Traps found, each of which produced
a wrong number before it was fixed:

- A cached `Start-Process -PassThru` object with `Refresh()` returned a
  frozen private-bytes value (a flat 1030 MiB through the whole session).
  Fresh objects per sample are mandatory.
- `Win32_PerfFormattedData_PerfProc_Process` filtered by `ProcessID`
  returned zero instances on this machine even for live processes — the
  provider is partially broken here; do not use it.
- `'{0:N1}'` formatting embeds the culture's thousands separator and
  silently corrupts CSV columns. Use `[int]` casts.

**rusty_alloc's own counters are not trustworthy for attribution:**
`stats::merged()` reported `allocs`/`frees` as 0 through the entire session
while its `segments` counter moved, and its implied committed bytes (160 MiB
at exit) disagreed with the OS private-commit reading (1030 MiB). Treat
`merged()` as indicative only; the OS working-set/commit numbers are the
ground truth. (Likely cause: the registry iteration misses the main thread's
heap counters — unverified, and not worth chasing given the OS data.)

**Arms and raw results** (60 TXDs / 2000 scenes session; medians; WS =
working set, PRIV = private commit):

| phase | CRT | rusty purge off | rusty purge on |
|---|---|---|---|
| textures held (23 MiB RGBA) | 57 WS / 33 P | 59 WS / 1028 P | 59 WS / 1028 P |
| scenes held (194 MiB est.) | 255 WS / 198 P | 289 WS / 1030 P | 286 WS / 1030 P |
| after scene drop | 69 WS / 12 P | 289 WS / 1030 P | 73 WS / 902 P |
| teardown idle (20 s) | **13 WS / 6 P** | **236 WS / 1029 P** | **16 WS / 896 P** |

Speed gates for the purge decision: `benchmark_table` 5 interleaved rounds
each arm — open median 9.93 ms (purge on) vs 9.87 ms (off), spreads fully
overlapping; `benchmark_export` ZeroCopy 25.45/25.64 s on vs 26.25/28.02 s
off. The 751-test suite passes with `RUSTY_ALLOC_PURGE_DELAY=0`.

## Codebase memory map (for future structural work)

Where the bytes live, so a structural pass can start from measurements
instead of guesses:

- **Scene decode dominates.** `parser/dff.rs` (`parse_dff`) +
  `inspector/scene3d/decode.rs` (`build_scene_from_dff`) produced 256 MiB of
  allocator commitment for 2000 scenes (~194 MiB estimated live) — 4× the
  texture burst. The UI caps this cache at 256/64 MiB
  (`SCENE_CACHE_WEIGHT_CAPACITY` in `ui/app.rs`, desktop/mobile).
- **Texture decode is secondary:** `parser/txd.rs` (`parse_txd`,
  `NativeTexture::decode_rgba`) — 23 MiB for 468 textures here; cache cap
  128/32 MiB.
- **Archive parse is small:** 16,316 entries ≈ a few MiB of `EntryInfo` +
  `CompactString` (the `benchmark_table` open phase, ~10 ms).
- **Export/Save are IO-bound:** zero-copy from the mmap; the allocator is
  irrelevant there (unchanged across every arm).

If structural work is ever wanted, the levers are: lower desktop cache
caps, stream decode→GPU-upload without holding the CPU copy, and reuse
decode scratch buffers. These reduce **peak** working set (the ~286 MiB
above); they do not affect the tail, which purge already fixed.

## Removing rusty_alloc someday

Three edits: delete the `#[global_allocator]` block and `init_allocator()`
from `src/lib.rs`, the `imgeditor::init_allocator()` call from
`src/main.rs`, and both `rusty_alloc*` lines from `Cargo.toml`. The memory
consequence is nil-to-positive (the CRT heap's measured tail was 13 MiB /
6 MiB — better than rusty's 16 / 896 on the commit axis). The cost is the
−10…−15 % on open/filter/sort. Do not remove without re-running the
interleaved A/B above; and note the commit-number asymmetry means a
pagefile-constrained machine is a legitimate reason to prefer the CRT arm.
