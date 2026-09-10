# Independent `gta-img` reference audit

Status: reference notes and implementation record for IMGEditor Plus
Checked: 2026-09-10
Reference: `https://github.com/connorhaigh/gta-img`
Snapshot: commit `5f63eeb441c73a6bf99190b1c966bff035578bf4` (`v0.2.0`)
Local checkout: `C:\Dev\IMGEditor-master\gta-img`

This document preserves the useful findings from inspecting the independent Rust
implementation so that future work does not require re-reading the reference
repository. It is a comparison and design record, not a claim that the reference
implementation is the compatibility authority. Real, legally obtained GTA III,
Vice City, and San Andreas archives remain necessary for compatibility validation.

The project scope is GTA III/3, Vice City, and San Andreas IMG archives. The
earlier mention of “GTA II” was a typo; this note does not establish or add a GTA
II format. IMG v3/RPF is also outside this phase.

## Repository and provenance

The checkout was made as a separate reference repository and is not part of the
authoritative `IMGEditor-rs` Git repository.

- `master` and `origin/master` point to `5f63eeb`.
- `origin/develop` points to the same commit; it contains no newer implementation.
- The latest commit is dated 2024-12-21 and changes the package to `v0.2.0`.
- The project is MIT licensed (`licence.txt`). Any future code reuse would still
  need to preserve the required copyright and license notice.
- The crate uses Rust 2021 and has only `byteorder` as a library dependency;
  `clap` supports its example CLI.

The reference was inspected locally rather than treated as an unreviewed code
snippet. Its source, examples, fixtures, commit history, branches, and tests were
read at the snapshot above.

## Repository layout

The implementation is intentionally compact:

| Path | Role |
| --- | --- |
| `src/lib.rs` | Public modules and shared constants: 2048-byte sectors, 23-byte maximum name, and `VER2`. |
| `src/read.rs` | v1/v2 readers, archive metadata, bounded entry reads, and read tests. |
| `src/write.rs` | v1/v2 streaming writers, sector padding, and write tests. |
| `src/error.rs` | Small read/write error enums. |
| `src/main.rs` | `inspect` and `extract` demonstration CLI. |
| `examples/` | Small library usage examples for inspecting, reading, and writing. |
| `test/` | Synthetic v1/v2 archives and tiny DFF payloads embedded by tests. |

The design has no UI, asynchronous runtime, memory map, asset decoder, preview
cache, or 3D renderer. It is a focused archive library with a demonstration CLI.

## Public API design

### Generic reader and writer traits

`src/read.rs` defines a generic `Reader` trait whose `read` method consumes a
format-specific reader and returns an in-memory `Archive` metadata object. The
format types are:

- `V1Reader<'a, 'b, D, I>` reads a separate directory source and image source.
  The directory only needs `Read`; the image needs `Read + Seek`.
- `V2Reader<'a, I>` reads a single image source with `Read + Seek`.
- `Archive` owns the parsed `Vec<Entry>` while retaining a mutable borrow of the
  image source.
- `Entry` exposes a display name, sector offset, and sector length.

`src/write.rs` defines a generic `Writer` trait. `V1Writer` and `V2Writer`
accept any source implementing `Read`, so an entry can be copied from a file,
cursor, or another stream rather than requiring a source path.

This is a useful separation for unit testing: the same parser can consume a
`Cursor<Vec<u8>>` or a real file. Our `ImgParser` trait and `ArchiveInfo` already
provide a richer application-level separation, but this reference confirms that
format-independent byte sources are a clean seam for lower-level tests.

### Bounded `OpenEntry` reader

`Archive::open(index)` returns an `OpenEntry` that implements `Read`. It stores:

```text
inner source, byte offset, byte length, current position
```

Each read:

1. Stops with `Ok(0)` after the declared entry length.
2. Seeks to `offset + position` in the shared image source.
3. Limits the destination slice to the remaining entry bytes.
4. Advances its own position by the number of bytes returned by the source.

This makes `io::copy(&mut entry, &mut output)` possible without first allocating
a `Vec<u8>` for the complete entry. It also prevents a caller from reading into
the next archive entry through the bounded view.

