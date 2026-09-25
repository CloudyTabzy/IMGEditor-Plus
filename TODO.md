# IMGEditor-rs — Next Objectives

Last tagged release: **v4.11.0** (shipped 2026-09-16: the complete Bully
AGR animation pipeline — all variants, all-rig binding, textured playback,
model picker, load caching — plus fullscreen texture previews, the
bulk-converter hardening, entry-list comparison, and the Ayu Dark theme).

Next phase: **Bully AGR completion and CAT/LIP/LUR inspection**, followed by
separate GTA animation adapters. The remaining
**DFF/NIF model profiles (Phase D)** and compatibility/asset-hardening items
are tracked below. The shipped compatibility-engine details are recorded in
the release notes and the dedicated documents under `docs/`.

## Unreleased (v4.16.0)

Committed on `master` since v4.11.0. A major implementation under the version
scheme (+5 minor): the fully translated interface in six languages headlines
the release, alongside the features below. `Cargo.toml` and the README heading
are bumped; tag once the release build is made.

- **Merged title bar** — frameless window; the menu bar carries the logo,
  centered window title, and caption buttons (`src/ui/title_bar.rs`). Native
  snap/animations/system menu kept; Win11 snap-layouts flyout unavailable.
- **Recent menu** — recent files moved from File ▸ Open Recent to a root
  "Recent" menu. **Keep menus one level deep:** iced_aw 0.14.1 panics in the
  menu overlay's `operate()` with a closed nested submenu (reported upstream
  as iced-rs/iced_aw#447; see the comment on `App::menubar`).
- **Per-archive game folder** — File ▸ Set game folder… / Reset game folder,
  persisted as `archive_game_root_N` in settings.ini and shown in the Export
  panel; the automatic guess (`parent().parent()`) remains the fallback.
- **Explorer integration** — View ▸ Open .img/.dir from Explorer registers a
  per-user ProgID + OpenWithProgids + RegisteredApplications entry (never the
  `.img` default) and opens Settings ▸ Default apps; an archive path on the
  command line opens on startup (`src/file_association.rs`).
- **Translations (English, Deutsch, Español, Bahasa Indonesia, Português
  (Brasil), Русский)** — Project Fluent via its
  Rust crates; `build/i18n.rs` generates a typed `i18n::t` API from
  `i18n/en.ftl` and validates the translations; root Language menu with a
  "System language" default and instant switching (`i18n/README.md`).
  - [x] Phase 1: infrastructure, all menus, entry context menu, title bar,
    Russian heading font fallback, pseudo-locale for layout checks (debug
    builds started with `IMGEDITOR_PSEUDO_LOCALE=1`).
  - [x] Phase 2: every dialog (about, welcome, unsaved changes, save and
    import checks, folder import, converters, compare, update check,
    validator, sort manager) and the Windows file-dialog titles; dialog
    text wraps instead of clipping, plurals replace "(s)".
  - [x] Phase 3: toasts, archive log lines, inspector/export/3D/texture
    panels, entry table, toolbar and status bar, empty states and pro tips,
    plus text the `compat` module generates (verdict labels, format notes,
    warnings, hint reasons).
  - [x] German, Brazilian Portuguese and Indonesian added (all 726
    messages).
  - [ ] Phase 4: native review of `de`, `es`, `id`, `pt-BR` and `ru`
    (drafts today).
- **Fixes** — Save As no longer deletes the source archive; saves rename the
  new file into place before removing anything; imported files that change
  size mid-save fail the save instead of corrupting the layout; inspector
  headers honor pending edits; Offset/Extension sort keys; crash on shortcuts
  with the File menu open; non-ASCII recent-file labels panicking; missing
  recent files hidden; tab labels no longer wrap; DFF skin and `.db` count
  hardening.

## 0. Entry-list comparison (implemented)

The Alci-compatible name manifest workflow is now shipped. Ctrl+L exports raw
archive storage order with UTF-8/CRLF/no trailing newline, and Ctrl+P opens an
asynchronous, generation-guarded comparison report. The default result remains
exact, case-sensitive, one-directional missing-name containment; the report
adds opt-in case-insensitive matching and archive-only diagnostics, plus
duplicate/blank-line counts and copyable results. See docs/compare-feature.md.

Shipped in v4.5.0 (previously listed here as unreleased):

