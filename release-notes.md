## IMG Editor Plus v3.8.0

### Highlights

- Hardened the embedded 3D viewer's wgpu resource lifecycle to avoid per-frame mesh and texture churn.
- Added scene-size, texture, index, viewport, and device-limit guards before GPU allocation.
- Added graceful GPU out-of-memory/device-loss reporting and a recovery action in the GUI.
- Improved headless rendering portability with conditional wireframe support and aligned readback buffers.
- Continued archive packing, folder import, and GUI workflow improvements from the prior agent contribution.

### Credits

- GUI and archive workflow improvements: the other IMGEditor agent.
- 3D/GPU resource lifecycle hardening: Codex.
