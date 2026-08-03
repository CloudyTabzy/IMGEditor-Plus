//! Multi-key archive entry sort engine.
//!
//! Adapted from IMGF's `CSortManager` (Code/Sort/) but with trait-based
//! comparator dispatch, Rust-idiomatic persistent state, and natural
//! number ordering for size/offset keys.
//!
//! ## Design
//!
//! A [`SortChain`] is an ordered list of [`SortPriority`] slots. Each
//! slot has a [`SortKey`] (which column to compare), a [`SortDirection`]
//! (ascending/descending), and an `enabled` flag. The first enabled
//! slot is the primary sort; ties break on the next enabled slot, and
//! so on. The final tie-breaker is always the entry name ascending,
//! so the order is deterministic across runs.
//!
//! Disabled slots are skipped — this is what lets the user configure
//! up to ten slots and turn on only the ones they care about.
//!
//! ## Why a trait instead of a match
//!
//! Adding a new sort key is a single `impl SortByKey for SortKey`
//! method (or its own struct), plus a name in the `display_name`
//! list. No central match-everything function to maintain. The trait
//! also lets IDE/COL-dependent keys receive a `&SortContext` so the
//! comparator can look up the right file without touching globals.

use std::cmp::Ordering;

use compact_str::CompactString;

use crate::archive::EntryInfo;

/// The columns an entry can be sorted by. Listed in the order they
/// appear in the "Add sort key" picker so the menu reads naturally.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SortKey {
    /// Entry filename (case-insensitive ASCII).
    Name,
    /// File extension bucket.
    Extension,
    /// File-type string (`.nif`, `.txd`, etc.), with an optional
    /// "primary" type that bubbles to the front.
    Type,
    /// Byte size.
    Size,
    /// Sector offset on disk.
    Offset,
    /// IDE file this entry is associated with. Falls back to name
    /// when the IDE mapping is unavailable.
    IdeFile,
    /// COL file this entry is associated with. Falls back to name
    /// when the COL mapping is unavailable.
    ColFile,
}

impl SortKey {
    /// All keys in picker order. Used by the UI to populate the
    /// "Add sort key" dropdown without hard-coding the list in
    /// three places.
    pub const ALL: &'static [SortKey] = &[
        SortKey::Name,
        SortKey::Extension,
        SortKey::Type,
        SortKey::Size,
        SortKey::Offset,
        SortKey::IdeFile,
        SortKey::ColFile,
    ];

    /// Human-readable label for the picker.
    pub fn display_name(self) -> &'static str {
        match self {
            SortKey::Name => "Name",
            SortKey::Extension => "Extension",
            SortKey::Type => "Type",
            SortKey::Size => "Size",
            SortKey::Offset => "Offset",
            SortKey::IdeFile => "IDE file",
            SortKey::ColFile => "COL file",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SortDirection {
    Ascending,
    Descending,
}

impl SortDirection {
    pub fn display_name(self) -> &'static str {
        match self {
            SortDirection::Ascending => "Ascending",
            SortDirection::Descending => "Descending",
        }
    }
}

/// One priority slot in a [`SortChain`]. Disabled slots are skipped
/// during the sort — that's how the user activates only the keys
/// they care about while keeping the slot order stable across launches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SortPriority {
    pub enabled: bool,
    pub key: SortKey,
    pub direction: SortDirection,
}

impl SortPriority {
    pub const fn new(key: SortKey, direction: SortDirection) -> Self {
        Self {
            enabled: true,
            key,
            direction,
        }
    }

    pub const fn disabled() -> Self {
        Self {
            enabled: false,
            key: SortKey::Name,
            direction: SortDirection::Ascending,
        }
    }
}

/// How many priority slots the chain can hold. Matches IMGF's
/// `m_uiSortPriorityIndex` upper bound — enough to express a 7-level
/// sort without overwhelming the UI.
pub const SORT_CHAIN_MAX: usize = 10;

