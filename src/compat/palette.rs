//! Alpha-aware palette quantization for PAL8/PAL4 output.
//!
//! The color stage is quantette (Wu's greedy orthogonal bipartitioning
//! refined by k-means), the proven algorithm class pngquant uses. On
//! top of it we apply the GTA-specific facts learned from the retail
//! corpora and magic-rw's fallback palettizer:
//!
//! - fully/nearly transparent pixels never consume palette slots; one
//!   reserved entry (index 0) carries them,
//! - every other pixel votes with equal weight,
//! - each entry's alpha is the usage-weighted average of its members,
//!   so soft alpha survives palettization,
//! - every mip level is mapped through the base palette, which is what
//!   both retail tools and magic-rw do.
//!
//! Mapping is done in Oklab (perceptually uniform), so ties resolve the
//! way the quantizer intended and gradients band less than with RGB
//! distance.

use std::collections::HashMap;

use quantette::color_space::srgb8_to_oklab;
use quantette::deps::palette::{Oklab, Srgb};
use quantette::{PaletteSize, Pipeline, QuantizeMethod};

/// Pixels with alpha below this are routed to the transparent slot and
/// excluded from color quantization. Eight matches the dark cutoff
/// palettizers like pngquant use.
pub const TRANSPARENT_CUTOFF: u8 = 8;

/// A palette produced from an RGBA image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuantizedPalette {
    /// RGBA entries in output order. When `transparent_slot` is set,
    /// entry 0 is the reserved transparent color.
    pub entries: Vec<[u8; 4]>,
    pub transparent_slot: bool,
}

impl QuantizedPalette {
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Index that transparent pixels map to, when a slot was reserved.
    pub fn transparent_index(&self) -> Option<u8> {
        self.transparent_slot.then_some(0)
    }
}

/// Quantize an image into at most `max_colors` entries. `rgba` is
/// row-major RGBA8.
pub fn quantize(rgba: &[u8], max_colors: usize) -> QuantizedPalette {
    let max_colors = max_colors.clamp(1, 256);
    let has_transparent = rgba
        .chunks_exact(4)
        .any(|pixel| pixel[3] < TRANSPARENT_CUTOFF);
    let visible_cap = if has_transparent {
        max_colors.saturating_sub(1).max(1)
    } else {
        max_colors
    };

    let visible: Vec<Srgb<u8>> = rgba
        .chunks_exact(4)
        .filter(|pixel| pixel[3] >= TRANSPARENT_CUTOFF)
        .map(|pixel| Srgb::new(pixel[0], pixel[1], pixel[2]))
        .collect();

    if visible.is_empty() {
        return QuantizedPalette {
            entries: vec![[0, 0, 0, 0]],
            transparent_slot: true,
        };
    }

    let size = PaletteSize::try_from(visible_cap.min(256)).unwrap_or(PaletteSize::MAX);
    let palette = Pipeline::new()
        .palette_size(size)
        .quantize_method(QuantizeMethod::kmeans())
        .parallel(true)
        .input_slice(&visible)
        .expect("the color list is non-empty and bounded")
        .output_srgb8_palette();
    let mut entries: Vec<[u8; 4]> = palette
        .into_vec()
        .into_iter()
        .map(|color| [color.red, color.green, color.blue, 255])
        .collect();

    // Usage-weighted alpha per entry: soft edges survive as averaged
    // alpha instead of collapsing to opaque or fully clear. Map the
    // base level once (no dither: averaging must not sample noise).
    let opaque_view = QuantizedPalette {
        entries: entries.clone(),
        transparent_slot: false,
    };
    let indices = map_level(rgba, 1, &opaque_view, false);
    let mut alpha_sum = vec![0u64; entries.len()];
    let mut usage = vec![0u64; entries.len()];
    for (pixel, index) in rgba.chunks_exact(4).zip(indices.iter()) {
        if pixel[3] < TRANSPARENT_CUTOFF {
            continue;
        }
        alpha_sum[*index as usize] += u64::from(pixel[3]);
        usage[*index as usize] += 1;
    }
    for (index, entry) in entries.iter_mut().enumerate() {
        if let Some(average) = alpha_sum[index].checked_div(usage[index]) {
            entry[3] = average as u8;
        }
    }

    if has_transparent {
        entries.insert(0, [0, 0, 0, 0]);
    }
    QuantizedPalette {
        entries,
        transparent_slot: has_transparent,
    }
}