Our current port already has stronger paths for its main workloads:

- archives are memory-mapped when opened;
- `stream_entry_data` writes archive-backed data without a per-entry `Vec` during
  rebuilds;
- the default bulk export uses the memory map and parallel zero-copy slices;
- buffered fallbacks use a large `BufReader` when a map is unavailable.

Therefore `OpenEntry` should be treated as an API pattern, not copied wholesale.
If future refactoring finds duplicated file/mmap fallback logic, a read-only
`EntrySource`/bounded-reader abstraction could unify those paths while retaining
our mmap, imported-file, cancellation, and validation behavior.

The reference reader does not implement `Seek` for `OpenEntry`; it only exposes
sequential reads. That is sufficient for extraction but not a complete random
access view.

## Format behavior confirmed by the reference

### Shared rules

- Sector size is 2048 bytes.
- Entry names occupy 24 bytes, with a practical 23-byte NUL-terminated name.
- Offsets and lengths are stored in sectors, not raw byte counts.
- The reference reads and writes little-endian integer fields.
- Entry payloads are copied and padded to a whole sector on writing.

### IMG v1

The reference reads a `.dir` stream until EOF. Each record is 32 bytes:

| Bytes | Size | Meaning |
| --- | ---: | --- |
| `0..4` | 4 | IMG data offset in sectors, `u32` |
| `4..8` | 4 | Entry length in sectors, `u32` |
| `8..32` | 24 | Filename field |

The image source begins at `offset * 2048`, and the bounded entry length is
`length * 2048`. The v1 writer starts at sector zero and emits directory records
in the same order as the payloads.

### Bully Xbox 360 IMG v1 (implemented)

Bully Scholarship Edition's Xbox 360 archive pair uses the same physical v1
record shape but stores the two directory integers in **big-endian** order:

```text
Scripts.dir   32-byte records to EOF
Scripts.img   sector-aligned payload data
```

The local sample at `C:\Dev\IMGEditor-master\Bully script img xbox 360`
contains 528 `.lur` records. Its directory is 16,896 bytes and its image is
5,281,792 bytes (2,579 sectors). The records are contiguous, cover the image
exactly, and every payload begins with `\x1bLuaP`. These checks establish that
the sample is a valid parser fixture even though its retail provenance and
complete game file list are unknown.

