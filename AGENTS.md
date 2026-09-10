# IMGEditor Plus — Agent Notes

## Version bump checklist

Before tagging a release or publishing a build, update every user-facing version location:

1. `Cargo.toml` — `package.version`.
2. `README.md` — top-level heading (e.g. `# IMG Editor Plus v3.3`).
3. `src/ui/view.rs` — status bar text uses `env!("CARGO_PKG_VERSION")`, so it updates automatically after step 1.
4. `src/ui/app.rs` — Welcome modal also uses `env!("CARGO_PKG_VERSION")` dynamically.
5. Git tag — create and push `v{MAJOR}.{MINOR}.{PATCH}` after committing.
6. GitHub release notes — reference the same version string.

Run `cargo check` after changing `Cargo.toml` to confirm the status bar and welcome modal pick up the new version.

## Resource-constrained build policy (IMPORTANT)

The dev machine has ~16 GB RAM, tight disk space, and a small Windows
paging file. Heavy cargo commands can OOM-kill the linker/compiler,
which **crashes or destabilizes the user's other programs** (browsers,
etc.) and leaves corrupted artifacts in `target/` that cascade into
bizarre "invalid metadata" / "can't find crate" errors on later runs.

Rules when building or testing here:

1. **One heavy command at a time.** Never chain `cargo check && cargo
   clippy && cargo test` in a single run — finish one, inspect, then
   start the next.
2. **Cap parallelism:** run `cargo build -j 2` / `cargo test -j 2`
   whenever the build touches many crates (after a clean, dependency
   bumps, or when more than ~10 crates need compiling). Incremental
   single-crate rebuilds may use default parallelism.
3. **Prefer narrow scopes:** `cargo check`, `cargo test --lib`, or
   `cargo test --lib <module>` before the full suite; full-suite runs
   (which link examples) only when needed and one at a time.
4. **Avoid `cargo clean` unless the target dir is provably corrupt**
   (E0460/E0786/E0463-style errors right after an OOM). It deletes
   gigabytes and forces a full multi-minute rebuild. A failed run's
   follow-up retry with `-j 2` is usually enough.
5. **Watch for the OOM signature:** `LNK1102: out of memory`,
   `os error 1455` ("paging file is too small"). If seen, stop, do not
   immediately retry at full parallelism — rerun with `-j 2`.
6. **Never run the full test suite and a release build concurrently**,
   and never leave background builds running while doing other work.

If artifacts were corrupted by an OOM kill, `cargo clean` followed by
`cargo test -j 2` is the reliable recovery path (done 2026-09-10).

## Dependency decisions (do not re-add without reading this)

These choices were audited deliberately; re-adding or "upgrading" them
without a measured need repeats the mistake they fixed.

- **`crossbeam` was removed (direct dep).** Zero references in `src/`.
  All inter-thread traffic is already served by iced `Task<Message>`
  (UI state is single-threaded by design) and `tokio::sync::mpsc`
  (viewer events). Message volumes are tiny (per user action) against
  millisecond-scale workloads (decode/IO/GPU), so crossbeam's wins —
  MPMC channels, `select!`, lock-free structures, runtime-free
  threading — are below the measurement floor here. Note: `crossbeam-*`
  crates remain in `Cargo.lock` **transitively via rayon** (rayon's
  scheduler is built on crossbeam-deque/epoch); that is expected and
  not a leftover. Re-add only if a feature genuinely needs
  multi-consumer channels or lock-free structures (>100k ops/sec,
  many threads).
- **`memmap2` is used at its most basic level on purpose.** Archive
  maps are plain `Mmap::map` (src/parser/pc_v1.rs, pc_v2.rs). All of
  memmap2's advice/prefetch surface (`advise`, `advise_range`,
  `MmapOptions::populate`, `lock`) is `#[cfg(unix)]`-gated — on
  Windows it either does not exist or is silently ignored
  (`populate` compiles but maps to an ignored `_populate` flag). The
  app is Windows-first, so plain `Mmap::map` is the correct API and
  lazy page-faulting already matches the random-access read pattern.
  When the far-future Linux port happens, add a `#[cfg(unix)]`
  `advise(Advice::Random)` + `populate()` to the archive mmap sites.
- **Do not reach for `parking_lot` to "improve" `std::sync::Mutex`.**
  The gap vs std is microbench territory (std uses SRWLock/futex
  internally); the app's one hot lock (`SceneHandle.inner`) is
  effectively uncontended. Reconsider only on a profile that shows
  lock overhead.

## Executable icon

