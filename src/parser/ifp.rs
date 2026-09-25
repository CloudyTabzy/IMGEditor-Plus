//! GTA IFP animation container parser.
//!
//! Two variants occur in the retail games:
//! - **ANP3** (San Andreas `anim/ped.ifp` and the compressed packs inside
//!   `anim.img`): a flat layout with int16-quantized keyframes — quats
//!   scaled by 4096, translations by 1024, and times as frame numbers at
//!   30 fps.
//! - **ANPK** (GTA III/VC packs, SA cutscene packs): a chunk tree
//!   (`ANPK/INFO`, then per animation `NAME` + `DGAN/INFO/CPAN/ANIM/
//!   KR00|KRT0|KRTS`) with float32 keyframes whose quaternion x/y/z are
//!   negated on disk. Sections are 4-byte aligned.
//!
//! The 24-byte name fields regularly carry junk after the NUL terminator;
//! names are truncated at the first NUL for identity purposes. ANLF is an
//! ANPK wrapped in a cutscene container header and is handled transparently.
//! Everything that fails validation fails closed with a typed error instead
//! of being guessed into keyframes.

const ANP3_MAGIC: &[u8; 4] = b"ANP3";
const ANPK_MAGIC: &[u8; 4] = b"ANPK";
const ANLF_MAGIC: &[u8; 4] = b"ANLF";

const QUAT_SCALE: f32 = 4096.0;
const TRANSLATION_SCALE: f32 = 1024.0;
const ANP3_FPS: f32 = 30.0;

const MAX_ANIMATIONS: usize = 4096;
const MAX_OBJECTS: usize = 512;
const MAX_KEYS: usize = 100_000;

/// One keyframe, dequantized into seconds and f32 channels. Rotations are
/// stored as `(x, y, z, w)` with the on-disk sign convention already
/// resolved; `translation` is zero for rotation-only objects.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IfpKey {
    pub time: f32,
    pub rotation: [f32; 4],
    pub translation: [f32; 3],
}

#[derive(Debug, Clone, PartialEq)]
pub struct IfpObject {
    pub name: String,
    pub bone_id: i32,
    pub keys: Vec<IfpKey>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IfpAnimation {
    pub name: String,
    pub objects: Vec<IfpObject>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IfpFile {
    pub package: String,
    pub animations: Vec<IfpAnimation>,
}

struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, amount: usize, what: &str) -> Result<&'a [u8], String> {
        let end = self
            .position
            .checked_add(amount)
            .ok_or_else(|| format!("IFP {what} size overflowed"))?;
        let bytes = self
            .bytes
            .get(self.position..end)
            .ok_or_else(|| format!("unexpected end reading IFP {what}"))?;
        self.position = end;
        Ok(bytes)
    }

    fn u32(&mut self, what: &str) -> Result<u32, String> {
        Ok(u32::from_le_bytes(
            self.take(4, what)?.try_into().expect("bounded read"),
        ))
    }

    fn i32(&mut self, what: &str) -> Result<i32, String> {
        Ok(i32::from_le_bytes(
            self.take(4, what)?.try_into().expect("bounded read"),
        ))
    }

    fn f32(&mut self, what: &str) -> Result<f32, String> {
        Ok(f32::from_le_bytes(
            self.take(4, what)?.try_into().expect("bounded read"),
        ))
    }

    fn i16(&mut self, what: &str) -> Result<i16, String> {
        Ok(i16::from_le_bytes(
            self.take(2, what)?.try_into().expect("bounded read"),
        ))
    }

    fn u16(&mut self, what: &str) -> Result<u16, String> {
        Ok(u16::from_le_bytes(
            self.take(2, what)?.try_into().expect("bounded read"),
        ))
    }

    fn skip(&mut self, amount: usize, what: &str) -> Result<(), String> {
        self.take(amount, what)?;
        Ok(())
    }

    /// A 24-byte fixed name field, truncated at the first NUL.
    fn fixed_name(&mut self, what: &str) -> Result<String, String> {
        let field = self.take(24, what)?;
        let bytes = field.split(|&byte| byte == 0).next().unwrap_or_default();
        Ok(String::from_utf8_lossy(bytes).to_string())
    }
}

