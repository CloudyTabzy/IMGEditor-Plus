# Inline small strings: `compact_str` 0.10.0

Decision and audit record for the `compact_str 0.8.2` → `0.10.0` upgrade and
inline-string audit (2026-11-27). Companion to
[allocator-rusty-alloc.md](allocator-rusty-alloc.md),
[hashmap-crate.md](hashmap-crate.md),
[rayon-parallelism.md](rayon-parallelism.md),
[memmap2-io.md](memmap2-io.md),
[rfd-tokio-runtime.md](rfd-tokio-runtime.md), and
[smallvec-inline-storage.md](smallvec-inline-storage.md).

## Version and feature decisions

- **Upgraded `compact_str` from `0.8.2` → `0.10.0`** (latest stable, MSRV 1.71;
  our MSRV is 1.96). `imgeditor` is the sole dependent in `Cargo.lock`, so the
  bump leaves zero duplicate versions and triggers no upstream rebuilds.
- **Key upstream changes gained across `0.9.0`–`0.10.0`:**
  - **Safety fixes:** resolved potential UB on `HeapString` reallocations,
    invalid UTF-8 reference creation in `retain`, and uninitialized spare
    capacity exposure in `as_mut_bytes` (replaced by `spare_capacity_mut()`).
  - **Dependency slimming:** replaced `ryu` with `zmij` for float formatting,
    removing `ryu v1.0.23` from `Cargo.lock`.
  - **Throughput & buffer reuse:** `CompactString::new` reuses owned `String`
    buffers when appropriate, faster inline creation, `append`, and
    `ToCompactString` specialization for `&str`.
- **Default features only:** Optional integrations (`serde`, `smallvec`,
  `bytes`, `garde`, `schemars`, `valuable`, `pyo3`, etc.) are unused here; the
  default `std` feature provides `CompactString`, `CompactString::const_new`,
  `CompactString::from_utf8_lossy`, `ToCompactString`, and `format_compact!`.

## Codebase audit and changes

### 1. Eliminated temporary heap `String`s in `EntryInfo::new` (`src/archive.rs`)
Although `EntryInfo`'s four string fields were already `CompactString`, every
archive open still allocated two short-lived heap `String`s per entry before
wrapping them:
- `infer_file_type(&file_name)` allocated `file_name.to_ascii_lowercase()`,
  repeating work already done for `file_name_lower`. Split out
  `infer_file_type_from_lower(&file_name_lower)` using
  `CompactString::const_new(...)` for all 12 static type labels and
  `format_compact!(".{ext} file")` for unknown extensions.
- `file_ext` allocated `ext.to_ascii_uppercase()` or `"FILE".to_string()`.
  Replaced with in-place `make_ascii_uppercase()` on an inline `CompactString`
  and `CompactString::const_new("FILE")`.
- Replaced `CompactString::new(file_name.to_lowercase())` with a direct inline
  lowercase (no redundant conversion), and switched `update_selected_list`'s
  normalized search query from `String` to inline `CompactString`.
- On `gta3.img` (`16,316` entries), this removes **32,632** temporary heap
  alloc/free pairs and one full case-folding pass per archive open.

### 2. ASCII-first `compact_to_lowercase` (`src/collections.rs`)
Benchmarking `IdeMap::build` after the initial switch exposed an instructive
implementation detail in `compact_str 0.10.0`:
`CompactString::from_str_to_lowercase` delegates to `convert_while_ascii`,
which processes ASCII in `2 * size_of::<usize>()` (**16-byte**) chunks and
falls through to char-by-char Unicode folding (`s.extend(c.to_lowercase())`)
for the remainder. Because most IMG entry names, GTA model names, and TXD names
are shorter than 16 bytes, `convert_while_ascii` ran zero iterations and sent
almost every short ASCII name down the slow per-character path.

Added `crate::collections::compact_to_lowercase(s: &str) -> CompactString`:
- Fast path (`s.is_ascii()`): single copy into `CompactString::new(s)` followed
  by in-place `out.make_ascii_lowercase()` directly in the 24-byte inline
  buffer (zero heap allocations, zero per-char repr branching).
- Fallback (`!s.is_ascii()`): `CompactString::from(s.to_lowercase())` (reuses
  the `String` allocation if `> 24 B`, inlines if `<= 24 B`), preserving exact
  `str::to_lowercase()` / `LowerStr` equivalence across all Unicode inputs —
  including avoiding a `compact_str 0.10.0` bug where `from_str_to_lowercase`
  misclassifies a word-final Greek `'Σ'` immediately following a 16-byte ASCII
  chunk.

Adopted across the hot lowercase sites (`src/archive.rs`,
`src/inspector/texture.rs`, `src/tasks.rs`) and replaced a redundant
`entry.file_name.to_lowercase()` in `src/ui/view.rs` with
`entry.file_name_lower.as_str()`.

