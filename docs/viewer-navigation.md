# 3D navigation gizmo

The top-right navigation disc shows the model's source X (red), Y (green),
and Z (blue) axes. Filled circles denote positive directions; outlined circles
with minus signs denote negative directions. Circles are drawn and hit-tested
from back to front so the visible target wins when axes overlap.

- Click a circle to enter orthographic projection looking toward the target from that axis.
- Click the facing circle again to view the opposite side.
- Drag the gizmo or the scene with LMB to orbit. Orbiting restores perspective.
- Click `PERSP` / `ORTHO` below the disc to toggle projection at the current angle.
- MMB pans; the wheel zooms in either projection. `Reset view` restores fitted perspective.

Projection changes preserve scale at the orbit target, target position, and zoom.
Orthographic views remove perspective foreshortening. Top/bottom panning uses
an analytic camera basis to avoid the singularity of a world-up look-at camera.
Axis selection does not modify model coordinates or archive data.

## Implementation

`scene3d/navigation.rs` generates the six projected positions for both GPU drawing
and CPU hit testing. Physical rendering uses the window DPI scale; pointer input
uses the same layout in logical pixels. The gizmo maintains a square footprint
and scales down in very small panes.

Source axes pass through `BaseOrientation::to_yup_matrix` before projection.
For Z-up models, source Z is renderer Y, and source Y is renderer -Z. The floor's
axis colors use the same source mapping. Internal renderer coordinates should
never be relabeled as source coordinates without this conversion.

`CameraUniform` is 384 bytes: the established 256-byte camera prefix plus a
128-byte navigation block. Rust buffer allocation and binding sizes follow the
struct size. The WGSL grid and navigation declarations share this layout.
The gizmo renders after the model, without depth writes, into the same offscreen
MSAA target. Its circles, strokes, and letters are original procedural drawings;
no image assets, external fonts, or new dependencies are required.

## Verification and reference

Regression coverage checks six signed views under all three base orientations,
finite pole matrices, depth-independent orthographic scale, panning/zooming,
opposite-side selection, DPI hit testing, click versus drag handling, and GPU
highlight alignment. GPU tests write `target/navigation-*.png` for inspection.
Optional GTA water-tank and Bully lamp fixtures also render perspective, top,
and front views when available locally; these assets are not distributed.

Interaction reference: [Blender 4.3 navigation manual](https://docs.blender.org/manual/en/4.3/editors/3dview/navigate/introduction.html).
This is an independent Rust/WGSL implementation, not copied Blender source.
