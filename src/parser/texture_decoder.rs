//! Decode RenderWare texture raster formats to raw RGBA pixels.
//!
//! Supported formats:
//! - DXT1 / BC1 (with 1-bit alpha)
//! - DXT3 / BC2 (explicit 4-bit alpha)
//! - DXT5 / BC3 (interpolated 8-bit alpha)
//! - 1555 ARGB (1-5-5-5)
//! - 565 RGB (5-6-5)
//! - 4444 ARGB (4-4-4-4)
//! - 8888 ARGB
//! - 888 RGB
//! - 555 XRGB
//! - LUM8 (8-bit luminance)
//! - PAL4 (4-bit index, palette)
//! - PAL8 (8-bit index, palette)

use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DxtType {
    Dxt1,
    Dxt3,
    Dxt5,
}

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("buffer too small: need {need} bytes, have {have}")]
    BufferTooSmall { need: usize, have: usize },
    #[error("unsupported format: 0x{0:03X}")]
    UnsupportedFormat(u32),
}

// ---- DXT block decoders ------------------------------------------------

fn dxt1_block(block: &[u8]) -> [[u8; 4]; 16] {
    let c0 = u16::from_le_bytes([block[0], block[1]]);
    let c1 = u16::from_le_bytes([block[2], block[3]]);

    let expand = |c: u16| -> [u8; 4] {
        let r5 = ((c >> 11) & 0x1F) as u8;
        let g6 = ((c >> 5) & 0x3F) as u8;
        let b5 = (c & 0x1F) as u8;
        [
            (r5 << 3) | (r5 >> 2),
            (g6 << 2) | (g6 >> 4),
            (b5 << 3) | (b5 >> 2),
            255,
        ]
    };

    let col0 = expand(c0);
    let col1 = expand(c1);
    let codes = u32::from_le_bytes([block[4], block[5], block[6], block[7]]);

    let mut out = [[0u8; 4]; 16];
    for i in 0..16 {
        let idx = ((codes >> (i * 2)) & 3) as u8;
        out[i] = match (c0 > c1, idx) {
            (true, 0) | (false, 0) => col0,
            (true, 1) | (false, 1) => col1,
            (true, 2) => {
                let r = ((col0[0] as u16 * 2 + col1[0] as u16) / 3) as u8;
                let g = ((col0[1] as u16 * 2 + col1[1] as u16) / 3) as u8;
                let b = ((col0[2] as u16 * 2 + col1[2] as u16) / 3) as u8;
                [r, g, b, 255]
            }
            (true, 3) => {
                let r = ((col0[0] as u16 + col1[0] as u16 * 2) / 3) as u8;
                let g = ((col0[1] as u16 + col1[1] as u16 * 2) / 3) as u8;
                let b = ((col0[2] as u16 + col1[2] as u16 * 2) / 3) as u8;
                [r, g, b, 255]
            }
            (false, 2) => {
                let avg = |a: u8, b: u8| ((a as u16 + b as u16) / 2) as u8;
                [
                    avg(col0[0], col1[0]),
                    avg(col0[1], col1[1]),
                    avg(col0[2], col1[2]),
                    255,
                ]
            }
            (false, 3) => [0, 0, 0, 0],
            _ => unreachable!(),
        };
    }
    out
}

fn dxt3_block(block: &[u8]) -> [[u8; 4]; 16] {
    // First 8 bytes: explicit 4-bit alpha per texel
    let mut out = dxt1_block(&block[8..16]);
    for i in 0..16 {
        let nibble = if i % 2 == 0 {
            block[i / 2] & 0x0F
        } else {
            (block[i / 2] >> 4) & 0x0F
        };
        out[i][3] = nibble * 17; // 4-bit → 8-bit
    }
    out
}

