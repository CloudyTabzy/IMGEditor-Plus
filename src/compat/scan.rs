//! Archive corpus scanner: the Phase 0 diagnostic that profiles every
//! texture in an IMG archive and reports class distribution, header
//! anomalies, and per-game verdicts. This is the verification tool for
//! the profile tables and the seed of the user-facing auditor.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Context;

use crate::archive::ArchiveInfo;
use crate::inspector::nif::{self, Endian};
use crate::parser::txd::parse_txd;
use crate::parser::{detect_version, read_entry_data_from_source, ImgVersion};
use crate::parser::pc_v1::PcV1Parser;
use crate::parser::pc_v2::PcV2Parser;
use crate::parser::xbox360::Xbox360Parser;
use crate::parser::ImgParser;

use super::games::{classify, ALL_GAMES};
use super::raster::{LogicalFormat, RasterProfile, Severity};

#[derive(Debug, Clone, Default)]
pub struct ScanReport {
    pub archive_path: String,
    pub archive_kind: String,
    pub entry_count: usize,
    pub txd_entries: usize,
    /// RenderWare model entries (counted, not profiled).
    pub nif_entries: usize,
    /// Gamebryo texture entries (Bully World.img et al.).
    pub nft_entries: usize,
    pub parse_failures: usize,
    pub textures: usize,
    /// NiPixelData blocks profiled across all NFT entries.
    pub nft_textures: usize,
    /// NiPixelData format name -> count ("DXT1", "PAL", ...).
    pub nft_formats: BTreeMap<String, usize>,
    /// Game id -> verdict label -> NFT raster count.
    pub nft_verdicts: BTreeMap<&'static str, BTreeMap<&'static str, usize>>,
    /// NFT rasters whose NiPixelData carries a palette reference.
    pub nft_paletted: usize,
    /// Logical-format histogram ("DXT1", "888 (32bpp)".).
    pub class_counts: BTreeMap<String, usize>,
    /// Anomaly code → occurrences.
    pub anomaly_counts: BTreeMap<&'static str, usize>,
    /// First few offending texture names per anomaly code.
    pub anomaly_examples: BTreeMap<&'static str, Vec<String>>,
    /// Game id → verdict label → texture count.
    pub verdicts: BTreeMap<&'static str, BTreeMap<&'static str, usize>>,
    /// The game the per-entry verdicts were computed against.
    pub target: Option<&'static str>,
    /// Per-entry verdicts for `target` (empty when no target was set).
    pub entry_verdicts: Vec<EntryVerdict>,
    /// Palette-reconstructible texture count (only with pixel decoding).
    pub palette_reconstructible: Option<usize>,
    /// Worst severity per anomaly code for report ordering.
    pub anomaly_severity: BTreeMap<&'static str, Severity>,
}
#[derive(Debug, Clone, Default)]
pub struct ScanOptions {
    /// Decode pixels to test palette-reconstructibility (≤ 256 unique
    /// colors). Slower: one RGBA decode per texture.
    pub decode_pixels: bool,
    /// Target game id for the per-entry verdict pass. When set, the
    /// report carries per-entry worst verdicts so the UI can highlight
    /// rows; when `None`, only the aggregate all-games table is built.
    pub target: Option<&'static str>,
}

/// Per-entry compatibility verdict for one target game. Recorded only
/// when [`ScanOptions::target`] is set.
#[derive(Debug, Clone)]
pub struct EntryVerdict {
    pub entry_index: usize,
    pub file_name: String,
    pub textures: usize,
    /// Most severe verdict among the entry's textures.
    pub worst: crate::compat::games::Verdict,
    /// Verdict label -> count for this entry.
    pub counts: BTreeMap<&'static str, usize>,
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
        target: options.target,
        ..ScanReport::default()
    };
    profile_entries(&archive, &mut report, options, None)?;
    Ok(report)
}

