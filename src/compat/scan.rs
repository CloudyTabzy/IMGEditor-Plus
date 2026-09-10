//! Archive corpus scanner: the Phase 0 diagnostic that profiles every
//! texture in an IMG archive and reports class distribution, header
//! anomalies, and per-game verdicts. This is the verification tool for
//! the profile tables and the seed of the user-facing auditor.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Context;

use crate::archive::ArchiveInfo;
use crate::parser::txd::parse_txd;
use crate::parser::{detect_version, read_entry_data_from_source, ImgVersion};
use crate::parser::pc_v1::PcV1Parser;
use crate::parser::pc_v2::PcV2Parser;
use crate::parser::xbox360::Xbox360Parser;
use crate::parser::ImgParser;

use super::games::{classify, ALL_GAMES};
use super::raster::{LogicalFormat, RasterProfile, Severity};

#[derive(Debug, Default)]
pub struct ScanReport {
    pub archive_path: String,
    pub archive_kind: String,
    pub entry_count: usize,
    pub txd_entries: usize,
    pub parse_failures: usize,
    pub textures: usize,
    /// Logical-format histogram ("DXT1", "888 (32bpp)"…).
    pub class_counts: BTreeMap<String, usize>,
    /// Anomaly code → occurrences.
    pub anomaly_counts: BTreeMap<&'static str, usize>,
    /// First few offending texture names per anomaly code.
    pub anomaly_examples: BTreeMap<&'static str, Vec<String>>,
    /// Game id → verdict label → texture count.
    pub verdicts: BTreeMap<&'static str, BTreeMap<&'static str, usize>>,
    /// Palette-reconstructible texture count (only with pixel decoding).
    pub palette_reconstructible: Option<usize>,
    /// Worst severity per anomaly code for report ordering.
    pub anomaly_severity: BTreeMap<&'static str, Severity>,
}
#[derive(Debug, Default)]
pub struct ScanOptions {
    /// Decode pixels to test palette-reconstructibility (≤ 256 unique
    /// colors). Slower: one RGBA decode per texture.
    pub decode_pixels: bool,
}

/// Open any supported archive headlessly and profile its textures.
pub fn scan_archive(path: &Path, options: &ScanOptions) -> anyhow::Result<ScanReport> {
    let version = detect_version(path);
    let mut archive = ArchiveInfo::new(
        path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default(),
        false,
        version,
    );
    archive.path = Some(path.to_path_buf());
    match version {
        ImgVersion::One => PcV1Parser.open(&mut archive)?,
        ImgVersion::Two => PcV2Parser.open(&mut archive)?,
        ImgVersion::Xbox360 => Xbox360Parser.open(&mut archive)?,
        ImgVersion::Unknown => anyhow::bail!("unrecognized IMG archive format: {}", path.display()),
    }

    let mut report = ScanReport {
        archive_path: path.display().to_string(),
        archive_kind: version_text(version).to_string(),
        entry_count: archive.entries.len(),
        ..ScanReport::default()
    };

    for entry in &archive.entries {
        if !entry.file_name_lower.ends_with(".txd") {
            continue;
        }
        report.txd_entries += 1;
        let bytes = match read_entry_data_from_source(entry, Some(path)) {
            Ok(bytes) => bytes,
            Err(err) => {
                report.parse_failures += 1;
                record_anomaly(
                    &mut report,
                    "TXD_READ_FAIL",
                    Severity::Error,
                    &entry.file_name,
                    format!("{err}"),
                );
                continue;
            }
        };
        let parsed = match parse_txd(&bytes) {
            Ok(parsed) => parsed,
            Err(err) => {
                report.parse_failures += 1;
                record_anomaly(
                    &mut report,
                    "TXD_PARSE_FAIL",
                    Severity::Error,
                    &entry.file_name,
                    err,
                );
                continue;
            }
        };

        for texture in &parsed.textures {
            report.textures += 1;
            let profile = RasterProfile::from_native(texture);

            let mut class_label = profile.logical.label().to_string();
            if profile.logical == LogicalFormat::R888 && profile.storage_bpp == 4 {
                class_label.push_str(" (32bpp storage)");
            }
            *report.class_counts.entry(class_label).or_default() += 1;

            for anomaly in profile.anomalies() {
                record_anomaly(
                    &mut report,
                    anomaly.code,
                    anomaly.severity,
                    texture.diffuse_name.as_str(),
                    anomaly.detail,
                );
            }

            for game in ALL_GAMES {
                let verdict = classify(game, &profile);
                *report
                    .verdicts
                    .entry(game.id)
                    .or_default()
                    .entry(verdict.verdict.label())
                    .or_default() += 1;
            }

            if options.decode_pixels {
                match texture.decode_rgba() {
                    Ok(rgba) if unique_color_count(&rgba) <= 256 => {
                        *report.palette_reconstructible.get_or_insert(0) += 1;
                    }
                    Ok(_) => {}
                    Err(err) => {
                        record_anomaly(
                            &mut report,
                            "TXD_DECODE_FAIL",
                            Severity::Error,
                            texture.diffuse_name.as_str(),
                            format!("{err}"),
                        );
                    }
                }
            }
        }
    }

    Ok(report)
}