- Byte-budgeted `quick_cache` LRU for decoded 3D scenes (256 MiB desktop / 64 MiB mobile, keyed by `(archive, generation, entry)`) and texture previews (128 MiB / 32 MiB, keyed by entry index); `ArchiveInfo::generation` invalidates both on entry mutations. `Arc<Scene>` / `Arc<Vec<DecodedTexture>>` values are shared zero-copy with the viewer handle and per-frame lookups.
- Memoized `IdeMap` per game root so only the first 3D load per root walks the directory.
- GPU test stabilization: headless tests share one renderer behind a lock (concurrent `wgpu::Instance` creation raced the driver loaders); suite ~4.8s → ~1.5s.
- Renderer trims: camera UBO written without a per-frame heap copy; depth attachments discarded instead of stored.
- MSAA 4x scene rendering per the Phase 17 plan: the scene pass renders into 4x multisampled color + depth targets and resolves into the 1x scene color texture; the headless renderer stays at 1x for deterministic pixel-diff tests.
- Scalability guard: `validate_scene_for_device` rejects scenes whose largest single mesh buffer exceeds `limits.max_buffer_size` (256 MiB on downlevel devices) with a clear error instead of a raw wgpu validation failure at upload time.
- Xbox 360 Bully IMG v1 support: auto-detected big-endian `.dir`/`.img` pairs,
  validated sector ranges, 24-byte filename preservation, and big-endian
  round-trip saves. See `docs/img-format-validation.md` for the format audit.

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

## 2. GTA III/VC/SA archive hardening (continuing)

The supplied corpora are now exercised by the compatibility scanner (Phase 0 of
v4.5.0), and the independent Rust `gta-img` audit confirmed the v1/v2 wire
layouts. Status is tracked below: completed hardening is checked off, while
remaining compatibility work stays open and testable against real GTA III,
Vice City, and San Andreas archives:

- [x] Preserve IMG v2 `streaming_size` and `archive_size` as separate fields;
  expose a checked effective size for reads and preserve both words on save.
- [x] Validate IMG v1 structure before mapping: complete records, checked
  sector-to-byte conversion, printable names, non-overlapping ranges, and every
  entry range against the image length.
- [x] Complete IMG v2 open-time validation: header/table arithmetic, checked
  sector-to-byte conversion, non-overlapping non-empty ranges, and every entry
  range against the image length.
- [x] Add the Bully Xbox 360 big-endian IMG v1 variant with automatic format
  detection and format-aware save/import/rename behavior.
- [ ] Add XMemDecompress support for the compressed Xbox 360/Wii image variant
  identified by `0x0FF512ED` (the supplied `Scripts.img` is uncompressed).
- [x] Canonicalize v1 input supplied as either `.img` or `.dir`; map the sibling
  `.img` as data, include `.dir` in open/drop filters, and test both entry paths.
- [x] Make extraction path-safe by rejecting absolute paths, separators, parent
  components, alternate data streams, and Windows device names.
- [x] Add a read-only metadata/range diagnostics path for comparing IMGEditor
  Plus with independent readers on real archives.
- [x] Keep local real-archive manifests and hashes untracked; commit only
  synthetic malformed-input fixtures and legally appropriate metadata/byte checks.
- [ ] Consider a bounded `Read` view over mmap/file/imported sources only if
  source-range logic becomes duplicated; retain the existing zero-copy export
  fast path.

The reference’s compact v2 data-start calculation is implemented and covered by
synthetic and optional local-corpus tests. Rebuilt archives still need manual
validation in the matching San Andreas game build before gameplay compatibility
is claimed.

## 3. RenderWare DFF/TXD preview follow-ups

The embedded viewer now supports the common PC RenderWare path used by GTA
III, Vice City, and San Andreas: non-native DFF geometry, frame/atomic
transforms, PC D3D8/D3D9 TXDs, diffuse texture resolution, and the core raster
decoders. See the local GTA RenderWare preview notes
for the format notes and the important caveat that the current local
`Gta_3_img` corpus is labelled as GTA III but contains San Andreas-style
assets.

### Shipped converter hardening (2026-09-16)

- ✅ **Bulk converter selection + honesty** — "Convert selection to target
  dialect" now reads the per-entry selection (the old code read the table's
  display-row list and converted every visible row with nothing selected);
  empty selections are refused, non-TXD selections explain themselves
  ("Only TXD entries can be converted..."), and the confirm dialog states
  how many selected entries stay untouched.
