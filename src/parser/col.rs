//! GTA Collision (`.col`) file parser.
//!
//! Supported versions:
//! - **COL1** (`COLL`): GTA III — 12-byte float32 vertices, 16-byte faces
//! - **COL2** (`COL2`): Vice City — 6-byte compressed (int16) vertices, 8-byte faces
//! - **COL3** (`COL3`): San Andreas — same as COL2 + shadow mesh
//!
//! A `.col` file is a concatenation of entries (one per model). Each entry
//! carries its own header, bounding shapes, and optionally a collision mesh.

use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColVersion {
    V1,  // COLL
    V2,  // COL2
    V3,  // COL3
}

#[derive(Debug, Error)]
pub enum ColError {
    #[error("file too short")]
    TooShort,
    #[error("unknown COL version magic: {0:?}")]
    UnknownMagic([u8; 4]),
    #[error("unexpected end of entry data")]
    Truncated,
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
    pub num_spheres: u32,
    pub num_boxes: u32,
    pub has_shadow: bool,
}

/// Parse a complete `.col` file, returning all entries.
pub fn parse_col(bytes: &[u8]) -> Result<ColFile, ColError> {
    if bytes.len() < 8 {
        return Err(ColError::TooShort);
    }

    let mut entries = Vec::new();
    let mut pos = 0usize;

    loop {
        if pos + 4 > bytes.len() {
            break; // gracefully stop at EOF
        }

        // Try to read a magic at the current position.
        let magic = [bytes[pos], bytes[pos + 1], bytes[pos + 2], bytes[pos + 3]];
        let (version, vert_size, face_size) = match &magic {
            b"COLL" => (ColVersion::V1, 12usize, 16usize),
            b"COL2" => (ColVersion::V2, 6usize, 8usize),
            b"COL3" => (ColVersion::V3, 6usize, 8usize),
            _ => break, // no more entries
        };

        if pos + 8 > bytes.len() {
            break;
        }

        // Total entry size (including header) — skip past it to find next entry.
        let entry_total = u32::from_le_bytes([
            bytes[pos + 4], bytes[pos + 5], bytes[pos + 6], bytes[pos + 7],
        ]) as usize;

        let _entry_start = pos;
        let entry_end = (pos + entry_total).min(bytes.len());

        // Move past the 8-byte header.
        let mut hp = pos + 8;

        // --- Read model name (null-terminated string, padded to 4 bytes) ---
        let name_start = hp;
        let mut name_len = 0usize;
        while hp + name_len < entry_end && bytes[name_start + name_len] != 0 {
            name_len += 1;
        }
        let model_name = if name_len > 0 {
            std::str::from_utf8(&bytes[name_start..name_start + name_len])
                .unwrap_or("")
                .to_string()
        } else {
            String::new()
        };
        hp = name_start + name_len + 1; // skip past NUL
        // Pad to 4 bytes.
        hp = (hp + 3) & !3;

        // --- Skip past header fields (model_id, unknown flags, offsets, etc.) ---
        // The header between the name and the data section layout varies by version.
        //
        // From the IMGF reference the header stores:
        //   - model_id: u16
        //   - unknown1: u32 (COL1 only)
        //   - various counts (spheres, boxes, vertices, faces, face_groups, cones)
        //   - shadow mesh counts (COL2/COL3)
        //   - version_flags
        //
        // Rather than parsing every field, we scan for the known counts by
        // reading the standard header layout for each version.

        // Common header fields after name:
        //   u16 model_id
        //   u32 unknown1 (COL1 only)
        //   9 × u32 counts:
        //     num_spheres, num_boxes, num_vertices, num_faces,
        //     num_face_groups, num_cones,
        //     shadow_num_vertices, shadow_num_faces (0 for COL1),
        //     version_flags

        let _header_fields_size = if version == ColVersion::V1 {
            2 + 4 + 9 * 4 // u16 + u32 + 9*u32
        } else {
            2 + 9 * 4 // u16 + 9*u32 (no unknown1)
        };

        // Read counts from the expected offset.
        let counts_offset = if version == ColVersion::V1 {
            hp + 2 + 4 // skip model_id + unknown1
        } else {
            hp + 2 // skip model_id
        };

        if counts_offset + 6 * 4 > entry_end {
            pos = entry_end;
            continue;
        }

        // Read counts in order: num_spheres, num_boxes, num_vertices, num_faces,
        // num_face_groups, num_cones
        let num_spheres = u32::from_le_bytes([
            bytes[counts_offset], bytes[counts_offset + 1],
            bytes[counts_offset + 2], bytes[counts_offset + 3],
        ]);
        let num_boxes = u32::from_le_bytes([
            bytes[counts_offset + 4], bytes[counts_offset + 5],
            bytes[counts_offset + 6], bytes[counts_offset + 7],
        ]);
        let num_vertices = u32::from_le_bytes([
            bytes[counts_offset + 8], bytes[counts_offset + 9],
            bytes[counts_offset + 10], bytes[counts_offset + 11],
        ]);
        let num_faces = u32::from_le_bytes([
            bytes[counts_offset + 12], bytes[counts_offset + 13],
            bytes[counts_offset + 14], bytes[counts_offset + 15],
        ]);
        let _num_face_groups = u32::from_le_bytes([
            bytes[counts_offset + 16], bytes[counts_offset + 17],
            bytes[counts_offset + 18], bytes[counts_offset + 19],
        ]);
        let _num_cones = u32::from_le_bytes([
            bytes[counts_offset + 20], bytes[counts_offset + 21],
            bytes[counts_offset + 22], bytes[counts_offset + 23],
        ]);

        let shadow_verts_offset = if version != ColVersion::V1 {
            counts_offset + 24
        } else {
            counts_offset + 24
        };

        let shadow_counts_offset = shadow_verts_offset;
        let shadow_num_v = if version != ColVersion::V1 {
            if shadow_counts_offset + 8 <= entry_end {
                let sv = u32::from_le_bytes([
                    bytes[shadow_counts_offset], bytes[shadow_counts_offset + 1],
                    bytes[shadow_counts_offset + 2], bytes[shadow_counts_offset + 3],
                ]);
                let _sf = u32::from_le_bytes([
                    bytes[shadow_counts_offset + 4], bytes[shadow_counts_offset + 5],
                    bytes[shadow_counts_offset + 6], bytes[shadow_counts_offset + 7],
                ]);
                sv
            } else {
                0
            }
        } else {
            0
        };
        let has_shadow = shadow_num_v > 0;

        // Data section order: TBounds (40 bytes), spheres, boxes, vertices, faces,
        // face_groups, shadow_vertices, shadow_faces.
        let data_offset = if version == ColVersion::V1 {
            counts_offset + 24
        } else {
            counts_offset + 24 + 8 // extra 2 shadow counts + padding
        };
        // Pad data_offset to 4 bytes.
        let data_offset = (data_offset + 3) & !3;

        let mut dp = data_offset;

        // TBounds: 40 bytes (bounding sphere + AABB). Skip.
        dp += 40;

        // Spheres: num_spheres × 20 bytes each.
        dp += num_spheres as usize * 20;

        // Boxes: num_boxes × 28 bytes each.
        dp += num_boxes as usize * 28;

        // Vertices.
        let mut vertices: Vec<[f32; 3]> = Vec::new();
        if num_vertices > 0 {
            let vert_bytes = num_vertices as usize * vert_size;
            if dp + vert_bytes <= entry_end {
                for i in 0..num_vertices as usize {
                    let vp = dp + i * vert_size;
                    if vp + vert_size <= entry_end {
                        let (x, y, z) = if vert_size == 12 {
                            // Uncompressed float32 × 3
                            let x = f32::from_le_bytes([
                                bytes[vp], bytes[vp+1], bytes[vp+2], bytes[vp+3],
                            ]);
                            let y = f32::from_le_bytes([
                                bytes[vp+4], bytes[vp+5], bytes[vp+6], bytes[vp+7],
                            ]);
                            let z = f32::from_le_bytes([
                                bytes[vp+8], bytes[vp+9], bytes[vp+10], bytes[vp+11],
                            ]);
                            (x, y, z)
                        } else {
                            // Compressed int16 × 3
                            let x = i16::from_le_bytes([bytes[vp], bytes[vp+1]]) as f32;
                            let y = i16::from_le_bytes([bytes[vp+2], bytes[vp+3]]) as f32;
                            let z = i16::from_le_bytes([bytes[vp+4], bytes[vp+5]]) as f32;
                            (x, y, z)
                        };
                        vertices.push([x, y, z]);
                    }
                }
                dp += vert_bytes;
                // Pad to 4 bytes.
                dp = (dp + 3) & !3;
            }
        }

        let mut indices: Vec<u32> = Vec::new();

        // Read raw face indices from the faces section.
        // To determine where faces data lives in the stream, read from the
        // current position after vertex data plus any triangle plane data.
        // After vertices, there may be triangle plane data (4 × f32 per face).
        let tri_plane_bytes = if version == ColVersion::V1 {
            num_faces as usize * 16 // 4 × f32
        } else {
            num_faces as usize * 16 // 4 × f32
        };

        // Faces follow triangle planes. Skip planes.
        if dp + tri_plane_bytes <= entry_end {
            dp += tri_plane_bytes;
            dp = (dp + 3) & !3;
        }

        // Read faces.
        if num_faces > 0 {
            let face_bytes = num_faces as usize * face_size;
            if dp + face_bytes <= entry_end {
                for i in 0..num_faces as usize {
                    let fp = dp + i * face_size;
                    if fp + face_size <= entry_end {
                        let (v0, v1, v2) = if face_size == 16 {
                            // COL1: 3 × uint32 + surface
                            let v0 = u32::from_le_bytes([
                                bytes[fp], bytes[fp+1], bytes[fp+2], bytes[fp+3],
                            ]);
                            let v1 = u32::from_le_bytes([
                                bytes[fp+4], bytes[fp+5], bytes[fp+6], bytes[fp+7],
                            ]);
                            let v2 = u32::from_le_bytes([
                                bytes[fp+8], bytes[fp+9], bytes[fp+10], bytes[fp+11],
                            ]);
                            (v0, v1, v2)
                        } else {
                            // COL2/COL3: 3 × uint16 + surface
                            let v0 = u16::from_le_bytes([bytes[fp], bytes[fp+1]]) as u32;
                            let v1 = u16::from_le_bytes([bytes[fp+2], bytes[fp+3]]) as u32;
                            let v2 = u16::from_le_bytes([bytes[fp+4], bytes[fp+5]]) as u32;
                            (v0, v1, v2)
                        };
                        indices.push(v0);
                        indices.push(v1);
                        indices.push(v2);
                    }
                }
            }
        }

        if !vertices.is_empty() && !indices.is_empty() {
            entries.push(ColEntry {
                version,
                model_name,
                num_vertices,
                vertices,
                num_faces,
                indices,
                num_spheres,
                num_boxes,
                has_shadow,
            });
        }

        pos = entry_end;
    }

    if entries.is_empty() {
        return Err(ColError::NoGeometry);
    }

    Ok(ColFile { entries })
}

#[cfg(test)]
mod tests {
    use super::*;

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
        // Build a minimal COL1 entry with no geometry.
        let mut data = Vec::new();
        data.extend_from_slice(b"COLL");
        // Entry total size placeholder — fill in at end.
        let size_pos = data.len();
        data.extend_from_slice(&[0u8; 4]);

        // Model name: null (1 byte)
        data.push(0);
        // Pad to 4
        data.extend_from_slice(&[0, 0, 0]);

        // model_id: u16 = 0
        data.extend_from_slice(&[0u8; 2]);
        // unknown1: u32 = 0 (COL1 only)
        data.extend_from_slice(&[0u8; 4]);

        // 9 × u32 counts: all 0
        for _ in 0..9 {
            data.extend_from_slice(&[0u8; 4]);
        }
        // Pad data_offset to 4 bytes
        while data.len() % 4 != 0 {
            data.push(0);
        }

        let total_size = data.len();
        data[size_pos..size_pos + 4].copy_from_slice(&(total_size as u32).to_le_bytes());

        let result = parse_col(&data);
        assert!(result.is_err(), "expected NoGeometry error");
    }
}
