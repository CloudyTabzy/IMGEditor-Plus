//! Gamebryo NIF 20.3.0.9 parser for Bully Scholarship Edition.
//!
//! The parser is hand-rolled against the format spec in
//! `Docs/bully_nif_format.md`. It does **not** depend on the `nif 0.5`
//! crate (which only targets 20.0.0.4) or on the niftools nifgen schema
//! (which is GPL and written in Python).
//!
//! Reading a NIF:
//!
//! ```no_run
//! use imgeditor::inspector::nif::NifFile;
//!
//! let bytes = std::fs::read("test.nif").unwrap();
//! let file = NifFile::parse(&bytes).unwrap();
//! println!("{} blocks, root = {}", file.blocks.len(), file.footer.roots[0]);
//! ```
//!
//! The parser tolerates big-endian files (Bully ships ~4.3 % BE), the
//! string table is decoded correctly, and the block array is laid out
//! in declared order so that `Ref` indices are valid as soon as the
//! header has been consumed.

use thiserror::Error;

/// Gamebryo NIF file version supported by this parser.
///
/// Bully ships `0x14030009` (20.3.0.9, `user_version = 0`). Other
/// versions in the wild are not currently parsed.
pub const BULLY_NIF_VERSION: u32 = 0x14030009;

#[derive(Debug, Error)]
pub enum NifError {
    #[error("file is too short to be a NIF (got {0} bytes)")]
    TooShort(usize),
    #[error("unsupported NIF version 0x{0:08X} (expected 0x{BULLY_NIF_VERSION:08X})")]
    UnsupportedVersion(u32),
    #[error("header is not a Gamebryo NIF: {0:?}")]
    BadHeader([u8; 6]),
    #[error("end of file while reading {0}")]
    UnexpectedEof(&'static str),
    #[error("invalid string table: {0}")]
    BadStringTable(String),
    #[error("invalid {0}: {1}")]
    InvalidField(&'static str, String),
    #[error("block {block} ({block_type}) at offset {offset} truncated: expected {expected} bytes, have {actual}")]
    TruncatedBlock {
        block: usize,
        block_type: String,
        offset: u64,
        expected: u64,
        actual: u64,
    },
    #[error("invalid reference: block index {index} (file has {count} blocks)")]
    BadRef { index: i32, count: usize },
}

pub type NifResult<T> = Result<T, NifError>;

/// Endianness of multi-byte values in the rest of the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endian {
    Little,
    Big,
}

impl Endian {
    fn from_byte(b: u8) -> Self {
        match b {
            1 => Endian::Little,
            _ => Endian::Big,
        }
    }

    pub fn read_u16(self, bytes: [u8; 2]) -> u16 {
        match self {
            Endian::Little => u16::from_le_bytes(bytes),
            Endian::Big => u16::from_be_bytes(bytes),
        }
    }

    pub fn read_u32(self, bytes: [u8; 4]) -> u32 {
        match self {
            Endian::Little => u32::from_le_bytes(bytes),
            Endian::Big => u32::from_be_bytes(bytes),
        }
    }

    pub fn read_i32(self, bytes: [u8; 4]) -> i32 {
        match self {
            Endian::Little => i32::from_le_bytes(bytes),
            Endian::Big => i32::from_be_bytes(bytes),
        }
    }

    pub fn read_f32(self, bytes: [u8; 4]) -> f32 {
        match self {
            Endian::Little => f32::from_le_bytes(bytes),
            Endian::Big => f32::from_be_bytes(bytes),
        }
    }
}

/// Magic header line that prefixes every Gamebryo NIF file.
pub const NIF_HEADER_MAGIC: &[u8] = b"Gamebryo File Format, Version ";

/// Top-level parsed NIF file.
#[derive(Debug, Clone)]
pub struct NifFile {
    /// Raw header line including the trailing `0x0A`.
    pub header_line: String,
    /// File version (`0x14030009` for Bully).
    pub version: u32,
    /// Endianness of all multi-byte fields in the file.
    pub endian: Endian,
    /// User version (0 for Bully, 0x10000 for Divinity 2, ...).
    pub user_version: u32,
    /// String table, indexed by `NiFixedString` values.
    pub strings: Vec<String>,
    /// Block type names, indexed by `block_type_index[i]`.
    pub block_types: Vec<String>,
    /// Declared block array, in file order. `blocks[i]` is the
    /// block whose declared index is `i`; the on-disk offset of its
    /// payload is `header_end + sum(block_sizes[0..i])`.
    pub blocks: Vec<BlockMeta>,
    /// Per-block parsed payload, if the parser recognised the type.
    /// Same length and order as `blocks`; `None` means the block
    /// type is not yet implemented in the inspector.
    pub payloads: Vec<Option<BlockPayload>>,
    /// File footer.
    pub footer: Footer,
}

/// Per-block header information.
#[derive(Debug, Clone)]
pub struct BlockMeta {
    /// Type index into `block_types[]`.
    pub type_index: u16,
    /// Resolved type name (e.g. `"NiTriShape"`).
    pub type_name: String,
    /// Declared size of the block payload in bytes. The next block
    /// starts this many bytes after the start of this one.
    pub size: u32,
    /// Absolute file offset of the first byte of the block payload
    /// (i.e. immediately after the header).
    pub offset: u64,
}

/// File footer: roots are the indices of the top-level scene-graph
/// blocks (always `NiNode`s in Bully).
#[derive(Debug, Clone, Default)]
pub struct Footer {
    pub roots: Vec<i32>,
}

/// Discriminated union of the block payloads we know how to parse.
/// Add new variants as more block types are implemented.
#[derive(Debug, Clone)]
pub enum BlockPayload {
    NiNode(NiNodeData),
    NiTriShape(NiTriShapeData),
    NiTriStrips(NiTriStripsData),
    NiTriShapeData(NiTriShapeDataPayload),
    NiTriStripsData(NiTriStripsDataPayload),
    NiStringExtraData(NiStringExtraDataData),
    NiSourceTexture(NiSourceTextureData),
    NiMaterialProperty(NiMaterialPropertyData),
    NiTexturingProperty(NiTexturingPropertyData),
    NiAlphaProperty(NiAlphaPropertyData),
    NiZBufferProperty(NiZBufferPropertyData),
    NiSpecularProperty(NiSpecularPropertyData),
    NiStencilProperty(NiStencilPropertyData),
    NiVertexColorProperty(NiVertexColorPropertyData),
    /// Block type not yet implemented; the raw bytes are preserved
    /// for the hex view in the inspector panel.
    Unsupported { raw: Vec<u8> },
}

/// 12 bytes: `f32 x, f32 y, f32 z`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// 16 bytes: `f32 r, f32 g, f32 b, f32 a`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Color4 {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

/// 8 bytes: `f32 u, f32 v`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TexCoord {
    pub u: f32,
    pub v: f32,
}