- ✅ **Bulk converter performance** — each TXD is parsed exactly once for
  planning (`plan_conversion_for_texture`, skipping the per-texture preview
  decode and RGBA buffer the bulk dialog never renders) and all planned
  textures are spliced in a single pass per entry
  (`txd_writer::replace_textures`); a unit test proves the batch result is
  byte-identical to sequential single splices.
- ✅ **Fullscreen full-quality texture previews** — the import/replace
  dialogs show a centered, downscaled (max 512px) preview with an expand
  button that opens a fullscreen layer showing the untouched
  full-resolution encoded result on a 75%-opaque backdrop, with the
  texture tab's pan/zoom navigation (scroll zoom, left/middle drag) and
  Esc/button to close; the pixels drop with the dialog state.

Recommended future adaptations, each gated by representative fixtures:

- [x] Add optional local GTA III and Vice City DFF/TXD corpus coverage and an
  untracked per-game manifest policy; legally obtained clean fixtures remain
  outside the repository.
- [ ] Decode native PS2, Xbox, GameCube, and PSP geometry/texture streams
  instead of treating them as PC vertex data.
- [x] Add DFF skin/bone/HAnim data, IFP animation discovery, and optional pose
  playback in the viewer (initial adapter: `parse_dff_rig` preserves frame
  hierarchy + HAnimPLG + SkinPLG; `gta.rs` adapter builds ModelAsset +
  AnimationLibrary; name-based binding via GTA calibration contract;
  context action + dock IFP picker for pack swapping; corpus gates on
  SA ped.ifp + bmyst.dff; DFF texture resolution deferred).
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

## 3A. Shared animation viewer infrastructure (planned — AV phases)

Detailed local-only design: `docs/animation-viewer-infrastructure-plan.md`
(ignored from Git). Build and validate the player with synthetic assets before
depending on AGR/CAT semantics. The same runtime should accept future GTA
DFF/IFP adapters. AV0–AV7 are implemented against the synthetic fixtures; the
real Bully AGR adapter is live (see AV9 below); AV8 and the CAT/IFP adapters
remain pending.

- [x] **AV0 contracts and baselines** — specify coordinate/time/target identity
  rules, analytical rigid/skinned fixtures, and current static preview baselines.
- [x] **AV1 persistent sessions** — separate immutable assets from instance
  pose/camera state; add explicit resource/pose revisions and dependency/request
  tokens without per-frame scene reloads or stale completions.
- [x] **AV2 hierarchy and rigid motion** — retain nodes and mesh attachments;
  bridge existing flattened NIF/DFF/COL scenes with unchanged static output.
- [x] **AV3 sampler and transport** — deterministic clip sampling, play/pause,
  seek/step/range/loop/speed, compact timeline and viewer-driven redraws independent
  of decorative UI animation/toast timing.
- [x] **AV4 reference skinning** — validated mesh/rig bindings, full-influence CPU
  deformation and reusable dynamic vertex buffers; solid and wire render the
  same pose without re-uploading textures or topology.
- [x] **AV5 framing and overlays** — rest/current/clip framing, posed bounds,
  root-motion/in-place/follow policies, skeleton and motion-path diagnostics.
- [x] **AV6 action previews** — compatible clip switching, optional crossfade,
  finite preview sequences and markers; expose unresolved game conditions.
- [x] **AV7 integrated readiness** — verify notifications/scroll, focus/modal
  suspension, scrubbing, resize, archive mutation, cache eviction, bounded
  resource use and performance; hand off a debug synthetic demo for GUI testing.
- [ ] **AV8 GPU skinning (optional)** — optimize only after profiling; maintain
  CPU/GPU parity, device-limit checks and a supported fallback.