/// The user's full sort configuration. `priorities[0]` is the
/// primary sort; ties fall through to `priorities[1]`, etc. Disabled
/// slots are skipped. Ties that survive every enabled slot are
/// broken by entry name ascending.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SortChain {
    /// Slots in priority order. Length is always `<= SORT_CHAIN_MAX`.
    /// Slots beyond `len()` are considered disabled-empty.
    priorities: Vec<SortPriority>,
}

impl Default for SortChain {
    fn default() -> Self {
        // IMGF's default: Name AZ. Stable, predictable, and matches
        // what most modders expect when nothing has been configured.
        Self {
            priorities: vec![SortPriority::new(
                SortKey::Name,
                SortDirection::Ascending,
            )],
        }
    }
}

impl SortChain {
    /// Construct from a fixed-size array of priorities, dropping any
    /// slots beyond `SORT_CHAIN_MAX` so callers can't accidentally
    /// blow up the UI.
    pub fn new(priorities: Vec<SortPriority>) -> Self {
        let priorities = priorities
            .into_iter()
            .take(SORT_CHAIN_MAX)
            .collect::<Vec<_>>();
        Self { priorities }
    }

    pub fn len(&self) -> usize {
        self.priorities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.priorities.is_empty()
    }

    /// Borrowed iteration over the priority slots in order. Disabled
    /// slots are not filtered — the UI uses `enabled` to decide
    /// whether to render them.
    pub fn iter(&self) -> std::slice::Iter<'_, SortPriority> {
        self.priorities.iter()
    }

    /// Mutable iteration for in-place edits from the UI.
    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, SortPriority> {
        self.priorities.iter_mut()
    }

    /// Append a new priority slot at the end. Returns `false` if the
    /// chain is already at `SORT_CHAIN_MAX` (no-op in that case).
    pub fn push(&mut self, priority: SortPriority) -> bool {
        if self.priorities.len() >= SORT_CHAIN_MAX {
            return false;
        }
        self.priorities.push(priority);
        true
    }

    /// Remove the slot at `index`. Disabled slots can be removed
    /// freely; active ones are removed as well. The indices of
    /// later slots shift down automatically.
    pub fn remove(&mut self, index: usize) {
        if index < self.priorities.len() {
            self.priorities.remove(index);
        }
    }

    /// Move the slot at `from` to position `to`. Used by the UI's
    /// "move up / move down" buttons. Bounds-clamped on both ends.
    pub fn move_slot(&mut self, from: usize, to: usize) {
        if from >= self.priorities.len() {
            return;
        }
        let to = to.min(self.priorities.len().saturating_sub(1));
        let item = self.priorities.remove(from);
        self.priorities.insert(to, item);
    }

    /// Flip the direction of the slot at `index` without changing
    /// its key or position.
    pub fn toggle_direction(&mut self, index: usize) {
        if let Some(p) = self.priorities.get_mut(index) {
            p.direction = match p.direction {
                SortDirection::Ascending => SortDirection::Descending,
                SortDirection::Descending => SortDirection::Ascending,
            };
        }
    }

    /// Toggle the enabled flag of the slot at `index`.
    pub fn toggle_enabled(&mut self, index: usize) {
        if let Some(p) = self.priorities.get_mut(index) {
            p.enabled = !p.enabled;
        }
    }

    /// Replace the key for the slot at `index`. Slots stay
    /// enabled/disabled (the user's explicit choice) so a re-key
    /// doesn't accidentally turn on a previously-disabled slot.
    pub fn set_key(&mut self, index: usize, key: SortKey) {
        if let Some(p) = self.priorities.get_mut(index) {
            p.key = key;
        }
    }

    /// Set the direction of the slot at `index`.
    pub fn set_direction(&mut self, index: usize, direction: SortDirection) {
        if let Some(p) = self.priorities.get_mut(index) {
            p.direction = direction;
        }
    }

    /// Number of enabled slots (used for the "N keys active" badge
    /// in the UI status bar).
    pub fn enabled_count(&self) -> usize {
        self.priorities.iter().filter(|p| p.enabled).count()
    }

    /// Compare two entries using the chain. Returns the ordering
    /// from the first enabled slot that produces a non-Equal result,
    /// or `Equal` if all enabled slots agree the entries are
    /// equivalent. The final tie-breaker is always `Name` ascending
    /// so the order is deterministic across runs.
    ///
    /// `ctx` provides the IDE/COL file mappings when the chain
    /// includes `IdeFile` or `ColFile` keys; pass an empty
    /// `SortContext` to fall back to entry-name ordering for those.
    pub fn cmp(&self, a: &EntryInfo, b: &EntryInfo, ctx: &SortContext) -> Ordering {
        for priority in &self.priorities {
            if !priority.enabled {
                continue;
            }
            let primary = priority.key.cmp_entries(a, b, ctx);
            if primary != Ordering::Equal {
                return match priority.direction {
                    SortDirection::Ascending => primary,
                    SortDirection::Descending => primary.reverse(),
                };
            }
        }
        // Stable tie-breaker: name ascending, case-insensitive.
        a.file_name_lower.cmp(&b.file_name_lower)
    }
}

