//! RenderWare Texture Dictionary serializer (Phase B).
//!
//! Two write paths:
//! - New dictionaries are serialized from the parsed model
//!   ([`write_txd`] / [`single_texture_txd`]).
//! - Replacing a texture inside an existing file is **surgical**
//!   ([`replace_texture`]): only that texture's STRUCT section is
//!   re-emitted and the enclosing section sizes are patched, so
//!   extension chunks, version words, unknown fields, and IMG sector
//!   padding survive byte-for-byte. Retail TXDs carry per-file version
//!   words (III uses `0x0401FFFF`) and empty extension chunks, which
//!   full re-serialization would have to model; splicing does not.
//!
//! Layout per GTA's PC natives:
//! `[0x16 dict][struct: count u16 + device u16][0x15 native [0x01 struct:
//! header + palette + length-prefixed mips][0x03 extension]]...`

use crate::compat::encode::EncodedTexture;
use crate::parser::txd::{rw, MipmapLevel, NativeSplice, NativeTexture, TxdFile};

/// The version word `write_txd` uses when the caller has no original
/// (RW 3.6.0.3). Replacement always preserves the file's own version.
pub const RW_VERSION_DEFAULT: u32 = 0x1803_FFFF;

fn section(kind: u32, version: u32, body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(12 + body.len());
    out.extend_from_slice(&kind.to_le_bytes());
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&version.to_le_bytes());
    out.extend_from_slice(body);
    out
}

fn fixed_name(name: &str) -> [u8; 32] {
    let mut out = [0u8; 32];
    let bytes = name.as_bytes();
    let take = bytes.len().min(31);
    out[..take].copy_from_slice(&bytes[..take]);
    out
}

fn struct_body(texture: &NativeTexture) -> Vec<u8> {
    let mut out = Vec::with_capacity(88 + texture.palette.len() + texture.mipmaps.len() * 4);
    out.extend_from_slice(&texture.platform_id.to_le_bytes());
    out.push(texture.filter_mode);
    out.push(texture.uv_addressing);
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&fixed_name(&texture.diffuse_name));
    out.extend_from_slice(&fixed_name(&texture.alpha_name));
    out.extend_from_slice(&texture.raster_format.to_le_bytes());
    // Offset 76 carries the D3D format code on platform 9 and the
    // has-alpha flag on platform 8; the parsed `d3d_format` field is
    // that raw value in both cases.
    out.extend_from_slice(&texture.d3d_format.to_le_bytes());
    out.extend_from_slice(&(texture.width as u16).to_le_bytes());
    out.extend_from_slice(&(texture.height as u16).to_le_bytes());
    out.push(texture.depth);
    out.push(texture.num_mipmaps.max(1));
    out.push(texture.raster_type);
    out.push(texture.platform_properties);
    out.extend_from_slice(&texture.palette);
    for mip in &texture.mipmaps {
        out.extend_from_slice(&(mip.data.len() as u32).to_le_bytes());
        out.extend_from_slice(&mip.data);
    }
    out
}

/// Serialize one native texture section.
pub fn write_native(texture: &NativeTexture) -> Vec<u8> {
    write_native_ver(texture, RW_VERSION_DEFAULT)
}

fn write_native_ver(texture: &NativeTexture, version: u32) -> Vec<u8> {
    section(
        rw::TEXTURE_NATIVE,
        version,
        &section(rw::STRUCT, version, &struct_body(texture)),
    )
}

/// Serialize a full texture dictionary. Every section uses the
/// dictionary's own version word.
pub fn write_txd(txd: &TxdFile) -> Vec<u8> {
    let version = txd.rw_version;
    let mut dict_struct = Vec::with_capacity(4);
    dict_struct.extend_from_slice(&(txd.textures.len().min(u16::MAX as usize) as u16).to_le_bytes());
    dict_struct.extend_from_slice(&txd.device_id.to_le_bytes());
    let mut body = section(rw::STRUCT, version, &dict_struct);
    for texture in &txd.textures {
        body.extend_from_slice(&write_native_ver(texture, version));
    }
    section(rw::TEXTURE_DICTIONARY, version, &body)
}

