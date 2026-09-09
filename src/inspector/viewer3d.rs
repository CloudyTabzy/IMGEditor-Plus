use std::fs;
use std::io::Write;
use std::panic;
use std::path::{Path, PathBuf};
use std::thread;

use tokio::sync::mpsc;

use crate::inspector::nif::{
    self, BlockPayload, NiTriShapeDataPayload, NiTriStripsDataPayload, NifFile,
};
use crate::inspector::texture::{IdeMap, resolve_textures_for_nif};
use crate::parser::col::ColFile;
use crate::parser::dff::DffMesh;

#[derive(Debug, Clone)]
pub enum ViewerEvent {
    Opened { name: String },
    Failed { reason: String },
    Closed,
}

/// Export the NIF geometry to a temporary .obj/.mtl pair (with texture when
/// available) and open it with the system's default viewer.
///
/// `texture_files` maps a bare filename (e.g. `"P_ipoor_1950fridge_d.tga"`)
/// to its raw bytes, extracted from the IMG archive ahead of time.
pub fn spawn_render_window(
    nif_data: Vec<u8>,
    name: String,
    game_root: Option<PathBuf>,
) -> mpsc::UnboundedReceiver<ViewerEvent> {
    let (tx, rx) = mpsc::unbounded_channel();
    thread::Builder::new()
        .name(format!("nif-viewer-{name}"))
        .spawn(move || {
            let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
                export_viewer(nif_data, name, game_root, tx.clone());
            }));
            if let Err(err) = result {
                let msg = match err.downcast_ref::<&str>() {
                    Some(s) => s.to_string(),
                    None => match err.downcast_ref::<String>() {
                        Some(s) => s.clone(),
                        None => "unknown panic".to_string(),
                    },
                };
                eprintln!("[IMGEditor] NIF viewer panicked: {msg}");
                let _ = tx.send(ViewerEvent::Failed {
                    reason: format!("viewer panicked: {msg}"),
                });
            }
        })
        .expect("failed to spawn NIF viewer thread");
    rx
}

fn export_viewer(
    nif_data: Vec<u8>,
    name: String,
    game_root: Option<PathBuf>,
    tx: mpsc::UnboundedSender<ViewerEvent>,
) {
    let data_len = nif_data.len();
    if data_len < 12 || !nif_data.starts_with(b"Gamebryo File Format") {
        let msg = format!("Not a valid NIF file ({data_len} bytes)");
        eprintln!("[IMGEditor] viewer: {msg}");
        let _ = tx.send(ViewerEvent::Failed { reason: msg });
        return;
    }

    let mut nif = match NifFile::parse(&nif_data) {
        Ok(n) => n,
        Err(e) => {
            let msg = format!("NIF parse failed: {e}");
            eprintln!("[IMGEditor] viewer: {msg}");
            let _ = tx.send(ViewerEvent::Failed { reason: msg });
            return;
        }
    };
    nif.resolve_string_indices();

    eprintln!("[IMGEditor] viewer: collecting geometry");
    let mesh_data = match collect_mesh(&nif) {
        Some(m) => m,
        None => {
            let msg = "No renderable geometry found in NIF file.".to_string();
            eprintln!("[IMGEditor] viewer: {msg}");
            let _ = tx.send(ViewerEvent::Failed { reason: msg });
            return;
        }
    };

    let diffuse_texture = find_diffuse_texture(&nif);
    if let Some(ref t) = diffuse_texture {
        eprintln!("[IMGEditor] viewer: diffuse texture found: {t}");
    } else {
        eprintln!("[IMGEditor] viewer: no diffuse texture found in NIF");
    }

    let stem = Path::new(&name)
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or(&name);
    let temp_dir = std::env::temp_dir().join("IMGEditor").join("preview");
    let _ = fs::create_dir_all(&temp_dir);

    // Resolve texture via IDE → NFT pipeline.
    let nif_basename = stem;
    eprintln!("[IMGEditor] viewer: nif basename = {nif_basename}, game_root = {game_root:?}");
    let ide_map = game_root.as_ref().map(|root| IdeMap::build(root));
    let nft_catalog = ide_map
        .as_ref()
        .and_then(|map| resolve_textures_for_nif(nif_basename, map));
    if let Some(ref cat) = nft_catalog {
        eprintln!(
            "[IMGEditor] viewer: NFT catalog has {} entries",
            cat.entries.len()
        );
        for (k, v) in &cat.entries {
            eprintln!(
                "  {k}: {} bytes, source={}",
                v.pixel_data.as_ref().map_or(0, |d| d.len()),
                v.source_path
            );
        }
    } else {
        eprintln!("[IMGEditor] viewer: no NFT catalog resolved");
    }

    // Try exporting textured OBJ when texture data is available.
    let used_obj = if let Some(ref tex_name) = diffuse_texture {
        let tex_data = nft_catalog
            .as_ref()
            .and_then(|cat| cat.get_pixels(tex_name));
        if let Some(data) = tex_data {
            match write_obj_with_texture(&temp_dir, stem, &mesh_data, tex_name, data) {
                Ok(out_path) => {
                    eprintln!("[IMGEditor] viewer: wrote textured OBJ to {out_path:?}");
                    true
                }
                Err(e) => {
                    eprintln!("[IMGEditor] viewer: OBJ export failed ({e}), falling back to PLY");
                    false
                }
            }
        } else {
            eprintln!(
                "[IMGEditor] viewer: texture {tex_name} not found in NFT, falling back to PLY"
            );
            false
        }
    } else {
        eprintln!("[IMGEditor] viewer: no diffuse texture found in NIF, falling back to PLY");
        false
    };

    if !used_obj {
        let ply_path = temp_dir.join(format!("{stem}.ply"));
        match write_ply(&ply_path, &mesh_data) {
            Ok(()) => {
                eprintln!("[IMGEditor] viewer: wrote untextured PLY to {ply_path:?}");
            }
            Err(e) => {
                let msg = format!("Failed to write PLY: {e}");
                eprintln!("[IMGEditor] viewer: {msg}");
                let _ = tx.send(ViewerEvent::Failed { reason: msg });
                return;
            }
        }
    }

    let _ = tx.send(ViewerEvent::Opened { name: name.clone() });

    // Open the file with the system default handler (detached).
    let out_path = if used_obj {
        temp_dir.join(format!("{stem}.obj"))
    } else {
        temp_dir.join(format!("{stem}.ply"))
    };
    open_file_detached(&out_path);
}