/// 36 bytes: 9 × f32 in column-major order
/// (`m11 m21 m31 m12 m22 m32 m13 m23 m33`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Matrix33 {
    pub m: [[f32; 3]; 3],
}

impl Matrix33 {
    pub fn identity() -> Self {
        Self {
            m: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        }
    }
}

/// 16 bytes: `Vector3 center` + `f32 radius`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct NiBound {
    pub center: Vector3,
    pub radius: f32,
}

/// 52 bytes: `Matrix33` + `Vector3` + `f32`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct NiTransform {
    pub rotation: Matrix33,
    pub translation: Vector3,
    pub scale: f32,
}

// ---- Block payload types -------------------------------------------------

/// Fields specific to `NiNode` (the rest are inherited from
/// `NiAVObject` and `NiObjectNET` and are exposed as part of the
/// scene graph in `nif::Scene`).
#[derive(Debug, Clone, Default)]
pub struct NiNodeData {
    pub name: Option<String>,
    pub translation: Vector3,
    pub rotation: Matrix33,
    pub scale: f32,
    pub flags: u16,
    pub properties: Vec<i32>,
    pub collision_object: i32,
    pub children: Vec<i32>,
    pub effects: Vec<i32>,
    pub extra_data: Vec<i32>,
    pub controller: i32,
}

/// Fields specific to `NiTriShape` / `NiTriStrips`. They differ only
/// in the data block they point to; the parser keeps the data-block
/// ref and the material data inline.
#[derive(Debug, Clone, Default)]
pub struct NiTriShapeData {
    pub name: Option<String>,
    pub translation: Vector3,
    pub rotation: Matrix33,
    pub scale: f32,
    pub flags: u16,
    pub properties: Vec<i32>,
    pub collision_object: i32,
    pub data_ref: i32,
    pub skin_instance_ref: i32,
    pub material_data: MaterialData,
    pub extra_data: Vec<i32>,
    pub controller: i32,
}

#[derive(Debug, Clone, Default)]
pub struct NiTriStripsData {
    pub base: NiTriShapeData,
}

/// Material reference table attached to a `NiGeometry` since
/// 20.2.0.5.
#[derive(Debug, Clone, Default)]
pub struct MaterialData {
    pub names: Vec<u32>,
    pub extra: Vec<i32>,
    pub active: i32,
    pub needs_update: bool,
}

#[derive(Debug, Clone, Default)]
pub struct NiStringExtraDataData {
    pub string_index: u32,
    pub string: Option<String>,
    pub name: Option<String>,
    pub extra_data: Vec<i32>,
    pub controller: i32,
}

#[derive(Debug, Clone, Default)]
pub struct NiSourceTextureData {
    pub use_external: u8,
    pub file_name_index: u32,
    pub file_name: Option<String>,
    pub pixel_layout: u32,
    pub use_mipmaps: u32,
    pub alpha_format: u32,
    pub is_static: u8,
    pub direct_render: bool,
    pub persist_render_data: bool,
}

#[derive(Debug, Clone, Default)]
pub struct NiMaterialPropertyData {
    pub ambient: [f32; 3],
    pub diffuse: [f32; 3],
    pub specular: [f32; 3],
    pub emissive: [f32; 3],
    pub glossiness: f32,
    pub alpha: f32,
    pub emissive_mult: f32,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct NiAlphaPropertyData {
    pub flags: u16,
    pub threshold: u8,
}

#[derive(Debug, Clone, Default)]
pub struct NiZBufferPropertyData {
    pub flags: u16,
}

#[derive(Debug, Clone, Default)]
pub struct NiSpecularPropertyData {
    pub flags: u16,
}

#[derive(Debug, Clone, Default)]
pub struct NiStencilPropertyData {
    pub flags: u16,
    pub stencil_ref: u32,
    pub stencil_mask: u32,
}

#[derive(Debug, Clone, Default)]
pub struct NiVertexColorPropertyData {
    pub flags: u16,
}

/// Single texture slot in a `NiTexturingProperty`.
#[derive(Debug, Clone, Default)]
pub struct TexDesc {
    pub source_ref: i32,
    pub flags: u16,
    pub has_transform: bool,
    pub translation: Option<TexCoord>,
    pub scale: Option<TexCoord>,
    pub rotation: Option<f32>,
    pub transform_method: Option<u32>,
    pub center: Option<TexCoord>,
}

#[derive(Debug, Clone, Default)]
pub struct NiTexturingPropertyData {
    pub flags: u16,
    pub texture_count: u32,
    pub base: Option<TexDesc>,
    pub dark: Option<TexDesc>,
    pub detail: Option<TexDesc>,
    pub gloss: Option<TexDesc>,
    pub glow: Option<TexDesc>,
    pub bump_map: Option<TexDesc>,
    pub decal: [Option<TexDesc>; 4],
}

/// A single triangle index triple.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Triangle {
    pub v0: u16,
    pub v1: u16,
    pub v2: u16,
}