### 3. Inline TXD texture names (`src/parser/txd.rs`, `src/parser/txd_writer.rs`)
- Switched `NativeTexture::{diffuse_name, alpha_name}` and
  `PlatformIndependentTexture::{name, mask}` from `String` to `CompactString`.
  Because `size_of::<CompactString>() == size_of::<String>() == 24 B`,
  `NativeTexture` stays at **128 bytes (+0 B)**.
- Updated `read_fixed_name` and `read_string_section` to construct
  `CompactString` directly (`std::str::from_utf8` fast path into
  `CompactString::new(text.trim())`, with `CompactString::from_utf8_lossy`
  fallback), removing intermediate `String` allocations on every texture parsed
  during compatibility scans, normalization, previews, and 3D loads.

### 4. Inline `IdeMap` entries and zero-alloc lookups (`src/inspector/texture.rs`)
- Switched `IdeMap::inner` from `HashMap<String, String>` to
  `HashMap<CompactString, CompactString>` (same 48 B per bucket slot; eliminates
  per-entry heap strings held for the session across GTA San Andreas `.ide`
  files).
- Switched `locate_nft` and `resolve_nft_path` candidate strings to
  `format_compact!`, and replaced per-file `n.to_lowercase() == *target` inside
  `find_file_recursive` with zero-allocation `n.eq_ignore_ascii_case(target)`.

### 5. Backup rename formatting (`src/ui/app.rs`)
- Replaced `CompactString::from(format!("{}.bak", entry.file_name))` with
  `format_compact!("{}.bak", entry.file_name)`.

## Candidates deliberately left as `String`

- **UI widget and dialog state (`src/ui/`):** Iced `Text`, `text_input`, logs,
  status lines, and Fluent-rendered messages expect owned `String` or hold
  arbitrarily long strings that would spill past 24 B anyway.
- **Filesystem paths (`PathBuf`), error messages, and `DffMesh` / `Scene`
  material labels:** Infrequent allocations or strings handed directly to
  Iced/wgpu/UI APIs that require `String`.

## Measured impact (`examples/benchmark_compact_str.rs`, release)

Corpora: `corpus\Gta_3_img\gta3.img` (`16,316` entries, `26,004` textures) and
`corpus\GTA San Andreas` (`.ide` tree). Windows, Rust 1.99.1,
`rusty_alloc 2.6.0`, `compact_str 0.10.0`, 3 warm runs each:

### Exact layout & inline counts (deterministic)

| Metric | Before (`0.8.2`) | After (`0.10.0`) | Delta |
|---|---:|---:|---|
| `size_of::<EntryInfo>()` | 184 B | **184 B** | **+0 B** |
| `size_of::<NativeTexture>()` | 128 B | **128 B** | **+0 B** |
| `EntryInfo` fields inline (`gta3.img`) | 65,264 / 65,264 | **65,264 / 65,264 (100.0%)** | **−32,632 temp heap `String`s/open** |
| `NativeTexture::diffuse_name` inline (`gta3.img`) | 0 / 26,004 (`String`) | **25,938 / 26,004 (99.75%)** | **−25,938 heap allocs/scan** |
| `NativeTexture::alpha_name` inline (`gta3.img`) | 0 / 1,733 (`String`) | **1,723 / 1,733 (99.42%)** | **−1,723 heap allocs/scan** |
| Total TXD name heap allocs removed per full scan | — | — | **−27,661 (99.73% inline)** |

### Wall-clock medians across 3 warm runs (ms)

| Workload | Before (`0.8.2`, runs / med) | After (`0.10.0`, runs / med) | Notes |
|---|---|---|---|
| `ArchiveInfo::open` (`gta3.img`, 16,316 entries) | 7.091 / 7.077 / 7.194 (**7.091**, min 6.730) | 5.580 / 5.352 / 5.748 (**5.580**, min **5.038**) | **−21.3% median (−25.1% min)**; −32,632 temp heap allocs + single lowercase pass |
| `scan_archive` (`gta3.img`, 26,004 textures) | 27.968 / 27.774 / 27.413 (**27.774**, min 26.774) | 26.991 / 25.722 / 28.861 (**26.991**, min **24.893**) | **~1–2 ms faster** (−7.0% min); −27,661 heap allocs/scan with 0 B struct growth |
| `IdeMap::build` (`GTA San Andreas` `.ide`s) | 9.092 / 9.152 / 9.136 (**9.136**, min 8.650) | 9.705 / 9.480 / 10.577 (**9.705**, min **9.240**) | Dominated by NTFS `read_dir` + `read_to_string`; `compact_to_lowercase` recovered ~0.6 ms from the `< 16`-byte fallback (`10.104` → `9.48–9.70 ms`) while eliminating session-lived heap strings |
