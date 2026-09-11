//! RenderWare Texture Dictionary (TXD) parser.
//!
//! The GTA PC dictionaries used by III, Vice City, and San Andreas contain
//! D3D8/D3D9 Texture Native sections. Their native payload has a nested
//! STRUCT chunk, fixed 32-byte names, raster flags, an optional palette, and
//! length-prefixed mip levels. Older RenderWare tools can also emit a
//! platform-independent dictionary (`0x23`) with IMAGE sections; those are
//! normalized into the same viewer texture model.

use crate::parser::texture_decoder;

/// RenderWare section type IDs.
pub mod rw {
    pub const STRUCT: u32 = 0x01;
    pub const STRING: u32 = 0x02;
    pub const EXTENSION: u32 = 0x03;
    pub const TEXTURE: u32 = 0x06;
    pub const TEXTURE_NATIVE: u32 = 0x15;
    pub const TEXTURE_DICTIONARY: u32 = 0x16;
    pub const IMAGE: u32 = 0x18;
    pub const PI_TEXTURE_DICTIONARY: u32 = 0x23;
}

pub const PLATFORM_D3D8: u32 = 8;
pub const PLATFORM_D3D9: u32 = 9;
const MAX_TEXTURE_DIMENSION: u32 = 8_192;
const MAX_TEXTURES: u32 = 16_384;
const MAX_PI_MIPMAPS: u32 = 16;

