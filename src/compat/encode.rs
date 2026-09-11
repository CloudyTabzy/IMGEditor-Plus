//! Texture encoders for the Phase B converter: RGBA pixels in, target
//! dialect bytes out.
//!
//! Two consumers share this module:
//! - texture replacement/new-TXD authoring, which encodes an imported
//!   image into one native texture, and
//! - the bulk converter, which re-encodes existing rasters.
//!
//! Every encoder produces the byte order our own decoder (and the
//! games, per the retail corpora) expect, and every format carries its
//! RenderWare header triple so the TXD writer never has to guess.
//! DXT compression uses texpresso (MIT); the INU Tools encoder was
//! consulted for facts only (GPL-3.0, no code reuse).

use crate::parser::texture_decoder::format;

/// Output formats the converter can write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodeFormat {
    Pal8,
    Pal4,
    Rgb565,
    Argb1555,
    Argb4444,
    /// RW "888" in the 32-bit X8R8G8B8 storage every PC target ships.
    Rgb888,
    Argb8888,
    Dxt1,
    Dxt3,
    Dxt5,
}

impl EncodeFormat {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pal8 => "PAL8",
            Self::Pal4 => "PAL4",
            Self::Rgb565 => "565 RGB",
            Self::Argb1555 => "1555 ARGB",
            Self::Argb4444 => "4444 ARGB",
            Self::Rgb888 => "888 (X8R8G8B8 32bpp)",
            Self::Argb8888 => "8888 ARGB",
            Self::Dxt1 => "DXT1",
            Self::Dxt3 => "DXT3",
            Self::Dxt5 => "DXT5",
        }
    }

    pub fn is_paletted(self) -> bool {
        matches!(self, Self::Pal8 | Self::Pal4)
    }

    pub fn is_dxt(self) -> bool {
        matches!(self, Self::Dxt1 | Self::Dxt3 | Self::Dxt5)
    }

    /// Whether the format carries per-pixel alpha (PAL formats carry it
    /// in the palette, so they count).
    pub fn has_alpha(self) -> bool {
        matches!(
            self,
            Self::Argb1555 | Self::Argb4444 | Self::Argb8888 | Self::Dxt3 | Self::Dxt5
        ) || self.is_paletted()
    }
}

/// The RenderWare header fields for one output format on one platform.
#[derive(Debug, Clone, Copy)]
pub struct HeaderSpec {
    pub raster_format: u32,
    pub d3d_format: u32,
    pub depth: u8,
    pub raster_type: u8,
    pub platform_properties: u8,
    pub paletted: bool,
}

impl HeaderSpec {
    /// Same header with the mip-map flag applied.
    pub fn with_mips(self, levels: u8) -> Self {
        let mut spec = self;
        if levels > 1 {
            spec.raster_format |= format::EXT_MIPMAP;
        } else {
            spec.raster_format &= !format::EXT_MIPMAP;
        }
        spec
    }
}

