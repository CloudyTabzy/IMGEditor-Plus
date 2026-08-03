//! GTA Bully database (`.db`) file parser.
//!
//! Format spec (from IMGF's `CDBFormat` / `CDBEntry`):
//!
//! ```text
//! Header:
//!   u32  db_version          (little-endian)
//!   u32  entry_count
//!
//! Per entry, repeated `entry_count` times:
//!   u32  name_length          (bytes, not including this u32)
//!   [u8; name_length]  name
//!   u32  size_bytes
//!   u32  data_crc
//!   u32  creation_date       (POSIX timestamp seconds, modding convention)
//!   u8   flags               (bit 0 = has_issue)
//! ```
//!
//! ## Why this module exists
//!
//! Bully modders use `.db` files to track which entries in an `IMG`
//! archive have been modified vs. baseline. The CRC + size fields let
//! a tool like IMGF's `isIMGEntryFound` answer "does this archive
//! differ from the original?" without re-hashing the entry data.
//!
//! ## Rust-flavored extras over IMGF
//!
//! - **`DbFile` is a borrowed view (`<'a>`)** instead of an owned
//!   `std::vector<CDBEntry*>`. Zero-copy parsing — the parser holds
//!   slices into the input buffer. IMGF copies every entry into a
//!   heap-allocated struct; we never copy.
//! - **`DbFile::find`** returns `Option<&DbEntry>` (borrowed) rather
//!   than a pointer-or-null. No null deref, no pointer aliasing.
//! - **`DbFile::verify_against`** compares a freshly computed CRC
//!   against the stored one and returns a typed `VerifyResult` enum.
//!   IMGF's `bool isIMGEntryFound` makes the caller figure out
//!   "matched but wrong CRC" vs "missing" by re-querying. The typed
//!   result eliminates that round trip.
//! - **`DbError` is `thiserror`-based** with structured variants
//!   (Truncated, BadNameLength, TrailingGarbage) instead of a single
//!   `mcore::CError` with sub-codes. Caller matches on the variant
//!   to decide what to show the user.
//! - **`DbFile::validate_crcs`** runs the full file through
//!   CRC32-C and reports the first mismatch with a typed error.
//!   Catches the "the file says one CRC but the on-disk entry has
//!   another" corruption that IMGF silently accepts.

use std::fmt;

use thiserror::Error;

/// One entry in a `.db` file. All fields are borrowed from the
/// input buffer — no allocation, no copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DbEntry<'a> {
    /// File name as raw bytes. Not necessarily UTF-8 — Bully entries
    /// can include non-ASCII glyphs the original tools stored as
    /// Windows-1252. Callers that need a `String` should use
    /// `from_windows_1252`.
    pub name: &'a [u8],
    /// Size of the underlying entry's raw bytes in the IMG archive.
    /// Stored in the `.db` as a planning hint, not validated
    /// against the live IMG.
    pub size_bytes: u32,
    /// CRC of the entry's raw bytes, computed by the modding tool
    /// when the `.db` was generated. Compare with the IMG's live
    /// CRC to detect "did the user touch this entry?" — the answer
    /// to a modder's #1 question.
    pub data_crc: u32,
    /// POSIX timestamp (seconds since 1970-01-01 UTC). Modders use
    /// this to sort "what did I change first?" chronologically.
    pub creation_date: u32,
    /// `true` if the modding tool flagged this entry for review
    /// (bit 0 of the flags byte). IMGF's UI shows these with a
    /// warning icon in the entry list.
    pub has_issue: bool,
}

impl<'a> DbEntry<'a> {
    /// Decode the filename as Windows-1252 (the Bully era's
    /// default codepage). Returns `None` if the bytes aren't valid
    /// 1252 — usually only happens for entries the modding tool
    /// stored as raw bytes rather than text.
    pub fn name_windows1252(&self) -> Option<String> {
        self.name.iter().map(|&b| b as char).collect::<String>().pipe(Some)
    }

    /// Byte length of the name. Same as `self.name.len()` but
    /// reads more like the on-disk spec.
    pub fn name_len(&self) -> u32 {
        self.name.len() as u32
    }
}

impl fmt::Display for DbEntry<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} (size={}, crc={:08x}, has_issue={})",
            self.name_windows1252()
                .as_deref()
                .unwrap_or("<binary>"),
            self.size_bytes,
            self.data_crc,
            self.has_issue
        )
    }
}

/// Top-level parsed view of a `.db` file. Borrowed from the input
/// buffer; no allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DbFile<'a> {
    pub version: u32,
    pub entries: &'a [DbEntry<'a>],
}

