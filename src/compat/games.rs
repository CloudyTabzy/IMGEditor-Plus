//! Per-game dialect profiles and the texture verdict tables.
//!
//! Every verdict carries its evidence class so untested cells stay
//! visible in code (see docs/asset-compatibility-engine.md §3). As of
//! 2026-09-11 all four retail corpora are verified: III, VC, SA and
//! Bully rows are `Evidence::Retail`, and cells retail absence cannot
//! resolve stay `Untested` with a retail note.
//!
//! [`validate_rasters`] is the validator primitive: it reduces a set
//! of rasters (an imported file, an entry, a whole archive) to a
//! [`ValidationSummary`] against one target profile.

use std::collections::BTreeMap;

use super::raster::{LogicalFormat, RasterProfile};

/// How confident we are in a verdict.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Evidence {
    /// Measured against the modded-corpus forensics (dialect evidence).
    Corpus,
    /// Documented behavior from validated third-party tooling.
    Docs,
    /// Verified against a retail game archive.
    Retail,
    /// No evidence either way — surfaced as "?" in the UI.
    Untested,
}

/// Compatibility verdict for one texture against one target game.
/// Declaration order = severity order (combine() relies on it).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Verdict {
    /// Insufficient evidence to judge — sorts lowest so any concrete
    /// verdict wins when combined.
    Untested,
    /// Engine-authored format; no action needed.
    Native,
    /// Loads, but wasteful or unusual for this engine.
    Supported,
    /// Loads after a lossless transform (format/platform rewrite).
    ConvertibleLossless,
    /// Requires a pixel-changing transform (compression, quantization).
    LossyConvertible,
    /// The engine cannot consume this asset form.
    Unsupported,
}

