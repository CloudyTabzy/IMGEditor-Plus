# IMG v1/v2 validation notes

Status: parser-hardening and local-corpus validation record
Checked: 2026-09-12
Scope: PC IMG v1/v2 and the Bully Xbox 360 big-endian IMG v1 variant; IMG v3
and RPF are intentionally out of scope.

**Update (2026-09-12):** the parser-hardening pass was checked against the
supplied GTA III, Vice City, and San Andreas corpora. The optional tests now
open v1 archives from either side of a `.img`/`.dir` pair, parse representative
RenderWare and collision assets, rebuild representative v1 data, and reopen the
result. The supplied files are useful compatibility evidence, but they are not
treated as proof of every retail release or of in-game compatibility.

## Why this document exists

IMG Editor Plus was derived from the original C++ IMG Editor. This document
records the external format knowledge, implementation comparisons, malformed
input defenses, and local corpus evidence used to harden the Rust parser. Real
gameplay compatibility still requires testing a rebuilt copy in the matching
game build.

The request that mentioned “GTA II” is kept distinct from GTA III here. The
existing code, the original IMG Editor, and the consulted IMG references all
identify the early 3D IMG family as GTA III, Vice City, and San Andreas. No
IMG-compatible GTA II layout was established during this audit, so a GTA II
sample must not be routed through the v1/v2 parser without separate format
research and fixtures.

The supplied `Gta_3_img\gta3.img` at the corpus root is intentionally excluded
from validation because it has no sibling `.dir` file. Its size and contents
cannot establish a complete paired GTA III archive; it may be incomplete,
modified, or mislabeled. The paired files under `Gta_3_img\models` are the
ones used by the tests.

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
each range against the `.img` length before mapping the data file. The Xbox 360
big-endian variant uses the same v1 validation, while the v2 path applies its
own record-aware validation described below.

### Direct `.dir` input (implemented)

All open, detection, scan, and drag-and-drop paths canonicalize a selected
`.dir` to its sibling `.img` before dispatch. The `.dir` file remains the
directory half used by the v1 parser; the canonical `.img` path is stored as
the archive identity and is used for data mapping, cache keys, saves, and
deduplication. The file picker accepts both extensions, and saving a v1
archive writes the paired `.img` and `.dir` files.

The direct-directory path is covered by synthetic tests and by the optional
GTA III/Vice City corpus test. This matches the local C# reference behavior
without making the UI expose two tabs for one physical archive.

For the Xbox 360 layout, see the implemented big-endian parser in
[`xbox360.rs`](../src/parser/xbox360.rs) and the deliberately
uncompressed-only boundary tracked in `TODO.md`.

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
end of the entry table. The supplied San Andreas corpus confirms that PC
archives begin data at the next 2048-byte boundary after the table:
`ceil((8 + entry_count * 32) / 2048) * 2048`. The Rust writer uses that compact
layout now; it deliberately no longer inherits the original C++ writer's
unnecessary `0x300000` reservation.

### Rust implementation and validation

The Rust implementation correctly recognizes `VER2`, reads the entry count,
uses 32-byte records, preserves 24-byte names, and interprets offsets in
2048-byte sectors. This is visible in [`PcV2Parser::open`](../src/parser/pc_v2.rs)
and the shared constants in [`parser/mod.rs`](../src/parser/mod.rs).

`EntryInfo` now retains an `ImgV2Size` pair for each v2 entry while its generic
`sector` field holds the checked effective count. This preserves uncommon
fallback records (`streaming_size == 0`) through a save/rebuild without making
all readers v2-specific.

The open path validates the complete directory before publishing any entries:

- checked header/table arithmetic and a table that fits in the file;
- printable names and at least one named record (while retaining record order
  and raw name bytes);
- checked sector-to-byte conversion and every effective data range against the
  IMG length;
- non-empty data cannot point into the directory region;
- non-empty data ranges cannot overlap another v2 entry;
- allocation failures from implausibly large directory counts return an error
  rather than attempting an unchecked reservation.

