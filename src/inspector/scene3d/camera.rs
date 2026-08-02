//! Orbit camera and base-orientation math.
//!
//! The camera state lives entirely on the CPU side; the widget just
//! snapshots `OrbitCamera::view_proj()` once per frame and uploads the
//! resulting `glam::Mat4` as a uniform buffer.
//!
//! Convention: the camera is positioned at
//! `target + eye_offset(yaw, pitch, distance)` and looks at `target`,
//! with `+Y` of the orbit frame pointing up regardless of the world's
//! base orientation. World orientation is applied at decode time
//! ([`crate::inspector::scene3d::decode`]), so the renderer always
//! operates in this Y-up camera frame.

use glam::Mat4;

use crate::inspector::scene3d::mesh::Aabb;

/// Smallest allowed eye height above the world floor plane (Y=0).
const MIN_EYE_HEIGHT: f32 = 0.05;

/// Base orientation of the source coordinate system. Applied at parse
/// time so the GPU pipeline never has to branch on this.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BaseOrientation {
    /// +Y up — default for modern engines and modding tools.
    #[default]
    Yup,
    /// +Z up — Gamebryo / Bully's on-disk convention.
    Zup,
    /// +X up.
    Xup,
}

impl BaseOrientation {
    /// Cycle in the order used by the toolbar `B` key.
    pub fn next(self) -> Self {
        match self {
            BaseOrientation::Yup => BaseOrientation::Zup,
            BaseOrientation::Zup => BaseOrientation::Xup,
            BaseOrientation::Xup => BaseOrientation::Yup,
        }
    }

    /// Returns the 4x4 matrix that transforms a vertex in the source
    /// orientation into a Y-up camera frame. The result is meant to be
    /// applied via `vertex_pos = MAT * vertex_pos` on the CPU before
    /// the vertex buffer is built.
    pub fn to_yup_matrix(self) -> Mat4 {
        match self {
            BaseOrientation::Yup => Mat4::IDENTITY,
            // Z-up -> Y-up: source Z (up) becomes camera Y (up), source
            // Y maps to camera -Z so the model's "depth" axis ends up
            // facing the default orbit camera.
            BaseOrientation::Zup => Mat4::from_cols_array_2d(&[
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 0.0, -1.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ]),
            // X-up -> Y-up: source X (up) becomes camera Y (up).
            BaseOrientation::Xup => Mat4::from_cols_array_2d(&[
                [0.0, 1.0, 0.0, 0.0],
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ]),
        }
    }
}

/// Current framebuffer size. The camera refreshes its perspective when
/// this changes; the widget invalidates and re-draws.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
}

impl Viewport {
    pub fn aspect(&self) -> f32 {
        if self.height == 0 {
            1.0
        } else {
            self.width as f32 / self.height as f32
        }
    }

    pub fn is_zero_area(&self) -> bool {
        self.width == 0 || self.height == 0
    }
}

/// Orbit camera around a target point on a sphere of `distance` radius.
#[derive(Clone, Debug)]
pub struct OrbitCamera {
    pub target: [f32; 3],
    pub distance: f32,
    /// Yaw around the world-up (Y) axis, radians.
    pub yaw: f32,
    /// Pitch above the horizon, radians (clamped to avoid gimbal lock).
    pub pitch: f32,
    pub fov_y_deg: f32,
    pub near: f32,
    pub far: f32,
    pub viewport: Viewport,
}

impl Default for OrbitCamera {
    /// A derived `Default` would zero `fov_y_deg`/`near`/`far`, and
    /// `reset_to_aabb` never touches those fields — the projection then
    /// contains inf/NaN entries and the GPU draws nothing (the black
    /// viewport seen in the GUI, which builds its camera via `Default`).
    /// Delegate to `new` so every construction path yields a sane frustum.
    fn default() -> Self {
        Self::new(Viewport::default())
    }
}

impl OrbitCamera {
    /// Build a camera looking at the origin from the +Z side, with a
    /// reasonable default FOV that keeps Bully-scale models (10s of
    /// units) inside the frustum.
    pub fn new(viewport: Viewport) -> Self {
        Self {
            target: [0.0, 0.0, 0.0],
            distance: 10.0,
            yaw: 0.0,
            pitch: 0.3,
            fov_y_deg: 45.0,
            near: 0.1,
            // Far plane is relaxed; Bully NIFs scale to a few hundred
            // units and we want some headroom after orbit/zoom.
            far: 10_000.0,
            viewport,
        }
    }

