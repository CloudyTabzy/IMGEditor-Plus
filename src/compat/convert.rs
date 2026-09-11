//! Import/convert pipeline for Phase B: decoded source images in,
//! target-dialect natives and plan metadata out.
//!
//! The planner never writes anything: it decodes the source, chooses a
//! target format (or honors the user's override), encodes through
//! [`crate::compat::encode`], and reports warnings plus a "true"
//! preview decoded back from the encoded bytes. Execution helpers
//! apply a plan through the splices in
//! [`crate::parser::txd_writer`], so untouched textures stay verbatim.

use crate::compat::encode::{
    encode_texture, EncodeFormat, EncodeOptions, EncodedTexture,
};
use crate::compat::games::{profile_by_id, GameProfile, Verdict, SA};
use crate::compat::raster::{classify_format, RasterProfile};
use crate::parser::texture_decoder::{decode_native_raster, RasterDescriptor, PLATFORM_D3D8, PLATFORM_D3D9};
use crate::parser::txd::{parse_txd, NativeTexture};
use crate::parser::txd_writer;

/// A decoded source image (PNG/DDS/BMP/TGA).
#[derive(Debug, Clone)]
pub struct SourceImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub has_alpha: bool,
    pub format_label: String,
    pub encoded_bytes: usize,
}

impl SourceImage {
    pub fn unique_colors(&self) -> Option<u16> {
        crate::parser::texture_decoder::palette_colors(&self.rgba)
    }
}

/// Decode an imported image. `DDS` covers the compressed forms modders
/// export; other image types are rejected with a clear message.
///
/// Formats without magic bytes (TGA) need [`decode_source_image_named`].
pub fn decode_source_image(bytes: &[u8]) -> Result<SourceImage, String> {
    let format = image::guess_format(bytes)
        .map_err(|error| format!("unrecognized image format: {error}"))?;
    decode_image_with_format(bytes, format)
}

/// Decode an imported image using the file name as a format hint first
/// (TGA has no header magic), then content sniffing.
pub fn decode_source_image_named(bytes: &[u8], file_name: &str) -> Result<SourceImage, String> {
    let hinted = std::path::Path::new(file_name)
        .extension()
        .and_then(|extension| extension.to_str())
        .and_then(|extension| match extension.to_ascii_lowercase().as_str() {
            "png" => Some(image::ImageFormat::Png),
            "dds" => Some(image::ImageFormat::Dds),
            "bmp" => Some(image::ImageFormat::Bmp),
            "tga" => Some(image::ImageFormat::Tga),
            _ => None,
        });
    let format = match hinted {
        Some(format) => format,
        None => image::guess_format(bytes)
            .map_err(|error| format!("unrecognized image format: {error}"))?,
    };
    decode_image_with_format(bytes, format)
}

fn decode_image_with_format(
    bytes: &[u8],
    format: image::ImageFormat,
) -> Result<SourceImage, String> {
    let label = match format {
        image::ImageFormat::Png => "PNG",
        image::ImageFormat::Dds => "DDS",
        image::ImageFormat::Bmp => "BMP",
        image::ImageFormat::Tga => "TGA",
        other => {
            return Err(format!(
                "{other:?} images are not supported; use PNG, DDS, BMP, or TGA"
            ));
        }
    };
    let decoded = image::load_from_memory_with_format(bytes, format)
        .map_err(|error| format!("{label} decode failed: {error}"))?;
    let rgba = decoded.to_rgba8();
    let (width, height) = rgba.dimensions();
    if width == 0 || height == 0 || width > 8192 || height > 8192 {
        return Err(format!("unsupported image size {width}x{height}"));
    }
    let has_alpha = rgba.pixels().any(|pixel| pixel[3] < 255);
    Ok(SourceImage {
        width,
        height,
        rgba: rgba.into_raw(),
        has_alpha,
        format_label: label.to_string(),
        encoded_bytes: bytes.len(),
    })
}

/// The platform a target's TXDs are written for (III/VC D3D8, SA
/// D3D9).
pub fn target_platform(target: &GameProfile) -> u32 {
    if target.id == SA.id {
        PLATFORM_D3D9
    } else {
        PLATFORM_D3D8
    }
}

