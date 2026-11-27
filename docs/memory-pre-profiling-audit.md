# Memory pre-profiling audit (structural pass, unmeasured)

Date: 2026-09-30

## Status: read-only, nothing benchmarked

This is a **pre-profiling** record. No build, no benchmark, no profiler run
produced any number here. Every quantity below is **derived by reading the
source** — struct field lists, `size_of` arithmetic, loop bounds, and the
existing measurements already recorded in
[allocator-rusty-alloc.md](allocator-rusty-alloc.md). That distinction is the
organizing principle of this document:

- **Tier 1** — waste that is provable from the code. The *existence* of the
  problem does not depend on a measurement, only the *size* of the win might.
- **Tier 2** — real but unsized. Each needs one counter, and every counter is
  either an existing struct field or a one-line loop. Listed with the exact
  expression to run.
- **Tier 3** — already correct. Recorded so a later pass does not "optimize"
  them back into a problem.

The companion measured record is
[allocator-rusty-alloc.md](allocator-rusty-alloc.md) §"Codebase memory map",
which is where this pass was queued: that section ends "so a structural pass can
start from measurements instead of guesses." This document is the structural
pass, and its output is the measurement list in Tier 2.

## The one unverified assumption

Almost all arithmetic below is exact. The exception, flagged wherever it is
used, is the **6 indices per vertex** ratio used to size findings 1, 2 and 4.
It is a RenderWare-typical figure, not a measured one. Tier 2 item 2 measures
it.

---

## Dependency candidates evaluated (all rejected)

The audit was requested as a search for memory-efficient crates. Five
candidates were evaluated against the current tree; **none was added**.

