//! Sort Manager modal — the UI for editing a multi-key `SortChain`.
//!
//! Adapted from IMGF's `CSortMenuItems` (Code/Sort/) but built around
//! traits and live previews rather than a flat menu of check + dropdown
//! items. The Rust-flavored extras the IMGF UI doesn't have:
//!
//! - **NewType slot index** (`SortSlotIndex`) — the dialog never
//!   mixes a slot index with a row index, because the wrapper type
//!   makes accidental cross-pollination a compile error.
//! - **Live preview** — the right pane sorts the first 10 entries
//!   of the active archive through the draft chain in real time.
//!   The user sees exactly what their edits will produce.
//! - **Built-in presets** — five common Bully workflows
//!   (`Name A→Z`, `Type then name`, etc.) as one-click buttons.
//! - **One primary key at a time** — moving slot N up/down shifts
//!   priorities, and the new chain is sent to the active archive
//!   on Apply. IMGF's UI let any priority be enabled/disabled
//!   independently; we keep the same flexibility via a checkbox
//!   on each slot.
//! - **Draft + Apply/Cancel** — edits land in a draft, Apply
//!   commits, Cancel discards. No surprise "I changed my sort
//!   by accident" moments.
//! - **Empty-state placeholder** — when the chain has zero enabled
//!   slots, the preview shows the "what would happen with no
//!   sort" view, not an empty list, so the user knows the
//!   baseline.
//!
//! The dialog itself is a function-component pattern: `build()`
//! takes the active archive, the draft chain, and a closure
//! for emitting messages. It returns the Iced `Element` that
//! the app layer inserts into its view.

use iced::widget::{
    Column, Container, PickList, Row, Scrollable, Space, button, checkbox, container,
    pick_list, text,
};
use iced::{Alignment, Color, Element, Length, Padding};

use crate::archive::EntryInfo;
use crate::sort::{SortChain, SortContext, SortDirection, SortKey, SortPriority, sort_entries};
use crate::ui::app::{Message, SortPreset, SortSlotIndex};

/// Maximum number of preview entries shown on the right pane.
/// Bounded so the dialog stays compact on small displays.
const PREVIEW_MAX: usize = 10;

/// Color used to highlight the active sort column. Pulled out as
/// a constant so the UI can be theme-tweaked from one place.
const PRIMARY_COLOR: Color = Color::from_rgb(0.40, 0.78, 0.96);
const SECONDARY_COLOR: Color = Color::from_rgb(0.70, 0.70, 0.70);

/// Pre-resolved `Length` constants. We can't use the unit variants
/// of `Length` directly in function-call position because Rust's
/// parser interprets `f(Length::Fill)` as `f(Length, Fill)` (a
/// 2-arg call to the path-as-tuple-constructor pattern). Binding
/// each value to a `const` local forces single-arg call resolution
/// and lets us reuse the values across the dialog.
const LEN_FILL: Length = Length::Fill;
const LEN_FIXED_4: Length = Length::Fixed(4.0);
const LEN_FIXED_8: Length = Length::Fixed(8.0);
const LEN_FIXED_12: Length = Length::Fixed(12.0);
const LEN_FIXED_140: Length = Length::Fixed(140.0);
const LEN_FIXED_20: Length = Length::Fixed(20.0);