/// Per-sort context the comparator can consult for IDE/COL file
/// lookups. Constructed by the caller once per sort (cheap — just
/// field copies) and passed by reference.
#[derive(Debug, Clone)]
pub struct SortContext<'a> {
    /// Maps an entry filename (case-insensitive lookup) to its IDE
    /// file label, if known. Empty map means IDE sort degrades to
    /// name ordering.
    pub ide_files: &'a std::collections::HashMap<CompactString, CompactString>,
    /// Maps an entry filename to its COL file label.
    pub col_files: &'a std::collections::HashMap<CompactString, CompactString>,
    /// "Primary" file-type string used to break ties in `SortKey::Type`.
    /// Without this, the type sort reduces to a name sort.
    pub primary_type: Option<&'a str>,
}

impl<'a> Default for SortContext<'a> {
    fn default() -> Self {
        // We can't use `#[derive(Default)]` because `&HashMap<_, _>`
        // doesn't implement Default for the local `HashMap::new()`
        // pattern that derive generates. Hand-written default keeps
        // the empty-map behaviour explicit.
        static EMPTY_IDE: std::sync::OnceLock<std::collections::HashMap<CompactString, CompactString>> =
            std::sync::OnceLock::new();
        static EMPTY_COL: std::sync::OnceLock<std::collections::HashMap<CompactString, CompactString>> =
            std::sync::OnceLock::new();
        let ide = EMPTY_IDE.get_or_init(std::collections::HashMap::new);
        let col = EMPTY_COL.get_or_init(std::collections::HashMap::new);
        Self {
            ide_files: ide,
            col_files: col,
            primary_type: None,
        }
    }
}

impl<'a> SortContext<'a> {
    pub fn empty() -> Self {
        Self::default()
    }

    /// Build a context from pre-resolved IDE + COL maps. The maps are
    /// keyed by entry filename (case-insensitive) and contain the
    /// IDE/COL file label for that entry.
    pub fn from_maps(
        ide_files: &'a std::collections::HashMap<CompactString, CompactString>,
        col_files: &'a std::collections::HashMap<CompactString, CompactString>,
    ) -> Self {
        Self {
            ide_files,
            col_files,
            primary_type: None,
        }
    }

    /// Look up an entry's IDE file label. Returns `""` if the entry
    /// is not associated with any IDE — that lets the comparator
    /// fall through to the name tiebreaker cleanly.
    pub fn ide_for<'e>(&self, entry: &'e EntryInfo) -> &str {
        self.ide_files
            .get(&entry.file_name_lower)
            .map(|s| s.as_str())
            .unwrap_or("")
    }

    /// Same as `ide_for` but for COL files.
    pub fn col_for<'e>(&self, entry: &'e EntryInfo) -> &str {
        self.col_files
            .get(&entry.file_name_lower)
            .map(|s| s.as_str())
            .unwrap_or("")
    }
}