// ---- OBJ + MTL export ------------------------------------------------

fn write_obj_with_texture(
    dir: &Path,
    stem: &str,
    mesh: &MeshData,
    _tex_name: &str,
    tex_bytes: &[u8],
) -> std::io::Result<PathBuf> {
    // Detect format: DDS files start with b"DDS ", TGA otherwise.
    let ext = if tex_bytes.starts_with(b"DDS ") {
        "dds"
    } else {
        "tga"
    };
    let tex_filename = format!("{stem}.{ext}");
    let tex_dst = dir.join(&tex_filename);
    fs::write(&tex_dst, tex_bytes)?;

    // Write MTL.
    let mtl_path = dir.join(format!("{stem}.mtl"));
    {
        let mut f = fs::File::create(&mtl_path)?;
        writeln!(f, "newmtl material_0")?;
        writeln!(f, "Ka 0.6 0.6 0.6")?;
        writeln!(f, "Kd 1.0 1.0 1.0")?;
        writeln!(f, "Ks 0.0 0.0 0.0")?;
        writeln!(f, "Ns 10.0")?;
        if let Some(fname) = tex_dst.file_name().and_then(|n| n.to_str()) {
            writeln!(f, "map_Kd {fname}")?;
        }
    }

    // Write OBJ.
    let obj_path = dir.join(format!("{stem}.obj"));
    {
        let mut f = fs::File::create(&obj_path)?;
        writeln!(f, "mtllib {stem}.mtl")?;
        writeln!(f, "o {stem}")?;

        let has_uv = !mesh.uvs.is_empty();
        let has_normals = !mesh.normals.is_empty();

        // Position records (v).
        for p in &mesh.positions {
            writeln!(f, "v {} {} {}", p[0], p[1], p[2])?;
        }

        // UV records (vt) — must be 1-indexed in face lines.
        if has_uv {
            for uv in &mesh.uvs {
                writeln!(f, "vt {} {}", uv[0], uv[1])?;
            }
        }

        // Normal records (vn) — must be 1-indexed in face lines.
        if has_normals {
            for n in &mesh.normals {
                writeln!(f, "vn {} {} {}", n[0], n[1], n[2])?;
            }
        }

        writeln!(f, "usemtl material_0")?;
        writeln!(f, "s off")?;

        // OBJ uses 1-based indices into v / vt / vn separately. The
        // vertex i holds position v[i+1], and optionally vt[i+1] and
        // vn[i+1] when those arrays are populated.
        for tri in mesh.indices.chunks(3) {
            let i0 = tri[0] + 1;
            let i1 = tri[1] + 1;
            let i2 = tri[2] + 1;
            if has_uv && has_normals {
                writeln!(f, "f {i0}/{i0}/{i0} {i1}/{i1}/{i1} {i2}/{i2}/{i2}")?;
            } else if has_uv {
                writeln!(f, "f {i0}/{i0} {i1}/{i1} {i2}/{i2}")?;
            } else if has_normals {
                writeln!(f, "f {i0}//{i0} {i1}//{i1} {i2}//{i2}")?;
            } else {
                writeln!(f, "f {i0} {i1} {i2}")?;
            }
        }
    }

    Ok(obj_path)
}

// ---- PLY export (fallback) -------------------------------------------