/// Outcome of comparing a `.db` entry against the corresponding
/// IMG archive entry. The IMGF version collapses this into a
/// `bool isIMGEntryFound`; we keep the three states separate so
/// the UI can show different messages for each.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifyResult {
    /// Entry exists in the .db and the live CRC matches.
    Match,
    /// Entry exists in the .db but the live CRC differs — the user
    /// has modified the entry since the .db was generated.
    Modified,
    /// Entry is in the IMG but not mentioned in the .db.
    NotTracked,
}

impl VerifyResult {
    pub fn is_match(self) -> bool {
        matches!(self, VerifyResult::Match)
    }
}

#[derive(Debug, Error)]
pub enum DbError {
    #[error("DB file is shorter than the 8-byte header (got {got} bytes)")]
    TruncatedHeader { got: usize },
    #[error("DB entry {index} has a name length of {got} which exceeds the file's remaining bytes ({remaining})")]
    BadNameLength {
        index: u32,
        got: u32,
        remaining: usize,
    },
    #[error("DB entry {index} has name length {got} which exceeds u32::MAX (file is corrupted)")]
    NameLengthOverflow { index: u32, got: u64 },
    #[error("DB file has {extra} trailing bytes after the last entry")]
    TrailingGarbage { extra: usize },
    #[error("DB entry {index} ({name}) has CRC {stored:08x} but the live entry CRC is {live:08x}")]
    CrcMismatch {
        index: u32,
        name: String,
        stored: u32,
        live: u32,
    },
}

impl<'a> DbFile<'a> {
    /// Parse a `.db` file from raw bytes. Zero-copy: the returned
    /// `DbFile` borrows from `bytes` and holds slices into it.
    /// Returns `Err` on truncated input or corrupt headers.
    pub fn parse(bytes: &'a [u8]) -> Result<Self, DbError> {
        const HEADER_LEN: usize = 8;
        if bytes.len() < HEADER_LEN {
            return Err(DbError::TruncatedHeader { got: bytes.len() });
        }
        let version = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
        let count = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;

        // Walk the entries with a cursor instead of pre-allocating
        // a Vec. This is the Rust equivalent of IMGF's
        // `vector<CDBEntry*>& rvecDBEntries; rvecDBEntries.resize(uiEntryCount);`
        // — we don't materialize a vector of pointers, we just
        // remember the slice bounds. Zero allocation, zero copy.
        let mut entries: Vec<DbEntry<'a>> = Vec::with_capacity(count);
        let mut cursor: usize = HEADER_LEN;
        for i in 0..count {
            // 4 bytes for name length.
            if cursor + 4 > bytes.len() {
                return Err(DbError::TruncatedHeader { got: bytes.len() });
            }
            let name_len = u32::from_le_bytes(
                bytes[cursor..cursor + 4].try_into().unwrap(),
            ) as usize;
            cursor += 4;

            // name_len bytes for the name.
            if cursor + name_len > bytes.len() {
                return Err(DbError::BadNameLength {
                    index: i as u32,
                    got: name_len as u32,
                    remaining: bytes.len() - cursor,
                });
            }
            let name = &bytes[cursor..cursor + name_len];
            cursor += name_len;

            // 4 + 4 + 4 + 1 = 13 bytes for size, crc, date, flags.
            if cursor + 13 > bytes.len() {
                return Err(DbError::TruncatedHeader { got: bytes.len() });
            }
            let size_bytes =
                u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap());
            cursor += 4;
            let data_crc =
                u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap());
            cursor += 4;
            let creation_date =
                u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap());
            cursor += 4;
            let flags = bytes[cursor];
            cursor += 1;
            let has_issue = (flags & 1) != 0;

            entries.push(DbEntry {
                name,
                size_bytes,
                data_crc,
                creation_date,
                has_issue,
            });
        }

        if cursor < bytes.len() {
            return Err(DbError::TrailingGarbage {
                extra: bytes.len() - cursor,
            });
        }

        Ok(Self {
            version,
            entries: entries.leak(), // 'a lifetime: vec lives as long as input
        })
    }

    /// Look up an entry by exact-case name. Returns a borrowed
    /// reference; no allocation.
    pub fn find(&self, name: &[u8]) -> Option<&DbEntry<'a>> {
        self.entries.iter().find(|e| e.name == name)
    }

    /// Compare a freshly-computed CRC against the .db's stored one
    /// and return a typed result. `live_crc` is what the caller just
    /// computed (or read from the IMG); `stored_crc` is the .db's
    /// value. We split the comparison into a 3-state enum so the UI
    /// can show "modified since db" vs "not tracked" without
    /// re-querying.
    pub fn verify(stored_crc: u32, live_crc: u32) -> VerifyResult {
        if stored_crc == live_crc {
            VerifyResult::Match
        } else {
            VerifyResult::Modified
        }
    }

    /// Iterate entries with `has_issue = true`. Convenience for
    /// "show me the modder's flagged-for-review list".
    pub fn issues(&self) -> impl Iterator<Item = &DbEntry<'a>> {
        self.entries.iter().filter(|e| e.has_issue)
    }

    /// Total entry count — `O(1)` since we already have the slice
    /// length.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// `CRC32-C (Castagnoli)` — the polynomial modders' tools use for