/// Version words measured from the retail corpora (2026-09-11e): III
/// world 0x0401FFFF, III player TXDs 0x0C02FFFF, VC 0x1003FFFF, SA
/// 0x1803FFFF.
pub fn target_rw_version(target: &GameProfile) -> u32 {
    match target.id {
        "gta3" => 0x0401_FFFF,
        "vc" => 0x1003_FFFF,
        "sa" => 0x1803_FFFF,
        _ => txd_writer::RW_VERSION_DEFAULT,
    }
}

/// One selectable output format for the import/replace dialogs.
#[derive(Debug, Clone)]
pub struct FormatChoice {
    pub format: EncodeFormat,
    /// Whether this is a stock form for the target (native dialect).
    pub native: bool,
    pub note: &'static str,
}

/// Suggested formats for a target, best-first. The default choice is
/// picked by [`default_format`]; everything else is an explicit
/// opt-in (smaller files, quantization, hardware-only forms).
pub fn format_choices(target: &GameProfile, archive_file_name: &str) -> Vec<FormatChoice> {
    let player_archive = archive_file_name.eq_ignore_ascii_case("player.img");
    match target.id {
        "gta3" => vec![
            FormatChoice {
                format: EncodeFormat::Rgb888,
                native: true,
                note: "32-bit X8R8G8B8, lossless; retail III txd.img standard",
            },
            FormatChoice {
                format: EncodeFormat::Argb8888,
                native: true,
                note: "A8R8G8B8, keeps alpha; retail III ships 1,121",
            },
            FormatChoice {
                format: EncodeFormat::Pal8,
                native: true,
                note: "8-bit palette, quantizes colors; retail world dialect (96.5%)",
            },
            FormatChoice {
                format: EncodeFormat::Pal4,
                native: true,
                note: "4-bit palette, quantizes hard; 16-color art only",
            },
            FormatChoice {
                format: EncodeFormat::Argb1555,
                native: true,
                note: "16-bit with 1-bit alpha; retail ships 24",
            },
            FormatChoice {
                format: EncodeFormat::Dxt1,
                native: false,
                note: "hardware-supported but not shipped by III; lossy",
            },
            FormatChoice {
                format: EncodeFormat::Dxt3,
                native: false,
                note: "hardware-supported but not shipped by III; lossy",
            },
        ],
        "vc" => vec![
            FormatChoice {
                format: EncodeFormat::Dxt1,
                native: true,
                note: "retail VC world dialect (D3D8 pp=1); lossy compression",
            },
            FormatChoice {
                format: EncodeFormat::Dxt3,
                native: true,
                note: "retail VC alpha dialect (D3D8 pp=3); lossy compression",
            },
            FormatChoice {
                format: EncodeFormat::Rgb888,
                native: true,
                note: "32-bit X8R8G8B8, lossless; retail VC ships one",
            },
            FormatChoice {
                format: EncodeFormat::Argb8888,
                native: true,
                note: "A8R8G8B8, keeps alpha; D3D8-era form",
            },
            FormatChoice {
                format: EncodeFormat::Pal8,
                native: true,
                note: "8-bit palette, quantizes colors; retail VC ships 27",
            },
            FormatChoice {
                format: EncodeFormat::Rgb565,
                native: false,
                note: "16-bit; retail VC labels 565 as DXT data, raw form unmeasured",
            },
            FormatChoice {
                format: EncodeFormat::Argb4444,
                native: false,
                note: "16-bit with alpha; retail VC labels 4444 as DXT3 data",
            },
        ],
        "sa" => {
            let mut choices = Vec::new();
            if player_archive {
                choices.push(FormatChoice {
                    format: EncodeFormat::Rgb888,
                    native: true,
                    note: "32-bit X8R8G8B8; retail player.img ships 269",
                });
                choices.push(FormatChoice {
                    format: EncodeFormat::Argb8888,
                    native: true,
                    note: "A8R8G8B8, keeps alpha; retail player.img ships 125",
                });
                choices.push(FormatChoice {
                    format: EncodeFormat::Dxt1,
                    native: true,
                    note: "supported everywhere; lossy (player.img ships none)",
                });
                choices.push(FormatChoice {
                    format: EncodeFormat::Dxt3,
                    native: true,
                    note: "supported everywhere; lossy (player.img ships none)",
                });
            } else {
                choices.push(FormatChoice {
                    format: EncodeFormat::Dxt1,
                    native: true,
                    note: "retail SA world dialect; lossy compression",
                });
                choices.push(FormatChoice {
                    format: EncodeFormat::Dxt3,
                    native: true,
                    note: "retail SA alpha dialect; lossy compression",
                });
                choices.push(FormatChoice {
                    format: EncodeFormat::Argb8888,
                    native: true,
                    note: "A8R8G8B8, lossless; retail SA ships it in player.img",
                });
                choices.push(FormatChoice {
                    format: EncodeFormat::Pal8,
                    native: false,
                    note: "quantizes colors; retail SA ships zero paletted rasters",
                });
            }
            choices
        }
        _ => Vec::new(),
    }
}