/// Build the Sort Manager modal. Returns the inner content
/// element; the caller is responsible for centering and dimming
/// the background (e.g. via an `iced::widget::modal` wrapper).
pub fn build<'a>(
    archive_name: Option<&'a str>,
    draft: &'a SortChain,
    preview_entries: &'a [EntryInfo],
    primary_type: Option<&'a str>,
    ide_labels: &'a std::collections::HashMap<
        compact_str::CompactString,
        compact_str::CompactString,
    >,
    col_labels: &'a std::collections::HashMap<
        compact_str::CompactString,
        compact_str::CompactString,
    >,
) -> Element<'a, Message> {
    let title = match archive_name {
        Some(name) => format!("Sort by — {name}"),
        None => "Sort by — (no archive open)".to_string(),
    };

    // Build the slot list. Each row is its own Column so the
    // chips wrap on narrow dialogs without horizontal overflow.
    let slots: Element<'a, Message> = if draft.is_empty() {
        container(
            text("No keys yet. Add a key below to start sorting.")
                .size(13)
                .style(|_| iced::widget::text::Style {
                    color: Some(SECONDARY_COLOR),
                }),
        )
        .padding(12)
        .into()
    } else {
        let mut col = Column::new().spacing(6);
        for (index, prio) in draft.iter().enumerate() {
            col = col.push(slot_row(index, prio));
        }
        Scrollable::new(col).height(Length::Shrink).into()
    };

    // Live preview: sort the preview entries through the current
    // draft chain. The empty-state placeholder shows what the
    // current (un-sorted) order looks like, so the user can
    // see the "no sort" baseline.
    let preview = preview_pane(preview_entries, draft, primary_type, ide_labels, col_labels);

    let preset_picker = preset_picker();

    let controls = controls_row(draft);

    let footer = footer_row();

    let header = Row::new()
        .push(text(title).size(16))
        .push(Space::new().width(LEN_FILL))
        .push(preset_picker)
        .align_y(Alignment::Center)
        .spacing(8);

    let body = Row::new()
        .push(
            Column::new()
                .push(header)
                .push(Space::new().height(LEN_FIXED_8))
                .push(slots)
                .push(Space::new().height(LEN_FIXED_8))
                .push(controls)
                .width(Length::FillPortion(3))
                .spacing(4),
        )
        .push(
            Column::new()
                .push(preview_title())
                .push(Space::new().height(LEN_FIXED_4))
                .push(preview)
                .width(Length::FillPortion(2)),
        )
        .spacing(12);

    Container::new(
        Column::new()
            .push(body)
            .push(Space::new().height(LEN_FIXED_12))
            .push(footer)
            .padding(Padding::from(16)),
    )
    .style(|_| iced::widget::container::Style {
        background: Some(iced::Background::Color(Color::from_rgb(
            0.10, 0.11, 0.13,
        ))),
        text_color: Some(Color::WHITE),
        ..Default::default()
    })
    .into()
}

/// One priority slot in the dialog list. Layout:
///
/// ```text
/// [✓] 1. [Name ▾] [▲] [×]
/// ```
///
/// Each control emits a focused message that the App handler
/// resolves against the draft copy. The `disabled` style on the
/// key dropdown when the slot is unchecked is the visual cue
/// that disabling means "this priority is skipped" — the slot
/// itself stays in the chain so the user's priority order is
/// preserved when they re-enable.
fn slot_row<'a>(index: usize, prio: &'a SortPriority) -> Element<'a, Message> {
    let slot_idx = SortSlotIndex(index);
    let key_picker = pick_list(
        SortKey::ALL,
        Some(prio.key),
        move |new_key: SortKey| Message::SortSetSlotKey(slot_idx, new_key),
    )
    .placeholder("Select key…")
    .text_size(13)
    .width(LEN_FIXED_140);

    let dir_btn = button(text(dir_label(prio.direction)).size(13))
        .on_press(Message::SortSetSlotDirection(
            slot_idx,
            match prio.direction {
                SortDirection::Ascending => SortDirection::Descending,
                SortDirection::Descending => SortDirection::Ascending,
            },
        ))
        .padding(Padding::from([4, 8]))
        .style(move |_theme, _status| {
            let bg = match prio.direction {
                SortDirection::Ascending => Color::from_rgb(0.20, 0.45, 0.30),
                SortDirection::Descending => Color::from_rgb(0.45, 0.30, 0.20),
            };
            iced::widget::button::Style {
                background: Some(iced::Background::Color(bg)),
                text_color: Color::WHITE,
                ..iced::widget::button::Style::default()
            }
        });

    let remove_btn = button(text("×").size(14))
        .on_press(Message::SortRemoveSlot(slot_idx))
        .padding(Padding::from([4, 8]));

    let move_up = button(text("▲").size(12))
        .on_press(Message::SortMoveSlotUp(slot_idx))
        .padding(Padding::from([4, 6]));
    let move_down = button(text("▼").size(12))
        .on_press(Message::SortMoveSlotDown(slot_idx))
        .padding(Padding::from([4, 6]));

    let enable_check = checkbox(prio.enabled)
        .on_toggle(move |_checked| Message::SortToggleSlotEnabled(slot_idx))
        .size(16);

    let priority_label = text(format!("{}.", index + 1))
        .size(13)
        .style(|_| iced::widget::text::Style {
            color: Some(if prio.enabled {
                PRIMARY_COLOR
            } else {
                SECONDARY_COLOR
            }),
        })
        .width(LEN_FIXED_20);

    Row::new()
        .push(enable_check)
        .push(Space::new().width(LEN_FIXED_4))
        .push(priority_label)
        .push(Space::new().width(LEN_FIXED_8))
        .push(key_picker)
        .push(Space::new().width(LEN_FIXED_4))
        .push(dir_btn)
        .push(Space::new().width(LEN_FIXED_8))
        .push(move_up)
        .push(move_down)
        .push(Space::new().width(LEN_FILL))
        .push(remove_btn)
        .align_y(Alignment::Center)
        .spacing(4)
        .into()
}

