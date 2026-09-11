## IMG Editor Plus v4.5.0

### Highlights

- **Asset compatibility engine (validator):** retail-verified profiles for GTA III, Vice City, San Andreas and Bully. Set the game an archive is for - or accept an advisory content hint - and every texture is checked against that engine's dialect: entry rows are tinted by verdict with an icon legend, the target is remembered per archive path, and imported files get a pre-flight format check before touching the archive.
- **Texture converter:** import **PNG / DDS / BMP / TGA** and re-encode into the target's native language - DXT1/3/5, PAL8/PAL4, 888/8888/565/1555/4444 - with full mip chains. A plan dialog shows the chosen format, honest loss warnings (alpha, compression, quantization) and a **true preview decoded back from the encoded bytes** before anything is written.
- **Import image as TXD:** author a brand-new texture entry from any image, with a name and format picker.
- **Convert selection to target dialect:** bulk re-encode selected TXDs (for example, move content between games) with a before/after plan.
- **Save check + lossless repair:** saving runs a pre-save report (native / convertible / incompatible / unknown counts, container mismatches, broken headers). *Fix & Save* applies header-only repairs in the background.
- **Per-texture verdicts:** the Texture tab shows a compatibility chip per texture with the classifier's notes, plus a *PAL8-ready* badge when the pixels fit an 8-bit palette.
- **Byte-preserving edits:** replacing or converting a texture re-serializes only that texture's section; every other entry and all container metadata stay verbatim. Untouched retail archives are never rewritten.
- **Correctness fixes:** Vice City's texture dialect is now classified as DXT1/DXT3 (its "565/4444" headers were DXT data with stale labels), DXT5 alpha decoding follows the spec, and generated entries show their real byte size.
- **New crates:** `texpresso` (MIT) for BC1/BC2/BC3 compression and `quantette` (MIT or Apache-2.0) for Wu + k-means palette quantization in Oklab.

## IMG Editor Plus v4.2.0

### Highlights

- **Bully Xbox 360 IMG v1:** structural auto-detection for big-endian `.dir`/`.img` pairs, 24-byte filename preservation, export/import/rename support, and big-endian round-trip saves. The optional XMemDecompress image variant is documented as a future task.
- **RenderWare texture correctness — fixed garbled "888" textures in modded archives:** D3D9-platform rasters claiming 24-bit "888" are stored as 32-bit X8R8G8B8 (D3D9 has no 24-bit texture format); 355 textures in the investigated archive decoded as checkerboard noise and now render correctly. The decoder cross-checks the D3D format word, the depth byte, and the mip data length so mixed-import rasters identify themselves, and format labels distinguish "X8R8G8B8 (888 RGB, 32bpp)" from true 24-bit. The full case study and provenance forensics live in `docs/renderware-format-mismatches.md`.
- **Middle-click autoscroll (reworked to native):** the entry table uses Iced's native `Scrollable::auto_scroll` controller, keeping the table's scroll state and its circular origin-anchored indicator together. It no longer replaces the table's widget tree on MMB, preventing the former snap-to-top failure. The one-time toast confirms activation; clicking, middle-clicking, right-clicking, scrolling, or pressing a key stops in place.
- **Bounded autoscroll momentum:** `View → Autoscroll momentum` enables a short, physically damped glide after fast scrolling returns to the native neutral zone. It samples speed once, caps both initial velocity and total travel, and cancels instantly on new input or renewed cursor movement — no speed multiplier can accumulate while held at an edge.
- **Selection & sorting polish:** right-click selection toggle (`context_selection_accumulates`, default on) with a "+N more selected" context-menu count; state-aware sort-header tooltips including the archive's primary type label; literal file-type mode (`literal_file_types`) with curated Bully labels (.nif Model, .idb Definition, .ipb Placement, .db Data); centered sort-manager dialog; refined themed sort controls and entry rows.
- **Search ergonomics:** inline clear button in the search box, and adopting a prediction moves the caret to the end of the committed name.
- **Archive tab closing:** middle-click closes an archive tab, alongside the existing close button and shortcut.
- **Robustness:** scroll position survives notifications; rapid repeated loads of the same entry (3D model, TXD/NFT textures) are single-flighted through quick_cache placeholder guards — duplicates skip, results publish atomically, failures release the slot for retry, and results decoded after an entry-list change are discarded instead of polluting the cache; inspector summaries moved into a byte-budgeted LRU (16 MiB desktop / 4 MiB mobile); clippy is fully clean.
- **Documentation:** `docs/renderware-format-mismatches.md` (format-vs-storage mismatch case study and cross-check discipline), `docs/asset-compatibility-engine.md` (profiles, validator, and loss-ladder design for asset compatibility), `docs/renderware-gta-preview.md` (corpus forensics cross-reference), and AGENTS.md framework-research guidance.

