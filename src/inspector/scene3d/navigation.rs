//! Shared screen-space geometry for GPU drawing and pointer hit testing.

use glam::{Vec2, Vec3};

use super::camera::{BaseOrientation, OrbitCamera};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AxisView {
    PositiveX,
    NegativeX,
    PositiveY,
    NegativeY,
    PositiveZ,
    NegativeZ,
}

impl AxisView {
    pub const ALL: [Self; 6] = [
        Self::PositiveX,
        Self::NegativeX,
        Self::PositiveY,
        Self::NegativeY,
        Self::PositiveZ,
        Self::NegativeZ,
    ];

    pub fn direction(self, orientation: BaseOrientation) -> Vec3 {
        let source = match self {
            Self::PositiveX => Vec3::X,
            Self::NegativeX => Vec3::NEG_X,
            Self::PositiveY => Vec3::Y,
            Self::NegativeY => Vec3::NEG_Y,
            Self::PositiveZ => Vec3::Z,
            Self::NegativeZ => Vec3::NEG_Z,
        };
        orientation.to_yup_matrix().transform_vector3(source)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavigationAction {
    Axis(AxisView),
    ToggleProjection,
}

impl NavigationAction {
    pub fn id(self) -> u32 {
        match self {
            Self::Axis(axis) => axis as u32 + 1,
            Self::ToggleProjection => 7,
        }
    }

    pub fn apply(self, camera: &mut OrbitCamera) {
        match self {
            Self::Axis(axis) => camera.snap_to_axis(axis),
            Self::ToggleProjection => camera.orthographic = !camera.orthographic,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct NavigationUniform {
    /// Pixel-space center x/y, UI scale, hovered target ID (0 = none).
    pub layout: [f32; 4],
    /// Back-to-front ordered pixel x/y, view depth, axis ID (1..=6).
    pub tips: [[f32; 4]; 6],
    /// Orthographic state, source-axis indices for the floor's X/Z lines, reserved.
    pub state: [f32; 4],
}

impl NavigationUniform {
    pub fn new(camera: &OrbitCamera, width: f32, height: f32, ui_scale: f32) -> Self {
        let scale = ui_scale.min(width / 152.0).min(height / 180.0).max(0.001);
        let center = Vec2::new(width - 76.0 * scale, 76.0 * scale);
        let view = camera.view();
        let mut tips = AxisView::ALL.map(|axis| {
            let dir = view.transform_vector3(axis.direction(camera.base_orientation));
            let point = center + Vec2::new(dir.x, -dir.y) * 42.0 * scale;
            [point.x, point.y, dir.z, axis as u32 as f32 + 1.0]
        });
        tips.sort_by(|a, b| a[2].total_cmp(&b[2]));
        let (floor_x, floor_z) = match camera.base_orientation {
            BaseOrientation::Yup => (0.0, 2.0),
            BaseOrientation::Zup => (0.0, 1.0),
            BaseOrientation::Xup => (1.0, 2.0),
        };
        Self {
            layout: [center.x, center.y, scale, camera.navigation_hover as f32],
            tips,
            state: [u32::from(camera.orthographic) as f32, floor_x, floor_z, 0.0],
        }
    }

    pub fn hit_test(&self, point: Vec2) -> Option<NavigationAction> {
        let [cx, cy, scale, _] = self.layout;
        for tip in self.tips.iter().rev() {
            if point.distance(Vec2::new(tip[0], tip[1])) <= 12.0 * scale {
                return Some(NavigationAction::Axis(AxisView::ALL[tip[3] as usize - 1]));
            }
        }
        let local = (point - Vec2::new(cx, cy)) / scale;
        if local.x.abs() <= 34.0 && (63.0..=85.0).contains(&local.y) {
            Some(NavigationAction::ToggleProjection)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inspector::scene3d::camera::Viewport;

    #[test]
    fn z_up_gizmo_uses_source_axes_and_hit_targets_match_at_dpi_scales() {
        let camera = OrbitCamera {
            base_orientation: BaseOrientation::Zup,
            ..OrbitCamera::default()
        };
        for scale in [1.0, 1.5, 2.0] {
            let nav = NavigationUniform::new(&camera, 900.0 * scale, 600.0 * scale, scale);
            let up = nav.tips.iter().find(|tip| tip[3] == 5.0).unwrap();
            assert!(up[1] < nav.layout[1]);
            assert_eq!(
                nav.hit_test(Vec2::new(up[0], up[1])),
                Some(NavigationAction::Axis(AxisView::PositiveZ))
            );
            assert!(nav.hit_test(Vec2::ZERO).is_none());
        }
    }

    #[test]
    fn six_axis_snaps_are_exact_finite_and_flip_to_opposite() {
        for orientation in [
            BaseOrientation::Yup,
            BaseOrientation::Zup,
            BaseOrientation::Xup,
        ] {
            for axis in AxisView::ALL {
                let mut camera = OrbitCamera::new(Viewport {
                    width: 800,
                    height: 600,
                });
                camera.base_orientation = orientation;
                camera.snap_to_axis(axis);
                assert!(camera.orthographic);
                assert!(
                    camera
                        .backward()
                        .abs_diff_eq(axis.direction(orientation), 1e-6)
                );
                assert!(camera.view_proj().is_finite());
                assert!(camera.view_proj().inverse().is_finite());
                let nav = NavigationUniform::new(&camera, 800.0, 600.0, 1.0);
                assert_eq!(
                    nav.hit_test(Vec2::new(nav.layout[0], nav.layout[1])),
                    Some(NavigationAction::Axis(axis))
                );
                camera.snap_to_axis(axis);
                assert!(
                    camera
                        .backward()
                        .abs_diff_eq(-axis.direction(orientation), 1e-6)
                );
            }
        }
    }

    #[test]
    fn orthographic_has_no_depth_foreshortening_and_pole_pan_remains_usable() {
        let mut camera = OrbitCamera::new(Viewport {
            width: 800,
            height: 600,
        });
        camera.snap_to_axis(AxisView::PositiveY);
        let projection = camera.projection();
        let a = projection.project_point3(Vec3::new(1.0, 1.0, -2.0));
        let b = projection.project_point3(Vec3::new(1.0, 1.0, -20.0));
        assert!((a.x - b.x).abs() < 1e-6 && (a.y - b.y).abs() < 1e-6);
        let before = camera.view_proj().project_point3(Vec3::ZERO);
        camera.pan(20.0, 20.0, 0.001);
        let after = camera.view_proj().project_point3(Vec3::ZERO);
        assert!(after.x > before.x && after.y < before.y);
        assert!(camera.orthographic);
        let height = camera.projection().y_axis.y;
        camera.dolly(0.8);
        assert!(camera.projection().y_axis.y > height);
        camera.orbit(2.0, 2.0, 0.01);
        assert!(!camera.orthographic);
    }
}
