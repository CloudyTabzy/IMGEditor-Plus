# IMGEditor-rs — Next Objectives

Last shipped: **v4.1.0** (search prediction, themed UI refinement, and continued archive/3D viewer improvements).

Next phase: **validating and hardening GTA III/VC/SA IMG and RenderWare support** before
considering other container families.

On master (post-v3.16.0, unreleased):

- Byte-budgeted `quick_cache` LRU for decoded 3D scenes (256 MiB desktop / 64 MiB mobile, keyed by `(archive, generation, entry)`) and texture previews (128 MiB / 32 MiB, keyed by entry index); `ArchiveInfo::generation` invalidates both on entry mutations. `Arc<Scene>` / `Arc<Vec<DecodedTexture>>` values are shared zero-copy with the viewer handle and per-frame lookups.
- Memoized `IdeMap` per game root so only the first 3D load per root walks the directory.
- GPU test stabilization: headless tests share one renderer behind a lock (concurrent `wgpu::Instance` creation raced the driver loaders); suite ~4.8s → ~1.5s.
- Renderer trims: camera UBO written without a per-frame heap copy; depth attachments discarded instead of stored.
- MSAA 4x scene rendering per the Phase 17 plan: the scene pass renders into 4x multisampled color + depth targets and resolves into the 1x scene color texture; the headless renderer stays at 1x for deterministic pixel-diff tests.
- Scalability guard: `validate_scene_for_device` rejects scenes whose largest single mesh buffer exceeds `limits.max_buffer_size` (256 MiB on downlevel devices) with a clear error instead of a raw wgpu validation failure at upload time.
- Xbox 360 Bully IMG v1 support: auto-detected big-endian `.dir`/`.img` pairs,
  validated sector ranges, 24-byte filename preservation, and big-endian
  round-trip saves. See the local `gta-img` reference-audit notes.

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

## 2. GTA III/VC/SA archive hardening (next major phase)

The independent Rust `gta-img` audit
confirmed the v1/v2 wire layouts and identified the following safe follow-ups.
These should be completed against real, legally obtained GTA III, Vice City, and
San Andreas archives before we expand the supported container scope.

- [ ] Preserve IMG v2 `streaming_size` and `archive_size` as separate fields;
  expose a checked effective size for reads and preserve both words on save.
- [x] Validate IMG v1 structure before mapping: complete records, checked
  sector-to-byte conversion, printable names, non-overlapping ranges, and every
  entry range against the image length.
- [ ] Complete IMG v2 open-time validation: header/table arithmetic, checked
  sector-to-byte conversion, and every entry range against the image length.
- [x] Add the Bully Xbox 360 big-endian IMG v1 variant with automatic format
  detection and format-aware save/import/rename behavior.
- [ ] Add XMemDecompress support for the compressed Xbox 360/Wii image variant
  identified by `0x0FF512ED` (the supplied `Scripts.img` is uncompressed).
- [ ] Canonicalize v1 input supplied as either `.img` or `.dir`; map the sibling
  `.img` as data, include `.dir` in open/drop filters, and test both entry paths.
- [ ] Make extraction path-safe by rejecting absolute paths, separators, and
  parent components, or by proving normalized output stays inside the destination.
- [ ] Add a read-only metadata/range diagnostics path for comparing IMGEditor
  Plus with independent readers on real archives.
- [ ] Keep local real-archive manifests and hashes untracked; commit only
  synthetic malformed-input fixtures and legally appropriate metadata/byte checks.
- [ ] Consider a bounded `Read` view over mmap/file/imported sources only if
  source-range logic becomes duplicated; retain the existing zero-copy export
  fast path.

The reference’s compact v2 data-start calculation is intentionally not a target
for adoption: retain the current `0x300000` rebuild convention until real San
Andreas validation confirms a different layout is safe.

## 3. RenderWare DFF/TXD preview follow-ups

