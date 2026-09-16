//! RenderWare DFF (Drawable File Format) parser.
//!
//! GTA III, Vice City, and San Andreas PC models are RenderWare clumps. The
//! parser intentionally focuses on the shared, non-native geometry stream:
//! positions, normals, UVs, triangle records, and the diffuse names carried
//! by the material list. Console-native geometry remains an explicit
//! unsupported case instead of being mistaken for ordinary vertex data.
//!
//! A second, rig-preserving entry point ([`parse_dff_rig`]) keeps the frame
//! hierarchy, frame names, HAnimPLG bone data, and SkinPLG skin weights so
//! the animation adapter can build a deformable model. The flat path bakes
//! frame transforms into vertices and remains the viewer/export default.

use std::collections::BTreeMap;

const CLUMP: u32 = 0x10;
const GEOMETRY: u32 = 0x0F;
const GEOMETRY_LIST: u32 = 0x1A;
const STRUCT: u32 = 0x01;
const STRING: u32 = 0x02;
const EXTENSION: u32 = 0x03;
const MATERIAL: u32 = 0x07;
const MATERIAL_LIST: u32 = 0x08;
const FRAME_LIST: u32 = 0x0E;
const TEXTURE: u32 = 0x06;
const ATOMIC: u32 = 0x14;

const HANIM_PLG: u32 = 0x011E;
const SKIN_PLG: u32 = 0x0116;
/// The Frame List's per-frame node-name plugin (a bare null-terminated
/// string), used by every skinned GTA export.
const NODE_NAME_PLG: u32 = 0x0253_F2FE;

const FLAG_TRI_STRIP: u32 = 0x0000_0001;
const FLAG_TEXTURED: u32 = 0x0000_0004;
const FLAG_PRELIT: u32 = 0x0000_0008;
const FLAG_TEXTURED2: u32 = 0x0000_0080;
const FLAG_NATIVE: u32 = 0x0100_0000;

const MAX_GEOMETRY_VERTICES: usize = 4_000_000;
const MAX_GEOMETRY_TRIANGLES: usize = 8_000_000;
const MAX_MORPH_TARGETS: usize = 32;
const MAX_FRAMES: usize = 16_384;
const MAX_GEOMETRIES: usize = 16_384;
const MAX_ATOMICS: usize = 16_384;

/// Parsed mesh from one GEOMETRY section within a DFF Clump.
#[derive(Debug, Clone)]
pub struct DffMesh {
    /// Stable display name assigned from the geometry/material position.
    pub name: String,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
    pub material_name: Option<String>,
    pub texture_name: Option<String>,
}

#[derive(Clone, Copy, Debug)]
struct Section {
    kind: u32,
    start: usize,
    end: usize,
    version: u32,
}

#[derive(Clone, Debug)]
struct Triangle {
    a: u16,
    b: u16,
    c: u16,
    material: u16,
}

#[derive(Debug, Clone)]
struct GeometryData {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    triangles: Vec<Triangle>,
    material_textures: Vec<Option<String>>,
    /// Raw SKIN PLG body stashed during the section walk; the modern
    /// (geometry-level) variant is parsed once the vertex count is known.
    skin_body: Option<Vec<u8>>,
}

#[derive(Clone, Copy, Debug)]
struct FrameData {
    /// RenderWare stores the local basis as right, up, and at vectors.
    basis: [[f32; 3]; 3],
    position: [f32; 3],
    parent: i32,
}

#[derive(Clone, Debug)]
struct AtomicData {
    frame: usize,
    geometry: Option<usize>,
    /// Raw SKIN PLG body from the atomic's extension (the legacy,
    /// atomic-level skin variant of old RenderWare versions).
    skin_body: Option<Vec<u8>>,
}

#[derive(Debug, Default)]
struct ClumpData {
    frames: Vec<FrameData>,
    frame_names: Vec<Option<String>>,
    frame_hanims: Vec<Option<DffHAnim>>,
    geometries: Vec<GeometryData>,
    atomics: Vec<AtomicData>,
}

#[derive(Clone, Copy, Debug)]
struct AffineTransform {
    /// Basis vectors are columns: right, up, and at.
    basis: [[f32; 3]; 3],
    translation: [f32; 3],
}

impl AffineTransform {
    const IDENTITY: Self = Self {
        basis: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        translation: [0.0, 0.0, 0.0],
    };

    fn from_frame(frame: FrameData) -> Self {
        Self {
            basis: frame.basis,
            translation: frame.position,
        }
    }

    fn transform_point(self, point: [f32; 3]) -> [f32; 3] {
        let vector = self.transform_vector(point);
        [
            vector[0] + self.translation[0],
            vector[1] + self.translation[1],
            vector[2] + self.translation[2],
        ]
    }

    fn transform_vector(self, vector: [f32; 3]) -> [f32; 3] {
        [
            self.basis[0][0] * vector[0]
                + self.basis[1][0] * vector[1]
                + self.basis[2][0] * vector[2],
            self.basis[0][1] * vector[0]
                + self.basis[1][1] * vector[1]
                + self.basis[2][1] * vector[2],
            self.basis[0][2] * vector[0]
                + self.basis[1][2] * vector[1]
                + self.basis[2][2] * vector[2],
        ]
    }

    fn compose(self, local: Self) -> Self {
        Self {
            basis: [
                self.transform_vector(local.basis[0]),
                self.transform_vector(local.basis[1]),
                self.transform_vector(local.basis[2]),
            ],
            translation: self.transform_point(local.translation),
        }
    }
}

struct Cursor<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn take(&mut self, amount: usize, what: &str) -> Result<&'a [u8], String> {
        let end = self
            .position
            .checked_add(amount)
            .ok_or_else(|| format!("{what} size overflowed"))?;
        let bytes = self
            .bytes
            .get(self.position..end)
            .ok_or_else(|| format!("unexpected end reading {what}"))?;
        self.position = end;
        Ok(bytes)
    }

    fn skip(&mut self, amount: usize, what: &str) -> Result<(), String> {
        let _ = self.take(amount, what)?;
        Ok(())
    }

    fn u16(&mut self, what: &str) -> Result<u16, String> {
        Ok(u16::from_le_bytes(
            self.take(2, what)?.try_into().expect("bounded read"),
        ))
    }

    fn u32(&mut self, what: &str) -> Result<u32, String> {
        Ok(u32::from_le_bytes(
            self.take(4, what)?.try_into().expect("bounded read"),
        ))
    }

    fn f32(&mut self, what: &str) -> Result<f32, String> {
        Ok(f32::from_le_bytes(
            self.take(4, what)?.try_into().expect("bounded read"),
        ))
    }

    fn i32(&mut self, what: &str) -> Result<i32, String> {
        Ok(i32::from_le_bytes(
            self.take(4, what)?.try_into().expect("bounded read"),
        ))
    }
}

/// Parse a DFF file and return all meshes found.
pub fn parse_dff(bytes: &[u8]) -> Result<Vec<DffMesh>, String> {
    let top = read_section(bytes, 0, bytes.len())?;
    if top.kind != CLUMP {
        return Err(format!(
            "expected CLUMP section (0x10), got 0x{:02X}",
            top.kind
        ));
    }

    let mut clump = ClumpData::default();
    parse_clump_contents(bytes, top, &mut clump)?;
    if clump.geometries.is_empty() {
        return Err("DFF has no geometry sections".to_string());
    }

    let frame_transforms = resolve_frame_transforms(&clump.frames);
    let mut meshes = Vec::new();
    if clump.atomics.is_empty() {
        for (geometry_index, geometry) in clump.geometries.into_iter().enumerate() {
            append_geometry_meshes(geometry, geometry_index, None, &mut meshes);
        }
    } else {
        let mut referenced = vec![false; clump.geometries.len()];
        let mut valid_atomics = 0usize;
        for atomic in &clump.atomics {
            let Some(geometry_index) = atomic.geometry else {
                continue;
            };
            let Some(geometry) = clump.geometries.get(geometry_index).cloned() else {
                continue;
            };
            let transform = frame_transforms
                .get(atomic.frame)
                .copied()
                .unwrap_or(AffineTransform::IDENTITY);
            append_geometry_meshes(geometry, geometry_index, Some(transform), &mut meshes);
            referenced[geometry_index] = true;
            valid_atomics += 1;
        }

        // A few old exporters omit atomics or leave stale geometry indices.
        // Preserve a useful preview instead of returning a blank scene, and
        // include otherwise-unreferenced geometry for diagnostic visibility.
        if valid_atomics == 0 {
            for (geometry_index, geometry) in clump.geometries.into_iter().enumerate() {
                append_geometry_meshes(geometry, geometry_index, None, &mut meshes);
            }
        } else {
            for (geometry_index, geometry) in clump.geometries.into_iter().enumerate() {
                if !referenced[geometry_index] {
                    append_geometry_meshes(geometry, geometry_index, None, &mut meshes);
                }
            }
        }
    }

    if meshes.is_empty() {
        return Err("no renderable geometry found in DFF".to_string());
    }

    Ok(meshes)
}