/// Vertex data payload of `NiTriShapeData` / `NiTriStripsData`. The
/// `triangles` field is non-empty for `NiTriShapeData`; the `strips`
/// field is non-empty for `NiTriStripsData` (the parser fills exactly
/// one of them depending on the source block).
#[derive(Debug, Clone, Default)]
pub struct NiTriShapeDataPayload {
    pub group_id: i32,
    pub num_vertices: u16,
    pub keep_flags: u8,
    pub compress_flags: u8,
    pub has_vertices: bool,
    pub vertices: Vec<Vector3>,
    pub data_flags: u16,
    pub has_normals: bool,
    pub normals: Vec<Vector3>,
    pub has_tangents: bool,
    pub tangents: Vec<Vector3>,
    pub bitangents: Vec<Vector3>,
    pub bounding_sphere: NiBound,
    pub has_vertex_colors: bool,
    pub vertex_colors: Vec<Color4>,
    pub num_uv_sets: u16,
    /// Flattened UVs: `uvs[set * num_vertices + vertex]`. Only the
    /// first set is exposed via the v3.0 MVP inspector.
    pub uvs: Vec<TexCoord>,
    pub consistency_flags: u16,
    pub additional_data_ref: i32,
    pub triangles: Vec<Triangle>,
}

#[derive(Debug, Clone, Default)]
pub struct NiTriStripsDataPayload {
    pub base: NiTriShapeDataPayload,
    pub num_strips: u16,
    pub strip_lengths: Vec<u16>,
    pub has_points: bool,
    pub points: Vec<u16>,
}

// ---- Cursor-based reader -------------------------------------------------

/// A cursor over a borrowed byte slice that respects a fixed
/// endianness for all multi-byte reads. Strings are stored as
/// length-prefixed Pascal strings with no padding.
pub(crate) struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
    endian: Endian,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(data: &'a [u8], endian: Endian) -> Self {
        Self { data, pos: 0, endian }
    }

    pub(crate) fn position(&self) -> usize {
        self.pos
    }

    pub(crate) fn set_position(&mut self, pos: usize) {
        self.pos = pos;
    }

    pub(crate) fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    pub(crate) fn eof(&self) -> bool {
        self.remaining() == 0
    }

    pub(crate) fn require(&self, n: usize, what: &'static str) -> NifResult<()> {
        if self.remaining() < n {
            Err(NifError::UnexpectedEof(what))
        } else {
            Ok(())
        }
    }

    pub(crate) fn read_u8(&mut self, what: &'static str) -> NifResult<u8> {
        self.require(1, what)?;
        let v = self.data[self.pos];
        self.pos += 1;
        Ok(v)
    }

    pub(crate) fn read_u16(&mut self, what: &'static str) -> NifResult<u16> {
        self.require(2, what)?;
        let bytes = [self.data[self.pos], self.data[self.pos + 1]];
        self.pos += 2;
        Ok(self.endian.read_u16(bytes))
    }

    pub(crate) fn read_u32(&mut self, what: &'static str) -> NifResult<u32> {
        self.require(4, what)?;
        let bytes = [
            self.data[self.pos],
            self.data[self.pos + 1],
            self.data[self.pos + 2],
            self.data[self.pos + 3],
        ];
        self.pos += 4;
        Ok(self.endian.read_u32(bytes))
    }

    pub(crate) fn read_i32(&mut self, what: &'static str) -> NifResult<i32> {
        self.require(4, what)?;
        let bytes = [
            self.data[self.pos],
            self.data[self.pos + 1],
            self.data[self.pos + 2],
            self.data[self.pos + 3],
        ];
        self.pos += 4;
        Ok(self.endian.read_i32(bytes))
    }

    pub(crate) fn read_f32(&mut self, what: &'static str) -> NifResult<f32> {
        self.require(4, what)?;
        let bytes = [
            self.data[self.pos],
            self.data[self.pos + 1],
            self.data[self.pos + 2],
            self.data[self.pos + 3],
        ];
        self.pos += 4;
        Ok(self.endian.read_f32(bytes))
    }

    pub(crate) fn read_bool(&mut self, what: &'static str) -> NifResult<bool> {
        Ok(self.read_u8(what)? != 0)
    }

    pub(crate) fn read_vector3(&mut self, what: &'static str) -> NifResult<Vector3> {
        let x = self.read_f32(what)?;
        let y = self.read_f32(what)?;
        let z = self.read_f32(what)?;
        Ok(Vector3 { x, y, z })
    }

    pub(crate) fn read_color4(&mut self, what: &'static str) -> NifResult<Color4> {
        let r = self.read_f32(what)?;
        let g = self.read_f32(what)?;
        let b = self.read_f32(what)?;
        let a = self.read_f32(what)?;
        Ok(Color4 { r, g, b, a })
    }

    pub(crate) fn read_texcoord(&mut self, what: &'static str) -> NifResult<TexCoord> {
        let u = self.read_f32(what)?;
        let v = self.read_f32(what)?;
        Ok(TexCoord { u, v })
    }

    pub(crate) fn read_matrix33(&mut self, what: &'static str) -> NifResult<Matrix33> {
        // Column-major: m11 m21 m31 m12 m22 m32 m13 m23 m33
        let mut m = [[0.0f32; 3]; 3];
        for col in 0..3 {
            for row in 0..3 {
                m[col][row] = self.read_f32(what)?;
            }
        }
        Ok(Matrix33 { m })
    }

    pub(crate) fn read_ni_bound(&mut self, what: &'static str) -> NifResult<NiBound> {
        let center = self.read_vector3(what)?;
        let radius = self.read_f32(what)?;
        Ok(NiBound { center, radius })
    }

    /// Read a length-prefixed Pascal string with **no** terminator and
    /// **no** alignment padding. This is the on-disk encoding used by
    /// the NIF header for both block type names and the string table.
    pub(crate) fn read_sized_string(&mut self, what: &'static str) -> NifResult<String> {
        let len = self.read_u32(what)? as usize;
        self.require(len, what)?;
        let s = std::str::from_utf8(&self.data[self.pos..self.pos + len])
            .map_err(|e| NifError::InvalidField(what, format!("non-UTF8: {e}")))?;
        self.pos += len;
        Ok(s.to_string())
    }

    /// `NiFixedString` / `FilePath` / `string` since 20.1.0.3: a
    /// 4-byte index into the header string table.
    pub(crate) fn read_ni_fixed_string_index(
        &mut self,
        what: &'static str,
    ) -> NifResult<u32> {
        self.read_u32(what)
    }

    /// Read an array of `i32` references. Indices are signed; -1
    /// means "no reference" in the niftools spec.
    pub(crate) fn read_i32_array(
        &mut self,
        len: usize,
        what: &'static str,
    ) -> NifResult<Vec<i32>> {
        let mut out = Vec::with_capacity(len);
        for _ in 0..len {
            out.push(self.read_i32(what)?);
        }
        Ok(out)
    }
}

