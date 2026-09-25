//! Synthetic fixtures: invented geometry and clips with analytical
//! expected results. These power the deterministic tests and the
//! development demo entry point. No game assets are embedded, and a
//! selected real archive file never claims animation support through
//! these fixtures.

use glam::{Mat4, Quat, Vec3};

use crate::inspector::scene3d::camera::BaseOrientation;
use crate::inspector::scene3d::mesh::{SceneTexture, Vertex};

use crate::inspector::animation::clip::{
    AnimationClip, AnimationLibrary, ClipMarker, Interpolation, PropertyTrack, SourceRate,
    TrackChannel,
};
use crate::inspector::animation::model::{
    MeshAsset, ModelAsset, NodeTransform, SceneNode, SkinBinding, VertexSkin,
};
use crate::inspector::animation::{ClipId, NodeId};

fn vertex(position: [f32; 3], normal: [f32; 3], uv: [f32; 2]) -> Vertex {
    Vertex {
        position,
        normal,
        uv,
    }
}

fn trs(translation: Vec3) -> NodeTransform {
    NodeTransform {
        translation,
        ..NodeTransform::IDENTITY
    }
}

fn node(
    id: u32,
    parent: Option<u32>,
    name: &str,
    local: NodeTransform,
    mesh: Option<usize>,
) -> SceneNode {
    SceneNode {
        id: NodeId(id),
        parent: parent.map(NodeId),
        name: name.to_string(),
        local,
        mesh,
    }
}

/// Rest-pose world matrices for a small node list (fixture-local helper;
/// the runtime path is `ModelAsset::compose_world`).
fn rest_world(nodes: &[SceneNode]) -> Vec<Mat4> {
    let mut world = vec![None; nodes.len()];
    fn compute(index: usize, nodes: &[SceneNode], world: &mut [Option<Mat4>]) -> Mat4 {
        if let Some(matrix) = world[index] {
            return matrix;
        }
        let local = nodes[index].local.matrix();
        let matrix = match nodes[index].parent {
            Some(parent) => compute(parent.0 as usize, nodes, world) * local,
            None => local,
        };
        world[index] = Some(matrix);
        matrix
    }
    for index in 0..nodes.len() {
        compute(index, nodes, &mut world);
    }
    world.into_iter().map(|entry| entry.unwrap()).collect()
}

/// `Bj = inverse(Gj_bind) * Gm_bind` — the bind correction mapping the
/// mesh's bind-local coordinates into the joint's bind-local frame, for
/// fixtures whose bind pose equals the rest pose.
fn bind_corrections(nodes: &[SceneNode], mesh_node: NodeId, joints: &[NodeId]) -> Vec<Mat4> {
    let world = rest_world(nodes);
    let mesh_world = world[mesh_node.0 as usize];
    joints
        .iter()
        .map(|joint| world[joint.0 as usize].inverse() * mesh_world)
        .collect()
}

