//! Raster profile extraction: turns a parsed RenderWare texture native
//! into a resolved description of what is *actually stored*, plus the
//! header-consistency anomalies that predict engine-side failures.
//!
//! The resolution chain mirrors `parser::texture_decoder::decode_native_raster`
//! (the D3D format word / FourCC is authoritative on D3D9, the depth byte
//! and data length disambiguate otherwise) so the validator always judges
//! the same bytes the decoder will render. Rules adapted from the INU
//! Tools lint catalog — see docs/research-inu-tools-gta.md (facts only).

use std::collections::BTreeMap;

use crate::parser::txd::NativeTexture;

/// Logical raster format after the cross-check chain resolves the header.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LogicalFormat {
    R1555,
    R565,
    R4444,
    Lum8,
    R888,
    R8888,
    R555,
    Pal4,
    Pal8,
    A8l8,
    Dxt1,
    Dxt2,
    Dxt3,
    Dxt4,
    Dxt5,
    Unknown,
}

impl LogicalFormat {
    pub fn label(self) -> &'static str {
        match self {
            Self::R1555 => "1555",
            Self::R565 => "565",
            Self::R4444 => "4444",
            Self::Lum8 => "LUM8",
            Self::R888 => "888",
            Self::R8888 => "8888",
            Self::R555 => "555",
            Self::Pal4 => "PAL4",
            Self::Pal8 => "PAL8",
            Self::A8l8 => "A8L8",
            Self::Dxt1 => "DXT1",
            Self::Dxt2 => "DXT2",
            Self::Dxt3 => "DXT3",
            Self::Dxt4 => "DXT4",
            Self::Dxt5 => "DXT5",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteKind {
    None,
    Pal4,
    Pal8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Warn,
    Error,
}

#[derive(Clone, Debug)]
pub struct Anomaly {
    pub code: &'static str,
    pub severity: Severity,
    pub detail: String,
}

/// The resolved storage description of one texture native.
#[derive(Clone, Debug)]
pub struct RasterProfile {
    pub platform_id: u32,
    pub raster_format: u32,
    pub d3d_format: u32,
    /// `'DXT1'`-style FourCC when the format word carries one.
    pub fourcc: Option<&'static str>,
    pub logical: LogicalFormat,
    /// Bytes per pixel of the *stored* data (after the 888/32bpp and
    /// DXT resolutions), 0 for unknown.
    pub storage_bpp: u32,
    pub width: u32,
    pub height: u32,
    pub depth: u8,
    pub mip_levels: u8,
    pub palette: PaletteKind,
    pub has_alpha_header: bool,
    pub automipmap: bool,
}

/// Resolution order: FourCC > D3D format word (8-bit formats carry no
/// raster nibble) > D3D8 compression code > palette > raster nibble
/// (+ depth). Shared by the full profile extraction and the header-only
/// hint probe so both always agree.
pub(crate) fn classify_format(
    raster_format: u32,
    platform_id: u32,
    d3d_format: u32,
    platform_properties: u8,
    raster_type: u8,
    depth: u8,
    paletted: bool,
) -> (LogicalFormat, u32) {
    if let Some(code) = fourcc_name(d3d_format) {
        let fmt = match code {
            "DXT1" => LogicalFormat::Dxt1,
            "DXT2" => LogicalFormat::Dxt2,
            "DXT3" => LogicalFormat::Dxt3,
            "DXT4" => LogicalFormat::Dxt4,
            _ => LogicalFormat::Dxt5,
        };
        return (fmt, 0);
    }
    if d3d_format == 50 {
        return (LogicalFormat::Lum8, 1);
    }
    if d3d_format == 51 {
        return (LogicalFormat::A8l8, 2);
    }
    // D3D8 natives have no FourCC: the compression code lives in the
    // platform-properties byte (1..5 = DXT1..DXT5) and the raster nibble
    // is stale from whatever the source format was before compression.
    // Retail VC is the proof: its "565/1555/4444"-labelled rasters have
    // DXT-sized mip data and pp 1/3.
    if let Some(dxt) = crate::parser::texture_decoder::native_dxt_type(
        raster_format,
        platform_id,
        d3d_format,
        platform_properties,
        raster_type,
    ) {
        let fmt = match dxt {
            crate::parser::texture_decoder::DxtType::Dxt1 => LogicalFormat::Dxt1,
            crate::parser::texture_decoder::DxtType::Dxt2 => LogicalFormat::Dxt2,
            crate::parser::texture_decoder::DxtType::Dxt3 => LogicalFormat::Dxt3,
            crate::parser::texture_decoder::DxtType::Dxt4 => LogicalFormat::Dxt4,
            crate::parser::texture_decoder::DxtType::Dxt5 => LogicalFormat::Dxt5,
        };
        return (fmt, 0);
    }
    if paletted {
        return (LogicalFormat::Pal8, 1);
    }
    match (raster_format >> 8) & 0xF {
        0x1 => (LogicalFormat::R1555, 2),
        0x2 => (LogicalFormat::R565, 2),
        0x3 => (LogicalFormat::R4444, 2),
        0x4 => (LogicalFormat::Lum8, 1),
        0x5 => (LogicalFormat::R8888, 4),
        0x6 => {
            // RW "888": D3D9-era conversions store it as 32-bit
            // X8R8G8B8; D3D8-era rasters keep true 24-bit rows. Same
            // disambiguation the decoder applies.
            if depth == 32 {
                (LogicalFormat::R888, 4)
            } else {
                (LogicalFormat::R888, 3)
            }
        }
        0xA => (LogicalFormat::R555, 2),
        _ => (LogicalFormat::Unknown, u32::from(depth) / 8),
    }
}

impl RasterProfile {
    /// Resolve a parsed native through the same cross-check chain the
    /// decoder uses, so verdicts judge what the pixels will actually be.
    pub fn from_native(texture: &NativeTexture) -> Self {
        let fourcc = fourcc_name(texture.d3d_format);
        let palette = match (texture.raster_format >> 13) & 0x3 {
            1 => PaletteKind::Pal8,
            2 | 3 => PaletteKind::Pal4,
            _ => PaletteKind::None,
        };
        let automipmap = texture.raster_format & 0x1000 != 0;

        let (logical, storage_bpp) = classify_format(
            texture.raster_format,
            texture.platform_id,
            texture.d3d_format,
            texture.platform_properties,
            texture.raster_type,
            texture.depth,
            palette != PaletteKind::None,
        );
        // D3D8 natives signal compression through the platform-properties
        // byte, not a FourCC; surface the resolved fourcc for the
        // anomaly checks and UI labels.
        let fourcc = fourcc.or_else(|| {
            crate::parser::texture_decoder::native_dxt_type(
                texture.raster_format,
                texture.platform_id,
                texture.d3d_format,
                texture.platform_properties,
                texture.raster_type,
            )
            .map(|dxt| match dxt {
                crate::parser::texture_decoder::DxtType::Dxt1 => "DXT1",
                crate::parser::texture_decoder::DxtType::Dxt2 => "DXT2",
                crate::parser::texture_decoder::DxtType::Dxt3 => "DXT3",
                crate::parser::texture_decoder::DxtType::Dxt4 => "DXT4",
                crate::parser::texture_decoder::DxtType::Dxt5 => "DXT5",
            })
        });

        Self {
            platform_id: texture.platform_id,
            raster_format: texture.raster_format,
            d3d_format: texture.d3d_format,
            fourcc,
            logical,
            storage_bpp,
            width: texture.width,
            height: texture.height,
            depth: texture.depth,
            mip_levels: texture.num_mipmaps,
            palette,
            has_alpha_header: texture.has_alpha != 0,
            automipmap,
        }
    }

    pub fn is_dxt(&self) -> bool {
        matches!(
            self.logical,
            LogicalFormat::Dxt1
                | LogicalFormat::Dxt2
                | LogicalFormat::Dxt3
                | LogicalFormat::Dxt4
                | LogicalFormat::Dxt5
        )
    }

    /// Header-consistency rules. A violation here is the class of defect
    /// that renders in some tools yet "silently never arrives in game"
    /// (the DXT3-on-8888-header failure documented in
    /// docs/research-inu-tools-gta.md §6.1).
    pub fn anomalies(&self) -> Vec<Anomaly> {
        let mut issues = Vec::new();
        let mut push = |code: &'static str, severity: Severity, detail: String| {
            issues.push(Anomaly {
                code,
                severity,
                detail,
            });
        };

        if self.platform_id != 8 && self.platform_id != 9 {
            push(
                "PLATFORM_UNRECOGNIZED",
                Severity::Error,
                format!("platform id {} is not a PC raster platform (8/9)", self.platform_id),
            );
        }
        if self.width == 0 || self.height == 0 {
            push(
                "DIMS_INVALID",
                Severity::Error,
                format!("dimensions {}x{}", self.width, self.height),
            );
        } else {
            if !self.width.is_power_of_two() || !self.height.is_power_of_two() {
                push(
                    "DIMS_NOT_POT",
                    Severity::Error,
                    format!("{}x{} is not power-of-two", self.width, self.height),
                );
            }
            if self.width > 1024 || self.height > 1024 {
                push(
                    "DIMS_TOO_LARGE",
                    Severity::Warn,
                    format!(
                        "{}x{} exceeds the 1024 px SA stream budget",
                        self.width, self.height
                    ),
                );
            }
            if self.is_dxt()
                && (!self.width.is_multiple_of(4) || !self.height.is_multiple_of(4))
            {
                push(
                    "DXT_DIMS_UNALIGNED",
                    Severity::Error,
                    format!(
                        "{}x{} is not 4-aligned; DXT blocks are 4x4",
                        self.width, self.height
                    ),
                );
            }
        }
        if ![4u8, 8, 16, 24, 32].contains(&self.depth) {
            push(
                "DEPTH_INVALID",
                Severity::Error,
                format!("depth {} bits", self.depth),
            );
        }
        if self.is_dxt() && self.palette != PaletteKind::None {
            push(
                "PALETTE_WITH_DXT",
                Severity::Error,
                "palette and DXT flags are mutually exclusive; readers branch incorrectly"
                    .to_string(),
            );
        }
        if self.automipmap && self.mip_levels > 1 {
            push(
                "AUTOMIPMAP_WITH_LEVELS",
                Severity::Error,
                format!("AUTOMIPMAP set with {} explicit levels — TXD won't load", self.mip_levels),
            );
        }
        let max_dim = self.width.max(self.height);
        if max_dim > 0 {
            let max_levels = max_dim.ilog2() as usize + 1;
            if self.mip_levels as usize > max_levels {
                push(
                    "MIP_COUNT_INVALID",
                    Severity::Error,
                    format!(
                        "{} levels exceed the max {} for {}x{}",
                        self.mip_levels, max_levels, self.width, self.height
                    ),
                );
            }
        }
        if self.is_dxt() && (self.depth == 32 || (self.raster_format >> 8) & 0xF == 0x5) {
            push(
                "CONTRADICTORY_DXT_HEADER",
                Severity::Error,
                format!(
                    "DXT fourcc with an uncompressed 8888 header ({} bits, raster 0x{:X}) — \
                     engines silently drop the texture",
                    self.depth, self.raster_format
                ),
            );
        }
        if let Some(code) = self.fourcc {
            let nibble = (self.raster_format >> 8) & 0xF;
            let expected = match code {
                "DXT1" | "DXT2" => 0x2, // 565-family
                "DXT3" | "DXT4" => 0x3, // 4444-family
                "DXT5" => 0x3,
                _ => nibble,
            };
            if nibble != expected && nibble != 0 {
                push(
                    "STALE_RASTER_NIBBLE",
                    Severity::Info,
                    format!(
                        "raster nibble 0x{:X} with {} fourcc — the FourCC wins, \
                         the nibble is stale from an earlier conversion",
                        nibble, code
                    ),
                );
            }
        }
        issues
    }
}

pub(crate) fn fourcc_name(d3d_format: u32) -> Option<&'static str> {
    match d3d_format {
        0x3154_5844 => Some("DXT1"),
        0x3254_5844 => Some("DXT2"),
        0x3354_5844 => Some("DXT3"),
        0x3454_5844 => Some("DXT4"),
        0x3554_5844 => Some("DXT5"),
        _ => None,
    }
}

