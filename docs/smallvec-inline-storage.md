# Inline small-buffer storage: `smallvec` 1.16.6

Decision and audit record for the `smallvec 1.15.2` → `1.16.6` upgrade and
inline-storage audit (2026-11-27). Companion to
[allocator-rusty-alloc.md](allocator-rusty-alloc.md),
[hashmap-crate.md](hashmap-crate.md),
[rayon-parallelism.md](rayon-parallelism.md),
[memmap2-io.md](memmap2-io.md), and
[rfd-tokio-runtime.md](rfd-tokio-runtime.md).

## Version and feature decisions

- **Upgraded `smallvec` from `1.15.2` → `1.16.6`** (latest stable `1.x`),
  unifying cleanly across all 13 dependent crates in `Cargo.lock` (`wgpu`,
  `wgpu-core`, `wgpu-hal`, `idna`, `icu_normalizer`, `fluent-bundle`,
  `harfrust`, `rustybuzz`, `kurbo`, `exr`, `parking_lot_core`,
  `wayland-backend`, `imgeditor`) with zero duplicate versions.
- **Why not `2.0.0-beta.5`:** Pre-release beta with breaking API changes
  (`SmallVec<T, N>`) that would duplicate `smallvec` alongside every `1.x`
  crate in the dependency graph.
- **Key upstream fixes gained in `1.16.0`–`1.16.6`:**
  - **Rust 1.93+ codegen fix (`rust-lang/rust#157743`):** On Rust 1.93+ (our
    MSRV is `1.96`, toolchain `1.99.1`), `SmallVec::new()` previously let GVN
    propagate `const <uninit>` into `ManuallyDrop`, causing LLVM to materialize
    a global constant and `memset` the inline buffer (and inflate debug stack
    frames). `1.16.6` fixes this via `SmallVecData::empty()`.
  - **Faster `push()` (`1.16.1`, `#557`)** via expanded `triple_mut`.
  - **Backported v2 `retain()` (`1.16.2`, `#596`)** and `may_dangle` safety fix.
- **Explicit features in `Cargo.toml`:**
  `smallvec = { version = "1.16.6", features = ["union", "const_generics", "const_new"] }`
  - `union` (also enabled by `wgpu-hal`): overlaps the inline buffer with
    `(*mut T, usize)` (16 B on 64-bit) instead of adding an enum tag word. Any
    `SmallVec<[T; N]>` with `size_of::<[T; N]>() <= 16` and `align_of::<T>() <= 8`
    is **24 bytes — identical to `Vec<T>`** with zero struct bloat.
  - `const_generics` (also enabled by `idna`) + `const_new`: allows arbitrary
    inline lengths (such as `SORT_CHAIN_MAX = 10`) and const construction.

## Codebase audit and changes

### 1. Removed `SmallVec` anti-pattern: `ArchiveInfo::selected_indices` (`src/archive.rs`)
- Previously `SmallVec<[usize; 8]>` (72 B inline). In practice, `selected_indices`
  holds **every visible row** of an open archive (`16,316` entries on `gta3.img`),
  so it spilled to the heap on every real archive while adding +48 B to
  `ArchiveInfo` and a `spilled()` branch on every row read and push.
- Replaced with `Vec<usize>`, pre-reserved to `matches.len()` in
  `update_selected_list`, and removed the redundant `.filter(|..| contains(""))`
  on empty queries.

### 2. Zero-allocation inline `SortChain` (`src/sort.rs`, `src/ui/app.rs`)
- `SortChain` is hard-capped at `SORT_CHAIN_MAX = 10` slots of 3-byte
  `SortPriority` values (`30 B` payload).
- Switched `SortChain::priorities` from `Vec<SortPriority>` to
  `SmallVec<[SortPriority; SORT_CHAIN_MAX]>` (`40 B` total under `union`):
  **never spills to the heap**, eliminates heap allocations in `Default`,
  `SortPreset::to_chain`, column-click handlers, and `SortChain::clone()`, and
  removes a pointer indirection from `SortChain::cmp`.
- Generalized `SortChain::new` to `impl IntoIterator<Item = SortPriority>` so
  callers pass stack arrays `[SortPriority::new(...)]` directly while keeping
  existing `Vec` callers source-compatible.
- Net effect on `ArchiveInfo` combining #1 and #2: **`832 B` → `800 B` (−32 B)**.