fn parse_clump_contents(
    bytes: &[u8],
    clump_section: Section,
    clump: &mut ClumpData,
) -> Result<(), String> {
    let mut position = clump_section.start;
    while position < clump_section.end {
        if clump_section.end - position < 12 {
            // Entry data is sector padded, but padding cannot be inside the
            // declared clump. Tolerate a short zero tail for old exporters.
            break;
        }
        let section = read_section(bytes, position, clump_section.end)?;
        match section.kind {
            FRAME_LIST => parse_frame_list(bytes, section, clump)?,
            GEOMETRY_LIST => parse_geometry_list(bytes, section, clump)?,
            ATOMIC => parse_atomic(bytes, section, clump)?,
            // Old DFF variants may embed a geometry directly in an atomic.
            // The normal path handles that in parse_atomic; accepting a
            // direct section here is useful for lightly malformed exports.
            GEOMETRY => push_geometry(clump, parse_geometry_section(bytes, section)?)?,
            _ => {}
        }
        position = section.end;
    }
    Ok(())
}

fn parse_frame_list(bytes: &[u8], section: Section, clump: &mut ClumpData) -> Result<(), String> {
    let mut position = section.start;
    let mut parsed_struct = false;
    let mut frame_index = 0usize;
    while position < section.end {
        if section.end - position < 12 {
            break;
        }
        let child = read_section(bytes, position, section.end)?;
        if child.kind == STRUCT && !parsed_struct {
            parsed_struct = true;
            let body = &bytes[child.start..child.end];
            let mut cursor = Cursor::new(body);
            let frame_count = bounded_count(cursor.u32("frame count")?, MAX_FRAMES, "frames")?;
            let frame_bytes = frame_count
                .checked_mul(56)
                .ok_or_else(|| "frame data size overflowed".to_string())?;
            if cursor.take(frame_bytes, "frame data")?.len() != frame_bytes {
                return Err("frame data is truncated".to_string());
            }
            let frame_data = &body[4..4 + frame_bytes];
            let mut frame_cursor = Cursor::new(frame_data);
            clump.frames.reserve(frame_count);
            clump.frame_names.reserve(frame_count);
            clump.frame_hanims.reserve(frame_count);
            for _ in 0..frame_count {
                let frame = parse_frame(&mut frame_cursor)?;
                clump.frames.push(frame);
                clump.frame_names.push(None);
                clump.frame_hanims.push(None);
            }
        } else if parsed_struct && clump.frames.len() > frame_index {
            // After the STRUCT, the Frame List carries one EXTENSION section
            // per frame in frame order (node names, HAnimPLG, user data...).
            parse_frame_extension(bytes, child, frame_index, clump)?;
            frame_index += 1;
        }
        position = child.end;
    }
    Ok(())
}

fn parse_frame(cursor: &mut Cursor<'_>) -> Result<FrameData, String> {
    let right = [
        cursor.f32("frame right X")?,
        cursor.f32("frame right Y")?,
        cursor.f32("frame right Z")?,
    ];
    let up = [
        cursor.f32("frame up X")?,
        cursor.f32("frame up Y")?,
        cursor.f32("frame up Z")?,
    ];
    let at = [
        cursor.f32("frame at X")?,
        cursor.f32("frame at Y")?,
        cursor.f32("frame at Z")?,
    ];
    let position = [
        cursor.f32("frame position X")?,
        cursor.f32("frame position Y")?,
        cursor.f32("frame position Z")?,
    ];
    let parent = cursor.i32("frame parent")?;
    cursor.skip(4, "frame creation flags")?;
    Ok(FrameData {
        basis: [right, up, at],
        position,
        parent,
    })
}

fn parse_geometry_list(
    bytes: &[u8],
    section: Section,
    clump: &mut ClumpData,
) -> Result<(), String> {
    let mut position = section.start;
    while position < section.end {
        if section.end - position < 12 {
            break;
        }
        let child = read_section(bytes, position, section.end)?;
        if child.kind == GEOMETRY {
            push_geometry(clump, parse_geometry_section(bytes, child)?)?;
        }
        position = child.end;
    }
    Ok(())
}

fn parse_atomic(bytes: &[u8], section: Section, clump: &mut ClumpData) -> Result<(), String> {
    let mut position = section.start;
    let mut frame = 0usize;
    let mut geometry = None;
    let mut skin_body = None;
    let mut parsed_struct = false;
    while position < section.end {
        if section.end - position < 12 {
            break;
        }
        let child = read_section(bytes, position, section.end)?;
        match child.kind {
            STRUCT if !parsed_struct => {
                parsed_struct = true;
                let body = &bytes[child.start..child.end];
                if body.len() < 12 {
                    return Err("ATOMIC STRUCT is truncated".to_string());
                }
                frame = u32::from_le_bytes(body[0..4].try_into().expect("bounded read")) as usize;
                if body.len() >= 16 {
                    geometry = Some(
                        u32::from_le_bytes(body[4..8].try_into().expect("bounded read")) as usize,
                    );
                } else {
                    // The compact Atomic layout used by older PC DFFs
                    // omits the geometry index; RenderWare conventionally
                    // binds it to geometry 0.
                    geometry = Some(0);
                }
            }
            GEOMETRY => {
                let index = clump.geometries.len();
                push_geometry(clump, parse_geometry_section(bytes, child)?)?;
                geometry = Some(index);
            }
            EXTENSION => {
                // The legacy atomic-level skin lives in the extension.
                let mut position = child.start;
                while position < child.end {
                    if child.end - position < 12 {
                        break;
                    }
                    let plugin = read_section(bytes, position, child.end)?;
                    if plugin.kind == SKIN_PLG {
                        skin_body =
                            Some(bytes[plugin.start..plugin.end].to_vec());
                    }
                    position = plugin.end;
                }
            }
            _ => {}
        }
        position = child.end;
    }
    if parsed_struct {
        if clump.atomics.len() >= MAX_ATOMICS {
            return Err(format!("atomics exceed viewer limit {MAX_ATOMICS}"));
        }
        clump.atomics.push(AtomicData {
            frame,
            geometry,
            skin_body,
        });
    }
    Ok(())
}

fn push_geometry(clump: &mut ClumpData, geometry: GeometryData) -> Result<(), String> {
    if clump.geometries.len() >= MAX_GEOMETRIES {
        return Err(format!("geometries exceed viewer limit {MAX_GEOMETRIES}"));
    }
    clump.geometries.push(geometry);
    Ok(())
}

// ---- Rig-preserving parse (frames, names, HAnim, Skin) ------------------

/// HAnimPLG payload on one frame: the skeleton identity of a bone.
#[derive(Debug, Clone, PartialEq)]
pub struct DffHAnim {
    pub version: i32,
    pub bone_id: i32,
    pub bone_count: i32,
    /// `(bone id, frame index, node type)` triples; empty for non-root
    /// bone frames, which carry only the header.
    pub bones: Vec<DffBone>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DffBone {
    pub id: i32,
    pub index: i32,
    pub kind: i32,
}

/// SkinPLG payload: per-vertex bone influences plus the per-bone bind
/// matrices. Both the modern geometry-level variant (used by every GTA
/// PC ped/player) and the legacy atomic-level variant are preserved.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DffSkin {
    pub num_bones: usize,
    /// Modern variant only: the bones this mesh actually uses; per-vertex
    /// indices reference this array. The values are frame indices.
    pub used_bones: Vec<u8>,
    pub vertex_indices: Vec<[u8; 4]>,
    pub vertex_weights: Vec<[f32; 4]>,
    /// Bind matrices, stored row-major as 16 f32 each.
    pub bone_matrices: Vec<[[f32; 4]; 4]>,
    /// Legacy variant only: explicit per-bone records (id, frame index, type).
    pub bones: Vec<DffBone>,
    pub legacy: bool,
}