/// The default output format for an import into `target`.
pub fn default_format(
    target: &GameProfile,
    archive_file_name: &str,
    has_alpha: bool,
) -> Result<EncodeFormat, String> {
    match target.id {
        "gta3" => Ok(if has_alpha {
            EncodeFormat::Argb8888
        } else {
            EncodeFormat::Rgb888
        }),
        "vc" => Ok(if has_alpha {
            EncodeFormat::Dxt3
        } else {
            EncodeFormat::Dxt1
        }),
        "sa" => {
            let player_archive = archive_file_name.eq_ignore_ascii_case("player.img");
            if player_archive {
                Ok(if has_alpha {
                    EncodeFormat::Argb8888
                } else {
                    EncodeFormat::Rgb888
                })
            } else {
                Ok(if has_alpha {
                    EncodeFormat::Dxt3
                } else {
                    EncodeFormat::Dxt1
                })
            }
        }
        _ => Err("Bully (Gamebryo) texture writing is not supported yet".to_string()),
    }
}

/// Everything the confirm dialog shows about a planned conversion.
#[derive(Debug, Clone)]
pub struct ConversionPlan {
    pub format: EncodeFormat,
    pub format_label: String,
    pub platform_id: u32,
    pub width: u32,
    pub height: u32,
    pub source_label: String,
    pub source_bytes: usize,
    pub output_bytes: usize,
    pub warnings: Vec<String>,
    /// Base level decoded back from the *encoded* bytes: what the game
    /// will actually sample.
    pub preview_rgba: Vec<u8>,
    pub encoded: EncodedTexture,
}

/// Plan an import of a decoded source image.
pub fn plan_import(
    image: &SourceImage,
    target: &GameProfile,
    archive_file_name: &str,
    format_override: Option<EncodeFormat>,
    options: EncodeOptions,
) -> Result<ConversionPlan, String> {
    let format = match format_override {
        Some(format) => format,
        None => default_format(target, archive_file_name, image.has_alpha)?,
    };
    let platform = target_platform(target);
    let encoded = encode_texture(
        &image.rgba,
        image.width,
        image.height,
        format,
        platform,
        options,
    )?;

    let mut warnings = Vec::new();
    if image.has_alpha && !format.has_alpha() {
        warnings.push(format!(
            "{} has alpha, but {} cannot store it - the alpha channel will be discarded.",
            image.format_label,
            format.label()
        ));
    }
    if format.is_dxt() {
        warnings.push(format!(
            "{} compression is lossy; the preview shows the encoded result.",
            format.label()
        ));
    }
    if format.is_paletted() {
        let cap = if format == EncodeFormat::Pal4 { 16 } else { 256 };
        match image.unique_colors() {
            Some(colors) if usize::from(colors) <= cap => {
                warnings.push(format!(
                    "{format_label} stores the image exactly ({colors} colors).",
                    format_label = format.label()
                ));
            }
            _ => warnings.push(format!(
                "Colors will be quantized to at most {cap} entries.",
                cap = cap
            )),
        }
    }
    if target.id == "sa" && image.width.max(image.height) > 1024 {
        warnings.push(format!(
            "{}x{} exceeds the 1024 px SA stream budget; the game may not stream it.",
            image.width, image.height
        ));
    }
    // The target's own verdict for the chosen form, so the dialog never
    // hides a non-native choice.
    let sample = synthetic_profile(&encoded, platform);
    let report = crate::compat::games::classify(target, &sample);
    if report.verdict > Verdict::Supported {
        warnings.push(format!(
            "{} is {} for {}: {}",
            format.label(),
            report.verdict.label(),
            target.display,
            report.note
        ));
    }

    let preview_rgba = decode_encoded(&encoded, platform)?;
    let output_bytes = encoded.palette.len()
        + encoded
            .mipmaps
            .iter()
            .map(|mip| 4 + mip.len())
            .sum::<usize>();

    Ok(ConversionPlan {
        format,
        format_label: format.label().to_string(),
        platform_id: platform,
        width: image.width,
        height: image.height,
        source_label: image.format_label.clone(),
        source_bytes: image.encoded_bytes,
        output_bytes,
        warnings,
        preview_rgba,
        encoded,
    })
}

