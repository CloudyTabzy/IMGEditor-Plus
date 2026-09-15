//! Entry-list export and comparison.
//!
//! The original IMG Editor's list format is deliberately small: one entry
//! name per line. Keeping parsing and comparison independent from the UI
//! makes the compatibility behavior easy to test and lets large manifests be
//! processed away from the event loop.

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::Path;

/// Refuse unbounded input from a user-selected manifest. Real archive lists
/// are normally a few megabytes at most; this still leaves room for very
/// large archives without allowing an accidental multi-gigabyte read.
pub const MAX_MANIFEST_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedManifest {
    /// Non-empty, trimmed entry names in their original order. A # is an
    /// ordinary entry name prefix, not a comment marker.
    pub entries: Vec<String>,
    pub ignored_blank_lines: usize,
}

impl ParsedManifest {
    pub fn new(entries: Vec<String>, ignored_blank_lines: usize) -> Self {
        Self {
            entries,
            ignored_blank_lines,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompareOptions {
    /// Preserve the reference editor's exact, case-sensitive matching by
    /// default. The UI may opt into case-insensitive matching for lists
    /// produced by tools that normalize filename case.
    pub case_sensitive: bool,
}

impl Default for CompareOptions {
    fn default() -> Self {
        Self {
            case_sensitive: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompareReport {
    /// Number of archive records, including any duplicate names.
    pub archive_entry_count: usize,
    /// Number of non-empty manifest lines, including duplicate names.
    pub manifest_entry_count: usize,
    /// Number of distinct manifest names under the selected case policy.
    pub unique_manifest_count: usize,
    /// Number of manifest lines represented by the archive, including
    /// duplicate manifest lines. This mirrors the reference behavior.
    pub matched_count: usize,
    /// Missing lines in manifest order. Duplicate missing lines are kept so
    /// the default report remains compatible with the original tool.
    pub missing: Vec<String>,
    /// Number of distinct missing names under the selected case policy.
    pub unique_missing_count: usize,
    /// Distinct archive names that do not occur in the manifest, in archive
    /// storage order. This is an opt-in QoL view; it does not affect missing.
    pub archive_only: Vec<String>,
    /// Additional manifest lines whose matching key was already seen.
    pub duplicate_manifest_count: usize,
}

/// Parse a manifest from UTF-8 text. LF, CRLF, and legacy lone-CR line
/// endings are accepted because lists may have been produced by older tools.
pub fn parse_manifest(text: &str) -> ParsedManifest {
    let mut entries = Vec::new();
    let mut ignored_blank_lines = 0;
    let mut line_start = 0;
    let mut chars = text.char_indices().peekable();

    while let Some((index, character)) = chars.next() {
        if character != '\r' && character != '\n' {
            continue;
        }

        let line = &text[line_start..index];
        record_line(
            line,
            &mut entries,
            &mut ignored_blank_lines,
            line_start == 0,
        );

        if character == '\r' && chars.peek().is_some_and(|(_, next)| *next == '\n') {
            chars.next();
        }
        line_start = chars
            .peek()
            .map(|(next_index, _)| *next_index)
            .unwrap_or(text.len());
    }

    if line_start < text.len() {
        record_line(
            &text[line_start..],
            &mut entries,
            &mut ignored_blank_lines,
            line_start == 0,
        );
    }

    ParsedManifest::new(entries, ignored_blank_lines)
}

fn record_line(
    line: &str,
    entries: &mut Vec<String>,
    ignored_blank_lines: &mut usize,
    first_line: bool,
) {
    // UTF-8 BOMs are common when a list was saved from a Windows editor. It
    // is metadata, not part of the first archive name.
    let line = if first_line {
        line.strip_prefix('\u{feff}').unwrap_or(line)
    } else {
        line
    };
    let line = line.trim();
    if line.is_empty() {
        *ignored_blank_lines += 1;
    } else {
        entries.push(line.to_owned());
    }
}

/// Read and parse a manifest selected by the user.
pub fn read_manifest_file(path: &Path) -> Result<ParsedManifest, String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("Could not inspect manifest '{}': {error}", path.display()))?;
    if metadata.len() > MAX_MANIFEST_BYTES {
        return Err(format!(
            "Manifest '{}' is too large ({}; limit is {}).",
            path.display(),
            format_bytes(metadata.len()),
            format_bytes(MAX_MANIFEST_BYTES)
        ));
    }
    let bytes = fs::read(path)
        .map_err(|error| format!("Could not read manifest '{}': {error}", path.display()))?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(format!(
            "Manifest '{}' grew beyond the {} limit while it was being read.",
            path.display(),
            format_bytes(MAX_MANIFEST_BYTES)
        ));
    }
    let text = std::str::from_utf8(&bytes)
        .map_err(|error| format!("Manifest '{}' is not valid UTF-8: {error}", path.display()))?;
    Ok(parse_manifest(text))
}

/// Encode names in the compatible export format: archive order, CRLF between
/// names, and no final newline.
pub fn encode_manifest(names: &[String]) -> String {
    names.join("\r\n")
}

pub fn write_manifest_file(path: &Path, names: &[String]) -> io::Result<()> {
    fs::write(path, encode_manifest(names))
}

pub fn compare_names(
    archive_entries: &[String],
    manifest: &ParsedManifest,
    options: CompareOptions,
) -> CompareReport {
    let archive_keys: HashSet<String> = archive_entries
        .iter()
        .map(|name| comparison_key(name, options))
        .collect();
    let manifest_keys: HashSet<String> = manifest
        .entries
        .iter()
        .map(|name| comparison_key(name, options))
        .collect();

    let mut seen_manifest = HashSet::with_capacity(manifest.entries.len());
    let mut missing = Vec::new();
    let mut seen_missing = HashSet::with_capacity(manifest.entries.len());
    let mut matched_count = 0;
    let mut duplicate_manifest_count = 0;
    for name in &manifest.entries {
        let key = comparison_key(name, options);
        if !seen_manifest.insert(key.clone()) {
            duplicate_manifest_count += 1;
        }
        if archive_keys.contains(&key) {
            matched_count += 1;
        } else {
            seen_missing.insert(key);
            missing.push(name.clone());
        }
    }

    let mut seen_archive = HashSet::with_capacity(archive_entries.len());
    let archive_only = archive_entries
        .iter()
        .filter_map(|name| {
            let key = comparison_key(name, options);
            (seen_archive.insert(key.clone()) && !manifest_keys.contains(&key))
                .then_some(name.clone())
        })
        .collect();

    CompareReport {
        archive_entry_count: archive_entries.len(),
        manifest_entry_count: manifest.entries.len(),
        unique_manifest_count: manifest_keys.len(),
        matched_count,
        missing,
        unique_missing_count: seen_missing.len(),
        archive_only,
        duplicate_manifest_count,
    }
}

fn comparison_key(name: &str, options: CompareOptions) -> String {
    if options.case_sensitive {
        name.to_owned()
    } else {
        name.to_lowercase()
    }
}

fn format_bytes(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = KIB * 1024;
    if bytes >= MIB {
        format!("{:.1} MiB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.1} KiB", bytes as f64 / KIB as f64)
    } else {
        format!("{bytes} bytes")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn manifest(lines: &[&str]) -> ParsedManifest {
        parse_manifest(&lines.join("\n"))
    }

    #[test]
    fn accepts_all_supported_line_endings_and_unterminated_last_line() {
        let parsed = parse_manifest(" a.dff\r\nb.txd\nc.col\rd.ifp");
        assert_eq!(parsed.entries, ["a.dff", "b.txd", "c.col", "d.ifp"]);
    }

    #[test]
    fn trims_whitespace_skips_blanks_and_preserves_hash_names() {
        let parsed = parse_manifest("\u{feff}  A.DFF  \n\n  #not-a-comment\r\n\tB.TXD\t");
        assert_eq!(parsed.entries, ["A.DFF", "#not-a-comment", "B.TXD"]);
        assert_eq!(parsed.ignored_blank_lines, 1);
    }

    #[test]
    fn empty_input_has_no_entries() {
        let parsed = parse_manifest("");
        assert!(parsed.entries.is_empty());
        assert_eq!(parsed.ignored_blank_lines, 0);
    }

    #[test]
    fn export_uses_crlf_without_trailing_newline() {
        let names = vec!["one.dff".to_owned(), "two.txd".to_owned()];
        assert_eq!(encode_manifest(&names), "one.dff\r\ntwo.txd");
        assert!(!encode_manifest(&names).ends_with(['\r', '\n']));
        assert_eq!(encode_manifest(&[]), "");
    }

    #[test]
    fn export_round_trips_through_parser() {
        let names = vec!["one.dff".to_owned(), "two.txd".to_owned()];
        assert_eq!(parse_manifest(&encode_manifest(&names)).entries, names);
    }

    #[test]
    fn exact_matching_is_case_sensitive_and_keeps_duplicate_missing_lines() {
        let report = compare_names(
            &["A.DFF".to_owned(), "present.txd".to_owned()],
            &manifest(&[
                "A.DFF",
                "a.dff",
                "missing.col",
                "missing.col",
                "present.txd",
            ]),
            CompareOptions::default(),
        );
        assert_eq!(report.matched_count, 2);
        assert_eq!(report.missing, ["a.dff", "missing.col", "missing.col"]);
        assert_eq!(report.unique_missing_count, 2);
        assert_eq!(report.duplicate_manifest_count, 1);
        assert!(report.archive_only.is_empty());
    }

    #[test]
    fn case_insensitive_matching_is_opt_in() {
        let report = compare_names(
            &["A.DFF".to_owned(), "extra.txd".to_owned()],
            &manifest(&["a.dff"]),
            CompareOptions {
                case_sensitive: false,
            },
        );
        assert_eq!(report.matched_count, 1);
        assert!(report.missing.is_empty());
        assert_eq!(report.archive_only, ["extra.txd"]);
    }

    #[test]
    fn archive_only_names_are_unique_and_keep_archive_order() {
        let report = compare_names(
            &[
                "present.dff".to_owned(),
                "extra.txd".to_owned(),
                "extra.txd".to_owned(),
                "later.col".to_owned(),
            ],
            &manifest(&["present.dff"]),
            CompareOptions::default(),
        );
        assert_eq!(report.archive_only, ["extra.txd", "later.col"]);
    }

    #[test]
    fn read_manifest_file_rejects_invalid_utf8() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("bad.compare");
        fs::write(&path, [0xff, 0xfe]).unwrap();
        let error = read_manifest_file(&path).unwrap_err();
        assert!(error.contains("not valid UTF-8"));
    }

    #[test]
    fn read_manifest_file_rejects_oversized_input_before_reading() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("oversized.compare");
        let file = fs::File::create(&path).unwrap();
        file.set_len(MAX_MANIFEST_BYTES + 1).unwrap();
        let error = read_manifest_file(&path).unwrap_err();
        assert!(error.contains("too large"));
    }

    #[test]
    fn read_manifest_file_reports_valid_content() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("entries.compare");
        fs::write(&path, "one.dff\r\ntwo.txd").unwrap();
        let parsed = read_manifest_file(&path).unwrap();
        assert_eq!(parsed.entries, ["one.dff", "two.txd"]);
    }
}
