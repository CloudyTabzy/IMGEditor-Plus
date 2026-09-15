# Entry-list export and comparison

IMGEditor Plus now adapts the entry-list feature from Alci's IMG Editor 1.5.
The reference behavior was recovered in
alci_imgeditor/compare-feature-spec.md. This document records the Rust
implementation contract, the intentional improvements, and the boundaries
that should remain stable when the feature evolves.

## User workflow

The File menu contains two actions:

- Export as list (Ctrl+L) writes the currently open archive's entry names.
- Compare with list (Ctrl+P) reads a previously exported list and opens a
  report for the selected archive.

The native dialogs remember the last directory used by either action in the
last_compare_folder setting. Export and comparison are disabled gracefully
with a toast when no archive is open. A comparison can be closed while the
worker is reading; its late result is ignored.
An empty archive has no comparison target and is rejected with a guidance toast.

## Compatible manifest format

The manifest is intentionally not a new archive format. It is plain text with
one entry name per line:

- names are emitted in the archive's raw storage order;
- UTF-8 is used for new exports;
- CRLF separates names;
- the final name has no trailing line ending;
- no header, count, checksum, or comment syntax is added.

The no-header choice keeps files interchangeable with Alci's list exports and
with simple text tools. The reference used the system text codec, but the
original GTA names are normally ASCII and UTF-8 is a safe, deterministic
modernization. Existing lists containing LF, CRLF, or lone CR line endings are
accepted.

On input, a UTF-8 BOM is ignored only at the beginning of the first line.
Every line is Unicode-trimmed. Blank and whitespace-only lines are ignored.
Lines beginning with # remain ordinary names because that is what the
reference tool did; treating them as comments would silently change results.
An unterminated final line is processed.

The reader rejects invalid UTF-8 and files larger than 64 MiB before parsing.
The limit is a resource-safety guard for a user-selected file, not a format
limit on IMG archives.

## Comparison semantics

The primary result remains the reference tool's one-directional containment
check:

1. Build a set of names from the archive.
2. Walk non-empty manifest lines in their original order.
3. Append each name not in the archive to the missing list.

Matching is exact and case-sensitive by default. Missing names intentionally
retain duplicate manifest lines and manifest order, preserving the old
dialog's observable behavior. Archive names are taken from the raw entries
vector, not the filtered or sorted table.

The report also computes non-breaking diagnostics:

- archive record count;
- non-empty manifest line count;
- distinct manifest-name count under the active case policy;
- matched manifest-line count;
- unique missing-name count;
- duplicate manifest-line count;
- archive-only names in archive storage order.

These diagnostics do not change the default missing list. They make a
duplicate or incomplete list explainable instead of reducing the result to an
all-or-nothing message.

## Intentional quality-of-life additions

The report modal adds:

- a summary with matched, missing, unique-missing, and archive counts;
- an opt-in Case-sensitive matching toggle (enabled initially);
- an opt-in archive-only section for names present in the archive but absent
  from the manifest;
- duplicate and ignored-blank-line counts;
- a Copy missing names action using CRLF output;
- a bounded scrollable result area so a large failure cannot make the modal
  exceed a small window;
- a loading spinner while the file is read and compared off the UI thread.

The optional case-insensitive mode normalizes comparison keys with Unicode
lowercase conversion. It does not rewrite the displayed names or the archive.
The optional archive-only report is deduplicated because it is a diagnostic
view, while the reference-compatible missing list is not.

The comparison task snapshots archive names, archive identity, and the
archive generation before it leaves the UI thread. Each request also receives
a unique request id. A completion is accepted only when the request id,
archive index, file name, path, generation, and manifest path still match.
This prevents a result from being displayed after a rename, import, delete,
cross-archive move, tab close, or other mutation. The request id also prevents
a canceled worker from overwriting a replacement comparison targeting the
same archive and manifest path. The generation guard complements the existing
scene and texture cache invalidation rules.

## Implementation map

- src/compare.rs: UTF-8 parsing, compatible encoding, comparison, limits, and
  unit tests.
- src/ui/dialogs.rs: native manifest open/save dialogs and remembered folder
  support.
- src/ui/keymap.rs: Ctrl+L and Ctrl+P shortcuts.
- src/ui/app.rs: messages, asynchronous workers, snapshots, stale-result
  rejection, modal lifecycle, clipboard action, and menu entries.
- src/ui/view.rs: bounded, theme-aware loading and result modal.
- src/config.rs: persisted last_compare_folder setting.

No content, size, CRC, or hash comparison is performed. Such a mode would
need an explicit versioned manifest format and carefully defined source-byte
semantics, so it remains a future feature rather than an accidental extension
of this name-only contract.

## Validation

The core tests cover:

- LF, CRLF, lone CR, and unterminated final lines;
- BOM and Unicode whitespace trimming;
- blank-line handling and literal hash-prefixed names;
- CRLF export with no final newline;
- exact and opt-in case-insensitive matching;
- duplicate missing lines and duplicate counts;
- archive-only ordering and deduplication;
- invalid UTF-8 and file reading;
- app-level completion installation and stale generation rejection.

Manual Windows coverage should verify the two native picker starting
directories, saving an empty or large list, resizing the modal in a minimized
window, copying missing names, and closing the report while a comparison is
still loading.
