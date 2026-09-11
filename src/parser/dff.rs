//! RenderWare DFF (Drawable File Format) parser.
//!
//! GTA III, Vice City, and San Andreas PC models are RenderWare clumps. The
//! parser intentionally focuses on the shared, non-native geometry stream:
//! positions, normals, UVs, triangle records, and the diffuse names carried
//! by the material list. Console-native geometry remains an explicit
//! unsupported case instead of being mistaken for ordinary vertex data.

use std::collections::BTreeMap;

const CLUMP: u32 = 0x10;
const GEOMETRY: u32 = 0x0F;
const GEOMETRY_LIST: u32 = 0x1A;
const STRUCT: u32 = 0x01;
const STRING: u32 = 0x02;
const MATERIAL: u32 = 0x07;
const MATERIAL_LIST: u32 = 0x08;
const FRAME_LIST: u32 = 0x0E;
const TEXTURE: u32 = 0x06;
const ATOMIC: u32 = 0x14;

const FLAG_TRI_STRIP: u32 = 0x0000_0001;
const FLAG_POSITIONS: u32 = 0x0000_0002;
const FLAG_TEXTURED: u32 = 0x0000_0004;
const FLAG_PRELIT: u32 = 0x0000_0008;
const FLAG_NORMALS: u32 = 0x0000_0010;
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
}

#[derive(Clone, Copy, Debug)]
struct FrameData {
    /// RenderWare stores the local basis as right, up, and at vectors.
    basis: [[f32; 3]; 3],
    position: [f32; 3],
    parent: i32,
}

#[derive(Clone, Copy, Debug)]
struct AtomicData {
    frame: usize,
    geometry: Option<usize>,
}

#[derive(Debug, Default)]
struct ClumpData {
    frames: Vec<FrameData>,
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
            for _ in 0..frame_count {
                clump.frames.push(parse_frame(&mut frame_cursor)?);
            }
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
            _ => {}
        }
        position = child.end;
    }
    if parsed_struct {
        if clump.atomics.len() >= MAX_ATOMICS {
            return Err(format!("atomics exceed viewer limit {MAX_ATOMICS}"));
        }
        clump.atomics.push(AtomicData { frame, geometry });
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
            _ => {}
        }
        position = child.end;
    }

    let mut geometry = geometry.ok_or_else(|| "GEOMETRY has no STRUCT section".to_string())?;
    geometry.material_textures = material_textures;
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
            if flags & FLAG_POSITIONS == 0 {
                cursor.skip(vertex_bytes, "unused vertex data")?;
            } else if morph == 0 {
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
            if morph == 0 && flags & FLAG_NORMALS != 0 {
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