fn write_ply(path: &Path, mesh: &MeshData) -> std::io::Result<()> {
    let mut f = fs::File::create(path)?;
    writeln!(f, "ply")?;
    writeln!(f, "format ascii 1.0")?;
    writeln!(f, "element vertex {}", mesh.positions.len())?;
    writeln!(f, "property float x")?;
    writeln!(f, "property float y")?;
    writeln!(f, "property float z")?;
    if !mesh.normals.is_empty() {
        writeln!(f, "property float nx")?;
        writeln!(f, "property float ny")?;
        writeln!(f, "property float nz")?;
    }
    if !mesh.uvs.is_empty() {
        writeln!(f, "property float u")?;
        writeln!(f, "property float v")?;
    }
    writeln!(f, "element face {}", mesh.indices.len() / 3)?;
    writeln!(f, "property list uchar int vertex_indices")?;
    writeln!(f, "end_header")?;
    for i in 0..mesh.positions.len() {
        let p = &mesh.positions[i];
        write!(f, "{} {} {}", p[0], p[1], p[2])?;
        if i < mesh.normals.len() {
            let n = &mesh.normals[i];
            write!(f, " {} {} {}", n[0], n[1], n[2])?;
        }
        if i < mesh.uvs.len() {
            let uv = &mesh.uvs[i];
            write!(f, " {} {}", uv[0], uv[1])?;
        }
        writeln!(f)?;
    }
    for tri in mesh.indices.chunks(3) {
        writeln!(f, "3 {} {} {}", tri[0], tri[1], tri[2])?;
    }
    Ok(())
}

// ---- DFF render window ------------------------------------------------

/// Spawn a render window for a DFF (RenderWare Clump) mesh.
/// Parses the DFF, extracts geometry, writes a PLY file, and opens it
/// with the system default 3D viewer.
pub fn spawn_dff_render_window(
    dff_data: Vec<u8>,
    name: String,
) -> mpsc::UnboundedReceiver<ViewerEvent> {
    let (tx, rx) = mpsc::unbounded_channel();
    thread::Builder::new()
        .name(format!("dff-viewer-{name}"))
        .spawn(move || {
            let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
                export_dff_viewer(dff_data, name, tx.clone());
            }));
            if let Err(err) = result {
                let msg = match err.downcast_ref::<&str>() {
                    Some(s) => s.to_string(),
                    None => match err.downcast_ref::<String>() {
                        Some(s) => s.clone(),
                        None => "unknown panic".to_string(),
                    },
                };
                eprintln!("[IMGEditor] DFF viewer panicked: {msg}");
                let _ = tx.send(ViewerEvent::Failed {
                    reason: format!("viewer panicked: {msg}"),
                });
            }
        })
        .expect("failed to spawn DFF viewer thread");
    rx
}

fn export_dff_viewer(dff_data: Vec<u8>, name: String, tx: mpsc::UnboundedSender<ViewerEvent>) {
    let meshes = match crate::parser::dff::parse_dff(&dff_data) {
        Ok(m) => m,
        Err(e) => {
            let msg = format!("DFF parse failed: {e}");
            eprintln!("[IMGEditor] viewer: {msg}");
            let _ = tx.send(ViewerEvent::Failed { reason: msg });
            return;
        }
    };

    eprintln!(
        "[IMGEditor] viewer: DFF has {} mesh(es), {} total vertices",
        meshes.len(),
        meshes.iter().map(|m| m.positions.len()).sum::<usize>()
    );

    let temp_dir = std::env::temp_dir().join("IMGEditor").join("preview");
    let _ = fs::create_dir_all(&temp_dir);

    let stem = Path::new(&name)
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or(&name);
    let ply_path = temp_dir.join(format!("{stem}.ply"));

    // Flatten all meshes into one PLY with vertex-offset indexing.
    let total_verts: usize = meshes.iter().map(|m| m.positions.len()).sum();
    let total_indices: usize = meshes.iter().map(|m| m.indices.len()).sum();

    match write_ply_from_meshes(&ply_path, &meshes, total_verts, total_indices) {
        Ok(()) => {
            eprintln!(
                "[IMGEditor] viewer: wrote DFF PLY to {ply_path:?} ({} verts, {} faces)",
                total_verts,
                total_indices / 3
            );
            let _ = tx.send(ViewerEvent::Opened { name });
            open_file_detached(&ply_path);
        }
        Err(e) => {
            let msg = format!("Failed to write PLY: {e}");
            eprintln!("[IMGEditor] viewer: {msg}");
            let _ = tx.send(ViewerEvent::Failed { reason: msg });
        }
    }
}

fn write_ply_from_meshes(
    path: &Path,
    meshes: &[DffMesh],
    total_verts: usize,
    total_indices: usize,
) -> std::io::Result<()> {
    let mut f = fs::File::create(path)?;
    writeln!(f, "ply")?;
    writeln!(f, "format ascii 1.0")?;
    writeln!(f, "element vertex {total_verts}")?;
    writeln!(f, "property float x")?;
    writeln!(f, "property float y")?;
    writeln!(f, "property float z")?;
    writeln!(f, "element face {}", total_indices / 3)?;
    writeln!(f, "property list uchar int vertex_indices")?;
    writeln!(f, "end_header")?;

    for mesh in meshes {
        for p in &mesh.positions {
            writeln!(f, "{} {} {}", p[0], p[1], p[2])?;
        }
    }

    let mut base: u32 = 0;
    for mesh in meshes {
        for chunk in mesh.indices.chunks(3) {
            if chunk.len() == 3 {
                writeln!(
                    f,
                    "3 {} {} {}",
                    base + chunk[0],
                    base + chunk[1],
                    base + chunk[2]
                )?;
            }
        }
        base += mesh.positions.len() as u32;
    }

    Ok(())
}

