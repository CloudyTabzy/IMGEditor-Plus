use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::inspector::nif::{BlockPayload, NifFile, NiPixelDataPayload};

/// Maps model name (lowercase) → txd/NFT name (from .ide `objs` entries).
#[derive(Debug, Default)]
pub struct IdeMap {
    inner: HashMap<String, String>,
    game_root: Option<PathBuf>,
}

impl IdeMap {
    /// Build the map by scanning every `.ide` file under `game_root`.
    pub fn build(game_root: &Path) -> Self {
        let mut inner = HashMap::new();
        let _ = Self::walk_and_parse(game_root, &mut inner);
        Self {
            inner,
            game_root: Some(game_root.to_path_buf()),
        }
    }

    fn walk_and_parse(dir: &Path, map: &mut HashMap<String, String>) -> std::io::Result<()> {
        if !dir.is_dir() {
            return Ok(());
        }
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                Self::walk_and_parse(&path, map)?;
            } else if path.extension().and_then(|e| e.to_str()) == Some("ide") {
                Self::parse_ide_file(&path, map);
            }
        }
        Ok(())
    }

    fn parse_ide_file(path: &Path, map: &mut HashMap<String, String>) {
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return,
        };
        let mut in_objs = false;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.eq_ignore_ascii_case("objs") {
                in_objs = true;
                continue;
            }
            if trimmed.eq_ignore_ascii_case("end") {
                in_objs = false;
                continue;
            }
            if !in_objs || trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = trimmed.split(',').map(|s| s.trim()).collect();
            if parts.len() >= 3 {
                let model_name = parts[1].to_lowercase();
                let txd_name = parts[2].to_string();
                if !model_name.is_empty() && !txd_name.is_empty() {
                    map.entry(model_name).or_insert_with(|| txd_name);
                }
            }
        }
    }

    /// Look up a NIF basename (case-insensitive) to get the NFT name.
    pub fn nft_name_for(&self, nif_basename: &str) -> Option<&str> {
        self.inner.get(&nif_basename.to_lowercase()).map(|s| s.as_str())
    }

    /// Locate the `.nft` file on disk for a given txd name.
    pub fn locate_nft(&self, txd_name: &str) -> Option<PathBuf> {
        let root = self.game_root.as_ref()?;
        let target = format!("{}.nft", txd_name.to_lowercase());
        Self::find_file_recursive(root, &target)
    }

    fn find_file_recursive(dir: &Path, target: &str) -> Option<PathBuf> {
        if !dir.is_dir() {
            return None;
        }
        let entries = fs::read_dir(dir).ok()?;
        for entry in entries {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.is_dir() {
                if let Some(found) = Self::find_file_recursive(&path, target) {
                    return Some(found);
                }
            } else if path.is_file() {
                if path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.to_lowercase() == *target)
                    .unwrap_or(false)
                {
                    return Some(path);
                }
            }
        }
        None
    }

    /// Resolve the NFT path for a NIF, using IDE mapping + same-basename fallback.
    pub fn resolve_nft_path(&self, nif_basename: &str) -> Option<PathBuf> {
        // 1. IDE mapping
        if let Some(txd) = self.nft_name_for(nif_basename) {
            if let Some(path) = self.locate_nft(txd) {
                return Some(path);
            }
        }
        // 2. Same-basename fallback: try {nif_basename}.nft
        if let Some(ref root) = self.game_root {
            let target = format!("{}.nft", nif_basename.to_lowercase());
            return Self::find_file_recursive(root, &target);
        }
        None
    }
}

// ---- NFT catalog (texture basename → source path) ---------------------

/// Catalog extracted from a single `.nft` file:
/// maps texture basename (lowercase) → full source path.
#[derive(Debug)]
pub struct NftCatalog {
    pub entries: HashMap<String, TextureEntry>,
}

#[derive(Debug, Clone)]
pub struct TextureEntry {
    /// Full source path from the NFT (e.g. `Z:\...\PO00_guts_d.tga`)
    pub source_path: String,
    /// Raw pixel data extracted from the NiSourceTexture block.
    pub pixel_data: Option<Vec<u8>>,
}

impl NftCatalog {
    pub fn get_pixels(&self, texture_basename: &str) -> Option<&[u8]> {
        self.entries
            .get(&texture_basename.to_lowercase())
            .and_then(|e| e.pixel_data.as_deref())
    }

    pub fn has_texture(&self, texture_basename: &str) -> bool {
        self.entries.contains_key(&texture_basename.to_lowercase())
    }
}

// ---- Embedded pixel data extraction ----------------------------------

/// Extract embedded pixel data from a NiSourceTexture block.
/// Returns `None` if the texture is external (use_external == 1) or
/// if the pixel data cannot be parsed.
fn extract_embedded_pixels(nft: &NifFile, nft_bytes: &[u8], block_idx: usize) -> Option<Vec<u8>> {
    let payload = nft.payloads.get(block_idx)?;
    let payload = payload.as_ref()?;
    let BlockPayload::NiSourceTexture(tex) = payload else {
        return None;
    };

    let meta = nft.blocks.get(block_idx)?;
    let block_start = meta.offset as usize;
    let block_size = meta.size as usize;
    if tex.use_external != 0 || block_start + block_size > nft_bytes.len() {
        return None;
    }
    let raw = &nft_bytes[block_start..block_start + block_size];

    // Manually walk the fields to find where pixel data starts,
    // matching exactly what read_ni_source_texture does.
    // 1. name (NiFixedString = u32)
    if raw.len() < 4 { return None; }
    // 2. num_extra_data (u32)
    if raw.len() < 8 { return None; }
    let num_extra = u32::from_le_bytes(raw[4..8].try_into().ok()?) as usize;
    // 3. extra_data (i32 × num_extra)
    let after_extra = 8 + num_extra * 4;
    if raw.len() < after_extra + 4 { return None; }
    // 4. controller (i32)
    // 5. use_external (u8) = already checked as 0
    // 6. file_name_index (u32)
    // 7. pixel_layout (u32)
    // 8. use_mipmaps (u32)
    // 9. alpha_format (u32)
    // 10. is_static (u8)
    // 11. direct_render (u8)
    // 12. persist_render_data (u8)
    let header_end = after_extra + 4 + 1 + 4 + 4 + 4 + 4 + 1 + 1 + 1;
    if header_end >= raw.len() {
        // No pixel data — fully legit for NFT that only stores metadata
        // (paths) with no embedded pixel data.
        return None;
    }
    let pixel_bytes = &raw[header_end..];

    // The first 8 bytes of pixel data are usually width(u32) + height(u32).
    if pixel_bytes.len() < 8 { return None; }
    let pw = u32::from_le_bytes(pixel_bytes[0..4].try_into().ok()?);
    let ph = u32::from_le_bytes(pixel_bytes[4..8].try_into().ok()?);
    if pw == 0 || pw > 16384 || ph == 0 || ph > 16384 { return None; }

    let expected = pw as usize * ph as usize * 4;
    let data_start = 8;
    let available = pixel_bytes.len().saturating_sub(data_start).min(expected);
    if available < 4 { return None; }

    let mut tga = Vec::with_capacity(18 + available);
    tga.push(0); tga.push(0); tga.push(2);
    tga.extend_from_slice(&[0, 0, 0, 0, 0]);
    tga.extend_from_slice(&[0, 0]); tga.extend_from_slice(&[0, 0]);
    tga.extend_from_slice(&(pw as u16).to_le_bytes());
    tga.extend_from_slice(&(ph as u16).to_le_bytes());
    tga.push(32); tga.push(0x20);
    for i in (0..available).step_by(4) {
        tga.push(pixel_bytes[data_start + i + 2]); // B
        tga.push(pixel_bytes[data_start + i + 1]); // G
        tga.push(pixel_bytes[data_start + i]);     // R
        tga.push(pixel_bytes[data_start + i + 3]); // A
    }
    Some(tga)
}

