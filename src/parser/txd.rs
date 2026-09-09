//! RenderWare Texture Dictionary (TXD) parser.
//!
//! The GTA PC dictionaries used by III, Vice City, and San Andreas contain
//! D3D8/D3D9 Texture Native sections. Their native payload has a nested
//! STRUCT chunk, fixed 32-byte names, raster flags, an optional palette, and
//! length-prefixed mip levels. Console-native dictionaries are left for a
//! future platform-specific decoder.

use crate::parser::texture_decoder;

/// RenderWare section type IDs.
pub mod rw {
    pub const STRUCT: u32 = 0x01;
    pub const STRING: u32 = 0x02;
    pub const EXTENSION: u32 = 0x03;
    pub const TEXTURE: u32 = 0x06;
    pub const TEXTURE_NATIVE: u32 = 0x15;
    pub const TEXTURE_DICTIONARY: u32 = 0x16;
}

pub const PLATFORM_D3D8: u32 = 8;
pub const PLATFORM_D3D9: u32 = 9;
const MAX_TEXTURE_DIMENSION: u32 = 8_192;

/// D3D9 format values used by RenderWare's PC native texture stream.
pub mod d3d_format {
    pub const _8888: u32 = 21;
    pub const _888: u32 = 22;
    pub const _565: u32 = 23;
    pub const _555: u32 = 24;
    pub const _1555: u32 = 25;
    pub const _4444: u32 = 26;
    pub const L8: u32 = 50;
    pub const A8L8: u32 = 51;
    pub const DXT1: u32 = 0x3154_5844;
    pub const DXT2: u32 = 0x3254_5844;
    pub const DXT3: u32 = 0x3354_5844;
    pub const DXT4: u32 = 0x3454_5844;
    pub const DXT5: u32 = 0x3554_5844;
}

#[derive(Clone, Copy, Debug)]
struct Section {
    kind: u32,
    start: usize,
    end: usize,
    version: u32,
}

struct Cursor<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn take(&mut self, amount: usize, what: &str) -> Result<&'a [u8], String> {
        let end = self
            .position
            .checked_add(amount)
            .ok_or_else(|| format!("{what} size overflowed"))?;
        let bytes = self
            .bytes
            .get(self.position..end)
            .ok_or_else(|| format!("unexpected end reading {what}"))?;
        self.position = end;
        Ok(bytes)
    }

    fn u8(&mut self, what: &str) -> Result<u8, String> {
        Ok(self.take(1, what)?[0])
    }

    fn u16(&mut self, what: &str) -> Result<u16, String> {
        Ok(u16::from_le_bytes(
            self.take(2, what)?.try_into().expect("bounded read"),
        ))
    }

    fn u32(&mut self, what: &str) -> Result<u32, String> {
        Ok(u32::from_le_bytes(
            self.take(4, what)?.try_into().expect("bounded read"),
        ))
    }
}

/// A parsed TXD file containing zero or more textures.
#[derive(Debug, Clone, Default)]
pub struct TxdFile {
    pub device_id: u16,
    pub texture_count: u16,
    pub rw_version: u32,
    pub textures: Vec<NativeTexture>,
}

/// A single native texture within a TXD.
#[derive(Debug, Clone)]
pub struct NativeTexture {
    pub platform_id: u32,
    pub filter_mode: u8,
    pub uv_addressing: u8,
    pub diffuse_name: String,
    pub alpha_name: String,
    /// Complete RenderWare raster format flags.
    pub raster_format: u32,
    /// D3D9 format/FourCC. D3D8 normally leaves this at zero and stores its
    /// compression selector in platform_properties.
    pub d3d_format: u32,
    pub width: u32,
    pub height: u32,
    pub depth: u8,
    pub num_mipmaps: u8,
    pub raster_type: u8,
    pub platform_properties: u8,
    /// Kept as a byte for compatibility with the earlier parser API.
    pub has_alpha: u8,
    pub palette: Vec<u8>,
    pub mipmaps: Vec<MipmapLevel>,
}

