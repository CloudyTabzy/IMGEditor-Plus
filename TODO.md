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