- [ ] **AV9 real format adapters** — connect verified Bully clips/rigs/actions,
  then GTA frame/skin/HAnim/IFP data with separate corpus acceptance gates.
  - [x] Bully AGR adapter: variants 999–1004 decode into playback tracks; HXD
    catalog pairing + real clip names; archive and loose `Anim/*.agr` loading;
    validated on Sk8Board, AniBroom, Bike, C_Player
    (`742136d`, `14d6ee1`, `6736790`).
  - [x] Character skinning (`NiSkinInstance`/`NiSkinPartition`) with direct
    `NiSkinData` fallback, CPU LBS, strict palette/weight validation, and
    corpus coverage (`5c67290`, `1593e10`).
  - [ ] Bully CAT/IFP adapters; GTA adapters.
  - [x] Character and mission clip naming via `MAINPED.HXD`: sequence owner
    indices and duplicated AGR chunk sizes provide guarded one-to-one mapping,
    including `C_Player.agr`'s 439 clips without namespace guessing.
  - [x] Reverse-engineer the 1004 packed-key layout from the retail Bully
    executable: predecessor-linked curves, 9-bit normalized time, packed
    quaternion/translation fields, default root, and terminal sentinel. The
    Rust decoder consumes the declared fixed record span and is covered by a
    captured retail key plus real-corpus tests. See
    `bully-probe/FINDINGS.md` §2.15.
  - [x] Reverse-engineer the 1002 packed character stream from the retail
    executable: the declared `count × 8` records form predecessor-linked
    rotation curves, followed by runtime auxiliary records. The shared 9-bit
    normalized time and packed quaternion fields are decoded; default and
    terminal identity roots are excluded. Validated against loose
    `C_Player`/`Grap`/`NPC_Cher` and archive mission AGRs.
  - [x] Decode AGR variants 1000 and 1001 from the retail descriptors:
    20-byte full-float and 12-byte compact linked rotation records, plus
    16-byte/8-byte sparse translation rows. In-range metadata admission,
    padding-safe logical sizes, linked curves and terminal sentinels are
    covered by synthetic and retail C_Player tests.
  - [x] Calibrate character AGR curves across the clip library and recover the
    guarded player `track_i → track_(i + 1)` importer offset for action-only
    root/torso clips (`0308bf9`, `ed06dbe`, `75ea7f6`); the stream never
    animates the `Dummy` placeholder, so wrapper-heavy rigs shift by their
    wrapper count (`JKGirl_Mandy` `+3`; fixes `f349d21`, `40bedff`).
  - [x] Establish that observed character 1002 clips are rotation-only and
    keep floor placement as an explicit constant per-clip viewer policy
    (`43f5660`).
  - [x] Prove the 1004 curve-root-to-NIF joint-name mapping across additional
    same-stem HXD/NIF prop rigs (AsyGate, Armor, Bike and SK8Board); keep the
    numeric fallback for unrelated or unsupported rigs.
  - [x] Preserve variant-1002 auxiliary-tail records as opaque raw bytes while
    excluding them from playback; their runtime lookup semantics remain open.
  - [x] Harden wrapper-heavy character previews: choose the semantic `Dummy`
    root, suppress axis/arrow helper meshes from rendering and floor bounds,
    convert grounding between source/view spaces, frame the grounded motion
    envelope, cover Mandy's right-arm deformation with a real-corpus gate,
    and bind her full `+3` stream (placeholder skipped) with the
    `MANDY_PUKE_LOOP` render GUI-validated 2026-09-15.
  - [x] Corpus-wide AGR audit (`agr_corpus_audit_when_requested`; 433/550
    paired, 280 character-shaped): the placeholder-skip rule holds on every
    pair, with offsets `+1` (193), `+2` (43), `+3` (38) and `+5` (2) set by
    the model's wrapper count. Fixed the `Editable Poly` body-hiding
    regression it found (`wrapper_ped_body_stays_visible_when_available`).
    Honest leftovers: 2-track weapon AGRs (0/2), `V_*` vehicle mission
    groups (~12/35) and nonsensical heuristic pairs — association work, not
    binding work.
  - [x] Textured AGR playback and a dock model picker: AGR meshes carry
    their NIF diffuse texture names and the loader resolves them through the
    static preview's three-tier resolver (so Textured/Alpha blend work in
    the animated path); the animation dock lists the catalog-associated
    models and re-plays the retained AGR clip set on another model with the
    binding badge recomputed per selection.
  - [x] Runtime polish on that path (2026-09-16): the AGR loader keys its
    NFT lookup by the model *stem* (the file-name key probed
    `PLAYER.nif.nft` and left every animated scene gray; gated by
    `agr_textures_resolve_for_models_when_available`, 6/6 PLAYER diffuses);
    the Texture tab follows the picked model while a re-play is active
    (`agr_texture_follow`); decoded AGR+model pairs cache in a byte-budgeted
    `quick_cache` LRU (128 MiB desktop / 32 MiB mobile) so repeat loads and
    model switches install instantly; the animation tab reuses the static
    preview's spinner + "Preparing…" notice while loading; dock dropdowns
    are height-capped and the transport readout is a fixed-width padded
    monospace block that never reflows.
  - [x] Facing conventions established (`facing_conventions_when_available`):
    the static preview and animation rest share one facing (bind front =
    source −Y, toward the default camera), every C_Player clip stays on the
    source ±Y facing line, and the clip data itself splits ~evenly between
    −Y (211) and +Y (228) — played clips may legitimately show the back
    because clips are authored relative to the game's actor node (the
    never-animated `Dummy`). See research record §6.6 and FINDINGS §2.21.
  - [ ] **Low priority — resolve the 22 partial-bound pairs** (from the
    corpus audit; these are association problems, not binding defects):
    - ~14 mission groups mispaired onto same-named props via the stem
      heuristic (`W_Camera`→`WCamera.nif`, `W_FlashLight`, `NPC_Love`,
      `Gift`, `Try_Clothes`, `ErrandCrab`, `ARC3D`, `Area_Asylum`,
      `Area_Infirmary`, `Px_Garb`, `Px_Tree`, `2_S02/S04CharSheets`,
      `2_06MovieTickets`; `1_07_Sk8Board` is the documented honest case) —
      their real target is `PLAYER.nif`/an actor; fix by preferring the
      MAINPED resource target or by LUR-script actor mining, and consider
      refusing absurd pairs outright.
    - 3 vehicle mission groups (`V_Bike` 12/35, `V_COPBIKE` 9/35,
      `V_SCOOTER` 12/35): ped-on-vehicle groups bind only the vehicle
      subset; revisit with the vehicle-side binding evidence.
    - 6 tiny prop edges: `BATON`/`BROCKETL`/`Slingsh` (0/2 — the rest-angle
      admission finds no rest passage in 2-track object clips) and
      `AtcPlank` (24/26). Revisit only if these props matter in practice.