## IMG Editor Plus v4.1.0

### Highlights

- **Fuzzy search:** the entry filter now uses a scored subsequence matcher tuned for game file names — scattered initials (`pld`) and partial names match, results rank by relevance (prefix and word-boundary bonuses, consecutive runs, gap penalties, shorter/denser names first), and the sort chain breaks ties. When nothing matches, a Jaro-Winkler typo fallback (via the feature-gated, zero-dependency `fuzzt` crate) still surfaces near-misses like `policastr` → `police_car.dff`.
- **Search prediction dropdown:** a floating overlay anchored under the search box lists the top 8 matches while typing. Click a row (or `↑`/`↓` + `Enter`) to adopt the full name, select the entry like a row click, and scroll to it; `Esc` dismisses; a "Did you mean …" suggestion appears for queries with no subsequence match. The overlay is rendered through a window-level `Float` over an always-stable widget tree, so toggling it never drops the text input's focus.
- **Hideable search bar:** `View → Search bar` hides the strip to free vertical space for the table and info panels; hiding clears any active filter so entries are never silently hidden, and `Ctrl+F` reveals and focuses it again. The preference persists in `settings.ini`.

## IMG Editor Plus v4.0.0

### Highlights

- **GTA IMG content rendering:** entries inside IMG v1/v2 archives now open natively in the in-app viewers. PC RenderWare DFF models render with frame/atomic transforms, material splits, and TXD diffuse texture resolution (GTA III/VC/SA-style assets); Bully NIF models and NFT texture catalogs keep their dedicated pipelines; COL collision meshes remain available. The Export / 3D viewer / Texture viewer panes are switchable with the `1`/`2`/`3` keys (numpad supported).
- **UI overhaul:** a full design-token refresh brings Affinity-style shade hierarchy, hairline chrome dividers, an inset log well, and a pane layout with custom split styling across all six themes.
- **Motion effects:** floating toast snackbar (slide + fade lifecycle), progress-bar shimmer, breathing empty-state hero, click ripples, selection pulses, and icon micro-motion — all individually toggleable under `View → Motion effects`.
- **Keyboard shortcuts expanded and fixed:** `Ctrl+D` deselects (`Esc` still works), `Ctrl+X` deletes alongside `Del`, `Ctrl+F` focuses the search box. Under the hood, shortcut focus detection no longer deadlocks when the search or rename input is absent from the widget tree — shortcuts now work reliably from the empty welcome screen, and they are suppressed while typing, renaming, or when a dialog is open.

## IMG Editor Plus v3.16.0

### Highlights

- **Archive/export:** added the zero-copy export engine and two-pass sequential save/rebuild path. Large archives stream entry data directly from the memory map, with a buffered fallback when mapping is unavailable.
- **3D and textures:** added synchronized full-panel texture navigation, stable image/grid/UV compositing, NIF UV overlays, and a portable mesh wire overlay for inspecting triangle edges.
- **GTA RenderWare preview:** added in-app PC DFF parsing with frame/atomic transforms, material-aware mesh splitting, D3D8/D3D9 TXD decoding, and archive-backed diffuse texture resolution for GTA III/VC/SA-style assets.
- **Preview workflow:** selecting another model or texture now updates the active inspector view directly instead of requiring a manual scene reset.
- **GUI/UX:** added optional row wobble, pulse, and ripple feedback with theme-aware contrast; repaired sorting; made context menus adaptive; and improved tooltip and welcome-screen presentation.
- **Camera controls:** smoothed 3D viewer zoom with a symmetric exponential wheel curve, keeping mouse-wheel and high-resolution pixel scrolling consistent in both directions, and made Blender-style middle-mouse panning slightly more responsive.
- The release passes the full 305-test Rust suite (with one intentionally ignored doctest).

### Credits

- GUI, archive workflow, and selection/preview UX improvements: the other IMGEditor agent.
- NIF and RenderWare DFF/TXD parsing, texture/UV mapping, 3D viewer navigation/rendering, and the floor-depth fix: Codex.