/// Validate an already-open archive: the app-facing path. Reports
/// progress through the archive's own [`ProgressInfo`] (which drives
/// the toolbar progress bar and the cancel button) and bails with a
/// "cancelled" error when the user cancels.
pub fn validate_open_archive(
    archive: &ArchiveInfo,
    options: &ScanOptions,
) -> anyhow::Result<ScanReport> {
    // Resolve the source *before* claiming the progress slot: an early
    // return after start() would leave the archive stuck "in use" and
    // block every later run.
    let source = archive
        .path
        .clone()
        .ok_or_else(|| anyhow::anyhow!("archive has no source path"))?;
    let progress = archive.progress.clone();
    progress.start();
    let mut report = ScanReport {
        archive_path: source.display().to_string(),
        archive_kind: version_text(archive.version).to_string(),
        entry_count: archive.entries.len(),
        target: options.target,
        ..ScanReport::default()
    };
    let result = profile_entries(archive, &mut report, options, Some(&progress));
    progress.finish();
    result?;
    Ok(report)
}

impl ScanReport {
    /// Total textures carrying ERROR-severity anomalies.
    pub fn error_count(&self) -> usize {
        self.anomaly_counts
            .iter()
            .filter(|(code, _)| self.anomaly_severity.get(*code) == Some(&Severity::Error))
            .map(|(_, count)| *count)
            .sum()
    }

    /// Total textures carrying WARN-severity anomalies.
    pub fn warning_count(&self) -> usize {
        self.anomaly_counts
            .iter()
            .filter(|(code, _)| self.anomaly_severity.get(*code) == Some(&Severity::Warn))
            .map(|(_, count)| *count)
            .sum()
    }
}

/// Shared profiling loop: reads each TXD entry (mmap slice when the
/// archive is mapped), parses it, and accumulates the report.
fn profile_entries(
    archive: &ArchiveInfo,
    report: &mut ScanReport,
    options: &ScanOptions,
    progress: Option<&crate::archive::ProgressInfo>,
) -> anyhow::Result<()> {
    let total = archive.entries.len();
    for (index, entry) in archive.entries.iter().enumerate() {
        if let Some(progress) = progress {
            if progress.is_cancelled() {
                anyhow::bail!("Validation cancelled");
            }
            if index % 16 == 0 || index + 1 == total {
                progress.set_percentage((index + 1) as f32 / total.max(1) as f32);
            }
        }
        if entry.file_name_lower.ends_with(".txd") {
            profile_txd_entry(archive, index, entry, report, options)?;
        } else if entry.file_name_lower.ends_with(".nft") {
            profile_nft_entry(archive, index, entry, report, options)?;
        } else if entry.file_name_lower.ends_with(".nif") {
            report.nif_entries += 1;
        }
    }
    Ok(())
}

fn profile_txd_entry(
    archive: &ArchiveInfo,
    entry_index: usize,
    entry: &crate::archive::EntryInfo,
    report: &mut ScanReport,
    options: &ScanOptions,
) -> anyhow::Result<()> {
    report.txd_entries += 1;
    let Some(bytes) = read_entry_bytes(archive, entry, report, "TXD_READ_FAIL")? else {
        return Ok(());
    };
    let parsed = match parse_txd(&bytes) {
            Ok(parsed) => parsed,
            Err(err) => {
                report.parse_failures += 1;
                record_anomaly(
                    report,
                    "TXD_PARSE_FAIL",
                    Severity::Error,
                    &entry.file_name,
                    err,
                );
                return Ok(());
            }
        };

        // Per-entry verdicts for the chosen target power the row
        // highlights in the UI. Computed alongside the aggregate pass.
        let target = options.target.and_then(crate::compat::games::profile_by_id);
        let mut entry_counts: BTreeMap<&'static str, usize> = BTreeMap::new();
        let mut entry_worst: Option<crate::compat::games::Verdict> = None;

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
                    report,
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

            if let Some(target) = target {
                let verdict = classify(target, &profile).verdict;
                *entry_counts.entry(verdict.label()).or_default() += 1;
                entry_worst = Some(match entry_worst {
                    Some(previous) => previous.max(verdict),
                    None => verdict,
                });
            }

            if options.decode_pixels {
                match texture.decode_rgba() {
                    Ok(rgba) if unique_color_count(&rgba) <= 256 => {
                        *report.palette_reconstructible.get_or_insert(0) += 1;
                    }
                    Ok(_) => {}
                    Err(err) => {
                         record_anomaly(
                             report,
                             "TXD_DECODE_FAIL",
                             Severity::Error,
                             texture.diffuse_name.as_str(),
                             format!("{err}"),
                         );
                    }
                }
            }
        }

        if let (Some(_), Some(worst)) = (target, entry_worst) {
            report.entry_verdicts.push(EntryVerdict {
                entry_index,
                file_name: entry.file_name.to_string(),
                textures: parsed.textures.len(),
                worst,
                counts: entry_counts,
            });
        }
    Ok(())
}