/// One frame of the preserved hierarchy.
#[derive(Debug, Clone)]
pub struct DffFrame {
    pub name: Option<String>,
    /// Local basis stored as right, up, and at rows.
    pub basis: [[f32; 3]; 3],
    pub position: [f32; 3],
    pub parent: i32,
    pub hanim: Option<DffHAnim>,
}

/// A material-split mesh in **local (unbaked) space**, attached to its
/// atomic's frame, with the geometry's skin when present.
#[derive(Debug, Clone)]
pub struct DffRigMesh {
    pub frame: usize,
    pub mesh: DffMesh,
    pub skin: Option<DffSkin>,
}

/// The rig-preserving counterpart of [`parse_dff`]'s flat output.
#[derive(Debug, Clone)]
pub struct DffRig {
    pub frames: Vec<DffFrame>,
    pub meshes: Vec<DffRigMesh>,
}

/// Parse a DFF while preserving everything an animation adapter needs:
/// the frame hierarchy with names and HAnim data, atomics wired to their
/// frames, geometry left in local space, and SkinPLG payloads (modern and
/// legacy variants). Frame transforms are NOT baked into the vertices.
pub fn parse_dff_rig(bytes: &[u8]) -> Result<DffRig, String> {
    let top = read_section(bytes, 0, bytes.len())?;
    if top.kind != CLUMP {
        return Err(format!(
            "expected CLUMP section (0x10), got 0x{:02X}",
            top.kind
        ));
    }

    let mut clump = ClumpData::default();
    parse_clump_contents(bytes, top, &mut clump)?;
    if clump.frames.is_empty() {
        return Err("DFF has no frame list; cannot build a rig".to_string());
    }

    let mut frames: Vec<DffFrame> = clump
        .frames
        .iter()
        .enumerate()
        .map(|(index, frame)| DffFrame {
            name: clump.frame_names.get(index).cloned().flatten(),
            basis: frame.basis,
            position: frame.position,
            parent: frame.parent,
            hanim: clump.frame_hanims.get(index).cloned().flatten(),
        })
        .collect();

    // Attach per-material split meshes to their atomic frames. Geometry
    // stays in local space; skinning supplies the transforms at runtime.
    let mut meshes: Vec<DffRigMesh> = Vec::new();
    let mut valid_atomics = 0usize;
    for atomic in &clump.atomics {
        let Some(geometry_index) = atomic.geometry else {
            continue;
        };
        let Some(geometry) = clump.geometries.get(geometry_index) else {
            continue;
        };
        valid_atomics += 1;
        let skin = atomic
            .skin_body
            .as_deref()
            .map(|body| parse_legacy_skin(body, geometry.positions.len()))
            .or_else(|| {
                geometry
                    .skin_body
                    .as_deref()
                    .map(|body| parse_modern_skin(body, geometry.positions.len()))
            })
            .transpose()?;
        append_rig_meshes(
            geometry,
            geometry_index,
            atomic.frame,
            skin,
            &mut meshes,
        );
    }
    if valid_atomics == 0 {
        // Old exports without atomics: hang every geometry on frame 0 so
        // the rig still validates.
        for (geometry_index, geometry) in clump.geometries.iter().enumerate() {
            let skin = geometry
                .skin_body
                .as_deref()
                .map(|body| parse_modern_skin(body, geometry.positions.len()))
                .transpose()?;
            append_rig_meshes(geometry, geometry_index, 0, skin, &mut meshes);
        }
    }

    if meshes.is_empty() {
        return Err("no renderable geometry found in DFF".to_string());
    }

    Ok(DffRig { frames, meshes })
}

fn append_rig_meshes(
    geometry: &GeometryData,
    geometry_index: usize,
    frame: usize,
    skin: Option<DffSkin>,
    meshes: &mut Vec<DffRigMesh>,
) {
    if geometry.positions.is_empty() {
        return;
    }
    let has_any_texture = geometry.material_textures.iter().any(Option::is_some);
    let mut groups: BTreeMap<u16, Vec<u32>> = BTreeMap::new();
    for triangle in &geometry.triangles {
        if triangle.a as usize >= geometry.positions.len()
            || triangle.b as usize >= geometry.positions.len()
            || triangle.c as usize >= geometry.positions.len()
            || triangle.a == triangle.b
            || triangle.a == triangle.c
            || triangle.b == triangle.c
        {
            continue;
        }
        let group = if has_any_texture {
            triangle.material
        } else {
            0
        };
        groups.entry(group).or_default().extend_from_slice(&[
            triangle.a as u32,
            triangle.b as u32,
            triangle.c as u32,
        ]);
    }
    for (material, indices) in groups {
        if indices.is_empty() {
            continue;
        }
        let texture_name = geometry
            .material_textures
            .get(material as usize)
            .and_then(Clone::clone);
        let name = if has_any_texture {
            format!("geometry-{geometry_index}-material-{material}")
        } else {
            format!("geometry-{geometry_index}")
        };
        meshes.push(DffRigMesh {
            frame,
            mesh: DffMesh {
                name,
                positions: geometry.positions.clone(),
                normals: geometry.normals.clone(),
                uvs: geometry.uvs.clone(),
                indices,
                material_name: None,
                texture_name,
            },
            skin: skin.clone(),
        });
    }
}

/// Parse a frame's extension chunks in frame order: the Frame List places
/// each frame's plugins (node name, HAnimPLG, user data, ...) sequentially
/// after the STRUCT, one EXTENSION section per frame.
fn parse_frame_extension(
    bytes: &[u8],
    section: Section,
    frame_index: usize,
    clump: &mut ClumpData,
) -> Result<(), String> {
    let Some(frame) = clump.frames.get(frame_index) else {
        return Ok(());
    };
    let _ = frame;
    let mut position = section.start;
    while position < section.end {
        if section.end - position < 12 {
            break;
        }
        let child = read_section(bytes, position, section.end)?;
        match child.kind {
            NODE_NAME_PLG => {
                let body = &bytes[child.start..child.end];
                let name = read_renderware_string(body);
                if let Some(slot) = clump.frame_names.get_mut(frame_index) {
                    *slot = (!name.is_empty()).then_some(name);
                }
            }
            HANIM_PLG => {
                let hanim = parse_hanim_plg(&bytes[child.start..child.end])?;
                if let Some(slot) = clump.frame_hanims.get_mut(frame_index) {
                    *slot = Some(hanim);
                }
            }
            _ => {}
        }
        position = child.end;
    }
    Ok(())
}

fn parse_hanim_plg(body: &[u8]) -> Result<DffHAnim, String> {
    let mut cursor = Cursor::new(body);
    let version = cursor.i32("HAnim version")?;
    let bone_id = cursor.i32("HAnim bone id")?;
    let bone_count = bounded_count(
        cursor.i32("HAnim bone count")?.max(0) as u32,
        MAX_FRAMES,
        "HAnim bones",
    )?;
    // Non-root bone frames carry only the 12-byte header. The root frame's
    // bone array is preceded by keyframe size and flag words (offset 20).
    let mut bones = Vec::new();
    if bone_count > 0 {
        cursor.skip(8, "HAnim keyframe header")?;
        for _ in 0..bone_count {
            bones.push(DffBone {
                id: cursor.i32("HAnim bone id")?,
                index: cursor.i32("HAnim bone frame index")?,
                kind: cursor.i32("HAnim bone type")?,
            });
        }
    }
    Ok(DffHAnim {
        version,
        bone_id,
        bone_count: bone_count as i32,
        bones,
    })
}