fn unique_color_count(rgba: &[u8]) -> usize {
    use std::collections::HashSet;
    let mut seen = HashSet::new();
    for px in rgba.chunks_exact(4) {
        seen.insert([px[0], px[1], px[2], px[3]]);
    }
    seen.len()
}

fn record_anomaly(
    report: &mut ScanReport,
    code: &'static str,
    severity: Severity,
    texture: &str,
    detail: String,
) {
    let severity = report
        .anomaly_severity
        .get(code)
        .copied()
        .map(|existing| existing.max_by_rank(severity))
        .unwrap_or(severity);
    report.anomaly_severity.insert(code, severity);
    *report.anomaly_counts.entry(code).or_default() += 1;
    let examples = report.anomaly_examples.entry(code).or_default();
    if examples.len() < 5 {
        examples.push(format!("{texture}: {detail}"));
    }
}

trait SeverityRank {
    fn max_by_rank(self, other: Self) -> Self;
}

impl SeverityRank for Severity {
    fn max_by_rank(self, other: Self) -> Self {
        if self >= other {
            self
        } else {
            other
        }
    }
}

fn version_text(version: ImgVersion) -> &'static str {
    match version {
        ImgVersion::One => "IMG v1",
        ImgVersion::Two => "IMG v2",
        ImgVersion::Xbox360 => "IMG v1 (Xbox 360)",
        ImgVersion::Unknown => "unknown",
    }
}

/// Parse the `--scan-corpus` CLI arguments and run the report.
/// `args` is the full process argument vector (`args[1] == "--scan-corpus"`).
pub fn run_cli_args(args: &[String]) -> anyhow::Result<()> {
    let mut archive: Option<std::path::PathBuf> = None;
    let mut target: Option<&str> = None;
    let mut decode_pixels = false;

    let mut iter = args.iter().skip(2);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--colors" => decode_pixels = true,
            "--target" => {
                target = Some(
                    iter.next()
                        .ok_or_else(|| anyhow::anyhow!("--target needs a game id (gta3|vc|sa|bully)"))?
                        .as_str(),
                );
            }
            other if archive.is_none() => archive = Some(std::path::PathBuf::from(other)),
            other => anyhow::bail!("unexpected argument: {other}"),
        }
    }

    if let Some(id) = target
        && super::games::profile_by_id(id).is_none()
    {
        anyhow::bail!("unknown target game '{id}' (gta3|vc|sa|bully)");
    }

    let path = archive.ok_or_else(|| anyhow::anyhow!("--scan-corpus needs an archive path"))?;
    run_cli(
        &path,
        &ScanOptions {
            decode_pixels,
        },
        target,
    )
}