// ---- COL render window ------------------------------------------------

pub fn spawn_col_render_window(
    col_data: Vec<u8>,
    name: String,
) -> mpsc::UnboundedReceiver<ViewerEvent> {
    let (tx, rx) = mpsc::unbounded_channel();
    thread::Builder::new()
        .name(format!("col-viewer-{name}"))
        .spawn(move || {
            let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
                export_col_viewer(col_data, name, tx.clone());
            }));
            if let Err(err) = result {
                let msg = match err.downcast_ref::<&str>() {
                    Some(s) => s.to_string(),
                    None => match err.downcast_ref::<String>() {
                        Some(s) => s.clone(),
                        None => "unknown panic".to_string(),
                    },
                };
                eprintln!("[IMGEditor] COL viewer panicked: {msg}");
                let _ = tx.send(ViewerEvent::Failed {
                    reason: format!("viewer panicked: {msg}"),
                });
            }
        })
        .expect("failed to spawn COL viewer thread");
    rx
}

fn export_col_viewer(col_data: Vec<u8>, name: String, tx: mpsc::UnboundedSender<ViewerEvent>) {
    let col = match crate::parser::col::parse_col(&col_data) {
        Ok(c) => c,
        Err(e) => {
            let msg = format!("COL parse failed: {e}");
            eprintln!("[IMGEditor] viewer: {msg}");
            let _ = tx.send(ViewerEvent::Failed { reason: msg });
            return;
        }
    };

    let temp_dir = std::env::temp_dir().join("IMGEditor").join("preview");
    let _ = fs::create_dir_all(&temp_dir);

    let stem = name.rsplit('.').next().unwrap_or(&name);
    let ply_path = temp_dir.join(format!("{stem}.ply"));

    match write_ply_from_col(&ply_path, &col) {
        Ok(()) => {
            eprintln!(
                "[IMGEditor] viewer: wrote COL PLY to {ply_path:?} ({} entries)",
                col.entries.len()
            );
            let _ = tx.send(ViewerEvent::Opened { name });
            open_file_detached(&ply_path);
        }
        Err(e) => {
            let msg = format!("Failed to write COL PLY: {e}");
            eprintln!("[IMGEditor] viewer: {msg}");
            let _ = tx.send(ViewerEvent::Failed { reason: msg });
        }
    }
}

fn write_ply_from_col(path: &Path, col: &ColFile) -> std::io::Result<()> {
    let total_verts: usize = col.entries.iter().map(|e| e.vertices.len()).sum();
    let total_faces: usize = col.entries.iter().map(|e| e.indices.len() / 3).sum();

    if total_verts == 0 || total_faces == 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "COL file has no geometry",
        ));
    }

    let mut f = fs::File::create(path)?;
    writeln!(f, "ply")?;
    writeln!(f, "format ascii 1.0")?;
    writeln!(f, "element vertex {total_verts}")?;
    writeln!(f, "property float x")?;
    writeln!(f, "property float y")?;
    writeln!(f, "property float z")?;
    writeln!(f, "element face {total_faces}")?;
    writeln!(f, "property list uchar int vertex_indices")?;
    writeln!(f, "end_header")?;

    for entry in &col.entries {
        for p in &entry.vertices {
            writeln!(f, "{} {} {}", p[0], p[1], p[2])?;
        }
    }

    let mut base: u32 = 0;
    for entry in &col.entries {
        for chunk in entry.indices.chunks(3) {
            if chunk.len() == 3 {
                writeln!(
                    f,
                    "3 {} {} {}",
                    base + chunk[0],
                    base + chunk[1],
                    base + chunk[2]
                )?;
            }
        }
        base += entry.vertices.len() as u32;
    }

    Ok(())
}

// ---- System open -----------------------------------------------------

#[cfg(target_os = "windows")]
fn open_file_detached(path: &Path) {
    let _ = std::process::Command::new("cmd")
        .args(["/c", "start", "", &path.to_string_lossy()])
        .spawn();
}

#[cfg(not(target_os = "windows"))]
fn open_file_detached(path: &Path) {
    let _ = std::process::Command::new("open").arg(path).spawn();
}

// ---- Texture resolution ----------------------------------------------

/// Walk NIF blocks to find the first diffuse texture file name. Base slots
/// are preferred; a conservative detail/orphan fallback covers Bully files
/// that store the diffuse map outside the canonical base slot.
pub(crate) fn find_diffuse_texture(nif: &NifFile) -> Option<String> {
    for (property_idx, payload) in nif.payloads.iter().enumerate() {
        if matches!(payload, Some(BlockPayload::NiTexturingProperty(_)))
            && let Some(name) = diffuse_texture_for_properties(nif, &[property_idx as i32])
        {
            return Some(name);
        }
    }
    // Second pass: orphan NiSourceTexture blocks (no property references
    // them but they carry a file name — Bully often stores textures this way).
    for payload in nif.payloads.iter().flatten() {
        if let BlockPayload::NiSourceTexture(tex) = payload
            && let Some(ref name) = tex.file_name
            && !name.is_empty()
            && looks_like_diffuse_texture(name)
        {
            return Some(name.clone());
        }
    }
    None
}