/// Read one entry's bytes through the mmap when available. Returns
/// `Ok(None)` after recording a read-failure anomaly for the entry.
fn read_entry_bytes(
    archive: &ArchiveInfo,
    entry: &crate::archive::EntryInfo,
    report: &mut ScanReport,
    code: &'static str,
) -> anyhow::Result<Option<Vec<u8>>> {
    match (&archive.source_mmap, &archive.path) {
        (Some(mmap), _) => {
            let start = entry.offset as usize * crate::parser::SECTOR_SIZE as usize;
            let end = start + entry.sector as usize * crate::parser::SECTOR_SIZE as usize;
            match mmap.get(start..end) {
                Some(slice) => Ok(Some(slice.to_vec())),
                None => {
                    report.parse_failures += 1;
                    record_anomaly(
                        report,
                        code,
                        Severity::Error,
                        &entry.file_name,
                        "entry range lies outside the archive".to_string(),
                    );
                    Ok(None)
                }
            }
        }
        (None, Some(source)) => match read_entry_data_from_source(entry, Some(source)) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(err) => {
                report.parse_failures += 1;
                record_anomaly(report, code, Severity::Error, &entry.file_name, format!("{err}"));
                Ok(None)
            }
        },
        (None, None) => anyhow::bail!("archive has no source to read from"),
    }
}

/// One decoded Gamebryo raster (NiPixelData block).
#[derive(Debug, Clone, Copy)]
pub struct NftPixelInfo {
    pub format: u32,
    pub width: u32,
    pub height: u32,
    pub mipmaps: u32,
    pub palette_ref: i32,
}

impl NftPixelInfo {
    pub fn format_name(&self) -> &'static str {
        nif_format_name(self.format)
    }

    pub fn paletted(&self) -> bool {
        self.palette_ref >= 0 || self.format <= 3
    }
}

/// NiPixelData `PixelFormat` enum values (nif.xml).
pub fn nif_format_name(format: u32) -> &'static str {
    match format {
        0 => "RGB",
        1 => "RGBA",
        2 => "PAL",
        3 => "PALA",
        4 => "DXT1",
        5 => "DXT3",
        6 => "DXT5",
        7 => "RGB24NONINT",
        8 => "BUMP",
        9 => "BUMPLUMA",
        10 => "RENDERSPEC",
        11 => "1CH",
        12 => "2CH",
        13 => "3CH",
        14 => "4CH",
        _ => "OTHER",
    }
}