    /// Eye position in world space. Pitch is clamped so we never look
    /// straight up or down (which collapses the look-at basis). The
    /// convention is `eye = target + distance * (sin(yaw)*cos(pitch),
    /// sin(pitch), cos(yaw)*cos(pitch))` so that yaw=0 places the
    /// camera on the +Z axis (consistent with right-handed
    /// `look_at_rh`) and yaw rotates around world Y.
    pub fn eye(&self) -> [f32; 3] {
        let cp = self.pitch.cos();
        let sp = self.pitch.sin();
        let cy = self.yaw.cos();
        let sy = self.yaw.sin();
        let d = self.distance;
        [
            self.target[0] + d * cp * sy,
            self.target[1] + d * sp,
            self.target[2] + d * cp * cy,
        ]
    }

    /// View matrix that puts the camera at `eye()` looking at `target`.
    pub fn view(&self) -> Mat4 {
        Mat4::look_at_rh(
            self.eye().into(),
            self.target.into(),
            glam::Vec3::new(0.0, 1.0, 0.0),
        )
    }

    /// Perspective matrix built from `fov_y_deg` and the viewport.
    /// Returns an identity placeholder if the viewport is zero-area;
    /// the widget treats that as "skip the draw for this frame".
    pub fn projection(&self) -> Mat4 {
        if self.viewport.is_zero_area() {
            return Mat4::IDENTITY;
        }
        Mat4::perspective_rh(
            self.fov_y_deg.to_radians(),
            self.viewport.aspect(),
            self.near,
            self.far,
        )
    }

    /// `projection() * view()` — what the renderer uploads as a UBO.
    pub fn view_proj(&self) -> Mat4 {
        self.projection() * self.view()
    }

    /// Set `target` to the AABB centre, `distance` to fit the bounding
    /// sphere inside the vertical FOV with a small margin.
    pub fn reset_to_aabb(&mut self, aabb: &Aabb) {
        let center = aabb.center();
        self.target = if center.iter().all(|v| v.is_finite()) {
            center
        } else {
            [0.0, 0.0, 0.0]
        };
        let r = aabb.bounding_radius();
        let r = if r.is_finite() && r >= 0.0 { r } else { 1.0 };
        // For a vertical FOV `θ`, the smallest distance that fits a
        // sphere of radius `r` vertically is `r / sin(θ / 2)`.
        // We add a 1.2× margin so the model doesn't touch the edges.
        let half_fov = (self.fov_y_deg * 0.5).to_radians();
        let sin = half_fov.sin();
        let d = if sin > 0.001 && sin.is_finite() {
            (r / sin) * 1.2
        } else {
            r.max(1.0) * 4.0
        };
        self.distance = if d.is_finite() {
            d.max(self.near * 2.0 + 0.1)
        } else {
            10.0
        };
        self.yaw = 0.0;
        self.pitch = 0.3;
    }

    /// Apply an orbit delta given in pixels. `sensitivity` is radians per
    /// pixel; defaults to ~0.01 which feels good on a 1080p viewport.
    /// Blender-style turntable: dragging right swings the viewpoint to
    /// the left around the model (and dragging down drops it), so the
    /// model appears to rotate with the cursor.
    pub fn orbit(&mut self, dx: f32, dy: f32, sensitivity: f32) {
        self.yaw -= dx * sensitivity;
        self.pitch = (self.pitch - dy * sensitivity).clamp(-1.55, 1.55);
        self.clamp_pitch_to_floor();
    }

    /// Pan the target perpendicular to the view direction. `dx` and `dy`
    /// are pixel deltas; `pan_scale` is world-units-per-pixel derived
    /// from the current distance so pan feels consistent at any zoom.
    pub fn pan(&mut self, dx: f32, dy: f32, sensitivity: f32) {
        let s = (self.distance * sensitivity).max(self.near);
        // Forward vector (from eye to target); we use the camera's
        // local right and up to move the target in screen space.
        let eye: glam::Vec3 = self.eye().into();
        let target: glam::Vec3 = self.target.into();
        let forward = (target - eye).normalize_or_zero();
        let world_up = glam::Vec3::Y;
        let right = forward.cross(world_up).normalize_or_zero();
        let up = right.cross(forward).normalize_or_zero();
        self.target[0] += (right.x * dx - up.x * dy) * s;
        self.target[1] += (right.y * dx - up.y * dy) * s;
        self.target[2] += (right.z * dx - up.z * dy) * s;
        self.clamp_pitch_to_floor();
    }