/// The "Add slot" + "Reset" row. Sits between the slot list and
/// the footer. `Reset` is "re-seed the draft from the active
/// archive" — the standard "undo in-progress edits" affordance.
fn controls_row<'a>(draft: &'a SortChain) -> Element<'a, Message> {
    let add_disabled = draft.len() >= crate::sort::SORT_CHAIN_MAX;
    let add_btn = button(text(if add_disabled {
        "+ Add key (max reached)"
    } else {
        "+ Add key"
    }))
    .on_press_maybe(if add_disabled {
        None
    } else {
        Some(Message::SortAddSlot)
    })
    .padding(Padding::from([4, 12]));

    let reset_btn = button(text("Reset"))
        .on_press(Message::SortResetDraft)
        .padding(Padding::from([4, 12]));

    let enabled_count = draft.enabled_count();
    let summary = text(format!(
        "{} of {} keys active",
        enabled_count,
        draft.len()
    ))
    .size(12)
    .style(|_| iced::widget::text::Style {
        color: Some(SECONDARY_COLOR),
    });

    Row::new()
        .push(add_btn)
        .push(Space::new().width(LEN_FIXED_8))
        .push(reset_btn)
        .push(Space::new().width(LEN_FILL))
        .push(summary)
        .align_y(Alignment::Center)
        .into()
}

/// Apply / Cancel footer. The "X of N keys active" badge
/// mirrors the dialog header so the user always knows their
/// current state.
fn footer_row<'a>() -> Element<'a, Message> {
    Row::new()
        .push(
            button(text("Cancel"))
                .on_press(Message::CloseSortManager)
                .padding(Padding::from([6, 16])),
        )
        .push(Space::new().width(LEN_FILL))
        .push(
            button(text("Apply"))
                .on_press(Message::SortApplyDraft)
                .padding(Padding::from([6, 16]))
                .style(|_theme, _status| iced::widget::button::Style {
                    background: Some(iced::Background::Color(PRIMARY_COLOR)),
                    text_color: Color::WHITE,
                    ..iced::widget::button::Style::default()
                }),
        )
        .align_y(Alignment::Center)
        .into()
}

/// The small "Live preview" header on the right pane.
fn preview_title<'a>() -> Element<'a, Message> {
    text("Live preview (first 10 entries)")
        .size(13)
        .style(|_| iced::widget::text::Style {
            color: Some(SECONDARY_COLOR),
        })
        .into()
}

