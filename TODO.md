# IMGEditor-rs — Next Objectives

Last shipped: **v3.15.0** (smoother 3D camera zoom/panning, synchronized texture overlays, and continued archive/UI refinements). 294 tests passing.

Next phase: **supporting other game formats**.

On master (post-v3.15.0, unreleased):

- Byte-budgeted `quick_cache` LRU for decoded 3D scenes (256 MiB desktop / 64 MiB mobile, keyed by `(archive, generation, entry)`) and texture previews (128 MiB / 32 MiB, keyed by entry index); `ArchiveInfo::generation` invalidates both on entry mutations. `Arc<Scene>` / `Arc<Vec<DecodedTexture>>` values are shared zero-copy with the viewer handle and per-frame lookups.
- Memoized `IdeMap` per game root so only the first 3D load per root walks the directory.
- GPU test stabilization: headless tests share one renderer behind a lock (concurrent `wgpu::Instance` creation raced the driver loaders); suite ~4.8s → ~1.5s.
- Renderer trims: camera UBO written without a per-frame heap copy; depth attachments discarded instead of stored.
- MSAA 4x scene rendering per the Phase 17 plan: the scene pass renders into 4x multisampled color + depth targets and resolves into the 1x scene color texture; the headless renderer stays at 1x for deterministic pixel-diff tests.
- Scalability guard: `validate_scene_for_device` rejects scenes whose largest single mesh buffer exceeds `limits.max_buffer_size` (256 MiB on downlevel devices) with a clear error instead of a raw wgpu validation failure at upload time.

---

## 1. Embedded texture pixel data (implemented)

The Bully 20.3.0.9 `NiPixelData` path is now implemented in
`src/inspector/texture.rs` and covered by regression tests. The viewer and
texture export path can resolve embedded NFT pixels instead of relying only on
source-path metadata.

### Current behavior

- Parses the explicit Bully mipmap header before using the legacy size guesser.
- Maps Bully pixel formats 4, 5, and 6 to DXT1/BC1, DXT3/BC2, and DXT5/BC3.
- Resolves textures from loose files and IMG archive entries, including per-mesh
  NIF texture references.
- Supports archive-backed TGA/DDS/PNG sources and embedded compressed payloads.

### Remaining texture edge cases

- Palette-based and other non-DXT Bully payloads still need installed-game
  fixtures before they can be supported safely.
- Unusual mipmap layouts should be added as fixtures if encountered in the wild.

---

## 2. Other game formats (next major phase)

Pick a target before scoping the work. Candidate families:

- **DIR + IMG splits** — GTA III/VC already in scope (v1). Worth re-checking whether SA's split archives behave the same way.
- **GTA IV / V `.rpf` containers** — Rage PakFormat; binary table-of-contents, key-encrypted on console but plaintext on PC. Requires its own parser module.
- **Non-Rockstar formats** — REDengine `.bundle` (Witcher, CP2077), Unreal `.pak`, idTech `.pk4`/`.pak`. Different scope each; pick based on community demand.

### Extension seam (current shape)

`src/parser/iparser.rs`:

```rust
pub trait ImgParser {
    fn open(&self, archive: &mut ArchiveInfo) -> anyhow::Result<()>;
    fn export_entry(&self, archive: &ArchiveInfo, entry: &EntryInfo, output_path: &Path) -> anyhow::Result<()>;
    fn import_entry(archive: &mut ArchiveInfo, path: &Path, replace: bool) -> anyhow::Result<()>;
    fn save(&self, archive: &mut ArchiveInfo, output_path: &Path, remove_existing: bool) -> anyhow::Result<()>;
    fn version_text(&self) -> &'static str;
    fn is_valid(&self, path: &Path) -> bool;
}
```

This is fine for **another IMG version** (`PcV3Parser` etc.). For a brand-new container family (`.rpf`, `.bundle`, `.pak`) we will likely need to abstract entry addressing — `parser/mod.rs` currently hard-codes `SECTOR_SIZE = 2048` and the `sector`/`offset` fields on `EntryInfo` are sector-relative. The least-invasive change is to introduce a `ContainerLayout` trait next to `ImgParser` that exposes entry addressing; keep `EntryInfo` as-is for the IMG family and add a parallel struct only if a non-sector container actually lands.

### Detection dispatch

`parser::detect_version` walks `PcV1Parser.is_valid` then `PcV2Parser.is_valid`. New container families need a separate entry point (e.g. `detect_container(path) -> ContainerKind`) so the IMG and non-IMG worlds don't share a `is_valid` ordering. The format switcher in `ui/view.rs` is keyed on file extension today — extend it with the container kind from detection.

---

## 3. Quality-of-life improvements

- **Cache parsed NFT catalogs** — partially done. The per-game-root `IdeMap` is memoized and decoded texture pixels are cached in the `quick_cache` LRU, so repeated 3D loads skip the directory walk and pixel decode. The catalog parse itself (`parse_nft_catalog_bytes`) still runs per NIF load; parked because parsing is cheap next to decode.
- **Game root path override** — not started. `Config` has no `game_root` field; the root is still derived from the archive path (`parent().parent()`). A CLI flag or settings field is the planned seam.
- **Clear old temp files on startup** — done. `main.rs::clean_temp_preview` sweeps `%TEMP%\IMGEditor\preview\` best-effort on launch; locked entries are skipped so it never blocks startup.
- **Windows file-association registration** — not started. No registry/`ftype` code exists yet.

---

## 4. Code-quality follow-ups

- **§4.1 and §4.2 completed** — the reusable modules now live in `src/lib.rs`, the executable uses `src/main.rs`, and the crate-root `dead_code` suppression/duplicate-target issue described by the old notes no longer applies.
- **§4.3 `BlockPayload` large enum variant** (`inspector/nif.rs`). `BlockPayload::NiTriShapeDataPayload` (~150 B inline) inflates every other variant. Boxing the heavy variant would shrink the enum to ~32 B. Touches ~35 match sites across `nif.rs`, `viewer3d.rs`, `texture.rs`. A `Cow`-based or `Arc`-based variant may be cleaner than `Box<>` if multiple consumers read the same payload.
- **§4.4 `Message` large enum variant** (`ui/app.rs`). `Viewer3dLoadCompleted` carries a full `Scene` (potentially MB-sized) and `ExportCompleted` carries a `Vec<String>`. Box only `Viewer3dLoadCompleted` (the largest by far) to keep `Message` small enough to inline in the iced task queue. Profile first to confirm it's a hotspot.

---

## 5. Release infra

- **§5.1 `package-release.ps1` excludes `docs/`** (done in v3.5.0 post-release). Zip contains only `imgeditor.exe`, `README.md`, `LICENSE`.
- **§5.2 `Cargo.toml` `[profile.release].codegen-units = 16`** (done in v3.4.0). Hardens against the `harfrust`/`regex-automata` stack-overrun on Rust 1.96.
- **§5.3 `rust-toolchain.toml`** — completed. The project pins `channel = "1.96.0"` so the §5.2 workaround is correct-by-default after any toolchain bump.