The embedded viewer now supports the common PC RenderWare path used by GTA
III, Vice City, and San Andreas: non-native DFF geometry, frame/atomic
transforms, PC D3D8/D3D9 TXDs, diffuse texture resolution, and the core raster
decoders. See the local GTA RenderWare preview notes
for the format notes and the important caveat that the current local
`Gta_3_img` corpus is labelled as GTA III but contains San Andreas-style
assets.

Recommended future adaptations, each gated by representative fixtures:

- [ ] Add legally obtained GTA III and Vice City DFF/TXD fixtures and an
  untracked per-game manifest of names, dimensions, raster formats, and hashes.
- [ ] Decode native PS2, Xbox, GameCube, and PSP geometry/texture streams
  instead of treating them as PC vertex data.
- [ ] Add DFF skin/bone/HAnim data, IFP animation discovery, and optional pose
  playback in the viewer.
- [ ] Preserve and preview additional UV sets, prelit vertex colors, multiple
  material properties, and material-split geometry where the source needs it.
- [ ] Support common RenderWare effects such as MatFX, dual/environment
  textures, bump/specular masks, and material colors without flattening them
  into an inaccurate diffuse-only result.
- [ ] Cover Bin Mesh/native geometry variants and unusual TXD palette, mipmap,
  and stream layouts with focused decoder tests.
- [ ] Replace the bounded fallback TXD scan with a generation-aware reverse
  texture index once archive sizes or load latency justify the extra state.
- [ ] Decide whether DFF/TXD editing and serialization belong in IMGEditor Plus;
  if so, add round-trip tests before exposing write actions in the UI.

The current PC inspection path should remain the safe default: unsupported
platform payloads must be reported clearly rather than guessed into malformed
geometry or colors.

## 4. Other game formats (deferred)

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

## 4. Quality-of-life improvements

- **Cache parsed NFT catalogs** — partially done. The per-game-root `IdeMap` is memoized and decoded texture pixels are cached in the `quick_cache` LRU, so repeated 3D loads skip the directory walk and pixel decode. The catalog parse itself (`parse_nft_catalog_bytes`) still runs per NIF load; parked because parsing is cheap next to decode.
- **Game root path override** — not started. `Config` has no `game_root` field; the root is still derived from the archive path (`parent().parent()`). A CLI flag or settings field is the planned seam.
- **Clear old temp files on startup** — done. `main.rs::clean_temp_preview` sweeps `%TEMP%\IMGEditor\preview\` best-effort on launch; locked entries are skipped so it never blocks startup.
- **Windows file-association registration** — not started. No registry/`ftype` code exists yet.

---

## 5. Code-quality follow-ups

- **§4.1 and §4.2 completed** — the reusable modules now live in `src/lib.rs`, the executable uses `src/main.rs`, and the crate-root `dead_code` suppression/duplicate-target issue described by the old notes no longer applies.
- **§4.3 `BlockPayload` large enum variant** (`inspector/nif.rs`). `BlockPayload::NiTriShapeDataPayload` (~150 B inline) inflates every other variant. Boxing the heavy variant would shrink the enum to ~32 B. Touches ~35 match sites across `nif.rs`, `viewer3d.rs`, `texture.rs`. A `Cow`-based or `Arc`-based variant may be cleaner than `Box<>` if multiple consumers read the same payload.
- **§4.4 `Message` large enum variant** (`ui/app.rs`). `Viewer3dLoadCompleted` carries a full `Scene` (potentially MB-sized) and `ExportCompleted` carries a `Vec<String>`. Box only `Viewer3dLoadCompleted` (the largest by far) to keep `Message` small enough to inline in the iced task queue. Profile first to confirm it's a hotspot.

---

## 6. Release infra

- **§5.1 `package-release.ps1` excludes `docs/`** (done in v3.5.0 post-release). Zip contains only `imgeditor.exe`, `README.md`, `LICENSE`.
- **§5.2 `Cargo.toml` `[profile.release].codegen-units = 16`** (done in v3.4.0). Hardens against the `harfrust`/`regex-automata` stack-overrun on Rust 1.96.
- **§5.3 `rust-toolchain.toml`** — completed. The project pins `channel = "1.96.0"` so the §5.2 workaround is correct-by-default after any toolchain bump.