/// Modern geometry-level SKIN PLG (DragonFF `SkinPLG.from_mem(geometry)`):
/// `3×u8 header`, `used_bones[num_used]`, per-vertex `4×u8 + 4×f32`,
/// then `num_bones × 4×4 f32` matrices (optionally 12 bytes of skin-split
/// data that we do not need).
fn parse_modern_skin(body: &[u8], vertex_count: usize) -> Result<DffSkin, String> {
    let mut cursor = Cursor::new(body);
    let num_bones = cursor.take(1, "skin bone count")?[0] as usize;
    let num_used_bones = cursor.take(1, "skin used-bone count")?[0] as usize;
    let _max_weights = cursor.take(1, "skin max weights")?[0] as usize;
    cursor.skip(1, "skin header pad");
    if num_bones > MAX_FRAMES {
        return Err(format!("skin bone count {num_bones} exceeds viewer limit"));
    }

    let oldver = num_used_bones == 0;
    // DragonFF reads the influences as two contiguous blocks: 4 bytes of
    // bone indices per vertex (the whole array first), then 4 f32 weights
    // per vertex. Interleaving the two blocks garbles every weight.
    let index_bytes = vertex_count
        .checked_mul(4)
        .ok_or_else(|| "skin index size overflowed".to_string())?;
    let weight_bytes = vertex_count
        .checked_mul(16)
        .ok_or_else(|| "skin weight size overflowed".to_string())?;
    let (used_bones, vertex_indices, vertex_weights, bone_matrices) = if oldver {
        // Old RW versions omit the used-bone array entirely: per-vertex
        // indices point straight at the bone list, and each matrix is
        // preceded by a 0xDEADDEAD marker.
        let mut indices_flat = Vec::with_capacity(vertex_count * 4);
        for _ in 0..index_bytes {
            indices_flat.push(cursor.take(1, "skin vertex bone index")?[0]);
        }
        let mut vertex_weights = Vec::with_capacity(vertex_count);
        for _ in 0..vertex_count {
            let mut weights = [0.0f32; 4];
            for weight in weights.iter_mut() {
                *weight = cursor.f32("skin vertex weight")?;
            }
            vertex_weights.push(weights);
        }
        let mut bone_matrices = Vec::with_capacity(num_bones);
        for _ in 0..num_bones {
            cursor.skip(4, "skin matrix marker")?;
            bone_matrices.push(read_skin_matrix(&mut cursor)?);
        }
        let vertex_indices = flat_to_vertex_indices(indices_flat, vertex_count);
        (Vec::new(), vertex_indices, vertex_weights, bone_matrices)
    } else {
        let mut used_bones = Vec::with_capacity(num_used_bones);
        for _ in 0..num_used_bones {
            used_bones.push(cursor.take(1, "skin used bone")?[0]);
        }
        let mut indices_flat = Vec::with_capacity(vertex_count * 4);
        for _ in 0..index_bytes {
            indices_flat.push(cursor.take(1, "skin vertex bone index")?[0]);
        }
        let mut vertex_weights = Vec::with_capacity(vertex_count);
        for _ in 0..vertex_count {
            let mut weights = [0.0f32; 4];
            for weight in weights.iter_mut() {
                *weight = cursor.f32("skin vertex weight")?;
            }
            vertex_weights.push(weights);
        }
        let mut bone_matrices = Vec::with_capacity(num_bones);
        for _ in 0..num_bones {
            bone_matrices.push(read_skin_matrix(&mut cursor)?);
        }
        let vertex_indices = flat_to_vertex_indices(indices_flat, vertex_count);
        (used_bones, vertex_indices, vertex_weights, bone_matrices)
    };

    Ok(DffSkin {
        num_bones,
        used_bones,
        vertex_indices,
        vertex_weights,
        bone_matrices,
        bones: Vec::new(),
        legacy: false,
    })
}

fn read_skin_matrix(cursor: &mut Cursor<'_>) -> Result<[[f32; 4]; 4], String> {
    let mut matrix = [[0.0f32; 4]; 4];
    for row in matrix.iter_mut() {
        for value in row.iter_mut() {
            *value = cursor.f32("skin matrix")?;
        }
    }
    Ok(matrix)
}

/// Reshape the contiguous `4 × vertices` index block into per-vertex
/// four-slot arrays.
fn flat_to_vertex_indices(flat: Vec<u8>, vertex_count: usize) -> Vec<[u8; 4]> {
    let mut vertex_indices = Vec::with_capacity(vertex_count);
    for vertex in 0..vertex_count {
        let base = vertex * 4;
        vertex_indices.push([
            flat[base],
            flat.get(base + 1).copied().unwrap_or(0),
            flat.get(base + 2).copied().unwrap_or(0),
            flat.get(base + 3).copied().unwrap_or(0),
        ]);
    }
    vertex_indices
}

/// Legacy atomic-level SKIN PLG (DragonFF `from_mem(data, geometry, frame)`):
/// `2×u32 header (num_bones, vertices)`, per-vertex `4×u8 + 4×f32`, then
/// per bone a 12-byte record plus its bind matrix.
fn parse_legacy_skin(body: &[u8], vertex_count_hint: usize) -> Result<DffSkin, String> {
    let mut cursor = Cursor::new(body);
    let num_bones = bounded_count(cursor.u32("legacy skin bone count")?, MAX_FRAMES, "bones")?;
    let vertex_count = bounded_count(
        cursor.u32("legacy skin vertex count")?,
        MAX_GEOMETRY_VERTICES,
        "vertices",
    )?;
    if vertex_count != vertex_count_hint {
        return Err(format!(
            "legacy skin covers {vertex_count} vertices but the geometry has {vertex_count_hint}"
        ));
    }
    // Same contiguous index/weight blocks as the modern variant.
    let mut indices_flat = Vec::with_capacity(vertex_count * 4);
    for _ in 0..vertex_count * 4 {
        indices_flat.push(cursor.take(1, "skin vertex bone index")?[0]);
    }
    let mut vertex_weights = Vec::with_capacity(vertex_count);
    for _ in 0..vertex_count {
        let mut weights = [0.0f32; 4];
        for weight in weights.iter_mut() {
            *weight = cursor.f32("skin vertex weight")?;
        }
        vertex_weights.push(weights);
    }
    let vertex_indices = flat_to_vertex_indices(indices_flat, vertex_count);
    let mut bones = Vec::with_capacity(num_bones);
    let mut bone_matrices = Vec::with_capacity(num_bones);
    for _ in 0..num_bones {
        bones.push(DffBone {
            id: cursor.i32("legacy skin bone id")?,
            index: cursor.i32("legacy skin bone frame index")?,
            kind: cursor.i32("legacy skin bone type")? & 0x3,
        });
        bone_matrices.push(read_skin_matrix(&mut cursor)?);
    }
    Ok(DffSkin {
        num_bones,
        used_bones: Vec::new(),
        vertex_indices,
        vertex_weights,
        bone_matrices,
        bones,
        legacy: true,
    })
}

fn resolve_frame_transforms(frames: &[FrameData]) -> Vec<AffineTransform> {
    let mut resolved = vec![None; frames.len()];
    let mut visiting = vec![false; frames.len()];
    for index in 0..frames.len() {
        let _ = resolve_frame_transform(index, frames, &mut resolved, &mut visiting);
    }
    resolved
        .into_iter()
        .map(|transform| transform.unwrap_or(AffineTransform::IDENTITY))
        .collect()
}

fn resolve_frame_transform(
    index: usize,
    frames: &[FrameData],
    resolved: &mut [Option<AffineTransform>],
    visiting: &mut [bool],
) -> AffineTransform {
    if let Some(transform) = resolved[index] {
        return transform;
    }
    if visiting[index] {
        return AffineTransform::from_frame(frames[index]);
    }

    visiting[index] = true;
    let frame = frames[index];
    let local = AffineTransform::from_frame(frame);
    let transform = usize::try_from(frame.parent)
        .ok()
        .filter(|&parent| parent < frames.len() && parent != index)
        .map(|parent| resolve_frame_transform(parent, frames, resolved, visiting).compose(local))
        .unwrap_or(local);
    visiting[index] = false;
    resolved[index] = Some(transform);
    transform
}

