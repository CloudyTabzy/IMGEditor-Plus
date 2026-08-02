# IMGEditor-rs — Next Objectives

## 1. Reverse-engineer texture pixel data storage

The `.nft` (NIF texture catalog) files store **path metadata only** — source paths like
`Z:\Bully\Temp\Export\Textures\Scenes\iobserv\PO00_guts_d.tga` — but **not the actual pixel bytes**.

### Known facts

| Fact | Detail |
|------|--------|
| `World.img` has 11980 entries | 5724 NIFs, 4469 NFTs, 550 `.agr`, 493 `.lip`, 488 `.col`, 119 `.cat`, 85 `.ipb`, 52 `.lur`. **Zero `.tga`/`.dds`** entries |
| NFT `NiSourceTexture.use_external == 0` | Implies embedded pixel data, but 0 bytes follow the header fields in practice |
| NFT has `NiPixelData` blocks | e.g. **43891 bytes** of data for `observ4.nft` — the pixels ARE there but the header format is unknown |
| NiPixelData header (attempted) | `pixel_format=4` at offset 0 (plausible), but `num_faces=0xFFFFFF00` garbage at offset 4 |
| Plausible mipmap entries at offset+28 | `19×5`, repeated 3 times with `data_offset=0` — suggests a variable-length mipmap table before pixel data |
| Pixel data starts somewhere after offset 64+ | Full block is 43891 bytes, so data section is ~43800 bytes |
| No `.tga`/`.dds` on disk at all | Confirmed: neither as loose files nor as named IMG entries. Pixels live **inside NFT NIFs** |
| `.txd` files in `TXD\` dir | Only frontend/UI textures (7 files, ~6 MB total) — not world textures |

### Investigation approaches

**A. Reverse NiPixelData header for 20.3.0.9**
- Dump raw bytes of a known NiPixelData block and reverse the field layout.
- Initial observations from `observ4.nft` block 3 (43891 bytes) and block 7 (2907 bytes):
  ```
  Header (first ~72 bytes):
  +0:    pixel_format (u32) = 4  (RGB8? RGBA8? DXT?)
  +4-7:  ??? (0x00FFFFFF in both blocks — constant)
  +8-15: ??? (255, 256 — constant)
  +16-23: 1024, 1024? (0x0400 as u16 — seen in both)
  +24-27: 0
  +28-63: 3 × (width=19, height=5?) repeated — possibly mipmap or face descriptors
  +64:    sentinel (0xFFFFFFFF)
  +68:    count (9 in block 3, 7 in block 7 — different!)
  +72+:   array of (width?, height?, data_offset?) at variable offset
  ```
- Block 7: total 2907 bytes. At +80-87: (64, 64) — plausible texture size.
- Block 3: total 43891 bytes. At +80-87: (1, 1) — plausible smallest mip size.
- Pixel data starts somewhere after offset ~96 bytes; the remaining bytes
  are likely DXT-compressed (block 7: 2907-96=2811 px bytes fits 64x64 DXT).
- Key unknowns: pixel_format enum values for 20.3.0.9, exact mipmap descriptor
  layout, face count field location.

**B. Cross-reference unnamed IMG entries with NFT source paths**
- For each unnamed IMG entry, read its raw bytes and compute a hash
- For each NFT, hash the known texture source paths and look for matches
- If a match is found, record the IMG entry name → texture mapping

**C. Scan the IMG directory for entries containing TGA/DDS magic bytes**
- Even without matching names, the first few bytes of each entry can reveal the format (TGA starts with `0x00 0x00 0x02`, DDS with `0x44 0x44 0x53 0x20`)
- Build a `{magic → entry_name}` map to identify which entries hold texture data

**D. Search for the NFT source paths as binary strings inside World.img**
- The full paths like `Z:\Bully\Temp\Export\Textures\Scenes\iobserv\PO00_guts_d.tga` may appear as ASCII strings in the IMG data near their corresponding pixel data

## 2. Intermediate format options (while texture extraction is incomplete)

- **Checkerboard/placeholder texture**: when pixel data isn't available, generate a coloured
  checkerboard TGA so the user can at least see UV mapping in F3D
- **Vertex colour fallback**: if the NIF has vertex colours, write them to the PLY and let
  F3D display them (PLY supports per-vertex colours with `property uchar red` etc.)

## 3. Quality-of-life improvements

- Cache parsed NFT catalogs (the same NFT serves many NIFs)
- Add a CLI or GUI option to specify the game root path (instead of deriving from archive path)
- Clear old temp files on startup (`%TEMP%\IMGEditor\preview\`)

---

# 4. v3.4 — Embedded wgpu 3D viewer (replaces external PLY-viewer fallback)

Last planning session produced the following implementation backlog. Items here mirror `Docs/implementation_phases.md` Phase 17 — cross-check that doc before relying on this one, it is the higher-level source of truth.

## Architecture recap

- Right-pane `Info` pane gets an `iced_aw::tabs::Tabs` with two tabs: `Model` (new) and `Texture` (existing TXD preview).
- 3D viewer is a custom Iced widget (`src/ui/viewer3d_widget.rs`) that issues wgpu 27.0.1 render passes into Iced's existing device/queue — no second window, no thread battle, replaces the `cmd /c start` PLY spawn entirely for the in-app path.
- NIF only. DFF and COL stay on external viewer until v4.
- External PLY-viewer kept behind a renamed context-menu entry ("Open in external viewer") for one release; removed in v4.

## New files

- [ ] `src/inspector/scene3d/mod.rs`
- [ ] `src/inspector/scene3d/mesh.rs` — interleaved position/normal/uv vertices, `u32` indices, owned GPU buffers
- [ ] `src/inspector/scene3d/camera.rs` — orbit camera (`glam::Mat4`), `reset_to_aabb`, view/proj math
- [ ] `src/inspector/scene3d/scene.rs` — `Scene { meshes, ambient, key_light, base_orientation }`
- [ ] `src/inspector/scene3d/pipeline.rs` — lit WGSL shader, wireframe shader, depth state, MSAA 4x, camera UBO, diffuse bind group
- [ ] `src/inspector/scene3d/decode.rs` — pure decode from NIF + NFT catalog to `Scene`
- [ ] `src/ui/viewer3d_widget.rs` — `impl Widget<Message, Theme, Renderer>` for Iced 0.14
- [ ] `src/ui/viewer3d_state.rs` — `Viewer3dState`, LRU cache (4 entries) on `App`
- [ ] `src/ui/viewer3d_toolbar.rs` — icon-button toolbar (Reset / Wireframe / Textured / Cull / Alpha / Skeleton / Base Orient / Help)

## Modified files

- [ ] `Cargo.toml` — add `wgpu = "27"` (pin exact, no `^`) and `glam = { version = "0.29", features = ["bytemuck"] }`
- [ ] `src/inspector/mod.rs` — `pub mod scene3d;`
- [ ] `src/config.rs` — extend `Config` with `Camera { base_orientation, sensitive_mouse, invert_y }`
- [ ] `src/ui/app.rs` — new `TabId { Model, Texture }`, `selected_inspector_tab`, `viewer3d_cache`. New messages `Viewer3dLoadRequested`, `Viewer3dLoadCompleted`, `Viewer3dCameraChanged`, `Viewer3dSelectTab`. Split `EntryAction::Render` into `Render` (in-app) and `RenderExternal` (PLY fallback).
- [ ] `src/ui/view.rs` — embed `iced_aw::tabs::Tabs` in `build_info_panel`. Wire Model tab to `viewer3d_widget::view`, keep Texture tab on existing TXD code. Update `build_context_menu`.
- [ ] `src/ui/keymap.rs` — add `Shortcut::{ToggleWireframe, ToggleTextured, ToggleFaceCull, ResetView, CycleAlpha, CycleBaseOrient, ToggleSkeleton, ToggleInfoOverlay}`. Only consume keys when `viewer3d_focused`.
- [ ] `src/ui/widgets.rs` — small `icon_button(icon, msg)` helper if missing.
- [ ] `README.md` + `dist/README.md` — one-paragraph mention of the new viewer + keymap.

## Tests

- [ ] `scene3d::camera`: orbit math against analytic values; `reset_to_aabb` against `1950Fridge.nif` known AABB.
- [ ] `scene3d::decode`: load `1950Fridge.nif`, assert triangle count is the known good value (filter degenerates via `viewer3d::append_mesh`).
- [ ] `scene3d::pipeline` headless: render one frame to offscreen wgpu target with `force_fallback_adapter = true`, PNG diff against committed baseline.
- [ ] `inspector::texture` regressions stay green (already covered by existing tests in `src/inspector/texture.rs`).

## Keymap (modern, not Noesis F-keys)

| Input | Action |
|---|---|
| LMB / MMB / Shift+LMB / wheel | Orbit / Pan / Pan / Zoom |
| R / F | Reset / Refit |
| W / T / C | Wireframe / Textured / Cull |
| B / V / X | Base orient / Alpha / Skeleton |
| H | Help overlay |
| Esc | Drop keyboard focus |

## Deferred to v4

- Specular / normal / detail map texture rendering.
- DFF and COL in the embedded viewer.
- Skeleton skinning (need unpacked `NiSkinInstance` blocks).
- Screenshot / OBJ export from the in-app viewer.
- Animated NIFs (NiControllerSequence, NiKeyframeController).
- Removal of `src/inspector/viewer3d.rs` external fallback.

## Implemented (v3.4)

- [x] `src/inspector/scene3d/mod.rs` + 5 submodule files
- [x] `src/ui/viewer3d_widget.rs` — Iced `Widget` + `iced_wgpu::primitive::{Primitive, Pipeline}` impls
- [x] `src/ui/viewer3d_state.rs` — replaced by inline `SceneHandle` (Arc<Mutex<...>>) since MVP only needs one slot
- [x] `src/inspector/scene3d/headless.rs` — headless GPU renderer for tests, sourced into the widget indirectly through `pipeline.rs`
- [x] `Cargo.toml`: added `wgpu = "27"`, `glam = "0.29"`, `bitflags = "2"`, `pollster = "0.4"`, `iced_widget = "0.14"` (wgpu/canvas features)
- [x] `Cargo.toml`: bumped `version = "3.4.0"`
- [x] `src/ui/app.rs`: `EntryAction::Render` (in-app NIF) vs `EntryAction::RenderExternal` (PLY fallback), `Message::Viewer3dRequestLoad/Completed/SelectTab/Clear/Reset`, `ViewerTab` enum, `viewer3d_handle: Arc<SceneHandle>` on `App`
- [x] `src/ui/view.rs`: `iced_aw::tabs::Tabs` in `build_info_panel` (Model 3D | Texture), toolbar (Reset view, Clear), status text uses `env!("CARGO_PKG_VERSION")` so it picks up the bump automatically
- [x] `src/ui/view.rs`: context menu split into "Open in 3D viewer" + "Open in external viewer"
- [x] `README.md` + `dist/README.md`: bumped to v3.4 with embedded viewer section
- [x] `tools/smoke_3d_viewer.py`: CLI smoke test that exercises the fixture path

## Tests

143 passing. Notable entries:
- 24 in `inspector::scene3d::mesh + camera + decode + scene + pipeline` (pure CPU)
- 4 in `inspector::scene3d::headless` (real wgpu against the fallback adapter, including the Bully fixture PNG path)
- 5 in `ui::viewer3d_widget` (handle state, signature, Send)

The fixture PNG (`target/scene3d-bully-1950fridge.png`) renders the actual 1950s refrigerator model from the test machine — confirms lit shading, depth test, vertex / index buffers, camera math, and orientation matrix all converge.

## Risks to re-check at implementation time

- `wgpu 27` must match exactly. If `iced_wgpu 0.14` ever bumps internally, we must bump in lockstep or pull a private patch.
- Iced 0.14 `Widget::draw` is `Fn`, so GPU handles have to live behind `Rc<RefCell<_>>`.
- DXT decode is CPU-bound, runs on `tokio::task::spawn_blocking` so the Iced thread doesn't stall.
- Skeleton toggle (`X`) is a no-op stub in v3.4; warn in the toolbar tooltip rather than the UI freezing.
- Camera orbit angles are NOT persisted per-entry. We re-fit on every selection.

## 5. Known issues to fix in v3.4.1 (deferred from v3.4.0)

Tracked separately because they are **bugs** (not backlog features). Release notes for v3.4.0 already call these out as known issues.

### 5.1 Black 3D viewport in GUI (headless test passes) — FIXED

**Root cause:** `OrbitCamera` derived `Default`, which zeroed `fov_y_deg`/`near`/`far`. The GUI builds its camera through `SceneHandleInner::default()` and `reset_to_aabb` never sets those three fields, so the uploaded projection matrix contained inf/NaN. The grid and lit pipelines then drew nothing (grid rays failed the `abs(ray_dir.y)` test and fell back to the background colour), while the gizmo — which uses no camera UBO — still rendered. Headless tests always constructed the camera via `OrbitCamera::new` (fov 45, near 0.1, far 10 000), which is why they passed.

**Fix:**
- `camera.rs`: manual `impl Default for OrbitCamera` delegating to `OrbitCamera::new(Viewport::default())`, plus a regression test (`default_camera_yields_finite_view_proj_after_aabb_reset`).
- `viewer3d_widget.rs`: offscreen colour/depth targets and the camera frustum are now sized to the widget's physical rect (was: whole window), and `composite_to_frame` sets its viewport to the widget rect with the scissor on `clip_bounds` — the blit is 1:1 and centred in the pane instead of squashing a window-sized render into it (old diagnosis #2).

**Original diagnostic notes (superseded, kept for history):**

**Symptom (original report):** clicking the "3D view" tab on a Bully NIF entry produces a black pane except for the small axis gizmo in the bottom-right corner. Stats line correctly shows `392 vertices 359 triangles 0 textures 1100×720`, the dev log confirms all four pipelines execute every frame (grid → lit 1077 verts → gizmo → compositor), and no wgpu validation errors fire.

**Workaround:** use the external viewer button (PLY spawn).

**Diagnostic commands (in order):**

1. Add a one-line log in `ScenePipelines::new` (pipeline.rs:386):
   ```rust
   log::info!(target: "imgeditor.scene3d", "ScenePipelines::new: target_format = {target_format:?}");
   ```
   Confirm whether Iced is handing us `Bgra8UnormSrgb` (what the headless test uses) or something else. If different, the compositor pipeline's color target format is wrong for the Iced surface.

2. If format is correct, the next suspect is the compositor sampling interaction with `clip_bounds` viewport. The headless test puts the composite pass at `(0, 0, w, h)`; the GUI uses `Rect { x: 554, y: 224, w: 542, h: 440 }`. The compositor vertex shader writes `clip_position = vec4(position, 1.0)` with position in `[-1, 1]`, no UV remap. UVs `(0, 0)` → `(1, 1)` map directly to `scene_color_target` texels. If the aspect ratio or scissor math is off, the compositor samples outside the populated region (where `scene_color_target` was cleared but the grid/lit didn't write). The gizmo still renders correctly because it uses its own NDC box `(0.78, -0.78) ± 0.15` regardless of widget bounds.

3. If neither diagnosis pinpoints it, add a debug-only PNG dump of `scene_color_target` to `target/debug/scene_color_gui_<n>.png` for the first 3 frames, triggered from `Primitive::render` after `render_to_offscreen`. Compare against `target/scene3d-two-pass-gui.png` from the headless test (they should be byte-identical for the same input).

**Files involved:**
- `src/inspector/scene3d/shaders/compositor.wgsl`
- `src/ui/viewer3d_widget.rs` (`render_to_offscreen`, `composite_to_frame`, `Primitive::render`)
- `src/inspector/scene3d/pipeline.rs` (`ScenePipelines::new`, `target_format` plumbing)

### 5.2 Mouse / wheel / keyboard input not verified

**Status (2026-08-02): orbit drag FIXED.** User testing showed the camera stuck in a fixed view with drags never orbiting. Root cause: `Scene3dWidget::update` pre-seeded `state.last` with the current cursor position on every `CursorMoved` before `handle_event` ran, so every drag delta computed as zero. Fixed by letting `handle_event` own the drag anchor; covered by the `left_drag_orbits_camera` widget test.  Left-drag orbit confirmed working in the GUI. Follow-up (2026-08-02): orbit direction inverted to Blender style (mouse right orbits the view left) in `OrbitCamera::orbit`, and the view gizmo now rotates with the camera — `CameraUniform` carries the `view` matrix, the gizmo pipeline binds the camera UBO, and `gizmo.wgsl` projects the world axes through the view rotation; also fixed the gizmo border shader filling the whole box with the border color. Covered by the `gizmo_axes_follow_camera_orbit` headless test. Still unverified end-to-end: cursor Grab/Grabbing states, wheel dolly, shift+drag pan, keyboard shortcuts.

The `Scene3dWidget::update` handler looks correct (mouse drag → `camera.orbit`, shift+drag → `camera.pan`, wheel → `camera.dolly`, `Modifier::Shift` is tracked). However, this was never confirmed end-to-end in the GUI. The headless test exercises the GPU path but not the input path. Needs:

1. Launch the GUI, click into the 3D view pane.
2. Confirm cursor changes to `Interaction::Grab` on hover and `Grabbing` while dragging.
3. Drag — camera should orbit. Scroll wheel — camera should dolly. Shift+drag — camera should pan.
4. Keyboard shortcuts from `keymap.rs` (`R`/`W`/`T`/`C`/`B`/`V`/`X`/`H`) are deferred per the original TODO but should at minimum not cause focus stealing from the table search input.

If dragging doesn't change the camera, add `dev_logger::breadcrumb` calls at the top of `Scene3dWidget::update` and inside `handle_event` to see whether the Iced event loop is delivering events to the widget at all.

**Files involved:**
- `src/ui/viewer3d_widget.rs` (lines 162-222, 253-319)
- `src/ui/keymap.rs`

### 5.3 `default_white` write_texture violates `COPY_BYTES_PER_ROW_ALIGNMENT`

`pipeline.rs:284` (in `GpuTexture::default_white`):
```rust
wgpu::TexelCopyBufferLayout {
    offset: 0,
    bytes_per_row: Some(4),     // <-- not a multiple of 256
    rows_per_image: Some(1),
},
```

This is a latent bug. `wgpu::COPY_BYTES_PER_ROW_ALIGNMENT` is 256. The 1×1 white texture is the only case where the actual row size is 4 bytes, so it would round up to 256. Currently masked because `has_texture` (`flags & 1`) is normally false, so no copy is issued for the default texture. If anyone enables `HAS_TEXTURE` for an untextured mesh (or future code paths auto-fall-back), the GPU may reject the copy or silently corrupt memory.

Fix:
```rust
bytes_per_row: Some(256),  // padded; actual row is 4 bytes
```

**Files involved:**
- `src/inspector/scene3d/pipeline.rs:284-321` (`GpuTexture::default_white`)

### 5.4 `CameraUniform` struct padding mismatch (Rust 180 B vs WGSL 192 B)

The Rust struct is 180 bytes, the WGSL `CameraUniform` rounds up to 192 bytes (roundUp(16) on the largest member boundary). This produces a 12-byte gap at offsets 164–176 between Rust and WGSL. Currently masked by `update_camera` writing `bytes.resize(256, 0)` — the WGSL reads from offsets the Rust struct never touches, so the gap is whatever was in the buffer (always zero from `resize`).

The bug is that `bytemuck::bytes_of(&uniform)` on the Rust side yields 180 B and `from_bytes` round-trips it cleanly, but the WGSL's `inverse_view_proj` at offset 64 (16 mat4) is then followed by `key_light` at offset 128, `ambient` at offset 144, `eye_pos` at offset 160, `flags` at offset 176, `pad` at offset 180 — but the Rust struct's `_pad: [u32; 3]` is only 12 B, and then the layout ends. `bytemuck` won't add trailing padding for `repr(C)`.

Either:
- Add explicit `_wgsl_pad: [u32; 3]` (12 B) to the Rust struct and verify it aligns to 192 B, **or**
- Drop the `_pad: [u32; 3]` in the WGSL shader and rely on the WGSL spec's round-up-to-16 rule (which it already does, so this would actually be cleaner).

Verify with `cargo test --lib camera_uniform_is_pod_and_aligned` (pipeline.rs:889) — it currently asserts `size <= 256` but doesn't catch the mismatch.

**Files involved:**
- `src/inspector/scene3d/pipeline.rs:34-44` (Rust `CameraUniform`)
- `src/inspector/scene3d/shaders/lit.wgsl:1-9` and `grid.wgsl:1-8` (WGSL `CameraUniform`)

### 5.5 `#![allow(dead_code)]` still on the crate root