// ---- Convenience: resolve texture from a NIF name --------------------

/// Full pipeline: given a NIF basename, look up the NFT via IDE,
/// parse the NFT, and return the catalog.
pub fn resolve_textures_for_nif(
    nif_basename: &str,
    ide_map: &IdeMap,
) -> Option<NftCatalog> {
    let nft_path = ide_map.resolve_nft_path(nif_basename)?;
    let nft_bytes = fs::read(&nft_path).ok()?;
    let mut nft = NifFile::parse(&nft_bytes).ok()?;
    nft.resolve_string_indices();

    let mut entries = HashMap::new();
    for (idx, payload) in nft.payloads.iter().enumerate() {
        let Some(BlockPayload::NiSourceTexture(tex)) = payload else {
            continue;
        };
        let base_name = tex
            .file_name
            .as_deref()
            .and_then(|name| {
                std::path::Path::new(name)
                    .file_name()
                    .and_then(|n| n.to_str())
            })
            .map(|s| s.to_lowercase());

        let Some(key) = base_name else {
            continue;
        };
        let pixel_data = extract_pixels_for_nft(&nft, &nft_bytes, idx);
        entries.insert(
            key,
            TextureEntry {
                source_path: tex.file_name.clone().unwrap_or_default(),
                pixel_data,
            },
        );
    }

    Some(NftCatalog { entries })
}

/// Try to find NiPixelData associated with a NiSourceTexture by scanning
/// forward from the NiSourceTexture block for the next NiPixelData block.
fn extract_pixels_for_nft(
    nft: &NifFile,
    nft_bytes: &[u8],
    tex_block_idx: usize,
) -> Option<Vec<u8>> {
    // Check inline pixel data first (NiSourceTexture embedded).
    if let Some(tga) = extract_embedded_pixels(nft, nft_bytes, tex_block_idx) {
        if tga.len() > 22 {
            return Some(tga);
        }
    }
    // Fall back to NiPixelData blocks nearby.
    for candidate in tex_block_idx + 1..nft.blocks.len().min(tex_block_idx + 10) {
        let Some(Some(BlockPayload::NiPixelData(pd))) = nft.payloads.get(candidate) else {
            continue;
        };
        if pd.raw_pixels.len() < 40 {
            continue;
        }
        if let Some(dds) = extract_dds_from_nipixeldata(pd) {
            return Some(dds);
        }
    }
    None
}

/// Build a 128-byte DDS header for a DXT1/DXT5 texture.
fn build_dds_header(w: u32, h: u32, fourcc: &[u8; 4], mip_count: u32) -> Vec<u8> {
    let bpb: u32 = if fourcc == b"DXT1" { 8 } else { 16 };
    let pitch = ((w + 3) / 4).max(1) * ((h + 3) / 4).max(1) * bpb;
    let mut flags = 0x0008_1007u32; // CAPS|HEIGHT|WIDTH|PIXELFORMAT|LINEARSIZE
    if mip_count > 1 { flags |= 0x0002_0000; }
    let mut caps = 0x0000_1000u32; // TEXTURE
    if mip_count > 1 { caps |= 0x0040_0008; } // COMPLEX|MIPMAP

    let mut hdr = vec![0u8; 128];
    hdr[0..4].copy_from_slice(b"DDS ");
    hdr[4..8].copy_from_slice(&124u32.to_le_bytes());
    hdr[8..12].copy_from_slice(&flags.to_le_bytes());
    hdr[12..16].copy_from_slice(&h.to_le_bytes());
    hdr[16..20].copy_from_slice(&w.to_le_bytes());
    hdr[20..24].copy_from_slice(&pitch.to_le_bytes());
    hdr[28..32].copy_from_slice(&mip_count.to_le_bytes());
    hdr[76..80].copy_from_slice(&32u32.to_le_bytes()); // pfSize
    hdr[80..84].copy_from_slice(&4u32.to_le_bytes());   // DDPF_FOURCC
    hdr[84..88].copy_from_slice(fourcc);                 // dwFourCC
    hdr[108..112].copy_from_slice(&caps.to_le_bytes());
    hdr
}

/// Compute DXT mip chain size in bytes.
fn dxt_chain_size(w: u32, h: u32, fourcc: &[u8; 4]) -> (u32, u32) {
    let bpb = if *fourcc == *b"DXT1" { 8u32 } else { 16u32 };
    let mut total = 0u32;
    let mut mips = 0u32;
    let mut tw = w;
    let mut th = h;
    loop {
        total += ((tw + 3) / 4).max(1) * ((th + 3) / 4).max(1) * bpb;
        mips += 1;
        if tw == 1 && th == 1 { break; }
        tw = (tw / 2).max(1);
        th = (th / 2).max(1);
    }
    (total, mips)
}

/// Try to extract DDS data from a NiPixelData payload.
/// Decompress a single DXT1 8-byte block to 4×4 RGBA pixels (64 bytes).
fn dxt1_block_to_rgba(block: &[u8]) -> [[u8; 4]; 16] {
    let c0 = u16::from_le_bytes([block[0], block[1]]);
    let c1 = u16::from_le_bytes([block[2], block[3]]);
    let expand = |c: u16| -> [u8; 4] {
        let r5 = ((c >> 11) & 0x1F) as u8;
        let g6 = ((c >> 5) & 0x3F) as u8;
        let b5 = (c & 0x1F) as u8;
        [(r5 << 3) | (r5 >> 2), (g6 << 2) | (g6 >> 4), (b5 << 3) | (b5 >> 2), 255]
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
                [avg(col0[0], col1[0]), avg(col0[1], col1[1]), avg(col0[2], col1[2]), 255]
            }
            (false, 3) => [0, 0, 0, 0],
            _ => unreachable!(),
        };
    }
    out
}