AV0–AV7 establish the shared player; AV8 is not a prerequisite for real-file
work. Bully E0–E4 may proceed alongside it; E5 consumes the verified runtime.
LIP/audio sync, animation authoring and retargeting remain later work.

## 3B. Bully animation/action/script resources (planned — Phase E)

The detailed design is kept in the local-only
`docs/bully-agr-cat-lip-lur-roadmap.md` roadmap (ignored from Git); this compact
checklist is tracked here. AGR and CAT are the first deliverable, LIP is
deliberately deferred, and LUR will eventually receive a dedicated read-only
Script tab rather than being forced into the 3D or texture viewers.

- [x] **E0 corpus profiler and fixtures** — inventory AGR/CAT/LIP/LUR/HXD
  samples from the installed Bully layout, record hashes and byte-order
  hypotheses, and keep legally obtained fixtures outside Git.
  (`bully-probe/`: census, probes 1–37, `FINDINGS.md`, `CHECKPOINT.md`.)
- [ ] **E1 resource detection and metadata** — classify the four extensions by
  validated signatures/structure plus archive context; show size, source,
  confidence, and unsupported/rejected reasons in the inspector.
- [x] **E2 AGR structural parser** — bounded, testable parsing for headers,
  groups/clips, timing, tracks, and references; all six observed record
  variants decode into playback tracks and render (`26124fb`…`742136d`).
- [ ] **E3 CAT action-tree parser** — expose action nodes, paths, parent/child
  relationships, and raw offsets; preserve unknown bytes and avoid guessed
  serializers.
- [ ] **E4 AGR/CAT relationship resolver** — connect logical action names to
  AGR clips and compatible NIF/NFT/rig candidates with evidence-labelled links.
  (AGR↔model pairing via the HXD catalogs is done; CAT links are not.)
- [x] **E5 animation inspector and playback proof** — a validated AGR clip
  plays on a matched model through the shared AV runtime and dock (archive
  entries and loose `Anim/*.agr`; Sk8Board/AniBroom/Bike/C_Player verified).
- [ ] **E6 LIP structural inspection (deferred)** — inspect the record table
  and Speech.bin references only after AGR/CAT are stable; do not claim audio
  decoding or editing until payload semantics are verified.
