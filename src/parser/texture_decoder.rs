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
    Dxt2,
    Dxt3,
    Dxt4,
    Dxt5,
}

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("buffer too small: need {need} bytes, have {have}")]
    BufferTooSmall { need: usize, have: usize },
    #[error("invalid texture dimensions: {width}x{height}")]
    InvalidDimensions { width: u32, height: u32 },
    #[error("unsupported format: 0x{0:03X}")]
    UnsupportedFormat(u32),
}

const MAX_DECODE_DIMENSION: u32 = 8_192;

fn checked_pixel_count(width: u32, height: u32) -> Result<usize, DecodeError> {
    if width == 0 || height == 0 || width > MAX_DECODE_DIMENSION || height > MAX_DECODE_DIMENSION {
        return Err(DecodeError::InvalidDimensions { width, height });
    }
    (width as usize)
        .checked_mul(height as usize)
        .ok_or(DecodeError::InvalidDimensions { width, height })
}

// ---- DXT block decoders ------------------------------------------------

fn dxt_color_block(block: &[u8], allow_transparent: bool) -> [[u8; 4]; 16] {
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
    for (i, pixel) in out.iter_mut().enumerate() {
        let idx = ((codes >> (i * 2)) & 3) as u8;
        *pixel = match (c0 > c1 || !allow_transparent, idx) {
            (true, 0) | (false, 0) => col0,
            (true, 1) | (false, 1) => col1,
            (true, 2) => {
                let r = ((col0[0] as u16 * 2 + col1[0] as u16) / 3) as u8;
                let g = ((col0[1] as u16 * 2 + col1[1] as u16) / 3) as u8;
                let b = ((col0[2] as u16 * 2 + col1[2] as u16) / 3) as u8;
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
            (true, 3) => {
                let r = ((col0[0] as u16 + col1[0] as u16 * 2) / 3) as u8;
                let g = ((col0[1] as u16 + col1[1] as u16 * 2) / 3) as u8;
                let b = ((col0[2] as u16 + col1[2] as u16 * 2) / 3) as u8;
                [r, g, b, 255]
            }
            (false, 3) => [0, 0, 0, 0],
            _ => unreachable!("DXT color selector is only two bits"),
        };
    }
    out
}

fn dxt1_block(block: &[u8]) -> [[u8; 4]; 16] {
    dxt_color_block(block, true)
}

fn dxt3_block(block: &[u8]) -> [[u8; 4]; 16] {
    // First 8 bytes: explicit 4-bit alpha per texel
    let mut out = dxt_color_block(&block[8..16], false);
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
        // The `1 *` and `0 *` coefficients below preserve the parallel
        // structure of the DXT5 alpha interpolation table; they are
        // load-bearing for readability even when the math collapses.
        #[allow(clippy::identity_op, clippy::erasing_op)]
        match idx {
            0 => alpha0,
            1 => alpha1,
            2 => if alpha0 > alpha1 {
                (6 * alpha0 as u16 + 1 * alpha1 as u16 + 3) / 7
            } else {
                (4 * alpha0 as u16 + 1 * alpha1 as u16 + 2) / 5
            }
            .min(255) as u8,
            3 => if alpha0 > alpha1 {
                (5 * alpha0 as u16 + 2 * alpha1 as u16 + 3) / 7
            } else {
                (3 * alpha0 as u16 + 2 * alpha1 as u16 + 2) / 5
            }
            .min(255) as u8,
            4 => if alpha0 > alpha1 {
                (4 * alpha0 as u16 + 3 * alpha1 as u16 + 3) / 7
            } else {
                (2 * alpha0 as u16 + 3 * alpha1 as u16 + 2) / 5
            }
            .min(255) as u8,
            5 => if alpha0 > alpha1 {
                (3 * alpha0 as u16 + 4 * alpha1 as u16 + 3) / 7
            } else {
                (1 * alpha0 as u16 + 4 * alpha1 as u16 + 2) / 5
            }
            .min(255) as u8,
            6 => if alpha0 > alpha1 {
                (2 * alpha0 as u16 + 5 * alpha1 as u16 + 3) / 7
            } else {
                (0 * alpha0 as u16 + 5 * alpha1 as u16 + 2) / 5
            }
            .min(255) as u8,
            7 => if alpha0 > alpha1 {
                (1 * alpha0 as u16 + 6 * alpha1 as u16 + 3) / 7
            } else {
                0
            }
            .min(255) as u8,
            _ => 0,
        }
    };

    let mut color_out = dxt_color_block(&block[8..16], false);
    for (i, pixel) in color_out.iter_mut().enumerate() {
        let alpha_idx = ((alpha_codes >> (i * 3)) & 7) as u8;
        pixel[3] = interpolate_alpha(alpha_idx);
    }
    color_out
}