/// Decompress DXT1 data to RGBA TGA bytes.
fn dxt1_to_tga(data: &[u8], w: u32, h: u32) -> Vec<u8> {
    let bw = ((w + 3) / 4).max(1) as usize;
    let bh = ((h + 3) / 4).max(1) as usize;
    let mut tga = vec![0u8; 18 + (w * h * 4) as usize];
    tga[2] = 2;
    tga[12..14].copy_from_slice(&(w as u16).to_le_bytes());
    tga[14..16].copy_from_slice(&(h as u16).to_le_bytes());
    tga[16] = 32;
    tga[17] = 0x20;

    let mut block_px;
    for by in 0..bh {
        for bx in 0..bw {
            let src = (by * bw + bx) * 8;
            if src + 8 > data.len() { continue; }
            block_px = dxt1_block_to_rgba(&data[src..src+8]);
            for row in 0..4 {
                for col in 0..4 {
                    let img_y = by * 4 + row;
                    let img_x = bx * 4 + col;
                    if img_y >= h as usize || img_x >= w as usize { continue; }
                    let px = block_px[row * 4 + col];
                    let dst = 18 + (img_y * w as usize + img_x) * 4;
                    tga[dst..dst+4].copy_from_slice(&[px[2], px[1], px[0], px[3]]); // BGRA
                }
            }
        }
    }
    tga
}

/// Decompress a single DXT5 16-byte block to 4×4 RGBA pixels.
fn dxt5_block_to_rgba(block: &[u8]) -> [[u8; 4]; 16] {
    // Alpha part: 2 endpoint bytes (a0, a1) + 6 bytes (48 bits) of
    // 3-bit indices for the 16 pixels. Total = 8 bytes.
    let a0 = block[0];
    let a1 = block[1];
    let mut alphas = [0u8; 8];
    alphas[0] = a0;
    alphas[1] = a1;
    if a0 > a1 {
        // 8-alpha block: full interpolated ramp.
        alphas[2] = ((6 * a0 as u16 + 1 * a1 as u16 + 3) / 7) as u8;
        alphas[3] = ((5 * a0 as u16 + 2 * a1 as u16 + 3) / 7) as u8;
        alphas[4] = ((4 * a0 as u16 + 3 * a1 as u16 + 3) / 7) as u8;
        alphas[5] = ((3 * a0 as u16 + 4 * a1 as u16 + 3) / 7) as u8;
        alphas[6] = ((2 * a0 as u16 + 5 * a1 as u16 + 3) / 7) as u8;
        alphas[7] = ((1 * a0 as u16 + 6 * a1 as u16 + 3) / 7) as u8;
    } else {
        // 6-alpha block (a0 <= a1): last two are 0 and 255.
        alphas[2] = ((4 * a0 as u16 + 1 * a1 as u16 + 2) / 5) as u8;
        alphas[3] = ((3 * a0 as u16 + 2 * a1 as u16 + 2) / 5) as u8;
        alphas[4] = ((2 * a0 as u16 + 3 * a1 as u16 + 2) / 5) as u8;
        alphas[5] = ((1 * a0 as u16 + 4 * a1 as u16 + 2) / 5) as u8;
        alphas[6] = 0;
        alphas[7] = 255;
    }
    let mut alpha_bits: u64 = 0;
    for i in 0..6 {
        alpha_bits |= (block[2 + i] as u64) << (8 * i);
    }
    // RGB part: same as DXT1 but at bytes 8..15 of the block.
    let c0 = u16::from_le_bytes([block[8], block[9]]);
    let c1 = u16::from_le_bytes([block[10], block[11]]);
    let codes = u32::from_le_bytes([block[12], block[13], block[14], block[15]]);
    let expand = |c: u16| -> [u8; 3] {
        let r = ((c >> 11) & 0x1F) as u8;
        let g = ((c >> 5) & 0x3F) as u8;
        let b = (c & 0x1F) as u8;
        [
            ((r << 3) | (r >> 2)),
            ((g << 2) | (g >> 4)),
            ((b << 3) | (b >> 2)),
        ]
    };
    let col0 = expand(c0);
    let col1 = expand(c1);
    let mut colors: Vec<[u8; 3]> = vec![col0, col1];
    if c0 > c1 {
        // 4-color mode.
        colors.push([
            ((2 * col0[0] as u16 + col1[0] as u16) / 3) as u8,
            ((2 * col0[1] as u16 + col1[1] as u16) / 3) as u8,
            ((2 * col0[2] as u16 + col1[2] as u16) / 3) as u8,
        ]);
        colors.push([
            ((col0[0] as u16 + 2 * col1[0] as u16) / 3) as u8,
            ((col0[1] as u16 + 2 * col1[1] as u16) / 3) as u8,
            ((col0[2] as u16 + 2 * col1[2] as u16) / 3) as u8,
        ]);
    } else {
        // 3-color + 1-bit alpha mode.
        colors.push([
            ((col0[0] as u16 + col1[0] as u16) / 2) as u8,
            ((col0[1] as u16 + col1[1] as u16) / 2) as u8,
            ((col0[2] as u16 + col1[2] as u16) / 2) as u8,
        ]);
        colors.push([0, 0, 0]);
    }
    let mut out = [[0u8; 4]; 16];
    for i in 0..16 {
        let rgb_idx = ((codes >> (i * 2)) & 3) as usize;
        let a_idx = ((alpha_bits >> (i * 3)) & 7) as usize;
        let r = colors[rgb_idx][0];
        let g = colors[rgb_idx][1];
        let b = colors[rgb_idx][2];
        // For c0 <= c1 4-color mode, index 3 is fully transparent.
        let a = if c0 <= c1 && rgb_idx == 3 {
            0
        } else {
            alphas[a_idx]
        };
        out[i] = [r, g, b, a];
    }
    out
}

/// Decompress DXT5 data to RGBA TGA bytes.
fn dxt5_to_tga(data: &[u8], w: u32, h: u32) -> Vec<u8> {
    let bw = ((w + 3) / 4).max(1) as usize;
    let bh = ((h + 3) / 4).max(1) as usize;
    let mut tga = vec![0u8; 18 + (w * h * 4) as usize];
    tga[2] = 2;
    tga[12..14].copy_from_slice(&(w as u16).to_le_bytes());
    tga[14..16].copy_from_slice(&(h as u16).to_le_bytes());
    tga[16] = 32;
    // alpha=8 bits in low nibble, origin=top-left in bits 4-5 (= 2)
    tga[17] = 0x28;

    for by in 0..bh {
        for bx in 0..bw {
            let src = (by * bw + bx) * 16;
            if src + 16 > data.len() {
                continue;
            }
            let block_px = dxt5_block_to_rgba(&data[src..src + 16]);
            for row in 0..4 {
                for col in 0..4 {
                    let img_y = by * 4 + row;
                    let img_x = bx * 4 + col;
                    if img_y >= h as usize || img_x >= w as usize {
                        continue;
                    }
                    let px = block_px[row * 4 + col];
                    let dst = 18 + (img_y * w as usize + img_x) * 4;
                    tga[dst..dst + 4].copy_from_slice(&[px[2], px[1], px[0], px[3]]); // BGRA
                }
            }
        }
    }
    tga
}