/// D3D9 format values used by RenderWare's PC native texture stream.
/// These are `D3DFMT_*` codes, not bit counts: 20 is the only true 24-bit
/// RGB format and is practically unused on D3D9; RW stores "888" rasters
/// as X8R8G8B8 (22) because D3D9 has no renderable 24-bit texture format.
pub mod d3d_format {
    pub const _8888: u32 = 21;
    pub const R8G8B8: u32 = 20;
    pub const X8R8G8B8: u32 = 22;
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
            &texture_decoder::RasterDescriptor {
                width: self.width,
                height: self.height,
                depth: self.depth,
                raster_format: self.raster_format,
                palette: &self.palette,
                platform_id: self.platform_id,
                d3d_format: self.d3d_format,
                platform_properties: self.platform_properties,
                raster_type: self.raster_type,
            },
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
        self.has_alpha != 0
            || texture_decoder::native_has_alpha(
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
    if top.kind == rw::PI_TEXTURE_DICTIONARY {
        return parse_platform_independent_txd(bytes, top);
    }
    if top.kind != rw::TEXTURE_DICTIONARY {
        return Err(format!(
            "expected TEXTURE_DICTIONARY or platform-independent dictionary, got 0x{:02X}",
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
                    if u32::from(texture_count) > MAX_TEXTURES {
                        return Err(format!(
                            "texture count {} exceeds the supported limit {}",
                            texture_count, MAX_TEXTURES
                        ));
                    }
                }
            }
            rw::TEXTURE_NATIVE => {
                let body = &bytes[child.start..child.end];
                if textures.len() < MAX_TEXTURES as usize
                    && let Ok(texture) = parse_native_texture(body)
                {
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

/// Where a texture's bytes live in the original file, for surgical
/// replacement that preserves extension chunks, version words, and IMG
/// sector padding.
#[derive(Debug, Clone, Copy)]
pub(crate) enum NativeSplice {
    /// The native has a nested STRUCT section; `start..end` is that
    /// section including its 12-byte header. `native_header` is the
    /// enclosing native section's header position (its size field needs
    /// patching too).
    Struct {
        start: usize,
        end: usize,
        version: u32,
        native_header: usize,
    },
    /// Old-tool native without a STRUCT wrapper: `start..end` covers the
    /// whole native section including its header.
    Native { start: usize, end: usize },
}

/// Byte ranges for every texture the parser would produce, in the same
/// order (skipped/broken natives are not included, matching
/// [`parse_txd`]).
pub(crate) fn native_splices(bytes: &[u8]) -> Vec<NativeSplice> {
    let mut out = Vec::new();
    let Ok(top) = read_section(bytes, 0, bytes.len()) else {
        return out;
    };
    if top.kind != rw::TEXTURE_DICTIONARY {
        return out;
    }
    let mut position = top.start;
    while position < top.end {
        if top.end - position < 12 {
            break;
        }
        let Ok(child) = read_section(bytes, position, top.end) else {
            break;
        };
        if child.kind == rw::TEXTURE_NATIVE {
            let body = &bytes[child.start..child.end];
            if out.len() < MAX_TEXTURES as usize && parse_native_texture(body).is_ok() {
                out.push(splice_for_native(bytes, &child, position));
            }
        }
        position = child.end;
    }
    out
}

fn splice_for_native(bytes: &[u8], native: &Section, native_header: usize) -> NativeSplice {
    let mut position = native.start;
    while position < native.end {
        if native.end - position < 12 {
            break;
        }
        let Ok(child) = read_section(bytes, position, native.end) else {
            break;
        };
        if child.kind == rw::STRUCT {
            return NativeSplice::Struct {
                start: position,
                end: child.end,
                version: child.version,
                native_header,
            };
        }
        position = child.end;
    }
    NativeSplice::Native {
        start: native_header,
        end: native.end,
    }
}

/// Parse the legacy platform-independent RenderWare texture dictionary.
///
/// Unlike a normal TXD, the `0x23` root stores the texture count/device pair
/// directly in its body. Each record then contains a mip count, one or more
/// `IMAGE` chunks, a `TEXTURE` metadata chunk, and an `EXTENSION` chunk. The
/// images are converted to the same tight-row representation used by native
/// textures so the existing decoder and UI need no format-specific branch.
fn parse_platform_independent_txd(bytes: &[u8], top: Section) -> Result<TxdFile, String> {
    let body = &bytes[top.start..top.end];
    let mut cursor = Cursor::new(body);
    let texture_count = cursor.u16("platform-independent texture count")?;
    let device_id = cursor.u16("platform-independent device id")?;
    if u32::from(texture_count) > MAX_TEXTURES {
        return Err(format!(
            "texture count {} exceeds the supported limit {}",
            texture_count, MAX_TEXTURES
        ));
    }

    let mut textures = Vec::with_capacity(texture_count as usize);
    for texture_index in 0..texture_count {
        let mip_count = cursor.u32("platform-independent mipmap count")?;
        if mip_count == 0 || mip_count > MAX_PI_MIPMAPS {
            return Err(format!(
                "texture {} declares {} mipmaps; supported range is 1..={}",
                texture_index, mip_count, MAX_PI_MIPMAPS
            ));
        }

        let mut images = Vec::with_capacity(mip_count as usize);
        for mip_index in 0..mip_count {
            let image_section = read_section(body, cursor.position, body.len())?;
            if image_section.kind != rw::IMAGE {
                return Err(format!(
                    "texture {} mip {} expected IMAGE section (0x18), got 0x{:02X}",
                    texture_index, mip_index, image_section.kind
                ));
            }
            images.push(parse_platform_independent_image(
                &body[image_section.start..image_section.end],
            )?);
            cursor.position = image_section.end;
        }

        let texture_section = read_section(body, cursor.position, body.len())?;
        if texture_section.kind != rw::TEXTURE {
            return Err(format!(
                "texture {} expected TEXTURE section (0x06), got 0x{:02X}",
                texture_index, texture_section.kind
            ));
        }
        let metadata =
            parse_platform_independent_texture(&body[texture_section.start..texture_section.end])?;
        cursor.position = texture_section.end;

        let extension = read_section(body, cursor.position, body.len())?;
        if extension.kind != rw::EXTENSION {
            return Err(format!(
                "texture {} expected EXTENSION section (0x03), got 0x{:02X}",
                texture_index, extension.kind
            ));
        }
        cursor.position = extension.end;

        let first = images
            .first()
            .ok_or_else(|| format!("texture {} has no image mipmaps", texture_index))?;
        let width = first.width;
        let height = first.height;
        let depth = first.depth;
        let palette = first.palette.clone();
        let has_alpha = platform_independent_has_alpha(
            depth,
            &palette,
            &first.pixels,
            width as usize * height as usize,
        );
        let (raster_format, raster_type) = platform_independent_raster(depth);
        let mipmaps = images
            .into_iter()
            .map(|image| MipmapLevel {
                width: image.width,
                height: image.height,
                data: image.pixels,
            })
            .collect::<Vec<_>>();

        textures.push(NativeTexture {
            platform_id: 0,
            filter_mode: metadata.filter_mode,
            uv_addressing: metadata.uv_addressing,
            diffuse_name: metadata.name,
            alpha_name: metadata.mask,
            raster_format,
            d3d_format: 0,
            width,
            height,
            depth,
            num_mipmaps: mipmaps.len() as u8,
            raster_type,
            platform_properties: 0,
            has_alpha: has_alpha as u8,
            palette,
            mipmaps,
        });
    }

    Ok(TxdFile {
        device_id,
        texture_count,
        rw_version: top.version,
        textures,
    })
}

#[derive(Debug)]
struct PlatformIndependentImage {
    width: u32,
    height: u32,
    depth: u8,
    pixels: Vec<u8>,
    palette: Vec<u8>,
}

#[derive(Debug)]
struct PlatformIndependentTexture {
    filter_mode: u8,
    uv_addressing: u8,
    name: String,
    mask: String,
}

fn parse_platform_independent_image(bytes: &[u8]) -> Result<PlatformIndependentImage, String> {
    let struct_section = read_section(bytes, 0, bytes.len())?;
    if struct_section.kind != rw::STRUCT {
        return Err(format!(
            "platform-independent IMAGE expected STRUCT section (0x01), got 0x{:02X}",
            struct_section.kind
        ));
    }
    let mut header = Cursor::new(&bytes[struct_section.start..struct_section.end]);
    let width = header.u32("image width")?;
    let height = header.u32("image height")?;
    let depth = header.u32("image depth")?;
    let pitch = header.u32("image pitch")?;
    if width == 0 || height == 0 || width > MAX_TEXTURE_DIMENSION || height > MAX_TEXTURE_DIMENSION
    {
        return Err(format!(
            "image dimensions {width}x{height} exceed the supported range"
        ));
    }
    let depth = u8::try_from(depth).map_err(|_| format!("unsupported image depth {depth}"))?;
    let row_bytes = platform_independent_row_bytes(width, depth)?;
    let pitch = usize::try_from(pitch).map_err(|_| "image pitch is too large".to_string())?;
    if pitch < row_bytes {
        return Err(format!(
            "image pitch {pitch} is smaller than its {row_bytes}-byte row"
        ));
    }
    let pixel_bytes = pitch
        .checked_mul(height as usize)
        .ok_or_else(|| "image pixel size overflowed".to_string())?;
    let pixel_end = struct_section
        .end
        .checked_add(pixel_bytes)
        .ok_or_else(|| "platform-independent IMAGE pixel range overflowed".to_string())?;
    let pixels = bytes
        .get(struct_section.end..pixel_end)
        .ok_or_else(|| "platform-independent IMAGE pixels are truncated".to_string())?;
    let tight_len = row_bytes
        .checked_mul(height as usize)
        .ok_or_else(|| "image tight pixel size overflowed".to_string())?;
    let mut tight_pixels = vec![0u8; tight_len];
    for (source, destination) in pixels
        .chunks_exact(pitch)
        .zip(tight_pixels.chunks_exact_mut(row_bytes))
    {
        destination.copy_from_slice(&source[..row_bytes]);
    }

    let palette_size = match depth {
        4 => 64,
        8 => 1024,
        _ => 0,
    };
    let palette_start = pixel_end;
    let palette_end = palette_start
        .checked_add(palette_size)
        .ok_or_else(|| "platform-independent IMAGE palette range overflowed".to_string())?;
    let palette = bytes
        .get(palette_start..palette_end)
        .ok_or_else(|| "platform-independent IMAGE palette is truncated".to_string())?
        .to_vec();

    Ok(PlatformIndependentImage {
        width,
        height,
        depth,
        pixels: tight_pixels,
        palette,
    })
}

fn parse_platform_independent_texture(bytes: &[u8]) -> Result<PlatformIndependentTexture, String> {
    let mut position = 0usize;
    let struct_section = read_section(bytes, position, bytes.len())?;
    if struct_section.kind != rw::STRUCT {
        return Err(format!(
            "platform-independent TEXTURE expected STRUCT section (0x01), got 0x{:02X}",
            struct_section.kind
        ));
    }
    let mut structure = Cursor::new(&bytes[struct_section.start..struct_section.end]);
    let filter_mode = structure.u8("texture filter mode")?;
    let uv_addressing = structure.u8("texture UV addressing")?;
    let _ = structure.u16("texture metadata padding")?;
    position = struct_section.end;

    let name_section = read_section(bytes, position, bytes.len())?;
    let name = read_string_section(bytes, name_section, "texture name")?;
    position = name_section.end;
    let mask_section = read_section(bytes, position, bytes.len())?;
    let mask = read_string_section(bytes, mask_section, "texture mask")?;

    Ok(PlatformIndependentTexture {
        filter_mode,
        uv_addressing,
        name,
        mask,
    })
}

fn read_string_section(bytes: &[u8], section: Section, what: &str) -> Result<String, String> {
    if section.kind != rw::STRING {
        return Err(format!(
            "expected STRING section for {what}, got 0x{:02X}",
            section.kind
        ));
    }
    Ok(read_fixed_name(&bytes[section.start..section.end]))
}

fn platform_independent_row_bytes(width: u32, depth: u8) -> Result<usize, String> {
    if !matches!(depth, 4 | 8 | 16 | 24 | 32) {
        return Err(format!(
            "unsupported platform-independent image depth {depth}"
        ));
    }
    let bits = (width as usize)
        .checked_mul(depth as usize)
        .ok_or_else(|| "image row size overflowed".to_string())?;
    Ok(bits.div_ceil(8))
}

fn platform_independent_raster(depth: u8) -> (u32, u8) {
    match depth {
        4 => (
            texture_decoder::format::FORMAT_888 | texture_decoder::format::EXT_PAL4,
            0,
        ),
        8 => (
            texture_decoder::format::FORMAT_888 | texture_decoder::format::EXT_PAL8,
            0,
        ),
        16 => (
            texture_decoder::format::FORMAT_1555,
            texture_decoder::format::RASTER_TYPE_1555 as u8,
        ),
        24 => (
            texture_decoder::format::FORMAT_888,
            texture_decoder::format::RASTER_TYPE_888 as u8,
        ),
        32 => (
            texture_decoder::format::FORMAT_8888,
            texture_decoder::format::RASTER_TYPE_8888 as u8,
        ),
        _ => unreachable!("platform-independent image depth was validated"),
    }
}

fn platform_independent_has_alpha(
    depth: u8,
    palette: &[u8],
    pixels: &[u8],
    pixel_count: usize,
) -> bool {
    match depth {
        4 => (0..pixel_count).any(|index| {
            let byte = pixels.get(index / 2).copied().unwrap_or_default();
            let palette_index = if index % 2 == 0 {
                byte >> 4
            } else {
                byte & 0x0F
            };
            palette
                .get(usize::from(palette_index) * 4 + 3)
                .is_some_and(|alpha| *alpha < 255)
        }),
        8 => pixels.iter().take(pixel_count).any(|index| {
            palette
                .get(usize::from(*index) * 4 + 3)
                .is_some_and(|alpha| *alpha < 255)
        }),
        16 | 32 => true,
        _ => false,
    }
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
        // PC D3D8/D3D9 natives store the full 256-entry palette even for
        // PAL4 rasters (only the first 16 entries are meaningful) — the
        // mip data follows the palette, so a short read misparses the
        // whole texture. Non-PC platforms pack the used entries only.
        // See docs/research-inu-tools-gta.md.
        2 | 3
            if platform_id == texture_decoder::PLATFORM_D3D8
                || platform_id == texture_decoder::PLATFORM_D3D9 =>
        {
            Some(1024usize)
        }
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

    fn paletted_d3d8_txd_fixture(raster_format: u32, indices: &[u8]) -> Vec<u8> {
        let mut native = Vec::new();
        native.extend_from_slice(&texture_decoder::PLATFORM_D3D8.to_le_bytes());
        native.extend_from_slice(&[6, 17, 0, 0]);
        native.extend_from_slice(&{
            let mut name = [0_u8; 32];
            name[..4].copy_from_slice(b"pal8");
            name.to_vec()
        });
        native.extend_from_slice(&[0; 32]);
        native.extend_from_slice(&raster_format.to_le_bytes());
        native.extend_from_slice(&0_u32.to_le_bytes());
        native.extend_from_slice(&2_u16.to_le_bytes());
        native.extend_from_slice(&1_u16.to_le_bytes());
        native.extend_from_slice(&[8, 1, 4, 0]);
        // PC palettes: full 256 BGRA entries. Entries are BGRA (D3D ARGB
        // surface byte order); after the platform swap: entry 0 = blue,
        // entry 1 = red, entry 2 = yellow, entry 3 = gray-blue.
        let mut palette = vec![0_u8; 1024];
        palette[0..4].copy_from_slice(&[255, 0, 0, 255]);
        palette[4..8].copy_from_slice(&[0, 0, 255, 255]);
        palette[8..12].copy_from_slice(&[0, 255, 255, 255]);
        palette[12..16].copy_from_slice(&[128, 64, 32, 255]);
        native.extend_from_slice(&palette);
        native.extend_from_slice(&(indices.len() as u32).to_le_bytes());
        native.extend_from_slice(indices);

        let native_struct = section(rw::STRUCT, 0x1803_FFFF, &native);
        let native_section = section(rw::TEXTURE_NATIVE, 0x1803_FFFF, &native_struct);
        let mut dict_body = section(rw::STRUCT, 0x1803_FFFF, &[1, 0, 0, 0]);
        dict_body.extend_from_slice(&native_section);
        section(rw::TEXTURE_DICTIONARY, 0x1803_FFFF, &dict_body)
    }

    fn platform_independent_txd_fixture() -> Vec<u8> {
        let version = 0x1803_FFFF;
        let mut image_struct = Vec::new();
        image_struct.extend_from_slice(&2_u32.to_le_bytes()); // width
        image_struct.extend_from_slice(&1_u32.to_le_bytes()); // height
        image_struct.extend_from_slice(&8_u32.to_le_bytes()); // depth
        image_struct.extend_from_slice(&4_u32.to_le_bytes()); // pitch, padded row

        let mut palette = vec![0_u8; 1024];
        palette[..4].copy_from_slice(&[255, 0, 0, 255]);
        palette[4..8].copy_from_slice(&[0, 255, 0, 255]);
        let mut image_body = section(rw::STRUCT, version, &image_struct);
        image_body.extend_from_slice(&[0, 1, 0, 0]);
        image_body.extend_from_slice(&palette);
        let image = section(rw::IMAGE, version, &image_body);

        let mut texture_body = section(rw::STRUCT, version, &[6, 17, 0, 0]);
        texture_body.extend_from_slice(&section(rw::STRING, version, b"pi_tex\0"));
        texture_body.extend_from_slice(&section(rw::STRING, version, b"mask\0"));
        let texture = section(rw::TEXTURE, version, &texture_body);

        let mut dictionary_body = Vec::new();
        dictionary_body.extend_from_slice(&[1, 0, 3, 0]); // count, device
        dictionary_body.extend_from_slice(&1_u32.to_le_bytes()); // mip count
        dictionary_body.extend_from_slice(&image);
        dictionary_body.extend_from_slice(&texture);
        dictionary_body.extend_from_slice(&section(rw::EXTENSION, version, &[]));
        section(rw::PI_TEXTURE_DICTIONARY, version, &dictionary_body)
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
    fn d3d8_pal8_palette_swaps_bgra_to_rgba() {
        // GTA III/VC palettized rasters store palette entries as BGRA.
        // Entry 0 is BGRA blue; without the swap it would decode as red.
        let bytes = paletted_d3d8_txd_fixture(0x2000 | 0x0600, &[0, 1]);
        let txd = parse_txd(&bytes).unwrap();
        assert_eq!(txd.textures.len(), 1);
        let texture = &txd.textures[0];
        assert_eq!(texture.platform_id, 8);
        let rgba = texture.decode_rgba().unwrap();
        assert_eq!(&rgba[..4], &[0, 0, 255, 255], "BGRA blue -> RGBA blue");
        assert_eq!(&rgba[4..8], &[255, 0, 0, 255], "BGRA red -> RGBA red");
    }

    #[test]
    fn pc_pal4_uses_a_full_256_entry_palette_and_byte_indices() {
        // PC PAL4 rasters: 1024-byte palette + one index byte per pixel.
        // A short (64/128-byte) palette read would misalign the mip data.
        let bytes = paletted_d3d8_txd_fixture(0x4000 | 0x0600, &[1, 2]);
        let txd = parse_txd(&bytes).expect("PAL4 native must parse fully");
        let texture = &txd.textures[0];
        assert_eq!(texture.palette.len(), 1024, "full PC palette stored");
        let rgba = texture.decode_rgba().unwrap();
        assert_eq!(&rgba[..4], &[255, 0, 0, 255], "palette entry 1 (red)");
        assert_eq!(&rgba[4..8], &[255, 255, 0, 255], "palette entry 2 (yellow)");
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
    fn parses_platform_independent_paletted_texture() {
        let txd = parse_txd(&platform_independent_txd_fixture())
            .expect("platform-independent TXD should parse");
        assert_eq!(txd.device_id, 3);
        assert_eq!(txd.texture_count, 1);
        assert_eq!(txd.textures.len(), 1);

        let texture = &txd.textures[0];
        assert_eq!(texture.diffuse_name, "pi_tex");
        assert_eq!(texture.alpha_name, "mask");
        assert_eq!(texture.width, 2);
        assert_eq!(texture.height, 1);
        assert_eq!(texture.format_name(), "PAL8");
        assert!(!texture.has_alpha_channel());
        assert_eq!(
            texture.decode_rgba().unwrap(),
            vec![255, 0, 0, 255, 0, 255, 0, 255]
        );
    }

    #[test]
    fn parses_real_renderware_texture_when_fixture_is_present() {
        let Some(root) = crate::test_paths::gta3_exports() else {
            return;
        };
        let path = root.join("gta_proc_grassland.txd");
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