// ---- Mesh collection -------------------------------------------------

/// Geometry extracted from one NIF geometry node. The embedded viewer keeps
/// these meshes separate so each one can resolve its own diffuse texture.
pub(crate) struct MeshData {
    pub(crate) name: String,
    pub(crate) texture_name: Option<String>,
    pub(crate) positions: Vec<[f32; 3]>,
    pub(crate) normals: Vec<[f32; 3]>,
    pub(crate) uvs: Vec<[f32; 2]>,
    pub(crate) indices: Vec<u32>,
}

#[derive(Clone, Copy, Debug)]
struct Transform3d {
    /// Row-major matrix that multiplies column vectors.
    rotation: [[f32; 3]; 3],
    translation: [f32; 3],
    scale: f32,
}

impl Transform3d {
    fn identity() -> Self {
        Self {
            rotation: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            translation: [0.0; 3],
            scale: 1.0,
        }
    }

    fn from_nif_transform(transform: &nif::NiTransform) -> Self {
        Self {
            // The NIF reader keeps each on-disk 3-f32 group in order. This is
            // already the basis expected by Bully's reference importer;
            // transposing it here applies node rotations in the opposite
            // direction. Transform3d stores the coefficients row-major for
            // its column-vector operations.
            rotation: transform.rotation.m,
            translation: [
                transform.translation.x,
                transform.translation.y,
                transform.translation.z,
            ],
            scale: transform.scale,
        }
    }

    fn compose(parent: Self, local: Self) -> Self {
        let rotation = multiply_mat3(parent.rotation, local.rotation);
        let local_translation =
            multiply_vec3(parent.rotation, scale_vec3(local.translation, parent.scale));
        Self {
            rotation,
            translation: add_vec3(parent.translation, local_translation),
            scale: parent.scale * local.scale,
        }
    }

    fn point(self, point: nif::Vector3) -> [f32; 3] {
        add_vec3(
            multiply_vec3(
                self.rotation,
                [
                    point.x * self.scale,
                    point.y * self.scale,
                    point.z * self.scale,
                ],
            ),
            self.translation,
        )
    }

    fn normal(self, normal: nif::Vector3) -> [f32; 3] {
        let matrix = glam::Mat3::from_cols_array(&[
            self.rotation[0][0],
            self.rotation[1][0],
            self.rotation[2][0],
            self.rotation[0][1],
            self.rotation[1][1],
            self.rotation[2][1],
            self.rotation[0][2],
            self.rotation[1][2],
            self.rotation[2][2],
        ]);
        let input = glam::Vec3::new(normal.x, normal.y, normal.z);
        let transformed = if matrix.determinant().abs() > 1e-6 {
            matrix.inverse().transpose() * input
        } else {
            matrix * input
        };
        let sign = if self.scale < 0.0 { -1.0 } else { 1.0 };
        [
            transformed.x * sign,
            transformed.y * sign,
            transformed.z * sign,
        ]
    }
}

fn add_vec3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scale_vec3(v: [f32; 3], scale: f32) -> [f32; 3] {
    [v[0] * scale, v[1] * scale, v[2] * scale]
}

fn multiply_vec3(matrix: [[f32; 3]; 3], vector: [f32; 3]) -> [f32; 3] {
    [
        matrix[0][0] * vector[0] + matrix[0][1] * vector[1] + matrix[0][2] * vector[2],
        matrix[1][0] * vector[0] + matrix[1][1] * vector[1] + matrix[1][2] * vector[2],
        matrix[2][0] * vector[0] + matrix[2][1] * vector[1] + matrix[2][2] * vector[2],
    ]
}

fn multiply_mat3(a: [[f32; 3]; 3], b: [[f32; 3]; 3]) -> [[f32; 3]; 3] {
    let mut out = [[0.0; 3]; 3];
    for row in 0..3 {
        for col in 0..3 {
            out[row][col] = (0..3).map(|k| a[row][k] * b[k][col]).sum();
        }
    }
    out
}

fn normalize3(vector: [f32; 3]) -> [f32; 3] {
    let length = (vector[0] * vector[0] + vector[1] * vector[1] + vector[2] * vector[2]).sqrt();
    if length > 1e-6 {
        [vector[0] / length, vector[1] / length, vector[2] / length]
    } else {
        [0.0, 1.0, 0.0]
    }
}

fn geometric_normals(positions: &[[f32; 3]], indices: &[u32]) -> Vec<[f32; 3]> {
    let mut normals = vec![[0.0; 3]; positions.len()];
    for triangle in indices.chunks_exact(3) {
        let a = positions[triangle[0] as usize];
        let b = positions[triangle[1] as usize];
        let c = positions[triangle[2] as usize];
        let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let face = [
            ab[1] * ac[2] - ab[2] * ac[1],
            ab[2] * ac[0] - ab[0] * ac[2],
            ab[0] * ac[1] - ab[1] * ac[0],
        ];
        for &index in triangle {
            let normal = &mut normals[index as usize];
            normal[0] += face[0];
            normal[1] += face[1];
            normal[2] += face[2];
        }
    }
    normals.into_iter().map(normalize3).collect()
}