| Candidate | Outcome | Reason |
|---|---|---|
| `smallvec` 1.16.6 | upgraded & audited (2026-11-27) | Upgraded `1.15.2` → `1.16.6` with explicit `union`, `const_generics`, `const_new` (fixing Rust 1.93+ `SmallVec::new()` codegen). Replaced the `archive.rs` `selected_indices` anti-pattern with `Vec<usize>`, moved `SortChain` inline (0 heap allocs), and inlined NIF `SkinPartition` rows and node/shape ref lists at 24 B (0 struct growth, −122,897 heap allocs across 418 retail Bully NIFs). See [smallvec-inline-storage.md](smallvec-inline-storage.md). |
| `compact_str` 0.9.1 | already present and already optimal | `Cargo.toml` pins `0.8`; used for all four `EntryInfo` name fields. `MAX_ENTRY_NAME_BYTES = 24` (`parser/mod.rs:35`) caps names at 23 chars, and `CompactString` inlines up to `size_of::<String>()` = 24 B — so **every entry name is already heap-allocation-free**. `MAX_SIZE` is 24 in both 0.8.2 and 0.9.1, so the bump is zero-delta, and 0.x minors are breaking for cargo. |
| `crossbeam-epoch` 0.9.21 | deliberately absent | Zero references in `src/`. Present in `Cargo.lock` only transitively via `crossbeam-deque` (rayon's scheduler). `AGENTS.md` records the audited removal. |
| `patricia_tree` (sile) | rejected on query class | The entry search is a scored **subsequence** match plus a Jaro-Winkler typo tier (`search.rs`), which explicitly rewards non-prefix matches. A Patricia trie answers *prefix* queries and cannot compute a gap/word-boundary score. The name-keyed maps that do exist are small exact-match lookups, where a trie is worse than a `HashMap`. Crate is healthy (0.10.2, 2026-09) — rejected on fit, not maintenance. |
| `sdd` (delayed memory reclaimer) | rejected as strictly more niche than `crossbeam-epoch` | It is *only* a reclamation primitive, so it has no use until a lock-free collection with read-during-remove already exists. The app's concurrency is refcount-based (`Arc`, `OnceLock`, `LazyLock`) and lock-based (`Mutex`, `RwLock`); atomics are used as flags, never node ownership. |
| `stumpalo` (bump allocator) | rejected on lifetime mismatch | The dominant memory is **individually evicted, `Arc`-shared, weighted-LRU** entries — the texture cache evicts one group at a time by weight, and `scene_cache` has per-archive `retain`. A bump arena frees everything at once, so it cannot address either. Also see the note on the "reuse decode scratch buffers" lever below. |

### Consequence for the "reuse decode scratch buffers" lever

`allocator-rusty-alloc.md` §"Codebase memory map" names three structural
levers: lower desktop cache caps, stream decode→GPU without holding the CPU
copy, and reuse decode scratch buffers. This pass **retires the third one**:
`parser/texture_decoder.rs` has no large transient scratch to pool. Its block
decoders write straight into the output buffer and `block_px` is a 64-byte
stack array. The waste on the texture path is in *copies that are made*, which
is fixed by not making them, not by recycling them.

---

## Tier 1 — Structural waste, provable from source

Ordered by impact ÷ risk. "Verified" means the agent that reported it and the
author independently re-read the cited lines.

### 1. The scene cache charges GPU-only memory against the CPU budget

`Scene::estimated_gpu_bytes()` (`scene3d/scene.rs`) sums
`vertices + indices + wire_indices + texture`, where `wire_indices = 2 × indices`
— the line-list buffer that exists **only on the GPU**
(`scene3d/pipeline.rs:291-305`). `SceneCpuWeight::weight` (`ui/app.rs:1560`)
calls that same function to weight the **CPU-side** LRU.

So the estimate is `32V + 12I + 4WH` while actual CPU residency is
`32V + 4I + 4WH`. At 6 indices/vertex that is **104 B/v charged against 56 B/v
resident — a 1.86× over-count**, meaning the documented 256 MiB scene cache cap
holds roughly 138 MiB of real scene memory and ~118 MiB of the budget is
unreachable.

**Verified.** Fix is small: give `Scene` a separate `estimated_cpu_bytes()` that
omits the wire term, and use it for `SceneCpuWeight` while leaving
`estimated_gpu_bytes()` for the GPU admission check. Do not lower the admission
cap on the strength of this — the *admission* estimate should stay
conservative.

This is worth doing first regardless of the memory outcome, because until the
weight is honest, no decision about whether to raise or lower the cap is
informed. Secondary defect: `unwrap_or(0).max(1)` at `app.rs:1561` maps an
overflowing scene to a weight of **1 byte**, i.e. effectively uncached but never
evicted. Unreachable today because `validate_scene_data` rejects such a scene
before upload, but the mapping is wrong in principle.

### 2. Scene decode holds three live copies of the vertex data

`build_scene_from_dff` takes `&[DffMesh]` (`scene3d/decode.rs:307`) but
`.clone()`s positions, normals, uvs and indices into a temporary `MeshData`
(`decode.rs:324-331`), then clones `indices` a **second** time
(`decode.rs:530`) and clones `name`/`texture_name` twice more. The caller keeps
`dff_meshes` alive to the end of the closure. At the worst instant, per vertex:
32 B of source + 32 B of `MeshData` + 32 B of the retained `Vec<Vertex>`, with
the index array present three times over.

`parse_and_build_scene_from_dff` (`decode.rs:363`) already *owns* the `Vec`, so
the fix is to make `build_scene_from_dff` take `Vec<DffMesh>` by value and
iterate with `into_iter()`, destructuring each `DffMesh` so the source vectors
are dropped as they are consumed. Only four call sites exist. **Peak 3× → 2×**,
per-mesh transient → 0.

### 3. Texture decode eagerly decodes every texture in a TXD; the view shows one

`ui/app.rs:8105` loops `for tex in &txd.textures` and calls `decode_rgba()` on
each, pushing all of them into the cached `Vec<DecodedTexture>`. The texture tab
then displays exactly one: `view.rs:1741-1742` (`&textures[tex_idx]`).
Selecting a different texture (`app.rs:6656`) only sets an index — no decode
happens.

**Verified.** Cost is exactly `4wh` retained per texture, bounded only by the
128 MiB cap (`archive.rs:23`) and the 256-item cap. For a 19-texture DXT1 TXD
that is ~19 MiB retained to draw 1 MiB.

Fix: make the cache value a lazy record (source bytes or an mmap slice plus
*metadata only*), and decode on `Message::TextureSelect` into a second cache
keyed by `(entry_index, texture_index)`. `quick_cache` is already a direct dep
and the `get_value_or_guard` single-flight pattern at `app.rs:8084` transfers
directly. A cheaper first step that needs no new type: keep the eager decode
but stop at a per-entry byte budget and log what was skipped.

### 4. The base mip level is cloned unconditionally in every conversion *(fixed 2026-11-27)*

`compat/encode.rs` previously cloned level 0 (`rgba[..w*h*4].to_vec()`) in
`mip_chain` for every format, keeping a 4 MiB duplicate alive at 1024×1024.
`encode_texture` now uses `mip_chain_borrowed` (`Cow::Borrowed` for level 0,
`Cow::Owned` for the downsampled tail) and `palette::map_levels_borrowed`,
removing 4 MiB from the peak of every conversion while keeping `pub fn mip_chain`
unchanged.

### 5. PAL8 conversion holds three full-pixel vectors, and feeds k-means 4× more samples than it can use *(partially fixed 2026-11-27: `visible` is now pre-sized with `Vec::with_capacity`; stride-sampling / `.max_samples(n)` remains open)*

`compat/palette.rs:66-70` collects every opaque pixel into
`Vec<Srgb<u8>>` — 3 MiB at 1024². Because it is a `filter().map().collect()`,
`filter` erases the `size_hint`, so it grows by repeated doubling with a ~1.5×
peak overshoot. That slice is then converted wholesale to Oklab at
`compat/convert.rs:317` → `Vec<Oklab>` = **12 MiB at 1024²**.

Three full-pixel vectors are therefore live at once: `rgba` (4 MiB), `visible`
(3 MiB), `Vec<Oklab>` (12 MiB).

But k-means **cannot use more than `max_samples = 512 * 512 = 262,144`**
(quantette `kmeans.rs:77`, applied at `kmeans.rs:174-177`) — it samples random
indices into the input — and the Wu histogram is an 8,192-bin grid. Passing
1,048,576 pixels buys nothing.

Two independent local fixes: give the `collect()` a real `with_capacity`, and
feed quantette a stride-sampled list of ≤ 262,144 entries. `visible` drops to
768 KiB and the Oklab vector to 3 MiB, with no change to the resulting palette.
The crate exposes `.max_samples(n)`, so the second is a builder call.

### 6. The animation dock rebuilds its whole clip picker every redraw

`view.rs:1016` calls `build_animation_dock()` unconditionally in the Model3D
view body. Iced rebuilds the view tree every redraw, and `app.rs:8459-8463`
installs a `window::frames()` subscription whenever the transport is playing —
so during playback this runs at vsync.

Per redraw it allocates: `clip_names: Vec<String>` with a `.clone()` **per clip**
(`view.rs:1178`), `clip_map: HashMap<String, ClipId>` with a second clone per
clip (`view.rs:1179-1183`), plus `Arc::new`, `ifp_map`, `model_map`, and
`to_ascii_lowercase` calls inside the model sort comparator
(`view.rs:1270-1272`) — two allocations per comparison.

**Verified.** Fix: build the clip/ifp/model name vectors and lookup maps once,
store them on `AgrPlayback`, and clone the `Vec`/`Arc` (1-2 allocations) per
redraw. Add a `library_generation` counter on the session so a stale cache is
detectable rather than a silent correctness hazard, and sort the model picker
once at load with `sort_by_cached_key`.

Related, same subsystem: `mod.rs:89-101` `PlaybackCapability::badge()` returns
`String` via `.to_string()` on a `&'static str` literal for five of six arms,
called once per redraw at `view.rs:1049` and cloned again at `view.rs:1109`.
Returning `&'static str` removes a guaranteed per-frame allocation.

### 7. Full-size RGBA is retained twice, once per texture, forever

`view.rs:2011`:
`Handle::from_rgba(tex.width, tex.height, tex.rgba.clone())`. iced's
`Handle::Rgba` holds `Bytes`, and `Vec<u8> → Bytes` is zero-copy, so this
`.clone()` is a **second full `4wh` buffer** that lives as long as the cached
`DecodedTexture` — the `OnceLock<Handle>` is per-texture, not per-frame, so this
is a permanent duplication, not churn. `TexturePreviewWeight`
(`archive.rs:62-69`) sums only `tex.rgba.len()`, so the 128 MiB cap does not see
the second copy.

**Verified.** Fix: `DecodedTexture::rgba: bytes::Bytes`. `Bytes::from(vec)` at
construction is zero-copy, the `Handle` construction becomes a refcount bump,
and `chunks_exact(4)` / `&tex.rgba` call sites keep working through
`Deref<Target = [u8]>`. Same change makes
`ui/texture_preview.rs:606` (`rgba: texture.rgba.clone()` from a scene's
`SceneTexture`) a refcount bump, so scene diffuse textures stop existing twice
across the scene and texture caches.

`bytes` is already in the tree via iced, but `iced::core` is private, so it must
be added as a direct dependency. This is the closest thing to a new dependency
in the whole audit, and it is a transitive promoted to direct.

### 8. The entry list is cloned twice per 3D load *(fixed 2026-11-27)*

Both the 3D scene load path and the Bully AGR playback load path in `app.rs`
now consume their cloned `Vec<EntryInfo>` via
`ArchiveTextureIndex::from_entries_owned` (computing `candidate_models` first
in the AGR path) instead of calling `from_entries` and cloning a second time.
This eliminates the redundant ~3 MB `Vec<EntryInfo>` allocation per 3D load on
16,316-entry archives.

### 9. Loads bypass the archive's own mmap *(fixed 2026-11-27)*

All remaining UI load paths in `app.rs` (3D scene load for DFF/NIF/COL, texture
preview tab for TXD/NFT, GTA IFP/DFF playback, Bully AGR/NIF playback,
`plan_replace`, `ReplaceConfirmed`, and external viewer launch) plus
`ArchiveTextureIndex` and `export_entry_to_file` now borrow entry slices
directly from `archive.source_mmap` via `read_entry_bytes_with_source`
(`Cow::Borrowed`, falling back to owned reads only for `imported` or unmapped
archives). Combined with a `parse_txd_headers` pre-check in
`ArchiveTextureIndex::resolve_txd_entry`, archive-only DFF texture resolution
dropped from 1,249 ms to 65.7 ms (19× faster) across 15 retail DFFs. See
[memmap2-io.md](memmap2-io.md).

### 10. Full-archive validation allocates ~500 K Strings and retains none of them *(fixed 2026-11-27)*

`compat/games.rs` now exposes allocation-free `classify_verdict` and
`classify_nft_verdict` sharing the exact same decision tables as `classify` /
`classify_nft_format` without calling Fluent or allocating `note: String`.
`compat/scan.rs` uses those verdicts, updates `class_counts` / `nft_formats`
without allocating a `String` on map hits, borrows entries zero-copy from
`source_mmap`, and uses `parse_txd_headers` when `decode_pixels` is false
(skipping 350+ MB of transient mipmap copies). Combined with Rayon `try_fold` +
`try_reduce`, full validation of `gta3.img` dropped from 263.8 ms to 25.7 ms.

### 11. An uncapped per-pixel `HashSet` where a capped one already exists *(fixed 2026-11-27)*

`compat/scan.rs:822-829` `unique_color_count` previously inserted every pixel
into an uncapped `HashSet<[u8;4]>` with no early exit. A photographic 1024²
texture could insert up to 1,048,576 entries — ~10 MiB final table, ~15 MiB
transient during the last rehash.

`compat/scan.rs` now delegates directly to
`parser/texture_decoder.rs::palette_colors(&rgba)` (`unique_color_count_capped(rgba, 256)`
with pre-sized `HashSet::with_capacity`), bounding the table at 257 entries and
exiting immediately once a texture exceeds an 8-bit palette.

### 12. A full-mesh scratch allocation to compute a bound already in hand

`animation/pose.rs:361` builds `let positions: Vec<[f32;3]> = vertices.iter()
.map(|v| v.position).collect()` solely to call `Aabb::from_points` at
`pose.rs:377` — 12 B per vertex of pure scratch (240 KB at 20 k vertices) on
every `rest_scene` and headless render. The answer is already computed by the
same vertex sweep into `buffers.posed_bounds` (`pose.rs:63`).

### One-liners worth folding into whatever is already being touched

- `app.rs:8104` `let mut decoded = Vec::new()` then pushes up to
  `MAX_TEXTURES` (16,384) entries — `with_capacity(txd.textures.len())`. Same
  class at `inspector/texture.rs:376` and `:582`.
- `viewer3d_widget.rs:389` `build_overlay_vertices` starts from `Vec::new()`
  with no `reserve`, and is called from `prepare` on every redraw whenever the
  skeleton or motion-path overlay is on. It emits `2×(nodes-1) + 2×47`
  `SkeletonVertex` at 28 B, grown by doubling. Pass a scratch buffer and
  `clear()` + `reserve` the exact count. It also clones the cached motion path
  per frame (`viewer3d_widget.rs:407` → `viewer_session.rs:502`); return
  `&[Vec3]` instead.
- `inspector.rs:386-390` clones the whole inspection to insert it into the
  cache and then returns the original — insert first, return
  `(*cached).clone()`.
- `animation/binding.rs:299-312` `calibration_inputs` **deep-clones every
  rotation channel** in the library (`TrackChannel::clone()` clones both inner
  `Vec`s) to feed `rotation_min_angle`, which only ever reads `values` and
  ignores `times` entirely (`binding.rs:257`). Store `Vec<&[Quat]>`. For a
  library of 200 clips × 50 rotation tracks × 60 keys that is ~12 MB of pure
  duplication held for the whole of `calibrate_bindings`. Same fix removes a
  `String` allocation per track visit at `binding.rs:392`.
- `animation/bully.rs` `push_key` (`bully.rs:811-824`) does a linear
  `tracks.iter_mut().find(...)` per key — O(tracks) per key, plus a
  `curve.contains(&next)` O(n²) cycle guard at `bully.rs:684` and six
  unreserved `Vec::new()`s. This is CPU, not memory, and the cycle guard is a
  correctness guard: replace it with a `Vec<bool>` visited bitmap rather than
  deleting it.

---

## Tier 2 — Real, but unsized. Run these first.

Each is a counter, not an investigation. All of them read a value the code
already has.

1. **Material-group multiplier M** — decides whether the per-material vertex
   duplication in `append_geometry_meshes` (`parser/dff.rs:1216-1238`, which
   clones positions/normals/uvs once *per referenced material*, so a geometry
   with M materials retains M × 32 B/vertex) is real. Per DFF, compare
   `Σ parse_dff(...).meshes[i].positions.len()` against
   `Σ build_scene_from_dff(...).meshes[i].vertices.len()`, and histogram
   `meshes.len()`. **Equal sums ⇒ M = 1 everywhere ⇒ ignore this entirely.**
   If M > 1 is common, the fix is structural (one vertex buffer plus N
   `(first_index, index_count, texture)` draw ranges), not local.
2. **Index-to-vertex ratio** — sizes finding 1 and the `u32`→`u16` change. From
   the same loop: `Σ indices.len() / Σ vertices.len()`. The ~6 I/v used above is
   an assumption. The DFF triangle record is *already* `u16`
   (`dff.rs:69-74`) and is widened at `dff.rs:1210`; the COL path genuinely
   needs `u32` (`decode.rs:35` allows 2,000,000 preview vertices), so this would
   have to be a per-mesh runtime format across ~15 draw sites, not a global
   switch.
3. **Textures per TXD (histogram)** — sizes finding 3. If the median TXD holds
   2-3 textures, a per-entry byte cap is sufficient and the lazy-decode refactor
   is not worth it. The existing measurement of 468 textures → 23 MiB
   (`allocator-rusty-alloc.md`) implies a ~113×113 average, which is the number
   to check against.
4. **Clips and tracks per retail AGR** — sizes finding 6 and the
   `PropertyTrack.target: String` interning. `PropertyTrack` is 88 B of which
   24 B is a `String` triple; `bully.rs:1354`/`1366` `format!("track_{:03}", …)`
   a fresh string **per track per clip** from a set bounded by the max track id,
   so a library holds N×T strings where T distinct values would do. `gta.rs:256`
   additionally keeps a second copy of every DFF frame name in
   `ModelAsset.source_names`.
5. **`rayon::current_num_threads()`** — sets the only genuine thread-count
   multiplier found: quantette's Wu histogram is a `Box<[[[Stats;16];16];32]>`
   = 320 KiB **per rayon chunk**, live simultaneously during
   `reduce_with` (`quantette wu/f32.rs:497`). Bounded and inside a tested crate,
   but it is the only term in the whole audit that moves with core count.
6. **`plan_fixes` output size** — `compat/normalize.rs:78-89` `collect_fixes`
   stores a full patched copy of every fixed entry into
   `entry.override_bytes`, which then takes priority over the mmap in every
   read path and the save writer. Unlike the three caches, this has **no byte
   budget**. It is inherent to the override design, so the fix is a budget plus
   a reported remainder, not a redesign.
7. **Worst-case single TXD entry** — `Σ entry.sector * 2048` over `.txd` entries.
   Sets the peak of the scan path, which holds the mmap range (`scan.rs:455`)
   and every mip level copied again by `parse_txd` (`txd.rs:705`) at the same
   time, plus one full 16,316-entry validation's working set.

---

## Tier 3 — Already correct. Recorded so it stays that way.

- **The animation core allocates nothing per frame.** `PoseBuffers`
  (`animation/pose.rs:48-96`) is allocated once in `AnimationSession::new` and
  reused; `sample_locals` writes into pre-sized `&mut [NodeTransform]` slices
  and `sample_track` returns by value. `Transport::advance`,
  `crossed_markers` (Bully AGR clips carry no markers, so the `Vec::new()` at
  `transport.rs:592` never allocates), `deform_vertex`, and `evaluate_pose` are
  all allocation-free. The module doc's claim is accurate. The per-frame
  pressure is in `view.rs` (finding 6), not here.
- **Clip switching does not copy clips.** `select_clip` stores a `ClipId` and a
  fresh `ClipBinding`; the library is a shared `Arc` and the clip is borrowed
  through it. The only copy is one `Vec<NodeTransform>` (40 B/node) for the
  optional crossfade, released when the fade completes. Cross-clip bind-pose
  calibration holds a `HashMap<String, NodeId>` assignment — **no poses, no
  vertex data**, built once per session and shared.
- **`Arc<Vec<DecodedTexture>>` as the texture-cache value.** The per-frame
  deep-copy hazard is prevented by the `OnceLock<Handle>`, not by the `Arc`;
  `view.rs:1693` clones an `Arc` (refcount bump). Correct as is.
- **`Vec<Vertex>` at 32 B, `bytemuck::Pod`, interleaved pos/normal/uv.** Zero-copy
  cast, one `create_buffer_init`. Quantizing positions would break the in-place
  pose rewrite (`pipeline.rs:321-324`) and the headless pixel-diff tests.
- **CPU `Scene` and the GPU vertex buffer co-resident — this is not
  duplication.** `SceneHandleInner.scene` and the cache value are the same
  `Arc<Scene>`; the GPU buffers in `mesh_cache` exist for the one displayed
  scene only.
- **The single-flight cache guards.** `get_value_or_guard` + `guard.insert` is
  correct, including the `Err` path (cache invalidated mid-decode) and the
  `retain`-skips-placeholders caveat handled by
  `drop_in_flight_placeholder`.
- **Export is disk-bound and zero-copy from the mmap** — 21.98 s, unchanged
  across every allocator arm in the A/B. The allocator cannot move it.
  Peak-only concerns remain in the NFT export path
  (`inspector/texture_export.rs`), which materializes the whole catalog before
  writing, and in `app.rs:6911`, which rebuilds `IdeMap::build` on every export
  instead of reusing the `App::ide_maps` memo the 3D viewer uses.
- **`parse_dff`'s bounds checks and `bounded_count` limits** (`dff.rs:40-45`) are
  why a malformed archive cannot become a 12 GB allocation.
- **The `compat` conversion driver is serial** (`app.rs:9281`, `app.rs:9357`),
  so conversion peak is 1× per texture, not threads×. rayon appears in
  `src/` only in `compat/palette.rs` (LUT build) and `tasks.rs` (chunked export,
  clamped to 8 workers). "Just add a `par_iter`" would be a new risk, not an
  existing one.
- **`raster.rs:389 pub type Counts<T> = BTreeMap<T, usize>`** is a dead alias
  with zero referents, not even re-exported from `compat/mod.rs`. It is not a
  per-pixel hot loop, contrary to a reasonable first reading. The live counters
  are per-texture `BTreeMap`s. Worth deleting so nobody later "optimizes"
  against a false premise.

---

## Suggested order of work

1. **Finding 1, as instrumentation rather than a fix.** It corrects a 1.86×
   accounting error, and until it is right no decision about the scene cache cap
   is informed.
2. The Tier-1 one-liners: finding 8, finding 11, the two
   `with_capacity`/`reserve` sites, and the `inspect_entry_cached` insert order.
3. Findings 2 and 4 — the largest peak reductions on the two dominant paths,
   both mechanical signature changes.
4. Finding 5 — the largest single allocation in the converter, and the
   downsampling is free quality-wise.
5. Run the Tier 2 counters and revisit findings 1, 3, 6 and the `u16` index
   change against real numbers.

## Re-measuring after any of this

The harness and its traps are documented in
[allocator-rusty-alloc.md](allocator-rusty-alloc.md) §"Profiling method". Note
two measurement rules: always use OS working set and private commit as the
ground truth (`rusty_alloc` 2.6.0 fixed `stats::merged()` reporting zeros for
allocs/frees, but OS counters remain the authoritative process-wide metric),
and a cached `Start-Process -PassThru` object with `Refresh()` returns frozen
private-bytes values, so a fresh `Get-Process` object per sample is mandatory.