`src/main.rs:1`. Should be removed or narrowed once Phase 17 is fully wired. The original concern (Phase 17 modules being temporarily unused) is no longer accurate — every `scene3d/*` module is reachable. Narrowing should be done by `#[allow(dead_code)]` on the specific items the compiler actually flags, not as a crate-wide attribute.

**Files involved:**
- `src/main.rs:1`

### 5.6 `package-release.ps1` doesn't pass `RUSTFLAGS="-C codegen-units=16"`

v3.4.0 release build hit `STATUS_STACK_BUFFER_OVERRUN` (0xc0000409) in `harfrust` and `regex-automata` on Rust 1.96. Worked around for v3.4.0 by manually setting `$env:RUSTFLAGS` and then packaging by hand. The script should default to `codegen-units=16` (or higher) so the next release doesn't time out.

Suggested patch:
```powershell
$env:RUSTFLAGS = ($env:RUSTFLAGS ?? "") + " -C codegen-units=16"
& cargo build --release
```

**Files involved:**
- `package-release.ps1:13` (the `cargo build --release` line)

### 5.7 No `rust-toolchain.toml` pinning the build

The stack-overrun in §5.6 is reproducible on Rust 1.96.0-x86_64-pc-windows-msvc. If anyone bumps the toolchain (or `rustup update` runs automatically), the behavior may change. Pinning with a `rust-toolchain.toml` containing:
```toml
[toolchain]
channel = "1.96.0"
components = ["rustfmt", "clippy"]
```
…makes the build environment deterministic and the §5.6 workaround correct-by-default.