    /// Exponential zoom. `factor > 1.0` zooms out, `< 1.0` zooms in.
    pub fn dolly(&mut self, factor: f32) {
        let f = factor.clamp(0.5, 2.0);
        self.distance = (self.distance * f)
            .max(self.near * 2.0 + 0.01)
            .min(self.far * 0.5);
        self.clamp_pitch_to_floor();
    }

    /// Clamp `pitch` so the eye stays just above the world floor plane
    /// (Y=0). The grid floor in `grid.wgsl` is ray-cast from the eye,
    /// so once the eye sinks below the plane the floor is behind every
    /// ray and vanishes — the "camera sunk into the floor" black view.
    fn clamp_pitch_to_floor(&mut self) {
        // eye_y = target[1] + distance * sin(pitch) >= MIN_EYE_HEIGHT
        let sin_min =
            ((MIN_EYE_HEIGHT - self.target[1]) / self.distance).clamp(-1.0, 1.0);
        let pitch_min = sin_min.asin().max(-1.55);
        self.pitch = self.pitch.max(pitch_min);
    }

    pub fn set_viewport(&mut self, viewport: Viewport) {
        self.viewport = viewport;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    fn approx_pt(a: [f32; 3], b: [f32; 3]) -> bool {
        approx_eq(a[0], b[0]) && approx_eq(a[1], b[1]) && approx_eq(a[2], b[2])
    }

    #[test]
    fn identity_orientation_is_no_op() {
        let m = BaseOrientation::Yup.to_yup_matrix();
        assert!(m.abs_diff_eq(Mat4::IDENTITY, 1e-6));
    }

    #[test]
    fn zup_y_axis_maps_to_camera_negative_z() {
        let m = BaseOrientation::Zup.to_yup_matrix();
        // World (0, 1, 0) -> camera (0, 0, -1).
        let v = m * glam::Vec4::new(0.0, 1.0, 0.0, 1.0);
        assert!(approx_eq(v.y, 0.0) && approx_eq(v.z, -1.0));
    }

    #[test]
    fn xup_x_axis_maps_to_camera_y() {
        let m = BaseOrientation::Xup.to_yup_matrix();
        let v = m * glam::Vec4::new(1.0, 0.0, 0.0, 1.0);
        assert!(approx_eq(v.y, 1.0));
    }

    #[test]
    fn camera_eye_default_looks_at_origin_from_front() {
        let cam = OrbitCamera::new(Viewport {
            width: 800,
            height: 600,
        });
        let e = cam.eye();
        // With yaw=0, pitch≈0.3, distance=10, the eye sits roughly at
        // (0, 3, 9.5) and looks at the origin.
        assert!(approx_pt(e, [0.0, 10.0 * 0.3_f32.sin(), 10.0 * 0.3_f32.cos()]));
    }

    #[test]
    fn reset_to_aabb_centres_target() {
        let mut cam = OrbitCamera::new(Viewport {
            width: 800,
            height: 600,
        });
        let aabb = Aabb {
            min: [-1.0, -2.0, -3.0],
            max: [3.0, 4.0, 5.0],
        };
        cam.reset_to_aabb(&aabb);
        assert!(approx_pt(cam.target, [1.0, 1.0, 1.0]));
        // Distance should comfortably exceed the bounding sphere radius.
        let r = aabb.bounding_radius();
        assert!(cam.distance > r);
    }

    #[test]
    fn dolly_clamps_to_near_plane() {
        let mut cam = OrbitCamera::new(Viewport {
            width: 800,
            height: 600,
        });
        cam.distance = cam.near * 1.5;
        cam.dolly(0.1); // zoom way in
        assert!(cam.distance > cam.near);
    }

    #[test]
    fn dolly_clamps_to_far_plane() {
        let mut cam = OrbitCamera::new(Viewport {
            width: 800,
            height: 600,
        });
        cam.dolly(10.0); // extreme zoom out
        assert!(cam.distance <= cam.far * 0.5 + 0.001);
    }

    #[test]
    fn orbit_clamps_pitch() {
        let mut cam = OrbitCamera::new(Viewport {
            width: 800,
            height: 600,
        });
        cam.orbit(0.0, 10000.0, 1.0);
        assert!(cam.pitch <= 1.55);
        cam.orbit(0.0, -10000.0, 1.0);
        assert!(cam.pitch >= -1.55);
    }

    #[test]
    fn orbit_keeps_eye_above_floor() {
        // Regression for the "camera sinks into the floor" view: the
        // ray-cast grid floor can only be seen from above Y=0, so
        // orbiting down must stop before the eye dips under it.
        let mut cam = OrbitCamera::new(Viewport {
            width: 800,
            height: 600,
        });
        cam.reset_to_aabb(&Aabb {
            min: [-1.0, -1.0, -1.0],
            max: [1.0, 1.0, 1.0],
        });
        cam.orbit(0.0, 10000.0, 1.0); // drag down hard
        assert!(cam.pitch >= -1.55);
        assert!(
            cam.eye()[1] >= MIN_EYE_HEIGHT - 1e-4,
            "eye sank below the floor: y = {}",
            cam.eye()[1]
        );
        // Zooming in at a low pitch must also lift the eye.
        cam.pitch = -0.2;
        cam.dolly(0.5);
        assert!(cam.eye()[1] >= MIN_EYE_HEIGHT - 1e-4);
    }

    #[test]
    fn view_matrix_is_invertible() {
        let cam = OrbitCamera::new(Viewport {
            width: 800,
            height: 600,
        });
        let m = cam.view();
        assert!(m.determinant().abs() > 0.0);
    }

    #[test]
    fn perspective_changes_with_aspect() {
        let square = OrbitCamera::new(Viewport {
            width: 600,
            height: 600,
        });
        let wide = OrbitCamera::new(Viewport {
            width: 1200,
            height: 600,
        });
        assert!(!approx_eq(
            square.projection().x_axis.x,
            wide.projection().x_axis.x
        ));
    }

    #[test]
    fn zero_viewport_returns_identity_proj() {
        let cam = OrbitCamera::new(Viewport {
            width: 0,
            height: 0,
        });
        assert!(cam.projection().abs_diff_eq(Mat4::IDENTITY, 1e-6));
    }

    #[test]
    fn default_camera_yields_finite_view_proj_after_aabb_reset() {
        // Regression for the black GUI viewport: the widget builds its
        // camera via `Default` and only calls `reset_to_aabb`, so the
        // default must carry a usable fov/near/far.
        let mut cam = OrbitCamera::default();
        cam.reset_to_aabb(&Aabb {
            min: [-1.0, -1.0, -1.0],
            max: [1.0, 1.0, 1.0],
        });
        cam.set_viewport(Viewport {
            width: 800,
            height: 600,
        });
        assert!(cam.near > 0.0 && cam.far > cam.near && cam.fov_y_deg > 0.0);
        for v in cam.view_proj().to_cols_array() {
            assert!(v.is_finite(), "view_proj contains non-finite value");
        }
        for v in cam.view_proj().inverse().to_cols_array() {
            assert!(v.is_finite(), "inverse view_proj contains non-finite value");
        }
    }

    #[test]
    fn reset_to_aabb_handles_nan_and_infinite_bounds() {
        let mut cam = OrbitCamera::new(Viewport {
            width: 800,
            height: 600,
        });
        cam.reset_to_aabb(&Aabb {
            min: [f32::NAN, f32::NAN, f32::NAN],
            max: [f32::NAN, f32::NAN, f32::NAN],
        });
        assert!(cam.target.iter().all(|v| v.is_finite()));
        assert!(cam.distance.is_finite());
        assert!(cam.distance > 0.0);

        cam.reset_to_aabb(&Aabb {
            min: [f32::INFINITY, f32::INFINITY, f32::INFINITY],
            max: [f32::INFINITY, f32::INFINITY, f32::INFINITY],
        });
        assert!(cam.target.iter().all(|v| v.is_finite()));
        assert!(cam.distance.is_finite());
        assert!(cam.distance > 0.0);
    }
}
