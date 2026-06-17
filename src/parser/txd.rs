//! RenderWare Texture Dictionary (TXD) binary stream parser.
//!
//! Layout for GTA III / VC / SA PC texture dictionaries:
//!
//! ```text
//! TEXDICT_MAIN (0x16)
//!   ├── STRUCT (0x01): device_id (u16)
//!   └── TEXTURENATIVE (0x15) × N
//!         ├── platform_id (u32)
//!         ├── filter_flags / wrap / padding (4 bytes)
//!         ├── diffuse_name: length (u32) + data
//!         ├── alpha_name:   length (u32) + data
//!         ├── raster_format (u32)
//!         ├── width (u16), height (u16)
//!         ├── depth (u8), num_mipmaps (u8), raster_type (u8), alpha (u8)
//!         └── [optional EXTENSION (0x03) for cube maps]
//! ```

use crate::parser::texture_decoder;

/// RenderWare section type IDs.
pub mod rw {
    pub const STRUCT: u32 = 0x01;
    pub const STRING: u32 = 0x02;
    pub const EXTENSION: u32 = 0x03;
    pub const TEXTURE: u32 = 0x06;
    pub const TEXTURE_NATIVE: u32 = 0x15;
    pub const TEXTURE_DICTIONARY: u32 = 0x16;
}

/// A parsed TXD file containing zero or more textures.
#[derive(Debug, Clone, Default)]
pub struct TxdFile {
    pub device_id: u16,
    pub rw_version: u32,
    pub textures: Vec<NativeTexture>,
}

/// A single native texture within a TXD.
#[derive(Debug, Clone)]
pub struct NativeTexture {
    pub platform_id: u32,
    pub diffuse_name: String,
    pub alpha_name: String,
    pub raster_format: u32,
    pub width: u32,
    pub height: u32,
    pub depth: u8,
    pub num_mipmaps: u8,
    pub raster_type: u8,
    pub has_alpha: u8,
    pub palette: Vec<u8>,
    pub mipmaps: Vec<MipmapLevel>,
}

#[derive(Debug, Clone)]
pub struct MipmapLevel {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl NativeTexture {
    /// Decode the base mipmap level to RGBA.
    pub fn decode_rgba(&self) -> Result<Vec<u8>, texture_decoder::DecodeError> {
        let mip = self
            .mipmaps
            .first()
            .expect("NativeTexture has at least one mipmap");
        texture_decoder::decode_raster(
            &mip.data,
            self.width,
            self.height,
            self.raster_format,
            &self.palette,
            self.raster_type,
        )
    }

    /// Human-readable format name.
    pub fn format_name(&self) -> &'static str {
        texture_decoder::format::format_name(self.raster_format)
    }

    /// Whether the raster format has DXT compression.
    pub fn is_dxt(&self) -> bool {
        texture_decoder::format::is_dxt(self.raster_format)
    }
}