**Files involved:**
- `rust-toolchain.toml` (new file, repo root inside `IMGEditor-rs/`)

---

# 6. Code-quality follow-ups (carried over from the v3.4.0 warning-cleanup pass)

The clippy-cleanup commit (`43db780`) suppressed a few warnings with documented
`#[allow]` attributes because the structural fix was out of scope at the time.
These are the remaining items that should be revisited when there's a clean
window for architectural changes.

## 6.1 `package-release.ps1` doesn't pass `RUSTFLAGS="-C codegen-units=16"`

The fix for §5.6 landed in `Cargo.toml` (`[profile.release].codegen-units = 16`),
but the `package-release.ps1` script still calls plain `cargo build --release`
and would silently use the workspace default of `1` if someone reverted the
Cargo.toml change. Hardening the script means future contributors don't have
to remember the Cargo.toml invariant.

Add to `package-release.ps1` before the `cargo build --release` call:
```powershell
$env:RUSTFLAGS = ($env:RUSTFLAGS ?? "") + " -C codegen-units=16"
```

**Files involved:**
- `package-release.ps1:13` (the `cargo build --release` line)

## 6.2 `Cargo.toml` duplicate `main.rs` warning

`Cargo.toml` declares both `[lib]` and `[[bin]]` pointing at `src/main.rs`.
This produces the `file ... found to be present in multiple build targets`
warning on every `cargo check`. The proper fix is to split:
- `src/lib.rs` contains all module declarations + the `pub mod` exports
  (everything currently in `src/main.rs` except `fn main()`).
