//! Drag-and-drop between archive tabs.
//!
//! Adapted from IMGF's `CDragListCtrl` + `CDropTarget` (Code/DragDrop/).
//! IMGF's version uses MFC's OLE drag-and-drop, which is Win32-specific
//! and incompatible with Iced. This module implements the same UX
//! without the legacy machinery:
//!
//! ## Rust-flavored extras over IMGF
//!
//! - **`DragState` is a `Copy` value-type** rather than a heap
//!   pointer to a mutable struct. The whole state lives in one
//!   `Option<DragState>` on `App`; no `CDraggableFile*` per-entry
//!   bookkeeping. The compiler enforces correct update via
//!   `Replace` semantics on `Option`.
//! - **Source archive + entry indices are part of the state**, so
//!   we never have to thread them through every event message.
//   The drop event is a single `Message::ArchiveDragReleased` —
//   the source is recovered from `App::drag_state`.
//! - **Cancel via `Drop`** — if the user presses Escape, closes the
//!   window, or otherwise aborts the drag without committing, the
//!   `Option<DragState>` is set to `None` directly. IMGF has to
//!   clean up a `std::vector<CDraggableFile*>` of heap pointers
//!   on every error path; we get it for free.
//! - **No "drop on empty space" drag-over feedback in the
//!   source C++ code** — IMGF's `OnDragOver` is undocumented and
//!   had no spec. Our `on_move` gives a typed `over: Option<usize>`
//!   so the UI can highlight the target tab before release.
//! - **No real "drag image" or cursor follow** — Iced 0.14
//!   doesn't expose that and we don't fake it with a custom
//!   shader. Instead, the source tab gets a subtle "carrying N
//!   entries" badge that updates the same frame the user moves
//!   the mouse, so the affordance is discoverable without
//!   platform-specific DND machinery.

use iced::Point;

/// Maximum number of entries the user can drag in a single
/// operation. 64 is generous — most modders move 1-10 at a time
/// — and small enough to keep the state in a fixed-size array so
/// the type is `Copy` (no heap allocation per drag event).
pub const DRAG_MAX_ENTRIES: usize = 64;

/// One in-flight drag-and-drop operation between archive tabs.
///
/// `Copy` so the value can be inspected anywhere without
/// `&self`/`&mut self` contortions. The set of entry indices is
/// captured at drag-start time and is independent of subsequent
/// `selected_indices` changes — if the user re-selects entries
/// mid-drag, we ignore that and use what was captured.
///
/// No `Eq` derive because `Point` (cursor) contains `f32`, and
/// `f32` doesn't implement `Eq`. `PartialEq` is enough for the
/// "is the drag in progress" check.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DragState {
    /// Index of the source archive in `Editor::archives`.
    pub source: usize,
    /// Indices of the entries to move, into `Editor::archives[source]`.
    /// Fixed-size for `Copy`; entries past `len` are ignored.
    pub entry_indices: [usize; DRAG_MAX_ENTRIES],
    /// Number of valid entries at the start of `entry_indices`.
    pub count: u16,
    /// Index of the archive the cursor is currently over, or
    /// `None` if the cursor is outside the tab strip. Updated
    /// on every `CursorMoved` (the App's drag handler maps
    /// it from `on_enter`/`on_exit` events on the tabs).
    pub hover_target: Option<usize>,
    /// Cursor position in window coordinates, sampled at
    /// `ArchiveDragMoved` time. Used to render the "carrying N
    /// entries" indicator in the right place. `None` if the
    /// dialog hasn't seen a move event yet.
    pub cursor: Option<Point>,
}

impl DragState {
    /// Create a new drag from `source` with the given selection.
    /// The hover target defaults to `None` (the user hasn't moved
    /// the cursor yet) and the cursor defaults to `None`.
    /// Truncates `entry_indices` at `DRAG_MAX_ENTRIES`; callers
    /// that want to move more than 64 entries at once need a
    /// different UI path (the multi-select "Copy all" button is
    /// already the right tool for that).
    pub fn new(source: usize, entry_indices: &[usize]) -> Self {
        let count = entry_indices.len().min(DRAG_MAX_ENTRIES);
        let mut indices = [0usize; DRAG_MAX_ENTRIES];
        indices[..count].copy_from_slice(&entry_indices[..count]);
        Self {
            source,
            entry_indices: indices,
            count: count as u16,
            hover_target: None,
            cursor: None,
        }
    }

    /// How many entries this drag is moving. Used in the
    /// status-bar indicator ("Moving 12 entries…").
    pub fn len(&self) -> usize {
        self.count as usize
    }