/// Plan converting an existing TXD texture to the target dialect.
pub fn plan_conversion(
    txd_bytes: &[u8],
    texture_index: usize,
    target: &GameProfile,
    archive_file_name: &str,
    format_override: Option<EncodeFormat>,
    options: EncodeOptions,
) -> Result<ConversionPlan, String> {
    let parsed = parse_txd(txd_bytes)?;
    let texture = parsed
        .textures
        .get(texture_index)
        .ok_or_else(|| format!("texture index {texture_index} is out of range"))?;
    let rgba = texture.decode_rgba().map_err(|error| {
        format!(
            "'{}' cannot be decoded ({error}); conversion needs readable pixels",
            texture.diffuse_name
        )
    })?;
    if texture.width == 0 || texture.height == 0 {
        return Err("texture has no dimensions".to_string());
    }
    let source_bytes = texture
        .mipmaps
        .iter()
        .map(|mip| 4 + mip.data.len())
        .sum::<usize>()
        + texture.palette.len();
    let mut image = SourceImage {
        width: texture.width,
        height: texture.height,
        rgba,
        has_alpha: texture.has_alpha_channel(),
        format_label: texture.format_name().to_string(),
        encoded_bytes: source_bytes,
    };
    // A source with real transparency in a non-alpha header should
    // still plan as alpha-carrying.
    image.has_alpha = image.has_alpha
        || image
            .rgba
            .chunks_exact(4)
            .any(|pixel| pixel[3] < 255);
    plan_import(&image, target, archive_file_name, format_override, options)
}

/// Apply a replace plan to an existing TXD entry. The texture's name
/// and every other texture are preserved.
pub fn apply_replace(
    txd_bytes: &[u8],
    texture_index: usize,
    plan: &ConversionPlan,
) -> Result<Vec<u8>, String> {
    let parsed = parse_txd(txd_bytes)?;
    let old = parsed
        .textures
        .get(texture_index)
        .ok_or_else(|| format!("texture index {texture_index} is out of range"))?;
    let native = txd_writer::native_from_encoded(
        &plan.encoded,
        plan.platform_id,
        &old.diffuse_name,
        &old.alpha_name,
    );
    txd_writer::replace_texture(txd_bytes, texture_index, native)
}

/// Build a brand-new single-texture TXD from an import plan.
pub fn build_new_txd(plan: &ConversionPlan, target: &GameProfile, texture_name: &str) -> Vec<u8> {
    let native = txd_writer::native_from_encoded(
        &plan.encoded,
        plan.platform_id,
        texture_name,
        "",
    );
    txd_writer::single_texture_txd(native, target_rw_version(target))
}