/// Header triples for a target platform, matched against the retail
/// corpora (III/VC platform 8, SA platform 9).
///
/// On platform 8 the `d3d_format` field is really the header's
/// has-alpha flag and DXT lives in `platform_properties` (1/3/5). On
/// platform 9 the FourCC selects DXT, `platform_properties` carries
/// the alpha flag in bit 0, and retail SA writes 8|alpha for
/// compressed rasters.
pub fn header_spec(format: EncodeFormat, platform_id: u32) -> HeaderSpec {
    let d3d9 = platform_id == 9;
    match format {
        EncodeFormat::Pal8 => HeaderSpec {
            // Retail III writes both 0x2500 and 0x2600 for PAL8; 0x2600
            // (888 base with the palette bit) is the common shape.
            // D3D9 paletted rasters use D3DFMT_P8 (41), per magic-rw's
            // D3D9 format table.
            raster_format: format::FORMAT_888 | format::EXT_PAL8,
            d3d_format: if d3d9 { 41 } else { 0 },
            depth: 8,
            raster_type: 4,
            platform_properties: 0,
            paletted: true,
        },
        EncodeFormat::Pal4 => HeaderSpec {
            raster_format: format::FORMAT_888 | format::EXT_PAL4,
            d3d_format: if d3d9 { 41 } else { 0 },
            depth: 4,
            raster_type: 4,
            platform_properties: 0,
            paletted: true,
        },
        EncodeFormat::Rgb565 => HeaderSpec {
            raster_format: format::FORMAT_565,
            d3d_format: if d3d9 { 23 } else { 0 },
            depth: 16,
            raster_type: 4,
            platform_properties: 0,
            paletted: false,
        },
        EncodeFormat::Argb1555 => HeaderSpec {
            raster_format: format::FORMAT_1555,
            d3d_format: if d3d9 { 25 } else { 1 },
            depth: 16,
            raster_type: 4,
            platform_properties: if d3d9 { 1 } else { 0 },
            paletted: false,
        },
        EncodeFormat::Argb4444 => HeaderSpec {
            raster_format: format::FORMAT_4444,
            d3d_format: if d3d9 { 26 } else { 1 },
            depth: 16,
            raster_type: 4,
            platform_properties: if d3d9 { 1 } else { 0 },
            paletted: false,
        },
        EncodeFormat::Rgb888 => HeaderSpec {
            raster_format: format::FORMAT_888,
            d3d_format: if d3d9 { 22 } else { 0 },
            depth: 32,
            raster_type: 4,
            platform_properties: 0,
            paletted: false,
        },
        EncodeFormat::Argb8888 => HeaderSpec {
            raster_format: format::FORMAT_8888,
            d3d_format: if d3d9 { 21 } else { 1 },
            depth: 32,
            raster_type: 4,
            platform_properties: if d3d9 { 1 } else { 0 },
            paletted: false,
        },
        EncodeFormat::Dxt1 => HeaderSpec {
            raster_format: format::FORMAT_565,
            d3d_format: if d3d9 { 0x3154_5844 } else { 0 },
            depth: 16,
            raster_type: 4,
            platform_properties: if d3d9 { 8 } else { 1 },
            paletted: false,
        },
        EncodeFormat::Dxt3 => HeaderSpec {
            raster_format: format::FORMAT_4444,
            d3d_format: if d3d9 { 0x3354_5844 } else { 1 },
            depth: 16,
            raster_type: 4,
            platform_properties: if d3d9 { 9 } else { 3 },
            paletted: false,
        },
        EncodeFormat::Dxt5 => HeaderSpec {
            raster_format: format::FORMAT_4444,
            d3d_format: if d3d9 { 0x3554_5844 } else { 1 },
            depth: 16,
            raster_type: 4,
            platform_properties: if d3d9 { 9 } else { 5 },
            paletted: false,
        },
    }
}

/// DXT encoder effort levels. `Standard` matches squish's default
/// (cluster fit + perceptual metric), which is what Magic.TXD and most
/// tools use; `High` runs squish's iterative cluster fit for the best
/// fit at several times the cost; `Fast` is range fit for bulk work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DxtQuality {
    Fast,
    #[default]
    Standard,
    High,
}

/// Options that trade encode time for quality.
#[derive(Debug, Clone, Copy, Default)]
pub struct EncodeOptions {
    /// DXT encoder effort (ignored by non-DXT formats).
    pub dxt_quality: DxtQuality,
    /// Optional Bayer 4x4 dithering for palette quantization.
    pub dither: bool,
}

/// An encoded native texture ready for the TXD writer.
#[derive(Debug, Clone)]
pub struct EncodedTexture {
    pub format: EncodeFormat,
    pub header: HeaderSpec,
    pub width: u32,
    pub height: u32,
    /// 1024 bytes (BGRA, PC storage) for paletted formats, empty else.
    pub palette: Vec<u8>,
    /// One entry per mip level, largest first.
    pub mipmaps: Vec<Vec<u8>>,
}

impl EncodedTexture {
    pub fn levels(&self) -> u8 {
        self.mipmaps.len().max(1) as u8
    }
}