/// Profile one Bully-style NFT entry: parse the NIF header (cheap,
/// no payloads) and decode every NiPixelData block's tail.
fn profile_nft_entry(
    archive: &ArchiveInfo,
    entry_index: usize,
    entry: &crate::archive::EntryInfo,
    report: &mut ScanReport,
    options: &ScanOptions,
) -> anyhow::Result<()> {
    report.nft_entries += 1;
    let Some(bytes) = read_entry_bytes(archive, entry, report, "NFT_READ_FAIL")? else {
        return Ok(());
    };
    let header = match nif::NifFile::parse_header(&bytes) {
        Ok(header) => header,
        Err(err) => {
            report.parse_failures += 1;
            record_anomaly(
                report,
                "NFT_HEADER_FAIL",
                Severity::Error,
                &entry.file_name,
                format!("{err}"),
            );
            return Ok(());
        }
    };

    // Per-entry verdicts for the chosen target power the row highlights
    // for Bully archives, exactly like the TXD path.
    let target = options.target.and_then(crate::compat::games::profile_by_id);
    let mut entry_counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut entry_worst: Option<crate::compat::games::Verdict> = None;
    let mut entry_textures = 0_usize;

    for block in &header.blocks {
        if block.type_name != "NiPixelData" {
            continue;
        }
        let end = block.offset as usize + block.size as usize;
        let Some(raw) = bytes.get(block.offset as usize..end) else {
            report.parse_failures += 1;
            record_anomaly(
                report,
                "NFT_HEADER_FAIL",
                Severity::Error,
                &entry.file_name,
                "NiPixelData block range outside entry".to_string(),
            );
            continue;
        };
        report.nft_textures += 1;
        entry_textures += 1;
        let format = info_format_label(raw, header.endian);
        for game in ALL_GAMES {
            let verdict = crate::compat::games::classify_nft_format(game, format);
            *report
                .nft_verdicts
                .entry(game.id)
                .or_default()
                .entry(verdict.verdict.label())
                .or_default() += 1;
        }
        if let Some(target) = target {
            let verdict = crate::compat::games::classify_nft_format(target, format).verdict;
            *entry_counts.entry(verdict.label()).or_default() += 1;
            entry_worst = Some(match entry_worst {
                Some(previous) => previous.max(verdict),
                None => verdict,
            });
        }
        match decode_ni_pixel_tail(raw, header.endian) {
            Some(info) => {
                *report.nft_formats.entry(info.format_name().to_string()).or_default() += 1;
                if info.paletted() {
                    report.nft_paletted += 1;
                }
                if info.width > 0 {
                    if !info.width.is_power_of_two() || !info.height.is_power_of_two() {
                        record_anomaly(
                            report,
                            "NFT_NON_POT",
                            Severity::Warn,
                            &entry.file_name,
                            format!("{}x{}", info.width, info.height),
                        );
                    }
                    if (4..=6).contains(&info.format) {
                        // Compressed rasters typically ship a full mip
                        // chain; radars/sprites legitimately skip mips,
                        // so keep this informational only.
                        let expected = 32 - (info.width.max(info.height) - 1).leading_zeros() + 1;
                        if info.mipmaps > 0 && info.mipmaps < expected {
                            record_anomaly(
                                report,
                                "NFT_MIP_CHAIN_SHORT",
                                Severity::Info,
                                &entry.file_name,
                                format!(
                                    "{}x{} has {} mips, full chain is {}",
                                    info.width, info.height, info.mipmaps, expected
                                ),
                            );
                        }
                    }
                }
            }
            None => {
                let fmt = info_format_label(raw, header.endian);
                record_anomaly(
                    report,
                    "NFT_PIXELDATA_UNPARSED",
                    Severity::Info,
                    &entry.file_name,
                    format!("format {fmt} ({}), dims not decodable", nif_format_name(fmt)),
                );
                *report
                    .nft_formats
                    .entry(nif_format_name(fmt).to_string())
                    .or_default() += 1;
            }
        }
    }

    if target.is_some() && entry_worst.is_some() {
        report.entry_verdicts.push(EntryVerdict {
            entry_index,
            file_name: entry.file_name.to_string(),
            textures: entry_textures,
            worst: entry_worst.unwrap_or(crate::compat::games::Verdict::Untested),
            counts: entry_counts,
        });
    }
    Ok(())
}

fn info_format_label(raw: &[u8], endian: Endian) -> u32 {
    read_u32_at(raw, 0, endian).unwrap_or(u32::MAX)
}

fn read_u32_at(bytes: &[u8], pos: usize, endian: Endian) -> Option<u32> {
    let b = bytes.get(pos..pos + 4)?;
    let raw = [b[0], b[1], b[2], b[3]];
    Some(match endian {
        Endian::Little => u32::from_le_bytes(raw),
        Endian::Big => u32::from_be_bytes(raw),
    })
}