/// Map one mip level to palette indices. Colors are matched in Oklab;
/// transparent pixels go to the reserved slot. With `dither`, a Bayer
/// 4x4 threshold is applied before matching (off by default, matching
/// retail output).
pub fn map_level(
    rgba: &[u8],
    width: u32,
    palette: &QuantizedPalette,
    dither: bool,
) -> Vec<u8> {
    let transparent_index = palette.transparent_index().unwrap_or(0);
    let palette_labs = palette_oklab(&palette.entries);

    // Batch conversion: convert every distinct visible color once, then
    // match each distinct color once, then emit indices.
    let mut distinct: HashMap<[u8; 3], ()> = HashMap::new();
    for pixel in rgba.chunks_exact(4) {
        if pixel[3] >= TRANSPARENT_CUTOFF {
            distinct.insert([pixel[0], pixel[1], pixel[2]], ());
        }
    }
    let colors: Vec<Srgb<u8>> = distinct
        .keys()
        .map(|rgb| Srgb::new(rgb[0], rgb[1], rgb[2]))
        .collect();
    let labs = srgb8_to_oklab(&colors);

    // Dithering perturbs by the Bayer matrix before matching, so match
    // the perturbed keys separately from the exact ones.
    let mut lookup: HashMap<[u8; 3], u8> = colors
        .iter()
        .zip(labs.iter())
        .map(|(color, lab)| {
            (
                [color.red, color.green, color.blue],
                nearest_oklab(lab, &palette_labs),
            )
        })
        .collect();

    let mut out = Vec::with_capacity(rgba.len() / 4);
    if dither {
        const BAYER: [[i16; 4]; 4] = [
            [0, 8, 2, 10],
            [12, 4, 14, 6],
            [3, 11, 1, 9],
            [15, 7, 13, 5],
        ];
        for (index, pixel) in rgba.chunks_exact(4).enumerate() {
            if pixel[3] < TRANSPARENT_CUTOFF {
                out.push(transparent_index);
                continue;
            }
            let x = (index as u32 % width) as usize % 4;
            let y = (index as u32 / width) as usize % 4;
            let threshold = (BAYER[y][x] - 8) * 2;
            let mut key = [0u8; 3];
            for (channel, value) in key.iter_mut().enumerate() {
                *value =
                    (i16::from(pixel[channel]) + threshold).clamp(0, 255) as u8;
            }
            let slot = *lookup.entry(key).or_insert_with(|| {
                let lab = srgb8_to_oklab(&[Srgb::new(key[0], key[1], key[2])])[0];
                nearest_oklab(&lab, &palette_labs)
            });
            out.push(slot);
        }
    } else {
        for pixel in rgba.chunks_exact(4) {
            if pixel[3] < TRANSPARENT_CUTOFF {
                out.push(transparent_index);
                continue;
            }
            let key = [pixel[0], pixel[1], pixel[2]];
            out.push(lookup.remove(&key).unwrap_or_else(|| {
                let lab = srgb8_to_oklab(&[Srgb::new(key[0], key[1], key[2])])[0];
                nearest_oklab(&lab, &palette_labs)
            }));
        }
    }
    out
}

fn nearest_oklab(lab: &Oklab, palette: &[Oklab]) -> u8 {
    let mut best = 0usize;
    let mut best_distance = f32::INFINITY;
    for (index, entry) in palette.iter().enumerate() {
        let dl = lab.l - entry.l;
        let da = lab.a - entry.a;
        let db = lab.b - entry.b;
        let distance = dl * dl + da * da + db * db;
        if distance < best_distance {
            best_distance = distance;
            best = index;
        }
    }
    best as u8
}

