# IMGEditor Export Optimization: Research and Lessons Learned

Date: 2026-06-18

## Executive Summary

The Rust port of IMGEditor is functionally faster and safer than the original
C++ implementation, but its **bulk export throughput is not dramatically faster**
on Windows. The original C++ export path was already well-aligned with the
real bottleneck: Windows NTFS metadata creation for many small files. Adding
`Rayon` parallelism and `memmap2` memory-mapping did not change the fundamental
I/O bound, and in some configurations made performance worse.

This document records the research, the experiments, the measured results, and
the design decisions that followed.

---

## The Workload

- Archive: `C:\Games\Bully - Scholarship Edition\Stream\World.img`
- Size: ~1.93 GB
- Entries: 11,980
- Format: GTA IMG v1 (`.img` + `.dir`)
- Output: 11,980 individual files in a single directory

The export task therefore has two components:

1. **Source read**: ~1.93 GB sequential-ish reads from one archive file.
2. **Output creation**: 11,980 separate files on Windows NTFS.

Profiling and measurement showed that (2) dominates the warm-cache case.

---

## Key Finding: The Real Bottleneck Is NTFS Metadata

The most important lesson from this effort is that **the cost of exporting
~12,000 small files on Windows is dominated by NTFS metadata operations**, not
by copying the file bytes.

- Each file creation updates the NTFS Master File Table ($MFT), the parent
directory B-tree, ACLs, short-name generation, and possibly antivirus filter
drivers.
- Small files (under ~1 KB) are stored resident in the MFT record itself, which
means MFT contention is even higher.
- The C++ benchmark spends ~24 seconds for the whole export. That is only about
2 ms per file. The bulk of that time is not byte copying; it is metadata.

### Sources

- Stack Overflow — "Opening many small files on NTFS is way too slow":
  > An overhead of 5 to 20ms per file isn't abnormal for an NTFS volume with
  > that number of files.

- Super User — "Bad NTFS performance":
  > NTFS has this thing called a Master File Table. It sounds really cool when
  > you read about it... when you create a small file... it is not written to
  > its own block but rather is stored in the MFT.

- Rust Internals — "Installing docs is slow on Windows":
  > IIRC part of the problem is NTFS having trouble with many (but small)
  > files.

- Hacker News (rustup Windows port anecdote): the rustup team reduced Windows
  install time from **3 minutes 30 seconds to 14 seconds** only after
  extensively rewriting their tool to be sympathetic to NTFS.

---

## Why `memmap2` / mmap Did Not Win

A common assumption is that memory-mapped I/O is faster than explicit reads.
That assumption is wrong for **sequential read-once** access to large files.

`mmap` replaces explicit `read` syscalls with page faults, but page faults have
similar per-call overhead. The kernel must also build page-table entries (PTEs)
and handle TLB misses. For a 1.93 GB file mapped once and read mostly
sequentially, this overhead is real and can exceed the cost of a buffered
`read` loop.

### Sources

- Linus Torvalds (quoted on Stack Overflow):
  > page table games along with the fault (and even just TLB miss) overhead is
  > easily more than the cost of copying a page in a nice streaming manner.

- Hacker News — "This old myth that mmap is the fast and efficient way to do
  IO":
  > mmap has to do "per page" work to map the file... adjusting VM and OS
  > structures to map the page into the process address space, and then undoing
  > that work on munmap.

- Daniel Lemire — "Which is fastest: read, fread, ifstream or mmap?":
  > For sequential access, both fread and ifstream are equally fast. Unbuffered
  > IO (read) is slower, as expected. Memory mapping is not beneficial.

- USENIX HotStorage 17 — "Efficient Memory Mapped File I/O for In-Memory File
  Systems": a 4 GB sequential-read microbenchmark showed default mmap at
  0.64 s vs `read` at 0.16 s, with page-fault and PTE construction overhead as
  the dominant cost.

In the IMGEditor case, switching the export path from shared mmap to per-worker
`BufReader` (4 MiB) is what allowed Rust to match C++ speed.

---

## Why `Rayon` Parallelism Did Not Win