Synthetic regression tests cover truncated tables, out-of-range data,
directory-overlapping data, overlapping ranges, malformed names, fallback-size
records, oversized writes, zero-length records, raw-size preservation, and
compact rebuilding. Shared mmap readers also reject a source that became
shorter after opening instead of silently clamping an entry to the new EOF.
An optional local-corpus test opens `cutscene.img`, `gta_int.img`, `gta3.img`,
and `player.img` when `IMGEDITOR_CORPUS_ROOT` is set.

## Asset-level findings from the local implementations

The IMG container is only the outer table. Real GTA archives contain several
independent RenderWare formats, so a successful IMG parse does not imply that
every entry is renderable.

### GTA III/Vice City DFF

The local `librw` reader and DragonFF implementation agree on an important
historical edge case: a GTA III geometry stream may advertise vertex and normal
arrays through the morph target while the geometry flags omit the usual
position bit. The Rust DFF reader now follows the morph target's
`has_vertices`/`has_normals` fields when consuming those arrays. Using only the
geometry flags shifts the stream cursor and produces incomplete or distorted
meshes. A synthetic regression fixture covers this case, and representative
DFF entries from the supplied GTA III-labelled and Vice City corpora are
parsed during the optional corpus test.

### Collision COL1/COL2/COL3/COL4

The collision parser now recognizes the legacy `COLL` header and the later
`COL2`, `COL3`, and `COL4` records. The hardening details are important:

- each entry starts with a fourcc and body-size word, followed by the 24-byte
  name/id record and a 40-byte bounds record; the body-size word counts bytes
  after the first eight header bytes;
- COL2+ metadata supplies counts and relative offsets, and the offsets point
  four bytes before the payload in the RenderWare/librw layout;
- compressed collision vertices are signed 16-bit coordinates divided by
  128.0, and triangle records use three 16-bit indices plus material/lighting
  bytes;
- sphere and box records, face-group bounds, and COL3/COL4 shadow faces are
  retained alongside the triangle mesh rather than being discarded;
- the embedded viewer converts the triangle mesh directly and tessellates
  collision spheres and boxes into bounded, untextured preview meshes. Shadow
  meshes are shown as a separate scene mesh when present. This reuses the
  normal scene cache, Z-up conversion, camera framing, wire overlay, and wgpu
  validation used by DFF/NIF previews.

The optional San Andreas corpus test parses representative COL2/COL3 entries
from the supplied `gta3.img` and `gta_int.img` archives, including renderable
collision triangles. A real Vice City `airport.col` is also covered by the
optional scene decoder and headless wgpu fixture. The parser and preview
builder use checked counts, offsets, lengths, and bounded primitive
tessellation so malformed or unusually large collision data cannot trigger an
unchecked allocation or GPU upload.

## Implementation comparison

| Behavior | Original C++ | Local C# reference | Rust port | Assessment |
|---|---|---|---|---|
| Sector size | 2048 | 2048 | 2048 | Match |
| v1 directory size | 32-byte records to EOF | 32-byte records to EOF | 32-byte records, rejects partial tail | Compatible, Rust is stricter |
| v1 data location | paired `.img` | paired `.img` | paired `.img` for `.img` or `.dir` input | Match |
| v2 marker/count | `VER2` + `u32` count | `VER2` + `u32` count | `VER2` + `u32` count | Match |
| v2 size words | flattened 4-byte field | flattened 4-byte field | separate raw `u16` words plus checked effective size | Rust corrects inherited ambiguity |
| Name storage | 24 bytes | 24 bytes | 24 raw bytes + display string | Match |
| Malformed range checks | minimal | sorted/order checks, some structural checks | checked table/name/range validation, non-overlap, and source-length checks before mapping | Rust is stricter and fails early |
| `.dir` as input | not supported by original UI path | explicitly supported | canonicalized at UI, detection, scan, and parser boundaries | Match |
| Archive mutation previews | no preview cache | no Rust scene cache | generation + texture cache + app scene cache | Move path now invalidates both sides |

The original C++ and local C# implementations flatten the size words. Their
agreement is an implementation inheritance point, not a format specification;
the retained pair is necessary for faithful v2 round trips.