/// Axis-aligned box with per-face normals, centered on the origin.
fn box_mesh(half: Vec3) -> (Vec<Vertex>, Vec<u32>) {
    let [hx, hy, hz] = half.to_array();
    let mut vertices = Vec::with_capacity(24);
    let mut indices = Vec::with_capacity(36);
    let faces = [
        // (normal, corner offsets in winding order)
        (
            [1.0, 0.0, 0.0],
            [[hx, -hy, -hz], [hx, hy, -hz], [hx, hy, hz], [hx, -hy, hz]],
        ),
        (
            [-1.0, 0.0, 0.0],
            [
                [-hx, -hy, hz],
                [-hx, hy, hz],
                [-hx, hy, -hz],
                [-hx, -hy, -hz],
            ],
        ),
        (
            [0.0, 1.0, 0.0],
            [[-hx, hy, -hz], [-hx, hy, hz], [hx, hy, hz], [hx, hy, -hz]],
        ),
        (
            [0.0, -1.0, 0.0],
            [
                [-hx, -hy, hz],
                [-hx, -hy, -hz],
                [hx, -hy, -hz],
                [hx, -hy, hz],
            ],
        ),
        (
            [0.0, 0.0, 1.0],
            [[-hx, -hy, hz], [hx, -hy, hz], [hx, hy, hz], [-hx, hy, hz]],
        ),
        (
            [0.0, 0.0, -1.0],
            [
                [hx, -hy, -hz],
                [-hx, -hy, -hz],
                [-hx, hy, -hz],
                [hx, hy, -hz],
            ],
        ),
    ];
    for (normal, corners) in faces {
        let base = vertices.len() as u32;
        let uvs = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
        for (corner, uv) in corners.iter().zip(uvs) {
            vertices.push(vertex(*corner, normal, uv));
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    (vertices, indices)
}

/// Flat disc on the local XZ plane (normal +Y), a triangle fan.
fn disc_mesh(radius: f32, segments: u32) -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices = vec![vertex([0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.5, 0.5])];
    for segment in 0..segments {
        let angle = segment as f32 / segments as f32 * std::f32::consts::TAU;
        let (sin, cos) = angle.sin_cos();
        vertices.push(vertex(
            [cos * radius, 0.0, sin * radius],
            [0.0, 1.0, 0.0],
            [cos * 0.5 + 0.5, sin * 0.5 + 0.5],
        ));
    }
    let mut indices = Vec::new();
    for segment in 0..segments {
        let next = (segment + 1) % segments;
        indices.extend_from_slice(&[0, 1 + next, 1 + segment]);
    }
    (vertices, indices)
}

/// Open tube along local +Y, with skin weights blended between two
/// joints. `weight_at` maps a vertex's bind-space Y to the upper joint's
/// weight. Ring order is bottom to top; joint slot 0 is the upper joint.
fn tube_mesh(
    radius: f32,
    height: f32,
    rings: u32,
    segments: u32,
    weight_at: impl Fn(f32) -> f32,
) -> (Vec<Vertex>, Vec<u32>, Vec<VertexSkin>) {
    let mut vertices = Vec::new();
    let mut weights = Vec::new();
    for ring in 0..=rings {
        let y = height * ring as f32 / rings as f32;
        let upper = weight_at(y);
        for segment in 0..segments {
            let angle = segment as f32 / segments as f32 * std::f32::consts::TAU;
            let (sin, cos) = angle.sin_cos();
            vertices.push(vertex(
                [cos * radius, y, sin * radius],
                [cos, 0.0, sin],
                [segment as f32 / segments as f32, y / height],
            ));
            // Slot 0 = upper joint, slot 1 = lower joint.
            let mut influence = VertexSkin::new();
            if upper > 0.0 {
                influence.push((0, upper));
            }
            if upper < 1.0 {
                influence.push((1, 1.0 - upper));
            }
            weights.push(influence);
        }
    }
    let mut indices = Vec::new();
    for ring in 0..rings {
        for segment in 0..segments {
            let next = (segment + 1) % segments;
            let a = ring * segments + segment;
            let b = ring * segments + next;
            let c = (ring + 1) * segments + segment;
            let d = (ring + 1) * segments + next;
            indices.extend_from_slice(&[a, b, d, a, d, c]);
        }
    }
    (vertices, indices, weights)
}

/// Procedural checkerboard so the demo exercises the textured path
/// without embedding any game asset.
fn checker_texture(size: u32, cells: u32) -> SceneTexture {
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    let cell = (size / cells).max(1);
    for y in 0..size {
        for x in 0..size {
            let on = ((x / cell) + (y / cell)).is_multiple_of(2);
            let offset = ((y * size + x) * 4) as usize;
            let (r, g, b) = if on {
                (232u8, 236u8, 240u8)
            } else {
                (38u8, 98u8, 108u8)
            };
            rgba[offset..offset + 4].copy_from_slice(&[r, g, b, 255]);
        }
    }
    SceneTexture {
        width: size,
        height: size,
        rgba,
    }
}

fn clip(
    id: u32,
    name: &str,
    tracks: Vec<PropertyTrack>,
    markers: Vec<ClipMarker>,
    source_rate: Option<SourceRate>,
) -> AnimationClip {
    let duration = tracks
        .iter()
        .flat_map(|track| track.channel.times().iter().copied())
        .fold(0.0f32, f32::max);
    let mut clip = AnimationClip {
        id: ClipId(id),
        name: name.to_string(),
        duration,
        tracks,
        markers,
        source_rate,
        provenance: "synthetic fixture; no game data".into(),
    };
    clip.normalize_rotation_keys();
    clip.validate().expect("fixture clips must validate");
    clip
}

fn rotation_track(target: &str, keys: &[(f32, Quat)]) -> PropertyTrack {
    PropertyTrack {
        target: target.into(),
        channel: TrackChannel::Rotation {
            times: keys.iter().map(|&(time, _)| time).collect(),
            values: keys.iter().map(|&(_, value)| value).collect(),
        },
        interpolation: Interpolation::Linear,
    }
}

fn translation_track(target: &str, keys: &[(f32, Vec3)]) -> PropertyTrack {
    PropertyTrack {
        target: target.into(),
        channel: TrackChannel::Translation {
            times: keys.iter().map(|&(time, _)| time).collect(),
            values: keys.iter().map(|&(_, value)| value).collect(),
        },
        interpolation: Interpolation::Linear,
    }
}

/// Two-joint strip with mixed weights: non-identity mesh transform, and
/// a joint order (`spine_tip` before `spine`) that differs from the
/// scene-node order. Bind pose == rest pose.
pub fn two_joint_strip() -> (ModelAsset, AnimationLibrary) {
    let nodes = vec![
        node(0, None, "root", NodeTransform::IDENTITY, None),
        node(
            1,
            Some(0),
            "mesh_node",
            trs(Vec3::new(1.0, 0.0, 0.0)),
            Some(0),
        ),
        node(2, Some(0), "spine", NodeTransform::IDENTITY, None),
        node(3, Some(2), "spine_tip", trs(Vec3::new(0.0, 1.0, 0.0)), None),
    ];
    // Center-line strip with a thin X width so it renders; two columns
    // per level, ordered (+x, -x), bottom to top (y = 0, 0.5, 1, 1.5, 2).
    let mut vertices = Vec::new();
    let mut weights = Vec::new();
    let levels = [
        (0.0f32, 0.0f32), // upper joint weight
        (0.5, 0.0),
        (1.0, 0.5),
        (1.5, 1.0),
        (2.0, 1.0),
    ];
    for (y, upper) in levels {
        for x in [0.1f32, -0.1] {
            vertices.push(vertex([x, y, 0.0], [0.0, 0.0, 1.0], [0.0, y / 2.0]));
            let mut influence = VertexSkin::new();
            if upper > 0.0 {
                influence.push((0, upper));
            }
            if upper < 1.0 {
                influence.push((1, 1.0 - upper));
            }
            weights.push(influence);
        }
    }
    let mut indices = Vec::new();
    for level in 0..4u32 {
        let a = level * 2;
        indices.extend_from_slice(&[a, a + 1, a + 3, a, a + 3, a + 2]);
    }
    // Joint order intentionally reversed relative to scene-node order.
    let joints = vec![NodeId(3), NodeId(2)];
    let inverse_bind = bind_corrections(&nodes, NodeId(1), &joints);
    let mesh = MeshAsset {
        name: "strip".into(),
        texture_name: None,
        vertices,
        indices,
        diffuse: None,
        skin: Some(SkinBinding {
            joints,
            inverse_bind,
            weights,
        }),
    };
    let model = ModelAsset::new(
        "two-joint strip".into(),
        "synthetic:two_joint_strip".into(),
        nodes,
        vec![mesh],
        Mat4::IDENTITY,
        BaseOrientation::Yup,
        None,
    )
    .expect("fixture model must validate");
    let bend = clip(
        0,
        "bend",
        vec![rotation_track(
            "spine_tip",
            &[
                (0.0, Quat::IDENTITY),
                (1.0, Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
            ],
        )],
        Vec::new(),
        Some(SourceRate::PREVIEW_30),
    );
    let library = AnimationLibrary {
        name: "two-joint strip clips".into(),
        clips: vec![bend],
        provenance: "synthetic fixture".into(),
    };
    (model, library)
}

/// Analytical positions of the strip's vertices at the clip's 90° bend
/// (t = 1.0), in the strip's vertex order ((+x, -x) pairs per level).
pub fn two_joint_strip_bent_positions() -> Vec<[f32; 3]> {
    vec![
        [1.1, 0.0, 0.0],
        [0.9, 0.0, 0.0],
        [1.1, 0.5, 0.0],
        [0.9, 0.5, 0.0],
        [0.55, 1.55, 0.0],
        [0.45, 1.45, 0.0],
        [-0.5, 2.1, 0.0],
        [-0.5, 1.9, 0.0],
        [-1.0, 2.1, 0.0],
        [-1.0, 1.9, 0.0],
    ]
}

/// Parent/child hierarchy with a rigid prop attached to the child.
pub fn parent_child_prop() -> (ModelAsset, AnimationLibrary) {
    let (vertices, indices) = box_mesh(Vec3::splat(0.25));
    let mesh = MeshAsset {
        name: "prop".into(),
        texture_name: None,
        vertices,
        indices,
        diffuse: None,
        skin: None,
    };
    let nodes = vec![
        node(0, None, "arm", trs(Vec3::new(0.0, 1.0, 0.0)), None),
        node(1, Some(0), "prop", trs(Vec3::new(0.0, 2.0, 0.0)), Some(0)),
    ];
    let model = ModelAsset::new(
        "parent/child prop".into(),
        "synthetic:parent_child_prop".into(),
        nodes,
        vec![mesh],
        Mat4::IDENTITY,
        BaseOrientation::Yup,
        None,
    )
    .expect("fixture model must validate");
    let rotate = clip(
        0,
        "rotate-parent",
        vec![rotation_track(
            "arm",
            &[
                (0.0, Quat::IDENTITY),
                (1.0, Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
            ],
        )],
        Vec::new(),
        Some(SourceRate::PREVIEW_30),
    );
    let library = AnimationLibrary {
        name: "parent/child clips".into(),
        clips: vec![rotate],
        provenance: "synthetic fixture".into(),
    };
    (model, library)
}

/// Root-motion walker: a box that travels forward 1.5 units over one
/// second with a vertical bob. Exercises source-motion vs in-place and
/// the motion envelope.
pub fn walker() -> (ModelAsset, AnimationLibrary) {
    let (vertices, indices) = box_mesh(Vec3::new(0.25, 0.5, 0.25));
    let mesh = MeshAsset {
        name: "walker".into(),
        texture_name: None,
        vertices,
        indices,
        diffuse: None,
        skin: None,
    };
    let nodes = vec![node(
        0,
        None,
        "walker_root",
        trs(Vec3::new(0.0, 0.5, 0.0)),
        Some(0),
    )];
    let model = ModelAsset::new(
        "walker".into(),
        "synthetic:walker".into(),
        nodes,
        vec![mesh],
        Mat4::IDENTITY,
        BaseOrientation::Yup,
        Some(NodeId(0)),
    )
    .expect("fixture model must validate");
    let walk = clip(
        0,
        "walk",
        vec![translation_track(
            "walker_root",
            &[
                (0.0, Vec3::new(0.0, 0.5, 0.0)),
                (0.25, Vec3::new(0.0, 0.57, 0.375)),
                (0.5, Vec3::new(0.0, 0.5, 0.75)),
                (0.75, Vec3::new(0.0, 0.57, 1.125)),
                (1.0, Vec3::new(0.0, 0.5, 1.5)),
            ],
        )],
        vec![
            ClipMarker {
                time: 0.25,
                label: "step L".into(),
            },
            ClipMarker {
                time: 0.75,
                label: "step R".into(),
            },
        ],
        Some(SourceRate::PREVIEW_30),
    );
    let library = AnimationLibrary {
        name: "walker clips".into(),
        clips: vec![walk],
        provenance: "synthetic fixture".into(),
    };
    (model, library)
}

/// Z-up source with an explicit `source_to_view` conversion: moving
/// "up" in source space must come out as +Y in the viewer.
pub fn zup_prop() -> (ModelAsset, AnimationLibrary) {
    let (vertices, indices) = box_mesh(Vec3::splat(0.3));
    let mesh = MeshAsset {
        name: "platform".into(),
        texture_name: None,
        vertices,
        indices,
        diffuse: None,
        skin: None,
    };
    let nodes = vec![node(0, None, "platform", NodeTransform::IDENTITY, Some(0))];
    let model = ModelAsset::new(
        "z-up platform".into(),
        "synthetic:zup_prop".into(),
        nodes,
        vec![mesh],
        BaseOrientation::Zup.to_yup_matrix(),
        BaseOrientation::Zup,
        None,
    )
    .expect("fixture model must validate");
    let rise = clip(
        0,
        "rise",
        vec![translation_track(
            "platform",
            &[(0.0, Vec3::ZERO), (1.0, Vec3::new(0.0, 0.0, 2.0))],
        )],
        Vec::new(),
        None,
    );
    let library = AnimationLibrary {
        name: "z-up clips".into(),
        clips: vec![rise],
        provenance: "synthetic fixture".into(),
    };
    (model, library)
}

/// The development demo: a small invented character with a skinned
/// torso, a rigid head, a textured wand prop and a static base, plus
/// four clips exercising sway, hierarchy motion, root motion and
/// sparse/step channels.
pub fn demo() -> (ModelAsset, AnimationLibrary) {
    // Nodes: 0 root, 1 base, 2 torso(mesh), 3 spine(joint), 4 neck(joint),
    //        5 head(mesh), 6 arm, 7 wand(mesh).
    let nodes = vec![
        node(0, None, "DemoRoot", NodeTransform::IDENTITY, None),
        node(1, Some(0), "Base", NodeTransform::IDENTITY, Some(0)),
        node(2, Some(0), "Torso", trs(Vec3::new(0.0, 0.9, 0.0)), Some(1)),
        node(3, Some(2), "Spine", trs(Vec3::new(0.0, 0.25, 0.0)), None),
        node(4, Some(3), "Neck", trs(Vec3::new(0.0, 0.45, 0.0)), None),
        node(5, Some(4), "Head", trs(Vec3::new(0.0, 0.2, 0.0)), Some(2)),
        node(
            6,
            Some(3),
            "ArmRight",
            trs(Vec3::new(-0.24, 0.35, 0.0)),
            None,
        ),
        node(
            7,
            Some(6),
            "Wand",
            trs(Vec3::new(-0.14, -0.04, 0.0)),
            Some(3),
        ),
    ];

    let (base_vertices, base_indices) = disc_mesh(0.7, 20);
    let base = MeshAsset {
        name: "Base".into(),
        texture_name: None,
        vertices: base_vertices,
        indices: base_indices,
        diffuse: None,
        skin: None,
    };

    // Torso tube: bind-local Y 0..0.9, blended from Spine to Neck.
    let (tube_vertices, tube_indices, tube_weights) =
        tube_mesh(0.16, 0.9, 6, 10, |y| ((y - 0.25) / 0.45).clamp(0.0, 1.0));
    let joints = vec![NodeId(4), NodeId(3)]; // slot 0 = Neck (upper)
    let inverse_bind = bind_corrections(&nodes, NodeId(2), &joints);
    let torso = MeshAsset {
        name: "Torso".into(),
        texture_name: None,
        vertices: tube_vertices,
        indices: tube_indices,
        diffuse: None,
        skin: Some(SkinBinding {
            joints,
            inverse_bind,
            weights: tube_weights,
        }),
    };

    let (head_vertices, head_indices) = box_mesh(Vec3::new(0.17, 0.15, 0.17));
    let head = MeshAsset {
        name: "Head".into(),
        texture_name: None,
        vertices: head_vertices,
        indices: head_indices,
        diffuse: None,
        skin: None,
    };

    let (wand_vertices, wand_indices) = box_mesh(Vec3::new(0.05, 0.28, 0.05));
    let wand = MeshAsset {
        name: "Wand".into(),
        texture_name: Some("demo_checker".into()),
        vertices: wand_vertices,
        indices: wand_indices,
        diffuse: Some(checker_texture(32, 8)),
        skin: None,
    };

    let model = ModelAsset::new(
        "Animation demo character".into(),
        "synthetic:demo_character".into(),
        nodes,
        vec![base, torso, head, wand],
        Mat4::IDENTITY,
        BaseOrientation::Yup,
        Some(NodeId(0)),
    )
    .expect("demo model must validate");

    let sway = 6.0f32.to_radians();
    let idle = clip(
        0,
        "Idle",
        vec![
            rotation_track(
                "Spine",
                &[
                    (0.0, Quat::IDENTITY),
                    (0.5, Quat::from_rotation_z(sway)),
                    (1.0, Quat::IDENTITY),
                    (1.5, Quat::from_rotation_z(-sway)),
                    (2.0, Quat::IDENTITY),
                ],
            ),
            rotation_track(
                "Neck",
                &[
                    (0.0, Quat::IDENTITY),
                    (0.5, Quat::from_rotation_z(-4.0f32.to_radians())),
                    (1.0, Quat::IDENTITY),
                    (1.5, Quat::from_rotation_z(4.0f32.to_radians())),
                    (2.0, Quat::IDENTITY),
                ],
            ),
        ],
        vec![
            ClipMarker {
                time: 0.5,
                label: "breath in".into(),
            },
            ClipMarker {
                time: 1.5,
                label: "breath out".into(),
            },
        ],
        Some(SourceRate::PREVIEW_30),
    );

    let wave = clip(
        1,
        "Wave",
        vec![
            rotation_track(
                "ArmRight",
                &[
                    (0.0, Quat::IDENTITY),
                    (0.2, Quat::from_rotation_z(-95.0f32.to_radians())),
                    (0.4, Quat::from_rotation_z(-70.0f32.to_radians())),
                    (0.6, Quat::from_rotation_z(-95.0f32.to_radians())),
                    (0.8, Quat::from_rotation_z(-70.0f32.to_radians())),
                    (1.0, Quat::from_rotation_z(-95.0f32.to_radians())),
                    (1.2, Quat::IDENTITY),
                ],
            ),
            rotation_track(
                "Wand",
                &[
                    (0.0, Quat::IDENTITY),
                    (0.6, Quat::from_rotation_y(std::f32::consts::PI)),
                    (1.2, Quat::from_rotation_y(std::f32::consts::TAU)),
                ],
            ),
        ],
        vec![
            ClipMarker {
                time: 0.2,
                label: "raise".into(),
            },
            ClipMarker {
                time: 0.6,
                label: "shake".into(),
            },
            ClipMarker {
                time: 1.0,
                label: "lower".into(),
            },
        ],
        Some(SourceRate::PREVIEW_30),
    );

    let walk = clip(
        2,
        "Walk",
        vec![
            translation_track(
                "DemoRoot",
                &[
                    (0.0, Vec3::ZERO),
                    (0.25, Vec3::new(0.0, 0.07, 0.375)),
                    (0.5, Vec3::ZERO),
                    (0.75, Vec3::new(0.0, 0.07, 1.125)),
                    (1.0, Vec3::new(0.0, 0.0, 1.5)),
                ],
            ),
            rotation_track(
                "Spine",
                &[
                    (0.0, Quat::IDENTITY),
                    (0.25, Quat::from_rotation_z(8.0f32.to_radians())),
                    (0.5, Quat::IDENTITY),
                    (0.75, Quat::from_rotation_z(-8.0f32.to_radians())),
                    (1.0, Quat::IDENTITY),
                ],
            ),
        ],
        vec![
            ClipMarker {
                time: 0.25,
                label: "step L".into(),
            },
            ClipMarker {
                time: 0.75,
                label: "step R".into(),
            },
        ],
        Some(SourceRate::PREVIEW_30),
    );

    let sparse = clip(
        3,
        "Sparse look",
        vec![
            rotation_track(
                "Neck",
                &[
                    (0.0, Quat::IDENTITY),
                    (0.9, Quat::from_rotation_x(25.0f32.to_radians())),
                    (1.7, Quat::IDENTITY),
                ],
            ),
            PropertyTrack {
                target: "Head".into(),
                channel: TrackChannel::Scale {
                    times: vec![0.0, 0.85],
                    values: vec![Vec3::ONE, Vec3::splat(1.2)],
                },
                interpolation: Interpolation::Step,
            },
        ],
        Vec::new(),
        None, // no source rate: exercises the labelled preview rate
    );

    let library = AnimationLibrary {
        name: "Synthetic demo clips".into(),
        clips: vec![idle, wave, walk, sparse],
        provenance: "synthetic fixture; invented geometry, no game data".into(),
    };
    (model, library)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_fixture_clips_validate() {
        let (_, a) = two_joint_strip();
        let (_, b) = parent_child_prop();
        let (_, c) = walker();
        let (_, d) = zup_prop();
        let (_, e) = demo();
        for library in [a, b, c, d, e] {
            for clip in &library.clips {
                clip.validate().unwrap();
            }
        }
    }

    #[test]
    fn demo_model_has_no_diagnostics() {
        let (model, _) = demo();
        assert!(model.diagnostics.is_empty(), "{:?}", model.diagnostics);
        assert!(model.has_skinning());
        assert_eq!(model.root_motion_node, Some(NodeId(0)));
    }

    #[test]
    fn demo_rest_scene_has_geometry_and_bounds() {
        let (model, _) = demo();
        let scene = crate::inspector::animation::pose::rest_scene(&model);
        assert!(scene.has_geometry());
        let extent = scene.aabb.extent();
        assert!(extent[1] > 1.5, "character should stand ~2 units tall");
        assert_eq!(scene.textured_mesh_count(), 1);
        // Mesh order matches the asset, so the animated path can write
        // dynamic buffers by the same index.
        assert_eq!(scene.meshes.len(), model.meshes.len());
    }

    #[test]
    fn strip_bind_pose_is_identity_deformation() {
        let (model, _) = two_joint_strip();
        let scene = crate::inspector::animation::pose::rest_scene(&model);
        // With bind == rest, flattening runs through the skin palette and
        // still lands exactly at the rigid-transformed positions.
        let first = scene.meshes[0].vertices[0].position;
        assert!((first[0] - 1.1).abs() < 1e-5);
        assert!(first[1].abs() < 1e-5);
    }
}
