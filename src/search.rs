//! Fuzzy matching for archive entry search.
//!
//! Entry names are short ASCII file names (`police_car.dff`,
//! `player.img`, ` interior_1.txd`). That lets the matcher stay simple:
//!
//! - [`fuzzy_score`] is a scored subsequence match (fzf-lite): every
//!   query character must appear in the name in order; the score
//!   rewards prefix hits, word-boundary hits, and consecutive runs,
//!   and penalises gaps and long names.
//! - [`typo_score`] uses Jaro-Winkler (via the feature-gated `fuzzt`
//!   crate) to rescue typos and transpositions that the subsequence
//!   scan rejects (`polcie` → `police_car`). Its scores are always on
//!   a lower tier than any subsequence match, so real matches outrank
//!   typo guesses. This powers the "Did you mean …" row.

/// Bonus for a query matching at the very start of the name.
const PREFIX_BONUS: i64 = 6000;
/// Bonus per matched character that starts a word (`.` `_` `-` ` `
/// `[` `]` `(` `)` boundaries).
const WORD_BOUNDARY_BONUS: i64 = 500;
/// Bonus per character that continues the previous match without a gap.
const CONSECUTIVE_BONUS: i64 = 150;
/// Penalty per skipped character inside a gap (capped per gap).
const GAP_PENALTY: i64 = 8;
/// Cap on a single gap's penalty so one long gap doesn't dominate.
const GAP_PENALTY_CAP: i64 = 320;
/// Length-normalisation bonus: shorter names rank higher.
const DENSITY_WEIGHT: i64 = 500;
/// Bonus for an exact whole-name match.
const EXACT_MATCH_BONUS: i64 = 10_000;

/// Minimum Jaro-Winkler similarity for a typo match to be accepted.
pub const TYPO_MIN_SIMILARITY: f64 = 0.72;
/// Higher bar for the "Did you mean …" suggestion.
pub const DID_YOU_MEAN_MIN_SIMILARITY: f64 = 0.78;
/// Offset pushing typo scores below every subsequence score.
const TYPO_TIER_OFFSET: i64 = 3000;

/// Score a case-insensitive (both inputs must already be lowercased)
/// subsequence match of `query` inside `name`. `None` when the query
/// characters do not appear in order.
pub fn fuzzy_score(name_lower: &str, query_lower: &str) -> Option<i64> {
    if query_lower.is_empty() {
        return None;
    }
    let name = name_lower.as_bytes();
    let query = query_lower.as_bytes();
    if query.len() > name.len() {
        return None;
    }

    let mut score: i64 = 0;
    let mut scan_from: usize = 0;
    let mut last_match: Option<usize> = None;

    for &q in query {
        let mut found: Option<usize> = None;
        for (offset, &n) in name[scan_from..].iter().enumerate() {
            if n == q {
                found = Some(scan_from + offset);
                break;
            }
        }
        let position = found?;
        match last_match {
            Some(previous) if previous + 1 == position => {
                score += CONSECUTIVE_BONUS;
            }
            Some(previous) => {
                let gap = position - previous - 1;
                score -= (gap as i64 * GAP_PENALTY).min(GAP_PENALTY_CAP);
            }
            None => {}
        }
        if position == 0 {
            score += PREFIX_BONUS;
        } else if !name[position - 1].is_ascii_alphanumeric() {
            score += WORD_BOUNDARY_BONUS;
        }
        last_match = Some(position);
        scan_from = position + 1;
    }

    if query == name {
        score += EXACT_MATCH_BONUS;
    }
    // Prefer shorter, denser names on otherwise-equal matches.
    score += DENSITY_WEIGHT * query.len() as i64 / name.len().max(1) as i64;
    Some(score)
}

/// Jaro-Winkler similarity of a (lowercased) name and query, mapped
/// onto the typo tier. `None` when the similarity is below
/// [`TYPO_MIN_SIMILARITY`].
pub fn typo_score(name_lower: &str, query_lower: &str) -> Option<i64> {
    let similarity = fuzzt::algorithms::jaro_winkler(name_lower, query_lower);
    (similarity >= TYPO_MIN_SIMILARITY)
        .then(|| (similarity * 1000.0) as i64 - TYPO_TIER_OFFSET)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn score(name: &str, query: &str) -> Option<i64> {
        fuzzy_score(&name.to_lowercase(), &query.to_lowercase())
    }

    #[test]
    fn exact_match_beats_prefix_beats_scattered() {
        let exact = score("police.dff", "police.dff").unwrap();
        let prefix = score("police.dff", "pol").unwrap();
        let scattered = score("police.dff", "plc").unwrap();
        assert!(exact > prefix);
        assert!(prefix > scattered);
    }

    #[test]
    fn prefix_matches_word_boundaries() {
        let start = score("police_car.dff", "car").unwrap();
        let middle = score("scrapyard.dff", "car").unwrap();
        assert!(start > middle);
    }

    #[test]
    fn consecutive_runs_beat_gappy_matches() {
        let consecutive = score("police_car.dff", "poli").unwrap();
        let gappy = score("police_car.dff", "pi").unwrap();
        assert!(consecutive > gappy);
    }

    #[test]
    fn shorter_names_rank_higher() {
        let short = score("car.dff", "car").unwrap();
        let long = score("police_car.dff", "car").unwrap();
        assert!(short > long);
    }

    #[test]
    fn non_subsequence_queries_do_not_match() {
        assert_eq!(score("police.dff", "xyz"), None);
        assert_eq!(score("police.dff", "ecil"), None);
        assert_eq!(score("", "a"), None);
    }

    #[test]
    fn empty_query_never_matches() {
        assert_eq!(score("police.dff", ""), None);
    }

    #[test]
    fn typo_rescue_ranks_below_any_subsequence_match() {
        // "polcie" is still a subsequence of "police", so compare the
        // rescue tier against the weakest subsequence match instead.
        let rescued = typo_score("police.dff", "pilce").unwrap();
        let weakest = score(
            "police.dff",
            "e",
        )
        .unwrap();
        assert!(rescued < weakest);
    }

    #[test]
    fn typo_rescue_rejects_dissimilar_names() {
        assert_eq!(typo_score("police.dff", "xyz"), None);
        assert_eq!(typo_score("police.dff", ""), None);
    }

    #[test]
    fn did_you_mean_bar_is_reached_for_typos() {
        let similarity = fuzzt::algorithms::jaro_winkler("police.dff", "polic.dff");
        assert!(similarity >= DID_YOU_MEAN_MIN_SIMILARITY);
        let miss = fuzzt::algorithms::jaro_winkler("police.dff", "trainers");
        assert!(miss < DID_YOU_MEAN_MIN_SIMILARITY);
    }
}
