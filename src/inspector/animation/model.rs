//! Immutable model assets: node hierarchy, mesh attachments, skins.
//!
//! A [`ModelAsset`] preserves source-space geometry, hierarchy, animation
//! values and bind corrections. A single `source_to_view` transform at the
//! display root enters the Y-up viewer; changing the viewer presentation
//! never rewrites joints or clips. The old flattened static scene is
//! already in viewer coordinates, so its bridge uses an identity
//! `source_to_view` and keeps the source orientation only for labels and
//! gizmo mapping.
//!
//! This module is data + validation only. Everything that turns a pose
//! into vertices lives in [`crate::inspector::animation::pose`], so the
//! rest-pose flatten and every animated frame share one code path.

use glam::{Mat4, Quat, Vec3};
use smallvec::SmallVec;

use crate::inspector::scene3d::camera::BaseOrientation;
use crate::inspector::scene3d::mesh::{SceneTexture, Vertex};

use crate::inspector::animation::NodeId;

/// Local transform of a node: translation, rotation, scale.
///
/// Only TRS components are animated; adapters that cannot represent shear
/// reliably must keep the exact matrix on the node and leave TRS at the
/// decomposed value, never silently destroying shear.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NodeTransform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl NodeTransform {
    pub const IDENTITY: Self = Self {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    };

    pub fn matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }
}

impl Default for NodeTransform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// One node of the model hierarchy. Non-bone nodes, attachment nodes and
/// mesh associations are all retained; multiple roots sit under the
/// asset's synthetic root (a `None` parent).
#[derive(Clone, Debug, PartialEq)]
pub struct SceneNode {
    pub id: NodeId,
    pub parent: Option<NodeId>,
    /// Source identity used for binding and for diagnostics.
    pub name: String,
    /// Default local transform. Missing animation channels sample to
    /// these values; this is the default/rest pose, which a source may
    /// define differently from the skin bind pose.
    pub local: NodeTransform,
    /// Mesh attached to this node (index into `meshes` on the asset).
    pub mesh: Option<usize>,
}

/// Per-vertex skin influences. Each entry is `(joint slot, weight)`; the
/// slot indexes into [`SkinBinding::joints`]. An empty list means the
/// vertex follows the mesh node's transform rigidly. Influences beyond
/// the fourth are preserved: the CPU reference path supports variable
/// counts, and a future GPU path must never silently discard them.
pub type VertexSkin = SmallVec<[(u32, f32); 4]>;

/// Skin binding for one mesh instance: the joint-to-node table, per-joint
/// bind corrections and per-vertex influences.
#[derive(Clone, Debug)]
pub struct SkinBinding {
    /// Joint slots map to nodes in the model hierarchy.
    pub joints: Vec<NodeId>,
    /// `Bj`: maps the mesh's bind-local coordinates into joint `j`'s
    /// bind-local coordinates (adapter-normalized). Parallel to `joints`.
    pub inverse_bind: Vec<Mat4>,
    /// One influence list per mesh vertex.
    pub weights: Vec<VertexSkin>,
}

/// Geometry for one mesh, kept in bind/local space so poses can deform
/// it. Topology and UVs never change during playback.
#[derive(Clone, Debug)]
pub struct MeshAsset {
    pub name: String,
    pub texture_name: Option<String>,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub diffuse: Option<SceneTexture>,
    pub skin: Option<SkinBinding>,
}