// ---- Top-level parser ----------------------------------------------------

impl NifFile {
    /// Parse a Gamebryo NIF 20.3.0.9 file. The `bytes` slice may be
    /// longer than the file; only the leading `file_size` bytes are
    /// consumed.
    pub fn parse(bytes: &[u8]) -> NifResult<Self> {
        if bytes.len() < 6 {
            return Err(NifError::TooShort(bytes.len()));
        }
        let mut header = [0u8; 6];
        header.copy_from_slice(&bytes[..6]);
        if !header.starts_with(b"Gamebr") {
            return Err(NifError::BadHeader(header));
        }

        // Find the header line terminator (0x0A).
        let newline_pos = bytes
            .iter()
            .position(|&b| b == 0x0A)
            .ok_or(NifError::UnexpectedEof("header line"))?;
        let header_line = std::str::from_utf8(&bytes[..newline_pos])
            .map_err(|e| NifError::InvalidField("header_line", format!("non-UTF8: {e}")))?
            .to_string();

        let mut r = Reader::new(bytes, Endian::Little); // endian not yet known
        r.set_position(newline_pos + 1);

        let version = r.read_u32("version")?;
        if version != BULLY_NIF_VERSION {
            return Err(NifError::UnsupportedVersion(version));
        }
        let endian_byte = r.read_u8("endian")?;
        let endian = Endian::from_byte(endian_byte);
        r.endian = endian;

        let user_version = r.read_u32("user_version")?;
        let num_blocks = r.read_u32("num_blocks")? as usize;
        let num_block_types = r.read_u16("num_block_types")? as usize;

        // Bully user_version == 0: no BSHeader. If we ever need
        // 0x10000 (Divinity 2) or higher, insert the BSHeader
        // reader here. Documented in bully_nif_format.md §"Header".
        if user_version != 0 {
            // Skip unknown future formats; treat as still acceptable
            // for the simple header but mark them as such. Real
            // parsing will likely fail at the string table.
        }

        let mut block_types = Vec::with_capacity(num_block_types);
        for i in 0..num_block_types {
            block_types.push(r.read_sized_string("block_type")?);
            let _ = i;
        }

        let block_type_index = {
            let mut v = Vec::with_capacity(num_blocks);
            for _ in 0..num_blocks {
                v.push(r.read_u16("block_type_index")?);
            }
            v
        };
        let block_sizes = {
            let mut v = Vec::with_capacity(num_blocks);
            for _ in 0..num_blocks {
                v.push(r.read_u32("block_size")?);
            }
            v
        };

        let num_strings = r.read_u32("num_strings")? as usize;
        let max_string_length = r.read_u32("max_string_length")? as usize;
        let mut strings = Vec::with_capacity(num_strings);
        for _ in 0..num_strings {
            strings.push(r.read_sized_string("string")?);
        }
        let _ = max_string_length;

        let num_groups = r.read_u32("num_groups")? as usize;
        for _ in 0..num_groups {
            // Group payload is undocumented in nifxml for 20.3.0.9 and
            // is always 0 for Bully. Skip conservatively by reading 0
            // bytes (the XML says `Groups` is `uint[Num Groups]`).
            for _ in 0..4 {
                r.read_u8("group")?;
            }
        }

        let header_end = r.position() as u64;

        // ---- Build block metadata --------------------------------------
        let mut blocks = Vec::with_capacity(num_blocks);
        let mut cursor = header_end;
        for (i, (&type_index, &size)) in
            block_type_index.iter().zip(block_sizes.iter()).enumerate()
        {
            let type_name = block_types
                .get(type_index as usize)
                .cloned()
                .ok_or_else(|| {
                    NifError::InvalidField(
                        "block_type_index",
                        format!("block {i} references unknown type {type_index}"),
                    )
                })?;
            blocks.push(BlockMeta {
                type_index,
                type_name,
                size,
                offset: cursor,
            });
            cursor = cursor
                .checked_add(size as u64)
                .ok_or_else(|| NifError::InvalidField("block_size", "overflow".into()))?;
        }

        // ---- Parse block payloads --------------------------------------
        let mut payloads = Vec::with_capacity(num_blocks);
        for block in &blocks {
            let end = block
                .offset
                .checked_add(block.size as u64)
                .ok_or_else(|| NifError::InvalidField("block_size", "overflow".into()))?;
            if end > bytes.len() as u64 {
                return Err(NifError::TruncatedBlock {
                    block: blocks.len(),
                    block_type: block.type_name.clone(),
                    offset: block.offset,
                    expected: block.size as u64,
                    actual: (bytes.len() as u64).saturating_sub(block.offset),
                });
            }
            let raw = &bytes[block.offset as usize..end as usize];
            payloads.push(Some(parse_block(&block.type_name, raw, endian)?));
        }

        // ---- Parse footer ----------------------------------------------
        let footer_start = cursor as usize;
        let mut r2 = Reader::new(bytes, endian);
        r2.set_position(footer_start);
        let num_roots = r2.read_u32("num_roots")? as usize;
        let mut roots = Vec::with_capacity(num_roots);
        for _ in 0..num_roots {
            roots.push(r2.read_i32("root")?);
        }
        let footer = Footer { roots };

        Ok(Self {
            header_line,
            version,
            endian,
            user_version,
            strings,
            block_types,
            blocks,
            payloads,
            footer,
        })
    }