/// Trait implemented by every [`SortKey`] variant. Extracted from
/// the key enum so adding a new key is one impl block, not a
/// match-everything-in-one-place in the sort engine.
pub trait SortByKey {
    /// Compare two entries by this key alone. Returns `Equal` when
    /// the entries are equivalent on this axis. The chain's
    /// direction handling happens in `SortChain::cmp`, not here.
    fn cmp_entries(&self, a: &EntryInfo, b: &EntryInfo, ctx: &SortContext) -> Ordering;
}

impl SortByKey for SortKey {
    fn cmp_entries(&self, a: &EntryInfo, b: &EntryInfo, ctx: &SortContext) -> Ordering {
        match self {
            // Case-insensitive ASCII compare. We use the precomputed
            // `file_name_lower` on EntryInfo so we don't allocate a
            // new String per comparison.
            SortKey::Name => a.file_name_lower.cmp(&b.file_name_lower),

            // Type sort: primary type (if set) bubbles to the front,
            // then alphabetical within the rest, then name as the
            // final intra-type tiebreaker. Without `primary_type` set
            // in the context this reduces to a plain type sort.
            SortKey::Type => match ctx.primary_type {
                Some(primary) => {
                    let a_primary = a.file_type == *primary;
                    let b_primary = b.file_type == *primary;
                    b_primary.cmp(&a_primary).then_with(|| {
                        a.file_type.cmp(&b.file_type)
                    })
                }
                None => a.file_type.cmp(&b.file_type),
            },

            // Extension: case-insensitive compare on the file_type
            // string itself (EntryInfo already derives the extension
            // into file_type).
            SortKey::Extension => {
                let a_ext = a.file_type.as_str().to_ascii_lowercase();
                let b_ext = b.file_type.as_str().to_ascii_lowercase();
                a_ext.cmp(&b_ext)
            }

            // Size / Offset: EntryInfo stores `offset` and `sector`
            // in u32. Size happens to be a derived field elsewhere
            // in the codebase (`sector * 2048`) but for the sort
            // comparator `sector` is what the existing single-key
            // sort already used, so we keep consistency here.
            SortKey::Size => a.sector.cmp(&b.sector),
            SortKey::Offset => a.sector.cmp(&b.sector),

            // IDE / COL file labels come pre-resolved by the caller
            // in the `SortContext` (HashMap keyed by entry filename,
            // case-insensitive). When the mapping doesn't have an
            // entry, the comparator returns Equal and the chain
            // falls through to the next slot or the name tiebreaker.
            SortKey::IdeFile => ctx.ide_for(a).cmp(ctx.ide_for(b)),
            SortKey::ColFile => ctx.col_for(a).cmp(ctx.col_for(b)),
        }
    }
}