#[derive(Debug, Clone)]
pub struct MipmapLevel {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl NativeTexture {
    /// Decode the base mipmap level to RGBA.
    pub fn decode_rgba(&self) -> Result<Vec<u8>, texture_decoder::DecodeError> {
        let mip = self
            .mipmaps
            .first()
            .ok_or(texture_decoder::DecodeError::BufferTooSmall { need: 1, have: 0 })?;
        texture_decoder::decode_native_raster(
            &mip.data,
            self.width,
            self.height,
            self.depth,
            self.raster_format,
            &self.palette,
            self.platform_id,
            self.d3d_format,
            self.platform_properties,
            self.raster_type,
        )
    }

    /// Human-readable format name.
    pub fn format_name(&self) -> &'static str {
        texture_decoder::native_format_name(
            self.raster_format,
            self.platform_id,
            self.d3d_format,
            self.platform_properties,
            self.raster_type,
        )
    }

    /// Whether the raster format is block-compressed.
    pub fn is_dxt(&self) -> bool {
        texture_decoder::native_is_dxt(
            self.raster_format,
            self.platform_id,
            self.d3d_format,
            self.platform_properties,
            self.raster_type,
        )
    }

    pub fn has_alpha_channel(&self) -> bool {
        texture_decoder::native_has_alpha(
            self.raster_format,
            self.platform_id,
            self.d3d_format,
            self.platform_properties,
            self.raster_type,
        )
    }
}

/// Parse a complete TXD file from raw bytes.
pub fn parse_txd(bytes: &[u8]) -> Result<TxdFile, String> {
    let top = read_section(bytes, 0, bytes.len())?;
    if top.kind != rw::TEXTURE_DICTIONARY {
        return Err(format!(
            "expected TEXTURE_DICTIONARY section (0x16), got 0x{:02X}",
            top.kind
        ));
    }

    let mut position = top.start;
    let mut device_id = 0u16;
    let mut texture_count = 0u16;
    let mut textures = Vec::new();

    while position < top.end {
        if top.end - position < 12 {
            break;
        }
        let child = read_section(bytes, position, top.end)?;
        match child.kind {
            rw::STRUCT => {
                let body = &bytes[child.start..child.end];
                if body.len() >= 4 {
                    texture_count = u16::from_le_bytes([body[0], body[1]]);
                    device_id = u16::from_le_bytes([body[2], body[3]]);
                }
            }
            rw::TEXTURE_NATIVE => {
                let body = &bytes[child.start..child.end];
                if let Ok(texture) = parse_native_texture(body) {
                    textures.push(texture);
                }
            }
            _ => {}
        }
        position = child.end;
    }

    Ok(TxdFile {
        device_id,
        texture_count,
        rw_version: top.version,
        textures,
    })
}

fn parse_native_texture(bytes: &[u8]) -> Result<NativeTexture, String> {
    // GTA PC Texture Native chunks contain a nested STRUCT. A raw fallback is
    // retained for old tools that omitted that wrapper.
    let mut position = 0usize;
    while position < bytes.len() {
        if bytes.len() - position < 12 {
            break;
        }
        let Ok(child) = read_section(bytes, position, bytes.len()) else {
            break;
        };
        if child.kind == rw::STRUCT {
            return parse_native_struct(&bytes[child.start..child.end]);
        }
        position = child.end;
    }
    parse_native_struct(bytes)
}