    /// Resolve a string-table index to its text.
    pub fn string(&self, index: u32) -> Option<&str> {
        if index == 0xFFFFFFFF {
            return None;
        }
        self.strings.get(index as usize).map(String::as_str)
    }

    /// Get the parsed payload of a block, by index.
    pub fn payload(&self, index: usize) -> Option<&BlockPayload> {
        self.payloads.get(index).and_then(|p| p.as_ref())
    }
}

fn parse_block(type_name: &str, raw: &[u8], endian: Endian) -> NifResult<BlockPayload> {
    let mut r = Reader::new(raw, endian);
    let payload = match type_name {
        "NiNode" => BlockPayload::NiNode(read_ni_node(&mut r)?),
        "NiTriShape" => BlockPayload::NiTriShape(read_ni_tri_shape(&mut r)?),
        "NiTriStrips" => {
            let base = read_ni_tri_shape(&mut r)?;
            BlockPayload::NiTriStrips(NiTriStripsData { base })
        }
        "NiTriShapeData" => BlockPayload::NiTriShapeData(read_ni_tri_shape_data(&mut r)?),
        "NiTriStripsData" => {
            let base = read_ni_tri_shape_data(&mut r)?;
            let (num_strips, strip_lengths, has_points, points) = read_strips_footer(&mut r);
            BlockPayload::NiTriStripsData(NiTriStripsDataPayload {
                base,
                num_strips,
                strip_lengths,
                has_points,
                points,
            })
        }
        t @ ("NiStringExtraData" | "NiSourceTexture" | "NiMaterialProperty"
             | "NiTexturingProperty" | "NiAlphaProperty" | "NiZBufferProperty"
             | "NiSpecularProperty" | "NiStencilProperty"
             | "NiVertexColorProperty") => {
            match t {
                "NiStringExtraData" => read_ni_string_extra_data(&mut r)
                    .map(BlockPayload::NiStringExtraData),
                "NiSourceTexture" => read_ni_source_texture(&mut r)
                    .map(BlockPayload::NiSourceTexture),
                "NiMaterialProperty" => read_ni_material_property(&mut r)
                    .map(BlockPayload::NiMaterialProperty),
                "NiTexturingProperty" => read_ni_texturing_property(&mut r)
                    .map(BlockPayload::NiTexturingProperty),
                "NiAlphaProperty" => read_ni_alpha_property(&mut r)
                    .map(BlockPayload::NiAlphaProperty),
                "NiZBufferProperty" => read_ni_zbuffer_property(&mut r)
                    .map(BlockPayload::NiZBufferProperty),
                "NiSpecularProperty" => read_ni_specular_property(&mut r)
                    .map(BlockPayload::NiSpecularProperty),
                "NiStencilProperty" => read_ni_stencil_property(&mut r)
                    .map(BlockPayload::NiStencilProperty),
                "NiVertexColorProperty" => read_ni_vertex_color_property(&mut r)
                    .map(BlockPayload::NiVertexColorProperty),
                _ => unreachable!(),
            }
            .unwrap_or_else(|e| {
                eprintln!("[IMGEditor] NIF: skipping block {type_name}: {e}");
                BlockPayload::Unsupported { raw: raw.to_vec() }
            })
        }
        _ => BlockPayload::Unsupported {
            raw: raw.to_vec(),
        },
    };
    Ok(payload)
}

// ---- Block readers -------------------------------------------------------

fn read_ni_object_net(r: &mut Reader<'_>, target: &mut NiNodeData) -> NifResult<()> {
    target.name = {
        let idx = r.read_ni_fixed_string_index("name")?;
        match idx {
            0xFFFFFFFF => None,
            i => Some(format!("__string_idx_{i}")),
        }
    };
    target.extra_data = {
        let n = r.read_u32("num_extra_data")? as usize;
        r.read_i32_array(n, "extra_data")?
    };
    target.controller = r.read_i32("controller")?;
    Ok(())
}