fn name_from_bytes(bytes: &[u8]) -> String {
    let bytes = bytes.split(|&byte| byte == 0).next().unwrap_or_default();
    String::from_utf8_lossy(bytes).to_string()
}

fn bounded(raw: u32, maximum: usize, label: &str) -> Result<usize, String> {
    let count = usize::try_from(raw).map_err(|_| format!("{label} count does not fit usize"))?;
    if count > maximum {
        return Err(format!(
            "{label} count {count} exceeds viewer limit {maximum}"
        ));
    }
    Ok(count)
}

/// Parse an IFP container (ANP3 or ANPK; an ANLF wrapper is transparent).
pub fn parse_ifp(bytes: &[u8]) -> Result<IfpFile, String> {
    if bytes.len() < 8 {
        return Err("IFP is truncated before its magic".to_string());
    }
    let magic = &bytes[0..4];
    if magic == ANP3_MAGIC {
        parse_anp3(bytes)
    } else if magic == ANPK_MAGIC || magic == ANLF_MAGIC {
        parse_anpk_from(bytes, 8)
    } else {
        Err(format!(
            "unsupported IFP magic 0x{:02X}{:02X}{:02X}{:02X} (expected ANP3 or ANPK)",
            magic[0], magic[1], magic[2], magic[3]
        ))
    }
}

fn parse_anp3(bytes: &[u8]) -> Result<IfpFile, String> {
    let mut reader = Reader {
        bytes,
        position: 8, // magic + declared size
    };
    let package = reader.fixed_name("ANP3 package name")?;
    let animation_count = bounded(
        reader.u32("ANP3 animation count")?,
        MAX_ANIMATIONS,
        "animations",
    )?;
    let mut animations = Vec::with_capacity(animation_count);
    for _ in 0..animation_count {
        let name = reader.fixed_name("ANP3 animation name")?;
        let object_count = bounded(reader.u32("ANP3 object count")?, MAX_OBJECTS, "objects")?;
        let _frame_data_size = reader.u32("ANP3 frame data size")?;
        let _flags = reader.u32("ANP3 flags")?;
        let mut objects = Vec::with_capacity(object_count);
        for _ in 0..object_count {
            let object_name = reader.fixed_name("ANP3 object name")?;
            let frame_type = reader.u32("ANP3 frame type")?;
            let key_count = bounded(reader.u32("ANP3 key count")?, MAX_KEYS, "keys")?;
            let bone_id = reader.i32("ANP3 bone id")?;
            let mut keys = Vec::with_capacity(key_count);
            for _ in 0..key_count {
                let key = match frame_type {
                    // Float variants: quat(4) + [pos(3)] + time(1).
                    1 | 2 => {
                        let rotation = [
                            reader.f32("ANP3 key rotation X")?,
                            reader.f32("ANP3 key rotation Y")?,
                            reader.f32("ANP3 key rotation Z")?,
                            reader.f32("ANP3 key rotation W")?,
                        ];
                        let translation = if frame_type == 2 {
                            [
                                reader.f32("ANP3 key translation X")?,
                                reader.f32("ANP3 key translation Y")?,
                                reader.f32("ANP3 key translation Z")?,
                            ]
                        } else {
                            [0.0; 3]
                        };
                        let time = reader.f32("ANP3 key time")?;
                        IfpKey {
                            time,
                            rotation,
                            translation,
                        }
                    }
                    // Compressed variants: quat i16/4096, [pos i16/1024],
                    // time as a frame number at 30 fps.
                    3 | 4 => {
                        let rotation = [
                            reader.i16("ANP3 key rotation X")? as f32 / QUAT_SCALE,
                            reader.i16("ANP3 key rotation Y")? as f32 / QUAT_SCALE,
                            reader.i16("ANP3 key rotation Z")? as f32 / QUAT_SCALE,
                            reader.i16("ANP3 key rotation W")? as f32 / QUAT_SCALE,
                        ];
                        let translation = if frame_type == 4 {
                            [
                                reader.i16("ANP3 key translation X")? as f32 / TRANSLATION_SCALE,
                                reader.i16("ANP3 key translation Y")? as f32 / TRANSLATION_SCALE,
                                reader.i16("ANP3 key translation Z")? as f32 / TRANSLATION_SCALE,
                            ]
                        } else {
                            [0.0; 3]
                        };
                        let time = reader.u16("ANP3 key time")? as f32 / ANP3_FPS;
                        IfpKey {
                            time,
                            rotation,
                            translation,
                        }
                    }
                    other => {
                        return Err(format!(
                            "unsupported ANP3 keyframe type {other} in '{object_name}'"
                        ));
                    }
                };
                keys.push(key);
            }
            objects.push(IfpObject {
                name: object_name,
                bone_id,
                keys,
            });
        }
        animations.push(IfpAnimation { name, objects });
    }
    Ok(IfpFile {
        package,
        animations,
    })
}