/// Model validation failure. Invalid assets are rejected at admission,
/// never partially evaluated.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum ModelError {
    #[error("node id {node} does not match its position {index}; adapters must assign dense ids")]
    NonDenseNodeIds { node: u32, index: usize },
    #[error("node {node} has parent {parent} which does not exist")]
    MissingParent { node: u32, parent: u32 },
    #[error("the hierarchy contains a cycle involving node {node}")]
    CyclicHierarchy { node: u32 },
    #[error("node '{node}' attaches mesh {mesh} which does not exist")]
    MissingMesh { node: String, mesh: usize },
    #[error("root-motion node {0} does not exist")]
    MissingRootMotionNode(u32),
    #[error("mesh '{mesh}' skin joint slot {slot} is out of range ({count} joints)")]
    SkinJointOutOfRange {
        mesh: String,
        slot: usize,
        count: usize,
    },
    #[error("mesh '{mesh}' skin joint {joint} maps to node {node} which does not exist")]
    SkinJointNodeMissing {
        mesh: String,
        joint: usize,
        node: u32,
    },
    #[error("mesh '{mesh}' skin has {bind} bind matrices for {joints} joints")]
    BindMatrixCountMismatch {
        mesh: String,
        bind: usize,
        joints: usize,
    },
    #[error("mesh '{mesh}' skin has {weights} weight lists for {vertices} vertices")]
    WeightCountMismatch {
        mesh: String,
        weights: usize,
        vertices: usize,
    },
    #[error("mesh '{mesh}' vertex {vertex} has a negative or non-finite weight")]
    InvalidWeight { mesh: String, vertex: usize },
    #[error("mesh '{mesh}' has an index {index} outside its {vertices} vertices")]
    IndexOutOfRange {
        mesh: String,
        index: u32,
        vertices: usize,
    },
    #[error("mesh '{mesh}' has non-finite vertex data")]
    NonFiniteVertex { mesh: String },
}

/// Validated, immutable model asset shared through `Arc`.
#[derive(Clone, Debug)]
pub struct ModelAsset {
    pub name: String,
    /// Source identity (archive entry, loose path, or `synthetic:…`).
    pub source_identity: String,
    pub nodes: Vec<SceneNode>,
    pub meshes: Vec<MeshAsset>,
    /// Topological evaluation order (parents before children), kept
    /// alongside the source order/ID mapping in `nodes`.
    pub eval_order: Vec<NodeId>,
    /// Source-space → Y-up viewer transform, applied at the display root.
    pub source_to_view: Mat4,
    /// Source base orientation, retained for labels and gizmo mapping.
    pub source_orientation: BaseOrientation,
    /// Node whose translation carries root motion (adapter-designated).
    pub root_motion_node: Option<NodeId>,
    /// Non-fatal admissions (e.g. renormalized weights).
    pub diagnostics: Vec<String>,
}

impl ModelAsset {
    /// Build and validate an asset. `nodes` must use dense `NodeId`s
    /// matching their vector index (adapters assign ids on insertion).
    pub fn new(
        name: String,
        source_identity: String,
        nodes: Vec<SceneNode>,
        meshes: Vec<MeshAsset>,
        source_to_view: Mat4,
        source_orientation: BaseOrientation,
        root_motion_node: Option<NodeId>,
    ) -> Result<Self, ModelError> {
        let mut asset = Self {
            name,
            source_identity,
            nodes,
            meshes,
            eval_order: Vec::new(),
            source_to_view,
            source_orientation,
            root_motion_node,
            diagnostics: Vec::new(),
        };
        asset.eval_order = asset.compute_eval_order()?;
        asset.validate_meshes()?;
        Ok(asset)
    }

    pub fn node(&self, id: NodeId) -> Option<&SceneNode> {
        // Ids are dense after validation, so this is a direct index.
        self.nodes.get(id.0 as usize).filter(|node| node.id == id)
    }

    pub fn node_by_name(&self, name: &str) -> Option<&SceneNode> {
        self.nodes.iter().find(|node| node.name == name)
    }

    /// All nodes carrying `name` (sources can reuse names; the binder
    /// reports these as ambiguous instead of guessing).
    pub fn nodes_named(&self, name: &str) -> Vec<NodeId> {
        self.nodes
            .iter()
            .filter(|node| node.name == name)
            .map(|node| node.id)
            .collect()
    }

    /// Node owning `mesh_index`, if any (meshes may dangle under the
    /// synthetic root with an identity model transform).
    pub fn mesh_node(&self, mesh_index: usize) -> Option<NodeId> {
        self.nodes
            .iter()
            .find(|node| node.mesh == Some(mesh_index))
            .map(|node| node.id)
    }