The independent [BullyX360img QuickBMS script](https://github.com/EdnessP/scripts/blob/main/bully/BullyX360img.bms)
matches this layout: it selects big-endian integers, reads 32-byte records,
multiplies offsets and sizes by 2048, and treats the 24-byte name field as raw
data. It also recognizes an optional `0x0FF512ED` XMemDecompress marker in the
image. That marker is absent from the supplied `Scripts.img`, so the current
implementation supports the uncompressed variant while compressed archive
support remains a separate follow-up.

IMGEditor Plus now detects this layout before opening the archive, dispatches it
to `Xbox360Parser`, preserves the payload bytes and sector padding during
export/rebuild, writes big-endian directory fields, and allows all 24 filename
bytes for Xbox entries. The existing PC v1 parser remains little-endian.

### IMG v2

The reference requires the following eight-byte prefix:

| Bytes | Size | Meaning |
| --- | ---: | --- |
| `0..4` | 4 | ASCII `VER2` |
| `4..8` | 4 | Entry count, little-endian `u32` |

It then reads 32-byte records:

| Bytes within record | Size | Meaning |
| --- | ---: | --- |
| `0..4` | 4 | IMG data offset in sectors, `u32` |
| `4..6` | 2 | Streaming size in sectors, `u16` |
| `6..8` | 2 | Size in archive, `u16` |
| `8..32` | 24 | Filename field |

The reference’s current reader keeps the first size word as `Entry.length` and
reads/discards the second size word. Its writer emits the length as `u16` and
writes zero for the second word. This independently reinforces the split-field
wire layout already recorded in `img-format-validation.md`, but it does not
settle the semantics for every real San Andreas build.

The v2 writer reserves the header-derived number of sectors:

```text
first_data_sector = ceil((8 + 32 * entry_count) / 2048)
```

That is compact and works for the reference’s synthetic files. It differs from
IMGEditor Plus’s current v2 rebuild convention of starting data at `0x300000`,
which follows the original C++ editor’s convention. The compact layout must not
replace the current one until real San Andreas archives prove it is accepted by
the target game/tools.

## Writer behavior worth remembering

The reference writers contain a few ideas and several edge cases:

- `io::copy` lets writing remain streaming and source-type agnostic.
- `div_ceil(2048)` computes the sector count.
- The remainder of the final sector is explicitly zero-padded.
- `V2Writer::new` receives the expected entry count so it can reserve the v2
  table before data is written.
- v2 refuses writes after the declared entry capacity with
  `InsufficientHeaderSize`.
- v1 and v2 data is written in physical entry order.

The following behaviors are not safe to inherit:

- Empty input produces a zero-sector entry because `0.div_ceil(2048) == 0`.
  The next entry can therefore reuse the same offset.
- `entries as u32`, `offset as u32`, and `length as u16` are unchecked casts.
- The writer does not require that exactly the declared v2 entry count was
  written before it is dropped.
- `InvalidNameLength` exists but is not returned by the writer.
- Name conversion silently drops characters that cannot be converted to one byte
  and truncates to the field width.
- There is no transaction/finalization layer for coordinating a v1 `.dir` and
  `.img` pair if a later write fails.

## Reader behavior and hardening gaps

The independent implementation is useful as a format comparison, but it is not
an adversarial parser. It does not validate all relationships between metadata
and file lengths before exposing an archive.

### Partial and truncated input

- The v1 reader treats `UnexpectedEof` while reading the next offset as normal
  end-of-directory. A directory truncated to one through three bytes can
  therefore look like a valid empty/end-of-file condition.
- The v1 and v2 filename helper uses `Read` rather than `read_exact`; a short
  name field can be accepted and converted from the bytes that arrived.
- A declared entry can extend beyond the image file. The error appears only when
  the bounded stream reaches the underlying EOF, rather than during archive open.
- v2 allocates its entry vector from the untrusted count before proving that the
  complete table fits in the input.

### Integer and range safety

The reference multiplies sector values by 2048 when opening an entry and casts
writer counters to on-disk widths without explicit checked arithmetic. A robust
application parser should check:

```text
header/table size
offset * sector_size
effective_size * sector_size
offset_bytes + size_bytes <= image_file_length
```

before mapping or exporting. This is especially important for a desktop editor
that accepts archives from mods and third-party tools.

### Extraction safety

The demonstration CLI creates an output path with `target.join(entry.name)`.
An untrusted archive name containing path separators, an absolute path, or `..`
could escape the requested extraction directory. IMGEditor Plus should normalize
or reject unsafe archive names before creating any output file.

### Documentation and build hygiene

The reference’s README examples are stale relative to its current API: one v1
example calls `.expect()` directly on `V1Reader::new` instead of calling `.read()`,
and one v2 example calls a top-level `gta_img::read(...)` function that is not
defined by the current `lib.rs`.

`cargo test --locked` at the inspected revision passes all 12 unit tests, but the
build reports two warnings:

- a hidden lifetime in `Archive::open` (`OpenEntry<I>` should spell out
  `OpenEntry<'_, I>`);
- an unused `command` import in `src/main.rs`.

## Test and fixture inventory

The reference’s tests are useful as small, readable examples, but they are not
real-game validation:

| Fixture | Size | Purpose |
| --- | ---: | --- |
| `test/v1.dir` | 96 bytes | Three synthetic v1 records |
| `test/v1.img` | 4096 bytes | Two sectors of synthetic v1 payload data |
| `test/v2.img` | 6144 bytes | Three synthetic v2 records and three data sectors |
| `test/virgo.dff` | 9 bytes | Tiny synthetic write source |
| `test/landstal.dff` | 12 bytes | Tiny synthetic write source |

The 12 tests cover name conversion, v1/v2 metadata reads, bounded partial reads,
v1 writing, v2 writing, sector padding, and v2 writer capacity. They do not cover
real archive headers, malformed ranges, path traversal, non-ASCII names, large
counts, empty files, or round-tripping a real GTA III/VC/SA archive.

IMGEditor Plus currently has broader application tests and Bully fixture coverage,
but its GTA III/VC/SA claims should still be checked against real archives once
they are available. Game archives and extracted copyrighted assets should remain
outside the repository; commit only manifests, hashes, metadata, and synthetic
fixtures.

## Comparison with IMGEditor Plus

| Concern | `gta-img` reference | IMGEditor Plus current direction |
| --- | --- | --- |
| Scope | Small v1/v2 library and CLI | Windows desktop editor with v1/v2 parsers, import/export, previews, and 3D tooling |
| Source access | Generic borrowed `Read`/`Seek` sources | Memory-mapped archives, buffered fallbacks, imported-file sources, and async/blocking task boundaries |
| Entry reads | Bounded `OpenEntry` stream | Mmap slices and streaming save/export helpers; most preview decoders still need byte slices |
| v1 records | 32-byte records to EOF | 32-byte records with raw name preservation; partial directory size is rejected |
| v2 records | Reads two `u16` words but discards the second | Currently flattens bytes `4..8` into one generic `sector` field; separate v2 metadata remains TODO |
| v2 data start when writing | Header-derived compact start | `0x300000`, matching the original C++ convention pending real SA validation |
| Validation | Header checks and ordinary I/O errors | More UI/application checks, but open-time data-range validation is still a documented gap |
| Extraction | Simple CLI path join | Must add explicit containment checks before broadening real-archive workflows |
| Tests | 12 synthetic unit tests | Larger parser/UI/renderer suite, but real GTA III/VC/SA fixtures are still pending |

The reference does not supersede our current mmap and zero-copy work. Its main
architectural contribution is a clear, testable way to represent a bounded entry
source independent of the filesystem.

## Recommended adaptations

These are ordered for the GTA III/VC/SA validation phase, before considering any
new container family:

1. **Preserve v2 size words.** Add a v2-specific record/metadata type holding
   `streaming_size` and `archive_size` separately. Expose a checked effective
   sector count (streaming size when nonzero, otherwise the archive size) and make
   save/rebuild preserve both words where possible.
2. **Validate before mapping.** Check v1 directory record completeness, v2 count
   and table arithmetic, integer conversions, sector multiplication, and every
   entry range against the image length during open. Do not rely on a clamped
   mmap read or a late EOF from an entry stream.
3. **Canonicalize v1 pairs.** Accept either a `.img` or its sibling `.dir`, map
   the `.img` as the data source in both cases, expose `.dir` in open/drop filters,
   and test both entry points.
4. **Protect extraction paths.** Reject absolute paths, separators, and parent
   components in archive entry names, or prove that the normalized destination
   remains under the selected output directory.
5. **Add a diagnostics path.** Provide a small read-only metadata/entry-range
   inspection command or developer mode that can compare IMGEditor Plus with
   `gta-img` on real archives without importing copyrighted files into the repo.
6. **Build real-archive manifests.** For each legally obtained PC release, keep
   local untracked archive paths and hashes; commit only metadata, selected
   exported-byte checks where legally appropriate, and synthetic malformed-input
   fixtures.
7. **Evaluate a bounded reader only if needed.** If fallback export/preview code
   accumulates duplicate source-range logic, introduce a Rust-flavored bounded
   reader over our mmap/file/imported sources. Do not replace the current parallel
   zero-copy path solely to match the reference API.

## Explicit non-adaptations

- Do not copy the reference’s compact v2 data-start policy without a real San
  Andreas compatibility result.
- Do not inherit unchecked casts, late range failures, silent short reads, or
  path-unsafe extraction.
- Do not add IMG v3 or RPF as part of this audit; that is a separate scope choice.
- Do not copy source code line-for-line. The reference is MIT, but the preferred
  approach is to preserve attribution obligations and implement the useful ideas
  within IMGEditor Plus’s existing ownership, caching, and async architecture.
