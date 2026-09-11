# IMG v1/v2 validation notes

Status: research baseline for GTA III, Vice City, San Andreas, and Bully Xbox 360
Checked: 2026-09-10
Scope: PC IMG v1/v2 and the Bully Xbox 360 big-endian IMG v1 variant; IMG v3
and RPF are intentionally out of scope.

## Why this document exists

IMG Editor Plus was derived from the original C++ IMG Editor, and its parser has
so far been exercised primarily with synthetic archives and Bully assets. This
document records the external format knowledge that should be checked against
real, legally obtained GTA III, Vice City, and San Andreas archives before we
claim full compatibility.

The request that mentioned “GTA II” is kept distinct from GTA III here. The
existing code, the original IMG Editor, and the consulted IMG references all
identify the early 3D IMG family as GTA III, Vice City, and San Andreas. No
IMG-compatible GTA II layout was established during this audit, so a GTA II
sample must not be routed through the v1/v2 parser without separate format
research and fixtures.

## Sources consulted

The sources agree on the broad IMG family and sector model, but independent
implementations do not all model every v2 field with the same fidelity. The
community format reference is therefore treated as the specification to test,
while the Rust, C#, and C++ implementations are treated as compatibility
comparisons.

- [GTAMods IMG format reference](https://gtamods.com/wiki/Img) — sector size,
  v1/v2 headers and directory records, alignment, and historical game mapping.
- [connorhaigh/gta-img](https://github.com/connorhaigh/gta-img) — independent
  Rust reader/writer for IMG/DIR v1 and v2.
- [Hancapo/rwfury](https://github.com/Hancapo/rwfury) — independent
  implementation covering IMG together with RenderWare DFF/TXD assets.
- [vaibhavpandeyvpz/gtaimg](https://github.com/vaibhavpandeyvpz/gtaimg) — the
  local C# reference project and its `IMGArchive` implementation.
- [Sergeanur/ImgFileTables](https://github.com/Sergeanur/ImgFileTables) — real
  PC III/VC/SA entry tables useful for names, ordering, and manifest checks.
- [user-grinch/IMGEditor](https://github.com/user-grinch/IMGEditor) — original
  C++ implementation from which the Rust port was derived.

The web pages describe file structure; they do not replace testing against the
specific PC release being opened. Game patches, modded archives, and tools that
rebuild an archive can differ in ordering, reserved space, or unused fields.

## Shared IMG rules

Both supported versions use a 2048-byte sector/block for offsets and sizes. A
stored size of one sector therefore represents a file with between one and
2048 bytes of payload; the unused tail is archive padding. Files are normally
stored linearly, without compression or a directory tree, and the directory
records point into the data file.

The common name field is 24 bytes, normally a NUL-terminated short filename.
The practical maximum for a NUL-terminated name is 23 bytes. The Rust parser
preserves the raw 24-byte field in `EntryInfo::file_name_raw` and uses a lossy
UTF-8 display string. This is appropriate for the usual ASCII game names, but
raw-byte preservation should remain the source of truth if non-ASCII names are
encountered.

The external references also recommend keeping directory records in data-file
order with no unnecessary gaps. That matters for game streaming behavior even
though a general-purpose reader can seek to arbitrary records.

## IMG v1 — GTA III and Vice City

IMG v1 has two files with the same stem:

```text
gta3.dir   directory records
gta3.img   sector-aligned file data
```

The `.dir` file has no magic or count header. It is a sequence of 32-byte
records until EOF:

| Byte range | Size | Meaning |
|---|---:|---|
| `0..4` | 4 | Data offset in 2048-byte sectors, little-endian `u32` |
| `4..8` | 4 | Stored data size in 2048-byte sectors, little-endian `u32` |
| `8..32` | 24 | NUL-terminated filename field |

The corresponding bytes live at `offset * 2048` in the `.img` file and occupy
`size * 2048` bytes. The `.dir` and `.img` names must share a stem.

### Rust comparison

The Rust implementation matches the v1 record shape:

- [`SECTOR_SIZE`, `ENTRY_SIZE`, and name constants](../src/parser/mod.rs)
  are 2048, 32, and 24 bytes.
- [`PcV1Parser::open`](../src/parser/pc_v1.rs) reads the paired `.dir`, decodes
  offset/size/name, and maps the paired `.img` data file.
- [`PcV1Parser::save`](../src/parser/pc_v1.rs) writes records and data in
  sequential sector-rounded order.
- `sector_rounded_size` gives empty and sub-sector imported files at least one
  sector, which agrees with the sector-padded archive model.

The Rust parser improves malformed-input handling over the original C++
implementation: it rejects a `.dir` whose byte length is not divisible by 32,
instead of silently ignoring a trailing partial record, and validates printable
names, checked sector-to-byte arithmetic, non-overlapping non-empty ranges, and
each range against the `.img` length before mapping the data file. This same
validation is used by the Xbox 360 big-endian variant; v2 validation remains a
separate task.

### Known v1 compatibility gap

The current UI and detection path are centered on an `.img` input. The parser's
v1 helper changes an input path's extension to `.dir`; passing a `.dir` path
directly is therefore still a future compatibility task. The C# reference
explicitly accepts either `.img` or `.dir` and canonicalizes the pair before
reading. Future work should canonicalize a v1 `.dir` input to its sibling
`.img`, add `.dir` to the open/drop filters, and test both entry points.

For the Xbox 360 layout, see the implemented format and the deliberately
uncompressed-only boundary documented in the local `gta-img` reference-audit
notes.

## IMG v2 — GTA San Andreas

IMG v2 combines the directory and data into one `.img` file. Its first eight
bytes are:

| Byte range | Size | Meaning |
|---|---:|---|
| `0..4` | 4 | ASCII `VER2` |
| `4..8` | 4 | Entry count, little-endian `u32` |

The count is followed by 32-byte records. The documented v2 record is:

| Byte range | Size | Meaning |
|---|---:|---|
| `0..4` | 4 | Data offset in sectors, little-endian `u32` |
| `4..6` | 2 | Streaming size in sectors, little-endian `u16` |
| `6..8` | 2 | Size in archive in sectors, little-endian `u16` |
| `8..32` | 24 | NUL-terminated filename field |

The streaming size is the number of sectors used when streaming the entry. If
it is zero, the archive-size field is used as the effective size. Stock PC
archives are generally reported to leave the archive-size field at zero and
use the streaming-size field, but both words are part of the format and should
be retained when reading and writing.

The data offset is relative to the beginning of the entire `.img`, not to the
end of the entry table. Tools commonly reserve a large header/data-start area
for extensibility; the current Rust writer uses `0x300000` as its rebuild data
start, matching the original C++ writer's convention. A real San Andreas
fixture must confirm that this remains appropriate for the archives we intend
to preserve.

### Rust comparison: structural match and important mismatch

The Rust implementation correctly recognizes `VER2`, reads the entry count,
uses 32-byte records, preserves 24-byte names, and interprets offsets in
2048-byte sectors. This is visible in [`PcV2Parser::open`](../src/parser/pc_v2.rs)
and the shared constants in [`parser/mod.rs`](../src/parser/mod.rs).

There is one important fidelity issue to resolve before declaring San Andreas
support complete: the current code reads `entry_record[4..8]` as one `u32`
(`EntryInfo::sector`) and writes the same flattened representation. The
original C++ parser and the local C# reference do the same, so our code agrees
with both implementations, but the external format reference describes two
separate `u16` size fields. A real SA archive is needed to settle how those
fields are populated in each supported game build. The safe Rust design is to
retain both words in a v2-specific metadata type and expose one checked
effective sector count for reading; this avoids losing information during a
save/rebuild.

Other v2 checks still needed:

- Ensure `8 + entry_count * 32` cannot overflow and is within the file before
  reading the table.
- Check every `offset * 2048 + effective_size * 2048` range against the IMG
  length during open. Current mmap reads clamp an out-of-range request rather
  than rejecting the archive early.
- Reject or clearly report names with no usable NUL-terminated content if a
  fixture shows that the game requires it.
- Preserve and test record order. Sorting records by name would change the
  archive's physical read pattern.
- Verify the rebuild reservation and empty-archive behavior against a real SA
  archive rather than relying only on synthetic tests.

## Implementation comparison

| Behavior | Original C++ | Local C# reference | Rust port | Assessment |
|---|---|---|---|---|
| Sector size | 2048 | 2048 | 2048 | Match |
| v1 directory size | 32-byte records to EOF | 32-byte records to EOF | 32-byte records, rejects partial tail | Compatible, Rust is stricter |
| v1 data location | paired `.img` | paired `.img` | paired `.img` for `.img` input | Match after `.dir` canonicalization is added |
| v2 marker/count | `VER2` + `u32` count | `VER2` + `u32` count | `VER2` + `u32` count | Match |
| v2 size words | flattened 4-byte field | flattened 4-byte field | flattened 4-byte field | Matches implementations; verify against spec/fixtures |
| Name storage | 24 bytes | 24 bytes | 24 raw bytes + display string | Match |
| Malformed range checks | minimal | sorted/order checks, some structural checks | read-time clamping, limited open-time checks | Rust hardening required |
| `.dir` as input | not supported by original UI path | explicitly supported | not yet canonicalized | Low-risk compatibility improvement |
| Archive mutation previews | no preview cache | no Rust scene cache | generation + texture cache + app scene cache | Move path now invalidates both sides |

The apparent v2 agreement between the C++/C# code and Rust must not be treated
as proof that the flattened field is correct for every stock archive. It is an
implementation inheritance point worth testing, not an independent
specification.

## Real-archive validation plan

When legally obtained samples are available, keep the binary archives outside
the repository. Commit only small metadata manifests, synthetic fixtures, or
tests that do not redistribute copyrighted game data.

For each PC release of GTA III, Vice City, and San Andreas:

1. Record the title, release/build, archive path, file sizes, and a SHA-256
   hash in a local, untracked manifest.
2. Detect the format without trusting the filename: v1 requires a valid pair;
   v2 requires `VER2` and a table that fits inside the file.
3. Read the first several records with IMG Editor Plus and an independent tool
   such as `gta-img`; compare names, offsets, raw size words, and calculated
   byte ranges.
4. Compare exported bytes for representative DFF, TXD, COL, animation, and
   data entries. The exported file should be trimmed only according to the
   effective sector size rule; sector padding is expected in an archive entry.
5. Check that offsets are sensible and that records are in physical data order.
6. Open, export, import/replace, save-as, and reopen a copy. Verify that the
   entry names, effective contents, and version survive the round trip.
7. Test a deliberately malformed copy: truncated table, partial v1 DIR
   record, out-of-range offset, oversized count, and a v2 record with distinct
   streaming/archive size words. The application should return a readable
   error and never panic or silently present truncated data.
8. If a rebuilt copy is intended for gameplay, test it in the matching game
   build only after the read/export comparison passes.

Useful manifest fields are:

```text
title/build
format (IMG v1 or IMG v2)
img_path / dir_path
img_sha256 / dir_sha256
file_size_bytes
entry_count
first_records: name, offset_sectors, raw_size_words, effective_size_sectors
validation_tool_and_version
notes
```

## Current project action list

### Completed in the cache fix

Cross-archive moves now:

- invalidate the source and target `ArchiveInfo` preview/file-type caches;
- bump both archive generations so cached scene keys cannot alias shifted
  entry indices;
- evict app-level scene-cache entries for both archives immediately;
- rebuild both filtered entry lists and their display-row lookup maps;
- remove source entries in descending original-index order;
- repair the selected entry and active viewer index when an earlier source row
  shifts it, or clear the viewer when the active entry is moved;
- mark both archives dirty.

The regression test is in [`ui/app.rs`](../src/ui/app.rs) and covers cache
clearing, generation changes, scene eviction, selection remapping, and the
source/target entry lists.

### Recommended next parser work

1. Add v1 `.dir` canonicalization and file-dialog/drag-and-drop coverage.
2. Add checked open-time table and data-range validation for v2.
3. Introduce v2-specific size metadata instead of overloading one generic
   `EntryInfo::sector` field.
4. Validate against real GTA III/VC/SA manifests and selected exported bytes.
5. Only after those checks, consider broader asset-level improvements such as
   more complete TXD/DFF variants.

No IMG v3 parser or RPF abstraction should be added as part of this phase.