fn read_ni_av_object(r: &mut Reader<'_>, target: &mut NiNodeData) -> NifResult<()> {
    target.flags = r.read_u16("flags")?;
    target.translation = r.read_vector3("translation")?;
    target.rotation = r.read_matrix33("rotation")?;
    target.scale = r.read_f32("scale")?;
    target.properties = {
        let n = r.read_u32("num_properties")? as usize;
        r.read_i32_array(n, "property")?
    };
    target.collision_object = r.read_i32("collision_object")?;
    Ok(())
}

fn read_ni_node(r: &mut Reader<'_>) -> NifResult<NiNodeData> {
    let mut node = NiNodeData::default();
    read_ni_object_net(r, &mut node)?;
    read_ni_av_object(r, &mut node)?;
    node.children = {
        let n = r.read_u32("num_children")? as usize;
        r.read_i32_array(n, "child")?
    };
    node.effects = {
        let n = r.read_u32("num_effects")? as usize;
        r.read_i32_array(n, "effect")?
    };
    Ok(node)
}

fn read_material_data(r: &mut Reader<'_>) -> NifResult<MaterialData> {
    let num_materials = r.read_u32("num_materials")? as usize;
    let mut names = Vec::with_capacity(num_materials);
    for _ in 0..num_materials {
        names.push(r.read_u32("material_name")?);
    }
    let extra = r.read_i32_array(num_materials, "material_extra")?;
    let active = r.read_i32("active_material")?;
    let needs_update = r.read_bool("material_needs_update")?;
    Ok(MaterialData {
        names,
        extra,
        active,
        needs_update,
    })
}

fn read_ni_tri_shape(r: &mut Reader<'_>) -> NifResult<NiTriShapeData> {
    read_geometry_block(r)
}

fn read_geometry_block(r: &mut Reader<'_>) -> NifResult<NiTriShapeData> {
    let mut tri = NiTriShapeData::default();
    // NiObjectNET
    let name_idx = r.read_ni_fixed_string_index("name")?;
    tri.name = if name_idx == 0xFFFFFFFF {
        None
    } else {
        Some(format!("__string_idx_{name_idx}"))
    };
    tri.extra_data = {
        let n = r.read_u32("num_extra_data")? as usize;
        r.read_i32_array(n, "extra_data")?
    };
    tri.controller = r.read_i32("controller")?;
    // NiAVObject
    tri.flags = r.read_u16("flags")?;
    tri.translation = r.read_vector3("translation")?;
    tri.rotation = r.read_matrix33("rotation")?;
    tri.scale = r.read_f32("scale")?;
    tri.properties = {
        let n = r.read_u32("num_properties")? as usize;
        r.read_i32_array(n, "property")?
    };
    tri.collision_object = r.read_i32("collision_object")?;
    // NiGeometry
    tri.data_ref = r.read_i32("data")?;
    tri.skin_instance_ref = r.read_i32("skin_instance")?;
    tri.material_data = read_material_data(r)?;
    Ok(tri)
}

fn read_ni_string_extra_data(r: &mut Reader<'_>) -> NifResult<NiStringExtraDataData> {
    let mut data = NiStringExtraDataData::default();
    let name_idx = r.read_ni_fixed_string_index("name")?;
    data.name = if name_idx == 0xFFFFFFFF {
        None
    } else {
        Some(format!("__string_idx_{name_idx}"))
    };
    data.extra_data = {
        let n = r.read_u32("num_extra_data")? as usize;
        r.read_i32_array(n, "extra_data")?
    };
    data.controller = r.read_i32("controller")?;
    data.string_index = r.read_u32("string_data")?;
    Ok(data)
}

fn read_ni_source_texture(r: &mut Reader<'_>) -> NifResult<NiSourceTextureData> {
    let name_idx = r.read_ni_fixed_string_index("name")?;
    let _ = name_idx;
    // NiObjectNET: extra_data list + controller  (NiTexture adds no fields)
    let _num_extra = r.read_u32("num_extra_data")? as usize;
    for _ in 0.._num_extra {
        let _ = r.read_i32("extra_data")?;
    }
    let _ = r.read_i32("controller")?;

    let mut tex = NiSourceTextureData::default();
    tex.use_external = r.read_u8("use_external")?;
    tex.file_name_index = r.read_ni_fixed_string_index("file_name")?;
    tex.pixel_layout = r.read_u32("pixel_layout")?;
    tex.use_mipmaps = r.read_u32("use_mipmaps")?;
    tex.alpha_format = r.read_u32("alpha_format")?;
    tex.is_static = r.read_u8("is_static")?;
    tex.direct_render = r.read_bool("direct_render")?;
    tex.persist_render_data = r.read_bool("persist_render_data")?;
    Ok(tex)
}

