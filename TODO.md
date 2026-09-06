# IMGEditor-rs — Next Objectives

Last shipped: **v3.12.0** (3D reference-grid depth fix, Bully NIF texture resolution, and safer textured 3D previews). 253 tests passing.

Next phase: **supporting other game formats**.

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

- Cache parsed NFT catalogs (the same NFT serves many NIFs)
- Add a CLI or GUI option to specify the game root path (instead of deriving from archive path)
- Clear old temp files on startup (`%TEMP%\IMGEditor\preview\`)
- File-association registration on Windows (right-click → open with IMG Editor Plus)

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
