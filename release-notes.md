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