// ---- DXT surface decoders ---------------------------------------------

fn decode_dxt_surface(data: &[u8], w: u32, h: u32, dxt: DxtType) -> Result<Vec<u8>, DecodeError> {
    let pixel_count = checked_pixel_count(w, h)?;
    let bw = w.div_ceil(4).max(1) as usize;
    let bh = h.div_ceil(4).max(1) as usize;
    let block_bytes: usize = match dxt {
        DxtType::Dxt1 => 8,
        DxtType::Dxt2 | DxtType::Dxt3 | DxtType::Dxt4 | DxtType::Dxt5 => 16,
    };
    let needed = bw
        .checked_mul(bh)
        .and_then(|blocks| blocks.checked_mul(block_bytes))
        .ok_or(DecodeError::InvalidDimensions {
            width: w,
            height: h,
        })?;
    if data.len() < needed {
        return Err(DecodeError::BufferTooSmall {
            need: needed,
            have: data.len(),
        });
    }

    let mut rgba = vec![
        0u8;
        pixel_count
            .checked_mul(4)
            .ok_or(DecodeError::InvalidDimensions {
                width: w,
                height: h
            })?
    ];

    for by in 0..bh {
        for bx in 0..bw {
            let src_offset = (by * bw + bx) * block_bytes;
            let block_px = match dxt {
                DxtType::Dxt1 => dxt1_block(&data[src_offset..src_offset + 8]),
                DxtType::Dxt2 | DxtType::Dxt3 => dxt3_block(&data[src_offset..src_offset + 16]),
                DxtType::Dxt4 | DxtType::Dxt5 => dxt5_block(&data[src_offset..src_offset + 16]),
            };
            for row in 0..4 {
                for col in 0..4 {
                    let img_y = by * 4 + row;
                    let img_x = bx * 4 + col;
                    if img_y >= h as usize || img_x >= w as usize {
                        continue;
                    }
                    let mut px = block_px[row * 4 + col];
                    if matches!(dxt, DxtType::Dxt2 | DxtType::Dxt4) && px[3] != 0 {
                        // DXT2/DXT4 store premultiplied color. The viewer's
                        // RGBA surface is straight-alpha, so restore the
                        // color channels for correct previews.
                        px[0] = ((u16::from(px[0]) * 255) / u16::from(px[3])).min(255) as u8;
                        px[1] = ((u16::from(px[1]) * 255) / u16::from(px[3])).min(255) as u8;
                        px[2] = ((u16::from(px[2]) * 255) / u16::from(px[3])).min(255) as u8;
                    }
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
    let pixel_count = checked_pixel_count(w, h)?;
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
    let pixel_count = checked_pixel_count(w, h)?;
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
    let pixel_count = checked_pixel_count(w, h)?;
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
    let pixel_count = checked_pixel_count(w, h)?;
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
    let pixel_count = checked_pixel_count(w, h)?;
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

/// D3DFMT_X8R8G8B8: 32-bit RGBX. The X byte is undefined; real rasters
/// fill it with 0xFF, but forcing opaque alpha is the format-correct
/// reading and keeps "Alpha: No" claims true regardless of the X bytes.
fn decode_x8r8g8b8(data: &[u8], w: u32, h: u32) -> Result<Vec<u8>, DecodeError> {
    let pixel_count = checked_pixel_count(w, h)?;
    let needed = pixel_count * 4;
    if data.len() < needed {
        return Err(DecodeError::BufferTooSmall {
            need: needed,
            have: data.len(),
        });
    }
    let mut rgba = vec![0u8; pixel_count * 4];
    for i in 0..pixel_count {
        rgba[i * 4] = data[i * 4 + 2]; // R
        rgba[i * 4 + 1] = data[i * 4 + 1]; // G
        rgba[i * 4 + 2] = data[i * 4]; // B
        rgba[i * 4 + 3] = 255;
    }
    Ok(rgba)
}

fn decode_555(data: &[u8], w: u32, h: u32) -> Result<Vec<u8>, DecodeError> {
    let pixel_count = checked_pixel_count(w, h)?;
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
    let pixel_count = checked_pixel_count(w, h)?;
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

fn decode_a8l8(data: &[u8], w: u32, h: u32) -> Result<Vec<u8>, DecodeError> {
    let pixel_count = checked_pixel_count(w, h)?;
    let needed = pixel_count * 2;
    if data.len() < needed {
        return Err(DecodeError::BufferTooSmall {
            need: needed,
            have: data.len(),
        });
    }
    let mut rgba = vec![0u8; pixel_count * 4];
    for i in 0..pixel_count {
        let l = data[i * 2];
        let a = data[i * 2 + 1];
        rgba[i * 4] = l;
        rgba[i * 4 + 1] = l;
        rgba[i * 4 + 2] = l;
        rgba[i * 4 + 3] = a;
    }
    Ok(rgba)
}

fn decode_pal4(data: &[u8], palette: &[u8], w: u32, h: u32) -> Result<Vec<u8>, DecodeError> {
    let pixel_count = checked_pixel_count(w, h)?;
    let data_needed = pixel_count.div_ceil(2);
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
            (data[i / 2] >> 4) & 0x0F
        } else {
            data[i / 2] & 0x0F
        } as usize;
        // RenderWare palettes are stored as RGBA entries. This differs from
        // the BGRA byte order used by uncompressed PC rasters.
        rgba[i * 4..i * 4 + 4].copy_from_slice(&palette[nibble * 4..nibble * 4 + 4]);
    }
    Ok(rgba)
}

fn decode_pal8(data: &[u8], palette: &[u8], w: u32, h: u32) -> Result<Vec<u8>, DecodeError> {
    let pixel_count = checked_pixel_count(w, h)?;
    let needed = pixel_count;
    if data.len() < needed {
        return Err(DecodeError::BufferTooSmall {
            need: needed,
            have: data.len(),
        });
    }
    let palette_entries = palette.len() / 4;
    if palette_entries == 0 {
        return Err(DecodeError::BufferTooSmall {
            need: 4,
            have: palette.len(),
        });
    }
    let mut rgba = vec![0u8; pixel_count * 4];
    for i in 0..pixel_count {
        let idx = data[i] as usize;
        if idx >= palette_entries {
            return Err(DecodeError::BufferTooSmall {
                need: (idx + 1) * 4,
                have: palette.len(),
            });
        }
        rgba[i * 4..i * 4 + 4].copy_from_slice(&palette[idx * 4..idx * 4 + 4]);
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

    pub const RASTER_TYPE_1555: u32 = 0x01;
    pub const RASTER_TYPE_565: u32 = 0x02;
    pub const RASTER_TYPE_4444: u32 = 0x03;
    pub const RASTER_TYPE_LUM8: u32 = 0x04;
    pub const RASTER_TYPE_8888: u32 = 0x05;
    pub const RASTER_TYPE_888: u32 = 0x06;
    pub const RASTER_TYPE_555: u32 = 0x0A;

    pub fn base_format(raster_format: u32) -> u32 {
        raster_format & 0xFFF
    }

    pub fn has_palette(raster_format: u32) -> bool {
        (raster_format & 0x6000) != 0
    }

    pub fn mipmap_count(raster_format: u32) -> u32 {
        if (raster_format & EXT_MIPMAP) != 0 {
            0
        } else {
            1
        }
    }

    pub fn format_name(raster_format: u32) -> &'static str {
        match base_format(raster_format) {
            FORMAT_1555 | RASTER_TYPE_1555 => "1555 ARGB",
            FORMAT_565 | RASTER_TYPE_565 => "565 RGB",
            FORMAT_4444 | RASTER_TYPE_4444 => "4444 ARGB",
            FORMAT_LUM8 | RASTER_TYPE_LUM8 => "LUM8",
            FORMAT_8888 | RASTER_TYPE_8888 => "8888 ARGB",
            FORMAT_888 | RASTER_TYPE_888 => "888 RGB",
            FORMAT_555 | RASTER_TYPE_555 => "555 XRGB",
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
    /// Lazily-built Iced image handle. `OnceLock` gives thread-safe one-time
    /// initialization without locking on reads, and keeps `DecodedTexture`
    /// `Send + Sync` for the parallel export path.
    pub handle: std::sync::OnceLock<iced::widget::image::Handle>,
}

/// Decode raster data to RGBA given the TXD raster format.
///
/// `data` is the raw mipmap pixel data (after any platform-specific header).
/// `palette` is the 32-bit RGBA palette bytes (for PAL4/PAL8).
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
            format::FORMAT_888 => {
                // The legacy path has no depth byte to consult. Tight rows
                // of true 24-bit data are exactly 3 bytes/px; anything with
                // 4 bytes/px or more is an X8R8G8B8-style 32-bit storage.
                let pixel_count = checked_pixel_count(width, height)?;
                if data.len() >= pixel_count * 4 {
                    decode_x8r8g8b8(data, width, height)
                } else {
                    decode_888(data, width, height)
                }
            }
            format::FORMAT_555 => decode_555(data, width, height),
            _ => Err(DecodeError::UnsupportedFormat(base)),
        }
    }
}

const PLATFORM_D3D8: u32 = 8;
const PLATFORM_D3D9: u32 = 9;
const D3D_8888: u32 = 21;
/// D3DFMT_R8G8B8: true 24-bit RGB. Practically unrenderable on D3D9
/// hardware, so almost no raster uses it — see [`D3D_X8R8G8B8`].
const D3D_R8G8B8: u32 = 20;
/// D3DFMT_X8R8G8B8: 32-bit RGBX. This is what RenderWare stores for "888"
/// rasters on the D3D9 platform, because D3D9 has no practical 24-bit
/// texture format. The X byte is undefined by the format and filled with
/// 0xFF by real assets; treating it as 24-bit misreads every pixel.
const D3D_X8R8G8B8: u32 = 22;
const D3D_565: u32 = 23;
const D3D_555: u32 = 24;
const D3D_1555: u32 = 25;
const D3D_4444: u32 = 26;
const D3D_L8: u32 = 50;
const D3D_A8L8: u32 = 51;
const D3D_DXT1: u32 = 0x3154_5844;
const D3D_DXT2: u32 = 0x3254_5844;
const D3D_DXT3: u32 = 0x3354_5844;
const D3D_DXT4: u32 = 0x3454_5844;
const D3D_DXT5: u32 = 0x3554_5844;

/// Format descriptors for one PC RenderWare Texture Native, as stored
/// on a `NativeTexture`. Groups the nine scalar/binary inputs of
/// [`decode_native_raster`] so call sites read by field name.
#[derive(Clone, Copy)]
pub struct RasterDescriptor<'a> {
    pub width: u32,
    pub height: u32,
    pub depth: u8,
    pub raster_format: u32,
    pub palette: &'a [u8],
    pub platform_id: u32,
    pub d3d_format: u32,
    pub platform_properties: u8,
    pub raster_type: u8,
}

/// Decode one PC RenderWare Texture Native mip level.
///
/// Unlike decode_raster, this function receives pixels after the native
/// mip-length prefix has already been removed. D3D9 selects the format with
/// its D3D format/FourCC, while D3D8 stores the DXT selector in the platform
/// properties byte. Raster flags remain the fallback for uncompressed data.
pub fn decode_native_raster(
    data: &[u8],
    desc: &RasterDescriptor<'_>,
) -> Result<Vec<u8>, DecodeError> {
    let RasterDescriptor {
        width,
        height,
        depth,
        raster_format,
        palette,
        platform_id,
        d3d_format,
        platform_properties,
        raster_type,
    } = *desc;
    let palette_type = (raster_format >> 13) & 0x3;
    if palette_type == 1 {
        return decode_pal8(data, palette, width, height);
    }
    if palette_type == 2 || palette_type == 3 {
        if depth == 4 {
            return decode_pal4(data, palette, width, height);
        }
        return decode_pal8(data, palette, width, height);
    }

    if let Some(dxt) = native_dxt_type(
        raster_format,
        platform_id,
        d3d_format,
        platform_properties,
        raster_type,
    ) {
        return decode_dxt_surface(data, width, height, dxt);
    }

    let raster = raster_type_code(raster_format);
    match (platform_id, d3d_format) {
        (PLATFORM_D3D9, D3D_8888) => decode_8888(data, width, height),
        (PLATFORM_D3D9, D3D_X8R8G8B8) => decode_x8r8g8b8(data, width, height),
        (PLATFORM_D3D9, D3D_R8G8B8) => decode_888(data, width, height),
        (PLATFORM_D3D9, D3D_565) => decode_565(data, width, height),
        (PLATFORM_D3D9, D3D_555) => decode_555(data, width, height),
        (PLATFORM_D3D9, D3D_1555) => decode_1555(data, width, height),
        (PLATFORM_D3D9, D3D_4444) => decode_4444(data, width, height),
        (PLATFORM_D3D9, D3D_L8) => decode_lum8(data, width, height),
        (PLATFORM_D3D9, D3D_A8L8) => decode_a8l8(data, width, height),
        // A raster flagged "888" whose depth byte says 32 is a D3D9-style
        // conversion stored as X8R8G8B8: honor the pixel width over the
        // legacy nibble.
        _ if raster == format::RASTER_TYPE_888 && depth == 32 => {
            decode_x8r8g8b8(data, width, height)
        }
        _ => match raster {
            format::RASTER_TYPE_1555 => decode_1555(data, width, height),
            format::RASTER_TYPE_565 => decode_565(data, width, height),
            format::RASTER_TYPE_4444 => decode_4444(data, width, height),
            format::RASTER_TYPE_LUM8 => decode_lum8(data, width, height),
            format::RASTER_TYPE_8888 => decode_8888(data, width, height),
            format::RASTER_TYPE_888 => decode_888(data, width, height),
            format::RASTER_TYPE_555 => decode_555(data, width, height),
            _ => Err(DecodeError::UnsupportedFormat(raster_format)),
        },
    }
}

pub fn native_dxt_type(
    raster_format: u32,
    platform_id: u32,
    d3d_format: u32,
    platform_properties: u8,
    raster_type: u8,
) -> Option<DxtType> {
    if platform_id == PLATFORM_D3D8 {
        let dxt = match platform_properties {
            1 => Some(DxtType::Dxt1),
            2 => Some(DxtType::Dxt2),
            3 => Some(DxtType::Dxt3),
            4 => Some(DxtType::Dxt4),
            5 => Some(DxtType::Dxt5),
            _ => None,
        };
        if dxt.is_some() {
            return dxt;
        }
    }

    if platform_id == PLATFORM_D3D9 {
        let dxt = match d3d_format {
            D3D_DXT1 => Some(DxtType::Dxt1),
            D3D_DXT2 => Some(DxtType::Dxt2),
            D3D_DXT3 => Some(DxtType::Dxt3),
            D3D_DXT4 => Some(DxtType::Dxt4),
            D3D_DXT5 => Some(DxtType::Dxt5),
            _ => None,
        };
        if dxt.is_some() {
            return dxt;
        }
    }

    // Keep compatibility with older callers that only supplied the legacy
    // raster type marker and raster-format base.
    if raster_type == 0x12 {
        return match format::base_format(raster_format) {
            format::FORMAT_1555 => Some(DxtType::Dxt1),
            format::FORMAT_565 => Some(DxtType::Dxt3),
            format::FORMAT_4444 => Some(DxtType::Dxt5),
            _ => Some(DxtType::Dxt1),
        };
    }
    None
}

pub fn native_is_dxt(
    raster_format: u32,
    platform_id: u32,
    d3d_format: u32,
    platform_properties: u8,
    raster_type: u8,
) -> bool {
    native_dxt_type(
        raster_format,
        platform_id,
        d3d_format,
        platform_properties,
        raster_type,
    )
    .is_some()
}

pub fn native_has_alpha(
    raster_format: u32,
    platform_id: u32,
    d3d_format: u32,
    platform_properties: u8,
    raster_type: u8,
) -> bool {
    if platform_id == PLATFORM_D3D9 {
        return platform_properties & 0x01 != 0
            || matches!(
                d3d_format,
                D3D_DXT2 | D3D_DXT3 | D3D_DXT4 | D3D_DXT5 | D3D_1555 | D3D_4444 | D3D_A8L8
            );
    }
    if platform_id == PLATFORM_D3D8 {
        return match platform_properties {
            2..=5 => true,
            1 => raster_type_code(raster_format) == format::RASTER_TYPE_1555,
            _ => matches!(
                raster_type_code(raster_format),
                format::RASTER_TYPE_1555 | format::RASTER_TYPE_4444 | format::RASTER_TYPE_8888
            ),
        };
    }
    matches!(
        raster_type_code(raster_format),
        format::RASTER_TYPE_1555 | format::RASTER_TYPE_4444 | format::RASTER_TYPE_8888
    ) || raster_type == 0x12
}

pub fn native_format_name(
    raster_format: u32,
    platform_id: u32,
    d3d_format: u32,
    platform_properties: u8,
    raster_type: u8,
) -> &'static str {
    match (raster_format >> 13) & 0x3 {
        1 => return "PAL8",
        2 => return "PAL4",
        3 => return "PAL4 (LSB)",
        _ => {}
    }
    match d3d_format {
        D3D_8888 => "8888 ARGB",
        D3D_X8R8G8B8 => "X8R8G8B8 (888 RGB, 32bpp)",
        D3D_R8G8B8 => "R8G8B8 (888 RGB, 24bpp)",
        D3D_565 => "565 RGB",
        D3D_555 => "555 XRGB",
        D3D_1555 => "1555 ARGB",
        D3D_4444 => "4444 ARGB",
        D3D_L8 => "LUM8",
        D3D_A8L8 => "A8L8",
        D3D_DXT1 => "DXT1",
        D3D_DXT2 => "DXT2",
        D3D_DXT3 => "DXT3",
        D3D_DXT4 => "DXT4",
        D3D_DXT5 => "DXT5",
        _ => match native_dxt_type(
            raster_format,
            platform_id,
            d3d_format,
            platform_properties,
            raster_type,
        ) {
            Some(DxtType::Dxt1) => "DXT1",
            Some(DxtType::Dxt2) => "DXT2",
            Some(DxtType::Dxt3) => "DXT3",
            Some(DxtType::Dxt4) => "DXT4",
            Some(DxtType::Dxt5) => "DXT5",
            None => format::format_name(raster_type_code(raster_format)),
        },
    }
}

fn raster_type_code(raster_format: u32) -> u32 {
    let encoded = (raster_format >> 8) & 0x0F;
    if encoded != 0 {
        encoded
    } else {
        match format::base_format(raster_format) {
            format::FORMAT_1555 => format::RASTER_TYPE_1555,
            format::FORMAT_565 => format::RASTER_TYPE_565,
            format::FORMAT_4444 => format::RASTER_TYPE_4444,
            format::FORMAT_LUM8 => format::RASTER_TYPE_LUM8,
            format::FORMAT_8888 => format::RASTER_TYPE_8888,
            format::FORMAT_888 => format::RASTER_TYPE_888,
            format::FORMAT_555 => format::RASTER_TYPE_555,
            other => other,
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

    #[test]
    fn decode_pal4_uses_high_nibble_first_and_rgba_palette_order() {
        let palette = (0..16)
            .flat_map(|index| [index * 4, index * 4 + 1, index * 4 + 2, index * 4 + 3])
            .collect::<Vec<_>>();
        let rgba = decode_pal4(&[0x12], &palette, 2, 1).expect("PAL4 should decode");

        assert_eq!(&rgba[..4], &[4, 5, 6, 7]);
        assert_eq!(&rgba[4..8], &[8, 9, 10, 11]);
    }

    #[test]
    fn decode_pal8_accepts_bounded_palettes_and_rejects_missing_entries() {
        let palette = [10, 20, 30, 40, 50, 60, 70, 80];
        let rgba = decode_pal8(&[1], &palette, 1, 1).expect("PAL8 should decode");
        assert_eq!(rgba, vec![50, 60, 70, 80]);

        let error = decode_pal8(&[2], &palette, 1, 1).expect_err("missing entry should fail");
        assert!(matches!(
            error,
            DecodeError::BufferTooSmall { need: 12, have: 8 }
        ));
    }

    #[test]
    fn reject_unreasonable_decode_dimensions_before_allocation() {
        let error = decode_8888(&[], MAX_DECODE_DIMENSION + 1, 1)
            .expect_err("oversized textures should be rejected");
        assert!(matches!(
            error,
            DecodeError::InvalidDimensions { width, height }
                if width == MAX_DECODE_DIMENSION + 1 && height == 1
        ));
    }

    #[test]
    fn native_dxt_falls_back_to_legacy_marker_for_d3d8() {
        assert_eq!(
            native_dxt_type(format::FORMAT_1555, PLATFORM_D3D8, 0, 0, 0x12,),
            Some(DxtType::Dxt1)
        );
    }

    #[test]
    fn decode_x8r8g8b8_forces_the_undefined_alpha_byte_opaque() {
        // One pixel, X byte deliberately not 0xFF.
        let data = [0x8C, 0x64, 0x54, 0x7F];
        let rgba = decode_x8r8g8b8(&data, 1, 1).unwrap();
        assert_eq!(rgba, vec![0x54, 0x64, 0x8C, 0xFF]);
    }

    #[test]
    fn d3d9_format_22_is_x8r8g8b8_not_24bit_888() {
        // Regression: dwayne.txd-class rasters (RW raster format 0x600 =
        // "888", D3D9 format word 22 = D3DFMT_X8R8G8B8, depth 32) store
        // 4 bytes per pixel. Decoding them as 24-bit 888 garbles every
        // pixel with a progressive 1-byte misalignment.
        let data = [
            0x8C, 0x64, 0x54, 0xFF, // pixel 0: B=8C G=64 R=54
            0x74, 0x54, 0x44, 0xFF, // pixel 1
            0x74, 0x54, 0x3C, 0xFF, // pixel 2
            0x6C, 0x4C, 0x3C, 0xFF, // pixel 3
        ];
        let desc = RasterDescriptor {
            width: 2,
            height: 2,
            depth: 32,
            raster_format: format::FORMAT_888,
            palette: &[],
            platform_id: PLATFORM_D3D9,
            d3d_format: D3D_X8R8G8B8,
            platform_properties: 0,
            raster_type: format::RASTER_TYPE_888 as u8,
        };
        let rgba = decode_native_raster(&data, &desc).unwrap();
        assert_eq!(&rgba[..4], &[0x54, 0x64, 0x8C, 0xFF]);
        assert_eq!(&rgba[4..8], &[0x44, 0x54, 0x74, 0xFF]);
        assert_eq!(&rgba[8..12], &[0x3C, 0x54, 0x74, 0xFF]);
        assert_eq!(&rgba[12..16], &[0x3C, 0x4C, 0x6C, 0xFF]);
    }

    #[test]
    fn d3d9_format_20_is_true_24bit_r8g8b8() {
        let data = [0x8C, 0x64, 0x54, 0x74, 0x54, 0x44];
        let desc = RasterDescriptor {
            width: 2,
            height: 1,
            depth: 24,
            raster_format: format::FORMAT_888,
            palette: &[],
            platform_id: PLATFORM_D3D9,
            d3d_format: D3D_R8G8B8,
            platform_properties: 0,
            raster_type: format::RASTER_TYPE_888 as u8,
        };
        let rgba = decode_native_raster(&data, &desc).unwrap();
        assert_eq!(&rgba[..4], &[0x54, 0x64, 0x8C, 0xFF]);
        assert_eq!(&rgba[4..8], &[0x44, 0x54, 0x74, 0xFF]);
    }

    #[test]
    fn raster_888_with_depth_32_reads_32bpp_even_without_a_d3d_format() {
        // Mixed-import rasters can carry the legacy "888" nibble with a
        // stale/zero D3D format word; the depth byte then carries the
        // truth about the pixel width.
        let data = [
            0x8C, 0x64, 0x54, 0x00, // X byte junk must not shift the read
            0x74, 0x54, 0x44, 0x00,
        ];
        let desc = RasterDescriptor {
            width: 2,
            height: 1,
            depth: 32,
            raster_format: format::FORMAT_888,
            palette: &[],
            platform_id: PLATFORM_D3D9,
            d3d_format: 0,
            platform_properties: 0,
            raster_type: format::RASTER_TYPE_888 as u8,
        };
        let rgba = decode_native_raster(&data, &desc).unwrap();
        assert_eq!(&rgba[..4], &[0x54, 0x64, 0x8C, 0xFF]);
        assert_eq!(&rgba[4..8], &[0x44, 0x54, 0x74, 0xFF]);
    }
}