/// Encode RGBA pixels into `format` for `platform_id`, generating the
/// full mip chain.
pub fn encode_texture(
    rgba: &[u8],
    width: u32,
    height: u32,
    format: EncodeFormat,
    platform_id: u32,
    options: EncodeOptions,
) -> Result<EncodedTexture, String> {
    if width == 0 || height == 0 || width > 8192 || height > 8192 {
        return Err(format!("unsupported texture size {width}x{height}"));
    }
    let expected = width as usize * height as usize * 4;
    if rgba.len() < expected {
        return Err(format!(
            "pixel buffer too small: need {expected} bytes, have {}",
            rgba.len()
        ));
    }
    let source_alpha = rgba[..expected]
        .chunks_exact(4)
        .any(|pixel| pixel[3] < 255);

    let levels = mip_chain(rgba, width, height);
    let mut palette = Vec::new();
    let mipmaps = match format {
        EncodeFormat::Pal8 => {
            let (pal, indices) = encode_paletted(&levels, 256, options.dither);
            palette = pal;
            indices
        }
        EncodeFormat::Pal4 => {
            let (pal, indices) = encode_paletted(&levels, 16, options.dither);
            palette = pal;
            indices
        }
        EncodeFormat::Rgb565 => levels
            .iter()
            .map(|(_, _, px)| encode_565(px))
            .collect(),
        EncodeFormat::Argb1555 => levels
            .iter()
            .map(|(_, _, px)| encode_1555(px))
            .collect(),
        EncodeFormat::Argb4444 => levels
            .iter()
            .map(|(_, _, px)| encode_4444(px))
            .collect(),
        EncodeFormat::Rgb888 => levels.iter().map(|(_, _, px)| encode_888(px)).collect(),
        EncodeFormat::Argb8888 => levels.iter().map(|(_, _, px)| encode_8888(px)).collect(),
        EncodeFormat::Dxt1 => encode_dxt_levels(&levels, texpresso::Format::Bc1, options, source_alpha),
        EncodeFormat::Dxt3 => encode_dxt_levels(&levels, texpresso::Format::Bc2, options, source_alpha),
        EncodeFormat::Dxt5 => encode_dxt_levels(&levels, texpresso::Format::Bc3, options, source_alpha),
    };

    let header = header_spec(format, platform_id).with_mips(mipmaps.len().max(1) as u8);
    Ok(EncodedTexture {
        format,
        header,
        width,
        height,
        palette,
        mipmaps,
    })
}

/// Downsample to the full mip chain with a box filter, halving until
/// 1x1. Odd dimensions round up (RenderWare levels never go below 1).
pub fn mip_chain(rgba: &[u8], width: u32, height: u32) -> Vec<(u32, u32, Vec<u8>)> {
    let mut levels = vec![(width, height, rgba[..width as usize * height as usize * 4].to_vec())];
    loop {
        let (w, h, ref px) = *levels.last().expect("level exists");
        if w == 1 && h == 1 {
            break;
        }
        let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
        let down = downsample(px, w, h, nw, nh);
        levels.push((nw, nh, down));
    }
    levels
}

fn downsample(px: &[u8], w: u32, h: u32, nw: u32, nh: u32) -> Vec<u8> {
    let mut out = vec![0u8; nw as usize * nh as usize * 4];
    for y in 0..nh {
        for x in 0..nw {
            let mut sums = [0u32; 4];
            let mut count = 0u32;
            for sy in (y * 2)..(y * 2 + 2).min(h) {
                for sx in (x * 2)..(x * 2 + 2).min(w) {
                    let o = ((sy * w + sx) * 4) as usize;
                    for c in 0..4 {
                        sums[c] += u32::from(px[o + c]);
                    }
                    count += 1;
                }
            }
            let o = ((y * nw + x) * 4) as usize;
            for c in 0..4 {
                out[o + c] = (sums[c] / count.max(1)) as u8;
            }
        }
    }
    out
}

// ---- Uncompressed formats --------------------------------------------

/// RW PC 8888 storage is BGRA.
fn encode_8888(px: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(px.len());
    for p in px.chunks_exact(4) {
        out.extend_from_slice(&[p[2], p[1], p[0], p[3]]);
    }
    out
}

/// RW PC "888" storage is BGRX (X8R8G8B8); the X byte is filled opaque.
fn encode_888(px: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(px.len());
    for p in px.chunks_exact(4) {
        out.extend_from_slice(&[p[2], p[1], p[0], 0xFF]);
    }
    out
}