fn triangle_indices(
    data: &NiTriShapeDataPayload,
    strips: Option<&NiTriStripsDataPayload>,
) -> Vec<u32> {
    let vertex_count = data.vertices.len() as u32;
    let mut indices = Vec::new();
    if !data.triangles.is_empty() {
        for triangle in &data.triangles {
            let candidate = [triangle.v0 as u32, triangle.v1 as u32, triangle.v2 as u32];
            if candidate.iter().all(|&index| index < vertex_count)
                && candidate[0] != candidate[1]
                && candidate[0] != candidate[2]
                && candidate[1] != candidate[2]
            {
                indices.extend_from_slice(&candidate);
            }
        }
    } else if let Some(strips) = strips {
        let mut offset = 0usize;
        for &strip_length in &strips.strip_lengths {
            let length = strip_length as usize;
            let Some(end) = offset.checked_add(length) else {
                break;
            };
            if end > strips.points.len() {
                break;
            }
            for j in 0..length.saturating_sub(2) {
                let mut triangle = [
                    strips.points[offset + j] as u32,
                    strips.points[offset + j + 1] as u32,
                    strips.points[offset + j + 2] as u32,
                ];
                if j % 2 != 0 {
                    triangle.swap(0, 1);
                }
                if triangle.iter().all(|&index| index < vertex_count)
                    && triangle[0] != triangle[1]
                    && triangle[0] != triangle[2]
                    && triangle[1] != triangle[2]
                {
                    indices.extend_from_slice(&triangle);
                }
            }
            offset = end;
        }
    }
    indices
}

fn source_texture_name(nif: &NifFile, desc: &nif::TexDesc) -> Option<String> {
    let source_index = usize::try_from(desc.source_ref).ok()?;
    let Some(Some(BlockPayload::NiSourceTexture(texture))) = nif.payloads.get(source_index) else {
        return None;
    };
    texture
        .file_name
        .as_deref()
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
}

fn texture_basename(name: &str) -> &str {
    name.rsplit(['/', '\\']).next().unwrap_or(name)
}

fn looks_like_diffuse_texture(name: &str) -> bool {
    let stem = texture_basename(name)
        .rsplit_once('.')
        .map_or(texture_basename(name), |(stem, _)| stem)
        .to_ascii_lowercase();
    let suffix = stem.rsplit('_').next().unwrap_or(stem.as_str());
    !matches!(
        suffix,
        "n" | "nm" | "normal" | "s" | "spec" | "specular" | "h" | "height"
    )
}

fn diffuse_texture_for_properties(nif: &NifFile, properties: &[i32]) -> Option<String> {
    let mut detail_fallback = None;
    for &property_ref in properties.iter().rev() {
        let Some(property_idx) = usize::try_from(property_ref).ok() else {
            continue;
        };
        let Some(Some(BlockPayload::NiTexturingProperty(texturing))) =
            nif.payloads.get(property_idx)
        else {
            continue;
        };
        if let Some(base) = texturing
            .base
            .as_ref()
            .and_then(|desc| source_texture_name(nif, desc))
        {
            return Some(base);
        }
        if detail_fallback.is_none() {
            detail_fallback = texturing
                .detail
                .as_ref()
                .and_then(|desc| source_texture_name(nif, desc))
                .filter(|name| looks_like_diffuse_texture(name));
        }
    }
    detail_fallback
}

fn geometry_mesh(
    nif: &NifFile,
    block_idx: usize,
    shape: &nif::NiTriShapeData,
    world: Transform3d,
    properties: &[i32],
) -> Option<MeshData> {
    if shape.flags & 1 != 0 || shape.data_ref < 0 {
        return None;
    }
    let data_idx = usize::try_from(shape.data_ref).ok()?;
    let (data, strips) = match nif.payloads.get(data_idx)?.as_ref()? {
        BlockPayload::NiTriShapeData(data) => (data, None),
        BlockPayload::NiTriStripsData(data) => (&data.base, Some(data)),
        _ => return None,
    };
    if data.vertices.is_empty() {
        return None;
    }

    let positions: Vec<_> = data
        .vertices
        .iter()
        .map(|point| world.point(*point))
        .collect();
    let mut indices = triangle_indices(data, strips);
    if world.scale < 0.0 {
        for triangle in indices.chunks_exact_mut(3) {
            triangle.swap(1, 2);
        }
    }
    let calculated_normals = geometric_normals(&positions, &indices);
    let normals = if data.normals.len() == positions.len() {
        data.normals
            .iter()
            .enumerate()
            .map(|(index, normal)| {
                let transformed = world.normal(*normal);
                let length = transformed[0] * transformed[0]
                    + transformed[1] * transformed[1]
                    + transformed[2] * transformed[2];
                if length > 1e-10 {
                    normalize3(transformed)
                } else {
                    calculated_normals[index]
                }
            })
            .collect()
    } else {
        calculated_normals
    };
    let uvs = if data.num_uv_sets > 0 && data.uvs.len() >= data.vertices.len() {
        data.uvs[..data.vertices.len()]
            .iter()
            .map(|uv| [uv.u, uv.v])
            .collect()
    } else {
        Vec::new()
    };

    Some(MeshData {
        name: shape
            .name
            .clone()
            .unwrap_or_else(|| format!("mesh_{block_idx}")),
        texture_name: diffuse_texture_for_properties(nif, properties)
            .or_else(|| find_diffuse_texture(nif)),
        positions,
        normals,
        uvs,
        indices,
    })
}

