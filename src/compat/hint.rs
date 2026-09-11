//! Target-game hints: a cheap content probe that *suggests* which game an
//! archive belongs to, with the evidence it used.
//!
//! Hints are advisory only. The archive's target stays user metadata
//! (design principle 5) - the suggestion is shown with reasons and is
//! never applied automatically, because container/name inference is
//! unsound (three games ship `gta3.img`, and total conversions mix
//! dialects).

use std::collections::BTreeMap;

use crate::archive::ArchiveInfo;
use crate::parser::txd::parse_txd;
use crate::parser::{read_entry_data_from_source, ImgVersion};

use super::raster::{LogicalFormat, RasterProfile};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HintConfidence {
    High,
    Medium,
}

#[derive(Debug, Clone)]
pub struct TargetHint {
    pub game_id: &'static str,
    pub confidence: HintConfidence,
    /// Human-readable evidence, most important first.
    pub reasons: Vec<String>,
    pub sampled_textures: usize,
}

/// How many TXD entries the probe parses before deciding.
pub const PROBE_SAMPLE_LIMIT: usize = 96;

/// What the probe found, before it is turned into a hint.
#[derive(Debug, Default)]
pub struct ProbeStats {
    pub platform_counts: BTreeMap<u32, usize>,
    pub class_counts: BTreeMap<LogicalFormat, usize>,
    pub nft_entries: usize,
    pub nif_entries: usize,
    pub txd_entries: usize,
    pub sampled_txds: usize,
    pub textures: usize,
}

impl ProbeStats {
    pub fn share(&self, predicate: impl Fn(&LogicalFormat) -> bool) -> f32 {
        if self.textures == 0 {
            return 0.0;
        }
        let hits: usize = self
            .class_counts
            .iter()
            .filter(|(class, _)| predicate(class))
            .map(|(_, count)| count)
            .sum();
        hits as f32 / self.textures as f32
    }

    pub fn platform_share(&self, platform: u32) -> f32 {
        if self.textures == 0 {
            return 0.0;
        }
        let hits = self.platform_counts.get(&platform).copied().unwrap_or(0);
        hits as f32 / self.textures as f32
    }
}

/// Probe a sample of the archive's textures and the entry extensions to
/// suggest a game. `None` when the sample is too ambiguous to guess.
pub fn probe_target(archive: &ArchiveInfo, sample_limit: usize) -> Option<TargetHint> {
    let stats = collect_stats(archive, sample_limit);
    classify(&stats, archive.version)
}

pub fn collect_stats(archive: &ArchiveInfo, sample_limit: usize) -> ProbeStats {
    let mut stats = ProbeStats::default();
    for entry in &archive.entries {
        if entry.file_name_lower.ends_with(".nft") {
            stats.nft_entries += 1;
            continue;
        }
        if entry.file_name_lower.ends_with(".nif") {
            stats.nif_entries += 1;
            continue;
        }
        if !entry.file_name_lower.ends_with(".txd") {
            continue;
        }
        stats.txd_entries += 1;
        if stats.sampled_txds >= sample_limit {
            continue;
        }
        let Some(bytes) = read_entry_bytes(archive, entry) else {
            continue;
        };
        let Ok(parsed) = parse_txd(&bytes) else {
            continue;
        };
        stats.sampled_txds += 1;
        for texture in &parsed.textures {
            let profile = RasterProfile::from_native(texture);
            *stats.platform_counts.entry(profile.platform_id).or_default() += 1;
            *stats.class_counts.entry(profile.logical).or_default() += 1;
            stats.textures += 1;
        }
    }
    stats
}

fn read_entry_bytes(archive: &ArchiveInfo, entry: &crate::archive::EntryInfo) -> Option<Vec<u8>> {
    if let Some(mmap) = &archive.source_mmap {
        let start = entry.offset as usize * crate::parser::SECTOR_SIZE as usize;
        let end = start + entry.sector as usize * crate::parser::SECTOR_SIZE as usize;
        return mmap.get(start..end).map(|slice| slice.to_vec());
    }
    read_entry_data_from_source(entry, archive.path.as_deref()).ok()
}

fn percent(share: f32) -> String {
    format!("{:.0}%", share * 100.0)
}