fn dxt5_block(block: &[u8]) -> [[u8; 4]; 16] {
    let alpha0 = block[0];
    let alpha1 = block[1];
    let alpha_codes = u64::from_le_bytes([
        block[2], block[3], block[4], block[5], block[6], block[7], 0, 0,
    ]);

    let interpolate_alpha = |idx: u8| -> u8 {
        match idx {
            0 => alpha0,
            1 => alpha1,
            2 => {
                if alpha0 > alpha1 {
                    (6 * alpha0 as u16 + 1 * alpha1 as u16 + 3) / 7
                } else {
                    (4 * alpha0 as u16 + 1 * alpha1 as u16 + 2) / 5
                }
                .min(255) as u8
            }
            3 => {
                if alpha0 > alpha1 {
                    (5 * alpha0 as u16 + 2 * alpha1 as u16 + 3) / 7
                } else {
                    (3 * alpha0 as u16 + 2 * alpha1 as u16 + 2) / 5
                }
                .min(255) as u8
            }
            4 => {
                if alpha0 > alpha1 {
                    (4 * alpha0 as u16 + 3 * alpha1 as u16 + 3) / 7
                } else {
                    (2 * alpha0 as u16 + 3 * alpha1 as u16 + 2) / 5
                }
                .min(255) as u8
            }
            5 => {
                if alpha0 > alpha1 {
                    (3 * alpha0 as u16 + 4 * alpha1 as u16 + 3) / 7
                } else {
                    (1 * alpha0 as u16 + 4 * alpha1 as u16 + 2) / 5
                }
                .min(255) as u8
            }
            6 => {
                if alpha0 > alpha1 {
                    (2 * alpha0 as u16 + 5 * alpha1 as u16 + 3) / 7
                } else {
                    (0 * alpha0 as u16 + 5 * alpha1 as u16 + 2) / 5
                }
                .min(255) as u8
            }
            7 => {
                if alpha0 > alpha1 {
                    (1 * alpha0 as u16 + 6 * alpha1 as u16 + 3) / 7
                } else {
                    0
                }
                .min(255) as u8
            }
            _ => 0,
        }
    };

    let mut color_out = dxt1_block(&block[8..16]);
    for i in 0..16 {
        let alpha_idx = ((alpha_codes >> (i * 3)) & 7) as u8;
        color_out[i][3] = interpolate_alpha(alpha_idx);
    }
    color_out
}

// ---- DXT surface decoders ---------------------------------------------

fn decode_dxt_surface(data: &[u8], w: u32, h: u32, dxt: DxtType) -> Result<Vec<u8>, DecodeError> {
    let bw = ((w + 3) / 4).max(1) as usize;
    let bh = ((h + 3) / 4).max(1) as usize;
    let block_bytes: usize = match dxt {
        DxtType::Dxt1 => 8,
        DxtType::Dxt3 | DxtType::Dxt5 => 16,
    };
    let needed = bw * bh * block_bytes;
    if data.len() < needed {
        return Err(DecodeError::BufferTooSmall {
            need: needed,
            have: data.len(),
        });
    }

    let mut rgba = vec![0u8; (w * h * 4) as usize];

    for by in 0..bh {
        for bx in 0..bw {
            let src_offset = (by * bw + bx) * block_bytes;
            let block_px = match dxt {
                DxtType::Dxt1 => dxt1_block(&data[src_offset..src_offset + 8]),
                DxtType::Dxt3 => dxt3_block(&data[src_offset..src_offset + 16]),
                DxtType::Dxt5 => dxt5_block(&data[src_offset..src_offset + 16]),
            };
            for row in 0..4 {
                for col in 0..4 {
                    let img_y = by * 4 + row;
                    let img_x = bx * 4 + col;
                    if img_y >= h as usize || img_x >= w as usize {
                        continue;
                    }
                    let px = block_px[row * 4 + col];
                    let dst = (img_y * w as usize + img_x) * 4;
                    rgba[dst..dst + 4].copy_from_slice(&px);
                }
            }
        }
    }

    Ok(rgba)
}

// ---- Uncompressed format decoders --------------------------------------

fn decode_1555(data: &[u8], w: u32, h: u32) -> Result<Vec<u8>, DecodeError> {
    let pixel_count = (w * h) as usize;
    let needed = pixel_count * 2;
    if data.len() < needed {
        return Err(DecodeError::BufferTooSmall {
            need: needed,
            have: data.len(),
        });
    }
    let mut rgba = vec![0u8; pixel_count * 4];
    for i in 0..pixel_count {
        let pixel = u16::from_le_bytes([data[i * 2], data[i * 2 + 1]]);
        let a = ((pixel >> 15) & 1) as u8 * 255;
        let r5 = ((pixel >> 10) & 0x1F) as u8;
        let g5 = ((pixel >> 5) & 0x1F) as u8;
        let b5 = (pixel & 0x1F) as u8;
        rgba[i * 4] = (r5 << 3) | (r5 >> 2);
        rgba[i * 4 + 1] = (g5 << 3) | (g5 >> 2);
        rgba[i * 4 + 2] = (b5 << 3) | (b5 >> 2);
        rgba[i * 4 + 3] = a;
    }
    Ok(rgba)
}

