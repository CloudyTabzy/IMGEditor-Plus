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

The full build can spike to 24 GiB of disk + several GiB of RAM
(release profile). If the toolchain runs out of either, run:

```powershell
cargo clean           # remove target/
cargo build           # fresh release + debug artefacts
```

The 143-test unit suite + the headless GPU smoke render pass on a
real Bully NIF (`python tools/smoke_3d_viewer.py`) takes ~60 s on a
debug build and produces a 9 KiB PNG with a real model + grid + gizmo
visible.