### 3. Zero-overhead inline NIF tables (`src/inspector/nif.rs`, `src/inspector/animation/bully.rs`)
Under `features = ["union"]`, every alias below is **24 bytes (`== size_of::<Vec<T>>`)**:
- `BlockRefList = SmallVec<[i32; 4]>` for `NiNodeData::{properties, children, effects, extra_data}`, `NiTriShapeData::{properties, extra_data}`, and `MaterialData::extra`.
- `MaterialNameList = SmallVec<[u32; 4]>` for `MaterialData::names`.
- `SkinWeightRow = SmallVec<[f32; 4]>` and `SkinBoneIndexRow = SmallVec<[u8; 4]>` for `SkinPartition::{weights, bone_indices}` (every Bully hardware-skinning partition stores `<= 4` weights per vertex).
- Replaced `read_ni_texturing_property`'s transient heap `vec![None; texture_count.max(12)]` with a stack `[Option<TexDesc>; 12]`.
- Switched `bully.rs::NifWalk::visit` to borrow `data.children.as_slice()` instead of cloning `children` at every scene-graph node during recursive traversal.
- `VertexSkin = SmallVec<[(u32, f32); 4]>` (`src/inspector/animation/model.rs`) stays at `40 B` under `union` and benefits automatically from `1.16.6`'s Rust 1.93+ `empty()` and `push()` improvements.

### 4. Stack mip-chain builder (`src/compat/encode.rs`)
- Switched `mip_chain_borrowed`'s transient outer list to
  `SmallVec<[(u32, u32, Cow<'_, [u8]>); 14]>` — textures up to `8192×8192` have
  at most 14 mip levels, eliminating 4 doubling reallocations per texture encode.

## Candidates deliberately left as `Vec`

- **`NativeTexture::mipmaps` (`parser/txd.rs`):** `parse_txd_headers` already
  leaves `mipmaps` empty (`Vec::new()`, 0 allocs). Each `MipmapLevel` is 32 B,
  and world TXDs commonly hold 4–9 mip levels; inlining 8 levels would bloat
  `NativeTexture` by +256 B, while inlining 1 level wastes 32 B on every
  mipmapped texture.
- **`EntryInspection::summary` / `txd_textures` (`parser/inspector.rs`):**
  Items are 48 B / 24 B and cached inside `Arc<EntryInspection>`; inlining
  would bloat cached structs by 200–500 B.
- **`AgrTrack::keys`, `DffMesh`, `ColEntry` geometry buffers:** Unbounded
  arrays of dozens to tens of thousands of items.

## Measured impact (`examples/benchmark_smallvec.rs`, release)

Corpora: `corpus\Gta_3_img\gta3.img` (16,316 entries) and 418 retail Bully NIFs
from `C:\Dev\bully-nif-tools\Nif_Files` (400 general + 18 skinned ped models).
Windows, Rust 1.99.1, `rusty_alloc 2.6.0`, `smallvec 1.16.6`, 3 warm runs:

### Exact layout & allocation counts (deterministic)

| Metric | Before | After | Delta |
|---|---:|---:|---|
| `size_of::<ArchiveInfo>()` | 832 B | **800 B** | **−32 B** |
| `size_of::<SortChain>()` | 24 B (+ heap) | **40 B (inline)** | **0 heap allocs ever** |
| `size_of::<NiNodeData>()` | 184 B | **184 B** | **+0 B** |
| `size_of::<SkinPartition>()` | 136 B | **136 B** | **+0 B** |
| `size_of::<VertexSkin>()` | 40 B | **40 B** | unchanged (`union`) |
| `SkinPartition` rows inline (418 NIFs) | 0 / 112,904 | **112,904 / 112,904 (100.0%)** | **−112,904 heap allocs** (~40% less table RAM) |
| Non-empty NIF ref lists inline (418 NIFs) | 0 / 8,517 | **7,951 / 8,517 (93.4%)** | **−7,951 heap allocs** |
| `NiTexturingProperty` temp `slots` heap allocs | 2,042 | **0 (100.0%)** | **−2,042 heap allocs** |
| **Total tiny heap alloc/free pairs removed (418 NIFs)** | — | — | **−122,897** |

### Wall-clock medians across 3 warm runs (ms)

| Workload | Before (runs / med) | After (runs / med) | Notes |
|---|---|---|---|
| `nif_parse` (418 files, 18 ped) | 69.164 / 81.478 / 79.082 (**79.082**, min 55.618) | 78.513 / 83.921 / 78.234 (**78.513**, min 53.410) | Within shared-CPU noise; `rusty_alloc 2.6.0` serves tiny allocs in ~4 ns, so the main win is −122,897 allocs and ~40% smaller contiguous `SkinPartition` weight/bone tables |
| `gta3` full multi-key sort (16,316 entries) | 5.386 / 8.002 / 7.755 (**7.755**, min 5.249) | 7.871 / 8.479 / 7.783 (**7.871**, min 5.404) | Within noise (comparison-bound) |
| `gta3` filtered sort (`"car"`) | 0.385 / 0.488 / 0.509 (**0.488**, min 0.368) | 0.554 / 0.530 / 0.393 (**0.530**, min 0.366) | Within noise |