/// Decode the encoded base level with the game's own decoder.
fn decode_encoded(encoded: &EncodedTexture, platform_id: u32) -> Result<Vec<u8>, String> {
    let level = encoded
        .mipmaps
        .first()
        .ok_or_else(|| "encoder produced no mip levels".to_string())?;
    let descriptor = RasterDescriptor {
        width: encoded.width,
        height: encoded.height,
        depth: encoded.header.depth,
        raster_format: encoded.header.raster_format,
        palette: &encoded.palette,
        platform_id,
        d3d_format: encoded.header.d3d_format,
        platform_properties: encoded.header.platform_properties,
        raster_type: encoded.header.raster_type,
    };
    decode_native_raster(level, &descriptor).map_err(|error| error.to_string())
}

/// A synthetic profile matching what [`encode_texture`] produced, for
/// the classifier.
fn synthetic_profile(encoded: &EncodedTexture, platform_id: u32) -> RasterProfile {
    let paletted = encoded.format.is_paletted();
    let (logical, storage_bpp) = classify_format(
        encoded.header.raster_format,
        platform_id,
        encoded.header.d3d_format,
        encoded.header.platform_properties,
        encoded.header.raster_type,
        encoded.header.depth,
        paletted,
    );
    RasterProfile {
        platform_id,
        raster_format: encoded.header.raster_format,
        d3d_format: encoded.header.d3d_format,
        fourcc: crate::compat::raster::fourcc_name(encoded.header.d3d_format),
        logical,
        storage_bpp,
        width: encoded.width,
        height: encoded.height,
        depth: encoded.header.depth,
        mip_levels: encoded.levels(),
        palette: if paletted {
            crate::compat::raster::PaletteKind::Pal8
        } else {
            crate::compat::raster::PaletteKind::None
        },
        has_alpha_header: encoded.format.has_alpha(),
        automipmap: false,
    }
}

/// Convenience: profile of a parsed texture.
pub fn texture_profile(texture: &NativeTexture) -> RasterProfile {
    RasterProfile::from_native(texture)
}