fn read_ni_material_property(r: &mut Reader<'_>) -> NifResult<NiMaterialPropertyData> {
    let name_idx = r.read_ni_fixed_string_index("name")?;
    // NiObjectNET: extra_data list + controller
    let _num_extra = r.read_u32("num_extra_data")? as usize;
    for _ in 0.._num_extra {
        let _ = r.read_i32("extra_data")?;
    }
    let _ = r.read_i32("controller")?;

    let mut mat = NiMaterialPropertyData::default();
    mat.name = if name_idx == 0xFFFFFFFF {
        None
    } else {
        Some(format!("__string_idx_{name_idx}"))
    };
    mat.ambient = [
        r.read_f32("ambient.r")?,
        r.read_f32("ambient.g")?,
        r.read_f32("ambient.b")?,
    ];
    mat.diffuse = [
        r.read_f32("diffuse.r")?,
        r.read_f32("diffuse.g")?,
        r.read_f32("diffuse.b")?,
    ];
    mat.specular = [
        r.read_f32("specular.r")?,
        r.read_f32("specular.g")?,
        r.read_f32("specular.b")?,
    ];
    mat.emissive = [
        r.read_f32("emissive.r")?,
        r.read_f32("emissive.g")?,
        r.read_f32("emissive.b")?,
    ];
    mat.glossiness = r.read_f32("glossiness")?;
    mat.alpha = r.read_f32("alpha")?;
    mat.emissive_mult = 1.0; // absent for Bully (bsver == 0)
    Ok(mat)
}

fn read_ni_alpha_property(r: &mut Reader<'_>) -> NifResult<NiAlphaPropertyData> {
    let flags = r.read_u16("flags")?;
    let threshold = r.read_u8("threshold")?;
    Ok(NiAlphaPropertyData { flags, threshold })
}

fn read_ni_zbuffer_property(r: &mut Reader<'_>) -> NifResult<NiZBufferPropertyData> {
    Ok(NiZBufferPropertyData {
        flags: r.read_u16("flags")?,
    })
}

fn read_ni_specular_property(r: &mut Reader<'_>) -> NifResult<NiSpecularPropertyData> {
    Ok(NiSpecularPropertyData {
        flags: r.read_u16("flags")?,
    })
}

fn read_ni_stencil_property(r: &mut Reader<'_>) -> NifResult<NiStencilPropertyData> {
    Ok(NiStencilPropertyData {
        flags: r.read_u16("flags")?,
        stencil_ref: r.read_u32("stencil_ref")?,
        stencil_mask: r.read_u32("stencil_mask")?,
    })
}

fn read_ni_vertex_color_property(r: &mut Reader<'_>) -> NifResult<NiVertexColorPropertyData> {
    Ok(NiVertexColorPropertyData {
        flags: r.read_u16("flags")?,
    })
}

fn read_tex_desc(r: &mut Reader<'_>) -> NifResult<TexDesc> {
    let source_ref = r.read_i32("texdesc.source")?;
    let flags = r.read_u16("texdesc.flags")?;
    let has_transform = r.read_bool("texdesc.has_transform")?;
    let (translation, scale, rotation, transform_method, center) = if has_transform {
        let t = r.read_texcoord("texdesc.translation")?;
        let s = r.read_texcoord("texdesc.scale")?;
        let rot = r.read_f32("texdesc.rotation")?;
        let method = r.read_u32("texdesc.transform_method")?;
        let c = r.read_texcoord("texdesc.center")?;
        (Some(t), Some(s), Some(rot), Some(method), Some(c))
    } else {
        (None, None, None, None, None)
    };
    Ok(TexDesc {
        source_ref,
        flags,
        has_transform,
        translation,
        scale,
        rotation,
        transform_method,
        center,
    })
}

fn read_ni_texturing_property(r: &mut Reader<'_>) -> NifResult<NiTexturingPropertyData> {
    let flags = r.read_u16("flags")?;
    let texture_count = r.read_u32("texture_count")?;
    let mut out = NiTexturingPropertyData {
        flags,
        texture_count,
        ..Default::default()
    };

    // Bully on-disk layout: 11 (has + TexDesc) pairs read in fixed
    // order regardless of `texture_count`:
    //   0 Base, 1 Dark, 2 Detail, 3 Gloss, 4 Glow, 5 Bump,
    //   6 Decal 0, 7 Decal 1, 8 Decal 2, 9 Decal 3.
    // A `has` byte of 0 skips the corresponding `TexDesc` body.
    let mut slots = [const { None }; 11];
    for slot in slots.iter_mut() {
        let has = r.read_bool("has_tex")?;
        if has {
            *slot = Some(read_tex_desc(r)?);
        }
    }

    out.base = slots[0].take();
    out.dark = slots[1].take();
    out.detail = slots[2].take();
    out.gloss = slots[3].take();
    out.glow = slots[4].take();
    out.bump_map = slots[5].take();
    out.decal = [slots[6].take(), slots[7].take(), slots[8].take(), slots[9].take()];

    let num_shader = r.read_u32("num_shader_textures")?;
    let _ = num_shader;
    Ok(out)
}