    fn compute_eval_order(&self) -> Result<Vec<NodeId>, ModelError> {
        // Kahn's algorithm over parent→child edges keeps parents ahead of
        // children and rejects cycles explicitly.
        let count = self.nodes.len();
        let mut children: Vec<Vec<usize>> = vec![Vec::new(); count];
        let mut indegree = vec![0usize; count];
        let mut queue = Vec::new();
        for (index, node) in self.nodes.iter().enumerate() {
            if node.id.0 as usize != index {
                return Err(ModelError::NonDenseNodeIds {
                    node: node.id.0,
                    index,
                });
            }
            match node.parent {
                None => queue.push(index),
                Some(parent) => {
                    let parent_index = parent.0 as usize;
                    if parent_index >= count {
                        return Err(ModelError::MissingParent {
                            node: node.id.0,
                            parent: parent.0,
                        });
                    }
                    if parent_index == index {
                        return Err(ModelError::CyclicHierarchy { node: node.id.0 });
                    }
                    children[parent_index].push(index);
                    indegree[index] += 1;
                }
            }
        }
        let mut cursor = 0;
        let mut order = Vec::with_capacity(count);
        while cursor < queue.len() {
            let index = queue[cursor];
            cursor += 1;
            order.push(NodeId(index as u32));
            for &child in &children[index] {
                indegree[child] -= 1;
                if indegree[child] == 0 {
                    queue.push(child);
                }
            }
        }
        if order.len() != count {
            let stuck = indegree.iter().position(|&degree| degree > 0).unwrap_or(0) as u32;
            return Err(ModelError::CyclicHierarchy { node: stuck });
        }
        Ok(order)
    }

    fn validate_meshes(&mut self) -> Result<(), ModelError> {
        for node in &self.nodes {
            if let Some(mesh) = node.mesh
                && mesh >= self.meshes.len()
            {
                return Err(ModelError::MissingMesh {
                    node: node.name.clone(),
                    mesh,
                });
            }
        }
        if let Some(root) = self.root_motion_node
            && root.0 as usize >= self.nodes.len()
        {
            return Err(ModelError::MissingRootMotionNode(root.0));
        }
        let node_count = self.nodes.len();
        let mut renormalize_notes = Vec::new();
        for mesh in &mut self.meshes {
            for vertex in &mesh.vertices {
                let finite = vertex
                    .position
                    .iter()
                    .chain(vertex.normal.iter())
                    .chain(vertex.uv.iter())
                    .all(|value| value.is_finite());
                if !finite {
                    return Err(ModelError::NonFiniteVertex {
                        mesh: mesh.name.clone(),
                    });
                }
            }
            if let Some(&index) = mesh
                .indices
                .iter()
                .find(|&&index| index as usize >= mesh.vertices.len())
            {
                return Err(ModelError::IndexOutOfRange {
                    mesh: mesh.name.clone(),
                    index,
                    vertices: mesh.vertices.len(),
                });
            }
            if let Some(skin) = &mut mesh.skin {
                if skin.inverse_bind.len() != skin.joints.len() {
                    return Err(ModelError::BindMatrixCountMismatch {
                        mesh: mesh.name.clone(),
                        bind: skin.inverse_bind.len(),
                        joints: skin.joints.len(),
                    });
                }
                for (joint, &node) in skin.joints.iter().enumerate() {
                    if node.0 as usize >= node_count {
                        return Err(ModelError::SkinJointNodeMissing {
                            mesh: mesh.name.clone(),
                            joint,
                            node: node.0,
                        });
                    }
                }
                if skin.weights.len() != mesh.vertices.len() {
                    return Err(ModelError::WeightCountMismatch {
                        mesh: mesh.name.clone(),
                        weights: skin.weights.len(),
                        vertices: mesh.vertices.len(),
                    });
                }
                for (vertex, influences) in skin.weights.iter_mut().enumerate() {
                    for &(slot, weight) in influences.iter() {
                        if slot as usize >= skin.joints.len() {
                            return Err(ModelError::SkinJointOutOfRange {
                                mesh: mesh.name.clone(),
                                slot: slot as usize,
                                count: skin.joints.len(),
                            });
                        }
                        if !weight.is_finite() || weight < 0.0 {
                            return Err(ModelError::InvalidWeight {
                                mesh: mesh.name.clone(),
                                vertex,
                            });
                        }
                    }
                    let sum: f32 = influences.iter().map(|&(_, weight)| weight).sum();
                    if !influences.is_empty() && sum <= 0.0 {
                        return Err(ModelError::InvalidWeight {
                            mesh: mesh.name.clone(),
                            vertex,
                        });
                    }
                    if !influences.is_empty() && (sum - 1.0).abs() > 1e-3 {
                        // Renormalize with a diagnostic; the correction is
                        // visible instead of silently changing the bind.
                        for influence in influences.iter_mut() {
                            influence.1 /= sum;
                        }
                        renormalize_notes.push(format!(
                            "mesh '{}' vertex {vertex}: weights summed to {sum:.4}; renormalized",
                            mesh.name
                        ));
                    }
                }
            }
        }
        self.diagnostics.extend(renormalize_notes);
        Ok(())
    }