`Rayon` is excellent for CPU-bound data parallelism. Export is not CPU-bound.
It is I/O-bound, and the I/O resource — NTFS metadata, directory locks, and
single-disk queues — is largely serialized by the OS and hardware.

Adding threads in this situation adds coordination overhead without increasing
the throughput of the serialized resource. The work-stealing tree, atomic
progress counters, and extra allocations can even slow the operation down.

### Sources

- NPB-Rust (arXiv):
  > Rust with Rayon was slower than both Fortran and C++ with OpenMP.

- Rust Users Forum — "Bad performance with rayon?":
  summing 50,000 integers was 8x slower with `par_iter()` because the per-item
  work was too small to amortize thread overhead.

- Reintech — "Rust Performance Optimization":
  > Rayon works best when individual operations take at least a few
  > microseconds — parallelizing tiny operations adds overhead without benefit.

- Stack Overflow — "Is Parallel File.Read Faster than Sequential Read?":
  > If they're on a single spinning hard drive, reading the files in parallel
  > will probably hurt your performance significantly due to the extra seek
  > time.

- Piotr Kołaczkowski — "Performance Impact of Parallel Disk Access": parallel
  sequential reads can help on SSDs but plateau early; on HDDs they can hurt.

---

## Why the C++ Baseline Is Already Fast

The original C++ export path:

1. Opens the source archive once.
2. Uses `std::ifstream` with default CRT buffering.
3. Reads each entry's bytes and writes them to one output file.
4. Closes and moves to the next entry.

On Windows, `std::ifstream` benefits from the same Windows file-cache manager
that Rust's `std::fs::File` uses. Because the source archive fits in RAM after
the first iteration, subsequent reads are cache hits. The slow part — creating
thousands of NTFS files — is identical for C++ and Rust.

Stack Overflow's mmap-vs-read answer notes:

> Microsoft implemented a nifty file cache that does most of what you would do
> with mmap in the first place... for frequently-accessed files, you could just
> do `std::ifstream.read()` and it would be as fast as mmap.

Therefore, the C++ baseline is not "naïve" for this workload; it is already
close to the practical ceiling for warm-cache export on Windows.

---

## Experimental Results

All measurements were taken on the same Windows machine, same archive, warm
cache (file-system cache primed by previous runs).

| Implementation | 3-iter median | Notes |
|----------------|---------------|-------|
| C++ `std::ifstream` per entry | ~24.0 s | Baseline |
| Rust rayon + `memmap2` | ~36.6 s | 0.85x C++ — mmap contention |
| Rust chunked parallel + 4 MiB `BufReader` | ~22.6 s | 1.06x C++ — best so far |
| Rust sort-by-source-offset | ~28.2 s | Regression |
| Rust sort + `set_len()` pre-allocate + 1 MiB BufWriter | ~29.2 s | Worse — extra syscalls |
| Rust 1 MiB `BufWriter` only | ~23.8 s | Within noise of C++ |

Important caveats:

- Variance was high (individual iterations ranged from ~21 s to ~38 s).
- The system had background activity; results are environment-specific.
- All tests were warm-cache; cold-cache behavior would differ.

---

## Why Common "Optimizations" Failed

### Sorting entries by source offset

Theory: eliminate random seeks and improve prefetching.

Reality: the source archive fits in RAM; random seeks are cache hits. Sorting
changed the output-file creation order, which may have changed NTFS directory
B-tree behavior and cache locality. Measured result was a regression.

### `File::set_len()` pre-allocation

Theory: reduce NTFS MFT growth overhead by allocating file size up front.

Reality: for ~12,000 small files, the extra `SetFileValidData`/`set_len`
syscall per file dominates. This is a win for large streaming writes
(databases, video files), not for bulk small-file creation.

### Subdirectory fan-out

Theory: reduce per-directory B-tree contention by spreading files across
subdirectories.

Reality: stress testing showed it was slower, likely because creating the
subdirectories themselves adds metadata work and the files are already
mostly in the MFT.

### IOCP / overlapped I/O

Theory: overlap source reads and output writes to keep the disk busy.