/// Turn probe statistics into a suggestion. Rules are ordered by signal
/// strength: Gamebryo content is unambiguous, then the D3D9 platform,
/// then the dialect-defining raster classes.
pub fn classify(stats: &ProbeStats, version: ImgVersion) -> Option<TargetHint> {
    // 1. Gamebryo content (Bully): NFTs/NIFs cannot appear in an RW game.
    let gamebryo = stats.nft_entries + stats.nif_entries;
    if gamebryo >= 4 || (gamebryo > 0 && gamebryo >= stats.sampled_txds) {
        let confidence = if gamebryo >= 16 {
            HintConfidence::High
        } else {
            HintConfidence::Medium
        };
        return Some(TargetHint {
            game_id: "bully",
            confidence,
            reasons: vec![format!(
                "{gamebryo} Gamebryo entries ({} NFT, {} NIF)",
                stats.nft_entries, stats.nif_entries
            )],
            sampled_textures: stats.textures,
        });
    }

    if stats.textures == 0 {
        return None;
    }

    // 2. D3D9 platform dominates: San Andreas (the only retail D3D9 game).
    let platform9 = stats.platform_share(9);
    if platform9 >= 0.6 {
        return Some(TargetHint {
            game_id: "sa",
            confidence: if platform9 >= 0.85 {
                HintConfidence::High
            } else {
                HintConfidence::Medium
            },
            reasons: vec![format!(
                "platform 9 (D3D9) on {} of {} sampled rasters",
                percent(platform9),
                stats.textures
            )],
            sampled_textures: stats.textures,
        });
    }

    if stats.platform_share(8) < 0.6 {
        return None;
    }

    // 3. D3D8 platform: the raster classes separate III from VC.
    let pal8 = stats.share(|class| matches!(class, LogicalFormat::Pal8 | LogicalFormat::Pal4));
    if pal8 >= 0.5 {
        return Some(TargetHint {
            game_id: "gta3",
            confidence: if pal8 >= 0.85 {
                HintConfidence::High
            } else {
                HintConfidence::Medium
            },
            reasons: vec![
                format!("PAL8/PAL4 on {} of sampled rasters", percent(pal8)),
                "platform 8 (D3D8)".to_string(),
            ],
            sampled_textures: stats.textures,
        });
    }

    let vc16 = stats.share(|class| {
        matches!(
            class,
            LogicalFormat::R565 | LogicalFormat::R4444 | LogicalFormat::R1555
        )
    });
    if vc16 >= 0.5 {
        return Some(TargetHint {
            game_id: "vc",
            confidence: if vc16 >= 0.85 {
                HintConfidence::High
            } else {
                HintConfidence::Medium
            },
            reasons: vec![
                format!("16-bit 565/4444/1555 on {} of sampled rasters", percent(vc16)),
                "platform 8 (D3D8)".to_string(),
            ],
            sampled_textures: stats.textures,
        });
    }

    // 4. Compressed platform-8 content is a modding dialect, not a retail
    //    signal; only the container version narrows it (VER2 = SA builds).
    let _ = version;
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ImgVersion;

    fn stats(paths: &[(u32, LogicalFormat, usize)]) -> ProbeStats {
        let mut stats = ProbeStats::default();
        for (platform, class, count) in paths {
            *stats.platform_counts.entry(*platform).or_default() += count;
            *stats.class_counts.entry(*class).or_default() += count;
            stats.textures += count;
        }
        stats
    }

    #[test]
    fn gamebryo_content_hints_bully() {
        let mut stats = stats(&[(9, LogicalFormat::Dxt1, 2)]);
        stats.nft_entries = 40;
        stats.nif_entries = 12;
        let hint = classify(&stats, ImgVersion::One).expect("hint");
        assert_eq!(hint.game_id, "bully");
        assert_eq!(hint.confidence, HintConfidence::High);
    }

    #[test]
    fn d3d9_content_hints_san_andreas() {
        let hint = classify(
            &stats(&[(9, LogicalFormat::Dxt1, 90), (9, LogicalFormat::R8888, 10)]),
            ImgVersion::Two,
        )
        .expect("hint");
        assert_eq!(hint.game_id, "sa");
        assert_eq!(hint.confidence, HintConfidence::High);
    }

    #[test]
    fn paletted_d3d8_content_hints_gta_iii() {
        let hint = classify(
            &stats(&[(8, LogicalFormat::Pal8, 88), (8, LogicalFormat::R888, 12)]),
            ImgVersion::One,
        )
        .expect("hint");
        assert_eq!(hint.game_id, "gta3");
    }

    #[test]
    fn sixteen_bit_d3d8_content_hints_vice_city() {
        let hint = classify(
            &stats(&[(8, LogicalFormat::R565, 70), (8, LogicalFormat::R4444, 20)]),
            ImgVersion::One,
        )
        .expect("hint");
        assert_eq!(hint.game_id, "vc");
        assert_eq!(hint.confidence, HintConfidence::High);
    }

    #[test]
    fn ambiguous_compressed_d3d8_content_stays_unhinted() {
        // A platform-8 DXT mod pack could belong to III or VC; the probe
        // must not guess.
        assert!(classify(&stats(&[(8, LogicalFormat::Dxt1, 50)]), ImgVersion::One).is_none());
        assert!(classify(&ProbeStats::default(), ImgVersion::One).is_none());
    }
}