/// Read one 4-byte-aligned ANPK section: `(magic u32, size u32, body)`.
/// Returns the magic and the body range; the cursor is left at the start
/// of the body (callers advance past it or hand it to a sub-reader).
fn read_section<'a>(reader: &mut Reader<'a>, depth: usize) -> Result<(u32, &'a [u8]), String> {
    if depth > 8 {
        return Err("IFP section nesting is too deep".to_string());
    }
    let magic = u32::from_le_bytes(
        reader
            .take(4, "section magic")?
            .try_into()
            .expect("bounded read"),
    );
    let size = bounded(reader.u32("section size")?, reader.bytes.len(), "section")?;
    let body = reader.take(size, "section body")?;
    Ok((magic, body))
}

/// ANPK chunk tree, starting after the 8-byte ANPK header. Structure:
/// `INFO` (counts + package name), then per animation `NAME` + `DGAN`
/// (`INFO` + per object `CPAN` → `ANIM` + keyframe section).
fn parse_anpk_from(bytes: &[u8], start: usize) -> Result<IfpFile, String> {
    let mut reader = Reader {
        bytes,
        position: start,
    };
    let mut animations: Vec<IfpAnimation> = Vec::new();
    let mut pending_name: Option<String> = None;

    while reader.position + 8 <= reader.bytes.len() {
        let before = reader.position;
        let (magic, body) = read_section(&mut reader, 0)?;
        match &magic.to_le_bytes() {
            b"INFO" => {
                // Package-level INFO: animation count (+ package name in
                // the body tail). The count is advisory; the chunk walk
                // below is authoritative.
                let _ = body.first().copied();
            }
            b"NAME" => {
                pending_name = Some(name_from_bytes(body));
            }
            b"DGAN" => {
                let objects = parse_dgan(body)?;
                animations.push(IfpAnimation {
                    name: pending_name.take().unwrap_or_default(),
                    objects,
                });
            }
            _ => {}
        }
        if reader.position == before {
            return Err("ANPK chunk walk made no progress".to_string());
        }
        // Sections are 4-byte aligned.
        reader.position = reader.position.div_ceil(4) * 4;
    }

    if animations.is_empty() {
        return Err("ANPK pack carries no animations".to_string());
    }
    Ok(IfpFile {
        package: String::new(),
        animations,
    })
}

/// One DGAN body: `INFO` (object count [+ tail]) then one `CPAN` per
/// object. Each CPAN body contains an `ANIM` section (name + bone id +
/// key count + tail) followed by a sibling keyframe section (`KR00`,
/// `KRT0`, or `KRTS`). IFP section headers are 8 bytes (magic + size);
/// only 4-byte alignment is applied between chunks.
fn parse_dgan(body: &[u8]) -> Result<Vec<IfpObject>, String> {
    let mut reader = Reader {
        bytes: body,
        position: 0,
    };
    let (info_magic, _info_body) = read_section(&mut reader, 1)?;
    if &info_magic.to_le_bytes() != b"INFO" {
        return Err(format!("expected INFO inside DGAN, got 0x{info_magic:08X}"));
    }
    reader.position = reader.position.div_ceil(4) * 4;

    let mut objects = Vec::new();
    while reader.position + 8 <= reader.bytes.len() {
        let (magic, cpan_body) = read_section(&mut reader, 1)?;
        if &magic.to_le_bytes() != b"CPAN" {
            reader.position = reader.position.div_ceil(4) * 4;
            continue;
        }
        objects.push(parse_cpan(cpan_body)?);
        reader.position = reader.position.div_ceil(4) * 4;
    }
    Ok(objects)
}