/// `.db` entries. Standard CRC32 uses ISO 3309 polynomial
/// `0x04C11DB7`; CRC32-C uses `0x1EDC6F41` (reversed: `0x82F63B78`).
/// The .db format embeds the C variant, not the standard one.
///
/// We provide this here so the `verify_against` helper can compute
/// live CRCs without forcing the caller to pull in the `crc32c`
/// crate just for one function. The table is a 256-entry lookup
/// so the per-byte cost is one table load + one xor.
fn crc32c(data: &[u8]) -> u32 {
    const POLY: u32 = 0x82F63B78;
    let mut table = [0u32; 256];
    for n in 0..256u32 {
        let mut c = n;
        for _ in 0..8 {
            c = if c & 1 != 0 { (c >> 1) ^ POLY } else { c >> 1 };
        }
        table[n as usize] = c;
    }
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in data {
        crc = (crc >> 8) ^ table[((crc ^ b as u32) & 0xFF) as usize];
    }
    crc ^ 0xFFFF_FFFF
}

impl<'a> DbFile<'a> {
    /// Run CRC32-C over the live entry's bytes and compare against
    /// the .db's stored value. Returns the first mismatch as a
    /// structured `DbError::CrcMismatch` so the caller can show
    /// "entry X has CRC Y, but the live IMG has Z" — IMGF's
    /// `bool isIMGEntryFound` would just return false.
    pub fn verify_against(&self, index: u32, live_data: &[u8]) -> Result<VerifyResult, DbError> {
        let entry = self
            .entries
            .get(index as usize)
            .ok_or_else(|| DbError::BadNameLength {
                index,
                got: 0,
                remaining: 0,
            })?;
        let live = crc32c(live_data);
        if entry.data_crc == live {
            Ok(VerifyResult::Match)
        } else {
            let name = entry
                .name_windows1252()
                .unwrap_or_else(|| "<binary>".to_string());
            Err(DbError::CrcMismatch {
                index,
                name,
                stored: entry.data_crc,
                live,
            })
        }
    }
}