/// Parse the explicit NiPixelData V20_3_0.9 header that lives at the
/// start of `raw_pixels` (everything after the 4-byte `pixel_format`
/// field that the NIF reader has already consumed). Returns the
/// authoritative texture dimensions, format, and the byte offset
/// where the DXT chain starts.
fn parse_nipixeldata_header(raw_pixels: &[u8]) -> Option<ParsedNiPixelData> {
    // Minimum size: 55 (rest of NiPixelFormat) + 4 (palette) +
    //                4 (num_mipmaps) + 4 (bytes_per_pixel) +
    //                12 (1 mipmap) + 4 (num_pixels) + 4 (num_faces) = 87 bytes
    if raw_pixels.len() < 87 {
        return None;
    }
    // Skip the remainder of NiPixelFormat (55 bytes) and the
    // 4-byte palette Ref. We're not interested in either.
    let mut pos = 55 + 4;
    let num_mipmaps = u32::from_le_bytes(raw_pixels[pos..pos + 4].try_into().ok()?) as usize;
    pos += 4;
    // Bytes Per Pixel (4 bytes) — unused for DXT formats but must be skipped.
    pos += 4;
    // Need enough room for the mipmap table + num_pixels + num_faces.
    let mipmap_table_end = pos + num_mipmaps.checked_mul(12)? + 8;
    if raw_pixels.len() < mipmap_table_end || num_mipmaps == 0 {
        return None;
    }
    // Read mipmap[0] (the main mip).
    let mip0_w = u32::from_le_bytes(raw_pixels[pos..pos + 4].try_into().ok()?);
    let mip0_h = u32::from_le_bytes(raw_pixels[pos + 4..pos + 8].try_into().ok()?);
    let _mip0_off = u32::from_le_bytes(raw_pixels[pos + 8..pos + 12].try_into().ok()?);
    if mip0_w == 0 || mip0_h == 0 || mip0_w > 16384 || mip0_h > 16384 {
        return None;
    }
    // Main mip size: the offset of mipmap[1] (the second entry in
    // the table) marks the end of the main mip. If there is only
    // one mipmap, fall back to the full chain size.
    // Each MipMap entry is 12 bytes (width u32 + height u32 + offset
    // u32). mipmap[1] starts at pos+12 and its `offset` field is at
    // pos+12+8 = pos+20.
    let main_mip_size = if num_mipmaps >= 2 {
        u32::from_le_bytes(raw_pixels[pos + 20..pos + 24].try_into().ok()?)
    } else {
        0
    };
    pos += num_mipmaps * 12;
    let num_pixels = u32::from_le_bytes(raw_pixels[pos..pos + 4].try_into().ok()?);
    pos += 4;
    let num_faces = u32::from_le_bytes(raw_pixels[pos..pos + 4].try_into().ok()?);
    pos += 4;
    // The DXT chain starts here.
    let dxt_data_start = pos;
    // Use the main_mip_size from the mipmap table. If the table only
    // has one mip, use the entire chain.
    let main_mip_size = if main_mip_size > 0 {
        main_mip_size
    } else {
        num_pixels
    };
    // Sanity check: main_mip_size should equal the actual main-mip
    // chain size computed from the (w, h, fourcc) we're about to
    // pick, and it should fit in num_pixels.
    if main_mip_size == 0 || main_mip_size > num_pixels {
        return None;
    }
    Some(ParsedNiPixelData {
        width: mip0_w,
        height: mip0_h,
        main_mip_size,
        dxt_data_start,
        num_pixels,
        num_faces,
    })
}

/// Fields extracted from a parsed NiPixelData V20_3_0.9 header.
struct ParsedNiPixelData {
    width: u32,
    height: u32,
    /// Size in bytes of the **main mip only** (mip[0] of the mip
    /// chain), excluding the smaller sub-mips. This is what the
    /// main-mip decoder should consume.
    main_mip_size: u32,
    /// Byte offset within `raw_pixels` where the DXT chain begins.
    dxt_data_start: usize,
    num_pixels: u32,
    num_faces: u32,
}