/// Parse a complete TXD file from raw bytes.
pub fn parse_txd(bytes: &[u8]) -> Result<TxdFile, String> {
    if bytes.len() < 12 {
        return Err("file too short".to_string());
    }

    let mut pos = 0usize;

    // Read top-level section header.
    let (section_type, section_size, rw_version) = read_section_header(bytes, &mut pos)?;
    if section_type != rw::TEXTURE_DICTIONARY {
        return Err(format!(
            "expected TEXTURE_DICTIONARY section (0x16), got 0x{:02X}",
            section_type
        ));
    }

    let section_end = pos + section_size as usize;
    if section_end > bytes.len() {
        return Err(format!(
            "section size {} exceeds file length {}",
            section_size,
            bytes.len()
        ));
    }

    // Read struct (0x01) within the dictionary.
    let mut device_id = 0u16;
    let mut textures = Vec::new();

    while pos < section_end {
        if pos + 12 > bytes.len() {
            break;
        }
        let (child_type, child_size, _) = read_section_header(bytes, &mut pos)?;
        let child_end = pos + child_size as usize;

        match child_type {
            rw::STRUCT => {
                // Struct data: device_id (u16) + 6 bytes padding
                if pos + 2 <= bytes.len() {
                    device_id = u16::from_le_bytes([bytes[pos], bytes[pos + 1]]);
                }
                pos = child_end.min(section_end);
            }
            rw::TEXTURE_NATIVE => {
                let chunk = &bytes[pos..child_end.min(bytes.len())];
                match parse_native_texture(chunk) {
                    Ok(tex) => textures.push(tex),
                    Err(e) => {
                        // Skip unparseable textures silently.
                        let _ = e;
                    }
                }
                pos = child_end.min(section_end);
            }
            _ => {
                // Skip unknown child sections.
                pos = child_end.min(section_end);
            }
        }
    }

    Ok(TxdFile {
        device_id,
        rw_version,
        textures,
    })
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

/// Read a length-prefixed Pascal-style string (u32 length + data).
fn read_pstring(bytes: &[u8], pos: &mut usize) -> Result<String, String> {
    if *pos + 4 > bytes.len() {
        return Err("unexpected end reading string length".to_string());
    }
    let len = u32::from_le_bytes([bytes[*pos], bytes[*pos + 1], bytes[*pos + 2], bytes[*pos + 3]]) as usize;
    *pos += 4;

    if *pos + len > bytes.len() {
        return Err(format!("string length {} exceeds buffer", len));
    }

    let s = if len > 0 {
        std::str::from_utf8(&bytes[*pos..*pos + len])
            .map_err(|e| format!("invalid UTF-8 in string: {e}"))?
            .to_string()
    } else {
        String::new()
    };
    // Strings are padded to 4-byte alignment in RW.
    let aligned = (len + 3) & !3;
    *pos += aligned;
    Ok(s)
}

/// Parse a TEXTURENATIVE section body.
fn parse_native_texture(bytes: &[u8]) -> Result<NativeTexture, String> {
    if bytes.len() < 36 {
        return Err(format!(
            "TEXTURENATIVE body too short: {} bytes",
            bytes.len()
        ));
    }

    let mut pos = 0usize;

    // platform_id
    let platform_id = u32::from_le_bytes([bytes[pos], bytes[pos + 1], bytes[pos + 2], bytes[pos + 3]]);
    pos += 4;

    // filter_flags, wrap_v, wrap_u, padding
    pos += 4;

    // diffuse name
    let diffuse_name = read_pstring(bytes, &mut pos)?;
    let alpha_name = read_pstring(bytes, &mut pos)?;

    // Raster format
    if pos + 4 > bytes.len() {
        return Err("unexpected end before raster_format".to_string());
    }
    let raster_format = u32::from_le_bytes([bytes[pos], bytes[pos + 1], bytes[pos + 2], bytes[pos + 3]]);
    pos += 4;

    // width, height
    if pos + 4 > bytes.len() {
        return Err("unexpected end before dimensions".to_string());
    }
    let width = u16::from_le_bytes([bytes[pos], bytes[pos + 1]]) as u32;
    let height = u16::from_le_bytes([bytes[pos + 2], bytes[pos + 3]]) as u32;
    pos += 4;

    // depth, num_mipmaps, raster_type, alpha
    if pos + 4 > bytes.len() {
        return Err("unexpected end before raster metadata".to_string());
    }
    let depth = bytes[pos];
    let num_mipmaps = bytes[pos + 1];
    let raster_type = bytes[pos + 2];
    let has_alpha = bytes[pos + 3];
    pos += 4;

    // For PC Direct3D textures, mipmap data follows immediately.
    // For textures with palettes, palette data comes first.
    let base = raster_format & 0xFFF;
    let is_pal4 = (raster_format & 0x4000) != 0;
    let is_pal8 = (raster_format & 0x2000) != 0;

    let palette = if is_pal4 || is_pal8 {
        let palette_size = if is_pal4 { 16 * 4 } else { 256 * 4 };
        if pos + 4 > bytes.len() {
            return Err("unexpected end before palette size".to_string());
        }
        let pal_data_size =
            u32::from_le_bytes([bytes[pos], bytes[pos + 1], bytes[pos + 2], bytes[pos + 3]]) as usize;
        pos += 4;

        if pos + pal_data_size > bytes.len() {
            return Err("palette data exceeds buffer".to_string());
        }
        let pal = bytes[pos..pos + pal_data_size.min(palette_size)].to_vec();
        // Align to 4 bytes
        let aligned = (pal_data_size + 3) & !3;
        pos += aligned;
        pal
    } else {
        Vec::new()
    };

    // Read mipmap data.
    // For PC textures on the GTA render path, mipmap data follows the
    // header with an optional 4-byte header size field for DXT formats.
    let remaining = bytes.len() - pos;

    let mut mipmaps = Vec::new();

    // Determine actual mipmap dimensions.
    let actual_mip_count = if num_mipmaps > 0 { num_mipmaps } else { 1 };
    let mip_widths: Vec<u32> = (0..actual_mip_count)
        .scan(width, |w, _| {
            let wc = *w;
            *w = (*w / 2).max(1);
            Some(wc)
        })
        .collect();
    let mip_heights: Vec<u32> = (0..actual_mip_count)
        .scan(height, |h, _| {
            let hc = *h;
            *h = (*h / 2).max(1);
            Some(hc)
        })
        .collect();

    // For DXT, there may be a 4-byte header size before each mipmap.
    // For uncompressed formats, mipmap data is stored end-to-end.
    if is_dxt_format(base) {
        // DXT-compressed: each mipmap has a 4-byte data_size prefix,
        // then the DXT blocks.
        let mut data_pos = pos;
        for mi in 0..actual_mip_count as usize {
            if data_pos + 4 > bytes.len() {
                break;
            }
            let mip_size = u32::from_le_bytes([
                bytes[data_pos],
                bytes[data_pos + 1],
                bytes[data_pos + 2],
                bytes[data_pos + 3],
            ]) as usize;
            data_pos += 4;

            let mw = mip_widths.get(mi).copied().unwrap_or(1);
            let mh = mip_heights.get(mi).copied().unwrap_or(1);

            let actual_data = if data_pos + mip_size <= bytes.len() {
                bytes[data_pos..data_pos + mip_size].to_vec()
            } else {
                let available = bytes.len().saturating_sub(data_pos);
                bytes[data_pos..data_pos + available].to_vec()
            };
            data_pos += (mip_size + 3) & !3; // Align to 4 bytes

            mipmaps.push(MipmapLevel {
                width: mw,
                height: mh,
                data: actual_data,
            });
        }
    } else if is_pal4 || is_pal8 {
        // Palettized: pixel indices follow the palette.
        let data_pos = pos;
        let pixel_data = bytes[data_pos..].to_vec();
        mipmaps.push(MipmapLevel {
            width,
            height,
            data: pixel_data,
        });
    } else {
        // Uncompressed: data is stored end-to-end.
        let data_pos = pos;
        let mut data_offset = 0usize;
        for mi in 0..actual_mip_count as usize {
            let mw = mip_widths.get(mi).copied().unwrap_or(1) as usize;
            let mh = mip_heights.get(mi).copied().unwrap_or(1) as usize;

            let (bpp, row_align) = bpp_and_align(base, raster_type);
            let row_stride = ((mw * bpp as usize + row_align - 1) / row_align) * row_align;
            let mip_byte_size = row_stride * mh;
            let aligned_size = (mip_byte_size + 3) & !3;

            let end = data_offset + aligned_size.min(remaining.saturating_sub(data_offset));
            let mip_data = if end <= remaining {
                bytes[data_pos + data_offset..data_pos + end].to_vec()
            } else {
                vec![]
            };

            mipmaps.push(MipmapLevel {
                width: mw as u32,
                height: mh as u32,
                data: mip_data,
            });
            data_offset += aligned_size;
        }
    }

    // Ensure at least one mipmap.
    if mipmaps.is_empty() {
        let data_len = remaining.min(bytes.len().saturating_sub(pos));
        mipmaps.push(MipmapLevel {
            width,
            height,
            data: bytes[pos..pos + data_len].to_vec(),
        });
    }

    Ok(NativeTexture {
        platform_id,
        diffuse_name,
        alpha_name,
        raster_format,
        width,
        height,
        depth,
        num_mipmaps: actual_mip_count,
        raster_type,
        has_alpha,
        palette,
        mipmaps,
    })
}

fn is_dxt_format(base: u32) -> bool {
    base == 0x100 || base == 0x200 || base == 0x300
}

fn bpp_and_align(base: u32, raster_type: u8) -> (usize, usize) {
    match base {
        0x400 => (1, 1),  // LUM8
        0x500 | 0x501 => (4, 4), // 8888
        0x600 => (3, 4),  // 888
        0x000..=0x500 => {
            // Try common formats.
            if raster_type == 0x12 {
                (4, 4) // DXT-like sizing
            } else {
                (2, 4) // Default 16-bit
            }
        }
        _ => (4, 4), // Default: 32-bit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reject_empty_file() {
        assert!(parse_txd(&[]).is_err());
    }

    #[test]
    fn reject_non_txd_section() {
        // Bogus section type 0x01 (STRUCT) with size 0
        let bytes = vec![
            0x01, 0x00, 0x00, 0x00, // type = STRUCT
            0x00, 0x00, 0x00, 0x00, // size = 0
            0xFF, 0xFF, 0x03, 0x10, // version
        ];
        assert!(parse_txd(&bytes).is_err());
    }

    #[test]
    fn parses_minimal_txd_header() {
        // Minimal TXD: TEXDICT_MAIN + struct { device_id=0 }
        let mut bytes = Vec::new();
        // TEXDICT_MAIN section
        bytes.extend_from_slice(&[0x16, 0x00, 0x00, 0x00]); // type
        bytes.extend_from_slice(&[0x08, 0x00, 0x00, 0x00]); // size = 8
        bytes.extend_from_slice(&[0xFF, 0xFF, 0x03, 0x10]); // version
        // STRUCT child
        bytes.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]); // type = STRUCT
        bytes.extend_from_slice(&[0x08, 0x00, 0x00, 0x00]); // size = 8
        bytes.extend_from_slice(&[0xFF, 0xFF, 0x03, 0x10]); // version
        bytes.extend_from_slice(&[0x02, 0x00]); // device_id = 2
        bytes.extend_from_slice(&[0x00; 6]); // padding

        let txd = parse_txd(&bytes).unwrap();
        assert_eq!(txd.device_id, 2);
        assert!(txd.textures.is_empty());
    }

    #[test]
    fn roundtrip_rw_version() {
        let bytes = vec![
            0x16, 0x00, 0x00, 0x00, // type = TEXDICT
            0x08, 0x00, 0x00, 0x00, // size = 8
            0x10, 0x00, 0x00, 0x00, // version
            0x01, 0x00, 0x00, 0x00, // STRUCT
            0x08, 0x00, 0x00, 0x00,
            0x10, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        let txd = parse_txd(&bytes).unwrap();
        assert_eq!(txd.rw_version, 0x10);
    }
}