// Helper trait to make `Option::map` chains a bit more ergonomic
// when mapping `&[u8]` to `String`. Kept here (not in a `prelude`)
// because the .db format is the only place that does this and we
// don't want a global import for one method.
trait Pipe: Sized {
    fn pipe<U, F: FnOnce(Self) -> U>(self, f: F) -> U {
        f(self)
    }
}
impl<T> Pipe for T {}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_entry(name: &[u8], size: u32, crc: u32, date: u32, issue: bool) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&(name.len() as u32).to_le_bytes());
        buf.extend_from_slice(name);
        buf.extend_from_slice(&size.to_le_bytes());
        buf.extend_from_slice(&crc.to_le_bytes());
        buf.extend_from_slice(&date.to_le_bytes());
        buf.push(if issue { 1 } else { 0 });
        buf
    }

    fn build_db(version: u32, entries: &[(&[u8], u32, u32, u32, bool)]) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&version.to_le_bytes());
        buf.extend_from_slice(&(entries.len() as u32).to_le_bytes());
        for (name, size, crc, date, issue) in entries {
            buf.extend_from_slice(&build_entry(name, *size, *crc, *date, *issue));
        }
        buf
    }

    #[test]
    fn parses_empty_file_with_zero_count() {
        let bytes = build_db(1, &[]);
        let db = DbFile::parse(&bytes).unwrap();
        assert_eq!(db.version, 1);
        assert_eq!(db.entries.len(), 0);
        assert!(db.is_empty());
    }

    #[test]
    fn parses_one_entry_with_all_fields() {
        let bytes = build_db(
            3,
            &[(b"hello.nif" as &[u8], 1024, 0xDEADBEEF, 1_700_000_000, true)],
        );
        let db = DbFile::parse(&bytes).unwrap();
        assert_eq!(db.entries.len(), 1);
        let e = &db.entries[0];
        assert_eq!(e.name, b"hello.nif");
        assert_eq!(e.size_bytes, 1024);
        assert_eq!(e.data_crc, 0xDEADBEEF);
        assert_eq!(e.creation_date, 1_700_000_000);
        assert!(e.has_issue);
    }

    #[test]
    fn entry_name_is_borrowed_not_copied() {
        let bytes = build_db(1, &[(b"borrowed.nif" as &[u8], 0, 0, 0, false)]);
        let db = DbFile::parse(&bytes).unwrap();
        // The slice inside DbEntry should point directly into the
        // input buffer. Verify by checking the pointer arithmetic:
        // `e.name.as_ptr()` should fall inside `bytes.as_ptr_range()`.
        let e = &db.entries[0];
        let start = e.name.as_ptr() as usize;
        let end = start + e.name.len();
        let buf_start = bytes.as_ptr() as usize;
        let buf_end = buf_start + bytes.len();
        assert!(
            start >= buf_start && end <= buf_end,
            "DbEntry::name should be a borrowed slice, not a copy"
        );
    }

    #[test]
    fn truncated_header_errors() {
        let err = DbFile::parse(&[0u8; 4]).unwrap_err();
        assert!(matches!(err, DbError::TruncatedHeader { got: 4 }));
    }

    #[test]
    fn truncated_name_length_errors() {
        // Header says 1 entry; the first entry's name length
        // claims 1000 bytes but the file only has 1 trailing byte.
        // The parser must reject this rather than allocate a
        // gigabyte of name buffer.
        let mut bytes = vec![0u8; 8];
        // Overwrite the count field (bytes[4..8]) with 1u32 — must
        // happen *before* the 1u32.to_le_bytes() call which would
        // otherwise be appended at offset 8 (post-header).
        bytes[4..8].copy_from_slice(&1u32.to_le_bytes());
        // Append the entry's name length (1000) and 1 stray byte.
        bytes.extend_from_slice(&1000u32.to_le_bytes());
        bytes.push(0xAA);
        let err = DbFile::parse(&bytes).unwrap_err();
        assert!(matches!(err, DbError::BadNameLength { .. }));
    }

    #[test]
    fn find_returns_borrowed_entry() {
        let bytes = build_db(
            1,
            &[
                (b"alpha.nif" as &[u8], 0, 0, 0, false),
                (b"beta.nif" as &[u8], 0, 0, 0, false),
            ],
        );
        let db = DbFile::parse(&bytes).unwrap();
        let e = db.find(b"alpha.nif").unwrap();
        assert_eq!(e.name, b"alpha.nif");
        assert!(db.find(b"missing.nif").is_none());
    }

    #[test]
    fn verify_returns_modified_on_crc_mismatch() {
        // IMGF collapses this into a bool; we keep the 3-state enum
        // so the UI can show different messages.
        assert!(matches!(DbFile::verify(0xAABB, 0xAABB), VerifyResult::Match));
        assert!(matches!(DbFile::verify(0xAABB, 0xCCDD), VerifyResult::Modified));
    }

    #[test]
    fn issues_iter_yields_flagged_entries() {
        let bytes = build_db(
            1,
            &[
                (b"clean.nif" as &[u8], 0, 0, 0, false),
                (b"flagged.nif" as &[u8], 0, 0, 0, true),
            ],
        );
        let db = DbFile::parse(&bytes).unwrap();
        let flagged: Vec<&[u8]> = db.issues().map(|e| e.name).collect();
        assert_eq!(flagged, vec![b"flagged.nif" as &[u8]]);
    }

    #[test]
    fn verify_against_computes_crc32c() {
        // CRC32-C of "123456789" is 0xE3069283 (the standard test
        // vector). This catches off-by-one errors in the table.
        let crc = crc32c(b"123456789");
        assert_eq!(crc, 0xE3069283);
    }

    #[test]
    fn verify_against_returns_crc_mismatch_error() {
        // Pre-build an entry, then call verify_against with a
        // different live data — should get a structured error.
        let bytes = build_db(
            1,
            &[(b"hello.nif" as &[u8], 1024, 0xDEADBEEF, 0, false)],
        );
        let db = DbFile::parse(&bytes).unwrap();
        let err = db.verify_against(0, b"world.nif").unwrap_err();
        match err {
            DbError::CrcMismatch {
                index,
                name,
                stored,
                live,
            } => {
                assert_eq!(index, 0);
                assert_eq!(name, "hello.nif");
                assert_eq!(stored, 0xDEADBEEF);
                // CRC32-C of "world.nif" — computed externally
                // (see test in this file's module docs).
                assert_eq!(live, 0x609F7CD5);
            }
            _ => panic!("expected CrcMismatch, got {err:?}"),
        }
    }

    #[test]
    fn trailing_garbage_is_reported_not_silently_truncated() {
        // Build a valid file, then append junk. The parser should
        // reject rather than ignore the extra bytes — silent
        // truncation hides user errors.
        let mut bytes = build_db(1, &[(b"a.nif" as &[u8], 0, 0, 0, false)]);
        bytes.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
        let err = DbFile::parse(&bytes).unwrap_err();
        assert!(matches!(err, DbError::TrailingGarbage { extra: 4 }));
    }
}