fn parse_geometry_section(bytes: &[u8], section: Section) -> Result<GeometryData, String> {
    let mut position = section.start;
    let mut geometry = None;
    let mut material_textures = Vec::new();
    let mut skin_body = None;

    while position < section.end {
        if section.end - position < 12 {
            break;
        }
        let child = read_section(bytes, position, section.end)?;
        match child.kind {
            STRUCT if geometry.is_none() => {
                geometry = Some(parse_geometry_struct(
                    &bytes[child.start..child.end],
                    if child.version == 0 {
                        section.version
                    } else {
                        child.version
                    },
                )?);
            }
            MATERIAL_LIST => {
                material_textures = parse_material_list(bytes, child)?;
            }
            EXTENSION => {
                // Plugin chunks (e.g. the geometry-level SkinPLG) live
                // inside the extension wrapper.
                let mut position = child.start;
                while position < child.end {
                    if child.end - position < 12 {
                        break;
                    }
                    let plugin = read_section(bytes, position, child.end)?;
                    if plugin.kind == SKIN_PLG {
                        skin_body =
                            Some(bytes[plugin.start..plugin.end].to_vec());
                    }
                    position = plugin.end;
                }
            }
            _ => {}
        }
        position = child.end;
    }

    let mut geometry = geometry.ok_or_else(|| "GEOMETRY has no STRUCT section".to_string())?;
    geometry.material_textures = material_textures;
    geometry.skin_body = skin_body;
    Ok(geometry)
}

fn parse_geometry_struct(bytes: &[u8], library_id: u32) -> Result<GeometryData, String> {
    let mut cursor = Cursor::new(bytes);
    let flags = cursor.u32("geometry flags")?;
    if flags & FLAG_NATIVE != 0 {
        return Err("native RenderWare geometry is not supported by the PC viewer".to_string());
    }

    let triangle_count = bounded_count(
        cursor.u32("triangle count")?,
        MAX_GEOMETRY_TRIANGLES,
        "triangles",
    )?;
    let vertex_count = bounded_count(
        cursor.u32("vertex count")?,
        MAX_GEOMETRY_VERTICES,
        "vertices",
    )?;
    let morph_count = bounded_count(
        cursor.u32("morph target count")?.max(1),
        MAX_MORPH_TARGETS,
        "morph targets",
    )?;

    let decoded_version = decode_library_version(library_id);
    if decoded_version != 0 && decoded_version < 0x0003_4000 {
        cursor.skip(12, "pre-3.4 surface properties")?;
    }

    if flags & FLAG_PRELIT != 0 {
        cursor.skip(
            vertex_count
                .checked_mul(4)
                .ok_or_else(|| "prelit color size overflowed".to_string())?,
            "prelit colors",
        )?;
    }

    let texture_set_count = ((flags >> 16) & 0xFF) as usize;
    let texture_set_count = if texture_set_count != 0 {
        texture_set_count
    } else if flags & FLAG_TEXTURED2 != 0 {
        2
    } else if flags & FLAG_TEXTURED != 0 {
        1
    } else {
        0
    };

    let mut uvs = Vec::new();
    for set in 0..texture_set_count {
        if set == 0 {
            uvs.reserve(vertex_count);
        }
        for _ in 0..vertex_count {
            let u = cursor.f32("texture U")?;
            let v = cursor.f32("texture V")?;
            if set == 0 {
                uvs.push([u, v]);
            }
        }
    }

    let mut triangles = Vec::with_capacity(triangle_count);
    for _ in 0..triangle_count {
        // RenderWare streams triangles as b, a, material, c. Convert to the
        // viewer's conventional a, b, c order while preserving winding.
        let b = cursor.u16("triangle vertex B")?;
        let a = cursor.u16("triangle vertex A")?;
        let material = cursor.u16("triangle material")?;
        let c = cursor.u16("triangle vertex C")?;
        triangles.push(Triangle { a, b, c, material });
    }

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    for morph in 0..morph_count {
        cursor.skip(16, "morph target bounding sphere")?;
        let has_vertices = cursor.u32("morph vertex flag")? != 0;
        let has_normals = cursor.u32("morph normal flag")? != 0;
        if has_vertices {
            let vertex_bytes = vertex_count
                .checked_mul(12)
                .ok_or_else(|| "vertex data size overflowed".to_string())?;
            // The morph-target flags describe the serialized arrays more
            // reliably than the geometry flags. In particular, GTA III
            // commonly writes 0x10034 (without FLAG_POSITIONS) while still
            // storing a complete position array here.
            if morph == 0 {
                positions.reserve(vertex_count);
                for _ in 0..vertex_count {
                    positions.push([
                        cursor.f32("vertex X")?,
                        cursor.f32("vertex Y")?,
                        cursor.f32("vertex Z")?,
                    ]);
                }
            } else {
                cursor.skip(vertex_bytes, "additional morph vertices")?;
            }
        }

        if has_normals {
            let normal_bytes = vertex_count
                .checked_mul(12)
                .ok_or_else(|| "normal data size overflowed".to_string())?;
            if morph == 0 {
                normals.reserve(vertex_count);
                for _ in 0..vertex_count {
                    normals.push([
                        cursor.f32("normal X")?,
                        cursor.f32("normal Y")?,
                        cursor.f32("normal Z")?,
                    ]);
                }
            } else {
                cursor.skip(normal_bytes, "unused normal data")?;
            }
        }
    }

    // A strip flag changes how an engine may optimize the stream, but the
    // on-disk PC DFF geometry still supplies the same triangle records. Keep
    // the flag consumed here for clarity and future native-strip support.
    let _is_triangle_strip = flags & FLAG_TRI_STRIP != 0;

    Ok(GeometryData {
        positions,
        normals,
        uvs,
        triangles,
        material_textures: Vec::new(),
        skin_body: None,
    })
}

fn append_geometry_meshes(
    mut geometry: GeometryData,
    geometry_index: usize,
    transform: Option<AffineTransform>,
    meshes: &mut Vec<DffMesh>,
) {
    if geometry.positions.is_empty() {
        return;
    }

    if let Some(transform) = transform {
        for position in &mut geometry.positions {
            *position = transform.transform_point(*position);
        }
        for normal in &mut geometry.normals {
            *normal = transform.transform_vector(*normal);
        }
    }

    let has_any_texture = geometry.material_textures.iter().any(Option::is_some);
    let mut groups: BTreeMap<u16, Vec<u32>> = BTreeMap::new();
    for triangle in geometry.triangles {
        if triangle.a as usize >= geometry.positions.len()
            || triangle.b as usize >= geometry.positions.len()
            || triangle.c as usize >= geometry.positions.len()
            || triangle.a == triangle.b
            || triangle.a == triangle.c
            || triangle.b == triangle.c
        {
            continue;
        }
        let group = if has_any_texture {
            triangle.material
        } else {
            0
        };
        groups.entry(group).or_default().extend_from_slice(&[
            triangle.a as u32,
            triangle.b as u32,
            triangle.c as u32,
        ]);
    }

    for (material, indices) in groups {
        if indices.is_empty() {
            continue;
        }
        let texture_name = geometry
            .material_textures
            .get(material as usize)
            .and_then(Clone::clone);
        let name = if has_any_texture {
            format!("geometry-{geometry_index}-material-{material}")
        } else {
            format!("geometry-{geometry_index}")
        };
        meshes.push(DffMesh {
            name,
            positions: geometry.positions.clone(),
            normals: geometry.normals.clone(),
            uvs: geometry.uvs.clone(),
            indices,
            material_name: None,
            texture_name,
        });
    }
}

fn parse_material_list(bytes: &[u8], section: Section) -> Result<Vec<Option<String>>, String> {
    let mut position = section.start;
    let mut textures = Vec::new();
    while position < section.end {
        if section.end - position < 12 {
            break;
        }
        let child = read_section(bytes, position, section.end)?;
        if child.kind == MATERIAL {
            textures.push(parse_material(bytes, child)?);
        }
        position = child.end;
    }
    Ok(textures)
}

fn parse_material(bytes: &[u8], section: Section) -> Result<Option<String>, String> {
    let mut position = section.start;
    while position < section.end {
        if section.end - position < 12 {
            break;
        }
        let child = read_section(bytes, position, section.end)?;
        if child.kind == TEXTURE
            && let Some(name) = parse_texture(bytes, child)?
        {
            return Ok(Some(name));
        }
        position = child.end;
    }
    Ok(None)
}

fn parse_texture(bytes: &[u8], section: Section) -> Result<Option<String>, String> {
    let mut position = section.start;
    while position < section.end {
        if section.end - position < 12 {
            break;
        }
        let child = read_section(bytes, position, section.end)?;
        if child.kind == STRING {
            let name = read_renderware_string(&bytes[child.start..child.end]);
            if !name.is_empty() {
                return Ok(Some(name));
            }
        }
        position = child.end;
    }
    Ok(None)
}

