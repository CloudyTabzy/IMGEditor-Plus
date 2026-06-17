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

/// Try to extract pixel data from a NiPixelData block. Returns RGBA TGA
/// bytes for DXT1, or DDS bytes for DXT5 (fallback).
fn extract_dds_from_nipixeldata(pd: &NiPixelDataPayload) -> Option<Vec<u8>> {
    let raw = &pd.raw_pixels;
    let block_size = raw.len() as u32;

    let candidates: [(u32, u32); 7] = [
        (256, 256), (128, 128), (64, 64), (256, 128),
        (128, 256), (256, 64), (512, 512),
    ];
    let four_dxt1 = b"DXT1";
    let four_dxt5 = b"DXT5";

    for &fourcc in &[four_dxt1, four_dxt5] {
        for &(w, h) in &candidates {
            let (chain, _mips) = dxt_chain_size(w, h, fourcc);
            if chain > block_size { continue; }
            let hdr_sz = block_size - chain;
            if !(40..=512).contains(&hdr_sz) { continue; }

            let px_start = hdr_sz as usize;
            if px_start + chain as usize > raw.len() { continue; }
            let pixel_data = &raw[px_start..px_start + chain as usize];

            if fourcc == b"DXT1" {
                // Decompress to RGBA TGA (universal viewer support)
                return Some(dxt1_to_tga(pixel_data, w, h));
            } else {
                // DXT5: keep as DDS (complex alpha decoding)
                let dds_hdr = build_dds_header(w, h, fourcc, 1);
                let mut out = Vec::with_capacity(dds_hdr.len() + pixel_data.len());
                out.extend_from_slice(&dds_hdr);
                out.extend_from_slice(pixel_data);
                return Some(out);
            }
        }
    }
    None
}