fn decode_565(data: &[u8], w: u32, h: u32) -> Result<Vec<u8>, DecodeError> {
    let pixel_count = (w * h) as usize;
    let needed = pixel_count * 2;
    if data.len() < needed {
        return Err(DecodeError::BufferTooSmall {
            need: needed,
            have: data.len(),
        });
    }
    let mut rgba = vec![0u8; pixel_count * 4];
    for i in 0..pixel_count {
        let pixel = u16::from_le_bytes([data[i * 2], data[i * 2 + 1]]);
        let r5 = ((pixel >> 11) & 0x1F) as u8;
        let g6 = ((pixel >> 5) & 0x3F) as u8;
        let b5 = (pixel & 0x1F) as u8;
        rgba[i * 4] = (r5 << 3) | (r5 >> 2);
        rgba[i * 4 + 1] = (g6 << 2) | (g6 >> 4);
        rgba[i * 4 + 2] = (b5 << 3) | (b5 >> 2);
        rgba[i * 4 + 3] = 255;
    }
    Ok(rgba)
}

fn decode_4444(data: &[u8], w: u32, h: u32) -> Result<Vec<u8>, DecodeError> {
    let pixel_count = (w * h) as usize;
    let needed = pixel_count * 2;
    if data.len() < needed {
        return Err(DecodeError::BufferTooSmall {
            need: needed,
            have: data.len(),
        });
    }
    let mut rgba = vec![0u8; pixel_count * 4];
    for i in 0..pixel_count {
        let pixel = u16::from_le_bytes([data[i * 2], data[i * 2 + 1]]);
        let r4 = ((pixel >> 12) & 0x0F) as u8;
        let g4 = ((pixel >> 8) & 0x0F) as u8;
        let b4 = ((pixel >> 4) & 0x0F) as u8;
        let a4 = (pixel & 0x0F) as u8;
        rgba[i * 4] = (r4 << 4) | r4;
        rgba[i * 4 + 1] = (g4 << 4) | g4;
        rgba[i * 4 + 2] = (b4 << 4) | b4;
        rgba[i * 4 + 3] = (a4 << 4) | a4;
    }
    Ok(rgba)
}

fn decode_8888(data: &[u8], w: u32, h: u32) -> Result<Vec<u8>, DecodeError> {
    let pixel_count = (w * h) as usize;
    let needed = pixel_count * 4;
    if data.len() < needed {
        return Err(DecodeError::BufferTooSmall {
            need: needed,
            have: data.len(),
        });
    }
    // RW stores BGRA32 natively on PC (little-endian ARGB in memory).
    // Convert ARGB (A B G R in LE u32) → RGBA.
    let mut rgba = vec![0u8; pixel_count * 4];
    for i in 0..pixel_count {
        let a = data[i * 4 + 3];
        let r = data[i * 4 + 2];
        let g = data[i * 4 + 1];
        let b = data[i * 4];
        rgba[i * 4] = r;
        rgba[i * 4 + 1] = g;
        rgba[i * 4 + 2] = b;
        rgba[i * 4 + 3] = a;
    }
    Ok(rgba)
}

fn decode_888(data: &[u8], w: u32, h: u32) -> Result<Vec<u8>, DecodeError> {
    let pixel_count = (w * h) as usize;
    let needed = pixel_count * 3;
    if data.len() < needed {
        return Err(DecodeError::BufferTooSmall {
            need: needed,
            have: data.len(),
        });
    }
    let mut rgba = vec![0u8; pixel_count * 4];
    for i in 0..pixel_count {
        // RW stores BGR24 on PC.
        rgba[i * 4] = data[i * 3 + 2]; // R
        rgba[i * 4 + 1] = data[i * 3 + 1]; // G
        rgba[i * 4 + 2] = data[i * 3]; // B
        rgba[i * 4 + 3] = 255;
    }
    Ok(rgba)
}

fn decode_555(data: &[u8], w: u32, h: u32) -> Result<Vec<u8>, DecodeError> {
    let pixel_count = (w * h) as usize;
    let needed = pixel_count * 2;
    if data.len() < needed {
        return Err(DecodeError::BufferTooSmall {
            need: needed,
            have: data.len(),
        });
    }
    let mut rgba = vec![0u8; pixel_count * 4];
    for i in 0..pixel_count {
        let pixel = u16::from_le_bytes([data[i * 2], data[i * 2 + 1]]);
        let r5 = ((pixel >> 10) & 0x1F) as u8;
        let g5 = ((pixel >> 5) & 0x1F) as u8;
        let b5 = (pixel & 0x1F) as u8;
        rgba[i * 4] = (r5 << 3) | (r5 >> 2);
        rgba[i * 4 + 1] = (g5 << 3) | (g5 >> 2);
        rgba[i * 4 + 2] = (b5 << 3) | (b5 >> 2);
        rgba[i * 4 + 3] = 255;
    }
    Ok(rgba)
}