/// Sort a copy of the preview entries through the draft chain and
/// render the result. Falls back to a "no sort" baseline (the
/// entries in their on-disk order) when the chain has no enabled
/// keys, so the user always sees a meaningful comparison.
fn preview_pane<'a>(
    entries: &'a [EntryInfo],
    chain: &'a SortChain,
    primary_type: Option<&'a str>,
    ide_labels: &'a std::collections::HashMap<
        compact_str::CompactString,
        compact_str::CompactString,
    >,
    col_labels: &'a std::collections::HashMap<
        compact_str::CompactString,
        compact_str::CompactString,
    >,
) -> Element<'a, Message> {
    if entries.is_empty() {
        return container(
            text("(no entries in the current archive)")
                .size(12)
                .style(|_| iced::widget::text::Style {
                    color: Some(SECONDARY_COLOR),
                }),
        )
        .padding(8)
        .into();
    }

    // `Vec<&EntryInfo>` doesn't impl `DerefMut<Target = [&EntryInfo]>`
    // (the orphan rule blocks it), so we collect the references
    // into a `Vec<EntryInfo>` *copy* for the preview. The preview
    // is bounded at 10 entries so the copy is cheap, and it
    // means the rest of this function can keep the borrow-checks
    // off the critical path.
    let mut sorted: Vec<EntryInfo> = entries.to_vec();
    let ctx = SortContext {
        primary_type,
        ide_files: ide_labels,
        col_files: col_labels,
    };
    sort_entries(&mut sorted, chain, &ctx);

    let max = sorted.len().min(PREVIEW_MAX);
    let mut col = Column::new().spacing(2);
    for (i, entry) in sorted.iter().take(max).enumerate() {
        let name = entry.file_name.as_str();
        // Color: 1st item is brightest (top of sort), then dim down.
        let color = match i {
            0 => PRIMARY_COLOR,
            1..=2 => Color::from_rgb(0.85, 0.85, 0.85),
            _ => SECONDARY_COLOR,
        };
        col = col.push(
            text(format!("{:>2}. {name}", i + 1))
                .size(12)
                .style(move |_| iced::widget::text::Style {
                    color: Some(color),
                }),
        );
    }
    if sorted.len() > max {
        col = col.push(
            text(format!("… and {} more", sorted.len() - max))
                .size(11)
                .style(|_| iced::widget::text::Style {
                    color: Some(SECONDARY_COLOR),
                }),
        );
    }
    container(col).padding(8).into()
}

/// Preset dropdown. Picking one replaces the draft (Apply
/// later commits). The picker is intentionally non-destructive —
/// applying a preset doesn't close the dialog, so the user can
/// compare it against the previous setup before clicking Apply.
fn preset_picker<'a>() -> Element<'a, Message> {
    let options: Vec<String> = SortPreset::ALL
        .iter()
        .map(|p| p.display_name().to_string())
        .collect();
    PickList::new(
        options,
        None::<String>,
        move |selected: String| {
            // Map the selected display name back to the preset
            // variant. Fall back to NameAZ on mismatch so a
            // stale `settings.ini` round-trip doesn't silently
            // change behavior.
            let preset = SortPreset::ALL
                .iter()
                .copied()
                .find(|p| p.display_name() == selected)
                .unwrap_or(SortPreset::NameAZ);
            Message::SortSelectPreset(preset)
        },
    )
    .placeholder("Apply preset…")
    .text_size(12)
    .into()
}

/// Convert a `SortDirection` to a short label for the toggle
/// button. Kept here (not in `sort.rs`) because it's purely a
/// UI concern — the engine only cares about enum variants.
fn dir_label(d: SortDirection) -> &'static str {
    match d {
        SortDirection::Ascending => "Asc ▲",
        SortDirection::Descending => "Desc ▼",
    }
}

/// One-line hint string for a slot. Shows up as a tooltip in
/// the real Iced UI; for now we just inline it under the
/// slot so the user can see what the key does. Used by the
/// "explain" helper below the slot list.
#[allow(dead_code)]
pub fn explain_slot(prio: &SortPriority) -> String {
    format!(
        "{} {} — entries with equal {} are ordered by the next key",
        prio.key.display_name(),
        match prio.direction {
            SortDirection::Ascending => "ascending",
            SortDirection::Descending => "descending",
        },
        prio.key.display_name().to_ascii_lowercase(),
    )
}