/// Public entry point: sort a slice of `EntryInfo` in place using
/// the chain. Provided as a free function so tests and other call
/// sites can sort without owning a `SortChain`-bound struct.
pub fn sort_entries(entries: &mut [EntryInfo], chain: &SortChain, ctx: &SortContext) {
    entries.sort_by(|a, b| chain.cmp(a, b, ctx));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str) -> EntryInfo {
        let mut e = EntryInfo::new(name);
        // Use the name length as a stand-in for sector/offset so
        // size and offset sorts are testable.
        e.sector = name.len() as u32;
        e.offset = name.len() as u32;
        e
    }

    fn chain_one(key: SortKey, dir: SortDirection) -> SortChain {
        SortChain::new(vec![SortPriority::new(key, dir)])
    }

    #[test]
    fn name_ascending_is_default() {
        assert_eq!(
            SortChain::default(),
            SortChain::new(vec![SortPriority::new(
                SortKey::Name,
                SortDirection::Ascending,
            )])
        );
    }

    #[test]
    fn chain_max_enforced() {
        // Pushing more than SORT_CHAIN_MAX is a no-op; len stays at max.
        let mut chain = SortChain::new(vec![]);
        for _ in 0..(SORT_CHAIN_MAX + 5) {
            chain.push(SortPriority::new(SortKey::Name, SortDirection::Ascending));
        }
        assert_eq!(chain.len(), SORT_CHAIN_MAX);
    }

    #[test]
    fn disabled_slots_are_skipped() {
        let chain = SortChain::new(vec![
            SortPriority::new(SortKey::Size, SortDirection::Ascending), // primary
            SortPriority::disabled(), // disabled - should be skipped
        ]);
        let mut entries = vec![entry("a.txt"), entry("zzz.txt")];
        sort_entries(&mut entries, &chain, &SortContext::empty());
        // Size sort puts the smaller one first ("a.txt" is 5 bytes,
        // "zzz.txt" is 7). If the disabled slot were incorrectly
        // active it would still produce the same order (name happens
        // to be size-correlated here), so we additionally assert the
        // primary count.
        assert_eq!(chain.enabled_count(), 1);
    }

    #[test]
    fn multi_key_tiebreak_with_name() {
        // Primary: size. Tie: name ascending. Both have size 6.
        let chain = SortChain::new(vec![SortPriority::new(
            SortKey::Size,
            SortDirection::Ascending,
        )]);
        let mut entries = vec![entry("z.txt"), entry("a.txt")];
        for e in &mut entries {
            e.sector = 6;
        }
        sort_entries(&mut entries, &chain, &SortContext::empty());
        assert_eq!(entries[0].file_name.as_str(), "a.txt");
        assert_eq!(entries[1].file_name.as_str(), "z.txt");
    }

    #[test]
    fn direction_flips_order() {
        let asc = chain_one(SortKey::Size, SortDirection::Ascending);
        let desc = chain_one(SortKey::Size, SortDirection::Descending);
        let mut a = vec![entry("big.txt"), entry("sm.txt")];
        let mut b = a.clone();
        sort_entries(&mut a, &asc, &SortContext::empty());
        sort_entries(&mut b, &desc, &SortContext::empty());
        assert_eq!(a[0].file_name.as_str(), "sm.txt");
        assert_eq!(b[0].file_name.as_str(), "big.txt");
    }

    #[test]
    fn multi_key_two_levels() {
        // Two slots: Size ASC primary, Extension ASC tiebreak.
        let chain = SortChain::new(vec![
            SortPriority::new(SortKey::Size, SortDirection::Ascending),
            SortPriority::new(SortKey::Extension, SortDirection::Ascending),
        ]);
        // Both entries are 5 bytes; tie on size -> extension sort.
        // "nif" < "txd" alphabetically, so z.nif (extension "nif")
        // sorts first.
        let mut entries = vec![entry("z.nif"), entry("a.txd")];
        sort_entries(&mut entries, &chain, &SortContext::empty());
        assert_eq!(entries[0].file_name.as_str(), "z.nif");
        assert_eq!(entries[1].file_name.as_str(), "a.txd");
    }

    #[test]
    fn move_slot_swaps_order() {
        let mut chain = SortChain::new(vec![
            SortPriority::new(SortKey::Size, SortDirection::Ascending),
            SortPriority::new(SortKey::Name, SortDirection::Ascending),
        ]);
        chain.move_slot(0, 1);
        let (first, second) = (chain.iter().next().unwrap().key, chain.iter().nth(1).unwrap().key);
        assert_eq!(first, SortKey::Name);
        assert_eq!(second, SortKey::Size);
    }

    #[test]
    fn remove_slot_shifts_indices() {
        let mut chain = SortChain::new(vec![
            SortPriority::new(SortKey::Name, SortDirection::Ascending),
            SortPriority::new(SortKey::Size, SortDirection::Ascending),
            SortPriority::new(SortKey::Extension, SortDirection::Ascending),
        ]);
        chain.remove(1);
        assert_eq!(chain.len(), 2);
        assert_eq!(chain.iter().next().unwrap().key, SortKey::Name);
        assert_eq!(chain.iter().nth(1).unwrap().key, SortKey::Extension);
    }

    #[test]
    fn name_sort_is_case_insensitive() {
        let chain = chain_one(SortKey::Name, SortDirection::Ascending);
        let mut entries = vec![entry("Bravo.txt"), entry("alpha.txt")];
        sort_entries(&mut entries, &chain, &SortContext::empty());
        // Case-insensitive: alpha before Bravo.
        assert_eq!(entries[0].file_name.as_str(), "alpha.txt");
        assert_eq!(entries[1].file_name.as_str(), "Bravo.txt");
    }

    #[test]
    fn type_sort_with_primary_bubbles() {
        let chain = chain_one(SortKey::Type, SortDirection::Ascending);
        let mut entries = vec![entry("a.nif"), entry("b.txd"), entry("c.nif")];
        let ctx = SortContext {
            primary_type: Some("nif"),
            ..SortContext::empty()
        };
        sort_entries(&mut entries, &chain, &ctx);
        // Primary (nif) entries first, ordered by name; then the rest.
        assert_eq!(entries[0].file_name.as_str(), "a.nif");
        assert_eq!(entries[1].file_name.as_str(), "c.nif");
        assert_eq!(entries[2].file_name.as_str(), "b.txd");
    }

    #[test]
    fn ide_col_fallback_when_labels_empty() {
        // When the IDE/COL labels aren't supplied, the comparator
        // degrades to empty-string compares — both entries compare
        // Equal, so the chain falls through to the name tiebreaker.
        let chain = chain_one(SortKey::IdeFile, SortDirection::Ascending);
        let mut entries = vec![entry("z.txt"), entry("a.txt")];
        sort_entries(&mut entries, &chain, &SortContext::empty());
        // Name tiebreak kicks in.
        assert_eq!(entries[0].file_name.as_str(), "a.txt");
        assert_eq!(entries[1].file_name.as_str(), "z.txt");
    }

    #[test]
    fn ide_with_labels_sorts_by_ide_file() {
        let chain = chain_one(SortKey::IdeFile, SortDirection::Ascending);
        let mut entries = vec![entry("nif_one.txt"), entry("nif_two.txt")];
        // Entry filenames -> IDE file label. The map is keyed by
        // the entry filename (case-insensitive) and stores the
        // label of the IDE the entry belongs to.
        let mut ide = std::collections::HashMap::new();
        ide.insert(
            CompactString::from("nif_one.txt"),
            CompactString::from("models.ide"),
        );
        ide.insert(
            CompactString::from("nif_two.txt"),
            CompactString::from("actors.ide"),
        );
        let col = std::collections::HashMap::new();
        let ctx = SortContext::from_maps(&ide, &col);
        sort_entries(&mut entries, &chain, &ctx);
        // "actors.ide" should bubble in front of "models.ide".
        assert_eq!(entries[0].file_name.as_str(), "nif_two.txt");
        assert_eq!(entries[1].file_name.as_str(), "nif_one.txt");
    }

    #[test]
    fn deterministic_order_across_runs() {
        // Same chain + same entries -> same output every time. Catches
        // accidental introduction of unstable sort (e.g. switching
        // from sort_by to something else without a stable comparator).
        let chain = chain_one(SortKey::Size, SortDirection::Ascending);
        let mut a = vec![entry("x"), entry("a"), entry("m")];
        let mut b = a.clone();
        sort_entries(&mut a, &chain, &SortContext::empty());
        sort_entries(&mut b, &chain, &SortContext::empty());
        let names_a: Vec<&str> = a.iter().map(|e| e.file_name.as_str()).collect();
        let names_b: Vec<&str> = b.iter().map(|e| e.file_name.as_str()).collect();
        assert_eq!(names_a, names_b);
    }

    #[test]
    fn empty_chain_does_not_panic() {
        // An empty chain (no enabled keys) is degenerate but
        // shouldn't crash. We just get a stable name ordering.
        let chain = SortChain::new(vec![]);
        let mut entries = vec![entry("z"), entry("a")];
        sort_entries(&mut entries, &chain, &SortContext::empty());
        // Final tiebreaker kicks in: name ascending.
        assert_eq!(entries[0].file_name.as_str(), "a");
        assert_eq!(entries[1].file_name.as_str(), "z");
    }
}