fn read_renderware_string(bytes: &[u8]) -> String {
    let bytes = bytes.split(|&byte| byte == 0).next().unwrap_or_default();
    String::from_utf8_lossy(bytes).trim().to_string()
}

fn read_section(bytes: &[u8], position: usize, limit: usize) -> Result<Section, String> {
    let header_end = position
        .checked_add(12)
        .ok_or_else(|| "section header offset overflowed".to_string())?;
    if header_end > limit || header_end > bytes.len() {
        return Err("unexpected end of section header".to_string());
    }
    let kind = u32::from_le_bytes(bytes[position..position + 4].try_into().expect("header"));
    let size = u32::from_le_bytes(
        bytes[position + 4..position + 8]
            .try_into()
            .expect("header"),
    ) as usize;
    let version = u32::from_le_bytes(
        bytes[position + 8..position + 12]
            .try_into()
            .expect("header"),
    );
    let start = header_end;
    let end = start
        .checked_add(size)
        .ok_or_else(|| format!("section 0x{kind:02X} size overflowed"))?;
    if end > limit || end > bytes.len() {
        return Err(format!(
            "section 0x{kind:02X} size {size} exceeds its container"
        ));
    }
    Ok(Section {
        kind,
        start,
        end,
        version,
    })
}

fn bounded_count(raw: u32, maximum: usize, label: &str) -> Result<usize, String> {
    let count = usize::try_from(raw).map_err(|_| format!("{label} count does not fit usize"))?;
    if count > maximum {
        return Err(format!(
            "{label} count {count} exceeds viewer limit {maximum}"
        ));
    }
    Ok(count)
}