/// CLI entry point: print a human-readable report.
pub fn run_cli(path: &Path, options: &ScanOptions, target: Option<&str>) -> anyhow::Result<()> {    let report = scan_archive(path, options).with_context(|| {
        format!("scanning {}", path.display())
    })?;

    println!("Archive: {} ({})", report.archive_path, report.archive_kind);
    println!(
        "Entries: {} | TXD entries: {} | textures: {} | parse failures: {}",
        report.entry_count, report.txd_entries, report.textures, report.parse_failures
    );

    println!("\nRaster class histogram:");
    for (class, count) in &report.class_counts {
        println!("  {class:24} {count}");
    }

    if !report.anomaly_counts.is_empty() {
        println!("\nHeader anomalies:");
        for (code, count) in &report.anomaly_counts {
            let severity = report
                .anomaly_severity
                .get(code)
                .copied()
                .map(|s| match s {
                    Severity::Error => "ERROR",
                    Severity::Warn => "WARN ",
                    Severity::Info => "INFO ",
                })
                .unwrap_or("     ");
            println!("  [{severity}] {code}: {count}");
            for example in report.anomaly_examples.get(code).into_iter().flatten() {
                println!("      e.g. {example}");
            }
        }
    } else {
        println!("\nHeader anomalies: none");
    }

    let games: Vec<&str> = match target {
        Some(id) => vec![id],
        None => ALL_GAMES.iter().map(|game| game.id).collect(),
    };
    println!("\nVerdicts per target game:");
    for game_id in games {
        let counts = report.verdicts.get(game_id);
        match counts {
            Some(counts) => {
                let summary = counts
                    .iter()
                    .map(|(verdict, count)| format!("{verdict}={count}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                println!("  {game_id}: {summary}");
            }
            None => println!("  {game_id}: no textures"),
        }
    }

    if let Some(reconstructible) = report.palette_reconstructible {
        println!(
            "\nPalette-reconstructible textures (<= 256 unique colors): {reconstructible} / {}",
            report.textures
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal valid IMG v2 archive with one TXD entry (a 4x4 DXT3
    /// native) and one non-TXD entry.
    fn fixture_archive(dir: &std::path::Path) -> std::path::PathBuf {
        let txd = fixture_txd_bytes();
        let mut img: Vec<u8> = Vec::new();
        img.extend_from_slice(b"VER2");
        img.extend_from_slice(&2_u32.to_le_bytes());
        // Entry 0: the TXD at sector 1.
        img.extend_from_slice(&1_u32.to_le_bytes());
        img.extend_from_slice(&1_u32.to_le_bytes());
        let mut name = [0_u8; 24];
        name[..9].copy_from_slice(b"test.txd\0");
        img.extend_from_slice(&name);
        // Entry 1: a model, no TXD suffix.
        img.extend_from_slice(&9_u32.to_le_bytes());
        img.extend_from_slice(&1_u32.to_le_bytes());
        let mut name = [0_u8; 24];
        name[..8].copy_from_slice(b"car.dff\0");
        img.extend_from_slice(&name);

        img.resize(2048, 0);
        let mut padded = txd;
        padded.resize(2048, 0);
        img.extend_from_slice(&padded);
        img.resize(4096, 0);

        let path = dir.join("fixture.img");
        std::fs::write(&path, &img).unwrap();
        path
    }

    fn fixture_txd_bytes() -> Vec<u8> {
        let section = |kind: u32, body: &[u8]| {
            let mut out = Vec::with_capacity(12 + body.len());
            out.extend_from_slice(&kind.to_le_bytes());
            out.extend_from_slice(&(body.len() as u32).to_le_bytes());
            out.extend_from_slice(&0x1803_FFFF_u32.to_le_bytes());
            out.extend_from_slice(body);
            out
        };
        let mut native = Vec::new();
        native.extend_from_slice(&9_u32.to_le_bytes());
        native.extend_from_slice(&[6, 17, 0, 0]);
        native.extend_from_slice(&[0; 32]);
        native.extend_from_slice(&[0; 32]);
        native.extend_from_slice(&0x0000_0300_u32.to_le_bytes());
        native.extend_from_slice(&0x3354_5844_u32.to_le_bytes());
        native.extend_from_slice(&4_u16.to_le_bytes());
        native.extend_from_slice(&4_u16.to_le_bytes());
        native.extend_from_slice(&[16, 1, 4, 9]);
        native.extend_from_slice(&16_u32.to_le_bytes());
        native.extend_from_slice(&[0xFF; 16]);
        let native_struct = section(1, &native);
        let native_section = section(0x15, &native_struct);
        let mut dict_body = section(1, &[1, 0, 2, 0]);
        dict_body.extend_from_slice(&native_section);
        section(0x16, &dict_body)
    }

    #[test]
    fn scanner_profiles_fixtures_and_counts_verdicts() {
        let dir = tempfile::tempdir().unwrap();
        let path = fixture_archive(dir.path());
        let report = scan_archive(&path, &ScanOptions::default()).unwrap();

        assert_eq!(report.entry_count, 2);
        assert_eq!(report.txd_entries, 1);
        assert_eq!(report.parse_failures, 0);
        assert_eq!(report.textures, 1);
        assert_eq!(report.class_counts.get("DXT3"), Some(&1));

        // The DXT3/16-bit native is native to SA and convertible for
        // GTA III (platform-9 in a platform-8 archive).
        let sa = report.verdicts.get("sa").unwrap();
        assert_eq!(sa.get("native"), Some(&1));
        let gta3 = report.verdicts.get("gta3").unwrap();
        assert_eq!(gta3.get("convertible (lossless)"), Some(&1));

        // Non-TXD entries don't register verdicts or failures.
        assert!(!report.class_counts.contains_key("unknown"));
    }

    #[test]
    fn scanner_flags_contradictory_headers() {
        // A DXT3 fourcc on an uncompressed 8888/32-bit header: the
        // shipped-and-fixed INU writer bug that engines silently drop.
        let dir = tempfile::tempdir().unwrap();
        let mut txd = fixture_txd_bytes();
        // Patch the raster format (0x300 -> 0x500) and depth (16 -> 32)
        // inside the native struct. The struct bytes start after the
        // dict + native section headers; locate the known byte pattern.
        let pattern = 0x0000_0300_u32.to_le_bytes();
        let pos = txd
            .windows(4)
            .position(|w| w == pattern)
            .expect("raster format bytes");
        txd[pos..pos + 4].copy_from_slice(&0x0000_0500_u32.to_le_bytes());
        let depth_pos = pos + 4 + 4 + 4; // fourcc + width + height
        txd[depth_pos] = 32;

        let mut img: Vec<u8> = Vec::new();
        img.extend_from_slice(b"VER2");
        img.extend_from_slice(&1_u32.to_le_bytes());
        img.extend_from_slice(&1_u32.to_le_bytes());
        img.extend_from_slice(&1_u32.to_le_bytes());
        let mut name = [0_u8; 24];
        name[..8].copy_from_slice(b"bad.txd\0");
        img.extend_from_slice(&name);
        img.resize(2048, 0);
        txd.resize(2048, 0);
        img.extend_from_slice(&txd);
        let path = dir.path().join("bad.img");
        std::fs::write(&path, &img).unwrap();

        let report = scan_archive(&path, &ScanOptions::default()).unwrap();
        assert!(report.anomaly_counts.contains_key("CONTRADICTORY_DXT_HEADER"));
        let gta3 = report.verdicts.get("gta3").unwrap();
        assert!(
            gta3.get("convertible (lossless)").copied().unwrap_or(0) >= 1,
            "a broken header is at least losslessly fixable"
        );
    }
}
