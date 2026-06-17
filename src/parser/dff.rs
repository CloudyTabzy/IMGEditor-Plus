//! RenderWare DFF (Drawable File Format) parser.
//!
//! Extracts mesh geometry (vertices, triangles, normals, UVs) from a RW
//! Clump (0x10) section. Textures and materials are noted but not decoded
//! — this parser is geometry-only, feeding the 3D viewer pipeline.

/// Parsed mesh from one GEOMETRY section within a DFF Clump.
#[derive(Debug, Clone)]
pub struct DffMesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
    pub material_name: Option<String>,
    pub texture_name: Option<String>,
}

/// Parse a DFF file and return all meshes found.
pub fn parse_dff(bytes: &[u8]) -> Result<Vec<DffMesh>, String> {
    if bytes.len() < 12 {
        return Err("file too short".to_string());
    }

    let mut pos = 0usize;

    // Top-level section: expects CLUMP (0x10)
    let (top_type, top_size, _rw_version) = read_section_header(bytes, &mut pos)?;
    if top_type != 0x10 {
        return Err(format!(
            "expected CLUMP section (0x10), got 0x{:02X}",
            top_type
        ));
    }

    let section_end = pos + top_size as usize;
    if section_end > bytes.len() {
        return Err(format!("section size {} exceeds file length", top_size));
    }

    // Walk all sections recursively within the Clump, collecting geometry.
    struct GeomInfo {
        vertices: Vec<[f32; 3]>,
        normals: Vec<[f32; 3]>,
        uvs: Vec<[f32; 2]>,
        triangles: Vec<u16>,
        num_verts: u32,
        num_tris: u32,
        material_name: Option<String>,
        texture_name: Option<String>,
    }

    let mut geoms: Vec<GeomInfo> = Vec::new();

    let mut child_pos = pos;
    while child_pos + 12 <= section_end {
        let (child_type, child_size, _) = read_section_header(bytes, &mut child_pos)?;
        let child_end = (child_pos + child_size as usize).min(section_end);

        match child_type {
            0x01 => {
                // STRUCT of the Clump: num_atomics, num_lights, num_cameras
                // Not needed for mesh extraction, skip.
            }
            0x14 => {
                // ATOMIC: references a geometry. Skip for now — geometry
                // data is embedded as its own sibling section.
            }
            0x12 | 0x05 => {
                // LIGHT / CAMERA — skip.
            }
            0x0F => {
                // GEOMETRY section
                let mut geom = GeomInfo {
                    vertices: Vec::new(),
                    normals: Vec::new(),
                    uvs: Vec::new(),
                    triangles: Vec::new(),
                    num_verts: 0,
                    num_tris: 0,
                    material_name: None,
                    texture_name: None,
                };

                // Walk GEOMETRY children.
                let mut g_pos = child_pos;
                while g_pos + 12 <= child_end {
                    let (g_type, g_size, _) = read_section_header(bytes, &mut g_pos)?;
                    let g_end = (g_pos + g_size as usize).min(child_end);

                    match g_type {
                        0x01 => {
                            // Geometry STRUCT — vertex/triangle data.
                            if g_pos + 16 > g_end {
                                break;
                            }
                            let format_flags = u32::from_le_bytes([
                                bytes[g_pos], bytes[g_pos + 1], bytes[g_pos + 2], bytes[g_pos + 3],
                            ]);
                            let flags = format_flags;
                            geom.num_tris = u32::from_le_bytes([
                                bytes[g_pos + 4], bytes[g_pos + 5], bytes[g_pos + 6], bytes[g_pos + 7],
                            ]);
                            geom.num_verts = u32::from_le_bytes([
                                bytes[g_pos + 8], bytes[g_pos + 9], bytes[g_pos + 10], bytes[g_pos + 11],
                            ]);
                            let num_morph = u32::from_le_bytes([
                                bytes[g_pos + 12], bytes[g_pos + 13], bytes[g_pos + 14], bytes[g_pos + 15],
                            ]);
                            let _ = num_morph;

                            let mut data_pos = g_pos + 16;

                            let has_prelit = (flags & 0x004) != 0;
                            let has_normals = (flags & 0x008) != 0;
                            let has_uvs_1 = (flags & 0x040) != 0;
                            let has_uvs_2 = (flags & 0x080) != 0;
                            let has_triangles = (flags & 0x002) != 0;
                            let num_uv_sets = if has_uvs_2 { 2 } else if has_uvs_1 { 1 } else { 0 };

                            // Skip prelit colors if present.
                            if has_prelit {
                                data_pos += geom.num_verts as usize * 4;
                            }

                            // Read UV sets.
                            if num_uv_sets > 0 {
                                if data_pos + 4 <= g_end {
                                    let stored_uv_sets = u32::from_le_bytes([
                                        bytes[data_pos], bytes[data_pos + 1],
                                        bytes[data_pos + 2], bytes[data_pos + 3],
                                    ]) as usize;
                                    data_pos += 4;
                                    let actual_sets = stored_uv_sets.min(num_uv_sets);
                                    for _s in 0..actual_sets {
                                        for _v in 0..geom.num_verts as usize {
                                            if data_pos + 8 <= g_end {
                                                let u = f32::from_le_bytes([
                                                    bytes[data_pos], bytes[data_pos + 1],
                                                    bytes[data_pos + 2], bytes[data_pos + 3],
                                                ]);
                                                let v = f32::from_le_bytes([
                                                    bytes[data_pos + 4], bytes[data_pos + 5],
                                                    bytes[data_pos + 6], bytes[data_pos + 7],
                                                ]);
                                                data_pos += 8;
                                                // Only store the first UV set.
                                                if _s == 0 {
                                                    geom.uvs.push([u, v]);
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            // Read triangles.
                            if has_triangles {
                                let tri_bytes = geom.num_tris as usize * 8; // 4 × u16 per tri
                                let end = (data_pos + tri_bytes).min(g_end);
                                let raw_range = if data_pos + 4 <= end {
                                    // Some versions store a 4-byte buffer flags field here.
                                    // Skip it if present — check for it by looking for
                                    // the triangle count match.
                                    data_pos += 4;
                                    data_pos..end.min(data_pos + (geom.num_tris as usize * 8))
                                } else {
                                    data_pos..end
                                };
                                for ti in 0..geom.num_tris as usize {
                                    let idx = raw_range.start + ti * 8;
                                    if idx + 8 <= raw_range.end {
                                        let v2 = u16::from_le_bytes([bytes[idx], bytes[idx + 1]]);
                                        let v1 = u16::from_le_bytes([bytes[idx + 2], bytes[idx + 3]]);
                                        let v0 = u16::from_le_bytes([bytes[idx + 4], bytes[idx + 5]]);
                                        geom.triangles.push(v0);
                                        geom.triangles.push(v1);
                                        geom.triangles.push(v2);
                                    }
                                }
                                // Triangle data ends at raw_range.end.
                                data_pos = raw_range.end;
                            }

                            // Morph targets: vertices and normals.
                            for _m in 0..num_morph.max(1) as usize {
                                // Bounding sphere: 4 × f32
                                if data_pos + 16 <= g_end {
                                    data_pos += 16;
                                }
                                // Has vertices (u8 or u32 — usually u32)
                                if data_pos + 4 <= g_end {
                                    let has_verts =
                                        u32::from_le_bytes([bytes[data_pos], bytes[data_pos + 1],
                                                             bytes[data_pos + 2], bytes[data_pos + 3]]) != 0;
                                    data_pos += 4;
                                    if has_verts {
                                        let verts_needed = geom.num_verts as usize * 12;
                                        let end = (data_pos + verts_needed).min(g_end);
                                        for _v in 0..geom.num_verts as usize {
                                            let idx = data_pos + _v * 12;
                                            if idx + 12 <= end {
                                                let x = f32::from_le_bytes([
                                                    bytes[idx], bytes[idx + 1], bytes[idx + 2], bytes[idx + 3],
                                                ]);
                                                let y = f32::from_le_bytes([
                                                    bytes[idx + 4], bytes[idx + 5], bytes[idx + 6], bytes[idx + 7],
                                                ]);
                                                let z = f32::from_le_bytes([
                                                    bytes[idx + 8], bytes[idx + 9], bytes[idx + 10], bytes[idx + 11],
                                                ]);
                                                geom.vertices.push([x, y, z]);
                                            }
                                        }
                                        data_pos += verts_needed;
                                    }
                                }

                                let has_norms_flag = has_normals;
                                if has_norms_flag {
                                    // Has normals (u8 or u32)
                                    if data_pos + 4 <= g_end {
                                        let has_norms_read =
                                            u32::from_le_bytes([bytes[data_pos], bytes[data_pos + 1],
                                                                 bytes[data_pos + 2], bytes[data_pos + 3]]) != 0;
                                        data_pos += 4;
                                        if has_norms_read {
                                            let norms_needed = geom.num_verts as usize * 12;
                                            let end = (data_pos + norms_needed).min(g_end);
                                            for _v in 0..geom.num_verts as usize {
                                                let idx = data_pos + _v * 12;
                                                if idx + 12 <= end {
                                                    let x = f32::from_le_bytes([
                                                        bytes[idx], bytes[idx + 1], bytes[idx + 2], bytes[idx + 3],
                                                    ]);
                                                    let y = f32::from_le_bytes([
                                                        bytes[idx + 4], bytes[idx + 5], bytes[idx + 6], bytes[idx + 7],
                                                    ]);
                                                    let z = f32::from_le_bytes([
                                                        bytes[idx + 8], bytes[idx + 9], bytes[idx + 10], bytes[idx + 11],
                                                    ]);
                                                    geom.normals.push([x, y, z]);
                                                }
                                            }
                                            data_pos += norms_needed;
                                        }
                                    }
                                }
                            }

                            // Pad to 4 bytes.
                            let _ = data_pos;
                        }
                        0x08 => {
                            // MATERIAL_LIST
                            let mut m_pos = g_pos;
                            while m_pos + 12 <= g_end {
                                let (m_type, m_size, _) = read_section_header(bytes, &mut m_pos)?;
                                let m_end = (m_pos + m_size as usize).min(g_end);
                                if m_type == 0x07 {
                                    // MATERIAL — walk children for TEXTURE.
                                    let mut t_pos = m_pos;
                                    while t_pos + 12 <= m_end {
                                        let (t_type, t_size, _) = read_section_header(bytes, &mut t_pos)?;
                                        let t_end = (t_pos + t_size as usize).min(m_end);
                                        if t_type == 0x06 {
                                            // TEXTURE — has sub-sections for name/alpha strings.
                                            let mut s_pos = t_pos;
                                            while s_pos + 12 <= t_end {
                                                let (s_type, s_size, _) = read_section_header(bytes, &mut s_pos)?;
                                                let s_end = (s_pos + s_size as usize).min(t_end);
                                                if s_type == 0x02 {
                                                    // STRING — texture name
                                                    let len = s_size as usize;
                                                    if s_pos + len <= s_end && len > 0 {
                                                        let name_str = std::str::from_utf8(
                                                            &bytes[s_pos..s_pos + len],
                                                        )
                                                        .ok()
                                                        .map(|s| s.trim_end_matches('\0').to_string())
                                                        .filter(|s| !s.is_empty());
                                                        if geom.texture_name.is_none() {
                                                            geom.texture_name = name_str;
                                                        } else if geom.material_name.is_none() {
                                                            geom.material_name = name_str;
                                                        }
                                                    }
                                                }
                                                s_pos = s_end;
                                            }
                                        }
                                        t_pos = t_end;
                                    }
                                }
                                m_pos = m_end;
                            }
                        }
                        0x03 => {
                            // EXTENSION — skip
                        }
                        _ => {}
                    }
                    g_pos = g_end;
                }

                geoms.push(geom);
            }
            0x0E => {
                // FRAME_LIST — skip (not needed for geometry extraction)
            }
            _ => {}
        }

        child_pos = child_end;
    }

    // Convert collected geom info to DffMesh.
    let meshes: Vec<DffMesh> = geoms
        .into_iter()
        .filter(|g| !g.vertices.is_empty())
        .map(|g| {
            let indices: Vec<u32> = g.triangles.iter().map(|&i| i as u32).collect();
            DffMesh {
                positions: g.vertices,
                normals: g.normals,
                uvs: g.uvs,
                indices,
                material_name: g.material_name,
                texture_name: g.texture_name,
            }
        })
        .collect();

    if meshes.is_empty() {
        return Err("no renderable geometry found in DFF".to_string());
    }

    Ok(meshes)
}

fn read_section_header(bytes: &[u8], pos: &mut usize) -> Result<(u32, u32, u32), String> {
    if *pos + 12 > bytes.len() {
        return Err("unexpected end of section header".to_string());
    }
    let section_type = u32::from_le_bytes([
        bytes[*pos],
        bytes[*pos + 1],
        bytes[*pos + 2],
        bytes[*pos + 3],
    ]);
    let section_size = u32::from_le_bytes([
        bytes[*pos + 4],
        bytes[*pos + 5],
        bytes[*pos + 6],
        bytes[*pos + 7],
    ]);
    let rw_version = u32::from_le_bytes([
        bytes[*pos + 8],
        bytes[*pos + 9],
        bytes[*pos + 10],
        bytes[*pos + 11],
    ]);
    *pos += 12;
    Ok((section_type, section_size, rw_version))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reject_empty() {
        assert!(parse_dff(&[]).is_err());
    }

    #[test]
    fn reject_non_clump() {
        let bytes = vec![
            0x01, 0x00, 0x00, 0x00, // type = STRUCT (not CLUMP)
            0x00, 0x00, 0x00, 0x00, // size = 0
            0xFF, 0xFF, 0x03, 0x10, // version
        ];
        assert!(parse_dff(&bytes).is_err());
    }
}