fn visit_scene_node(
    nif: &NifFile,
    block_idx: usize,
    parent: Transform3d,
    inherited_properties: &[i32],
    path: &mut Vec<usize>,
    meshes: &mut Vec<MeshData>,
) {
    if path.contains(&block_idx) {
        return;
    }
    let Some(Some(payload)) = nif.payloads.get(block_idx) else {
        return;
    };
    path.push(block_idx);
    match payload {
        BlockPayload::NiNode(node) => {
            let world = Transform3d::compose(
                parent,
                Transform3d::from_nif_transform(&nif::NiTransform {
                    rotation: node.rotation,
                    translation: node.translation,
                    scale: node.scale,
                }),
            );
            let mut properties = inherited_properties.to_vec();
            properties.extend_from_slice(&node.properties);
            for &child in &node.children {
                if let Ok(child_idx) = usize::try_from(child) {
                    visit_scene_node(nif, child_idx, world, &properties, path, meshes);
                }
            }
        }
        BlockPayload::NiTriShape(shape) => {
            let world = Transform3d::compose(
                parent,
                Transform3d::from_nif_transform(&nif::NiTransform {
                    rotation: shape.rotation,
                    translation: shape.translation,
                    scale: shape.scale,
                }),
            );
            let mut properties = inherited_properties.to_vec();
            properties.extend_from_slice(&shape.properties);
            if let Some(mesh) = geometry_mesh(nif, block_idx, shape, world, &properties) {
                meshes.push(mesh);
            }
        }
        BlockPayload::NiTriStrips(strips) => {
            let shape = &strips.base;
            let world = Transform3d::compose(
                parent,
                Transform3d::from_nif_transform(&nif::NiTransform {
                    rotation: shape.rotation,
                    translation: shape.translation,
                    scale: shape.scale,
                }),
            );
            let mut properties = inherited_properties.to_vec();
            properties.extend_from_slice(&shape.properties);
            if let Some(mesh) = geometry_mesh(nif, block_idx, shape, world, &properties) {
                meshes.push(mesh);
            }
        }
        _ => {}
    }
    path.pop();
}

pub(crate) fn collect_meshes(nif: &NifFile) -> Vec<MeshData> {
    let mut meshes = Vec::new();
    let mut path = Vec::new();
    for &root in &nif.footer.roots {
        if let Ok(root_idx) = usize::try_from(root) {
            visit_scene_node(
                nif,
                root_idx,
                Transform3d::identity(),
                &[],
                &mut path,
                &mut meshes,
            );
        }
    }
    if meshes.is_empty() {
        for block_idx in 0..nif.blocks.len() {
            visit_scene_node(
                nif,
                block_idx,
                Transform3d::identity(),
                &[],
                &mut path,
                &mut meshes,
            );
        }
    }
    meshes
}