    /// Borrow the live entry indices as a slice. The slice length
    /// is the captured `count`, not the full `DRAG_MAX_ENTRIES`.
    pub fn indices(&self) -> &[usize] {
        &self.entry_indices[..self.count as usize]
    }

    /// Is the cursor currently over a valid target archive (i.e. a
    /// different archive than the source)? UI uses this to decide
    /// whether the drop indicator is shown.
    pub fn has_valid_target(&self) -> bool {
        matches!(self.hover_target, Some(t) if t != self.source)
    }
}

/// The "moving N entries" indicator that lives in the source tab
/// during a drag. Used by the tab bar to render a small badge.
pub fn drag_indicator_text(state: &DragState) -> String {
    let n = state.len();
    let suffix = if n == 1 { "entry" } else { "entries" };
    let target_hint = match state.hover_target {
        Some(t) if t != state.source => format!(" → archive #{}", t + 1),
        _ => String::new(),
    };
    format!("↗ {n} {suffix}{target_hint}")
}
#[cfg(test)]
mod tests {
    use super::*;
    use iced::Point;

    #[test]
    fn new_captures_source_and_entries() {
        let state = DragState::new(2, &[0, 1, 2]);
        assert_eq!(state.source, 2);
        assert_eq!(state.count, 3);
        assert_eq!(state.indices(), &[0, 1, 2]);
        assert_eq!(state.hover_target, None);
        assert_eq!(state.cursor, None);
    }

    #[test]
    fn copy_semantics_means_state_is_value_typed() {
        // IMGF's CDragListCtrl owns a std::vector<CDraggableFile*>
        // that mutates across every drag event. We get this for free:
        // updating source / hover_target / cursor requires only a
        // struct literal, the borrow checker is happy.
        let original = DragState::new(0, &[5, 6]);
        let mut updated = original;
        updated.hover_target = Some(2);
        updated.cursor = Some(Point::new(100.0, 50.0));
        // Original is unchanged because DragState is Copy.
        assert_eq!(original.hover_target, None);
        assert_eq!(original.cursor, None);
        // Updated reflects the change.
        assert_eq!(updated.hover_target, Some(2));
        assert_eq!(updated.cursor, Some(Point::new(100.0, 50.0)));
    }

    #[test]
    fn has_valid_target_rejects_same_archive() {
        // Drop on the source archive itself isn't a move - it's a
        // no-op (or a re-select). The UI uses this to suppress the
        // "drop here" indicator.
        let mut state = DragState::new(1, &[0]);
        state.hover_target = Some(1);
        assert!(!state.has_valid_target());
        state.hover_target = Some(2);
        assert!(state.has_valid_target());
        state.hover_target = None;
        assert!(!state.has_valid_target());
    }

    #[test]
    fn count_handles_empty_selection() {
        // Defensive: dragging nothing should be a no-op even if the
        // state somehow ends up in the App. The count is 0 so the
        // status bar shows "Moving 0 entries" and the apply handler
        // would loop zero times.
        let state = DragState::new(0, &[]);
        assert_eq!(state.count, 0);
        assert!(state.indices().is_empty());
    }

    #[test]
    fn new_truncates_at_drag_max_entries() {
        // More entries than DRAG_MAX_ENTRIES — the extra ones are
        // silently dropped. The App's "Copy all" button is the right
        // tool for moving huge selections at once; the drag path is
        // for the common 1-10 case.
        let many: Vec<usize> = (0..100).collect();
        let state = DragState::new(0, &many);
        assert_eq!(state.count as usize, DRAG_MAX_ENTRIES);
        assert_eq!(state.indices().len(), DRAG_MAX_ENTRIES);
    }

    #[test]
    fn indicator_text_singular_and_plural() {
        // IMGF doesn't have a status-bar indicator at all - it
        // tracks the cursor with a real OLE drag image. We just
        // show text, but we get grammar right.
        let mut state = DragState::new(0, &[1]);
        assert_eq!(drag_indicator_text(&state), "↗ 1 entry");
        state.count = 3;
        assert_eq!(drag_indicator_text(&state), "↗ 3 entries");
        // Hovering a valid target adds the "→ archive #N" hint.
        state.hover_target = Some(2);
        assert_eq!(
            drag_indicator_text(&state),
            "↗ 3 entries → archive #3"
        );
        // Hovering the source archive itself is a no-op (drop here
        // doesn't move anything) so we hide the hint.
        state.hover_target = Some(0);
        assert_eq!(drag_indicator_text(&state), "↗ 3 entries");
    }
}