    /// Default local transforms (rest pose defaults) for every node.
    pub fn default_locals(&self) -> Vec<NodeTransform> {
        self.nodes.iter().map(|node| node.local).collect()
    }

    /// Compose model-space world matrices from local transforms. The
    /// evaluation order guarantees parents are composed before children.
    pub fn compose_world(&self, locals: &[NodeTransform]) -> Vec<Mat4> {
        debug_assert_eq!(locals.len(), self.nodes.len());
        let mut world = vec![Mat4::IDENTITY; self.nodes.len()];
        self.compose_world_into(locals, &mut world);
        world
    }

    /// Allocation-free variant of [`compose_world`](Self::compose_world)
    /// writing into a caller-owned scratch buffer.
    pub fn compose_world_into(&self, locals: &[NodeTransform], world: &mut [Mat4]) {
        debug_assert_eq!(locals.len(), self.nodes.len());
        debug_assert_eq!(world.len(), self.nodes.len());
        for &id in &self.eval_order {
            let index = id.0 as usize;
            let local = locals[index].matrix();
            world[index] = match self.nodes[index].parent {
                Some(parent) => world[parent.0 as usize] * local,
                None => local,
            };
        }
    }

    /// The unit "up" direction of the ground plane expressed in model
    /// (source) space — the inverse-mapped viewer +Y. Root-motion and
    /// follow policies use it to separate horizontal from vertical motion
    /// without assuming the source's axis convention.
    pub fn ground_normal_model(&self) -> Vec3 {
        let up = self
            .source_to_view
            .inverse()
            .transform_vector3(Vec3::Y)
            .try_normalize()
            .unwrap_or(Vec3::Y);
        if up.is_finite() { up } else { Vec3::Y }
    }

    pub fn has_skinning(&self) -> bool {
        self.meshes.iter().any(|mesh| mesh.skin.is_some())
    }

