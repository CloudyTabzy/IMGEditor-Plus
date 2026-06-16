use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::inspector::nif::{BlockPayload, NifFile};

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
/// This function is also called by the NFT catalog builder as a fallback
/// when inline pixel data is absent.
fn extract_pixels_for_nft(
    nft: &NifFile,
    nft_bytes: &[u8],
    tex_block_idx: usize,
) -> Option<Vec<u8>> {
    // Check inline pixel data first.
    if let Some(tga) = extract_embedded_pixels(nft, nft_bytes, tex_block_idx) {
        if tga.len() > 22 {
            // More than a 1x1 pixel — looks like real data.
            return Some(tga);
        }
    }
    // Fall back to NiPixelData blocks nearby.
    for candidate in tex_block_idx + 1..nft.blocks.len().min(tex_block_idx + 10) {
        let Some(Some(BlockPayload::NiPixelData(pd))) = nft.payloads.get(candidate) else {
            continue;
        };
        if pd.raw_pixels.is_empty() { continue; }
        let num_px = pd.num_pixels as usize;
        if num_px == 0 { continue; }
        let area = num_px;
        let side = (area as f32).sqrt() as u32;
        let w = side.max(1);
        let h = (area as u32 + w - 1) / w;
        if w > 4096 || h > 4096 { continue; }
        let bpp = pd.bytes_per_pixel as usize;
        let mip0_size = area * bpp;
        let raw = if mip0_size <= pd.raw_pixels.len() { &pd.raw_pixels[..mip0_size] } else { &pd.raw_pixels };
        let mut tga = Vec::with_capacity(18 + raw.len());
        tga.push(0); tga.push(0); tga.push(2);
        tga.extend_from_slice(&[0, 0, 0, 0, 0]);
        tga.extend_from_slice(&[0, 0]); tga.extend_from_slice(&[0, 0]);
        tga.extend_from_slice(&(w as u16).to_le_bytes());
        tga.extend_from_slice(&(h as u16).to_le_bytes());
        tga.push((bpp * 8) as u8); tga.push(0x20);
        if bpp == 4 {
            for i in (0..raw.len()).step_by(4) {
                tga.push(raw[i+2]); tga.push(raw[i+1]); tga.push(raw[i]); tga.push(raw[i+3]);
            }
        } else if bpp == 3 {
            for i in (0..raw.len()).step_by(3) {
                tga.push(raw[i+2]); tga.push(raw[i+1]); tga.push(raw[i]);
            }
        } else {
            tga.extend_from_slice(raw);
        }
        return Some(tga);
    }
    None
}