fn decode_library_version(library_id: u32) -> u32 {
    // Some tools write the already-unpacked 0x3xxxx version into the
    // section header. Do not run that value through the packed-ID formula a
    // second time.
    if (0x0003_0000..=0x0003_FFFF).contains(&library_id) {
        library_id
    } else if library_id & 0xFFFF_0000 != 0 {
        (((library_id >> 14) & 0x0003_FF00) + 0x0003_0000) | ((library_id >> 16) & 0x3F)
    } else if library_id != 0 {
        library_id << 8
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn section(kind: u32, version: u32, body: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(12 + body.len());
        bytes.extend_from_slice(&kind.to_le_bytes());
        bytes.extend_from_slice(&(body.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&version.to_le_bytes());
        bytes.extend_from_slice(body);
        bytes
    }

    fn actual_layout_fixture() -> Vec<u8> {
        let mut geometry_struct = Vec::new();
        geometry_struct.extend_from_slice(&0x0001_0037_u32.to_le_bytes());
        geometry_struct.extend_from_slice(&1_u32.to_le_bytes());
        geometry_struct.extend_from_slice(&3_u32.to_le_bytes());
        geometry_struct.extend_from_slice(&1_u32.to_le_bytes());
        for uv in [[0.0_f32, 0.0], [1.0, 0.0], [0.0, 1.0]] {
            geometry_struct.extend_from_slice(&uv[0].to_le_bytes());
            geometry_struct.extend_from_slice(&uv[1].to_le_bytes());
        }
        // b, a, material, c => viewer indices [0, 1, 2]
        geometry_struct.extend_from_slice(&1_u16.to_le_bytes());
        geometry_struct.extend_from_slice(&0_u16.to_le_bytes());
        geometry_struct.extend_from_slice(&0_u16.to_le_bytes());
        geometry_struct.extend_from_slice(&2_u16.to_le_bytes());
        geometry_struct.extend_from_slice(&[0_u8; 16]);
        geometry_struct.extend_from_slice(&1_u32.to_le_bytes());
        geometry_struct.extend_from_slice(&1_u32.to_le_bytes());
        for point in [[0.0_f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]] {
            for value in point {
                geometry_struct.extend_from_slice(&value.to_le_bytes());
            }
        }
        for _ in 0..3 {
            geometry_struct.extend_from_slice(&0.0_f32.to_le_bytes());
            geometry_struct.extend_from_slice(&1.0_f32.to_le_bytes());
            geometry_struct.extend_from_slice(&0.0_f32.to_le_bytes());
        }

        let string = section(STRING, 0, b"brick\0\0");
        let mut texture_body = section(STRUCT, 0, &[0; 4]);
        texture_body.extend_from_slice(&string);
        let texture = section(TEXTURE, 0, &texture_body);
        let mut material_body = section(STRUCT, 0, &[0; 28]);
        material_body.extend_from_slice(&texture);
        let material = section(MATERIAL, 0, &material_body);
        let mut material_list_body = section(STRUCT, 0, &[1, 0, 0, 0, 0, 0, 0, 0]);
        material_list_body.extend_from_slice(&material);

        let mut geometry_body = section(STRUCT, 0x0003_6003, &geometry_struct);
        geometry_body.extend_from_slice(&section(MATERIAL_LIST, 0, &material_list_body));
        let geometry = section(GEOMETRY, 0x1803_FFFF, &geometry_body);
        let mut geometry_list_body = section(STRUCT, 0, &[1, 0, 0, 0]);
        geometry_list_body.extend_from_slice(&geometry);
        let geometry_list = section(GEOMETRY_LIST, 0x1803_FFFF, &geometry_list_body);
        let mut clump_body = section(STRUCT, 0, &[0; 12]);
        clump_body.extend_from_slice(&geometry_list);
        section(CLUMP, 0x1803_FFFF, &clump_body)
    }

    fn compact_atomic_fixture() -> Vec<u8> {
        let mut frame_body = Vec::new();
        frame_body.extend_from_slice(&1_u32.to_le_bytes());
        for values in [
            [1.0_f32, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [10.0, 0.0, 0.0],
        ] {
            for value in values {
                frame_body.extend_from_slice(&value.to_le_bytes());
            }
        }
        frame_body.extend_from_slice(&(-1_i32).to_le_bytes());
        frame_body.extend_from_slice(&0_u32.to_le_bytes());
        let frame_list = section(
            FRAME_LIST,
            0x1803_FFFF,
            &section(STRUCT, 0x1803_FFFF, &frame_body),
        );
        let atomic = section(
            ATOMIC,
            0x1803_FFFF,
            &section(STRUCT, 0x1803_FFFF, &[0_u8; 12]),
        );

        let base = actual_layout_fixture();
        let top = read_section(&base, 0, base.len()).expect("base fixture section");
        let mut clump_body = base[top.start..top.end].to_vec();
        clump_body.extend_from_slice(&frame_list);
        clump_body.extend_from_slice(&atomic);
        section(CLUMP, top.version, &clump_body)
    }

    #[test]
    fn reject_empty() {
        assert!(parse_dff(&[]).is_err());
    }

    #[test]
    fn reject_non_clump() {
        let bytes = section(STRUCT, 0x1003_FFFF, &[]);
        assert!(parse_dff(&bytes).is_err());
    }

    #[test]
    fn parses_geometry_list_and_actual_triangle_layout() {
        let meshes = parse_dff(&actual_layout_fixture()).expect("fixture should parse");
        assert_eq!(meshes.len(), 1);
        assert_eq!(meshes[0].positions.len(), 3);
        assert_eq!(meshes[0].uvs.len(), 3);
        assert_eq!(meshes[0].indices, vec![0, 1, 2]);
        assert_eq!(meshes[0].texture_name.as_deref(), Some("brick"));
    }

    #[test]
    fn reads_morph_positions_when_gta_iii_omits_position_flag() {
        let mut fixture = actual_layout_fixture();
        let position_flag = 0x0001_0037_u32.to_le_bytes();
        let position = fixture
            .windows(4)
            .position(|bytes| bytes == position_flag)
            .expect("fixture geometry flags");
        fixture[position..position + 4].copy_from_slice(&0x0001_0035_u32.to_le_bytes());

        let meshes = parse_dff(&fixture).expect("fixture should parse");
        assert_eq!(meshes.len(), 1);
        assert_eq!(meshes[0].positions.len(), 3);
        assert_eq!(meshes[0].indices, vec![0, 1, 2]);
    }

    #[test]
    fn compact_atomic_uses_geometry_zero_and_applies_frame_transform() {
        let meshes = parse_dff(&compact_atomic_fixture()).expect("fixture should parse");
        assert_eq!(meshes.len(), 1);
        assert_eq!(meshes[0].positions[0], [10.0, 0.0, 0.0]);
        assert_eq!(meshes[0].positions[1], [11.0, 0.0, 0.0]);
    }

    #[test]
    fn decodes_renderware_library_version() {
        assert_eq!(decode_library_version(0x1803_FFFF), 0x0003_6003);
    }

    // ---- Rig-preserving parse ------------------------------------------

    fn frame_record(parent: i32, position: [f32; 3]) -> Vec<u8> {
        let mut frame = Vec::new();
        frame.extend_from_slice(&1.0_f32.to_le_bytes());
        frame.extend_from_slice(&0.0_f32.to_le_bytes());
        frame.extend_from_slice(&0.0_f32.to_le_bytes());
        frame.extend_from_slice(&0.0_f32.to_le_bytes());
        frame.extend_from_slice(&1.0_f32.to_le_bytes());
        frame.extend_from_slice(&0.0_f32.to_le_bytes());
        frame.extend_from_slice(&0.0_f32.to_le_bytes());
        frame.extend_from_slice(&0.0_f32.to_le_bytes());
        frame.extend_from_slice(&1.0_f32.to_le_bytes());
        for value in position {
            frame.extend_from_slice(&value.to_le_bytes());
        }
        frame.extend_from_slice(&parent.to_le_bytes());
        frame.extend_from_slice(&0_u32.to_le_bytes());
        frame
    }

    fn skin_matrix(translation: [f32; 3]) -> Vec<u8> {
        let mut matrix = Vec::new();
        matrix.extend_from_slice(&1.0_f32.to_le_bytes());
        matrix.extend_from_slice(&0.0_f32.to_le_bytes());
        matrix.extend_from_slice(&0.0_f32.to_le_bytes());
        matrix.extend_from_slice(&0.0_f32.to_le_bytes());
        matrix.extend_from_slice(&0.0_f32.to_le_bytes());
        matrix.extend_from_slice(&1.0_f32.to_le_bytes());
        matrix.extend_from_slice(&0.0_f32.to_le_bytes());
        matrix.extend_from_slice(&0.0_f32.to_le_bytes());
        matrix.extend_from_slice(&0.0_f32.to_le_bytes());
        matrix.extend_from_slice(&0.0_f32.to_le_bytes());
        matrix.extend_from_slice(&1.0_f32.to_le_bytes());
        matrix.extend_from_slice(&0.0_f32.to_le_bytes());
        for value in translation {
            matrix.extend_from_slice(&value.to_le_bytes());
        }
        matrix.extend_from_slice(&1.0_f32.to_le_bytes());
        matrix
    }

    /// Three frames ("Normal" root, "BoneA", "BoneB" with an HAnim root
    /// header), one textured geometry carrying a modern SkinPLG, and an
    /// atomic binding the geometry to frame 1.
    fn skinned_rig_fixture() -> Vec<u8> {
        let mut frames_struct = Vec::new();
        frames_struct.extend_from_slice(&3_u32.to_le_bytes());
        frames_struct.extend_from_slice(&frame_record(-1, [0.0, 0.0, 0.0]));
        frames_struct.extend_from_slice(&frame_record(0, [5.0, 0.0, 0.0]));
        frames_struct.extend_from_slice(&frame_record(0, [0.0, 3.0, 0.0]));

        let mut normal_name = section(NODE_NAME_PLG, 0, b"Normal\0");
        let mut hanim_body = Vec::new();
        hanim_body.extend_from_slice(&0x0000_0100_i32.to_le_bytes());
        hanim_body.extend_from_slice(&0_i32.to_le_bytes());
        hanim_body.extend_from_slice(&2_i32.to_le_bytes());
        hanim_body.extend_from_slice(&0_u32.to_le_bytes());
        hanim_body.extend_from_slice(&36_u32.to_le_bytes());
        hanim_body.extend_from_slice(&1_i32.to_le_bytes());
        hanim_body.extend_from_slice(&1_i32.to_le_bytes());
        hanim_body.extend_from_slice(&2_i32.to_le_bytes());
        hanim_body.extend_from_slice(&2_i32.to_le_bytes());
        hanim_body.extend_from_slice(&2_i32.to_le_bytes());
        hanim_body.extend_from_slice(&4_i32.to_le_bytes());
        let hanim_chunk = section(HANIM_PLG, 0, &hanim_body);
        // Frame 0's extension groups its node name and the root HAnim;
        // each remaining frame gets one extension with its node name.
        let normal_ext = section(EXTENSION, 0, &{
            let mut chunks = normal_name;
            chunks.extend_from_slice(&hanim_chunk);
            chunks
        });
        let mut bonea_ext = section(NODE_NAME_PLG, 0, b"BoneA\0");
        bonea_ext = section(EXTENSION, 0, &bonea_ext);
        let mut boneb_ext = section(NODE_NAME_PLG, 0, b"BoneB\0");
        boneb_ext = section(EXTENSION, 0, &boneb_ext);

        let mut frame_list_body = section(STRUCT, 0x1803_FFFF, &frames_struct);
        frame_list_body.extend_from_slice(&normal_ext);
        frame_list_body.extend_from_slice(&bonea_ext);
        frame_list_body.extend_from_slice(&boneb_ext);
        let frame_list = section(FRAME_LIST, 0x1803_FFFF, &frame_list_body);

        let mut geometry_struct = Vec::new();
        geometry_struct.extend_from_slice(&0x0000_0034_u32.to_le_bytes());
        geometry_struct.extend_from_slice(&1_u32.to_le_bytes());
        geometry_struct.extend_from_slice(&3_u32.to_le_bytes());
        geometry_struct.extend_from_slice(&1_u32.to_le_bytes());
        for uv in [[0.0_f32, 0.0], [1.0, 0.0], [0.0, 1.0]] {
            geometry_struct.extend_from_slice(&uv[0].to_le_bytes());
            geometry_struct.extend_from_slice(&uv[1].to_le_bytes());
        }
        geometry_struct.extend_from_slice(&1_u16.to_le_bytes());
        geometry_struct.extend_from_slice(&0_u16.to_le_bytes());
        geometry_struct.extend_from_slice(&0_u16.to_le_bytes());
        geometry_struct.extend_from_slice(&2_u16.to_le_bytes());
        geometry_struct.extend_from_slice(&[0_u8; 16]);
        geometry_struct.extend_from_slice(&1_u32.to_le_bytes());
        geometry_struct.extend_from_slice(&1_u32.to_le_bytes());
        for point in [[0.0_f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]] {
            for value in point {
                geometry_struct.extend_from_slice(&value.to_le_bytes());
            }
        }
        for _ in 0..3 {
            for value in [0.0_f32, 0.0, 1.0] {
                geometry_struct.extend_from_slice(&value.to_le_bytes());
            }
        }

        let mut skin = Vec::new();
        skin.push(2_u8);
        skin.push(2_u8);
        skin.push(4_u8);
        skin.push(0_u8); // header pad ("<3Bx")
        skin.extend_from_slice(&[1_u8, 2_u8]);
        // Contiguous index block (4 bytes per vertex), then the weight
        // block (16 bytes per vertex) — DragonFF's two-array layout.
        let vertex_data: [([u8; 4], [f32; 4]); 3] = [
            ([0, 1, 0, 0], [0.5, 0.5, 0.0, 0.0]),
            ([1, 1, 1, 1], [1.0, 0.0, 0.0, 0.0]),
            ([0, 0, 0, 0], [1.0, 0.0, 0.0, 0.0]),
        ];
        for (indices, _) in vertex_data {
            skin.extend_from_slice(&indices);
        }
        for (_, weights) in vertex_data {
            for weight in weights {
                skin.extend_from_slice(&weight.to_le_bytes());
            }
        }
        skin.extend_from_slice(&skin_matrix([-1.0, 0.0, 0.0]));
        skin.extend_from_slice(&skin_matrix([0.0, -2.0, 0.0]));
        let skin_ext = section(EXTENSION, 0, &section(SKIN_PLG, 0, &skin));

        let material = section(MATERIAL, 0, &section(STRUCT, 0, &[0; 28]));
        let mut material_list_body = section(STRUCT, 0, &[1, 0, 0, 0, 0, 0, 0, 0]);
        material_list_body.extend_from_slice(&material);
        let mut geometry_body = section(STRUCT, 0x1803_FFFF, &geometry_struct);
        geometry_body.extend_from_slice(&section(MATERIAL_LIST, 0, &material_list_body));
        geometry_body.extend_from_slice(&skin_ext);
        let geometry = section(GEOMETRY, 0x1803_FFFF, &geometry_body);
        let mut geometry_list_body = section(STRUCT, 0, &[1, 0, 0, 0]);
        geometry_list_body.extend_from_slice(&geometry);
        let geometry_list = section(GEOMETRY_LIST, 0x1803_FFFF, &geometry_list_body);

        let atomic = section(
            ATOMIC,
            0x1803_FFFF,
            &section(STRUCT, 0x1803_FFFF, &[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
        );

        let mut clump_body = section(STRUCT, 0, &[0; 12]);
        clump_body.extend_from_slice(&frame_list);
        clump_body.extend_from_slice(&geometry_list);
        clump_body.extend_from_slice(&atomic);
        section(CLUMP, 0x1803_FFFF, &clump_body)
    }

    #[test]
    fn rig_parse_preserves_frames_names_hanim_and_skin() {
        let rig = parse_dff_rig(&skinned_rig_fixture()).expect("skinned fixture should parse");
        assert_eq!(rig.frames.len(), 3);
        assert_eq!(rig.frames[0].name.as_deref(), Some("Normal"));
        assert_eq!(rig.frames[1].name.as_deref(), Some("BoneA"));
        assert_eq!(rig.frames[2].name.as_deref(), Some("BoneB"));
        assert_eq!(rig.frames[0].parent, -1);
        assert_eq!(rig.frames[1].parent, 0);

        // The root frame carries the HAnim bone list; frame 1 has the
        // offset [5, 0, 0] that must NOT be baked into the local vertices.
        let hanim = rig.frames[0].hanim.as_ref().expect("root HAnim");
        assert_eq!(hanim.bone_count, 2);
        assert_eq!(hanim.bones.len(), 2);
        assert_eq!(hanim.bones[0].id, 1);
        assert_eq!(hanim.bones[0].index, 1);
        assert_eq!(rig.frames[1].position, [5.0, 0.0, 0.0]);

        assert_eq!(rig.meshes.len(), 1);
        assert_eq!(rig.meshes[0].frame, 1);
        assert_eq!(rig.meshes[0].mesh.positions[0], [0.0, 0.0, 0.0]);
        assert_eq!(rig.meshes[0].mesh.indices, vec![0, 1, 2]);

        let skin = rig.meshes[0].skin.as_ref().expect("geometry skin");
        assert!(!skin.legacy);
        assert_eq!(skin.num_bones, 2);
        assert_eq!(skin.used_bones, vec![1, 2]);
        assert_eq!(skin.vertex_weights[0], [0.5, 0.5, 0.0, 0.0]);
        assert_eq!(skin.bone_matrices.len(), 2);
        // The second matrix's stored translation row.
        assert_eq!(skin.bone_matrices[1][3][1], -2.0);
    }

    #[test]
    fn rig_parse_rejects_files_without_frames() {
        let result = parse_dff_rig(&actual_layout_fixture());
        assert!(result.is_err());
    }

    #[test]
    fn gta_dff_rig_parses_when_available() {
        let Some(root) = crate::test_paths::gta3_exports() else {
            return;
        };
        let mut total_checked = 0usize;
        for name in ["player.dff", "bmyst.dff"] {
            let path = root.as_path().join(name);
            if !path.is_file() {
                continue;
            }
            let bytes = std::fs::read(&path).expect("fixture path was checked above");
            let rig = parse_dff_rig(&bytes)
                .unwrap_or_else(|error| panic!("{} should parse as a rig: {error}", path.display()));
            assert!(
                rig.frames.len() >= 20,
                "{} should carry a full skeleton (got {} frames)",
                path.display(),
                rig.frames.len()
            );
            assert!(
                rig.frames.iter().any(|frame| frame.name.is_some()),
                "{} frames should carry node names",
                path.display()
            );
            let hanim_roots = rig
                .frames
                .iter()
                .filter(|frame| frame.hanim.as_ref().is_some_and(|h| !h.bones.is_empty()))
                .count();
            assert_eq!(
                hanim_roots, 1,
                "{} should have exactly one HAnim root listing the bones",
                path.display()
            );
            let skinned = rig
                .meshes
                .iter()
                .filter(|mesh| mesh.skin.is_some())
                .count();
            assert!(
                skinned > 0,
                "{} should carry at least one skinned mesh",
                path.display()
            );
            for mesh in rig.meshes.iter().filter(|mesh| mesh.skin.is_some()) {
                let skin = mesh.skin.as_ref().unwrap();
                assert_eq!(skin.vertex_indices.len(), mesh.mesh.positions.len());
                assert_eq!(skin.vertex_weights.len(), mesh.mesh.positions.len());
                if skin.legacy {
                    assert_eq!(skin.bones.len(), skin.num_bones);
                } else {
                    // Used bones are skeleton frame indices; the matrix
                    // palette covers the whole skeleton.
                    assert!(
                        skin.used_bones
                            .iter()
                            .all(|&bone| (bone as usize) < skin.num_bones),
                        "used bones must index the skeleton"
                    );
                }
                assert_eq!(skin.bone_matrices.len(), skin.num_bones);
                // Referenced vertices carry positive weights summing to ~1;
                // stub geometries (like player.dff's placeholder) may be
                // entirely zero-weight, and unreferenced padding vertices
                // may be zero too.
                let referenced: std::collections::BTreeSet<u32> =
                    mesh.mesh.indices.iter().copied().collect();
                for &vertex in referenced.iter().take(64) {
                    let weights = &skin.vertex_weights[vertex as usize];
                    let sum: f32 = weights.iter().sum();
                    if sum <= 0.0 {
                        continue;
                    }
                    assert!(
                        (sum - 1.0).abs() < 0.05,
                        "[{name}] vertex {vertex} weights should sum to ~1 (got {sum})"
                    );
                    assert!(
                        weights.iter().all(|w| *w >= 0.0),
                        "weights must be non-negative"
                    );
                    total_checked += 1;
                }
            }
        }
        assert!(
            total_checked > 0,
            "at least one skinned mesh should carry positive-sum weights"
        );
    }

    #[test]
    fn parses_real_renderware_dff_samples_when_present() {
        let Some(root) = crate::test_paths::gta3_exports() else {
            return;
        };
        let root = root.as_path();
        let mut paths = [
            root.join("a51_blastdoorl.dff"),
            root.join("a51_crane.dff"),
            root.join("adm_lamp.dff"),
            root.join("player.dff"),
            root.join("infernus.dff"),
            root.join("taxi.dff"),
            root.join("landstal.dff"),
            root.join("hydra.dff"),
        ]
        .into_iter()
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
        if let Ok(entries) = std::fs::read_dir(root) {
            let mut corpus = entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| {
                    path.extension()
                        .is_some_and(|extension| extension.eq_ignore_ascii_case("dff"))
                })
                .collect::<Vec<_>>();
            corpus.sort_unstable();
            for path in corpus.into_iter().take(128) {
                if !paths.contains(&path) {
                    paths.push(path);
                }
            }
        }
        let found_fixture = !paths.is_empty();
        for path in &paths {
            let bytes = std::fs::read(path).expect("fixture path was checked above");
            let meshes = parse_dff(&bytes).unwrap_or_else(|error| {
                panic!(
                    "{} should parse as a RenderWare DFF: {error}",
                    path.display()
                )
            });
            assert!(
                !meshes.is_empty(),
                "{} should contain mesh data",
                path.display()
            );
            assert!(
                meshes
                    .iter()
                    .any(|mesh| !mesh.positions.is_empty() && !mesh.indices.is_empty()),
                "{} should contain renderable triangles",
                path.display()
            );
        }
        if !found_fixture {
            eprintln!("local RenderWare DFF fixtures are not available; skipping corpus check");
        }
    }
}