    /// Bridge from the current flattened static scene: one rigid node per
    /// mesh with identity locals and an identity `source_to_view`, because
    /// the flattened data is already in viewer coordinates. The source
    /// orientation is retained for labels/gizmo mapping only — applying it
    /// twice would repeat the earlier model-orientation bugs.
    pub fn from_flattened_scene(
        scene: &crate::inspector::scene3d::scene::Scene,
        source_identity: String,
    ) -> Self {
        let meshes: Vec<MeshAsset> = scene
            .meshes
            .iter()
            .map(|mesh| MeshAsset {
                name: mesh.name.clone(),
                texture_name: mesh.texture_name.clone(),
                vertices: mesh.vertices.clone(),
                indices: mesh.indices.clone(),
                diffuse: mesh.diffuse.clone(),
                skin: None,
            })
            .collect();
        let nodes: Vec<SceneNode> = meshes
            .iter()
            .enumerate()
            .map(|(index, mesh)| SceneNode {
                id: NodeId(index as u32),
                parent: None,
                name: mesh.name.clone(),
                local: NodeTransform::IDENTITY,
                mesh: Some(index),
            })
            .collect();
        // Identity transforms make bridge validation infallible.
        Self::new(
            scene
                .meshes
                .first()
                .map(|mesh| mesh.name.clone())
                .unwrap_or_else(|| "flattened scene".to_string()),
            source_identity,
            nodes,
            meshes,
            Mat4::IDENTITY,
            scene.base_orientation,
            None,
        )
        .expect("flattened-scene bridge is always valid")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_nodes() -> Vec<SceneNode> {
        vec![
            SceneNode {
                id: NodeId(0),
                parent: None,
                name: "root".into(),
                local: NodeTransform {
                    translation: Vec3::new(1.0, 0.0, 0.0),
                    ..NodeTransform::IDENTITY
                },
                mesh: None,
            },
            SceneNode {
                id: NodeId(1),
                parent: Some(NodeId(0)),
                name: "child".into(),
                local: NodeTransform {
                    translation: Vec3::new(0.0, 2.0, 0.0),
                    ..NodeTransform::IDENTITY
                },
                mesh: None,
            },
        ]
    }

    #[test]
    fn hierarchy_composes_parent_before_child() {
        let model = ModelAsset::new(
            "t".into(),
            "test".into(),
            simple_nodes(),
            Vec::new(),
            Mat4::IDENTITY,
            BaseOrientation::Yup,
            None,
        )
        .unwrap();
        let world = model.compose_world(&model.default_locals());
        let child_origin = world[1].transform_point3(Vec3::ZERO);
        assert!(child_origin.abs_diff_eq(Vec3::new(1.0, 2.0, 0.0), 1e-5));
    }

    #[test]
    fn cycles_are_rejected() {
        let mut nodes = simple_nodes();
        nodes[0].parent = Some(NodeId(1));
        assert!(matches!(
            ModelAsset::new(
                "t".into(),
                "test".into(),
                nodes,
                Vec::new(),
                Mat4::IDENTITY,
                BaseOrientation::Yup,
                None,
            ),
            Err(ModelError::CyclicHierarchy { .. })
        ));
    }

    #[test]
    fn missing_parent_is_rejected() {
        let mut nodes = simple_nodes();
        nodes[1].parent = Some(NodeId(9));
        assert!(matches!(
            ModelAsset::new(
                "t".into(),
                "test".into(),
                nodes,
                Vec::new(),
                Mat4::IDENTITY,
                BaseOrientation::Yup,
                None,
            ),
            Err(ModelError::MissingParent { .. })
        ));
    }

    #[test]
    fn non_dense_ids_are_rejected() {
        let mut nodes = simple_nodes();
        nodes[1].id = NodeId(7);
        assert!(matches!(
            ModelAsset::new(
                "t".into(),
                "test".into(),
                nodes,
                Vec::new(),
                Mat4::IDENTITY,
                BaseOrientation::Yup,
                None,
            ),
            Err(ModelError::NonDenseNodeIds { .. })
        ));
    }

    #[test]
    fn bridge_preserves_identity_transform_and_orientation_label() {
        let mut scene = Scene::empty(BaseOrientation::Zup);
        scene.meshes.push(SceneMesh {
            name: "flat".into(),
            texture_name: Some("tex".into()),
            vertices: vec![Vertex {
                position: [1.0, 2.0, 3.0],
                normal: [0.0, 1.0, 0.0],
                uv: [0.5, 0.25],
            }],
            indices: vec![0, 0, 0],
            diffuse: None,
            aabb: Aabb::default(),
        });
        let model = ModelAsset::from_flattened_scene(&scene, "test".into());
        assert_eq!(model.source_to_view, Mat4::IDENTITY);
        assert_eq!(model.source_orientation, BaseOrientation::Zup);
        assert_eq!(model.nodes.len(), 1);
        assert_eq!(model.meshes[0].vertices, scene.meshes[0].vertices);
    }

    use crate::inspector::scene3d::mesh::{Aabb, SceneMesh};
    use crate::inspector::scene3d::scene::Scene;
}