## Supplied corpus results

The following files were present under `C:\Dev\IMGEditor-master` on
2026-09-12. The sizes and entry counts were collected read-only; the archives
are not copied into this repository.

| Corpus | IMG form | IMG bytes | DIR bytes | Entries | Observed extensions |
|---|---|---:|---:|---:|---|
| `Gta_3_img/models/gta3` | v1 pair | 170,891,264 | 123,392 | 3,856 | DFF 3,138; TXD 718 |
| `Gta_3_img/models/txd` | v1 pair | 331,290,624 | 22,944 | 717 | TXD 717 |
| `Grand Theft Auto Vice City/models/gta3` | v1 pair | 327,487,488 | 193,376 | 6,043 | DFF 4,617; TXD 1,368; COL 30; IFP 28 |
| `Grand Theft Auto Vice City/anim/cuts` | v1 pair | 115,083,264 | 4,736 | 148 | IFP 76; DAT 72 |
| `GTA San Andreas/models/gta3.img` | v2 | 937,680,896 | — | 16,297 | v2 open/range validation |
| `GTA San Andreas/models/gta_int.img` | v2 | 150,024,192 | — | 2,484 | v2 open/range validation; COL2/COL3 representatives |
| `GTA San Andreas/models/player.img` | v2 | 66,738,176 | — | 542 | v2 open/range validation |
| `GTA San Andreas/models/cutscene.img` | v2 | 26,947,584 | — | 634 | v2 open/range validation |
| `Bully script img xbox 360/Scripts` | Xbox 360 big-endian v1 pair | 5,281,792 | 16,896 | 528 | endian/range validation; representative save/reopen |

The optional tests enabled by `IMGEDITOR_CORPUS_ROOT` validate all four v1
pairs from both `.img` and `.dir` entry points, read representative DFF/TXD/
COL/IFP records where present, and save/reopen representative v1 entries while
comparing their raw bytes and names. The v2 corpus test opens the four listed
San Andreas archives and checks their directory/range structure. The separate
COL corpus test exercises representative COL2/COL3 records from the San
Andreas model archives. The Xbox 360 corpus test detects and opens the supplied
big-endian `Scripts.img`/`Scripts.dir`, then verifies representative LUR data
survives a format-aware save/reopen.

Run the local corpus checks with:

```powershell
$env:IMGEDITOR_CORPUS_ROOT = 'C:\Dev\IMGEditor-master'
cargo test -j 2
```

These are structural and byte-round-trip checks, not gameplay tests. No game
executable was launched here, and no rebuilt archive should be considered
gameplay-safe until it has been tested in the matching game build. For clean
release evidence, use a legally obtained install and keep only a local,
untracked hash manifest.

## Real-archive validation plan

When legally obtained samples are available, keep the binary archives outside
the repository. Commit only small metadata manifests, synthetic fixtures, or
tests that do not redistribute copyrighted game data.

For each PC release of GTA III, Vice City, and San Andreas:

1. Record the title, release/build, archive path, file sizes, and a SHA-256
   hash in a local, untracked manifest.
2. Detect the format without trusting the filename: v1 requires a valid pair
   (either half may be selected); v2 requires `VER2` and a table that fits
   inside the file.
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
   record, out-of-range offset, overlapping ranges, oversized count, and a v2
   record with distinct streaming/archive size words. The application should
   return a readable error and never panic or silently present truncated data.
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

### Remaining validation and compatibility work

1. Test rebuilt San Andreas archives in the matching game build before making
   broader gameplay-compatibility claims.
2. Compare first-record metadata and representative exports with an independent
   reader on legally obtained, clean installs; keep hashes in an untracked
   manifest rather than committing game data.
3. Add focused fixtures for unusual TXD mipmaps/palettes, native platform
   streams, DFF skin/HAnim data, and collision primitive rendering as those
   formats become part of the product scope.
4. Keep native PS2/Xbox/GameCube/PSP IMG layouts and IMG v3 out of this phase;
   they need separate format research and fixtures.

No IMG v3 parser or RPF abstraction should be added as part of this phase.
