//! Pre-save validation: turn a [`ScanReport`] plus the archive's target
//! into the "should this save be reviewed?" answer.
//!
//! The save itself stays verbatim (design principle: no silent
//! re-encoding). This module only decides what the user should see
//! before writing.

use crate::archive::ArchiveInfo;
use crate::parser::ImgVersion;

use super::games::profile_by_id;
use super::raster::Severity;
use super::scan::ScanReport;

/// Everything the pre-save dialog needs to report.
#[derive(Debug, Clone, Default)]
pub struct SaveIssue {
    /// Textures the target cannot consume.
    pub incompatible: usize,
    /// Textures with no evidence either way.
    pub unknown: usize,
    /// Textures needing a lossless rewrite (shown, not gating-strong).
    pub convertible: usize,
    /// Native + supported count, for reassurance.
    pub fine: usize,
    /// Container does not match the target's convention.
    pub container_note: Option<String>,
    /// Error-severity anomalies (internally broken rasters).
    pub anomalies: Vec<(String, usize, String)>,
    /// Warn-severity anomaly count, informational.
    pub warnings: usize,
    pub textures: usize,
    pub entry_count: usize,
}

impl SaveIssue {
    /// Whether the save deserves the review dialog. Unknown and
    /// incompatible verdicts and broken headers gate; lossless
    /// conversions and warnings are reported but do not gate.
    pub fn needs_review(&self) -> bool {
        self.incompatible > 0 || self.unknown > 0 || self.container_note.is_some()
            || !self.anomalies.is_empty()
    }
}

/// Evaluate a scan report against the archive it came from.
pub fn evaluate_save(report: &ScanReport, archive: &ArchiveInfo) -> SaveIssue {
    let mut issue = SaveIssue {
        textures: report.textures,
        entry_count: report.entry_count,
        ..SaveIssue::default()
    };

    if let Some(target_id) = archive.target_game
        && let Some(counts) = report.verdicts.get(target_id)
    {
        for (label, count) in counts {
            match *label {
                "unsupported" => issue.incompatible += count,
                "untested" => issue.unknown += count,
                "convertible (lossless)" | "convertible (lossy)" => issue.convertible += count,
                _ => issue.fine += count,
            }
        }
        issue.container_note = container_note(archive.version, target_id);
    }

    for (code, count) in &report.anomaly_counts {
        match report.anomaly_severity.get(code) {
            Some(Severity::Error) => {
                let example = report
                    .anomaly_examples
                    .get(code)
                    .and_then(|examples| examples.first())
                    .cloned()
                    .unwrap_or_default();
                issue
                    .anomalies
                    .push((code.to_string(), *count, example));
            }
            Some(Severity::Warn) => issue.warnings += count,
            _ => {}
        }
    }
    issue.anomalies.sort_by_key(|(_, count, _)| std::cmp::Reverse(*count));

    issue
}