- `src/bin/imgeditor.rs` contains `fn main()` + the `hide_console_window`
  and `install_panic_hook` helpers, with `use imgeditor::...` paths.
- `Cargo.toml` `[[bin]]` block then points at `src/bin/imgeditor.rs`.

The bench feature already requires `pub mod` exports on every module,
so the library side is already structured correctly — this is mostly a
mechanical split.

**Files involved:**
- `Cargo.toml:13-16` (remove `[[bin]]` block; default `src/bin/imgeditor.rs`
  discovery will take over)
- `src/main.rs` → split into `src/lib.rs` + `src/bin/imgeditor.rs`

## 6.3 `BlockPayload` large enum variant (deferred from §5.1)

The `#[allow(clippy::large_enum_variant)]` on `BlockPayload` (nif.rs:158)
is a temporary workaround. The largest variant is
`BlockPayload::NiTriShapeDataPayload` (multiple `Vec<Vector3>` fields =
~150 bytes inline) which inflates every other variant to the same size.
Boxing the heavy variant would shrink the enum to ~32 bytes (one pointer
plus the discriminant) at the cost of an extra heap deref per match.

The refactor touches 35 `BlockPayload::` match sites across `nif.rs`,
`viewer3d.rs`, and `texture.rs`. Most are `BlockPayload::X(data) => ...`
patterns that become `BlockPayload::X(data) => &data` or
`&data.field` access. A `Cow`-based or `Arc`-based variant might be
cleaner than `Box<>` if multiple consumers read the same payload.