The Windows executable icon is embedded from `asset/logo/IMGEditorLogo.ico` via `build.rs` and `asset/logo/icon.rc`. The ICO was generated from `asset/logo/IMGEditorLogo.png` with Pillow at sizes 16, 32, 48, 128, and 256. If the source PNG changes, regenerate the ICO:

```powershell
python -c "from PIL import Image; img = Image.open('asset/logo/IMGEditorLogo.png').convert('RGBA'); img.save('asset/logo/IMGEditorLogo.ico', format='ICO', sizes=[(16,16),(32,32),(48,48),(128,128),(256,256)])"
```

The runtime window icon is decoded separately from the same PNG in `src/ui/app.rs`.

## Debugging crashes (dev workflow)

The 3D viewer pipeline (Phase 17) is built on top of `iced_wgpu` and a
custom `Primitive` that issues wgpu render commands inside Iced's
compositor. When something goes wrong, errors are reported by wgpu's
default error handler as **fatal panics** that propagate through the
winit event loop and silently tear down the window. The dev logger
(`src/dev_logger.rs`) and the two log files below exist specifically
to make those failures debuggable.

### Log file locations

| File | When populated | What it contains |
|---|---|---|
| `target/debug/imgeditor-dev.log` | every cold launch of the debug binary | all `log::*!` records (debug builds: trace+; release builds: warn+); mirrored to stderr in debug |
| `<exe-dir>/imgeditor-panic.log` | every panic (and on graceful exit during teardown) | version, profile, OS, panic location, full backtrace, breadcrumbs leading up to the crash |
| `imgeditor-panic.log` is appended to on every panic — never deleted. The user may also be running a release build that writes only `<exe-dir>/imgeditor.log`. |

### Workflow when a "silent" crash is reported

1. **Find the panic log first.** Read `target/debug/imgeditor-panic.log` (or `<exe-dir>/imageditor-panic.log` in release). Look for the `Caused by:` section — wgpu's validation error messages are very specific.
2. **Cross-reference `imgeditor-dev.log`.** The dev log has breadcrumbs like `user: open in 3D viewer (in-app)`, `3D load ok: 364 verts, 320 tris`, and `render_to_offscreen: <w>x<h>, ...`. Reading top-to-bottom tells you what the user was doing when the panic happened.
3. **Reproduce with `cargo run`.** In dev builds the dev logger mirrors to stderr, so `cargo run 2>&1 | grep imgeditor.scene3d` is enough to see the breadcrumbs live.
4. **Check release-only paths.** Release uses `LevelFilter::Warn`, so the dev log is much sparser. A release-only crash should be reproduced in dev before chasing.

### When adding new code paths

- Use `dev_logger::breadcrumb("event description")` at major user-action boundaries
  (entry into a tab, scene load, render error) so the log has a
  breadcrumb trail.
- Use `log::trace!` / `log::info!` / `log::error!` from the `log` crate
  for sub-event details. The dev logger installs itself as the global
  logger; no further setup needed.
- Do **not** swallow wgpu errors. If you must catch them, log via
  `log::error!` AND re-emit / re-panic — the dev logger does NOT
  replace wgpu's panic path; it's purely additive.

## Virtual scroll offset (entry table)

`App.scroll_y` is a **virtual offset maintained by the app**, not a live
readout of the scrollable. Iced 0.14's `Scrollable` publishes `on_scroll`
only for interactive scrolling (wheel, scrollbar drag, touch) —
**operation-driven `scroll_to` calls never fire it**. Consequences:

- Every code path that calls `scroll_to("entry_table", …)` MUST also
  update `self.scroll_y` to the target (sticky tick, drag mode,
  prediction commit, cancel-restore). Rebasing on a stale
  `self.scroll_y` snaps the view back to the last wheel position.
- Sticky autoscroll integrates its own running offset
  (`AutoScroll::sticky_scroll_y`) and mirrors it into `self.scroll_y`;
  wheel deltas arrive through `Message::ScrollOffsetChanged` and fold in.

## 3D scene cache (quick_cache) + telemetry

The 3D viewer caches decoded scenes so revisiting an entry is instant.
Everything lives in `src/ui/app.rs` unless noted:

- `App::scene_cache` — a byte-budgeted `quick_cache` (crate `quick_cache`,
  feature `stats`) keyed by `(archive file name, archive generation, entry
  index)`. Weighted by `Scene::estimated_gpu_bytes()`, soft cap
  `SCENE_CACHE_WEIGHT_CAPACITY` (256 MiB desktop, 64 MiB mobile via `cfg`).
  Cache hits restore the scene synchronously (`In-app 3D viewer ready
  (cached)` in the archive log); misses take the async load path.
- `ArchiveInfo::generation` (src/archive.rs) — bumped by
  `invalidate_entry_caches()` on every entry add/remove/rename/import;
  folding it into the key makes stale scenes miss. That hook also clears
  `inspection_cache`/`texture_cache`.