fn encode_565(px: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(px.len() / 2);
    for p in px.chunks_exact(4) {
        let v = ((u16::from(p[0] >> 3)) << 11)
            | ((u16::from(p[1] >> 2)) << 5)
            | u16::from(p[2] >> 3);
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

fn encode_1555(px: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(px.len() / 2);
    for p in px.chunks_exact(4) {
        let a = u16::from(p[3] >= 128);
        let v = (a << 15)
            | ((u16::from(p[0] >> 3)) << 10)
            | ((u16::from(p[1] >> 3)) << 5)
            | u16::from(p[2] >> 3);
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

fn encode_4444(px: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(px.len() / 2);
    for p in px.chunks_exact(4) {
        let v = ((u16::from(p[0] >> 4)) << 12)
            | ((u16::from(p[1] >> 4)) << 8)
            | ((u16::from(p[2] >> 4)) << 4)
            | u16::from(p[3] >> 4);
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

// ---- DXT --------------------------------------------------------------

fn encode_dxt_levels(
    levels: &[(u32, u32, Vec<u8>)],
    format: texpresso::Format,
    options: EncodeOptions,
    source_alpha: bool,
) -> Vec<Vec<u8>> {
    levels
        .iter()
        .map(|(w, h, px)| {
            let mut out = vec![0u8; format.compressed_size(*w as usize, *h as usize)];
            let params = texpresso::Params {
                algorithm: match options.dxt_quality {
                    DxtQuality::Fast => texpresso::Algorithm::RangeFit,
                    DxtQuality::Standard => texpresso::Algorithm::ClusterFit,
                    DxtQuality::High => texpresso::Algorithm::IterativeClusterFit,
                },
                // Alpha-blended textures fit better when the colour
                // error is weighted by alpha; squish offers the same
                // flag (kWeightColourByAlpha) and Magic.TXD never sets
                // it.
                weigh_colour_by_alpha: source_alpha,
                ..texpresso::Params::default()
            };
            format.compress(px, *w as usize, *h as usize, params, &mut out);
            out
        })
        .collect()
}

// ---- Palette quantization -------------------------------------------

/// Quantize to at most `max_colors` entries and map every mip level.
/// Returns the 1024-byte BGRA palette (PC storage) and per-level
/// palette indices. The quantizer is alpha-aware: transparent pixels
/// get a reserved entry instead of consuming color slots, and entry
/// alpha is the usage-weighted average of its members.
fn encode_paletted(
    levels: &[(u32, u32, Vec<u8>)],
    max_colors: usize,
    dither: bool,
) -> (Vec<u8>, Vec<Vec<u8>>) {
    let (_, _, base) = &levels[0];
    let quantized = crate::compat::palette::quantize(base, max_colors);
    let mut indices = Vec::with_capacity(levels.len());
    for (w, _, px) in levels {
        indices.push(crate::compat::palette::map_level(
            px,
            *w,
            &quantized,
            dither,
        ));
    }

    let mut palette_bytes = vec![0u8; 1024];
    for (i, c) in quantized.entries.iter().enumerate() {
        let o = i * 4;
        palette_bytes[o] = c[2];
        palette_bytes[o + 1] = c[1];
        palette_bytes[o + 2] = c[0];
        palette_bytes[o + 3] = c[3];
    }
    (palette_bytes, indices)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::texture_decoder::{RasterDescriptor, decode_native_raster};

    fn gradient(width: u32, height: u32) -> Vec<u8> {
        let mut px = Vec::with_capacity((width * height * 4) as usize);
        for y in 0..height {
            for x in 0..width {
                px.extend([
                    (x * 255 / width.max(1)) as u8,
                    (y * 255 / height.max(1)) as u8,
                    ((x + y) * 255 / (width + height).max(1)) as u8,
                    255,
                ]);
            }
        }
        px
    }

    /// Encode then decode through the game decoder; return mean
    /// absolute error per channel.
    fn round_trip_error(format: EncodeFormat, rgba: &[u8], w: u32, h: u32) -> f32 {
        let encoded = encode_texture(rgba, w, h, format, 9, EncodeOptions::default())
            .expect("encode");
        let level = &encoded.mipmaps[0];
        let descriptor = RasterDescriptor {
            width: w,
            height: h,
            depth: encoded.header.depth,
            raster_format: encoded.header.raster_format,
            palette: &encoded.palette,
            platform_id: 9,
            d3d_format: encoded.header.d3d_format,
            platform_properties: encoded.header.platform_properties,
            raster_type: encoded.header.raster_type,
        };
        let decoded = decode_native_raster(level, &descriptor).expect("decode");
        assert_eq!(decoded.len(), rgba.len());
        let mut total = 0u64;
        for (a, b) in rgba.iter().zip(decoded.iter()) {
            total += u64::from(a.abs_diff(*b));
        }
        total as f32 / rgba.len() as f32
    }

    #[test]
    fn uncompressed_formats_round_trip_within_quantization_error() {
        let px = gradient(64, 64);
        assert!(round_trip_error(EncodeFormat::Argb8888, &px, 64, 64) < 1.0);
        assert!(round_trip_error(EncodeFormat::Rgb888, &px, 64, 64) < 1.0);
        // The 16-bit and 1555 formats lose low bits only.
        assert!(round_trip_error(EncodeFormat::Rgb565, &px, 64, 64) < 5.0);
        assert!(round_trip_error(EncodeFormat::Argb4444, &px, 64, 64) < 10.0);
        let opaque_alpha = {
            let mut v = px.clone();
            for p in v.chunks_exact_mut(4) {
                p[3] = 255;
            }
            v
        };
        assert!(round_trip_error(EncodeFormat::Argb1555, &opaque_alpha, 64, 64) < 10.0);
    }

    #[test]
    fn dxt_formats_round_trip_within_compression_error() {
        let px = gradient(64, 64);
        for format in [EncodeFormat::Dxt1, EncodeFormat::Dxt3, EncodeFormat::Dxt5] {
            let error = round_trip_error(format, &px, 64, 64);
            assert!(error < 12.0, "{format:?} error {error}");
        }
    }

    #[test]
    fn palette_formats_keep_small_palettes_exact() {
        // Eight exact colors: PAL8 must reproduce them losslessly.
        let mut px = Vec::new();
        let colors: [[u8; 4]; 8] = [
            [0, 0, 0, 255],
            [255, 0, 0, 255],
            [0, 255, 0, 255],
            [0, 0, 255, 255],
            [255, 255, 0, 255],
            [255, 0, 255, 255],
            [0, 255, 255, 255],
            [128, 64, 32, 255],
        ];
        for i in 0..(8 * 8) {
            px.extend(colors[i % colors.len()]);
        }
        let error = round_trip_error(EncodeFormat::Pal8, &px, 8, 8);
        assert_eq!(error, 0.0, "a palette-sized image must survive PAL8");
        let error = round_trip_error(EncodeFormat::Pal4, &px, 8, 8);
        assert_eq!(error, 0.0, "a 16-color image must survive PAL4");
    }

    #[test]
    fn mip_chain_halves_until_one() {
        let px = gradient(16, 8);
        let levels = mip_chain(&px, 16, 8);
        let dims: Vec<(u32, u32)> = levels.iter().map(|(w, h, _)| (*w, *h)).collect();
        assert_eq!(dims, vec![(16, 8), (8, 4), (4, 2), (2, 1), (1, 1)]);
        for (w, h, data) in &levels {
            assert_eq!(data.len(), (*w * *h * 4) as usize);
        }
    }

    #[test]
    fn d3d8_header_spec_matches_retail_vc_conventions() {
        let dxt1 = header_spec(EncodeFormat::Dxt1, 8);
        assert_eq!(dxt1.platform_properties, 1);
        assert_eq!(dxt1.d3d_format, 0);
        let dxt3 = header_spec(EncodeFormat::Dxt3, 8);
        assert_eq!(dxt3.platform_properties, 3);
        // D3D9 uses FourCC + alpha bit instead.
        let dxt1 = header_spec(EncodeFormat::Dxt1, 9);
        assert_eq!(dxt1.d3d_format, 0x3154_5844);
        assert_eq!(dxt1.platform_properties, 8);
        let dxt3 = header_spec(EncodeFormat::Dxt3, 9);
        assert_eq!(dxt3.platform_properties, 9);
        // D3D9 palettes use D3DFMT_P8 (41), like magic-rw writes.
        assert_eq!(header_spec(EncodeFormat::Pal8, 9).d3d_format, 41);
        // Mip flag round-trips.
        let spec = header_spec(EncodeFormat::Rgb565, 8).with_mips(9);
        assert!(spec.raster_format & format::EXT_MIPMAP != 0);
        let spec = header_spec(EncodeFormat::Rgb565, 8).with_mips(1);
        assert_eq!(spec.raster_format & format::EXT_MIPMAP, 0);
    }
}