/// Replace one texture inside an existing dictionary, preserving every
/// other byte (including extension chunks and sector padding).
pub fn replace_texture(
    bytes: &[u8],
    index: usize,
    texture: NativeTexture,
) -> Result<Vec<u8>, String> {
    let parsed = crate::parser::txd::parse_txd(bytes)?;
    if index >= parsed.textures.len() {
        return Err(format!(
            "texture index {index} is out of range (the dictionary has {})",
            parsed.textures.len()
        ));
    }
    let splices = crate::parser::txd::native_splices(bytes);
    let Some(splice) = splices.get(index).copied() else {
        return Err("this dictionary's texture layout cannot be edited in place".to_string());
    };

    let (start, end, replacement) = match splice {
        NativeSplice::Struct {
            start,
            end,
            version,
            ..
        } => {
            let old = &parsed.textures[index];
            let body = bytes.get(start + 12..end).unwrap_or_default();
            (
                start,
                end,
                section(rw::STRUCT, version, &spliced_struct_body(body, old, &texture)),
            )
        }
        NativeSplice::Native { start, end } => {
            let version = read_u32(bytes, start + 8).unwrap_or(RW_VERSION_DEFAULT);
            (start, end, write_native_ver(&texture, version))
        }
    };
    let delta = replacement.len() as i64 - (end - start) as i64;
    let mut out = Vec::with_capacity((bytes.len() as i64 + delta).max(0) as usize);
    out.extend_from_slice(&bytes[..start]);
    out.extend_from_slice(&replacement);
    out.extend_from_slice(&bytes[end..]);

    // Patch the enclosing section sizes: the top dictionary always, and
    // the native header when only its STRUCT child was replaced.
    let top_size = read_u32(bytes, 4).ok_or("missing dictionary size")?;
    write_u32(
        &mut out,
        4,
        (top_size as i64 + delta) as u32,
    )?;
    if let NativeSplice::Struct { native_header, .. } = splice {
        let native_size =
            read_u32(bytes, native_header + 4).ok_or("missing native size")?;
        write_u32(
            &mut out,
            native_header + 4,
            (native_size as i64 + delta) as u32,
        )?;
    }

    Ok(out)
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let slice = bytes.get(offset..offset + 4)?;
    Some(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

/// Struct body for a replacement: keep the original header bytes (so
/// names, filter/addressing, and unknown header bytes survive), patch
/// the raster fields, then emit the new palette and mips plus any
/// trailing bytes the original carried after its pixel data.
fn spliced_struct_body(original_body: &[u8], old: &NativeTexture, new: &NativeTexture) -> Vec<u8> {
    if original_body.len() < 88 {
        return struct_body(new);
    }
    let mut out = original_body[..88].to_vec();
    out[0..4].copy_from_slice(&new.platform_id.to_le_bytes());
    out[72..76].copy_from_slice(&new.raster_format.to_le_bytes());
    out[76..80].copy_from_slice(&new.d3d_format.to_le_bytes());
    out[80..82].copy_from_slice(&(new.width as u16).to_le_bytes());
    out[82..84].copy_from_slice(&(new.height as u16).to_le_bytes());
    out[84] = new.depth;
    out[85] = new.num_mipmaps.max(1);
    out[86] = new.raster_type;
    out[87] = new.platform_properties;
    out.extend_from_slice(&new.palette);
    for mip in &new.mipmaps {
        out.extend_from_slice(&(mip.data.len() as u32).to_le_bytes());
        out.extend_from_slice(&mip.data);
    }
    let old_consumed = 88
        + old.palette.len()
        + old.mipmaps
            .iter()
            .map(|mip| 4 + mip.data.len())
            .sum::<usize>();
    if original_body.len() > old_consumed {
        out.extend_from_slice(&original_body[old_consumed..]);
    }
    out
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) -> Result<(), String> {
    let slice = bytes
        .get_mut(offset..offset + 4)
        .ok_or_else(|| "patch offset out of range".to_string())?;
    slice.copy_from_slice(&value.to_le_bytes());
    Ok(())
}

/// Device ids from the RenderWare dictionary header: 1 = D3D8,
/// 2 = D3D9 (librw texture.cpp documents the enum).
pub fn device_for_platform(platform_id: u32) -> u16 {
    match platform_id {
        8 => 1,
        9 => 2,
        _ => 0,
    }
}

/// Build a fresh single-texture dictionary around one native.
pub fn single_texture_txd(texture: NativeTexture, rw_version: u32) -> Vec<u8> {
    let device_id = device_for_platform(texture.platform_id);
    write_txd(&TxdFile {
        device_id,
        texture_count: 1,
        rw_version,
        textures: vec![texture],
    })
}

/// Turn an encoded texture into a writable native with the given
/// names. The filter mode is LINEAR_LINEAR (6), the value the retail
/// corpora use overwhelmingly; the linear component applies to
/// minification and the second enables mip filtering for the generated
/// chain. Addressing stays wrap.
pub fn native_from_encoded(
    encoded: &EncodedTexture,
    platform_id: u32,
    diffuse_name: &str,
    alpha_name: &str,
) -> NativeTexture {
    let mipmaps = encoded
        .mipmaps
        .iter()
        .enumerate()
        .map(|(level, data)| MipmapLevel {
            width: (encoded.width >> level).max(1),
            height: (encoded.height >> level).max(1),
            data: data.clone(),
        })
        .collect();
    NativeTexture {
        platform_id,
        filter_mode: 6,
        uv_addressing: 0,
        diffuse_name: diffuse_name.to_string(),
        alpha_name: alpha_name.to_string(),
        raster_format: encoded.header.raster_format,
        d3d_format: encoded.header.d3d_format,
        width: encoded.width,
        height: encoded.height,
        depth: encoded.header.depth,
        num_mipmaps: encoded.levels(),
        raster_type: encoded.header.raster_type,
        platform_properties: encoded.header.platform_properties,
        has_alpha: encoded.format.has_alpha() as u8,
        palette: encoded.palette.clone(),
        mipmaps,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::encode::{encode_texture, EncodeFormat, EncodeOptions};

    fn encoded_sample(format: EncodeFormat, platform: u32) -> EncodedTexture {
        let mut px = Vec::new();
        for y in 0..16u32 {
            for x in 0..16u32 {
                px.extend([(x * 16) as u8, (y * 16) as u8, (x * y) as u8, 255]);
            }
        }
        encode_texture(&px, 16, 16, format, platform, EncodeOptions::default()).unwrap()
    }

    #[test]
    fn synthetic_texture_round_trips_through_the_parser() {
        for (format, platform) in [
            (EncodeFormat::Pal8, 8),
            (EncodeFormat::Rgb888, 8),
            (EncodeFormat::Argb8888, 8),
            (EncodeFormat::Dxt1, 9),
            (EncodeFormat::Dxt3, 9),
            (EncodeFormat::Dxt5, 9),
        ] {
            let encoded = encoded_sample(format, platform);
            let native = native_from_encoded(&encoded, platform, "sample", "");
            let bytes = single_texture_txd(native, RW_VERSION_DEFAULT);
            let parsed = crate::parser::txd::parse_txd(&bytes).expect("parse");

            assert_eq!(parsed.textures.len(), 1, "{format:?}");
            assert_eq!(
                parsed.device_id,
                device_for_platform(platform),
                "{format:?}"
            );
            let tex = &parsed.textures[0];
            assert_eq!(tex.diffuse_name, "sample");
            assert_eq!(tex.width, 16);
            assert_eq!(tex.height, 16);
            assert_eq!(tex.filter_mode, 6, "retail filter mode for new natives");
            assert_eq!(tex.depth, encoded.header.depth);
            assert_eq!(tex.num_mipmaps, encoded.levels());
            assert_eq!(tex.raster_format, encoded.header.raster_format);
            assert_eq!(tex.d3d_format, encoded.header.d3d_format);
            assert_eq!(tex.platform_properties, encoded.header.platform_properties);
            assert_eq!(tex.palette, encoded.palette);
            assert_eq!(tex.mipmaps.len(), encoded.mipmaps.len());
            for (parsed_mip, raw) in tex.mipmaps.iter().zip(encoded.mipmaps.iter()) {
                assert_eq!(&parsed_mip.data, raw);
            }
            // The base level decodes and keeps the format identity.
            let rgba = tex.decode_rgba().expect("decode");
            assert_eq!(rgba.len(), 16 * 16 * 4);
            let _ = format;
        }
    }

    #[test]
    fn replacement_keeps_other_textures_verbatim() {
        let first = native_from_encoded(&encoded_sample(EncodeFormat::Pal8, 8), 8, "keepme", "");
        let second = native_from_encoded(&encoded_sample(EncodeFormat::Rgb565, 8), 8, "editme", "");
        let bytes = write_txd(&TxdFile {
            device_id: 1,
            texture_count: 2,
            rw_version: RW_VERSION_DEFAULT,
            textures: vec![first, second],
        });

        let replacement =
            native_from_encoded(&encoded_sample(EncodeFormat::Argb8888, 8), 8, "edited", "");
        let new_bytes = replace_texture(&bytes, 1, replacement).expect("replace");
        let parsed = crate::parser::txd::parse_txd(&new_bytes)
            .unwrap_or_else(|error| panic!("parse failed: {error}"));
        assert_eq!(parsed.textures.len(), 2);
        assert_eq!(parsed.textures[0].diffuse_name, "keepme");
        // A replacement edits the pixels, not the texture's name.
        assert_eq!(parsed.textures[1].diffuse_name, "editme");
        assert_eq!(parsed.textures[1].format_name(), "8888 ARGB");

        // The untouched first texture re-emits verbatim.
        let original = crate::parser::txd::parse_txd(&bytes).unwrap();
        assert_eq!(
            write_native(&parsed.textures[0]),
            write_native(&original.textures[0])
        );

        assert!(replace_texture(&bytes, 9, parsed.textures[0].clone()).is_err());
    }

    /// Retail exports are the fidelity gate: parse -> write must be
    /// byte-identical for every texture the parser understands.
    /// The real fidelity gate: identity-replacing every texture in
    /// sampled retail TXDs (III PAL8/888, VC DXT with D3D8 codes, SA
    /// DXT with mips) must reproduce the entry bytes exactly - the
    /// splice has to preserve extension chunks, version words, and
    /// sector padding it does not understand.
    #[test]
    fn retail_identity_replacement_is_byte_identical() {
        use crate::archive::ArchiveInfo;
        use crate::parser::{ImgParser, ImgVersion, PcV1Parser, PcV2Parser, detect_version};
        use std::collections::BTreeMap;

        let Some(root) = crate::test_paths::corpus_root() else {
            return;
        };
        let archives = [
            ("iii-world", root.join("Gta_3_img/models/gta3.img")),
            ("iii-txd", root.join("Gta_3_img/models/txd.img")),
            ("vc", root.join("Grand Theft Auto Vice City/models/gta3.img")),
            ("sa-gta3", root.join("GTA San Andreas/models/gta3.img")),
            ("sa-player", root.join("GTA San Andreas/models/player.img")),
            ("sa-cutscene", root.join("GTA San Andreas/models/cutscene.img")),
        ];

        let mut checked = 0usize;
        let mut versions: BTreeMap<String, BTreeMap<u32, usize>> = BTreeMap::new();
        for (label, path) in archives {
            if !path.exists() {
                continue;
            }
            let version = detect_version(&path);
            let mut archive = ArchiveInfo::new(label.to_string(), false, version);
            archive.path = Some(path.clone());
            match version {
                ImgVersion::One => PcV1Parser.open(&mut archive).expect("open v1"),
                ImgVersion::Two => PcV2Parser.open(&mut archive).expect("open v2"),
                _ => continue,
            }
            for (idx, entry) in archive.entries.iter().enumerate() {
                if !entry.file_name_lower.ends_with(".txd") || idx % 137 != 0 {
                    continue;
                }
                let Ok(bytes) = crate::parser::read_entry_data(&archive, entry) else {
                    continue;
                };
                let Ok(parsed) = crate::parser::txd::parse_txd(&bytes) else {
                    continue;
                };
                if parsed.textures.is_empty() {
                    continue;
                }
                *versions
                    .entry(label.to_string())
                    .or_default()
                    .entry(parsed.rw_version)
                    .or_insert(0) += 1;
                for (texture_index, texture) in parsed.textures.iter().enumerate() {
                    let rewritten = replace_texture(&bytes, texture_index, texture.clone())
                        .unwrap_or_else(|error| {
                            panic!("{label} / {}: {error}", entry.file_name)
                        });
                    if rewritten != bytes {
                        let first = rewritten
                            .iter()
                            .zip(bytes.iter())
                            .position(|(a, b)| a != b);
                        panic!(
                            "{label} / {} texture {texture_index} identity splice mismatch at {:?} ({} vs {} bytes)",
                            entry.file_name,
                            first,
                            rewritten.len(),
                            bytes.len()
                        );
                    }
                }
                checked += 1;
            }
        }
        for (label, map) in &versions {
            let list: Vec<String> = map
                .iter()
                .map(|(version, count)| format!("0x{version:X} x{count}"))
                .collect();
            eprintln!("{label} version words: {}", list.join(", "));
        }
        eprintln!("identity-replaced {checked} retail TXDs byte-identically");
        assert!(checked > 0, "corpus root is set but no archives were found");
    }
}