- Closing an archive evicts its scenes via
  `App::drop_scene_cache_for_archive` (cache `retain`), since the cache
  is app-global, not per-archive.
- The per-game-root `IdeMap` is memoized in `App::ide_maps`; the first 3D
  load per game root builds it, later loads reuse it.
- `ArchiveInfo::texture_cache` (src/archive.rs) — same `quick_cache`
  treatment as the scene cache, weighted by decoded RGBA bytes (128 MiB
  desktop / 32 MiB mobile). Values are `Arc<Vec<DecodedTexture>>` because
  `view.rs` reads it per-frame; a plain value type would deep-copy the
  buffers on every redraw (quick_cache `get` clones by value).

**Telemetry note (remove before "finished"):** the cache-hit breadcrumb
(`3D cache hit: entries N (hits X, misses Y, resident Z MiB)`) and the
`stats` feature on the `quick_cache` dependency exist as a lightweight
benchmark/observability aid, not a user feature. The `stats` feature
could be dropped from `Cargo.toml` and the breadcrumb removed with no
functional impact once development stabilizes.

## 3D viewer (Phase 17) architecture constraints

When modifying the scene3d pipeline, keep these invariants in mind
to avoid panics that surface as silent crashes:

- **Pipeline formats must match the render pass attachments.** The
  lit/wireframe/grid/gizmo pipelines all render into the offscreen
  target (Rgba8UnormSrgb) — they MUST use `scene_color_format()`.
  Only the compositor pipeline targets the Iced surface, so it
  MUST use the surface format. A mismatch produces a
  "Render pipeline targets are incompatible with render pass" panic
  the next time the user enters the 3D view tab.
- **Sample counts must match too (MSAA).** The widget builds its scene
  pipelines at `SCENE_MSAA_SAMPLES` (4x) and its depth/MSAA-color
  attachments at the same count; the scene pass resolves into the 1x
  `scene_color` texture (resolve targets need `RENDER_ATTACHMENT`
  usage). The headless renderer deliberately uses 1x pipelines +
  1x attachments so pixel-diff tests stay byte-deterministic — pass the
  sample count to `ScenePipelines::new`; never assume it.
- **Headless GPU tests must use the shared `gpu()` helper in
  `inspector/scene3d/headless.rs`.** Constructing a
  `HeadlessRenderer` creates a `wgpu::Instance`, and concurrent
  instance creation races the driver loaders (intermittent
  STATUS_ACCESS_VIOLATION that kills the whole test process). The
  helper shares one renderer behind `OnceLock` + a mutex (the mutex
  also stops tests from overwriting each other's camera UBO
  mid-frame). A 0xc0000005 test crash is now a regression, not a
  flake.
- **Bind groups must match the pipeline layout.** The grid and gizmo
  pipelines don't sample a texture. Sharing the model's 2-bind-group
  layout produces a "BindGroup to be set at index 1" panic. Each
  pipeline that has fewer bind groups than the layout declares must
  use its own `PipelineLayoutDescriptor`.
- **Drop / recreate scene-color + depth textures on viewport change.**
  `ScenePipelines::ensure_size` rebuilds them when `width` or
  `height` change; do not assume they outlive a window resize.
- **Panic inside `Primitive::draw` / `render` propagates to the
  winit event loop.** Wrap risky work in `std::panic::catch_unwind`
  and on panic log + fall back to a placeholder so the rest of the
  GUI keeps working.

## Building / cleaning

### Locked imgeditor.exe (user-authorized force close)

A running app instance locks `target/debug/imgeditor.exe`, which makes
`cargo build` fail with "Access is denied (os error 5)" on link. When a
build is blocked this way, the agent is **pre-authorized by the user to
force-terminate the process without asking**, under these conditions:

1. Verify the process actually points into this workspace before
   killing it:
   `Get-Process | Where-Object { $_.Path -like "*IMGEditor-rs*" }`
2. Kill only that PID (`Stop-Process -Id <pid> -Force`). Never kill
   anything whose path does not resolve under this workspace.
3. Record the termination (what was killed and why) in the response to
   the user.
4. Rerun the build.

The full build can spike to 24 GiB of disk + several GiB of RAM
(release profile). If the toolchain runs out of either, run:

```powershell
cargo clean           # remove target/
cargo build           # fresh release + debug artefacts
```

The 293-test unit suite + the headless GPU smoke render pass on a
real Bully NIF (`python tools/smoke_3d_viewer.py`) takes ~60 s on a
debug build and produces a 9 KiB PNG with a real model + grid + gizmo
visible.