impl Verdict {
    pub fn label(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Supported => "supported",
            Self::ConvertibleLossless => "convertible (lossless)",
            Self::LossyConvertible => "convertible (lossy)",
            Self::Unsupported => "unsupported",
            Self::Untested => "untested",
        }
    }

    /// Higher severity wins when combining a format verdict with a
    /// container/platform verdict.
    fn combine(self, other: Self) -> Self {
        if self >= other {
            self
        } else {
            other
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GameProfile {
    pub id: &'static str,
    pub display: &'static str,
    /// Platform ids this engine's RW layer accepts for texture natives.
    pub platforms: &'static [u32],
    /// Canonical packed RW version this game's vanilla writes.
    pub rw_version: u32,
    /// Inclusive RW-version band this game shipped across builds.
    pub rw_range: (u32, u32),
}

pub const GTA3: GameProfile = GameProfile {
    id: "gta3",
    display: "GTA III",
    platforms: &[8],
    rw_version: 0x33002,
    rw_range: (0x30000, 0x34000),
};

pub const VC: GameProfile = GameProfile {
    id: "vc",
    display: "Vice City",
    platforms: &[8],
    rw_version: 0x35000,
    rw_range: (0x34000, 0x36000),
};

/// SA writes platform-9 (D3D9) natives — retail-verified 2026-09-11
/// across all four archives; platform-8 acceptance is unmeasured.
pub const SA: GameProfile = GameProfile {
    id: "sa",
    display: "San Andreas",
    platforms: &[8, 9],
    rw_version: 0x36003,
    rw_range: (0x36000, 0x40000),
};

/// Bully's assets are Gamebryo (NIF/NFT), not RenderWare natives.
pub const BULLY: GameProfile = GameProfile {
    id: "bully",
    display: "Bully",
    platforms: &[],
    rw_version: 0,
    rw_range: (0, 0),
};

pub const ALL_GAMES: [&GameProfile; 4] = [&GTA3, &VC, &SA, &BULLY];

pub fn profile_by_id(id: &str) -> Option<&'static GameProfile> {
    ALL_GAMES.into_iter().find(|game| game.id == id)
}

/// One named raster that is not native to the target, worst first.
#[derive(Debug, Clone)]
pub struct Offender {
    pub name: String,
    pub verdict: Verdict,
    pub note: String,
}

/// Aggregated verdicts for a set of rasters (one imported file, one
/// entry, or a whole archive) against a single target game.
#[derive(Debug, Clone)]
pub struct ValidationSummary {
    pub game_id: &'static str,
    pub display: &'static str,
    pub textures: usize,
    /// Verdict label -> count (only verdicts that occurred).
    pub counts: BTreeMap<&'static str, usize>,
    /// Most severe verdict in the set; `Untested` for an empty set.
    pub worst: Verdict,
    /// Non-native rasters, most severe first, capped at
    /// [`MAX_OFFENDERS`] so summaries stay UI-sized.
    pub offenders: Vec<Offender>,
}

/// Cap on [`ValidationSummary::offenders`] (the counts keep the total).
pub const MAX_OFFENDERS: usize = 8;

impl ValidationSummary {
    /// True when every raster is engine-native for the target.
    pub fn all_native(&self) -> bool {
        self.textures > 0 && self.worst == Verdict::Native
    }

    /// True when nothing is outright incompatible (no `Unsupported`).
    pub fn has_incompatible(&self) -> bool {
        self.counts.contains_key(Verdict::Unsupported.label())
    }

    /// Count of rasters the target cannot consume as-is.
    pub fn incompatible_count(&self) -> usize {
        self.counts
            .get(Verdict::Unsupported.label())
            .copied()
            .unwrap_or(0)
    }

    /// Rasters whose compatibility is unknown (no evidence either way).
    pub fn unknown_count(&self) -> usize {
        self.counts
            .get(Verdict::Untested.label())
            .copied()
            .unwrap_or(0)
    }
}

impl Verdict {
    /// True when the raster needs no action for this target.
    pub fn is_native(self) -> bool {
        self == Verdict::Native
    }
}

/// Classify a set of named rasters against one target game and
/// aggregate the result. This is the validator primitive: an import,
/// an entry, or a whole archive reduced to "how much of it does this
/// engine accept, and what needs attention".
pub fn validate_rasters<'a, I>(target: &GameProfile, rasters: I) -> ValidationSummary
where
    I: IntoIterator<Item = (&'a str, &'a RasterProfile)>,
{
    let mut summary = ValidationSummary {
        game_id: target.id,
        display: target.display,
        textures: 0,
        counts: BTreeMap::new(),
        worst: Verdict::Untested,
        offenders: Vec::new(),
    };
    let mut worst: Option<Verdict> = None;
    for (name, profile) in rasters {
        let report = classify(target, profile);
        summary.textures += 1;
        *summary
            .counts
            .entry(report.verdict.label())
            .or_default() += 1;
        worst = Some(match worst {
            Some(previous) => previous.max(report.verdict),
            None => report.verdict,
        });
        if report.verdict != Verdict::Native {
            summary.offenders.push(Offender {
                name: name.to_string(),
                verdict: report.verdict,
                note: report.note,
            });
        }
    }
    if let Some(worst) = worst {
        summary.worst = worst;
    }
    // Most severe first; stable enough for UI listing.
    summary
        .offenders
        .sort_by_key(|offender| std::cmp::Reverse(offender.verdict));
    summary.offenders.truncate(MAX_OFFENDERS);
    summary
}

/// One texture's compatibility report against one target game.
#[derive(Clone, Debug)]
pub struct VerdictReport {
    pub game_id: &'static str,
    pub verdict: Verdict,
    pub evidence: Evidence,
    pub note: String,
}

/// Classify a texture raster against a target game.
pub fn classify(game: &GameProfile, profile: &RasterProfile) -> VerdictReport {
    // Bully has no RenderWare texture path at all: its assets are
    // Gamebryo NIF/NFT.
    if game.id == BULLY.id {
        return VerdictReport {
            game_id: game.id,
            verdict: Verdict::Unsupported,
            evidence: Evidence::Docs,
            note: "Bully assets are Gamebryo NIF/NFT, not RenderWare natives".to_string(),
        };
    }

    let (verdict, evidence, mut note) = format_verdict(game, profile);

    // A platform the target dialect doesn't accept needs a container
    // rewrite first. That rewrite is lossless (pixels unchanged), but it
    // never *lowers* the severity of the format verdict. The evidence
    // stays the format's: the format table governs the outcome.
    if !game.platforms.contains(&profile.platform_id) {
        let platform_note = format!(
            "platform-{} raster in a {} archive needs a platform/version rewrite",
            profile.platform_id, game.display
        );
        let verdict = verdict.combine(Verdict::ConvertibleLossless);
        note = if note.is_empty() {
            platform_note
        } else {
            format!("{note}; {platform_note}")
        };
        return VerdictReport {
            game_id: game.id,
            verdict,
            evidence,
            note,
        };
    }

    VerdictReport {
        game_id: game.id,
        verdict,
        evidence,
        note,
    }
}

fn format_verdict(
    game: &GameProfile,
    profile: &RasterProfile,
) -> (Verdict, Evidence, String) {
    match game.id {
        "sa" => sa_verdict(profile),
        "gta3" | "vc" => iii_vc_verdict(profile),
        _ => (
            Verdict::Untested,
            Evidence::Untested,
            "no profile table yet".to_string(),
        ),
    }
}

/// Classify a Gamebryo `NiPixelData` raster format (nif.xml
/// `PixelFormat` enum values: 0 RGB, 1 RGBA, 2 PAL, 3 PALA, 4 DXT1,
/// 5 DXT3, 6 DXT5) against a target game.
///
/// Bully is the only Gamebryo target; for RenderWare games an NFT
/// raster is simply not their native form. Evidence: the retail
/// World.img scan (35,655 rasters, 2026-09-11).
pub fn classify_nft_format(game: &GameProfile, format: u32) -> VerdictReport {
    if game.id != BULLY.id {
        return VerdictReport {
            game_id: game.id,
            verdict: Verdict::Unsupported,
            evidence: Evidence::Docs,
            note: "Gamebryo NFT rasters are not RenderWare natives".to_string(),
        };
    }
    let (verdict, evidence, note) = match format {
        4 => (
            Verdict::Native,
            Evidence::Retail,
            "retail Bully: 31,714 DXT1 rasters".to_string(),
        ),
        6 => (
            Verdict::Native,
            Evidence::Retail,
            "retail Bully: 3,526 DXT5 rasters".to_string(),
        ),
        0 | 1 => (
            Verdict::Native,
            Evidence::Retail,
            "retail Bully ships raw RGB/RGBA (138/134)".to_string(),
        ),
        2 | 3 => (
            Verdict::Native,
            Evidence::Retail,
            "retail Bully ships paletted rasters (127 PAL + 1 PALA)".to_string(),
        ),
        // The Gamebryo format enum and our decoder both handle DXT3,
        // but retail Bully ships none - keep it non-native.
        5 => (
            Verdict::Supported,
            Evidence::Docs,
            "Gamebryo supports DXT3 but retail Bully ships none".to_string(),
        ),
        _ => (Verdict::Untested, Evidence::Untested, String::new()),
    };
    VerdictReport {
        game_id: game.id,
        verdict,
        evidence,
        note,
    }
}

fn sa_verdict(profile: &RasterProfile) -> (Verdict, Evidence, String) {
    // Retail evidence base: GTA SA PC 1.0 (gta3.img, gta_int.img,
    // player.img, cutscene.img — 32,157 textures measured 2026-09-11,
    // zero parse failures, zero header anomalies). SA is a
    // palette-free dialect: no PAL, no 1555, no DXT5 in retail.
    match profile.logical {
        LogicalFormat::Dxt1 | LogicalFormat::Dxt3 => (
            Verdict::Native,
            Evidence::Retail,
            "retail SA: 28,807 DXT1 + 2,098 DXT3 rasters".to_string(),
        ),
        LogicalFormat::Dxt2 => (Verdict::Untested, Evidence::Retail, String::new()),
        // The old docs claim that SA fences/foliage ship DXT4 is not
        // visible in retail (0/32,157).
        LogicalFormat::Dxt4 => (
            Verdict::Untested,
            Evidence::Retail,
            "retail SA ships no DXT4".to_string(),
        ),
        LogicalFormat::Dxt5 => (
            Verdict::Supported,
            Evidence::Retail,
            "retail SA ships none; DXT5 rides D3D9 support (mod tooling uses it)".to_string(),
        ),
        // SA abandoned palettes entirely - the III/VC native form has
        // no retail precedent for an SA target.
        LogicalFormat::Pal8 | LogicalFormat::Pal4 => (
            Verdict::Untested,
            Evidence::Retail,
            "retail SA ships zero paletted rasters (0/32,157)".to_string(),
        ),
        // Retail SA ships uncompressed 888 as X8R8G8B8 32bpp storage
        // (1,015 rasters, mostly player.img ped skins) and 8888 (237).
        LogicalFormat::R888 if profile.storage_bpp == 4 => {
            (Verdict::Native, Evidence::Retail, String::new())
        }
        LogicalFormat::R888 => (
            Verdict::Untested,
            Evidence::Retail,
            "retail SA never ships true 24-bit 888".to_string(),
        ),
        LogicalFormat::R8888 => (Verdict::Native, Evidence::Retail, String::new()),
        LogicalFormat::R1555 | LogicalFormat::R565 | LogicalFormat::R4444 => (
            Verdict::Untested,
            Evidence::Retail,
            "retail SA ships no 16-bit uncompressed rasters".to_string(),
        ),
        LogicalFormat::R555 | LogicalFormat::Lum8 | LogicalFormat::A8l8 => {
            (Verdict::Untested, Evidence::Untested, String::new())
        }
        LogicalFormat::Unknown => (Verdict::Untested, Evidence::Untested, String::new()),
    }
}

fn iii_vc_verdict(
    profile: &RasterProfile,
) -> (Verdict, Evidence, String) {
    // Retail evidence base: GTA III PC 1.0 (gta3.img + txd.img,
    // 15,372 textures) and GTA VC PC 1.0 (gta3.img, 12,023 textures)
    // measured 2026-09-11, zero parse failures and zero header
    // anomalies.
    match profile.logical {
        // PAL is the III world-texture form (96.5% PAL8); VC barely
        // uses it (27 rasters) in favor of 565/4444.
        LogicalFormat::Pal8 | LogicalFormat::Pal4 => (
            Verdict::Native,
            Evidence::Retail,
            "retail III: 96.5% PAL8; retail VC: 27 rasters - accepted but rare".to_string(),
        ),
        // III and VC ship zero compressed rasters; DXT1 rides on D3D8
        // hardware support but is not either game's data dialect.
        LogicalFormat::Dxt1 => (
            Verdict::Supported,
            Evidence::Retail,
            "retail III+VC ship no compressed rasters (0/27,395)".to_string(),
        ),
        LogicalFormat::Dxt2 | LogicalFormat::Dxt3 | LogicalFormat::Dxt4 | LogicalFormat::Dxt5 => {
            (
                Verdict::Untested,
                Evidence::Retail,
                "retail III+VC ship none; engine acceptance unmeasured".to_string(),
            )
        }
        // Question 5 answered: retail III stores 888 exclusively as
        // 32-bit X8R8G8B8 (6,806 rasters; the txd.img player/vehicle
        // set is 87% of this class). True 24-bit never ships. VC
        // agrees (1 raster, 32bpp).
        LogicalFormat::R888 if profile.storage_bpp == 4 => {
            (Verdict::Native, Evidence::Retail, String::new())
        }
        LogicalFormat::R888 => (
            Verdict::Untested,
            Evidence::Retail,
            "retail III+VC never ship true 24-bit 888; D3D8 R8G8B8 acceptance unmeasured"
                .to_string(),
        ),
        LogicalFormat::R8888 => (
            Verdict::Native,
            Evidence::Retail,
            "retail III ships 8888 in both archives (1,121 rasters)".to_string(),
        ),
        // VC is the 565/4444 game: 10,682 + 1,149 rasters.
        LogicalFormat::R565 => (
            Verdict::Native,
            Evidence::Retail,
            "retail VC: 565 is the dominant world-texture form".to_string(),
        ),
        LogicalFormat::R1555 | LogicalFormat::R4444 => {
            (Verdict::Native, Evidence::Retail, String::new())
        }
        LogicalFormat::R555 | LogicalFormat::Lum8 | LogicalFormat::A8l8 => {
            (Verdict::Untested, Evidence::Untested, String::new())
        }
        LogicalFormat::Unknown => (Verdict::Untested, Evidence::Untested, String::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::raster::PaletteKind;

    #[test]
    fn bully_nft_formats_classify_against_retail_profile() {
        for (format, expected) in [
            (4_u32, Verdict::Native),  // DXT1
            (6, Verdict::Native),      // DXT5
            (0, Verdict::Native),      // RGB
            (1, Verdict::Native),      // RGBA
            (2, Verdict::Native),      // PAL
            (3, Verdict::Native),      // PALA
            (5, Verdict::Supported),   // DXT3 - engine-supported, retail-absent
            (99, Verdict::Untested),   // unknown enum value
        ] {
            let report = classify_nft_format(&BULLY, format);
            assert_eq!(report.verdict, expected, "format {format}");
            assert_eq!(report.game_id, "bully");
        }
    }

    #[test]
    fn nft_formats_are_unsupported_for_renderware_targets() {
        for game in [&GTA3, &VC, &SA] {
            let report = classify_nft_format(game, 4);
            assert_eq!(report.verdict, Verdict::Unsupported, "{}", game.id);
        }
    }

    fn raster(platform: u32, logical: LogicalFormat) -> RasterProfile {
        RasterProfile {
            platform_id: platform,
            raster_format: 0,
            d3d_format: 0,
            fourcc: None,
            logical,
            storage_bpp: 4,
            width: 64,
            height: 64,
            depth: 32,
            mip_levels: 1,
            palette: PaletteKind::None,
            has_alpha_header: true,
            automipmap: false,
        }
    }

    #[test]
    fn validate_rasters_reports_all_native_sets() {
        let a = raster(9, LogicalFormat::Dxt1);
        let b = raster(9, LogicalFormat::R8888);
        let summary = validate_rasters(&SA, [("a", &a), ("b", &b)]);
        assert_eq!(summary.textures, 2);
        assert!(summary.all_native());
        assert_eq!(summary.worst, Verdict::Native);
        assert!(summary.offenders.is_empty());
        assert_eq!(summary.counts.get("native"), Some(&2));
        assert!(!summary.has_incompatible());
    }

    #[test]
    fn validate_rasters_surfaces_worst_and_offenders() {
        let native = raster(8, LogicalFormat::Pal8);
        let dxt = raster(8, LogicalFormat::Dxt1);
        let summary = validate_rasters(&GTA3, [("tiles", &native), ("prop", &dxt)]);
        assert!(!summary.all_native());
        // DXT1 in III is Supported (retail ships none).
        assert_eq!(summary.worst, Verdict::Supported);
        assert_eq!(summary.offenders.len(), 1);
        assert_eq!(summary.offenders[0].name, "prop");
        assert_eq!(summary.offenders[0].verdict, Verdict::Supported);
        assert_eq!(summary.incompatible_count(), 0);
    }

    #[test]
    fn validate_rasters_flags_incompatible_and_unknown() {
        // A RenderWare raster against Bully is outright unsupported.
        let rw = raster(8, LogicalFormat::Pal8);
        let summary = validate_rasters(&BULLY, [("tiles", &rw)]);
        assert!(summary.has_incompatible());
        assert_eq!(summary.incompatible_count(), 1);
        assert_eq!(summary.worst, Verdict::Unsupported);

        // 555 has no retail evidence for SA: unknown, not incompatible.
        let odd = raster(9, LogicalFormat::R555);
        let summary = validate_rasters(&SA, [("odd", &odd)]);
        assert_eq!(summary.unknown_count(), 1);
        assert!(!summary.has_incompatible());
        assert_eq!(summary.worst, Verdict::Untested);
    }

    #[test]
    fn validate_rasters_marks_platform_rewrites_lossless() {
        // A platform-9 paletted raster in a III archive: format is
        // native but the container needs a rewrite.
        let p9 = raster(9, LogicalFormat::Pal8);
        let summary = validate_rasters(&GTA3, [("tiles", &p9)]);
        assert_eq!(summary.worst, Verdict::ConvertibleLossless);
        assert_eq!(
            summary.counts.get(Verdict::ConvertibleLossless.label()),
            Some(&1)
        );
    }

    #[test]
    fn validate_rasters_caps_offenders_but_counts_all() {
        let bad = raster(8, LogicalFormat::Dxt1);
        let names: Vec<&str> = (0..10).map(|_| "prop").collect();
        let items: Vec<(&str, &RasterProfile)> =
            names.iter().map(|n| (*n, &bad)).collect();
        let summary = validate_rasters(&GTA3, items);
        assert_eq!(summary.textures, 10);
        assert_eq!(summary.offenders.len(), MAX_OFFENDERS);
        assert_eq!(summary.counts.get("supported"), Some(&10));
    }

    #[test]
    fn validate_rasters_handles_empty_sets() {
        let summary = validate_rasters(
            &SA,
            std::iter::empty::<(&str, &RasterProfile)>(),
        );
        assert_eq!(summary.textures, 0);
        assert_eq!(summary.worst, Verdict::Untested);
        assert!(!summary.all_native());
    }
}
