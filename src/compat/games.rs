//! Per-game dialect profiles and the texture verdict tables.
//!
//! Every verdict carries its evidence class so untested cells stay
//! visible in code (see docs/asset-compatibility-engine.md §3). The
//! tables here are Phase 0 drafts: corpus evidence comes from the
//! modded `gta3.img` forensics, docs evidence from the INU Tools
//! research (docs/research-inu-tools-gta.md); retail verification
//! flips cells from `Docs`/`Untested` to `Retail` as corpora arrive.

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

/// PC SA accepts both platform 8 and 9 natives (docs evidence);
/// what vanilla itself writes is pending retail verification.
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

fn sa_verdict(profile: &RasterProfile) -> (Verdict, Evidence, String) {
    match profile.logical {
        LogicalFormat::Dxt1 | LogicalFormat::Dxt3 | LogicalFormat::Dxt2 => {
            (Verdict::Native, Evidence::Corpus, String::new())
        }
        // SA fences/foliage ship as DXT4 (docs-verified against stock
        // SA TXDs); DXT5 appears in mod tooling.
        LogicalFormat::Dxt4 => (Verdict::Native, Evidence::Docs, String::new()),
        LogicalFormat::Dxt5 => (
            Verdict::Supported,
            Evidence::Docs,
            "DXT5 exists in SA tooling; stock SA ships DXT1/DXT3".to_string(),
        ),
        LogicalFormat::Pal8 | LogicalFormat::Pal4 => (
            Verdict::Native,
            Evidence::Docs,
            "palettes accepted; retail SA palette conventions still to verify".to_string(),
        ),
        // The "888"-as-X8R8G8B8 storage: engine-accepted but 33% larger
        // than the 24-bit logical size.
        LogicalFormat::R888 if profile.storage_bpp == 4 => (
            Verdict::Supported,
            Evidence::Corpus,
            "32-bit X8R8G8B8 storage of a 24-bit format".to_string(),
        ),
        LogicalFormat::R888 => (Verdict::Untested, Evidence::Untested, String::new()),
        LogicalFormat::R8888 => (
            Verdict::Supported,
            Evidence::Corpus,
            "uncompressed 8888: stock SA prefers DXT".to_string(),
        ),
        LogicalFormat::R1555 | LogicalFormat::R565 | LogicalFormat::R4444 => {
            (Verdict::Native, Evidence::Corpus, String::new())
        }
        LogicalFormat::R555 | LogicalFormat::Lum8 | LogicalFormat::A8l8 => {
            (Verdict::Untested, Evidence::Untested, String::new())
        }
        LogicalFormat::Unknown => (Verdict::Untested, Evidence::Untested, String::new()),
    }
}

fn iii_vc_verdict(
    profile: &RasterProfile,
) -> (Verdict, Evidence, String) {
    match profile.logical {
        // III ships paletted rasters (PAL8, BGRA palettes) — the
        // engine-native form for skins and tiles (docs evidence; retail
        // corpus confirmation pending).
        LogicalFormat::Pal8 | LogicalFormat::Pal4 => (
            Verdict::Native,
            Evidence::Docs,
            "III-era engines are the native palette consumers".to_string(),
        ),
        LogicalFormat::Dxt1 => (Verdict::Native, Evidence::Docs, String::new()),
        // DXT3/4/5 in a 3.3/3.4-era archive is plausible but
        // unverified against a retail corpus.
        LogicalFormat::Dxt3 | LogicalFormat::Dxt4 | LogicalFormat::Dxt5 | LogicalFormat::Dxt2 => {
            (Verdict::Untested, Evidence::Untested, String::new())
        }
        // True 24-bit 888 is the D3D8-era storage; the 32-bit X8R8G8B8
        // form (the dwayne class) rewrites losslessly to either.
        LogicalFormat::R888 if profile.storage_bpp == 3 => {
            (Verdict::Native, Evidence::Docs, String::new())
        }
        LogicalFormat::R888 => (
            Verdict::ConvertibleLossless,
            Evidence::Docs,
            "32-bit X8R8G8B8 storage: rewrite to true 24-bit or 8888 — pixels unchanged"
                .to_string(),
        ),
        LogicalFormat::R8888 => (Verdict::Native, Evidence::Docs, String::new()),
        LogicalFormat::R1555 | LogicalFormat::R565 | LogicalFormat::R4444 => {
            (Verdict::Native, Evidence::Docs, String::new())
        }
        LogicalFormat::R555 | LogicalFormat::Lum8 | LogicalFormat::A8l8 => {
            (Verdict::Untested, Evidence::Untested, String::new())
        }
        LogicalFormat::Unknown => (Verdict::Untested, Evidence::Untested, String::new()),
    }
}