fn decode_lum8(data: &[u8], w: u32, h: u32) -> Result<Vec<u8>, DecodeError> {
    let pixel_count = (w * h) as usize;
    let needed = pixel_count;
    if data.len() < needed {
        return Err(DecodeError::BufferTooSmall {
            need: needed,
            have: data.len(),
        });
    }
    let mut rgba = vec![0u8; pixel_count * 4];
    for i in 0..pixel_count {
        let l = data[i];
        rgba[i * 4] = l;
        rgba[i * 4 + 1] = l;
        rgba[i * 4 + 2] = l;
        rgba[i * 4 + 3] = 255;
    }
    Ok(rgba)
}

fn decode_pal4(data: &[u8], palette: &[u8], w: u32, h: u32) -> Result<Vec<u8>, DecodeError> {
    let pixel_count = (w * h) as usize;
    let data_needed = (pixel_count + 1) / 2;
    if data.len() < data_needed {
        return Err(DecodeError::BufferTooSmall {
            need: data_needed,
            have: data.len(),
        });
    }
    if palette.len() < 64 {
        // 16 entries × 4 bytes
        return Err(DecodeError::BufferTooSmall {
            need: 64,
            have: palette.len(),
        });
    }
    let mut rgba = vec![0u8; pixel_count * 4];
    for i in 0..pixel_count {
        let nibble = if i % 2 == 0 {
            data[i / 2] & 0x0F
        } else {
            (data[i / 2] >> 4) & 0x0F
        } as usize;
        let b = palette[nibble * 4];
        let g = palette[nibble * 4 + 1];
        let r = palette[nibble * 4 + 2];
        let a = palette[nibble * 4 + 3];
        rgba[i * 4] = r;
        rgba[i * 4 + 1] = g;
        rgba[i * 4 + 2] = b;
        rgba[i * 4 + 3] = a;
    }
    Ok(rgba)
}

fn decode_pal8(data: &[u8], palette: &[u8], w: u32, h: u32) -> Result<Vec<u8>, DecodeError> {
    let pixel_count = (w * h) as usize;
    let needed = pixel_count;
    if data.len() < needed {
        return Err(DecodeError::BufferTooSmall {
            need: needed,
            have: data.len(),
        });
    }
    if palette.len() < 1024 {
        // 256 entries × 4 bytes
        return Err(DecodeError::BufferTooSmall {
            need: 1024,
            have: palette.len(),
        });
    }
    let mut rgba = vec![0u8; pixel_count * 4];
    for i in 0..pixel_count {
        let idx = data[i] as usize;
        let b = palette[idx * 4];
        let g = palette[idx * 4 + 1];
        let r = palette[idx * 4 + 2];
        let a = palette[idx * 4 + 3];
        rgba[i * 4] = r;
        rgba[i * 4 + 1] = g;
        rgba[i * 4 + 2] = b;
        rgba[i * 4 + 3] = a;
    }
    Ok(rgba)
}

// ---- Public API --------------------------------------------------------

/// TXD raster format flags (lower bits = base format, upper bits = extensions).
pub mod format {
    pub const FORMAT_1555: u32 = 0x100;
    pub const FORMAT_565: u32 = 0x200;
    pub const FORMAT_4444: u32 = 0x300;
    pub const FORMAT_LUM8: u32 = 0x400;
    pub const FORMAT_8888: u32 = 0x500;
    pub const FORMAT_888: u32 = 0x600;
    pub const FORMAT_555: u32 = 0xA00;
    pub const EXT_PAL8: u32 = 0x2000;
    pub const EXT_PAL4: u32 = 0x4000;
    pub const EXT_MIPMAP: u32 = 0x8000;

    pub fn base_format(raster_format: u32) -> u32 {
        raster_format & 0xFFF
    }

    pub fn has_palette(raster_format: u32) -> bool {
        (raster_format & 0x6000) != 0
    }

    pub fn mipmap_count(raster_format: u32) -> u32 {
        if (raster_format & EXT_MIPMAP) != 0 { 0 } else { 1 }
    }

    pub fn format_name(raster_format: u32) -> &'static str {
        match base_format(raster_format) {
            FORMAT_1555 => "1555 ARGB",
            FORMAT_565 => "565 RGB",
            FORMAT_4444 => "4444 ARGB",
            FORMAT_LUM8 => "LUM8",
            FORMAT_8888 => "8888 ARGB",
            FORMAT_888 => "888 RGB",
            FORMAT_555 => "555 XRGB",
            _ => "Unknown",
        }
    }

    pub fn is_dxt(raster_format: u32) -> bool {
        matches!(base_format(raster_format), 0x100..=0x300)
    }
}