pub(crate) fn collect_mesh(nif: &NifFile) -> Option<MeshData> {
    let meshes = collect_meshes(nif);
    let first = meshes.first()?;
    let all_normals = meshes
        .iter()
        .all(|mesh| mesh.normals.len() == mesh.positions.len());
    let all_uvs = meshes
        .iter()
        .all(|mesh| mesh.uvs.len() == mesh.positions.len());
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();
    for mesh in &meshes {
        let base = positions.len() as u32;
        positions.extend_from_slice(&mesh.positions);
        if all_normals {
            normals.extend_from_slice(&mesh.normals);
        }
        if all_uvs {
            uvs.extend_from_slice(&mesh.uvs);
        }
        indices.extend(mesh.indices.iter().map(|index| base + *index));
    }
    Some(MeshData {
        name: String::from("mesh"),
        texture_name: first.texture_name.clone(),
        positions,
        normals,
        uvs,
        indices,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inspector::nif::{
        BlockMeta, Endian, Footer, Matrix33, NiSourceTextureData, NiTexturingPropertyData, NifFile,
        TexDesc, Triangle, Vector3,
    };

    fn identity_transform(translation: [f32; 3], scale: f32) -> Transform3d {
        Transform3d {
            rotation: Matrix33::identity().m,
            translation,
            scale,
        }
    }

    fn fake_nif(payloads: Vec<Option<BlockPayload>>, roots: Vec<i32>) -> NifFile {
        let blocks = payloads
            .iter()
            .map(|_| BlockMeta {
                type_index: 0,
                type_name: String::new(),
                size: 0,
                offset: 0,
            })
            .collect();
        NifFile {
            header_line: String::new(),
            version: nif::BULLY_NIF_VERSION,
            endian: Endian::Little,
            user_version: 0,
            strings: Vec::new(),
            block_types: Vec::new(),
            blocks,
            payloads,
            footer: Footer { roots },
        }
    }

    #[test]
    fn parent_transform_composes_scale_then_translation() {
        let parent = identity_transform([10.0, 0.0, 0.0], 2.0);
        let child = identity_transform([1.0, 0.0, 0.0], 3.0);
        let world = Transform3d::compose(parent, child);
        assert_eq!(
            world.point(Vector3 {
                x: 1.0,
                ..Vector3::default()
            }),
            [18.0, 0.0, 0.0]
        );
    }

    #[test]
    fn nif_matrix_order_matches_bully_scene_transforms() {
        // This is the on-disk basis used by Bully's +90° mascot root
        // correction. Transposing it would turn the correction upside down.
        let transform = nif::NiTransform {
            rotation: Matrix33 {
                m: [[1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]],
            },
            scale: 1.0,
            ..Default::default()
        };
        let world = Transform3d::from_nif_transform(&transform);
        assert_eq!(
            world.point(Vector3 {
                y: 1.0,
                ..Vector3::default()
            }),
            [0.0, 0.0, 1.0]
        );
    }

    #[test]
    fn mascot_root_stays_upright_after_zup_conversion() {
        use crate::inspector::scene3d::camera::BaseOrientation;

        let transform = nif::NiTransform {
            rotation: Matrix33 {
                m: [[1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]],
            },
            scale: 1.0,
            ..Default::default()
        };
        let source = Transform3d::from_nif_transform(&transform).point(Vector3 {
            y: 1.0,
            ..Vector3::default()
        });
        let camera = BaseOrientation::Zup.to_yup_matrix()
            * glam::Vec4::new(source[0], source[1], source[2], 1.0);

        assert!((camera.y - 1.0).abs() < 1e-6);
        assert!(camera.x.abs() < 1e-6 && camera.z.abs() < 1e-6);
    }

    #[test]
    fn mascot_fixtures_have_upright_viewer_bounds_when_present() {
        for name in [
            "Player_Mascot.nif",
            "Player_Mascot_nh.nif",
            "Player_Mascot_W.nif",
        ] {
            let path =
                std::path::Path::new("C:/Games/Bully - Scholarship Edition/Stream/NIF").join(name);
            let Ok(bytes) = std::fs::read(path) else {
                continue;
            };
            let scene = crate::inspector::scene3d::decode::parse_and_build_scene(
                &bytes,
                crate::inspector::scene3d::camera::BaseOrientation::Zup,
                |_| None,
            )
            .expect("mascot fixture should decode");

            assert!(
                scene.aabb.min[1] > -0.25,
                "{name} feet should not be below the viewer origin"
            );
            assert!(
                scene.aabb.max[1] > 1.0,
                "{name} head should be above the viewer origin"
            );
        }
    }

    #[test]
    fn reflected_mesh_reverses_winding_and_calculates_normals() {
        let data = NiTriShapeDataPayload {
            num_vertices: 3,
            vertices: vec![
                Vector3::default(),
                Vector3 {
                    x: 1.0,
                    ..Vector3::default()
                },
                Vector3 {
                    y: 1.0,
                    ..Vector3::default()
                },
            ],
            triangles: vec![Triangle {
                v0: 0,
                v1: 1,
                v2: 2,
            }],
            ..Default::default()
        };
        let nif = fake_nif(vec![Some(BlockPayload::NiTriShapeData(data))], Vec::new());
        let shape = nif::NiTriShapeData {
            data_ref: 0,
            scale: -1.0,
            ..Default::default()
        };
        let mesh = geometry_mesh(
            &nif,
            1,
            &shape,
            identity_transform([0.0, 0.0, 0.0], -1.0),
            &[],
        )
        .unwrap();
        assert_eq!(mesh.indices, vec![0, 2, 1]);
        assert_eq!(mesh.normals, vec![[0.0, 0.0, -1.0]; 3]);
    }

    #[test]
    fn diffuse_selection_prefers_base_and_rejects_normal_fallbacks() {
        let source = NiSourceTextureData {
            file_name: Some(String::from("models/chair_d.tga")),
            ..Default::default()
        };
        let normal_source = NiSourceTextureData {
            file_name: Some(String::from("models/chair_n.tga")),
            ..Default::default()
        };
        let texturing = NiTexturingPropertyData {
            detail: Some(TexDesc {
                source_ref: 0,
                ..Default::default()
            }),
            ..Default::default()
        };
        let nif = fake_nif(
            vec![
                Some(BlockPayload::NiSourceTexture(source)),
                Some(BlockPayload::NiSourceTexture(normal_source)),
                Some(BlockPayload::NiTexturingProperty(texturing)),
            ],
            Vec::new(),
        );
        assert_eq!(
            diffuse_texture_for_properties(&nif, &[2]),
            Some(String::from("models/chair_d.tga"))
        );
        let normal_property = NiTexturingPropertyData {
            detail: Some(TexDesc {
                source_ref: 1,
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut nif = nif;
        nif.payloads[2] = Some(BlockPayload::NiTexturingProperty(normal_property));
        assert_eq!(diffuse_texture_for_properties(&nif, &[2]), None);
    }
}