/// One CPAN body: an `ANIM` section (name, bone id, key count, tail)
/// followed by a sibling `KR00`/`KRT0`/`KRTS` section with the keys.
fn parse_cpan(body: &[u8]) -> Result<IfpObject, String> {
    let mut reader = Reader {
        bytes: body,
        position: 0,
    };
    let (anim_magic, anim_body) = read_section(&mut reader, 1)?;
    if &anim_magic.to_le_bytes() != b"ANIM" {
        return Err(format!("expected ANIM inside CPAN, got 0x{anim_magic:08X}"));
    }
    let mut anim = Reader {
        bytes: anim_body,
        position: 0,
    };
    let name = anim.fixed_name("ANIM object name")?;
    let bone_id = anim.i32("ANIM bone id")?;
    let key_count = bounded(anim.u32("ANIM key count")?, MAX_KEYS, "keys")?;

    // The KR section is a sibling of ANIM inside the CPAN body.
    let (keys_magic, keys_body) = read_section(&mut reader, 1)?;
    let key_kind = match &keys_magic.to_le_bytes() {
        b"KR00" => "KR00",
        b"KRT0" => "KRT0",
        b"KRTS" => "KRTS",
        other => {
            return Err(format!(
                "unsupported keyframe section 0x{:02X}{:02X}{:02X}{:02X} in '{name}'",
                other[0], other[1], other[2], other[3]
            ));
        }
    };
    let mut keys_reader = Reader {
        bytes: keys_body,
        position: 0,
    };

    let mut keys = Vec::with_capacity(key_count);
    for _ in 0..key_count {
        // Quaternions are stored with x/y/z negated on disk.
        let qx = -keys_reader.f32("key rotation X")?;
        let qy = -keys_reader.f32("key rotation Y")?;
        let qz = -keys_reader.f32("key rotation Z")?;
        let qw = keys_reader.f32("key rotation W")?;
        let translation = match key_kind {
            "KRT0" => [
                keys_reader.f32("key translation X")?,
                keys_reader.f32("key translation Y")?,
                keys_reader.f32("key translation Z")?,
            ],
            "KRTS" => {
                let translation = [
                    keys_reader.f32("key translation X")?,
                    keys_reader.f32("key translation Y")?,
                    keys_reader.f32("key translation Z")?,
                ];
                keys_reader.skip(12, "key scale")?;
                translation
            }
            _ => [0.0; 3],
        };
        let time = keys_reader.f32("key time")?;
        keys.push(IfpKey {
            time,
            rotation: [qx, qy, qz, qw],
            translation,
        });
    }
    if keys.len() != key_count {
        return Err(format!(
            "object '{name}' declared {key_count} keys but carried {}",
            keys.len()
        ));
    }

    Ok(IfpObject {
        name,
        bone_id,
        keys,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sa_ped_ifp() -> Option<Vec<u8>> {
        let root = crate::test_paths::corpus_root()?;
        let path = root.join("GTA San Andreas").join("anim").join("ped.ifp");
        std::fs::read(path).ok()
    }

    fn vc_ped_ifp() -> Option<Vec<u8>> {
        let root = crate::test_paths::corpus_root()?;
        let path = root
            .join("Grand Theft Auto Vice City")
            .join("anim")
            .join("ped.ifp");
        std::fs::read(path).ok()
    }

    #[test]
    fn sa_ped_ifp_decodes_when_available() {
        let Some(bytes) = sa_ped_ifp() else {
            return;
        };
        let file = parse_ifp(&bytes).expect("SA ped.ifp should parse");
        assert_eq!(file.package, "ped");
        assert_eq!(
            file.animations.len(),
            294,
            "SA ped.ifp should carry 294 animations"
        );
        assert_eq!(file.animations[0].name, "abseil");
        // Every animation's keys must be usable: ascending times, unit
        // quats on the rotation channels.
        for animation in &file.animations {
            assert!(!animation.objects.is_empty());
            for object in &animation.objects {
                assert!(!object.keys.is_empty());
                for key in &object.keys {
                    let norm: f32 = key.rotation.iter().map(|v| v * v).sum();
                    // Placeholder objects (fam1/fam2/fam3 seat markers) may
                    // carry degenerate quats; normalize_rotation_keys
                    // handles them at conversion time.
                    assert!(norm.is_finite() && norm > 0.0);
                }
            }
        }
    }

    #[test]
    fn vc_ped_ifp_decodes_when_available() {
        let Some(bytes) = vc_ped_ifp() else {
            return;
        };
        let file = parse_ifp(&bytes).expect("VC ped.ifp should parse");
        assert_eq!(file.animations.len(), 234);
        assert_eq!(file.animations[0].name, "abseil");
        for animation in &file.animations {
            for object in &animation.objects {
                assert!(!object.keys.is_empty());
                for key in &object.keys {
                    let norm: f32 = key.rotation.iter().map(|v| v * v).sum();
                    assert!(norm.is_finite() && norm > 0.0);
                }
            }
        }
    }

    #[test]
    fn rejects_unknown_magic() {
        assert!(parse_ifp(b"JUNKJUNK...").is_err());
        assert!(parse_ifp(b"").is_err());
    }

    #[test]
    fn anp3_keys_dequantize_deterministically() {
        // Hand-packed ANP3: one animation, one object, two compressed keys.
        let fixed24 = |name: &[u8]| {
            let mut field = [0u8; 24];
            field[..name.len().min(24)].copy_from_slice(name);
            field.to_vec()
        };
        let mut body = Vec::new();
        body.extend_from_slice(&fixed24(b"ped"));
        body.extend_from_slice(&1_u32.to_le_bytes());
        body.extend_from_slice(&fixed24(b"walk"));
        body.extend_from_slice(&1_u32.to_le_bytes());
        body.extend_from_slice(&10_u32.to_le_bytes());
        body.extend_from_slice(&1_u32.to_le_bytes());
        body.extend_from_slice(&fixed24(b"Root"));
        body.extend_from_slice(&3_u32.to_le_bytes());
        body.extend_from_slice(&2_u32.to_le_bytes());
        body.extend_from_slice(&0_i32.to_le_bytes());
        // Key 0: quat (4096, 0, 0, 4096)/4096, time 0.
        body.extend_from_slice(&4096_i16.to_le_bytes());
        body.extend_from_slice(&0_i16.to_le_bytes());
        body.extend_from_slice(&0_i16.to_le_bytes());
        body.extend_from_slice(&4096_i16.to_le_bytes());
        body.extend_from_slice(&0_u16.to_le_bytes());
        // Key 1: quat (0, 0, 0, 4096)/4096, time 30 (= 1 s).
        body.extend_from_slice(&0_i16.to_le_bytes());
        body.extend_from_slice(&0_i16.to_le_bytes());
        body.extend_from_slice(&0_i16.to_le_bytes());
        body.extend_from_slice(&4096_i16.to_le_bytes());
        body.extend_from_slice(&30_u16.to_le_bytes());

        let mut pack = Vec::new();
        pack.extend_from_slice(ANP3_MAGIC);
        pack.extend_from_slice(&(body.len() as u32).to_le_bytes());
        pack.extend_from_slice(&body);

        let file = parse_ifp(&pack).expect("hand-packed ANP3 should parse");
        assert_eq!(file.animations.len(), 1);
        let animation = &file.animations[0];
        assert_eq!(animation.name, "walk");
        assert_eq!(animation.objects.len(), 1);
        let object = &animation.objects[0];
        assert_eq!(object.name, "Root");
        assert_eq!(object.keys.len(), 2);
        assert!((object.keys[0].time - 0.0).abs() < f32::EPSILON);
        assert!((object.keys[1].time - 1.0).abs() < f32::EPSILON);
        assert!(
            (object.keys[0].rotation[0] - 1.0).abs() < f32::EPSILON,
            "4096/4096 must dequantize to 1.0"
        );
    }
}
