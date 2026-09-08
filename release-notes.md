## IMG Editor Plus v3.15.0

### Highlights

- **Archive/export:** added the zero-copy export engine and two-pass sequential save/rebuild path. Large archives stream entry data directly from the memory map, with a buffered fallback when mapping is unavailable.
- **3D and textures:** added synchronized full-panel texture navigation, stable image/grid/UV compositing, NIF UV overlays, and a portable mesh wire overlay for inspecting triangle edges.
- **Preview workflow:** selecting another model or texture now updates the active inspector view directly instead of requiring a manual scene reset.
- **GUI/UX:** added optional row wobble, pulse, and ripple feedback with theme-aware contrast; repaired sorting; made context menus adaptive; and improved tooltip and welcome-screen presentation.
- **Camera controls:** smoothed 3D viewer zoom with a symmetric exponential wheel curve, keeping mouse-wheel and high-resolution pixel scrolling consistent in both directions, and made Blender-style middle-mouse panning slightly more responsive.
- The release passes the full 290-test Rust suite.

### Credits

- GUI, archive workflow, and selection/preview UX improvements: the other IMGEditor agent.
- NIF parsing, texture/UV mapping, 3D viewer navigation/rendering, and the floor-depth fix: Codex.