/// Frequency counter helper shared by the scanner report.
pub type Counts<T> = BTreeMap<T, usize>;

#[cfg(test)]
mod tests {
    use super::*;

    /// D3D8 (platform 8) natives carry the compression code in the
    /// platform-properties byte; the raster nibble is stale from the
    /// pre-compression source format. Retail VC is the proof case.
    #[test]
    fn d3d8_compression_code_beats_the_stale_nibble() {
        // VC "1555" that is really DXT1: rf nibble 0x1, pp=1.
        let (logical, _) = classify_format(0x100, 8, 1, 1, 4, 16, false);
        assert_eq!(logical, LogicalFormat::Dxt1);
        // VC "565" that is really DXT1: rf nibble 0x2, pp=1.
        let (logical, _) = classify_format(0x200, 8, 0, 1, 4, 16, false);
        assert_eq!(logical, LogicalFormat::Dxt1);
        // VC "4444" that is really DXT3: rf nibble 0x3, pp=3.
        let (logical, _) = classify_format(0x300, 8, 1, 3, 4, 16, false);
        assert_eq!(logical, LogicalFormat::Dxt3);
        // A genuine 1555 (pp=0) stays uncompressed.
        let (logical, _) = classify_format(0x100, 8, 1, 0, 4, 16, false);
        assert_eq!(logical, LogicalFormat::R1555);
        // D3D9 still resolves by FourCC.
        let (logical, _) = classify_format(0x200, 9, 0x3354_5844, 9, 4, 16, false);
        assert_eq!(logical, LogicalFormat::Dxt3);
    }
}