/// Decoded texture result.
#[derive(Debug, Clone)]
pub struct DecodedTexture {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub has_alpha: bool,
    pub format_name: String,
    pub mipmap_count: u32,
}

/// Decode raster data to RGBA given the TXD raster format.
///
/// `data` is the raw mipmap pixel data (after any platform-specific header).
/// `palette` is the 32-bit BGRA palette bytes (for PAL4/PAL8).
/// `raster_type` is the RW raster type field (0x12 = DXT compressed).
pub fn decode_raster(
    data: &[u8],
    width: u32,
    height: u32,
    raster_format: u32,
    palette: &[u8],
    raster_type: u8,
) -> Result<Vec<u8>, DecodeError> {
    let base = format::base_format(raster_format);
    let is_pal4 = (raster_format & format::EXT_PAL4) != 0;
    let is_pal8 = (raster_format & format::EXT_PAL8) != 0;
    let is_dxt = raster_type == 0x12;

    // For DXT formats, skip header bytes if present.
    let data = if is_dxt && data.len() > 4 {
        &data[4..]
    } else {
        data
    };

    if is_pal4 {
        return decode_pal4(data, palette, width, height);
    }
    if is_pal8 {
        return decode_pal8(data, palette, width, height);
    }

    if is_dxt {
        match base {
            0x100 => decode_dxt_surface(data, width, height, DxtType::Dxt1),
            0x200 => decode_dxt_surface(data, width, height, DxtType::Dxt3),
            0x300 => decode_dxt_surface(data, width, height, DxtType::Dxt5),
            _ => decode_dxt_surface(data, width, height, DxtType::Dxt1),
        }
    } else {
        match base {
            format::FORMAT_1555 => decode_1555(data, width, height),
            format::FORMAT_565 => decode_565(data, width, height),
            format::FORMAT_4444 => decode_4444(data, width, height),
            format::FORMAT_LUM8 => decode_lum8(data, width, height),
            format::FORMAT_8888 => decode_8888(data, width, height),
            format::FORMAT_888 => decode_888(data, width, height),
            format::FORMAT_555 => decode_555(data, width, height),
            _ => Err(DecodeError::UnsupportedFormat(base)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_1555_works() {
        // 2x2 image in 1555 format with known pixels
        let w = 2;
        let h = 2;
        // White (255,255,255,255) = 1|11111|11111|11111 = 0xFFFF
        // Black (0,0,0,255)       = 1|00000|00000|00000 = 0x8000
        let data: Vec<u8> = vec![
            0xFF, 0xFF, // pixel 0: white
            0x00, 0x80, // pixel 1: black
            0xFF, 0xFF, // pixel 2: white
            0x00, 0x80, // pixel 3: black
        ];
        let rgba = decode_1555(&data, w, h).unwrap();
        // pixel 0
        assert_eq!(rgba[0], 255); // R
        assert_eq!(rgba[1], 255); // G
        assert_eq!(rgba[2], 255); // B
        assert_eq!(rgba[3], 255); // A
        // pixel 1
        assert_eq!(rgba[4], 0);
        assert_eq!(rgba[5], 0);
        assert_eq!(rgba[6], 0);
        assert_eq!(rgba[7], 255);
    }

    #[test]
    fn decode_565_works() {
        let w = 1;
        let h = 1;
        // Red in 565: R=31, G=0, B=0 → 0xF800
        let data = vec![0x00, 0xF8];
        let rgba = decode_565(&data, w, h).unwrap();
        assert_eq!(rgba[0], 255); // R
        assert_eq!(rgba[1], 0);
        assert_eq!(rgba[2], 0);
        assert_eq!(rgba[3], 255);
    }

    #[test]
    fn decode_4444_works() {
        let w = 1;
        let h = 1;
        // 4444 ARGB in LE: bytes [0x8F, 0xFF] → u16 0xFF8F
        // decoder: r=bits[15:12], g=bits[11:8], b=bits[7:4], a=bits[3:0]
        // r=0xF→255, g=0xF→255, b=0x8→136, a=0xF→255
        let data = vec![0x8F, 0xFF];
        let rgba = decode_4444(&data, w, h).unwrap();
        assert_eq!(rgba[0], 255);
        assert_eq!(rgba[1], 255);
        assert_eq!(rgba[2], 136);
        assert_eq!(rgba[3], 255);
    }
}
