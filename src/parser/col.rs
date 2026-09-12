//! GTA Collision (`.col`) file parser.
//!
//! Supported versions:
//! - **COL1** (`COLL`): legacy float32 vertices and 32-bit face indices
//! - **COL2** (`COL2`): offset-based collision records with int16 vertices
//! - **COL3** (`COL3`): COL2 plus an optional shadow mesh
//! - **COL4** (`COL4`): COL3 with an additional header word
//!
//! A `.col` file is a concatenation of entries (one per model). Each entry
//! carries its own header, bounding shapes, and optionally a collision mesh.

use thiserror::Error;

const MAX_COLLISION_ITEMS: usize = 1_000_000;
const COL_FLAG_FACE_GROUPS: u32 = 0x08;
const COL_FLAG_SHADOW_MESH: u32 = 0x10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColVersion {
    V1, // COLL
    V2, // COL2
    V3, // COL3
    V4, // COL4
}

/// Surface metadata carried by a collision primitive or face.
///
/// The viewer currently uses collision geometry as an untextured preview,
/// but retaining these values keeps the parser lossless enough for future
/// material-aware collision rendering.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ColSurface {
    pub material: u8,
    pub flags: u8,
    pub brightness: u8,
    pub light: u8,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ColSphere {
    pub center: [f32; 3],
    pub radius: f32,
    pub surface: ColSurface,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ColBox {
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub surface: ColSurface,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ColFace {
    pub a: u32,
    pub b: u32,
    pub c: u32,
    pub surface: ColSurface,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ColFaceGroup {
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub start_face: u16,
    pub end_face: u16,
}

#[derive(Debug, Error)]
pub enum ColError {
    #[error("file too short")]
    TooShort,
    #[error("unknown COL version magic: {0:?}")]
    UnknownMagic([u8; 4]),
    #[error("unexpected end of entry data")]
    Truncated,
    #[error("invalid COL structure: {0}")]
    Invalid(String),
    #[error("no renderable geometry found")]
    NoGeometry,
}

/// Parsed collision file — a collection of entries.
#[derive(Debug, Clone)]
pub struct ColFile {
    pub entries: Vec<ColEntry>,
}

/// A single collision model entry.
#[derive(Debug, Clone)]
pub struct ColEntry {
    pub version: ColVersion,
    pub model_name: String,
    pub num_vertices: u32,
    pub vertices: Vec<[f32; 3]>,
    pub num_faces: u32,
    pub indices: Vec<u32>,
    pub faces: Vec<ColFace>,
    pub spheres: Vec<ColSphere>,
    pub boxes: Vec<ColBox>,
    pub face_groups: Vec<ColFaceGroup>,
    pub shadow_vertices: Vec<[f32; 3]>,
    pub shadow_faces: Vec<ColFace>,
    pub shadow_indices: Vec<u32>,
    pub num_spheres: u32,
    pub num_boxes: u32,
    pub has_shadow: bool,
}

/// Parse a complete `.col` file, returning every entry that contains
/// triangles, spheres, boxes, or a shadow mesh. The scene decoder can
/// tessellate the primitive-only entries for the embedded viewer.
pub fn parse_col(bytes: &[u8]) -> Result<ColFile, ColError> {
    if bytes.len() < 8 {
        return Err(ColError::TooShort);
    }

    let mut entries = Vec::new();
    let mut position = 0usize;

    while position < bytes.len() {
        if bytes.len() - position < 4 {
            if bytes[position..].iter().all(|&byte| byte == 0) {
                break;
            }
            return Err(ColError::Truncated);
        }
        let magic: [u8; 4] = bytes[position..position + 4]
            .try_into()
            .expect("four-byte COL magic");
        if magic == [0; 4] {
            if bytes[position..].iter().any(|&byte| byte != 0) {
                return Err(ColError::Invalid(
                    "non-zero bytes follow the final COL entry".to_string(),
                ));
            }
            break;
        }

        let version = match &magic {
            b"COLL" => ColVersion::V1,
            b"COL2" => ColVersion::V2,
            b"COL3" => ColVersion::V3,
            b"COL4" => ColVersion::V4,
            _ => return Err(ColError::UnknownMagic(magic)),
        };

        if position + 8 > bytes.len() {
            return Err(ColError::Truncated);
        }
        let body_size = read_u32_at(bytes, position + 4, bytes.len())? as usize;
        if body_size < 24 {
            return Err(ColError::Invalid(
                "entry body is smaller than its fixed header".to_string(),
            ));
        }
        let entry_end = position
            .checked_add(8)
            .and_then(|start| start.checked_add(body_size))
            .ok_or(ColError::Truncated)?;
        if entry_end > bytes.len() {
            return Err(ColError::Truncated);
        }

        if let Some(entry) = parse_entry(bytes, position, entry_end, version)? {
            entries.push(entry);
        }
        position = entry_end;
    }

    if entries.is_empty() {
        return Err(ColError::NoGeometry);
    }

    Ok(ColFile { entries })
}

fn parse_entry(
    bytes: &[u8],
    entry_start: usize,
    entry_end: usize,
    version: ColVersion,
) -> Result<Option<ColEntry>, ColError> {
    // 4-byte magic + 4-byte body size + 22-byte name + u16 model id.
    let mut cursor = entry_start;
    let _magic = read_bytes(bytes, &mut cursor, entry_end, 4)?;
    let _body_size = read_u32(bytes, &mut cursor, entry_end)?;
    let name_bytes = read_bytes(bytes, &mut cursor, entry_end, 22)?;
    let name_end = name_bytes
        .iter()
        .position(|&byte| byte == 0)
        .unwrap_or(name_bytes.len());
    let model_name = String::from_utf8_lossy(&name_bytes[..name_end]).into_owned();
    let _model_id = read_u16(bytes, &mut cursor, entry_end)?;

    // All COL versions store a 40-byte bounds record after the fixed header.
    read_bytes(bytes, &mut cursor, entry_end, 40)?;

    match version {
        ColVersion::V1 => parse_legacy_entry(bytes, cursor, entry_end, model_name),
        ColVersion::V2 | ColVersion::V3 | ColVersion::V4 => {
            parse_offset_entry(bytes, entry_start, cursor, entry_end, version, model_name)
        }
    }
}

fn parse_legacy_entry(
    bytes: &[u8],
    mut cursor: usize,
    entry_end: usize,
    model_name: String,
) -> Result<Option<ColEntry>, ColError> {
    // COL1 stores four counted blocks in this order. The second u32 is an
    // historical unknown-count field with no associated payload in the
    // published layout.
    let num_spheres = read_u32(bytes, &mut cursor, entry_end)?;
    let sphere_count = checked_count(num_spheres, "spheres")?;
    let mut spheres = Vec::with_capacity(sphere_count);
    for _ in 0..sphere_count {
        let radius = read_f32(bytes, &mut cursor, entry_end)?;
        let center = [
            read_f32(bytes, &mut cursor, entry_end)?,
            read_f32(bytes, &mut cursor, entry_end)?,
            read_f32(bytes, &mut cursor, entry_end)?,
        ];
        spheres.push(ColSphere {
            center,
            radius,
            surface: read_surface(bytes, &mut cursor, entry_end)?,
        });
    }
    let _unknown_count = read_u32(bytes, &mut cursor, entry_end)?;

    let num_boxes = read_u32(bytes, &mut cursor, entry_end)?;
    let box_count = checked_count(num_boxes, "boxes")?;
    let mut boxes = Vec::with_capacity(box_count);
    for _ in 0..box_count {
        let min = [
            read_f32(bytes, &mut cursor, entry_end)?,
            read_f32(bytes, &mut cursor, entry_end)?,
            read_f32(bytes, &mut cursor, entry_end)?,
        ];
        let max = [
            read_f32(bytes, &mut cursor, entry_end)?,
            read_f32(bytes, &mut cursor, entry_end)?,
            read_f32(bytes, &mut cursor, entry_end)?,
        ];
        boxes.push(ColBox {
            min,
            max,
            surface: read_surface(bytes, &mut cursor, entry_end)?,
        });
    }

    let num_vertices = read_u32(bytes, &mut cursor, entry_end)?;
    let vertex_count = checked_count(num_vertices, "vertices")?;
    let vertex_end = checked_items_end(cursor, vertex_count, 12, entry_end)?;
    let mut vertices = Vec::with_capacity(vertex_count);
    while cursor < vertex_end {
        vertices.push([
            read_f32(bytes, &mut cursor, entry_end)?,
            read_f32(bytes, &mut cursor, entry_end)?,
            read_f32(bytes, &mut cursor, entry_end)?,
        ]);
    }

    let num_faces = read_u32(bytes, &mut cursor, entry_end)?;
    let face_count = checked_count(num_faces, "faces")?;
    let face_end = checked_items_end(cursor, face_count, 16, entry_end)?;
    let mut faces = Vec::with_capacity(face_count);
    let mut indices = Vec::with_capacity(face_count.saturating_mul(3));
    while cursor < face_end {
        let a = read_u32(bytes, &mut cursor, entry_end)?;
        let b = read_u32(bytes, &mut cursor, entry_end)?;
        let c = read_u32(bytes, &mut cursor, entry_end)?;
        let surface = read_surface(bytes, &mut cursor, entry_end)?;
        faces.push(ColFace {
            a,
            b,
            c,
            surface,
        });
        if valid_triangle([a, b, c], vertices.len()) {
            indices.extend_from_slice(&[a, b, c]);
        }
    }

    Ok(make_entry(
        ColEntryMetadata {
            version: ColVersion::V1,
            model_name,
            num_vertices,
            num_faces,
            num_spheres,
            num_boxes,
            has_shadow: false,
        },
        ColGeometry {
            vertices,
            indices,
            faces,
            spheres,
            boxes,
            ..ColGeometry::default()
        },
    ))
}

fn parse_offset_entry(
    bytes: &[u8],
    entry_start: usize,
    mut cursor: usize,
    entry_end: usize,
    version: ColVersion,
    model_name: String,
) -> Result<Option<ColEntry>, ColError> {
    // COL2+ uses a compact header followed by offsets relative to the start
    // of this entry. Each non-zero offset points four bytes before the
    // corresponding payload. The counts in the compact header describe the
    // payloads; they are not repeated at the offsets.
    let sphere_count = read_u16(bytes, &mut cursor, entry_end)? as u32;
    let box_count = read_u16(bytes, &mut cursor, entry_end)? as u32;
    let face_count = read_u16(bytes, &mut cursor, entry_end)? as u32;
    let _line_count = read_u8(bytes, &mut cursor, entry_end)?;
    let _padding = read_u8(bytes, &mut cursor, entry_end)?;
    let flags = read_u32(bytes, &mut cursor, entry_end)?;
    let sphere_offset = read_u32(bytes, &mut cursor, entry_end)?;
    let box_offset = read_u32(bytes, &mut cursor, entry_end)?;
    let _line_offset = read_u32(bytes, &mut cursor, entry_end)?;
    let vertex_offset = read_u32(bytes, &mut cursor, entry_end)?;
    let face_offset = read_u32(bytes, &mut cursor, entry_end)?;
    let _triangle_offset = read_u32(bytes, &mut cursor, entry_end)?;

    let mut shadow_face_count = 0u32;
    let mut shadow_vertex_offset = 0u32;
    let mut shadow_face_offset = 0u32;
    if matches!(version, ColVersion::V3 | ColVersion::V4) {
        shadow_face_count = read_u32(bytes, &mut cursor, entry_end)?;
        shadow_vertex_offset = read_u32(bytes, &mut cursor, entry_end)?;
        shadow_face_offset = read_u32(bytes, &mut cursor, entry_end)?;
        if version == ColVersion::V4 {
            read_u32(bytes, &mut cursor, entry_end)?;
        }
    }

    validate_optional_block(
        entry_start,
        entry_end,
        sphere_offset,
        sphere_count,
        20,
        "spheres",
    )?;
    validate_optional_block(
        entry_start,
        entry_end,
        box_offset,
        box_count,
        28,
        "boxes",
    )?;

    let spheres = read_offset_spheres(
        bytes,
        entry_start,
        entry_end,
        sphere_offset,
        sphere_count,
    )?;
    let boxes = read_offset_boxes(
        bytes,
        entry_start,
        entry_end,
        box_offset,
        box_count,
    )?;

    let face_groups = if flags & COL_FLAG_FACE_GROUPS != 0 {
        read_offset_face_groups(
            bytes,
            entry_start,
            entry_end,
            face_offset,
            face_count,
        )?
    } else {
        Vec::new()
    };

    let raw_faces = read_offset_faces(
        bytes,
        entry_start,
        entry_end,
        face_offset,
        face_count,
    )?;
    let max_face_index = raw_faces
        .iter()
        .flat_map(|face| [face.a, face.b, face.c])
        .max();
    let required_vertices = max_face_index
        .map(|index| index as usize + 1)
        .unwrap_or(0);
    let (vertices, num_vertices) = read_offset_vertices(
        bytes,
        entry_start,
        entry_end,
        vertex_offset,
        required_vertices,
    )?;

    let mut indices = Vec::with_capacity(raw_faces.len().saturating_mul(3));
    for face in &raw_faces {
        let [a, b, c] = [face.a, face.b, face.c];
        if valid_triangle([a, b, c], vertices.len()) {
            indices.extend_from_slice(&[a, b, c]);
        }
    }

    let (shadow_vertices, shadow_faces, shadow_indices) = if matches!(
        version,
        ColVersion::V3 | ColVersion::V4
    ) && shadow_face_count > 0
    {
        let shadow_faces = read_offset_faces(
            bytes,
            entry_start,
            entry_end,
            shadow_face_offset,
            shadow_face_count,
        )?;
        let max_shadow_index = shadow_faces
            .iter()
            .flat_map(|face| [face.a, face.b, face.c])
            .max();
        let required_shadow_vertices = max_shadow_index
            .map(|index| index as usize + 1)
            .unwrap_or(0);
        let (shadow_vertices, _) = read_offset_vertices(
            bytes,
            entry_start,
            entry_end,
            shadow_vertex_offset,
            required_shadow_vertices,
        )?;
        let mut shadow_indices = Vec::with_capacity(shadow_faces.len().saturating_mul(3));
        for face in &shadow_faces {
            let [a, b, c] = [face.a, face.b, face.c];
            if valid_triangle([a, b, c], shadow_vertices.len()) {
                shadow_indices.extend_from_slice(&[a, b, c]);
            }
        }
        (shadow_vertices, shadow_faces, shadow_indices)
    } else {
        (Vec::new(), Vec::new(), Vec::new())
    };

    let has_shadow = !shadow_indices.is_empty() && flags & COL_FLAG_SHADOW_MESH != 0;

    Ok(make_entry(
        ColEntryMetadata {
            version,
            model_name,
            num_vertices,
            num_faces: face_count,
            num_spheres: sphere_count,
            num_boxes: box_count,
            has_shadow,
        },
        ColGeometry {
            vertices,
            indices,
            faces: raw_faces,
            spheres,
            boxes,
            face_groups,
            shadow_vertices,
            shadow_faces,
            shadow_indices,
        },
    ))
}

fn read_offset_faces(
    bytes: &[u8],
    entry_start: usize,
    entry_end: usize,
    offset: u32,
    expected_count: u32,
) -> Result<Vec<ColFace>, ColError> {
    if expected_count == 0 {
        return Ok(Vec::new());
    }
    if offset == 0 {
        return Err(ColError::Invalid(
            "face count is non-zero but its offset is missing".to_string(),
        ));
    }
    let block_position = relative_position(entry_start, offset, entry_end)?;
    let count = checked_count(expected_count, "faces")?;
    let data_start = block_position.checked_add(4).ok_or(ColError::Truncated)?;
    let data_end = checked_items_end(data_start, count, 8, entry_end)?;
    let mut cursor = data_start;
    let mut faces = Vec::with_capacity(count);
    while cursor < data_end {
        let a = read_u16(bytes, &mut cursor, entry_end)? as u32;
        let b = read_u16(bytes, &mut cursor, entry_end)? as u32;
        let c = read_u16(bytes, &mut cursor, entry_end)? as u32;
        let material = read_u8(bytes, &mut cursor, entry_end)?;
        let light = read_u8(bytes, &mut cursor, entry_end)?;
        faces.push(ColFace {
            a,
            b,
            c,
            surface: ColSurface {
                material,
                light,
                ..ColSurface::default()
            },
        });
    }
    Ok(faces)
}

fn read_surface(
    bytes: &[u8],
    cursor: &mut usize,
    limit: usize,
) -> Result<ColSurface, ColError> {
    let values = read_bytes(bytes, cursor, limit, 4)?;
    Ok(ColSurface {
        material: values[0],
        flags: values[1],
        brightness: values[2],
        light: values[3],
    })
}

fn read_offset_spheres(
    bytes: &[u8],
    entry_start: usize,
    entry_end: usize,
    offset: u32,
    expected_count: u32,
) -> Result<Vec<ColSphere>, ColError> {
    if expected_count == 0 {
        return Ok(Vec::new());
    }
    let block_position = relative_position(entry_start, offset, entry_end)?;
    let count = checked_count(expected_count, "spheres")?;
    let data_start = block_position.checked_add(4).ok_or(ColError::Truncated)?;
    let data_end = checked_items_end(data_start, count, 20, entry_end)?;
    let mut cursor = data_start;
    let mut spheres = Vec::with_capacity(count);
    while cursor < data_end {
        let center = [
            read_f32(bytes, &mut cursor, entry_end)?,
            read_f32(bytes, &mut cursor, entry_end)?,
            read_f32(bytes, &mut cursor, entry_end)?,
        ];
        let radius = read_f32(bytes, &mut cursor, entry_end)?;
        spheres.push(ColSphere {
            center,
            radius,
            surface: read_surface(bytes, &mut cursor, entry_end)?,
        });
    }
    Ok(spheres)
}

fn read_offset_boxes(
    bytes: &[u8],
    entry_start: usize,
    entry_end: usize,
    offset: u32,
    expected_count: u32,
) -> Result<Vec<ColBox>, ColError> {
    if expected_count == 0 {
        return Ok(Vec::new());
    }
    let block_position = relative_position(entry_start, offset, entry_end)?;
    let count = checked_count(expected_count, "boxes")?;
    let data_start = block_position.checked_add(4).ok_or(ColError::Truncated)?;
    let data_end = checked_items_end(data_start, count, 28, entry_end)?;
    let mut cursor = data_start;
    let mut boxes = Vec::with_capacity(count);
    while cursor < data_end {
        let min = [
            read_f32(bytes, &mut cursor, entry_end)?,
            read_f32(bytes, &mut cursor, entry_end)?,
            read_f32(bytes, &mut cursor, entry_end)?,
        ];
        let max = [
            read_f32(bytes, &mut cursor, entry_end)?,
            read_f32(bytes, &mut cursor, entry_end)?,
            read_f32(bytes, &mut cursor, entry_end)?,
        ];
        boxes.push(ColBox {
            min,
            max,
            surface: read_surface(bytes, &mut cursor, entry_end)?,
        });
    }
    Ok(boxes)
}

fn read_offset_face_groups(
    bytes: &[u8],
    entry_start: usize,
    entry_end: usize,
    face_offset: u32,
    face_count: u32,
) -> Result<Vec<ColFaceGroup>, ColError> {
    if face_count == 0 {
        return Ok(Vec::new());
    }
    if face_offset == 0 {
        return Err(ColError::Invalid(
            "face groups require a face block offset".to_string(),
        ));
    }
    // The stored offset is four bytes before the face payload in this
    // parser's entry-relative coordinate system. With face groups enabled,
    // that location is the trailing group-count word.
    let count_position = relative_position(entry_start, face_offset, entry_end)?;
    let group_count = read_u32_at(bytes, count_position, entry_end)?;
    let group_count = checked_count(group_count, "face groups")?;
    let group_bytes = group_count
        .checked_mul(28)
        .ok_or(ColError::Truncated)?;
    let group_start = count_position
        .checked_sub(group_bytes)
        .ok_or(ColError::Invalid(
            "face groups begin before the collision entry".to_string(),
        ))?;
    if group_start < entry_start {
        return Err(ColError::Invalid(
            "face groups begin before the collision entry".to_string(),
        ));
    }
    let group_end = checked_items_end(group_start, group_count, 28, entry_end)?;
    let mut cursor = group_start;
    let mut groups = Vec::with_capacity(group_count);
    while cursor < group_end {
        let min = [
            read_f32(bytes, &mut cursor, entry_end)?,
            read_f32(bytes, &mut cursor, entry_end)?,
            read_f32(bytes, &mut cursor, entry_end)?,
        ];
        let max = [
            read_f32(bytes, &mut cursor, entry_end)?,
            read_f32(bytes, &mut cursor, entry_end)?,
            read_f32(bytes, &mut cursor, entry_end)?,
        ];
        let start_face = read_u16(bytes, &mut cursor, entry_end)?;
        let end_face = read_u16(bytes, &mut cursor, entry_end)?;
        groups.push(ColFaceGroup {
            min,
            max,
            start_face,
            end_face,
        });
    }
    Ok(groups)
}

fn read_offset_vertices(
    bytes: &[u8],
    entry_start: usize,
    entry_end: usize,
    offset: u32,
    required_count: usize,
) -> Result<(Vec<[f32; 3]>, u32), ColError> {
    if required_count == 0 {
        return Ok((Vec::new(), 0));
    }
    if offset == 0 {
        return Err(ColError::Invalid(
            "face indices require a vertex block".to_string(),
        ));
    }
    let block_position = relative_position(entry_start, offset, entry_end)?;
    let data_start = block_position.checked_add(4).ok_or(ColError::Truncated)?;
    let data_end = checked_items_end(data_start, required_count, 6, entry_end)?;
    let mut cursor = data_start;
    let mut vertices = Vec::with_capacity(required_count);
    while cursor < data_end {
        vertices.push([
            read_i16(bytes, &mut cursor, entry_end)? as f32 / 128.0,
            read_i16(bytes, &mut cursor, entry_end)? as f32 / 128.0,
            read_i16(bytes, &mut cursor, entry_end)? as f32 / 128.0,
        ]);
    }
    let num_vertices = u32::try_from(required_count)
        .map_err(|_| ColError::Invalid("vertex count overflows u32".to_string()))?;
    Ok((vertices, num_vertices))
}

fn validate_optional_block(
    entry_start: usize,
    entry_end: usize,
    offset: u32,
    expected_count: u32,
    item_size: usize,
    label: &str,
) -> Result<(), ColError> {
    if offset == 0 {
        if expected_count == 0 {
            return Ok(());
        }
        return Err(ColError::Invalid(format!(
            "{label} count is non-zero but its offset is missing"
        )));
    }
    let block_position = relative_position(entry_start, offset, entry_end)?;
    let count = checked_count(expected_count, label)?;
    let data_start = block_position.checked_add(4).ok_or(ColError::Truncated)?;
    checked_items_end(data_start, count, item_size, entry_end)?;
    Ok(())
}

struct ColEntryMetadata {
    version: ColVersion,
    model_name: String,
    num_vertices: u32,
    num_faces: u32,
    num_spheres: u32,
    num_boxes: u32,
    has_shadow: bool,
}

#[derive(Default)]
struct ColGeometry {
    vertices: Vec<[f32; 3]>,
    indices: Vec<u32>,
    faces: Vec<ColFace>,
    spheres: Vec<ColSphere>,
    boxes: Vec<ColBox>,
    face_groups: Vec<ColFaceGroup>,
    shadow_vertices: Vec<[f32; 3]>,
    shadow_faces: Vec<ColFace>,
    shadow_indices: Vec<u32>,
}

fn make_entry(
    metadata: ColEntryMetadata,
    geometry: ColGeometry,
) -> Option<ColEntry> {
    let has_geometry = !geometry.indices.is_empty()
        || !geometry.spheres.is_empty()
        || !geometry.boxes.is_empty()
        || !geometry.shadow_indices.is_empty();
    if !has_geometry {
        return None;
    }
    Some(ColEntry {
        version: metadata.version,
        model_name: metadata.model_name,
        num_vertices: metadata.num_vertices,
        vertices: geometry.vertices,
        num_faces: metadata.num_faces,
        indices: geometry.indices,
        faces: geometry.faces,
        spheres: geometry.spheres,
        boxes: geometry.boxes,
        face_groups: geometry.face_groups,
        shadow_vertices: geometry.shadow_vertices,
        shadow_faces: geometry.shadow_faces,
        shadow_indices: geometry.shadow_indices,
        num_spheres: metadata.num_spheres,
        num_boxes: metadata.num_boxes,
        has_shadow: metadata.has_shadow,
    })
}

fn valid_triangle(indices: [u32; 3], vertex_count: usize) -> bool {
    indices.iter().all(|&index| (index as usize) < vertex_count)
        && indices[0] != indices[1]
        && indices[0] != indices[2]
        && indices[1] != indices[2]
}

fn checked_count(raw: u32, label: &str) -> Result<usize, ColError> {
    let count = usize::try_from(raw)
        .map_err(|_| ColError::Invalid(format!("{label} count overflows")))?;
    if count > MAX_COLLISION_ITEMS {
        return Err(ColError::Invalid(format!(
            "{label} count {count} exceeds the supported limit {MAX_COLLISION_ITEMS}"
        )));
    }
    Ok(count)
}

fn checked_items_end(
    start: usize,
    count: usize,
    item_size: usize,
    limit: usize,
) -> Result<usize, ColError> {
    let bytes = count.checked_mul(item_size).ok_or(ColError::Truncated)?;
    let end = start.checked_add(bytes).ok_or(ColError::Truncated)?;
    if end > limit {
        return Err(ColError::Truncated);
    }
    Ok(end)
}

fn relative_position(base: usize, offset: u32, limit: usize) -> Result<usize, ColError> {
    let position = base
        .checked_add(offset as usize)
        .ok_or(ColError::Truncated)?;
    if position >= limit {
        return Err(ColError::Truncated);
    }
    Ok(position)
}

fn read_bytes<'a>(
    bytes: &'a [u8],
    cursor: &mut usize,
    limit: usize,
    amount: usize,
) -> Result<&'a [u8], ColError> {
    let end = cursor.checked_add(amount).ok_or(ColError::Truncated)?;
    if end > limit || end > bytes.len() {
        return Err(ColError::Truncated);
    }
    let result = &bytes[*cursor..end];
    *cursor = end;
    Ok(result)
}

fn read_u8(bytes: &[u8], cursor: &mut usize, limit: usize) -> Result<u8, ColError> {
    Ok(read_bytes(bytes, cursor, limit, 1)?[0])
}

fn read_u16(bytes: &[u8], cursor: &mut usize, limit: usize) -> Result<u16, ColError> {
    Ok(u16::from_le_bytes(
        read_bytes(bytes, cursor, limit, 2)?
            .try_into()
            .expect("bounded u16 read"),
    ))
}

fn read_i16(bytes: &[u8], cursor: &mut usize, limit: usize) -> Result<i16, ColError> {
    Ok(i16::from_le_bytes(
        read_bytes(bytes, cursor, limit, 2)?
            .try_into()
            .expect("bounded i16 read"),
    ))
}

fn read_u32(bytes: &[u8], cursor: &mut usize, limit: usize) -> Result<u32, ColError> {
    Ok(u32::from_le_bytes(
        read_bytes(bytes, cursor, limit, 4)?
            .try_into()
            .expect("bounded u32 read"),
    ))
}

fn read_u32_at(bytes: &[u8], position: usize, limit: usize) -> Result<u32, ColError> {
    let end = position.checked_add(4).ok_or(ColError::Truncated)?;
    if end > limit || end > bytes.len() {
        return Err(ColError::Truncated);
    }
    Ok(u32::from_le_bytes(
        bytes[position..end]
            .try_into()
            .expect("bounded u32 read"),
    ))
}

fn read_f32(bytes: &[u8], cursor: &mut usize, limit: usize) -> Result<f32, ColError> {
    Ok(f32::from_le_bytes(
        read_bytes(bytes, cursor, limit, 4)?
            .try_into()
            .expect("bounded f32 read"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::ArchiveInfo;
    use crate::parser::read_entry_data;

    #[test]
    fn reject_empty() {
        assert!(parse_col(&[]).is_err());
    }

    #[test]
    fn reject_unknown_magic() {
        let bytes = b"XXXX";
        assert!(parse_col(bytes).is_err());
    }

    #[test]
    fn parse_minimal_col1_entry() {
        // Build a deliberately incomplete COL1 entry. The fixed header and
        // bounds record are absent, so the parser must reject it safely.
        let mut data = Vec::new();
        data.extend_from_slice(b"COLL");
        let size_pos = data.len();
        data.extend_from_slice(&[0u8; 4]);
        data.extend_from_slice(&[0u8; 24]);
        data.resize(72, 0);

        let total_size = data.len() - 8;
        data[size_pos..size_pos + 4].copy_from_slice(&(total_size as u32).to_le_bytes());

        let result = parse_col(&data);
        assert!(result.is_err(), "expected NoGeometry or malformed entry");
    }

    fn col2_triangle_fixture() -> Vec<u8> {
        let mut data = vec![0u8; 72 + 36];
        data[0..4].copy_from_slice(b"COL2");
        data[8..14].copy_from_slice(b"tri\0\0\0");

        let metadata = 72;
        data[metadata..metadata + 2].copy_from_slice(&0u16.to_le_bytes());
        data[metadata + 2..metadata + 4].copy_from_slice(&0u16.to_le_bytes());
        data[metadata + 4..metadata + 6].copy_from_slice(&1u16.to_le_bytes());
        data[metadata + 6] = 0;
        data[metadata + 7] = 0;
        data[metadata + 8..metadata + 12].copy_from_slice(&2u32.to_le_bytes());
        // COL2 offsets point four bytes before the payload. The vertex
        // payload therefore starts at 104 + 4, and the face payload at
        // 128 + 4.
        data[metadata + 12..metadata + 16].copy_from_slice(&0u32.to_le_bytes());
        data[metadata + 16..metadata + 20].copy_from_slice(&0u32.to_le_bytes());
        data[metadata + 20..metadata + 24].copy_from_slice(&0u32.to_le_bytes());
        data[metadata + 24..metadata + 28].copy_from_slice(&104u32.to_le_bytes());
        data[metadata + 28..metadata + 32].copy_from_slice(&128u32.to_le_bytes());
        data[metadata + 32..metadata + 36].copy_from_slice(&0u32.to_le_bytes());
        for vertex in [[0i16, 0, 0], [128, 0, 0], [0, 128, 0]] {
            for value in vertex {
                data.extend_from_slice(&value.to_le_bytes());
            }
        }
        data.resize(132, 0);
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&1u16.to_le_bytes());
        data.extend_from_slice(&2u16.to_le_bytes());
        data.extend_from_slice(&[0, 0]);
        let body_size = (data.len() - 8) as u32;
        data[4..8].copy_from_slice(&body_size.to_le_bytes());
        data
    }

    fn col1_shape_fixture() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(b"COLL");
        data.extend_from_slice(&[0; 4]);
        let mut body = Vec::new();
        let mut name = [0u8; 22];
        name[..5].copy_from_slice(b"shape");
        body.extend_from_slice(&name);
        body.extend_from_slice(&0i16.to_le_bytes());
        for value in [
            2.0f32, 0.0, 0.0, 0.0, -2.0, -2.0, -2.0, 2.0, 2.0, 2.0,
        ] {
            body.extend_from_slice(&value.to_le_bytes());
        }
        body.extend_from_slice(&1u32.to_le_bytes());
        body.extend_from_slice(&1.0f32.to_le_bytes());
        for value in [0.0f32, 1.0, 2.0] {
            body.extend_from_slice(&value.to_le_bytes());
        }
        body.extend_from_slice(&[3, 4, 5, 6]);
        body.extend_from_slice(&0u32.to_le_bytes());
        body.extend_from_slice(&1u32.to_le_bytes());
        for value in [-1.0f32, -2.0, -3.0, 1.0, 2.0, 3.0] {
            body.extend_from_slice(&value.to_le_bytes());
        }
        body.extend_from_slice(&[7, 8, 9, 10]);
        body.extend_from_slice(&0u32.to_le_bytes());
        body.extend_from_slice(&0u32.to_le_bytes());
        let body_size = u32::try_from(body.len()).expect("shape fixture fits");
        data[4..8].copy_from_slice(&body_size.to_le_bytes());
        data.extend_from_slice(&body);
        data
    }

    #[test]
    fn parses_compressed_col2_triangle_and_scales_vertices() {
        let data = col2_triangle_fixture();
        let parsed = parse_col(&data).expect("valid COL2 fixture");
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.entries[0].vertices[1], [1.0, 0.0, 0.0]);
        assert_eq!(parsed.entries[0].indices, vec![0, 1, 2]);
    }

    #[test]
    fn reads_col2_face_groups_before_the_face_block() {
        let mut data = col2_triangle_fixture();
        let metadata = 72;
        data[metadata + 8..metadata + 12].copy_from_slice(&10u32.to_le_bytes());
        data[metadata + 28..metadata + 32].copy_from_slice(&160u32.to_le_bytes());

        let mut group = Vec::new();
        for value in [
            -1.0f32, -1.0, -1.0, 1.0, 1.0, 1.0,
        ] {
            group.extend_from_slice(&value.to_le_bytes());
        }
        group.extend_from_slice(&0u16.to_le_bytes());
        group.extend_from_slice(&1u16.to_le_bytes());
        group.extend_from_slice(&1u32.to_le_bytes());
        data.splice(132..132, group);
        let body_size = u32::try_from(data.len() - 8).expect("fixture fits");
        data[4..8].copy_from_slice(&body_size.to_le_bytes());

        let parsed = parse_col(&data).expect("valid COL2 face-group fixture");
        assert_eq!(parsed.entries[0].face_groups.len(), 1);
        assert_eq!(parsed.entries[0].face_groups[0].start_face, 0);
        assert_eq!(parsed.entries[0].face_groups[0].end_face, 1);
        assert_eq!(parsed.entries[0].indices, vec![0, 1, 2]);
    }

    #[test]
    fn preserves_col1_shapes_for_primitive_preview() {
        let parsed = parse_col(&col1_shape_fixture()).expect("valid COL1 shape fixture");
        let entry = &parsed.entries[0];
        assert_eq!(entry.model_name, "shape");
        assert_eq!(entry.spheres.len(), 1);
        assert_eq!(entry.boxes.len(), 1);
        assert!(entry.indices.is_empty());
        assert_eq!(entry.spheres[0].surface.material, 3);
        assert_eq!(entry.boxes[0].surface.light, 10);
    }

    #[test]
    fn rejects_nonzero_bytes_after_a_collision_file() {
        let mut data = col2_triangle_fixture();
        data.extend_from_slice(&[1, 2, 3]);
        assert!(matches!(
            parse_col(&data),
            Err(ColError::Truncated) | Err(ColError::Invalid(_))
        ));
    }

    #[test]
    fn parses_retail_san_andreas_col2_and_col3_entries_when_present() {
        let Some(root) = crate::test_paths::corpus_root() else {
            return;
        };

        let gta3_names = ["seabed5.col", "seabed8.col", "seabed9.col"];
        let interior_names = ["stadint_4.col", "stadint_5.col"];
        let cases = [
            (
                root.join("GTA San Andreas/models/gta3.img"),
                gta3_names.as_slice(),
            ),
            (
                root.join("GTA San Andreas/models/gta_int.img"),
                interior_names.as_slice(),
            ),
        ];
        let mut checked = 0;

        for (archive_path, names) in cases {
            if !archive_path.is_file() {
                continue;
            }
            let archive = ArchiveInfo::open(&archive_path).unwrap_or_else(|error| {
                panic!("{} should open: {error}", archive_path.display())
            });
            for name in names {
                let Some(entry) = archive
                    .entries
                    .iter()
                    .find(|entry| entry.file_name.eq_ignore_ascii_case(name))
                else {
                    continue;
                };
                let bytes = read_entry_data(&archive, entry)
                    .unwrap_or_else(|error| panic!("{name} should be readable: {error}"));
                let parsed = parse_col(&bytes)
                    .unwrap_or_else(|error| panic!("{name} should parse as COL: {error}"));
                assert!(!parsed.entries.is_empty(), "{name} should contain geometry");
                assert!(
                    parsed
                        .entries
                        .iter()
                        .any(|entry| !entry.vertices.is_empty() && !entry.indices.is_empty()),
                    "{name} should contain renderable collision triangles"
                );
                assert!(bytes.starts_with(b"COL2") || bytes.starts_with(b"COL3"));
                checked += 1;
            }
        }

        assert!(
            checked > 0,
            "IMGEDITOR_CORPUS_ROOT is set but no representative San Andreas COL entries were found"
        );
    }

    #[test]
    fn parses_retail_vice_city_col_files_when_present() {
        let Some(root) = crate::test_paths::corpus_root() else {
            return;
        };
        let maps = root.join("Grand Theft Auto Vice City/data/maps");
        if !maps.is_dir() {
            return;
        }

        fn collect_col_files(dir: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
            for item in std::fs::read_dir(dir).expect("read Vice City map directory") {
                let path = item.expect("read Vice City map entry").path();
                if path.is_dir() {
                    collect_col_files(&path, files);
                } else if path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("col"))
                {
                    files.push(path);
                }
            }
        }

        let mut files = Vec::new();
        collect_col_files(&maps, &mut files);
        let mut checked = 0;
        for path in files {
            let bytes = std::fs::read(&path)
                .unwrap_or_else(|error| panic!("{} should be readable: {error}", path.display()));
            match parse_col(&bytes) {
                Ok(parsed) => assert!(
                    !parsed.entries.is_empty(),
                    "{} parsed without entries",
                    path.display()
                ),
                Err(ColError::NoGeometry) => {
                    // Empty collision model collections are valid in the
                    // retail data; they simply have nothing to preview.
                }
                Err(error) => panic!("{} should parse: {error}", path.display()),
            }
            checked += 1;
        }
        assert!(checked > 0, "expected at least one retail Vice City COL fixture");
    }
}