fn parse_native_struct(bytes: &[u8]) -> Result<NativeTexture, String> {
    let mut cursor = Cursor::new(bytes);
    let platform_id = cursor.u32("platform id")?;
    let filter_mode = cursor.u8("filter mode")?;
    let uv_addressing = cursor.u8("UV addressing")?;
    let _ = cursor.u16("native header padding")?;
    let diffuse_name = read_fixed_name(cursor.take(32, "diffuse name")?);
    let alpha_name = read_fixed_name(cursor.take(32, "alpha name")?);
    let raster_format = cursor.u32("raster format flags")?;
    let d3d_format = cursor.u32("D3D format")?;
    let width = u32::from(cursor.u16("texture width")?);
    let height = u32::from(cursor.u16("texture height")?);
    if width == 0 || height == 0 || width > MAX_TEXTURE_DIMENSION || height > MAX_TEXTURE_DIMENSION
    {
        return Err(format!(
            "texture dimensions {width}x{height} exceed the supported range"
        ));
    }
    let depth = cursor.u8("texture depth")?;
    let raw_levels = cursor.u8("mipmap level count")?;
    let raster_type = cursor.u8("raster type")?;
    let platform_properties = cursor.u8("platform properties")?;
    let num_mipmaps = raw_levels.max(1);

    let palette_size = match (raster_format >> 13) & 0x3 {
        1 => Some(1024usize),
        2 | 3 => Some(if depth == 4 { 64 } else { 128 }),
        _ => None,
    };
    let palette = match palette_size {
        Some(size) => cursor.take(size, "palette")?.to_vec(),
        None => Vec::new(),
    };

    let mut mipmaps = Vec::with_capacity(num_mipmaps as usize);
    for level in 0..num_mipmaps as usize {
        let byte_len = cursor.u32("mipmap byte length")? as usize;
        let data = cursor.take(byte_len, "mipmap pixels")?.to_vec();
        mipmaps.push(MipmapLevel {
            width: width.checked_shr(level as u32).unwrap_or(0).max(1),
            height: height.checked_shr(level as u32).unwrap_or(0).max(1),
            data,
        });
    }

    let has_alpha = texture_decoder::native_has_alpha(
        raster_format,
        platform_id,
        d3d_format,
        platform_properties,
        raster_type,
    ) as u8;

    Ok(NativeTexture {
        platform_id,
        filter_mode,
        uv_addressing,
        diffuse_name,
        alpha_name,
        raster_format,
        d3d_format,
        width,
        height,
        depth,
        num_mipmaps,
        raster_type,
        platform_properties,
        has_alpha,
        palette,
        mipmaps,
    })
}

fn read_fixed_name(bytes: &[u8]) -> String {
    let bytes = bytes.split(|&byte| byte == 0).next().unwrap_or_default();
    String::from_utf8_lossy(bytes).trim().to_string()
}

