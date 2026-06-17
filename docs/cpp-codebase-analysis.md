# Original C++ IMGEditor Codebase Analysis

This document records a source-level review of the original C++ [IMGEditor](https://github.com/user-grinch/IMGEditor) implementation. The goal was to verify the claims made in the Rust port's `README.md` comparison table and to avoid exaggerating the original code's problems.

## Scope

Files reviewed:

- `src/editor.cpp`, `src/editor.h`
- `src/imgarchive.cpp`, `src/imgarchive.h`
- `src/parser/pc_v1.cpp`, `src/parser/pc_v2.cpp`, `src/parser/iparser.h`
- `src/windialogs.cpp`
- `src/updater.cpp`
- `src/widget.cpp`, `src/utils.cpp`
- `include/ui/application.cpp`, `include/ui/renderer.cpp`

## 1. Null Pointers / Use-After-Free

**Verdict: Present and reproducible from the source.**

The C++ code relies heavily on raw pointers and does not always keep their lifetimes in sync with the containers that own the objects.

### Concrete findings

| Location | Issue |
|----------|-------|
| `imgarchive.h:62` + `imgarchive.cpp:31-50` | `SelectedList` stores `EntryInfo*` pointers into `EntryList`, which is a `std::vector<EntryInfo>`. Any operation that adds or removes entries can reallocate `EntryList`, invalidating those pointers. |
| `editor.cpp:364` | The UI renders `archive.SelectedList[i]` while also reading `archive.EntryList[i].bSelected`. Because the indices are unrelated, the wrong entry's selection state is displayed. |
| `editor.cpp:374` | Rename `InputText` writes into `pEntry->FileName` (`wchar_t[24]`) from a 24-byte buffer without an explicit length check; overflow is possible for long names. |
| `editor.cpp:690, 706, 738, 748` | `ArchiveInfo` is heap-allocated and passed to `CreateThread`. It contains a raw `IMGArchive*` that the worker thread accesses while the UI thread may close or delete the archive. |
| `imgarchive.cpp:200-203` | `GetFormatText` unconditionally `reinterpret_cast`s `Parser` to `ParserPCv1*`, even when the active parser is `ParserPCv2` or `UnknownFMT`. This is undefined behavior that happens to work only because the three classes place `GetVersionText` at the same vtable offset. |
| `editor.cpp:147, 149, 151, 164, 166, ...` | Many menu handlers and hotkey handlers dereference `pSelectedArchive` without a null check, even though it is reset to `nullptr` at the start of each frame. |
| `editor.cpp:539-541` | `CloseArchive(pSelectedArchive)` is called when the close-tab hotkey is pressed; if no archive is selected, a null pointer is passed into `CloseArchive`. |
| `editor.cpp:269, 738, 748` | Raw `new ArchiveInfo{...}` is passed to `CreateThread`. If `CreateThread` fails, the memory leaks. |

### Why Rust helps

Rust's ownership and borrow checker would catch the `SelectedList` invalidation pattern at compile time: `EntryList` cannot be mutated while borrowed pointers to its elements exist, and `Vec` reallocation would be visible in the type system. The raw `ArchiveInfo` lifetime across threads would also require `Send`/`Sync` and explicit synchronization or ownership transfer.

## 2. UI Thread Blocking on I/O

**Verdict: Partially true, but not universal.**

The README comparison table implies that all I/O blocked the UI thread in the original C++ editor. The source shows a more nuanced picture.

### Operations that already run off the UI thread

| Operation | Mechanism | Location |
|-----------|-----------|----------|
| Save | `CreateThread` worker | `editor.cpp:690` |
| Save As | `CreateThread` worker | `editor.cpp:708` |
| Export all | `CreateThread` worker | `editor.cpp:739` |
| Export selected | `CreateThread` worker | `editor.cpp:749` |
| Context-menu Export | `CreateThread` worker | `editor.cpp:270` |
| Update checker | `CreateThread` worker | `editor.cpp:632` |

### Operations that still block the UI thread

| Operation | Location | Impact |
|-----------|----------|--------|
| Open archive | `Editor::OpenArchive` → `IMGArchive` ctor → parser `Open` | Reads `.dir`/`.img` headers synchronously; blocks until complete. |
| Import files | `Editor::ImportFiles` / `ImportAndReplaceFiles` → `IMGArchive::ImportEntries` | Runs directly on the UI thread (`editor.cpp:718-730`); blocks while files are scanned and added. |
| Pre-save validation | `Editor::SaveArchive` checks `std::filesystem::exists` synchronously before spawning the worker | Small but real blocking call. |

### Correct way to describe this

The original C++ editor already offloads save and export to worker threads, so it is not accurate to say the UI thread blocked on *all* I/O. The real remaining blockers are **Open** and **Import**, which the Rust port moves to Tokio's async runtime. The comparison table should be read with that caveat in mind.

## 3. Other README Claims

| Claim | Verdict | Notes |
|-------|---------|-------|
| Vendored C++ libs replaced by Cargo | **True** | C++ vendored ImGui, FreeType, GLFW, GLEW, GLM and used Direct3D 9. Rust uses Iced, Tokio, rfd, etc. |
| Memory corruption in format parsers fixed by `Result` | **Mostly true** | Parsers use streams and exceptions, but `GetFormatText` has a type-punning bug and reads lack explicit bounds checks. Rust's `Result` and slice bounds are a real improvement. |
| Slow exports fixed by rayon + memmap2 | **Cannot verify from this source** | C++ export allocates a `std::vector<char>` for every entry, which is inefficient. Whether rayon + memmap2 materially outperforms the threaded C++ implementation depends on benchmarking. |

## Conclusion

The Rust port fixes real, source-level bugs in the C++ implementation, especially around pointer invalidation and unchecked dereferences. The "UI thread blocking on I/O" claim, however, is overstated as written: save and export were already threaded. The README comparison table has been updated to point readers to this document for the full, nuanced analysis.