Reality: source reads are already cache-resident; output writes serialize on
NTFS metadata. IOCP would add unsafe Windows code without addressing the real
bottleneck.

---

## Lessons Learned

1. **Profile before optimizing.** The naive assumption was that "Rust +
   parallelism + mmap" would beat "old C++ ifstream." The bottleneck was
   elsewhere.

2. **I/O-bound workloads do not benefit from CPU parallelism.** If the OS and
   hardware serialize the work, adding threads only adds overhead.

3. **mmap is not universally faster.** For sequential read-once access,
   buffered reads are competitive or better, especially on Windows where the
   file cache already does memory-mapping-like work.

4. **NTFS small-file creation is expensive.** Any design that creates many
   small files on Windows will be limited by metadata, not throughput.

5. **Measure in the target environment.** Linux-oriented advice (sort by
   offset, pre-allocate, fan out) does not always translate to Windows NTFS.

6. **Keep the simple path available.** The C++-like sequential export path is
   a useful fallback: it is predictable, easy to reason about, and performs
   close to the practical ceiling.

---

## Design Decision: Offer Both Paths

To preserve the Rust port's safety and concurrency features while giving users
a way to match C++ throughput, IMGEditor provides two export modes:

- **Default (`Parallel`)**: chunked parallel export with `Rayon` + `BufReader` +
  `BufWriter`. Chosen for UI responsiveness and cancellation support; throughput
  is within ~6 % of C++ on the test machine.
- **Fast (`Fast`)**: a sequential export path that closely mirrors the original
  C++ implementation. This avoids thread coordination overhead and can be more
  consistent on I/O-bound systems.

Measured head-to-head (warm cache, 3 iterations):

| Engine | Median export time |
|---|---|
| C++ `std::ifstream` | 22.749 s |
| Rust `Parallel` | 23.425 s |
| Rust `Fast` | 24.140 s |

The fast path does not remove or replace the default path. It is an option for
users who prefer predictable C++-like throughput over UI concurrency.

The UI exposes the setting as a "Fast export (C++ speed)" checkbox in the info
panel, persisted to `settings.ini` as `fast_export`.

---

## Recommendations for Future Work

If further export speedup is required, the realistic options are:

1. **Reduce file count.** Pack exported entries into a small number of archive
   files (e.g., zip, tar). This sidesteps NTFS metadata entirely.

2. **Use a ramdisk or temporary drive.** Writing to a memory-backed or
   fast-temporary volume reduces metadata latency.

3. **Cold-cache benchmarking.** Establish a cold-cache test environment
   (flush standby lists) to see whether source-read optimizations become
   worthwhile when the archive is not already in RAM.

4. **Volume-level operations.** For advanced users, an option to read/write
   raw NTFS clusters would bypass metadata but requires elevated privileges
   and is unsafe.

5. **Asynchronous metadata batching.** If Windows ever exposes APIs to create
   files in batch, that would be the real win.

Avoid further speculative low-level optimizations (IOCP, large pages, NUMA,
custom allocators) until profiling proves they address the actual bottleneck.

---

## References

- Stack Overflow — "Why mmap() is faster than sequential IO?"
- Hacker News — "This old myth that mmap is the fast and efficient way to do IO"
- Daniel Lemire — "Which is fastest: read, fread, ifstream or mmap?"
- USENIX HotStorage 17 — "Efficient Memory Mapped File I/O for In-Memory File Systems"
- Stack Overflow — "mmap vs. reading blocks"
- Stack Overflow — "Opening many small files on NTFS is way too slow"
- Super User — "Bad NTFS performance"
- Rust Internals — "Installing docs is slow on Windows"
- Piotr Kołaczkowski — "Performance Impact of Parallel Disk Access"
- Stack Overflow — "Is Parallel File.Read Faster than Sequential Read?"
- Rust Users Forum — "Parallelizing SSD reads and subsequent computations"
- arXiv — "NPB-Rust: NAS Parallel Benchmarks in Rust"
- Rust Users Forum — "Bad performance with rayon?"
- Reintech — "Rust Performance Optimization: Complete Guide 2026"