/// Container/target convention mismatches. Retail conventions: III, VC
/// and Bully ship IMG v1 (a separate `.dir`); SA ships the v2 container
/// with the directory embedded. GTA III and VC cannot see v2 archives at
/// all; SA can read v1 archives when they are listed in `gta.dat`, but
/// v2 is the conventional target.
fn container_note(version: ImgVersion, target_id: &str) -> Option<String> {
    let target = profile_by_id(target_id)?.display;
    match (version, target_id) {
        (ImgVersion::Two, "gta3" | "vc") => Some(format!(
            "IMG v2 container, but {target} expects IMG v1 - the game will not see these files."
        )),
        (ImgVersion::Two, "bully") => Some(format!(
            "IMG v2 container, but {target} expects IMG v1 - the game will not see these files."
        )),
        (ImgVersion::One, "sa") => Some(
            "IMG v1 container; retail San Andreas ships IMG v2 (v1 loads only when listed in gta.dat)."
                .to_string(),
        ),
        (ImgVersion::Xbox360, _) => Some(format!(
            "Xbox 360 packing; {target} expects a PC container."
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::scan::ScanReport;

    fn archive(target: Option<&'static str>, version: ImgVersion) -> ArchiveInfo {
        let mut archive = ArchiveInfo::new("gta3", false, version);
        archive.target_game = target;
        archive
    }

    fn report_with(
        target: &'static str,
        native: usize,
        unsupported: usize,
        untested: usize,
    ) -> ScanReport {
        let mut report = ScanReport {
            textures: native + unsupported + untested,
            entry_count: 3,
            ..ScanReport::default()
        };
        let counts = report.verdicts.entry(target).or_default();
        counts.insert("native", native);
        counts.insert("unsupported", unsupported);
        counts.insert("untested", untested);
        report
    }

    #[test]
    fn clean_report_needs_no_review() {
        let report = report_with("gta3", 100, 0, 0);
        let issue = evaluate_save(&report, &archive(Some("gta3"), ImgVersion::One));
        assert!(!issue.needs_review());
        assert_eq!(issue.fine, 100);
    }

    #[test]
    fn incompatible_and_unknown_gate_but_lossless_does_not() {
        let report = report_with("gta3", 10, 2, 1);
        let issue = evaluate_save(&report, &archive(Some("gta3"), ImgVersion::One));
        assert!(issue.needs_review());
        assert_eq!(issue.incompatible, 2);
        assert_eq!(issue.unknown, 1);

        let mut report = report_with("gta3", 10, 0, 0);
        report
            .verdicts
            .get_mut("gta3")
            .unwrap()
            .insert("convertible (lossless)", 5);
        let issue = evaluate_save(&report, &archive(Some("gta3"), ImgVersion::One));
        assert!(!issue.needs_review(), "lossless conversions do not gate");
        assert_eq!(issue.convertible, 5);
    }

    #[test]
    fn container_mismatch_gates_with_the_right_note() {
        // VER2 archive for a III target: the game cannot see it.
        let report = report_with("gta3", 100, 0, 0);
        let issue = evaluate_save(&report, &archive(Some("gta3"), ImgVersion::Two));
        assert!(issue.needs_review());
        assert!(issue.container_note.as_deref().unwrap().contains("IMG v1"));

        // v1 for an SA target: conventional mismatch, informational.
        let report = report_with("sa", 100, 0, 0);
        let issue = evaluate_save(&report, &archive(Some("sa"), ImgVersion::One));
        assert!(issue.container_note.is_some());

        // Matching containers stay silent.
        let issue = evaluate_save(
            &report_with("sa", 100, 0, 0),
            &archive(Some("sa"), ImgVersion::Two),
        );
        assert!(issue.container_note.is_none());
    }

    #[test]
    fn error_anomalies_gate_and_carry_an_example() {
        let mut report = report_with("gta3", 100, 0, 0);
        report.anomaly_counts.insert("CONTRADICTORY_DXT_HEADER", 3);
        report
            .anomaly_severity
            .insert("CONTRADICTORY_DXT_HEADER", Severity::Error);
        report
            .anomaly_examples
            .entry("CONTRADICTORY_DXT_HEADER")
            .or_default()
            .push("bad.txd: DXT3 fourcc with 888 header".to_string());
        report.anomaly_counts.insert("NFT_NON_POT", 5);
        report.anomaly_severity.insert("NFT_NON_POT", Severity::Warn);

        let issue = evaluate_save(&report, &archive(Some("gta3"), ImgVersion::One));
        assert!(issue.needs_review());
        assert_eq!(issue.anomalies.len(), 1);
        assert_eq!(issue.anomalies[0].1, 3);
        assert!(issue.anomalies[0].2.contains("bad.txd"));
        assert_eq!(issue.warnings, 5);
    }

    #[test]
    fn untargeted_archive_is_never_reviewed() {
        let report = report_with("gta3", 0, 100, 100);
        let issue = evaluate_save(&report, &archive(None, ImgVersion::One));
        assert!(
            !issue.needs_review(),
            "no target means no dialect judgement"
        );
    }
}