fn read_section(bytes: &[u8], position: usize, limit: usize) -> Result<Section, String> {
    let header_end = position
        .checked_add(12)
        .ok_or_else(|| "section header offset overflowed".to_string())?;
    if header_end > limit || header_end > bytes.len() {
        return Err("unexpected end of section header".to_string());
    }
    let kind = u32::from_le_bytes(bytes[position..position + 4].try_into().expect("header"));
    let size = u32::from_le_bytes(
        bytes[position + 4..position + 8]
            .try_into()
            .expect("header"),
    ) as usize;
    let version = u32::from_le_bytes(
        bytes[position + 8..position + 12]
            .try_into()
            .expect("header"),
    );
    let end = header_end
        .checked_add(size)
        .ok_or_else(|| format!("section 0x{kind:02X} size overflowed"))?;
    if end > limit || end > bytes.len() {
        return Err(format!(
            "section 0x{kind:02X} size {size} exceeds its container"
        ));
    }
    Ok(Section {
        kind,
        start: header_end,
        end,
        version,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn section(kind: u32, version: u32, body: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(12 + body.len());
        bytes.extend_from_slice(&kind.to_le_bytes());
        bytes.extend_from_slice(&(body.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&version.to_le_bytes());
        bytes.extend_from_slice(body);
        bytes
    }

    fn d3d9_txd_fixture() -> Vec<u8> {
        let mut native = Vec::new();
        native.extend_from_slice(&PLATFORM_D3D9.to_le_bytes());
        native.extend_from_slice(&[6, 17, 0, 0]);
        native.extend_from_slice(&{
            let mut name = [0_u8; 32];
            name[..5].copy_from_slice(b"oak2b");
            name.to_vec()
        });
        native.extend_from_slice(&[0; 32]);
        native.extend_from_slice(&0x0000_0300_u32.to_le_bytes());
        native.extend_from_slice(&d3d_format::DXT3.to_le_bytes());
        native.extend_from_slice(&4_u16.to_le_bytes());
        native.extend_from_slice(&4_u16.to_le_bytes());
        native.extend_from_slice(&[16, 1, 4, 9]);
        native.extend_from_slice(&16_u32.to_le_bytes());
        native.extend_from_slice(&[0xFF; 16]);

        let native_struct = section(rw::STRUCT, 0x1803_FFFF, &native);
        let native_section = section(rw::TEXTURE_NATIVE, 0x1803_FFFF, &native_struct);
        let mut dict_body = section(rw::STRUCT, 0x1803_FFFF, &[1, 0, 2, 0]);
        dict_body.extend_from_slice(&native_section);
        section(rw::TEXTURE_DICTIONARY, 0x1803_FFFF, &dict_body)
    }

    fn d3d8_txd_fixture() -> Vec<u8> {
        let mut native = Vec::new();
        native.extend_from_slice(&PLATFORM_D3D8.to_le_bytes());
        native.extend_from_slice(&[6, 17, 0, 0]);
        native.extend_from_slice(&[0; 32]);
        native.extend_from_slice(&[0; 32]);
        native.extend_from_slice(&0x0000_0100_u32.to_le_bytes());
        native.extend_from_slice(&0_u32.to_le_bytes());
        native.extend_from_slice(&1_u16.to_le_bytes());
        native.extend_from_slice(&1_u16.to_le_bytes());
        native.extend_from_slice(&[16, 1, 1, 0]);
        native.extend_from_slice(&2_u32.to_le_bytes());
        // 1555: opaque black.
        native.extend_from_slice(&0x8000_u16.to_le_bytes());

        let native_struct = section(rw::STRUCT, 0x1803_FFFF, &native);
        let native_section = section(rw::TEXTURE_NATIVE, 0x1803_FFFF, &native_struct);
        let mut dict_body = section(rw::STRUCT, 0x1803_FFFF, &[1, 0, 0, 0]);
        dict_body.extend_from_slice(&native_section);
        section(rw::TEXTURE_DICTIONARY, 0x1803_FFFF, &dict_body)
    }

    #[test]
    fn reject_empty_file() {
        assert!(parse_txd(&[]).is_err());
    }

    #[test]
    fn reject_non_txd_section() {
        let bytes = section(rw::STRUCT, 0x1003_FFFF, &[]);
        assert!(parse_txd(&bytes).is_err());
    }

    #[test]
    fn parses_texture_dictionary_header() {
        let bytes = section(
            rw::TEXTURE_DICTIONARY,
            0x1803_FFFF,
            &section(rw::STRUCT, 0x1803_FFFF, &[0, 0, 2, 0]),
        );
        let txd = parse_txd(&bytes).unwrap();
        assert_eq!(txd.device_id, 2);
        assert_eq!(txd.texture_count, 0);
        assert!(txd.textures.is_empty());
    }

    #[test]
    fn parses_pc_d3d9_native_texture() {
        let txd = parse_txd(&d3d9_txd_fixture()).expect("fixture should parse");
        assert_eq!(txd.device_id, 2);
        assert_eq!(txd.texture_count, 1);
        assert_eq!(txd.textures.len(), 1);
        let texture = &txd.textures[0];
        assert_eq!(texture.diffuse_name, "oak2b");
        assert_eq!(texture.width, 4);
        assert_eq!(texture.height, 4);
        assert_eq!(texture.mipmaps[0].data.len(), 16);
        assert_eq!(texture.format_name(), "DXT3");
        assert_eq!(texture.decode_rgba().unwrap().len(), 4 * 4 * 4);
    }

    #[test]
    fn parses_pc_d3d8_native_texture() {
        let txd = parse_txd(&d3d8_txd_fixture()).expect("D3D8 fixture should parse");
        assert_eq!(txd.textures.len(), 1);
        let texture = &txd.textures[0];
        assert_eq!(texture.platform_id, PLATFORM_D3D8);
        assert_eq!(texture.format_name(), "1555 ARGB");
        assert_eq!(texture.decode_rgba().unwrap(), vec![0, 0, 0, 255]);
    }

    #[test]
    fn parses_real_renderware_texture_when_fixture_is_present() {
        let path = "C:/Dev/IMGEditor-master/Gta_3_img/Exported/gta_proc_grassland.txd";
        let Ok(bytes) = std::fs::read(path) else {
            return;
        };
        let txd = parse_txd(&bytes).expect("real TXD should parse");
        assert!(!txd.textures.is_empty());
        let decoded = txd
            .textures
            .iter()
            .filter_map(|texture| texture.decode_rgba().ok())
            .next()
            .expect("real TXD should expose a decodable PC texture");
        assert!(!decoded.is_empty());
    }
}