fn read_ni_tri_shape_data(r: &mut Reader<'_>) -> NifResult<NiTriShapeDataPayload> {
    let mut out = NiTriShapeDataPayload::default();
    out.group_id = r.read_i32("group_id")?;
    out.num_vertices = r.read_u16("num_vertices")?;
    out.keep_flags = r.read_u8("keep_flags")?;
    out.compress_flags = r.read_u8("compress_flags")?;
    out.has_vertices = r.read_bool("has_vertices")?;
    if out.has_vertices {
        out.vertices = (0..out.num_vertices as usize)
            .map(|_| r.read_vector3("vertex"))
            .collect::<NifResult<Vec<_>>>()?;
    }
    out.data_flags = r.read_u16("data_flags")?;
    out.has_normals = r.read_bool("has_normals")?;
    if out.has_normals {
        out.normals = (0..out.num_vertices as usize)
            .map(|_| r.read_vector3("normal"))
            .collect::<NifResult<Vec<_>>>()?;
    }
    let has_tangents = (out.data_flags & 0x1000) != 0 && out.has_normals;
    out.has_tangents = has_tangents;
    if has_tangents {
        out.tangents = (0..out.num_vertices as usize)
            .map(|_| r.read_vector3("tangent"))
            .collect::<NifResult<Vec<_>>>()?;
        out.bitangents = (0..out.num_vertices as usize)
            .map(|_| r.read_vector3("bitangent"))
            .collect::<NifResult<Vec<_>>>()?;
    }
    out.bounding_sphere = r.read_ni_bound("bounding_sphere")?;
    out.has_vertex_colors = r.read_bool("has_vertex_colors")?;
    if out.has_vertex_colors {
        out.vertex_colors = (0..out.num_vertices as usize)
            .map(|_| r.read_color4("vertex_color"))
            .collect::<NifResult<Vec<_>>>()?;
    }
    out.num_uv_sets = (out.data_flags & 0x3F) as u16;
    let total_uvs = (out.num_uv_sets as usize) * (out.num_vertices as usize);
    if r.remaining() >= total_uvs * 8 {
        out.uvs = (0..total_uvs)
            .map(|_| r.read_texcoord("uv"))
            .collect::<NifResult<Vec<_>>>()?;
    }
    out.consistency_flags = r.read_u16("consistency_flags")?;
    out.additional_data_ref = r.read_i32("additional_data")?;
    // TriShapeData-specific: triangles
    if r.remaining() < 2 + 4 + 1 {
        // Truncated block; leave triangles empty.
        return Ok(out);
    }
    let num_triangles = r.read_u16("num_triangles")?;
    let _ = r.read_u32("num_triangle_points")?;
    let has_triangles = r.read_bool("has_triangles")?;
    if has_triangles {
        let mut triangles = Vec::with_capacity(num_triangles as usize);
        for _ in 0..num_triangles as usize {
            if r.remaining() < 6 {
                break;
            }
            let v0 = r.read_u16("triangle_v0")?;
            let v1 = r.read_u16("triangle_v1")?;
            let v2 = r.read_u16("triangle_v2")?;
            triangles.push(Triangle { v0, v1, v2 });
        }
        out.triangles = triangles;
    }
    let num_match_groups = r.read_u16("num_match_groups")?;
    for _ in 0..num_match_groups {
        if r.remaining() < 2 {
            break;
        }
        let n = r.read_u16("match_group_count")?;
        for _ in 0..n {
            if r.remaining() < 2 {
                break;
            }
            let _ = r.read_u16("match_group_index")?;
        }
    }
    Ok(out)
}

// ---- Post-parse name resolution ------------------------------------------

impl NifFile {
    /// After parsing, resolve the `__string_idx_N` placeholders in
    /// block payloads (which are set during read to avoid coupling
    /// the reader to the string table) to actual strings.
    pub fn resolve_string_indices(&mut self) {
        let strings = self.strings.clone();
        for payload in self.payloads.iter_mut().flatten() {
            match payload {
                BlockPayload::NiNode(d) => d.name = resolve_name(&strings, d.name.as_deref()),
                BlockPayload::NiTriShape(d) => {
                    d.name = resolve_name(&strings, d.name.as_deref())
                }
                BlockPayload::NiTriStrips(d) => {
                    d.base.name = resolve_name(&strings, d.base.name.as_deref())
                }
                BlockPayload::NiStringExtraData(d) => {
                    d.name = resolve_name(&strings, d.name.as_deref());
                    d.string = string_from(&strings, d.string_index);
                }
                BlockPayload::NiSourceTexture(d) => {
                    d.file_name = string_from(&strings, d.file_name_index);
                }
                BlockPayload::NiMaterialProperty(d) => {
                    d.name = resolve_name(&strings, d.name.as_deref())
                }
                _ => {}
            }
        }
    }
}

fn resolve_name(strings: &[String], name: Option<&str>) -> Option<String> {
    name.and_then(|n| {
        n.strip_prefix("__string_idx_")
            .and_then(|idx| idx.parse::<usize>().ok())
            .and_then(|i| strings.get(i).cloned())
    })
}

fn string_from(strings: &[String], index: u32) -> Option<String> {
    if index == 0xFFFFFFFF {
        None
    } else {
        strings.get(index as usize).cloned()
    }
}

/// Read the footer of a NiTriStripsData block (num_strips, strip_lengths,
/// has_points, points), tolerating truncation at every step.
fn read_strips_footer(r: &mut Reader<'_>) -> (u16, Vec<u16>, bool, Vec<u16>) {
    if r.remaining() < 2 {
        return (0, Vec::new(), false, Vec::new());
    }
    let num_strips = r.read_u16("num_strips").unwrap_or(0);
    if num_strips == 0 {
        let has_points = r.remaining() >= 1 && r.read_bool("has_points").unwrap_or(false);
        return (0, Vec::new(), has_points, Vec::new());
    }
    // strip_lengths are stored as i32 in the file (4 bytes each)
    let needed = num_strips as usize * 4;
    let strip_lengths = if r.remaining() >= needed {
        r.read_i32_array(num_strips as usize, "strip_length")
            .unwrap_or_default()
            .into_iter()
            .map(|v| v as u16)
            .collect()
    } else {
        Vec::new()
    };
    let has_points = r.remaining() >= 1 && r.read_bool("has_points").unwrap_or(false);
    let points = if has_points {
        let total: usize = strip_lengths.iter().map(|&l| l as usize).sum();
        let needed = total * 4;
        if r.remaining() >= needed {
            r.read_i32_array(total, "point")
                .unwrap_or_default()
                .into_iter()
                .map(|v| v as u16)
                .collect()
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };
    (num_strips, strip_lengths, has_points, points)
}
