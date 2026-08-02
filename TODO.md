# IMGEditor-rs — Next Objectives

Last shipped: **v3.5.0** (embedded 3D viewer round of fixes — Blender-style orbit, rotating view gizmo, AA grid floor, dimmed under-floor turntable). 153 tests passing.

Next phase: **supporting other game formats**.

---

## 1. Reverse-engineer embedded texture pixel data

`.nft` (NIF texture catalog) entries currently give us **path metadata only** — the actual pixel bytes live inside `NiPixelData` blocks whose header layout is not fully decoded for the 20.3.0.9 stream we see in Bully.

### Known facts

| Fact | Detail |
|------|--------|
| `World.img` has 11980 entries | 5724 NIFs, 4469 NFTs, 550 `.agr`, 493 `.lip`, 488 `.col`, 119 `.cat`, 85 `.ipb`, 52 `.lur`. **Zero `.tga`/`.dds`** entries |
| NFT `NiSourceTexture.use_external == 0` | Implies embedded pixel data, but 0 bytes follow the header fields in practice |
| NFT has `NiPixelData` blocks | e.g. **43891 bytes** of data for `observ4.nft` — the pixels ARE there but the header format is unknown |
| Plausible mipmap entries at offset+28 | `19×5`, repeated 3 times with `data_offset=0` — suggests a variable-length mipmap table before pixel data |
| Pixel data starts somewhere after offset 64+ | Full block is 43891 bytes, so data section is ~43800 bytes |
| No `.tga`/`.dds` on disk at all | Confirmed: neither as loose files nor as named IMG entries. Pixels live **inside NFT NIFs** |
| `.txd` files in `TXD\` dir | Only frontend/UI textures (7 files, ~6 MB total) — not world textures |

### Investigation approaches

**A. Reverse NiPixelData header for 20.3.0.9** — dump raw bytes of a known `NiPixelData` block and reverse the field layout. Block 3 (`observ4.nft`, 43891 B) and block 7 (2907 B) are good fixtures; sentinel `0xFFFFFFFF` at +64 followed by a count (9 / 7) is a strong hint at a mipmap descriptor table starting around +72.

**B. Cross-reference unnamed IMG entries with NFT source paths** — hash unnamed entries and the NFT source paths; a hit records the IMG-entry → texture mapping for non-NIF indexing.

**C. Scan IMG directory for entries containing TGA/DDS magic bytes** — TGA starts with `0x00 0x00 0x02`, DDS with `0x44 0x44 0x53 0x20`. Builds `{magic → entry_name}` so any loose texture blobs surface even without a name match.

**D. Search for NFT source paths as binary strings inside `World.img`** — full paths like `Z:\Bully\Temp\Export\Textures\Scenes\iobserv\PO00_guts_d.tga` may appear as ASCII near their corresponding pixel data.

When this is solved, the embedded viewer can stop falling back to checkerboards for textures with embedded pixels.

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

## 4. Code-quality follow-ups (open from v3.4 cleanup pass)

- **§4.1 `#![allow(dead_code)]` at the crate root** (`src/main.rs`). Removing the allow surfaces ~50 warnings across `archive`, `config`, `editor`, `parser`, `nif`, etc. The reason is structural: `#[cfg(not(feature = "bench"))]` makes those modules private (`mod` instead of `pub mod`), so the compiler can't trace `crate::ui::run_app` through to `ArchiveInfo`, `Editor`, etc. Right fix is one of:
  - Flip the non-bench cfg to `pub mod` (small blast radius, modules become crate-public).
  - Split `src/main.rs` into `src/lib.rs` + `src/bin/imgeditor.rs` (see §4.2) and use the lib path from the bin.
  Either lands in a quiet hour; do this before the next format phase so new modules don't inherit the wide-open allow.
- **§4.2 `Cargo.toml` `main.rs` in both `lib` and `bin` targets** — produces a `file found to be present in multiple build targets` warning on every `cargo check`. Mechanical split into `src/lib.rs` + `src/bin/imgeditor.rs`; the bench feature already requires `pub mod` exports so the lib side is already structured correctly.
- **§4.3 `BlockPayload` large enum variant** (`inspector/nif.rs`). `BlockPayload::NiTriShapeDataPayload` (~150 B inline) inflates every other variant. Boxing the heavy variant would shrink the enum to ~32 B. Touches ~35 match sites across `nif.rs`, `viewer3d.rs`, `texture.rs`. A `Cow`-based or `Arc`-based variant may be cleaner than `Box<>` if multiple consumers read the same payload.
- **§4.4 `Message` large enum variant** (`ui/app.rs`). `Viewer3dLoadCompleted` carries a full `Scene` (potentially MB-sized) and `ExportCompleted` carries a `Vec<String>`. Box only `Viewer3dLoadCompleted` (the largest by far) to keep `Message` small enough to inline in the iced task queue. Profile first to confirm it's a hotspot.

---

## 5. Release infra

- **§5.1 `package-release.ps1` excludes `docs/`** (done in v3.5.0 post-release). Zip contains only `imgeditor.exe`, `README.md`, `LICENSE`.
- **§5.2 `Cargo.toml` `[profile.release].codegen-units = 16`** (done in v3.4.0). Hardens against the `harfrust`/`regex-automata` stack-overrun on Rust 1.96.
- **§5.3 `rust-toolchain.toml`** — not yet committed. Pins `channel = "1.96.0"` so the §5.2 workaround is correct-by-default after any toolchain bump.