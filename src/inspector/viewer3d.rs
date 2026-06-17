use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::panic;
use std::thread;

use tokio::sync::mpsc;

use crate::inspector::nif::{
    self, BlockPayload, NifFile, NiTriShapeDataPayload, NiTriStripsDataPayload,
};
use crate::inspector::texture::{IdeMap, resolve_textures_for_nif};

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

    let stem = name.rsplit('.').next().unwrap_or(&name);
    let temp_dir = std::env::temp_dir().join("IMGEditor").join("preview");
    let _ = fs::create_dir_all(&temp_dir);

    // Resolve texture via IDE → NFT pipeline.
    let nif_basename = Path::new(&name).file_stem().and_then(|s| s.to_str()).unwrap_or(stem);
    eprintln!("[IMGEditor] viewer: nif basename = {nif_basename}, game_root = {game_root:?}");
    let ide_map = game_root.as_ref().map(|root| IdeMap::build(root));
    let nft_catalog = ide_map.as_ref().and_then(|map| resolve_textures_for_nif(nif_basename, map));
    if let Some(ref cat) = nft_catalog {
        eprintln!("[IMGEditor] viewer: NFT catalog has {} entries", cat.entries.len());
        for (k, v) in &cat.entries {
            eprintln!("  {k}: {} bytes, source={}", v.pixel_data.as_ref().map_or(0, |d| d.len()), v.source_path);
        }
    } else {
        eprintln!("[IMGEditor] viewer: no NFT catalog resolved");
    }

    // Try exporting textured OBJ when texture data is available.
    let used_obj = if let Some(ref tex_name) = diffuse_texture {
        let tex_data = nft_catalog.as_ref().and_then(|cat| cat.get_pixels(tex_name));
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
            eprintln!("[IMGEditor] viewer: texture {tex_name} not found in NFT, falling back to PLY");
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
    let ext = if tex_bytes.starts_with(b"DDS ") { "dds" } else { "tga" };
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

        // We must re-index: OBJ vertex indices are 1-based per-group and
        // the v/vt/vn arrays must be parallel (same length).
        // Build interleaved vertex data.
        let has_uv = !mesh.uvs.is_empty();
        let has_normals = !mesh.normals.is_empty();

        for i in 0..mesh.positions.len() {
            let p = &mesh.positions[i];
            write!(f, "v {} {} {}", p[0], p[1], p[2])?;
            if has_uv && i < mesh.uvs.len() {
                let uv = &mesh.uvs[i];
                write!(f, " {} {}", uv[0], uv[1])?;
            }
            if has_normals && i < mesh.normals.len() {
                let n = &mesh.normals[i];
                write!(f, " {} {} {}", n[0], n[1], n[2])?;
            }
            writeln!(f)?;
        }

        // UV-only vertex data for OBJ vt records.
        if has_uv {
            for uv in &mesh.uvs {
                writeln!(f, "vt {} {}", uv[0], uv[1])?;
            }
        }

        // Normal-only vertex data for OBJ vn records.
        if has_normals {
            for n in &mesh.normals {
                writeln!(f, "vn {} {} {}", n[0], n[1], n[2])?;
            }
        }

        writeln!(f, "usemtl material_0")?;
        writeln!(f, "s off")?;

        // OBJ uses 1-based indices.
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

/// Walk NIF blocks to find the first NiTexturingProperty's base texture file name.
/// Falls back to the first orphan NiSourceTexture block when no property has
/// a populated base slot (common in Bully NIFs that store textures outside
/// the property block).
fn find_diffuse_texture(nif: &NifFile) -> Option<String> {
    // First pass: look through NiTexturingProperty blocks for a populated base.
    for payload in nif.payloads.iter().flatten() {
        if let BlockPayload::NiTexturingProperty(tp) = payload {
            if let Some(ref base) = tp.base {
                if base.source_ref >= 0 {
                    let tex_idx = base.source_ref as usize;
                    if let Some(Some(BlockPayload::NiSourceTexture(tex))) =
                        nif.payloads.get(tex_idx)
                    {
                        if let Some(ref name) = tex.file_name {
                            if !name.is_empty() {
                                return Some(name.clone());
                            }
                        }
                    }
                }
            }
        }
    }
    // Second pass: orphan NiSourceTexture blocks (no property references
    // them but they carry a file name — Bully often stores textures this way).
    for payload in nif.payloads.iter().flatten() {
        if let BlockPayload::NiSourceTexture(tex) = payload {
            if let Some(ref name) = tex.file_name {
                if !name.is_empty() {
                    return Some(name.clone());
                }
            }
        }
    }
    None
}

fn locate_texture_data(
    file_name: &str,
    search_root: &Option<PathBuf>,
    texture_files: &HashMap<String, Vec<u8>>,
) -> Option<Vec<u8>> {
    let normalised = file_name.replace('\\', "/");
    let name = Path::new(&normalised)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(file_name);

    // 1. In-memory map (textures extracted from the IMG archive).
    if let Some(data) = texture_files.get(name) {
        eprintln!("[IMGEditor] viewer: texture {name} found in archive data ({} bytes)", data.len());
        return Some(data.clone());
    }

    // 2. Filesystem search — also try .nft extension (Bully stores textures
    //    as NIF files with embedded pixel data).
    let name_no_ext = Path::new(name).file_stem().and_then(|s| s.to_str()).unwrap_or(name);
    let mut candidates: Vec<PathBuf> = Vec::new();
    candidates.push(Path::new(file_name).to_path_buf());
    // Also try with .nft extension
    candidates.push(Path::new(&format!("{name_no_ext}.nft")).to_path_buf());
    if let Some(root) = search_root {
        candidates.push(root.join(name));
        candidates.push(root.join(&format!("{name_no_ext}.nft")));
        candidates.push(root.join("textures").join(name));
        candidates.push(root.join("textures").join(&format!("{name_no_ext}.nft")));
        if let Some(parent) = root.parent() {
            candidates.push(parent.join("textures").join(name));
            candidates.push(parent.join("textures").join(&format!("{name_no_ext}.nft")));
            if let Some(gp) = parent.parent() {
                candidates.push(gp.join("textures").join(name));
                candidates.push(gp.join("textures").join(&format!("{name_no_ext}.nft")));
            }
        }
    }
    // Also search Stream\Test and TXD directories (Bully stores .nft
    // texture containers alongside NIFs in Test\, or under TXD\.
    if let Some(root) = search_root {
        // search_root = Stream\
        // .nft files are in Test\, TXD\
        for sub in &["Test", "TXD"] {
            let subdir = root.join(sub);
            if subdir.exists() {
                candidates.push(subdir.join(name));
                candidates.push(subdir.join(&format!("{name_no_ext}.nft")));
            }
        }
        // Also check TXD subdirectories
        let txd = root.parent().map(|p| p.join("TXD"));
        if let Some(ref txd) = txd {
            if txd.exists() {
                candidates.push(txd.join(name));
                candidates.push(txd.join(&format!("{name_no_ext}.nft")));
            }
        }
    }

    for path in &candidates {
        eprintln!("[IMGEditor] viewer: trying texture path {path:?}");
        if !path.exists() {
            continue;
        }
        // If it's an .nft file, parse as NIF and extract embedded pixel data.
        if path.extension().and_then(|e| e.to_str()) == Some("nft") {
            if let Some(data) = extract_texture_from_nft(path) {
                eprintln!("[IMGEditor] viewer: extracted texture from NFT {} ({} bytes)", path.display(), data.len());
                return Some(data);
            }
            continue;
        }
        // Otherwise try loading as a regular image file.
        if let Ok(data) = fs::read(path) {
            eprintln!("[IMGEditor] viewer: found texture at {path:?} ({} bytes)", data.len());
            return Some(data);
        }
    }
    None
}

/// Parse a .nft NIF file and extract the first NiSourceTexture block's
/// embedded pixel data, writing it out as a raw TGA.
fn extract_texture_from_nft(path: &Path) -> Option<Vec<u8>> {
    let nft_bytes = fs::read(path).ok()?;
    let mut nif = NifFile::parse(&nft_bytes).ok()?;
    nif.resolve_string_indices();

    // Find the first NiSourceTexture block.
    for (idx, payload) in nif.payloads.iter().enumerate() {
        let Some(BlockPayload::NiSourceTexture(tex)) = payload else {
            continue;
        };
        if tex.use_external != 0 {
            continue;
        }
        let meta = nif.blocks.get(idx)?;
        // The raw block bytes start at meta.offset and are meta.size long.
        let block_start = meta.offset as usize;
        let block_size = meta.size as usize;
        if block_start + block_size > nft_bytes.len() {
            continue;
        }
        let raw = &nft_bytes[block_start..block_start + block_size];

        // Skip the parsed header fields within the block to reach pixel data.
        // NiSourceTexture on-disk field layout (20.3.0.9, use_external=0):
        //   name (NiFixedString, 4 bytes)
        //   num_extra_data (u32, 4)
        //   extra_data (i32[num_extra_data], 0 for textures = 0)
        //   controller (i32, 4)
        //   use_external (u8, 1 = 0)
        //   file_name (NiFixedString, 4)
        //   pixel_layout (u32, 4)
        //   use_mipmaps (u32, 4)
        //   alpha_format (u32, 4)
        //   is_static (u8, 1)
        //   direct_render (u8, 1)
        //   persist_render_data (u8, 1)
        //   --- embedded pixel data follows ---
        let skip = 4 + 4 + 0 + 4 + 1 + 4 + 4 + 4 + 4 + 1 + 1 + 1;
        // After the header fields, the remaining bytes should be the pixel data.
        // The first 4 bytes might be pixel data size (u32), followed by raw RGBA.
        if skip >= raw.len() {
            continue;
        }
        let pixel_bytes = &raw[skip..];

        // Create a minimal TGA header for raw RGBA data.
        // TGA 2.0 format: 18-byte header + pixel data

        // Actually, let's check if there's a width/height before the pixel data.
        // In some NIF versions, the pixel data starts with width(u32), height(u32).
        if pixel_bytes.len() >= 8 {
            let pw = u32::from_le_bytes(pixel_bytes[0..4].try_into().ok()?);
            let ph = u32::from_le_bytes(pixel_bytes[4..8].try_into().ok()?);
            // Sanity check on dimensions
            if pw > 0 && pw <= 16384 && ph > 0 && ph <= 16384 {
                let expected = pw as usize * ph as usize * 4;
                let data_start = 8; // width + height
                let actual = pixel_bytes.len().saturating_sub(data_start);
                if actual >= expected {
                    let mut tga = Vec::with_capacity(18 + expected);
                    // TGA header for uncompressed RGBA
                    tga.push(0);                           // ID length
                    tga.push(0);                           // colormap type
                    tga.push(2);                           // image type (uncompressed true-color)
                    tga.extend_from_slice(&[0, 0, 0, 0, 0]); // colormap spec (5 bytes)
                    tga.extend_from_slice(&[0, 0]);        // X origin
                    tga.extend_from_slice(&[0, 0]);        // Y origin
                    tga.extend_from_slice(&(pw as u16).to_le_bytes()); // width
                    tga.extend_from_slice(&(ph as u16).to_le_bytes()); // height
                    tga.push(32u8);                        // bits per pixel (32)
                    tga.push(0x20);                        // image descriptor (bit 5 = top-left origin)
                    // Pixel data (BGRA in TGA, we have RGBA)
                    for i in (0..expected).step_by(4) {
                        let r = pixel_bytes[data_start + i];
                        let g = pixel_bytes[data_start + i + 1];
                        let b = pixel_bytes[data_start + i + 2];
                        let a = pixel_bytes[data_start + i + 3];
                        tga.push(b);
                        tga.push(g);
                        tga.push(r);
                        tga.push(a);
                    }
                    return Some(tga);
                }
            }
        }

        // Fallback: write raw pixel bytes as a .data file (F3D might not
        // handle this, but better than nothing).
        eprintln!("[IMGEditor] viewer: NFT pixel data format unknown, saving raw ({} bytes)", pixel_bytes.len());
        return Some(pixel_bytes.to_vec());
    }
    None
}

// ---- Mesh collection -------------------------------------------------

struct MeshData {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
}

fn collect_mesh(nif: &NifFile) -> Option<MeshData> {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut base_vertex: u32 = 0;

    for (block_idx, _block) in nif.blocks.iter().enumerate() {
        let Some(ref payload) = nif.payloads[block_idx] else {
            continue;
        };

        let (data_ref, shape_xform) = match payload {
            BlockPayload::NiTriShape(data) => {
                let col0 = data.rotation.m[0];
                let col1 = data.rotation.m[1];
                let col2 = data.rotation.m[2];
                let rotation = [
                    [col0[0], col1[0], col2[0]],
                    [col0[1], col1[1], col2[1]],
                    [col0[2], col1[2], col2[2]],
                ];
                (
                    data.data_ref,
                    ShapeTransform {
                        rotation,
                        translation: data.translation,
                        scale: data.scale,
                    },
                )
            }
            BlockPayload::NiTriStrips(data) => {
                let col0 = data.base.rotation.m[0];
                let col1 = data.base.rotation.m[1];
                let col2 = data.base.rotation.m[2];
                let rotation = [
                    [col0[0], col1[0], col2[0]],
                    [col0[1], col1[1], col2[1]],
                    [col0[2], col1[2], col2[2]],
                ];
                (
                    data.base.data_ref,
                    ShapeTransform {
                        rotation,
                        translation: data.base.translation,
                        scale: data.base.scale,
                    },
                )
            }
            _ => continue,
        };

        if data_ref < 0 {
            continue;
        }
        let data_idx = data_ref as usize;

        let Some(Some(data_payload)) = nif.payloads.get(data_idx) else {
            continue;
        };

        match data_payload {
            BlockPayload::NiTriShapeData(data) => {
                append_mesh(
                    data,
                    None,
                    &shape_xform,
                    &mut positions,
                    &mut indices,
                    &mut normals,
                    &mut uvs,
                    &mut base_vertex,
                );
            }
            BlockPayload::NiTriStripsData(data) => {
                append_mesh(
                    &data.base,
                    Some(data),
                    &shape_xform,
                    &mut positions,
                    &mut indices,
                    &mut normals,
                    &mut uvs,
                    &mut base_vertex,
                );
            }
            _ => {}
        }
    }

    if positions.is_empty() {
        None
    } else {
        Some(MeshData {
            positions,
            normals,
            uvs,
            indices,
        })
    }
}

struct ShapeTransform {
    rotation: [[f32; 3]; 3],
    translation: nif::Vector3,
    scale: f32,
}

fn append_mesh(
    data: &NiTriShapeDataPayload,
    strips: Option<&NiTriStripsDataPayload>,
    xform: &ShapeTransform,
    positions: &mut Vec<[f32; 3]>,
    indices: &mut Vec<u32>,
    normals: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    base_vertex: &mut u32,
) {
    if data.vertices.is_empty() {
        return;
    }

    let rot = &xform.rotation;

    for v in &data.vertices {
        let sx = v.x * xform.scale;
        let sy = v.y * xform.scale;
        let sz = v.z * xform.scale;
        positions.push([
            rot[0][0] * sx + rot[1][0] * sy + rot[2][0] * sz + xform.translation.x,
            rot[0][1] * sx + rot[1][1] * sy + rot[2][1] * sz + xform.translation.y,
            rot[0][2] * sx + rot[1][2] * sy + rot[2][2] * sz + xform.translation.z,
        ]);
    }

    if !data.normals.is_empty() {
        for n in &data.normals {
            let nx = rot[0][0] * n.x + rot[1][0] * n.y + rot[2][0] * n.z;
            let ny = rot[0][1] * n.x + rot[1][1] * n.y + rot[2][1] * n.z;
            let nz = rot[0][2] * n.x + rot[1][2] * n.y + rot[2][2] * n.z;
            normals.push([nx, ny, nz]);
        }
    }

    if !data.uvs.is_empty() {
        let uv_count = data.num_vertices as usize;
        for i in 0..uv_count {
            let uv = data.uvs[i];
            uvs.push([uv.u, uv.v]);
        }
    }

    if !data.triangles.is_empty() {
        for tri in &data.triangles {
            indices.push(*base_vertex + tri.v0 as u32);
            indices.push(*base_vertex + tri.v1 as u32);
            indices.push(*base_vertex + tri.v2 as u32);
        }
    } else if let Some(strips) = strips {
        let mut offset = 0usize;
        for &len in &strips.strip_lengths {
            let len = len as usize;
            if len < 3 {
                offset += len;
                continue;
            }
            for j in 0..len - 2 {
                let i0 = strips.points[offset + j] as u32;
                let i1 = strips.points[offset + j + 1] as u32;
                let i2 = strips.points[offset + j + 2] as u32;
                if j % 2 == 0 {
                    indices.push(*base_vertex + i0);
                    indices.push(*base_vertex + i1);
                    indices.push(*base_vertex + i2);
                } else {
                    indices.push(*base_vertex + i1);
                    indices.push(*base_vertex + i0);
                    indices.push(*base_vertex + i2);
                }
            }
            offset += len;
        }
    }

    *base_vertex += data.vertices.len() as u32;
}