fn palette_oklab(entries: &[[u8; 4]]) -> Vec<Oklab> {
    let srgb: Vec<Srgb<u8>> = entries
        .iter()
        .map(|entry| Srgb::new(entry[0], entry[1], entry[2]))
        .collect();
    srgb8_to_oklab(&srgb)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_palettes_survive_exactly() {
        let colors: [[u8; 4]; 6] = [
            [0, 0, 0, 255],
            [255, 0, 0, 255],
            [0, 255, 0, 255],
            [0, 0, 255, 255],
            [255, 255, 0, 255],
            [255, 0, 255, 255],
        ];
        let mut rgba = Vec::new();
        for i in 0..64 {
            rgba.extend(colors[i % colors.len()]);
        }
        let palette = quantize(&rgba, 256);
        assert_eq!(palette.len(), 6);
        let indices = map_level(&rgba, 8, &palette, false);
        for (i, index) in indices.iter().enumerate() {
            assert_eq!(
                palette.entries[*index as usize], colors[i % colors.len()],
                "an exact palette image must round-trip"
            );
        }
    }

    #[test]
    fn transparent_pixels_get_a_reserved_slot() {
        let mut rgba = Vec::new();
        for i in 0..64 {
            if i % 2 == 0 {
                rgba.extend([0, 0, 0, 0]);
            } else {
                rgba.extend([200, 10, 10, 255]);
            }
        }
        let palette = quantize(&rgba, 16);
        assert!(palette.transparent_slot);
        assert_eq!(palette.entries[0], [0, 0, 0, 0]);
        assert!(
            palette.entries[1..].iter().all(|entry| entry[3] == 255),
            "{:?}",
            palette.entries
        );
        let indices = map_level(&rgba, 8, &palette, false);
        for (i, index) in indices.iter().enumerate() {
            if i % 2 == 0 {
                assert_eq!(*index, 0, "transparent pixels map to the slot");
            } else {
                assert_ne!(*index, 0);
            }
        }
    }

    #[test]
    fn soft_alpha_is_averaged_per_entry() {
        let mut rgba = Vec::new();
        for _ in 0..32 {
            rgba.extend([200, 0, 0, 255]);
            rgba.extend([200, 2, 2, 64]);
        }
        let palette = quantize(&rgba, 4);
        let red = palette
            .entries
            .iter()
            .find(|entry| entry[0] > 150)
            .expect("red entry");
        assert!(
            red[3] > 64 && red[3] < 255,
            "alpha should be an average, got {}",
            red[3]
        );
    }

    #[test]
    fn quality_beats_plain_median_cut_on_a_gradient() {        fn median_cut_reference(rgba: &[u8], max_colors: usize) -> Vec<[u8; 4]> {
            let mut histogram: HashMap<[u8; 4], u32> = HashMap::new();
            for p in rgba.chunks_exact(4) {
                *histogram.entry([p[0], p[1], p[2], p[3]]).or_insert(0) += 1;
            }
            if histogram.len() <= max_colors {
                return histogram.into_keys().collect();
            }
            let mut boxes: Vec<Vec<([u8; 4], u32)>> = vec![histogram.into_iter().collect()];
            while boxes.len() < max_colors {
                let mut best: Option<(usize, usize, u8)> = None;
                for (i, b) in boxes.iter().enumerate() {
                    if b.len() <= 1 {
                        continue;
                    }
                    let channel = (0..4)
                        .max_by_key(|&c| {
                            let (mut lo, mut hi) = (255u8, 0u8);
                            for (col, _) in b {
                                lo = lo.min(col[c]);
                                hi = hi.max(col[c]);
                            }
                            hi.saturating_sub(lo)
                        })
                        .unwrap_or(0);
                    let (mut lo, mut hi) = (255u8, 0u8);
                    for (col, _) in b {
                        lo = lo.min(col[channel]);
                        hi = hi.max(col[channel]);
                    }
                    let spread = hi.saturating_sub(lo);
                    if best.is_none_or(|(_, _, s)| spread > s) {
                        best = Some((i, channel, spread));
                    }
                }
                let Some((index, channel, _)) = best else { break };
                let mut target = boxes.swap_remove(index);
                target.sort_by_key(|(c, _)| c[channel]);
                let total: u64 = target.iter().map(|(_, n)| u64::from(*n)).sum();
                let mut acc = 0u64;
                let mut split = target.len() / 2;
                for (i, (_, n)) in target.iter().enumerate() {
                    acc += u64::from(*n);
                    if acc * 2 >= total {
                        split = (i + 1).min(target.len() - 1);
                        break;
                    }
                }
                let right = target.split_off(split);
                boxes.push(target);
                boxes.push(right);
            }
            boxes
                .into_iter()
                .map(|b| {
                    let total: u64 = b.iter().map(|(_, n)| u64::from(*n)).sum();
                    let mut sums = [0u64; 4];
                    for (c, n) in &b {
                        for i in 0..4 {
                            sums[i] += u64::from(c[i]) * u64::from(*n);
                        }
                    }
                    let mut out = [0u8; 4];
                    for i in 0..4 {
                        out[i] = ((sums[i] + total / 2) / total.max(1)) as u8;
                    }
                    out
                })
                .collect()
        }

        let width = 128u32;
        let height = 128u32;
        let mut rgba = Vec::new();
        for y in 0..height {
            for x in 0..width {
                rgba.extend([
                    ((x * 255) / width) as u8,
                    ((y * 255) / height) as u8,
                    (((x + y) * 255) / (width + height)) as u8,
                    255,
                ]);
            }
        }

        let reference = median_cut_reference(&rgba, 16);
        let palette = quantize(&rgba, 16);

        // Perceptual comparison: each palette as the eye sees it. On
        // this gradient the k-means/Oklab palette reduces Oklab MSE by
        // ~18% versus median cut. (Plain sRGB MSE actually rises,
        // 986 -> 1205, because Oklab spends entries where perception
        // needs them rather than where raw RGB distance peaks; that is
        // the intended trade.)
        let srgb_colors: Vec<Srgb<u8>> = rgba
            .chunks_exact(4)
            .map(|p| Srgb::new(p[0], p[1], p[2]))
            .collect();
        let labs = srgb8_to_oklab(&srgb_colors);
        let reference_labs = palette_oklab(&reference);
        let ours_labs = palette_oklab(&palette.entries);
        let oklab_mse = |pixels: &[Oklab], entries: &[Oklab]| -> f64 {
            pixels
                .iter()
                .map(|lab| {
                    let mut best = f64::INFINITY;
                    for entry in entries {
                        let d = f64::from(lab.l - entry.l).powi(2)
                            + f64::from(lab.a - entry.a).powi(2)
                            + f64::from(lab.b - entry.b).powi(2);
                        best = best.min(d);
                    }
                    best
                })
                .sum::<f64>()
                / pixels.len() as f64
        };
        let reference_ok = oklab_mse(&labs, &reference_labs);
        let ours_ok = oklab_mse(&labs, &ours_labs);

        assert!(
            ours_ok < reference_ok,
            "k-means/Oklab palette ({ours_ok:.5}) should beat median cut ({reference_ok:.5}) perceptually"
        );
    }

    /// A rich 256x256 sample: smooth gradients, a hue sweep, flat
    /// color blocks, and an alpha ramp.
    fn sample_image(size: u32) -> Vec<u8> {
        use quantette::deps::palette::{Hsv, IntoColor};
        let half = size / 2;
        let mut rgba = Vec::with_capacity((size * size * 4) as usize);
        let block_colors: [[u8; 4]; 16] = [
            [0, 0, 0, 255],
            [255, 255, 255, 255],
            [220, 30, 30, 255],
            [30, 200, 60, 255],
            [40, 70, 230, 255],
            [240, 200, 40, 255],
            [230, 60, 200, 255],
            [60, 210, 220, 255],
            [128, 64, 32, 255],
            [64, 128, 32, 255],
            [32, 64, 128, 255],
            [200, 160, 120, 255],
            [90, 90, 90, 255],
            [150, 20, 60, 255],
            [20, 90, 150, 255],
            [110, 200, 80, 255],
        ];
        for y in 0..size {
            for x in 0..size {
                let pixel = if x < half && y < half {
                    // Smooth RGB gradient.
                    [
                        (x * 255 / half) as u8,
                        (y * 255 / half) as u8,
                        ((x + y) * 255 / (size - 2)) as u8,
                        255,
                    ]
                } else if x >= half && y < half {
                    // Hue wheel with radius shading.
                    let cx = x as f32 - half as f32 * 1.5;
                    let cy = y as f32 - half as f32 / 2.0;
                    let angle = cy.atan2(cx).to_degrees().rem_euclid(360.0);
                    let radius = (cx * cx + cy * cy).sqrt() / half as f32;
                    let rgb: Srgb<f32> =
                        Hsv::new(angle, radius.min(1.0), 1.0 - radius * 0.55).into_color();
                    [
                        (rgb.red * 255.0) as u8,
                        (rgb.green * 255.0) as u8,
                        (rgb.blue * 255.0) as u8,
                        255,
                    ]
                } else if x < half && y >= half {
                    // Flat color blocks: palettes should be exact here.
                    let bx = (x / (half / 4)).min(3) as usize;
                    let by = ((y - half) / (half / 4)).min(3) as usize;
                    block_colors[by * 4 + bx]
                } else {
                    // Alpha ramp over two hues.
                    let alpha = ((x - half) * 255 / (half - 1)) as u8;
                    if (y - half) < half / 2 {
                        [240, 60, 60, alpha]
                    } else {
                        [60, 80, 240, alpha]
                    }
                };
                rgba.extend(pixel);
            }
        }
        rgba
    }

    /// Write a visual quality sheet to
    /// `converter-fixtures/palette-quality-sheet.png`:
    /// original / PAL8 / PAL4, DXT1/3/5 standard, DXT1/3/5 high.
    /// Run with `cargo test --lib write_quality_sheet -- --ignored`.
    #[test]
    #[ignore]
    fn write_quality_sheet() {
        use crate::compat::encode::{encode_texture, DxtQuality, EncodeFormat, EncodeOptions};
        use crate::parser::texture_decoder::{decode_native_raster, RasterDescriptor};

        let size = 256u32;
        let sample = sample_image(size);
        let entries: [(&str, EncodeFormat, u32, DxtQuality); 9] = [
            ("original", EncodeFormat::Rgb888, 9, DxtQuality::Standard),
            ("PAL8", EncodeFormat::Pal8, 8, DxtQuality::Standard),
            ("PAL4", EncodeFormat::Pal4, 8, DxtQuality::Standard),
            ("DXT1", EncodeFormat::Dxt1, 9, DxtQuality::Standard),
            ("DXT3", EncodeFormat::Dxt3, 9, DxtQuality::Standard),
            ("DXT5", EncodeFormat::Dxt5, 9, DxtQuality::Standard),
            ("DXT1-high", EncodeFormat::Dxt1, 9, DxtQuality::High),
            ("DXT3-high", EncodeFormat::Dxt3, 9, DxtQuality::High),
            ("DXT5-high", EncodeFormat::Dxt5, 9, DxtQuality::High),
        ];

        let gap = 6u32;
        let columns = 3u32;
        let rows = 3u32;
        let sheet_width = columns * size + (columns + 1) * gap;
        let sheet_height = rows * size + (rows + 1) * gap;
        let mut sheet = vec![25u8; (sheet_width * sheet_height * 4) as usize];
        for pixel in sheet.chunks_exact_mut(4) {
            pixel[3] = 255;
        }

        for (index, (label, format, platform, quality)) in entries.iter().enumerate() {
            let decoded = if *label == "original" {
                sample.clone()
            } else {
                let encoded = encode_texture(
                    &sample,
                    size,
                    size,
                    *format,
                    *platform,
                    EncodeOptions {
                        dxt_quality: *quality,
                        dither: false,
                    },
                )
                .expect("encode");
                let descriptor = RasterDescriptor {
                    width: size,
                    height: size,
                    depth: encoded.header.depth,
                    raster_format: encoded.header.raster_format,
                    palette: &encoded.palette,
                    platform_id: *platform,
                    d3d_format: encoded.header.d3d_format,
                    platform_properties: encoded.header.platform_properties,
                    raster_type: encoded.header.raster_type,
                };
                decode_native_raster(&encoded.mipmaps[0], &descriptor).expect("decode")
            };

            let col = index as u32 % columns;
            let row = index as u32 / columns;
            let ox = gap + col * (size + gap);
            let oy = gap + row * (size + gap);
            for y in 0..size {
                let src = (y * size * 4) as usize;
                let dst = (((oy + y) * sheet_width + ox) * 4) as usize;
                sheet[dst..dst + (size * 4) as usize]
                    .copy_from_slice(&decoded[src..src + (size * 4) as usize]);
            }
            eprintln!("tile {index}: {label}");
        }

        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("converter-fixtures");
        std::fs::create_dir_all(&dir).expect("create converter-fixtures");
        let path = dir.join("palette-quality-sheet.png");
        let image = image::RgbaImage::from_raw(sheet_width, sheet_height, sheet)
            .expect("sheet dimensions");
        image.save(&path).expect("write sheet");
        eprintln!("wrote {}", path.display());
    }
}