**Files involved:**
- `src/inspector/nif.rs:158` (enum def)
- `src/inspector/nif.rs` (internal match sites)
- `src/inspector/viewer3d.rs` (8 match sites)
- `src/inspector/texture.rs` (4 match sites)

## 6.4 `Message` large enum variant (deferred from §5.1)

The `#[allow(clippy::large_enum_variant)]` on `Message` (app.rs:62) is a
similar story: the `Viewer3dLoadCompleted` variant carries a full `Scene`
(Vec<SceneMesh> with Vec<Vertex>, potentially MB-sized for large NIFs) and
`ExportCompleted` carries a `Vec<String>` of export log lines. Boxing
these would shrink the enum but force a heap allocation on every
`iced::Task::done(Message::…)` — the event-loop hot path.

Profile first; if `Message` allocation shows up as a hotspot, then
box `Viewer3dLoadCompleted` only (the largest by far), keeping `Message`
small enough to inline in the iced task queue.

**Files involved:**
- `src/ui/app.rs:62` (enum def)

## 6.5 Input handling verification (deferred from §5.2)

The mouse-drag, scroll-wheel, shift+drag, and key-shortcut paths in
`Scene3dWidget::update` (viewer3d_widget.rs:253-319) have never been
verified end-to-end in the GUI. Code review found them correct in
isolation, but runtime confirmation is needed:

- Drag in the 3D pane → camera should orbit.
- Scroll wheel → camera should dolly.
- Shift+drag → camera should pan.
- Cursor changes to `Grab` on hover, `Grabbing` while dragging.
- `R` (reset) and `W/T/C/B` (render toggles) only fire when the
  3D pane is focused; the `keymap.rs` already gates this via
  `viewer3d_focused`.

If dragging produces no camera change, add
`dev_logger::breadcrumb("widget update: {event:?}")` calls at the top of
`Scene3dWidget::update` and inside `handle_event` (lines 162-222) to
trace whether Iced's event loop is delivering events to the widget.

**Files involved:**
- `src/ui/viewer3d_widget.rs:253-319` (widget update)
- `src/ui/viewer3d_widget.rs:162-222` (event handler)
- `src/ui/keymap.rs` (focus gating)

---

# 7. Verified-fixed in v3.4.0

- §5.1: fixed in commit `1befdcc` ("Fix black 3D viewport in GUI")
- §5.2: orbit drag fixed (zero-delta anchor bug); orbit inverted to Blender style; view gizmo rotates with camera; remaining input checks deferred to §6.5
- §5.3, §5.4, §5.5: fixed in commit `43db780` ("Clean up clippy warnings,
  fix release build infra")
- §5.6, §5.7: see §6.1 and the rust-toolchain entry above; §5.6 was
  fixed by `Cargo.toml` change in `43db780`, §5.7 still open
- §5.8 (v3.4.0 release blockers): release-cutover items that landed in
  `43db780` or earlier commits — codegen-units bump, stale TODO comment
  removal, all clippy warnings except the Cargo.toml structure issue