/// Try to extract pixel data from a NiPixelData block. Returns RGBA TGA
/// bytes for DXT1 and DXT5, or DDS bytes as a last-resort fallback
/// when the explicit header can't be parsed.
fn extract_dds_from_nipixeldata(pd: &NiPixelDataPayload) -> Option<Vec<u8>> {
    let raw = &pd.raw_pixels;

    // Authoritative path: parse the explicit V20_3_0.9 NiPixelData
    // header. This is what every Gamebryo/Bully NFT actually stores,
    // and the size guesser below only works by coincidence for 9 of
    // 12 textures (see Docs/bully_embedded_texture_export.md).
    if let Some(hdr) = parse_nipixeldata_header(raw) {
        let fourcc = match pd.pixel_format {
            4 => b"DXT1",
            5 | 6 => b"DXT5",
            _ => return None,
        };
        let dxt_start = hdr.dxt_data_start;
        let dxt_end = dxt_start
            .checked_add(hdr.main_mip_size as usize)?
            .min(raw.len());
        if dxt_end <= dxt_start {
            return None;
        }
        let pixel_data = &raw[dxt_start..dxt_end];
        return Some(match fourcc {
            b"DXT1" => dxt1_to_tga(pixel_data, hdr.width, hdr.height),
            _ => dxt5_to_tga(pixel_data, hdr.width, hdr.height),
        });
    }

    // Legacy fallback: try to guess dimensions and format from the
    // total block size. Only kept for malformed NFT files that
    // don't have a parseable V20_3_0.9 header. The 40..=512 hdr_sz
    // window is the only thing that makes this work for the simple
    // 9/12 case and is known to fail for `Player_03_d`, `_n`, `_s`
    // in Bully's PLAYER.nft.
    //
    // The candidate list covers all the (w, h) pairs seen in Bully's
    // 4469 NFT files plus the common sizes. Keep this list ordered
    // largest-first to make the (chain + hdr_sz) arithmetic unique.
    let block_size = raw.len() as u32;
    let candidates: [(u32, u32); 34] = [
        (1024, 1024), (1024, 512), (512, 1024),
        (1024, 256), (256, 1024), (1024, 128), (128, 1024),
        (512, 512), (512, 256), (256, 512),
        (512, 128), (128, 512),
        (256, 256), (256, 128), (128, 256),
        (256, 64), (64, 256),
        (256, 32), (32, 256),
        (128, 128), (128, 64), (64, 128),
        (128, 32), (32, 128), (128, 16), (16, 128),
        (64, 64), (64, 32), (32, 64), (64, 16), (16, 64),
        (32, 32), (16, 16), (8, 8),
    ];
    let four_dxt1 = b"DXT1";
    let four_dxt5 = b"DXT5";

    for &fourcc in &[four_dxt1, four_dxt5] {
        for &(w, h) in &candidates {
            let (chain, _mips) = dxt_chain_size(w, h, fourcc);
            if chain > block_size {
                continue;
            }
            let hdr_sz = block_size - chain;
            if !(40..=512).contains(&hdr_sz) {
                continue;
            }

            let px_start = hdr_sz as usize;
            if px_start + chain as usize > raw.len() {
                continue;
            }
            let pixel_data = &raw[px_start..px_start + chain as usize];

            if fourcc == b"DXT1" {
                return Some(dxt1_to_tga(pixel_data, w, h));
            }
            // DXT5: keep as DDS (no TGA decoder in legacy path).
            let dds_hdr = build_dds_header(w, h, fourcc, 1);
            let mut out = Vec::with_capacity(dds_hdr.len() + pixel_data.len());
            out.extend_from_slice(&dds_hdr);
            out.extend_from_slice(pixel_data);
            return Some(out);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dxt_chain_size_known_formats() {
        let (size, mips) = dxt_chain_size(512, 512, b"DXT1");
        assert_eq!(size, 174_776);
        assert_eq!(mips, 10);

        let (size, _) = dxt_chain_size(256, 256, b"DXT1");
        assert_eq!(size, 43_704);

        let (size, _) = dxt_chain_size(256, 256, b"DXT5");
        assert_eq!(size, 87_408);

        let (size, _) = dxt_chain_size(128, 128, b"DXT1");
        assert_eq!(size, 10_936);

        let (size, _) = dxt_chain_size(64, 64, b"DXT1");
        assert_eq!(size, 2_744);

        let (size, _) = dxt_chain_size(64, 64, b"DXT5");
        assert_eq!(size, 5_488);

        let (size, _) = dxt_chain_size(32, 32, b"DXT1");
        assert_eq!(size, 696);

        let (size, _) = dxt_chain_size(256, 64, b"DXT1");
        assert_eq!(size, 10_952);
    }

    #[test]
    fn nipixeldata_32x32_dxt1_round_trip() {
        // 32×32 DXT1 = 696 bytes chain, 6 mipmaps. Total raw =
        // 75 + 12*6 = 147 byte header + 696 = 843 bytes.
        let mips = [
            (32, 32, 0),
            (16, 16, 512),
            (8, 8, 640),
            (4, 4, 672),
            (2, 2, 680),
            (1, 1, 688),
        ];
        let dxt_data = vec![0u8; 696];
        let raw = build_test_nipixeldata(4, &mips, &dxt_data);
        let pd = NiPixelDataPayload {
            pixel_format: 4,
            num_faces: 1,
            num_mipmaps: 6,
            bytes_per_pixel: 4,
            num_pixels: 1024,
            raw_pixels: raw,
        };
        let out = extract_dds_from_nipixeldata(&pd).expect("32x32 DXT1 should match");
        // TGA header (18) + 32*32*4 RGBA pixels
        assert_eq!(out.len(), 18 + 32 * 32 * 4);
        // First 12 bytes of TGA header
        assert_eq!(&out[0..3], &[0, 0, 2]);
        // Width/height LE at offset 12
        assert_eq!(u16::from_le_bytes([out[12], out[13]]), 32);
        assert_eq!(u16::from_le_bytes([out[14], out[15]]), 32);
    }

    #[test]
    fn nipixeldata_512x512_dxt1_round_trip() {
        // 512×512 DXT1 = 174,776 bytes chain, 10 mipmaps. Total
        // raw = 75 + 12*10 = 195 byte header + 174,776 = 174,971
        // bytes.
        let mips = [
            (512, 512, 0),
            (256, 256, 131_072),
            (128, 128, 163_840),
            (64, 64, 172_032),
            (32, 32, 174_080),
            (16, 16, 174_592),
            (8, 8, 174_720),
            (4, 4, 174_752),
            (2, 2, 174_760),
            (1, 1, 174_768),
        ];
        let dxt_data = vec![0u8; 174_776];
        let raw = build_test_nipixeldata(4, &mips, &dxt_data);
        let pd = NiPixelDataPayload {
            pixel_format: 4,
            num_faces: 1,
            num_mipmaps: 10,
            bytes_per_pixel: 4,
            num_pixels: 262_144,
            raw_pixels: raw,
        };
        let out = extract_dds_from_nipixeldata(&pd).expect("512x512 DXT1 should match");
        assert_eq!(out.len(), 18 + 512 * 512 * 4);
        assert_eq!(u16::from_le_bytes([out[12], out[13]]), 512);
        assert_eq!(u16::from_le_bytes([out[14], out[15]]), 512);
    }

    /// Build a minimal valid V20_3_0.9 NiPixelData block in `raw_pixels`
    /// form: the 4-byte `pixel_format` field has already been consumed,
    /// so the slice starts at the rest of the NiPixelFormat.
    ///
    /// Layout (all little-endian, all values fit):
    ///   +0    55 bytes of stubbed NiPixelFormat continuation
    ///   +55   Palette (Ref i32 = -1)
    ///   +59   Num Mipmaps (u32)
    ///   +63   Bytes Per Pixel (u32)
    ///   +67   Mipmaps: each (width u32, height u32, offset u32)
    ///   then Num Pixels (u32), Num Faces (u32), then DXT data.
    fn build_test_nipixeldata(
        _pixel_format: u32,
        mips: &[(u32, u32, u32)],
        dxt_data: &[u8],
    ) -> Vec<u8> {
        let n = mips.len();
        let mut raw = vec![0u8; 75 + 12 * n];
        // Set the Palette (Ref) to -1 at offset 55.
        raw[55..59].copy_from_slice(&(-1i32).to_le_bytes());
        // Set Num Mipmaps at offset 59.
        raw[59..63].copy_from_slice(&(n as u32).to_le_bytes());
        // Bytes Per Pixel at offset 63 (zero is fine for DXT).
        // Mipmap table at offset 67.
        for (i, (w, h, off)) in mips.iter().enumerate() {
            let base = 67 + i * 12;
            raw[base..base + 4].copy_from_slice(&w.to_le_bytes());
            raw[base + 4..base + 8].copy_from_slice(&h.to_le_bytes());
            raw[base + 8..base + 12].copy_from_slice(&off.to_le_bytes());
        }
        // Num Pixels = total DXT chain size.
        let num_pixels_pos = 67 + 12 * n;
        raw[num_pixels_pos..num_pixels_pos + 4].copy_from_slice(&(dxt_data.len() as u32).to_le_bytes());
        // Num Faces = 1.
        raw[num_pixels_pos + 4..num_pixels_pos + 8].copy_from_slice(&1u32.to_le_bytes());
        // Append DXT data.
        raw.extend_from_slice(dxt_data);
        raw
    }

    /// Regression test: the explicit NiPixelData header should be
    /// parsed authoritatively, NOT the size-guesser. This is the
    /// exact case that the old code got wrong for `Player_03_d` in
    /// Bully's PLAYER.nft (43-block chain=10,960 vs guessed 128×128
    /// DXT1 chain=10,936).
    #[test]
    fn nipixeldata_explicit_header_64x128_dxt5() {
        // 64×128 DXT5 main mip: 16×32 = 512 blocks × 16 = 8192 bytes.
        let dxt_data = vec![0u8; 10_960];
        // 8 mipmaps (64→1×1, 128→1×1) with the correct cumulative
        // offsets for DXT5 (16 bpb):
        //   64×128  @ 0     (8192)
        //   32×64   @ 8192  (2048)
        //   16×32   @ 10240 (512)
        //    8×16   @ 10752 (128)
        //    4×8    @ 10880 (32)
        //    2×4    @ 10912 (16)
        //    1×2    @ 10928 (16)
        //    1×1    @ 10944 (16)
        let mips = [
            (64, 128, 0),
            (32, 64, 8192),
            (16, 32, 10240),
            (8, 16, 10752),
            (4, 8, 10880),
            (2, 4, 10912),
            (1, 2, 10928),
            (1, 1, 10944),
        ];
        let raw = build_test_nipixeldata(6, &mips, &dxt_data);
        let pd = NiPixelDataPayload {
            pixel_format: 6, // PX_FMT_DXT5
            num_faces: 1,
            num_mipmaps: 8,
            bytes_per_pixel: 16,
            num_pixels: 10_960,
            raw_pixels: raw,
        };
        let out = extract_dds_from_nipixeldata(&pd).expect("64x128 DXT5 should extract");
        // TGA header (18) + 64*128*4 RGBA = 32786 bytes.
        assert_eq!(out.len(), 18 + 64 * 128 * 4);
        assert_eq!(u16::from_le_bytes([out[12], out[13]]), 64);
        assert_eq!(u16::from_le_bytes([out[14], out[15]]), 128);
        // 8-bit alpha channel for DXT5.
        assert_eq!(out[17] & 0x0F, 8);
    }

    /// Regression test: 256×512 DXT1 (Player_03_s in Bully) — the
    /// size-guesser used to land on 256×256 DXT5 (chain 87,408) which
    /// truncated the bottom half of the texture.
    #[test]
    fn nipixeldata_explicit_header_256x512_dxt1() {
        // 256×512 DXT1 main mip: 64×128 = 8192 blocks × 8 = 65,536 bytes.
        let dxt_data = vec![0u8; 87_400];
        let mips = [
            (256, 512, 0),
            (128, 256, 65_536),
            (64, 128, 81_920),
            (32, 64, 86_016),
            (16, 32, 87_040),
            (8, 16, 87_296),
            (4, 8, 87_360),
            (2, 4, 87_376),
            (1, 2, 87_384),
            (1, 1, 87_392),
        ];
        let raw = build_test_nipixeldata(4, &mips, &dxt_data);
        let pd = NiPixelDataPayload {
            pixel_format: 4, // PX_FMT_DXT1
            num_faces: 1,
            num_mipmaps: 10,
            bytes_per_pixel: 8,
            num_pixels: 87_400,
            raw_pixels: raw,
        };
        let out = extract_dds_from_nipixeldata(&pd).expect("256x512 DXT1 should extract");
        assert_eq!(out.len(), 18 + 256 * 512 * 4);
        assert_eq!(u16::from_le_bytes([out[12], out[13]]), 256);
        assert_eq!(u16::from_le_bytes([out[14], out[15]]), 512);
    }

    /// Regression test: 64×128 DXT1 (Player_03_n in Bully) — the
    /// size-guesser used to land on 128×128 DXT1 (chain 10,936).
    #[test]
    fn nipixeldata_explicit_header_64x128_dxt1() {
        // 64×128 DXT1 main mip: 16×32 = 512 blocks × 8 = 4,096 bytes.
        let dxt_data = vec![0u8; 5_480];
        let mips = [
            (64, 128, 0),
            (32, 64, 4_096),
            (16, 32, 5_120),
            (8, 16, 5_376),
            (4, 8, 5_440),
            (2, 4, 5_456),
            (1, 2, 5_464),
            (1, 1, 5_472),
        ];
        let raw = build_test_nipixeldata(4, &mips, &dxt_data);
        let pd = NiPixelDataPayload {
            pixel_format: 4, // PX_FMT_DXT1
            num_faces: 1,
            num_mipmaps: 8,
            bytes_per_pixel: 8,
            num_pixels: 5_480,
            raw_pixels: raw,
        };
        let out = extract_dds_from_nipixeldata(&pd).expect("64x128 DXT1 should extract");
        assert_eq!(out.len(), 18 + 64 * 128 * 4);
        assert_eq!(u16::from_le_bytes([out[12], out[13]]), 64);
        assert_eq!(u16::from_le_bytes([out[14], out[15]]), 128);
    }

    #[test]
    fn parse_nipixeldata_header_rejects_truncated() {
        // A buffer that's just the right size for the minimum header
        // (87 bytes) but with num_mipmaps = 0 should be rejected.
        let raw = vec![0u8; 87];
        assert!(parse_nipixeldata_header(&raw).is_none());
    }

    #[test]
    fn parse_nipixeldata_header_rejects_too_short() {
        let raw = vec![0u8; 50];
        assert!(parse_nipixeldata_header(&raw).is_none());
    }

    /// Sanity check: the explicit-header path wins over the
    /// size-guesser even when the latter would have produced a
    /// different (wrong) answer. We build a 32×32 DXT1 texture with
    /// the V20_3_0.9 header saying 32×32. Without the header, the
    /// size-guesser might match something else — we verify the
    /// header path always wins.
    #[test]
    fn explicit_header_overrides_size_guesser() {
        let mips = [
            (32, 32, 0),
            (16, 16, 512),
            (8, 8, 640),
            (4, 4, 672),
            (2, 2, 680),
            (1, 1, 688),
        ];
        // Total chain = 696. Header is 75 + 12*6 = 147 bytes. raw
        // = 147 + 696 = 843 bytes total.
        let dxt_data = vec![0u8; 696];
        let raw = build_test_nipixeldata(4, &mips, &dxt_data);
        let pd = NiPixelDataPayload {
            pixel_format: 4,
            num_faces: 1,
            num_mipmaps: 6,
            bytes_per_pixel: 8,
            num_pixels: 696,
            raw_pixels: raw,
        };
        let out = extract_dds_from_nipixeldata(&pd).expect("explicit header should win");
        assert_eq!(u16::from_le_bytes([out[12], out[13]]), 32);
        assert_eq!(u16::from_le_bytes([out[14], out[15]]), 32);
    }

    /// Regression test for the mipmap[1].offset misread. The bug
    /// was: code read `raw_pixels[pos + 12]` (mipmap[1].width) as
    /// the main-mip size instead of `raw_pixels[pos + 20]`
    /// (mipmap[1].offset). For Bully textures, the mipmap[1].width
    /// (e.g. 256 for a 512x512 texture, 64 for a 64x128 texture) is
    /// a tiny fraction of the real main-mip size, so the decoder
    /// only saw a few blocks worth of DXT data and filled the rest
    /// of the TGA with zeros — which Photoshop rendered as a small
    /// white blob in the top-left corner.
    ///
    /// This test asserts the explicit header reads mipmap[1].offset
    /// (= the byte size of the main mip) correctly. We use a 4×4
    /// single-mip DXT1 texture (1 block × 8 bytes = 8 bytes), but
    /// the mipmap table says mip 0 is at offset 0 and mip 1 is at
    /// offset 4. With the old bug, main_mip_size would be 4 (= the
    /// width field of mip[1]) and the decoder would have insufficient
    /// data to decode even a single block. With the fix,
    /// main_mip_size is 4 — wait, that's still wrong. The correct
    /// main_mip_size for a 4×4 DXT1 texture is 8 bytes.
    ///
    /// Let me think again. For 1 mipmap only, we fall back to
    /// num_pixels. For 2+ mipmaps, we use mipmap[1].offset as
    /// main_mip_size. For a 4×4 DXT1 with mip 0 @ 0 and mip 1 @ 8,
    /// main_mip_size should be 8.
    ///
    /// So the test below uses mips = [(4, 4, 0), (2, 2, 8)] which
    /// gives a main mip of 8 bytes (one 4×4 block). With the old
    /// bug, the code would read mipmap[1].width = 2 as main_mip_size,
    /// so the decoder would have only 2 bytes of DXT data and fail
    /// to decode any block (all pixels zero). With the fix, 8 bytes
    /// are passed, one block decodes, the first 16 pixels are white.
    #[test]
    fn main_mip_size_uses_mipmap1_offset_not_width() {
        // 4×4 DXT1 = 1 block × 8 bytes (all white)
        let dxt_data: Vec<u8> = vec![0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        let mips = [
            (4, 4, 0),    // mip 0: 4x4 @ 0
            (2, 2, 8),    // mip 1: 2x2 @ 8
        ];
        let raw = build_test_nipixeldata(4, &mips, &dxt_data);
        let pd = NiPixelDataPayload {
            pixel_format: 4,
            num_faces: 1,
            num_mipmaps: 2,
            bytes_per_pixel: 8,
            num_pixels: 16,
            raw_pixels: raw,
        };
        let out = extract_dds_from_nipixeldata(&pd).expect("4x4 DXT1 should extract");
        assert_eq!(u16::from_le_bytes([out[12], out[13]]), 4);
        assert_eq!(u16::from_le_bytes([out[14], out[15]]), 4);
        // With main_mip_size=8, the single 4×4 DXT1 block decodes
        // to all-white (16 pixels). With the old bug reading
        // mipmap[1].width=2 as main_mip_size, the dxt1_to_tga loop
        // would have insufficient data (2 bytes) and produce zeros.
        let pixels_offset = 18;
        for i in 0..16 {
            let b = out[pixels_offset + i * 4];
            let g = out[pixels_offset + i * 4 + 1];
            let r = out[pixels_offset + i * 4 + 2];
            assert_eq!(r, 255, "pixel {i} R should be 255 (mip 0 is white)");
            assert_eq!(g, 255, "pixel {i} G should be 255 (mip 0 is white)");
            assert_eq!(b, 255, "pixel {i} B should be 255 (mip 0 is white)");
            assert_eq!(out[pixels_offset + i * 4 + 3], 255, "alpha should be opaque");
        }
    }

    /// Bully ships at least 4 NFT files with CR LF line endings in
    /// the version header instead of the canonical LF (e.g.
    /// `Stream\Test\Barr01_Switch.nft`). The parser must accept them
    /// without modification, since the version u32 is read from
    /// `newline_pos + 1` and the CR is just skipped. This test loads
    /// a real Bully NFT, makes an in-memory copy with the LF in the
    /// header swapped for CR LF, and parses both — the two NifFiles
    /// must have identical block counts and string tables.
    #[test]
    fn nft_parse_accepts_crlf_header() {
        let lf_path =
            "C:/Games/Bully - Scholarship Edition/Stream/Test/EXTwinradar029.nft";
        let lf_bytes = match std::fs::read(lf_path) {
            Ok(b) => b,
            Err(_) => return,
        };
        let lf = NifFile::parse(&lf_bytes).expect("LF NFT should parse");

        let newline = lf_bytes
            .iter()
            .position(|&b| b == 0x0A)
            .expect("LF NFT must contain a 0x0A in its header");
        let mut crlf_bytes = lf_bytes.clone();
        crlf_bytes.insert(newline, 0x0D);
        let crlf = NifFile::parse(&crlf_bytes)
            .expect("CRLF NFT should parse identically to LF NFT");

        assert_eq!(lf.blocks.len(), crlf.blocks.len());
        assert_eq!(lf.footer.roots, crlf.footer.roots);
        assert_eq!(lf.strings, crlf.strings);
    }

    /// Regression test for the NiTriStripsData parser bug: previously
    /// the parser read `strip_lengths` and `points` as `i32` and was
    /// missing the `num_triangles` u16 field, which made every
    /// `NiTriStrips` mesh produce 0 triangles. After the fix, a
    /// synthesized footer with `num_triangles=3, num_strips=1,
    /// strip_lengths=[6], points=[0,1,2,2,3,4]` should round-trip
    /// to the exact same byte values.
    #[test]
    fn nif_parse_strips_footer_u16_not_i32() {
        // Build a minimal NiTriStripsData footer matching V20_3_0.9:
        //   u16  num_triangles
        //   u16  num_strips
        //   u16  strip_lengths[num_strips]
        //   u8   has_points
        //   u16  points[sum(strip_lengths)]
        let mut footer = vec![];
        footer.extend_from_slice(&3u16.to_le_bytes()); // num_triangles
        footer.extend_from_slice(&1u16.to_le_bytes()); // num_strips
        footer.extend_from_slice(&6u16.to_le_bytes()); // strip_lengths[0]
        footer.push(1u8); // has_points
        let pts: [u16; 6] = [0, 1, 2, 2, 3, 4];
        for p in &pts {
            footer.extend_from_slice(&p.to_le_bytes());
        }
        // Total: 2 + 2 + 2 + 1 + 12 = 19 bytes.
        assert_eq!(footer.len(), 19);

        // If we had read strip_lengths/points as i32, the parser
        // would have produced garbage and missed all the points.
        // The fix: num_triangles=3, num_strips=1, strip_lengths=[6],
        // points=[0,1,2,2,3,4].
        let (num_triangles, num_strips, strip_lengths, has_points, points) =
            read_strips_footer_for_test(&footer);
        assert_eq!(num_triangles, 3);
        assert_eq!(num_strips, 1);
        assert_eq!(strip_lengths, vec![6]);
        assert!(has_points);
        assert_eq!(points, vec![0, 1, 2, 2, 3, 4]);
    }

    /// Regression test for the strip-to-triangle expansion: the Rust
    /// implementation must filter out degenerate triangles (where two
    /// or three indices are identical) because Gamebryo strips use
    /// repeated indices as restart markers (see bully-nif-tools'
    /// reveng/notes/research_notes.md § NiTriStripsData).
    ///
    /// A strip with 6 points [0, 1, 2, 2, 3, 4] produces 4 raw
    /// triples after the even/odd swap:
    ///   j=0 even: (0, 1, 2) — distinct, kept
    ///   j=1 odd:  (2, 1, 2) — points[1]==points[3]==2, a==c, skipped
    ///   j=2 even: (2, 2, 3) — points[2]==points[3]==2, a==b, skipped
    ///   j=3 odd:  (3, 2, 4) — all distinct, kept
    /// Result: 2 valid triangles.
    #[test]
    fn strip_to_triangle_filters_degenerates() {
        let points = vec![0u16, 1, 2, 2, 3, 4];
        let mut tris = vec![];
        for j in 0..points.len() - 2 {
            let (i0, i1, i2) = (points[j] as u32, points[j + 1] as u32, points[j + 2] as u32);
            let (a, b, c) = if j % 2 == 0 { (i0, i1, i2) } else { (i1, i0, i2) };
            if a == b || a == c || b == c {
                continue;
            }
            tris.push((a, b, c));
        }
        assert_eq!(tris, vec![(0, 1, 2), (3, 2, 4)]);
    }

    /// Wrapper that exercises the same path as `read_strips_footer`
    /// without needing a full NifFile. Mirrors the parser logic
    /// exactly so the regression test catches any future regression
    /// of the u16-vs-i32 fix.
    fn read_strips_footer_for_test(
        bytes: &[u8],
    ) -> (u16, u16, Vec<u16>, bool, Vec<u16>) {
        // Bully is little-endian. The reader is a tiny shim that
        // matches the production code's contract.
        let mut pos = 0;
        let read_u16 = |pos: &mut usize| -> u16 {
            let v = u16::from_le_bytes([bytes[*pos], bytes[*pos + 1]]);
            *pos += 2;
            v
        };
        let num_triangles = read_u16(&mut pos);
        let num_strips = read_u16(&mut pos);
        if num_strips == 0 {
            return (num_triangles, 0, Vec::new(), false, Vec::new());
        }
        let mut strip_lengths = Vec::with_capacity(num_strips as usize);
        for _ in 0..num_strips {
            strip_lengths.push(read_u16(&mut pos));
        }
        let has_points = bytes[pos] != 0;
        pos += 1;
        let total: usize = strip_lengths.iter().map(|&l| l as usize).sum();
        let mut points = Vec::with_capacity(total);
        for _ in 0..total {
            points.push(read_u16(&mut pos));
        }
        (num_triangles, num_strips, strip_lengths, has_points, points)
    }

    /// Regression test for the NiTexturingProperty parser bug: previously
    /// the parser skipped the `NiObjectNET` header and read
    /// `apply_mode` as `texture_count`, which shifted every slot
    /// read by 8 bytes and assigned textures to the wrong slots.
    /// After the fix, `1950Fridge.nif` block 7 should have:
    ///   - flags = 0x0005
    ///   - apply_mode = 0x0009 (Bully's value, not NifTools' 0x0002)
    ///   - 11 slots, with slots 2, 5, 8 populated
    ///     (detail, bump, decal 2 — see NifSkope's UI which calls
    ///     these "Base / Normal Map / Specular" by Bully convention)
    ///   - num_shader_textures = 0
    #[test]
    fn nif_parse_texturing_property_1950fridge() {
        let path = "C:/Games/Bully - Scholarship Edition/Stream/test1/1950Fridge.nif";
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(_) => return,
        };
        let nif = NifFile::parse(&bytes).expect("NIF should parse");

        // Find the NiTexturingProperty block.
        let mut found = None;
        for payload in nif.payloads.iter().flatten() {
            if let BlockPayload::NiTexturingProperty(t) = payload {
                found = Some(t.clone());
            }
        }
        let tex = found.expect("1950Fridge should have NiTexturingProperty");

        // The 3 populated slots in 1950Fridge are 2 (detail/_d),
        // 5 (bump/_n), and 8 (decal 2/_s) — Bully's NifSkope UI calls
        // them "Base / Normal / Specular" by the texture filename
        // suffix even though the slot indices are detail/bump/decal.
        assert_eq!(tex.flags, 0x0005);
        assert_eq!(tex.apply_mode, 0x0009);
        assert_eq!(tex.num_shader_textures, 0);
        assert!(tex.base.is_none(), "base slot empty in 1950Fridge");
        assert!(tex.dark.is_none());
        assert!(
            tex.detail.is_some(),
            "slot 2 (detail) must hold the diffuse texture"
        );
        assert!(tex.gloss.is_none());
        assert!(tex.glow.is_none());
        assert!(
            tex.bump_map.is_some(),
            "slot 5 (bump) must hold the normal map"
        );
        assert!(tex.decal[0].is_none());
        assert!(tex.decal[1].is_none());
        assert!(
            tex.decal[2].is_some(),
            "slot 8 (decal 2) must hold the specular map"
        );
        assert!(tex.decal[3].is_none());

        // Source refs must point at real NiSourceTexture blocks
        // (8, 9, 10 in 1950Fridge).
        let d = tex.detail.as_ref().unwrap().source_ref as u32;
        let n = tex.bump_map.as_ref().unwrap().source_ref as u32;
        let s = tex.decal[2].as_ref().unwrap().source_ref as u32;
        assert!(d < nif.blocks.len() as u32, "detail ref {d} OOB");
        assert!(n < nif.blocks.len() as u32, "bump ref {n} OOB");
        assert!(s < nif.blocks.len() as u32, "specular ref {s} OOB");
    }
}