/// Decode the well-known tail of a 20.3.0.9 `NiPixelData` block:
/// `[Palette ref][Num mipmaps][Bytes/px][MipMap{x12}...][Num px][Num faces][data]`
/// with the variable-length NiPixelFormat prefix in front. The scan
/// walks backward from the data length, so it never depends on the
/// prefix layout; the count/size cross-check rejects mismatches.
fn decode_ni_pixel_tail(raw: &[u8], endian: Endian) -> Option<NftPixelInfo> {
    let format = read_u32_at(raw, 0, endian)?;
    let end = raw.len();

    // Find the data start. Two layouts exist in the corpus:
    // `[Num pixels][Num faces][data]` (10.4.0.2+) and the older
    // `[Num pixels][data]` without the faces count. Try the faces
    // layout first; it is the more constrained match.
    let mut num_pixels = 0_u32;
    let mut data_start = None;
    let mut d = end.saturating_sub(8);
    while d + 8 <= end {
        let (px, faces) = match (
            read_u32_at(raw, d, endian),
            read_u32_at(raw, d + 4, endian),
        ) {
            (Some(px), Some(faces)) => (px, faces),
            _ => break,
        };
        if (1..=8).contains(&faces) && px > 0 && px.checked_mul(faces) == Some((end - d - 8) as u32)
        {
            num_pixels = px;
            data_start = Some(d);
            break;
        }
        d = match d.checked_sub(4) {
            Some(v) => v,
            None => break,
        };
    }
    if data_start.is_none() {
        let mut d = end.saturating_sub(4);
        while d + 4 <= end {
            match read_u32_at(raw, d, endian) {
                Some(px) if px > 0 && px as usize == end - d - 4 => {
                    num_pixels = px;
                    data_start = Some(d);
                    break;
                }
                _ => {}
            }
            d = match d.checked_sub(4) {
                Some(v) => v,
                None => break,
            };
        }
    }
    let mip_region_end = data_start?;

    // Walk back over 12-byte MipMap {width, height, offset} entries.
    // Each earlier (larger) mip must double the later one, which keeps
    // the walk honest even for non-power-of-two textures.
    let mut count = 0_usize;
    let mut mip0 = (0_u32, 0_u32);
    let mut prev = (0_u32, 0_u32);
    let mut cursor = mip_region_end;
    for _ in 0..16 {
        let entry_start = cursor.checked_sub(12)?;
        let w = read_u32_at(raw, entry_start, endian)?;
        let h = read_u32_at(raw, entry_start + 4, endian)?;
        let off = read_u32_at(raw, entry_start + 8, endian)?;
        let plausible = (1..=8192).contains(&w)
            && (1..=8192).contains(&h)
            && (off as u64) < num_pixels as u64;
        if !plausible {
            break;
        }
        if count > 0 {
            // Walking backward means each earlier entry doubles the
            // later one; a dimension pinned at 1 may stay 1 (wide
            // textures) or double (square ones).
            if !(w == prev.0 * 2 || (prev.0 == 1 && w == 1)) {
                break;
            }
            if !(h == prev.1 * 2 || (prev.1 == 1 && h == 1)) {
                break;
            }
        }
        mip0 = (w, h);
        prev = (w, h);
        count += 1;
        cursor = entry_start;
    }
    if count == 0 {
        return None;
    }
    let mip0_start = mip_region_end - 12 * count;
    let declared_mips = read_u32_at(raw, mip0_start - 8, endian)?;
    if declared_mips as usize != count {
        return None;
    }
    let _bytes_per_pixel = read_u32_at(raw, mip0_start - 4, endian)?;
    let palette_ref = read_u32_at(raw, mip0_start - 12, endian)? as i32;

    Some(NftPixelInfo {
        format,
        width: mip0.0,
        height: mip0.1,
        mipmaps: declared_mips,
        palette_ref,
    })
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
            target: target.and_then(|id| crate::compat::games::profile_by_id(id).map(|g| g.id)),
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
    if report.nft_entries > 0 || report.nif_entries > 0 {
        println!(
            "Gamebryo content: NFT entries: {} | NIF entries: {} | NiPixelData rasters: {}",
            report.nft_entries, report.nif_entries, report.nft_textures
        );
    }

    println!("\nRaster class histogram:");
    for (class, count) in &report.class_counts {
        println!("  {class:24} {count}");
    }

    if !report.nft_formats.is_empty() {
        println!("\nNFT raster histogram:");
        for (class, count) in &report.nft_formats {
            println!("  {class:24} {count}");
        }
        if report.nft_paletted > 0 {
            println!("  paletted (palette ref present): {}", report.nft_paletted);
        }
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
        match (
            report.verdicts.get(game_id),
            report.nft_verdicts.get(game_id),
        ) {
            (None, None) => println!("  {game_id}: no textures"),
            (rw, nft) => {
                if let Some(counts) = rw {
                    let summary = counts
                        .iter()
                        .map(|(verdict, count)| format!("{verdict}={count}"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    println!("  {game_id} (RW): {summary}");
                }
                if let Some(counts) = nft {
                    let summary = counts
                        .iter()
                        .map(|(verdict, count)| format!("{verdict}={count}"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    println!("  {game_id} (NFT): {summary}");
                }
            }
        }
    }

    if let Some(target) = report.target {
        if !report.entry_verdicts.is_empty() {
            println!(
                "\nRow verdicts: {} entries classified for the {target} target",
                report.entry_verdicts.len()
            );
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
    fn validate_open_archive_reports_and_finishes_progress() {
        let dir = tempfile::tempdir().unwrap();
        let path = fixture_archive(dir.path());
        let mut archive = ArchiveInfo::new("fixture.img", false, ImgVersion::Two);
        archive.path = Some(path.clone());
        PcV2Parser.open(&mut archive).unwrap();

        let report = validate_open_archive(&archive, &ScanOptions::default()).unwrap();
        assert_eq!(report.txd_entries, 1);
        assert_eq!(report.textures, 1);
        assert_eq!(report.archive_kind, "IMG v2");
        assert!(!archive.progress.in_use(), "progress must be released");

        // Re-running works: the progress slot is not stuck in use.
        let again = validate_open_archive(&archive, &ScanOptions::default()).unwrap();
        assert_eq!(again.textures, 1);
    }

    #[test]
    fn validate_loop_respects_cancellation() {
        let dir = tempfile::tempdir().unwrap();
        let path = fixture_archive(dir.path());
        let mut archive = ArchiveInfo::new("fixture.img", false, ImgVersion::Two);
        archive.path = Some(path);
        PcV2Parser.open(&mut archive).unwrap();

        // Cancel while the loop is running (start() clears stale flags,
        // so the request must land after it).
        let progress = archive.progress.clone();
        progress.start();
        progress.request_cancel();
        let mut report = ScanReport::default();
        let error = profile_entries(&archive, &mut report, &ScanOptions::default(), Some(&progress))
            .expect_err("cancelled validation must fail");
        progress.finish();
        assert!(format!("{error}").contains("cancelled"));
        assert!(!archive.progress.in_use(), "cancel must release the slot");
    }

    #[test]
    fn scanner_profiles_fixtures_and_counts_verdicts() {        let dir = tempfile::tempdir().unwrap();
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

    /// Build one 20.3.0.9 NiPixelData block body (after the block-type
    /// dispatch) for the given format, mip chain, and palette ref.
    fn nif_pixel_data_block(fmt: u32, mips: &[(u32, u32)], palette: i32) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&fmt.to_le_bytes());
        b.push(0); // bits per pixel: compressed
        b.extend_from_slice(&(-1_i32).to_le_bytes()); // renderer hint
        b.extend_from_slice(&0_u32.to_le_bytes()); // extra data
        b.push(1); // flags
        b.extend_from_slice(&0_u32.to_le_bytes()); // tiling
        b.push(0); // sRGB
        b.extend_from_slice(&[4, 0, 0, 0]); // channels
        b.extend_from_slice(&(palette as u32).to_le_bytes());
        b.extend_from_slice(&(mips.len() as u32).to_le_bytes());
        b.extend_from_slice(&0_u32.to_le_bytes()); // bytes per pixel
        let mut offset = 0_u32;
        for (w, h) in mips {
            b.extend_from_slice(&w.to_le_bytes());
            b.extend_from_slice(&h.to_le_bytes());
            b.extend_from_slice(&offset.to_le_bytes());
            offset += (*w * *h / 2).max(8);
        }
        b.extend_from_slice(&offset.to_le_bytes()); // num pixels
        b.extend_from_slice(&1_u32.to_le_bytes()); // num faces
        b.resize(b.len() + offset as usize, 0x8A);
        b
    }

    fn nif_nft_bytes(pixel_data: &[u8]) -> Vec<u8> {
        crate::inspector::nif::tests::build_nif(&[
            ("NiSourceTexture", &[0xAA; 44]),
            ("NiPixelData", pixel_data),
        ])
    }

    #[test]
    fn nft_tail_decodes_radar_like_single_mip() {
        let raw = nif_pixel_data_block(4, &[(8, 8)], -1);
        let info = decode_ni_pixel_tail(&raw, Endian::Little).expect("decodable");
        assert_eq!(info.format, 4);
        assert_eq!((info.width, info.height), (8, 8));
        assert_eq!(info.mipmaps, 1);
        assert_eq!(info.palette_ref, -1);
        assert!(!info.paletted());
        assert_eq!(info.format_name(), "DXT1");
    }

    #[test]
    fn nft_tail_decodes_full_chain_and_pal_palette() {
        let chain = nif_pixel_data_block(4, &[(8, 8), (4, 4), (2, 2), (1, 1)], -1);
        let info = decode_ni_pixel_tail(&chain, Endian::Little).expect("decodable");
        assert_eq!((info.width, info.height), (8, 8));
        assert_eq!(info.mipmaps, 4);

        let pal = nif_pixel_data_block(2, &[(64, 64)], 2);
        let info = decode_ni_pixel_tail(&pal, Endian::Little).expect("decodable");
        assert_eq!(info.format, 2);
        assert_eq!(info.format_name(), "PAL");
        assert!(info.paletted(), "a palette ref counts as paletted");
    }

    #[test]
    fn nft_tail_decodes_pre_10_4_layout_without_faces() {
        // Older exports end the block with `[Num pixels][data]`.
        let raw = nif_pixel_data_block(4, &[(8, 8)], -1);
        // Strip the faces field AND the original num-pixels field,
        // then rebuild the tail as `[Num pixels][data]`.
        let data_len = 32_u32;
        let mut body = raw[..raw.len() - 8 - data_len as usize].to_vec();
        body.extend_from_slice(&data_len.to_le_bytes());
        body.extend(vec![0x5A; data_len as usize]);
        let info = decode_ni_pixel_tail(&body, Endian::Little).expect("decodable");
        assert_eq!(info.format, 4);
        assert_eq!((info.width, info.height), (8, 8));
        assert_eq!(info.mipmaps, 1);
    }

    #[test]
    fn nft_tail_rejects_garbage() {
        let raw = vec![0x41_u8; 60];
        assert!(decode_ni_pixel_tail(&raw, Endian::Little).is_none());
        let raw = nif_pixel_data_block(4, &[(8, 8)], -1);
        // A truncated block (no data bytes) must not decode.
        assert!(decode_ni_pixel_tail(&raw[..raw.len() - 20], Endian::Little).is_none());
    }

    #[test]
    fn scanner_records_per_entry_verdicts_for_the_target() {
        use crate::compat::games::Verdict;

        let dir = tempfile::tempdir().unwrap();
        let path = fixture_archive(dir.path());

        // The fixture TXD is a DXT3 native on platform 9.
        let sa = scan_archive(
            &path,
            &ScanOptions {
                decode_pixels: false,
                target: Some("sa"),
            },
        )
        .unwrap();
        assert_eq!(sa.target, Some("sa"));
        assert_eq!(sa.entry_verdicts.len(), 1);
        let entry = &sa.entry_verdicts[0];
        assert_eq!(entry.file_name, "test.txd");
        assert_eq!(entry.textures, 1);
        assert_eq!(entry.worst, Verdict::Native);
        assert_eq!(entry.counts.get("native"), Some(&1));

        // Against GTA III the same raster needs a platform rewrite
        // (platform 9 in a platform-8 dialect), which is lossless.
        let gta3 = scan_archive(
            &path,
            &ScanOptions {
                decode_pixels: false,
                target: Some("gta3"),
            },
        )
        .unwrap();
        assert_eq!(gta3.entry_verdicts[0].worst, Verdict::ConvertibleLossless);

        // Without a target the per-entry pass is skipped.
        let none = scan_archive(&path, &ScanOptions::default()).unwrap();
        assert!(none.entry_verdicts.is_empty());
        assert_eq!(none.target, None);
    }

    #[test]
    fn scanner_profiles_bully_nft_entries() {
        let dir = tempfile::tempdir().unwrap();
        let nft_full = nif_nft_bytes(&nif_pixel_data_block(4, &[(8, 8), (4, 4), (2, 2), (1, 1)], -1));
        let nft_short = nif_nft_bytes(&nif_pixel_data_block(4, &[(8, 8), (4, 4), (2, 2)], -1));
        let nft_pal = nif_nft_bytes(&nif_pixel_data_block(2, &[(32, 32)], 2));
        let nft_nonpot = nif_nft_bytes(&nif_pixel_data_block(4, &[(48, 48)], -1));
        let nif_model = crate::inspector::nif::tests::build_nif(&[("NiNode", &[0u8; 8])]);

        let entries = [
            ("tex_full.nft", nft_full),
            ("model.nif", nif_model),
            ("tex_short.nft", nft_short),
            ("tex_pal.nft", nft_pal),
            ("tex_nonpot.nft", nft_nonpot),
        ];
        let mut img: Vec<u8> = Vec::new();
        img.extend_from_slice(b"VER2");
        img.extend_from_slice(&(entries.len() as u32).to_le_bytes());
        for (i, (name, _)) in entries.iter().enumerate() {
            img.extend_from_slice(&((i + 1) as u32).to_le_bytes());
            img.extend_from_slice(&1_u32.to_le_bytes());
            let mut name_buf = [0_u8; 24];
            name_buf[..name.len()].copy_from_slice(name.as_bytes());
            img.extend_from_slice(&name_buf);
        }
        img.resize(2048, 0);
        for (_, data) in &entries {
            let mut padded = data.clone();
            padded.resize(2048, 0);
            img.extend_from_slice(&padded);
        }
        let path = dir.path().join("world.img");
        std::fs::write(&path, &img).unwrap();

        let report = scan_archive(&path, &ScanOptions::default()).unwrap();
        assert_eq!(report.nft_entries, 4);
        assert_eq!(report.nif_entries, 1);
        assert_eq!(report.nft_textures, 4);
        assert_eq!(report.parse_failures, 0);
        assert_eq!(report.nft_formats.get("DXT1"), Some(&3));
        assert_eq!(report.nft_formats.get("PAL"), Some(&1));
        assert_eq!(report.nft_paletted, 1);
        // tex_short (3 of 4 mips) and the non-POT raster (1 mip).
        assert_eq!(report.anomaly_counts.get("NFT_MIP_CHAIN_SHORT"), Some(&2));
        assert_eq!(report.anomaly_counts.get("NFT_NON_POT"), Some(&1));
        assert!(!report.anomaly_counts.contains_key("NFT_HEADER_FAIL"));
        assert!(!report.anomaly_counts.contains_key("NFT_PIXELDATA_UNPARSED"));

        // NFT verdicts: the three DXT1 + one PAL rasters are native to
        // Bully; every RenderWare target marks them unsupported.
        let bully = report.nft_verdicts.get("bully").unwrap();
        assert_eq!(bully.get("native"), Some(&4));
        for game in ["gta3", "vc", "sa"] {
            let verdicts = report.nft_verdicts.get(game).unwrap();
            assert_eq!(verdicts.get("unsupported"), Some(&4), "{game}");
        }

        // Per-entry verdicts for a chosen target (row highlighting):
        // all four NFT entries are native for Bully, unsupported for a
        // RenderWare target.
        use crate::compat::games::Verdict;
        let bully = scan_archive(
            &path,
            &ScanOptions {
                decode_pixels: false,
                target: Some("bully"),
            },
        )
        .unwrap();
        assert_eq!(bully.entry_verdicts.len(), 4, "one verdict per NFT entry");
        assert!(bully
            .entry_verdicts
            .iter()
            .all(|entry| entry.worst == Verdict::Native));

        let gta3 = scan_archive(
            &path,
            &ScanOptions {
                decode_pixels: false,
                target: Some("gta3"),
            },
        )
        .unwrap();
        assert_eq!(gta3.entry_verdicts.len(), 4);
        assert!(gta3
            .entry_verdicts
            .iter()
            .all(|entry| entry.worst == Verdict::Unsupported));
    }
}