- [ ] **E7 LUR Script tab (future)** — add a read-only Lua 5.0-era bytecode
  header/prototype/constant/instruction inspector; never execute untrusted LUR
  chunks in the editor.
- [x] **E8 archive-backed preview invalidation** — source and target archive
  generations, scene/texture cache eviction, stale completion rejection, and
  index repair after cross-archive moves are covered by regression tests.
- [ ] **E8b dependency-graph cache** — once CAT/resource graphs exist, key
  parsed AGR/CAT/model summaries by every participating archive identity,
  generation, entry, and parser profile; invalidate the whole graph after a
  move or mutation.
- [ ] **E9 regression and compatibility gates** — add synthetic truncation,
  count/offset overflow, endian, alias-resolution, and cross-resource tests;
  validate against representative World, Act, Scripts, and Anim samples.
- [ ] **E10 editing decision** — keep all four formats read-only until a
  lossless round-trip contract exists for each format and its dependencies.

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

## 5. Quality-of-life improvements

- ✅ **Ayu Dark theme** — eighth theme mode: Ayu's dark palette
  (bg #0B0E14, panels #131722) tuned toward a deep terracotta accent
  (#F29A4B) per the user's reference, with Ayu green/yellow/red/cyan
  semantic ramps; persisted as "Ayu Dark"/"Ayu" in settings.ini.
  `design_for_theme` now resolves custom themes by name to their own
  token sets, so Everforest and GitHub Dark style closures match
  `App::design` too.
- **Cache parsed NFT catalogs** — partially done. The per-game-root `IdeMap` is memoized and decoded texture pixels are cached in the `quick_cache` LRU, so repeated 3D loads skip the directory walk and pixel decode. The catalog parse itself (`parse_nft_catalog_bytes`) still runs per NIF load; parked because parsing is cheap next to decode.
- **Game root path override** — done (unreleased). Per archive rather than global, since III/VC/SA/Bully archives are routinely open side by side: `Config::game_root_for` returns the File ▸ Set game folder… choice, else the `parent().parent()` guess; all 3D/IDE lookups go through it.
- **Clear old temp files on startup** — done. `main.rs::clean_temp_preview` sweeps `%TEMP%\IMGEditor\preview\` best-effort on launch; locked entries are skipped so it never blocks startup.
- **Windows file-association registration** — done (unreleased). HKCU only, via `winreg`; Windows 8+ lets only the user pick a default, so the app registers itself as a candidate and opens its Default apps page. Each Explorer open starts a new instance (no single-instance forwarding yet).

---

## 6. Code-quality follow-ups

- **§4.1 and §4.2 completed** — the reusable modules now live in `src/lib.rs`, the executable uses `src/main.rs`, and the crate-root `dead_code` suppression/duplicate-target issue described by the old notes no longer applies.
- **§4.3 `BlockPayload` large enum variant** — done (unreleased). Clippy's
  size report showed the outlier was `NiTexturingProperty` (784 bytes of
  inline `TexDesc` slots), not `NiTriShapeData`; boxing that one variant
  shrank every parsed block threefold.
- **§4.4 `Message` large enum variant** — resolved; the lint no longer fires
  because the decoded scene already travels as `Arc<Scene>`. The stale
  `allow` is gone.
- **Baseline (unreleased):** `cargo clippy --all-targets` reports nothing on
  Rust 1.96, the crate is `rustfmt`-formatted (the formatting commit is in
  `.git-blame-ignore-revs`), and the long argument lists behind
  `too_many_arguments` allows became `NifWalk`, `ReplaceJob` and the 3D
  viewer's `FrameView`/`SceneUploadKey`. The remaining allows are deliberate
  (DXT5 alpha table, `ColorScale::new`, ordered binary reads). Fresh clones
  build again: `Cargo.toml` no longer declares the gitignored benchmark
  examples.
---

## 7. Release infra

- **§5.1 `package-release.ps1` excludes `docs/`** (done in v3.5.0 post-release). Zip contains only `imgeditor.exe`, `README.md`, `LICENSE`.
- **§5.2 `Cargo.toml` `[profile.release].codegen-units = 16`** (done in v3.4.0). Hardens against the `harfrust`/`regex-automata` stack-overrun on Rust 1.96.
- **§5.3 `rust-toolchain.toml`** — completed. The project pins `channel = "1.96.0"` so the §5.2 workaround is correct-by-default after any toolchain bump.