/// Resolve a target id, erroring when unknown or Bully.
pub fn writable_target(id: &str) -> Result<&'static GameProfile, String> {
    let target = profile_by_id(id).ok_or_else(|| format!("unknown game target '{id}'"))?;
    if target.id == crate::compat::games::BULLY.id {
        return Err("Bully (Gamebryo) texture writing is not supported yet".to_string());
    }
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::encode::{encode_texture, DxtQuality};
    use crate::compat::games::{GTA3, VC};
    use crate::parser::txd_writer::single_texture_txd;

    fn png_bytes(width: u32, height: u32, alpha: bool) -> Vec<u8> {
        let mut rgba = image::RgbaImage::new(width, height);
        for (x, y, pixel) in rgba.enumerate_pixels_mut() {
            *pixel = image::Rgba([
                (x * 7) as u8,
                (y * 5) as u8,
                ((x + y) * 3) as u8,
                if alpha { ((x * 3) % 255) as u8 } else { 255 },
            ]);
        }
        let mut out = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(rgba)
            .write_to(&mut out, image::ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    #[test]
    fn decodes_png_bmp_tga_and_reports_alpha() {
        let png = png_bytes(8, 8, true);
        let image = decode_source_image(&png).expect("png");
        assert_eq!((image.width, image.height), (8, 8));
        assert!(image.has_alpha);

        // A source without alpha stays reported as opaque (BMP carries
        // no reliable alpha channel in its common 24-bit form).
        let opaque = image::load_from_memory(&png).unwrap().to_rgb8();
        let mut bmp = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(opaque)
            .write_to(&mut bmp, image::ImageFormat::Bmp)
            .unwrap();
        let image = decode_source_image(&bmp.into_inner()).expect("bmp");
        assert!(!image.has_alpha);

        let decoded = image::load_from_memory(&png).unwrap().to_rgba8();

        let mut tga = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(decoded)
            .write_to(&mut tga, image::ImageFormat::Tga)
            .unwrap();
        let image = decode_source_image_named(&tga.into_inner(), "tex.tga").expect("tga");
        assert_eq!((image.width, image.height), (8, 8));
    }

    #[test]
    fn decodes_a_dxt5_dds_export() {
        // Build a DDS around texpresso-compressed BC3 blocks: the shape
        // modders export.
        let width = 16u32;
        let height = 16u32;
        let mut rgba = Vec::new();
        for y in 0..height {
            for x in 0..width {
                rgba.extend([(x * 16) as u8, (y * 16) as u8, 128, 200]);
            }
        }
        let mut blocks = vec![0u8; texpresso::Format::Bc3.compressed_size(width as usize, height as usize)];
        texpresso::Format::Bc3.compress(&rgba, width as usize, height as usize, texpresso::Params::default(), &mut blocks);
        let mut dds = vec![0u8; 128];
        dds[0..4].copy_from_slice(b"DDS ");
        dds[4..8].copy_from_slice(&124u32.to_le_bytes());
        let flags: u32 = 0x1 | 0x2 | 0x4 | 0x1000 | 0x80000;
        dds[8..12].copy_from_slice(&flags.to_le_bytes());
        dds[12..16].copy_from_slice(&height.to_le_bytes());
        dds[16..20].copy_from_slice(&width.to_le_bytes());
        dds[20..24].copy_from_slice(&(blocks.len() as u32).to_le_bytes());
        dds[76..80].copy_from_slice(&32u32.to_le_bytes());
        dds[80..84].copy_from_slice(&0x4u32.to_le_bytes());
        dds[84..88].copy_from_slice(b"DXT5");
        dds[108..112].copy_from_slice(&0x1000u32.to_le_bytes());
        dds.extend_from_slice(&blocks);

        let image = decode_source_image(&dds).expect("dds");
        assert_eq!((image.width, image.height), (16, 16));
        // Alpha 200 everywhere -> reported as alpha-carrying.
        assert!(image.has_alpha);
    }

    #[test]
    fn defaults_follow_the_corrected_retail_dialects() {
        assert_eq!(
            default_format(&GTA3, "gta3.img", false).unwrap(),
            EncodeFormat::Rgb888
        );
        assert_eq!(
            default_format(&GTA3, "gta3.img", true).unwrap(),
            EncodeFormat::Argb8888
        );
        // VC is DXT1/DXT3 since the 2026-09-11e correction.
        assert_eq!(
            default_format(&VC, "gta3.img", false).unwrap(),
            EncodeFormat::Dxt1
        );
        assert_eq!(
            default_format(&VC, "gta3.img", true).unwrap(),
            EncodeFormat::Dxt3
        );
        assert_eq!(
            default_format(&SA, "gta3.img", false).unwrap(),
            EncodeFormat::Dxt1
        );
        assert_eq!(
            default_format(&SA, "gta3.img", true).unwrap(),
            EncodeFormat::Dxt3
        );
        // player.img stays uncompressed.
        assert_eq!(
            default_format(&SA, "player.img", false).unwrap(),
            EncodeFormat::Rgb888
        );
        assert!(default_format(profile_by_id("bully").unwrap(), "World.img", false).is_err());
    }

    #[test]
    fn alpha_loss_and_quantization_are_warned_about() {
        let png = png_bytes(8, 8, true);
        let image = decode_source_image(&png).unwrap();
        let plan = plan_import(&image, &GTA3, "txd.img", Some(EncodeFormat::Rgb888), EncodeOptions::default()).unwrap();
        assert!(
            plan.warnings.iter().any(|w| w.contains("alpha")),
            "{:?}",
            plan.warnings
        );

        let plan = plan_import(&image, &GTA3, "gta3.img", Some(EncodeFormat::Pal4), EncodeOptions::default()).unwrap();
        assert!(
            plan.warnings.iter().any(|w| w.contains("quantized")),
            "{:?}",
            plan.warnings
        );
    }

    #[test]
    fn replace_plan_round_trips_through_the_splice() {
        // Start from a generated PAL8 TXD, convert it to 888, splice,
        // parse back, and check the pixels survive within quantization.
        let mut px = Vec::new();
        for y in 0..16u32 {
            for x in 0..16u32 {
                px.extend([(x * 15) as u8, (y * 15) as u8, 64, 255]);
            }
        }
        let encoded =
            encode_texture(&px, 16, 16, EncodeFormat::Pal8, PLATFORM_D3D8, EncodeOptions::default())
                .unwrap();
        let native = txd_writer::native_from_encoded(&encoded, PLATFORM_D3D8, "grass", "");
        let txd = single_texture_txd(native, 0x1003_FFFF);

        let plan = plan_conversion(&txd, 0, &VC, "gta3.img", Some(EncodeFormat::Rgb888), EncodeOptions::default()).unwrap();
        assert_eq!(plan.width, 16);
        let converted = apply_replace(&txd, 0, &plan).unwrap();
        let parsed = parse_txd(&converted).unwrap();
        assert_eq!(parsed.textures.len(), 1);
        assert_eq!(parsed.textures[0].diffuse_name, "grass");
        let rgba = parsed.textures[0].decode_rgba().unwrap();
        let error: u64 = rgba
            .iter()
            .zip(plan.preview_rgba.iter())
            .map(|(a, b)| u64::from(a.abs_diff(*b)))
            .sum();
        assert_eq!(error, 0, "preview must match what the file decodes to");
    }

    #[test]
    fn new_txd_plan_builds_a_parseable_dictionary() {
        let png = png_bytes(16, 16, false);
        let image = decode_source_image(&png).unwrap();
        let plan = plan_import(&image, &SA, "gta3.img", None, EncodeOptions::default()).unwrap();
        assert_eq!(plan.format, EncodeFormat::Dxt1);
        let txd = build_new_txd(&plan, &SA, "newtex");
        let parsed = parse_txd(&txd).expect("parse");
        assert_eq!(parsed.device_id, 2, "SA dictionaries carry device 2");
        assert_eq!(parsed.rw_version, 0x1803_FFFF);
        assert_eq!(parsed.textures.len(), 1);
        assert_eq!(parsed.textures[0].diffuse_name, "newtex");
        assert_eq!(parsed.textures[0].num_mipmaps, 5);
    }

    /// The DDS fixtures written by `tools/gen_converter_fixtures.py`
    /// must decode with the same path the import dialog uses.
    #[test]
    fn reads_generated_dds_fixtures_when_present() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("converter-fixtures");
        for name in ["dxt1_gradient_128.dds", "dxt5_checker_128.dds"] {
            let path = dir.join(name);
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            let image = decode_source_image(&bytes)
                .unwrap_or_else(|error| panic!("{name}: {error}"));
            assert_eq!((image.width, image.height), (128, 128), "{name}");
        }
    }

    /// Timing harness for 1024x1024 imports. Run with
    /// `cargo test --lib bench_plan_import -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn bench_plan_import() {
        use std::time::Instant;
        let size = 1024u32;
        let mut rgba = Vec::with_capacity((size * size * 4) as usize);
        let mut seed = 0x1234_5678u32;
        for y in 0..size {
            for x in 0..size {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let noise = ((seed >> 24) as u8) / 8;
                rgba.extend([
                    ((x * 255 / size) as u8).wrapping_add(noise),
                    ((y * 255 / size) as u8).wrapping_add(noise),
                    (((x + y) * 255 / (2 * size)) as u8).wrapping_add(noise),
                    255,
                ]);
            }
        }
        let image = SourceImage {
            width: size,
            height: size,
            rgba,
            has_alpha: false,
            format_label: "bench".to_string(),
            encoded_bytes: 4 * 1024 * 1024,
        };
        for (label, target, format, options) in [
            ("gta3-888", &GTA3, None, EncodeOptions::default()),
            ("vc-dxt1", &VC, None, EncodeOptions::default()),
            ("sa-dxt3", &SA, None, EncodeOptions::default()),
            (
                "gta3-dxt1-high",
                &GTA3,
                Some(EncodeFormat::Dxt1),
                EncodeOptions {
                    dxt_quality: DxtQuality::High,
                    dither: false,
                },
            ),
            (
                "gta3-pal8",
                &GTA3,
                Some(EncodeFormat::Pal8),
                EncodeOptions::default(),
            ),
        ] {
            let resolved = format
                .unwrap_or_else(|| default_format(target, "gta3.img", image.has_alpha).unwrap());
            let platform = target_platform(target);
            let start = Instant::now();
            let encoded = encode_texture(&image.rgba, size, size, resolved, platform, options)
                .expect("encode");
            let encode_ms = start.elapsed().as_secs_f64() * 1000.0;
            let start = Instant::now();
            let _ = decode_encoded(&encoded, platform).expect("decode");
            let decode_ms = start.elapsed().as_secs_f64() * 1000.0;
            let start = Instant::now();
            let plan = plan_import(&image, target, "gta3.img", format, options).expect("plan");
            let plan_ms = start.elapsed().as_secs_f64() * 1000.0;
            eprintln!(
                "{label}: encode {encode_ms:.0} ms, preview {decode_ms:.0} ms, plan {plan_ms:.0} ms ({} bytes out)",
                plan.output_bytes
            );
        }
    }

    /// The full retail gate: take real textures out of the III/VC/SA
    /// archives, convert each to its target's default dialect, and
    /// require the result to parse, classify native, and decode.
    #[test]
    fn retail_textures_convert_to_native_dialects() {
        use crate::archive::ArchiveInfo;
        use crate::compat::games::{classify, profile_by_id, Verdict};
        use crate::parser::{ImgParser, ImgVersion, PcV1Parser, PcV2Parser, detect_version};

        let Some(root) = crate::test_paths::corpus_root() else {
            return;
        };
        let archives = [
            ("gta3", root.join("Gta_3_img/models/gta3.img")),
            ("gta3", root.join("Gta_3_img/models/txd.img")),
            ("vc", root.join("Grand Theft Auto Vice City/models/gta3.img")),
            ("sa", root.join("GTA San Andreas/models/gta3.img")),
            ("sa", root.join("GTA San Andreas/models/player.img")),
            ("sa", root.join("GTA San Andreas/models/cutscene.img")),
        ];

        let mut checked = 0usize;
        for (target_id, path) in archives {
            if !path.exists() {
                continue;
            }
            let target = profile_by_id(target_id).unwrap();
            let version = detect_version(&path);
            let mut archive = ArchiveInfo::new(target_id.to_string(), false, version);
            archive.path = Some(path.clone());
            match version {
                ImgVersion::One => PcV1Parser.open(&mut archive).expect("open v1"),
                ImgVersion::Two => PcV2Parser.open(&mut archive).expect("open v2"),
                _ => continue,
            }
            for (idx, entry) in archive.entries.iter().enumerate() {
                if !entry.file_name_lower.ends_with(".txd") || idx % 499 != 0 {
                    continue;
                }
                let Ok(bytes) = crate::parser::read_entry_data(&archive, entry) else {
                    continue;
                };
                let Ok(txd) = parse_txd(&bytes) else {
                    continue;
                };
                for texture_index in 0..txd.textures.len() {
                    let Ok(plan) = plan_conversion(
                        &bytes,
                        texture_index,
                        target,
                        &archive.file_name,
                        None,
                        EncodeOptions::default(),
                    ) else {
                        continue;
                    };
                    let converted = apply_replace(&bytes, texture_index, &plan).expect("apply");
                    let parsed = parse_txd(&converted).expect("parse converted");
                    let new = &parsed.textures[texture_index];
                    let profile = RasterProfile::from_native(new);
                    let report = classify(target, &profile);
                    assert_eq!(
                        report.verdict,
                        Verdict::Native,
                        "{target_id} / {} texture {texture_index}: {} ({})",
                        entry.file_name,
                        new.format_name(),
                        report.note
                    );
                    let pixels = new.decode_rgba().expect("decode converted");
                    assert_eq!(pixels.len(), new.width as usize * new.height as usize * 4);
                    checked += 1;
                }
            }
        }
        eprintln!("retail conversion gate: {checked} textures converted and verified");
        assert!(checked > 0, "corpus root is set but no archives were found");
    }
}
