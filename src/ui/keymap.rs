use iced::keyboard::Key;
use iced::keyboard::Modifiers;
use iced::keyboard::key::{Code, Named, Physical};

use crate::ui::app::InspectorTab;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shortcut {
    New,
    Open,
    Save,
    SaveAs,
    Close,
    Import,
    ImportReplace,
    ExportAll,
    ExportSelected,
    SelectAll,
    InvertSelection,
    ClearSelection,
    Delete,
    FocusSearch,
    CheckUpdates,
    SwitchTab(InspectorTab),
}

impl Shortcut {
    pub fn label(self) -> &'static str {
        match self {
            Shortcut::New => "New",
            Shortcut::Open => "Open…",
            Shortcut::Save => "Save",
            Shortcut::SaveAs => "Save as…",
            Shortcut::Close => "Close tab",
            Shortcut::Import => "Import",
            Shortcut::ImportReplace => "Import and replace",
            Shortcut::ExportAll => "Export all",
            Shortcut::ExportSelected => "Export selected",
            Shortcut::SelectAll => "Select all",
            Shortcut::InvertSelection => "Invert selection",
            Shortcut::ClearSelection => "Clear selection",
            Shortcut::Delete => "Delete selected",
            Shortcut::FocusSearch => "Focus search",
            Shortcut::CheckUpdates => "Check for updates",
            Shortcut::SwitchTab(InspectorTab::Export) => "Export tab",
            Shortcut::SwitchTab(InspectorTab::Model3D) => "3D viewer",
            Shortcut::SwitchTab(InspectorTab::Texture) => "Texture viewer",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyChord {
    pub physical: Physical,
    pub modifiers: Modifiers,
}

impl KeyChord {
    pub const fn new(physical: Physical, modifiers: Modifiers) -> Self {
        Self {
            physical,
            modifiers,
        }
    }
}

pub fn shortcut_chord(shortcut: Shortcut) -> KeyChord {
    let (physical, mods) = match shortcut {
        Shortcut::New => (Physical::Code(Code::KeyN), Modifiers::CTRL),
        Shortcut::Open => (Physical::Code(Code::KeyO), Modifiers::CTRL),
        Shortcut::Save => (Physical::Code(Code::KeyS), Modifiers::CTRL),
        Shortcut::SaveAs => (
            Physical::Code(Code::KeyS),
            Modifiers::CTRL | Modifiers::SHIFT,
        ),
        Shortcut::Close => (Physical::Code(Code::KeyX), Modifiers::SHIFT),
        Shortcut::Import => (Physical::Code(Code::KeyI), Modifiers::CTRL),
        Shortcut::ImportReplace => (
            Physical::Code(Code::KeyI),
            Modifiers::CTRL | Modifiers::SHIFT,
        ),
        Shortcut::ExportAll => (Physical::Code(Code::KeyE), Modifiers::CTRL),
        Shortcut::ExportSelected => (
            Physical::Code(Code::KeyE),
            Modifiers::CTRL | Modifiers::SHIFT,
        ),
        Shortcut::SelectAll => (Physical::Code(Code::KeyA), Modifiers::CTRL),
        Shortcut::InvertSelection => (
            Physical::Code(Code::KeyA),
            Modifiers::CTRL | Modifiers::SHIFT,
        ),
        Shortcut::ClearSelection => (Physical::Code(Code::KeyD), Modifiers::CTRL),
        Shortcut::Delete => (Physical::Code(Code::Delete), Modifiers::empty()),
        Shortcut::FocusSearch => (Physical::Code(Code::KeyF), Modifiers::CTRL),
        Shortcut::CheckUpdates => (Physical::Code(Code::KeyU), Modifiers::CTRL),
        Shortcut::SwitchTab(InspectorTab::Export) => {
            (Physical::Code(Code::Digit1), Modifiers::empty())
        }
        Shortcut::SwitchTab(InspectorTab::Model3D) => {
            (Physical::Code(Code::Digit2), Modifiers::empty())
        }
        Shortcut::SwitchTab(InspectorTab::Texture) => {
            (Physical::Code(Code::Digit3), Modifiers::empty())
        }
    };
    KeyChord::new(physical, mods)
}

/// Secondary chord accepted for a shortcut, for keyboard layouts and
/// hardware (numpad) where the primary binding is awkward. Detection
/// accepts both; menus advertise only the primary chord.
fn alternate_chord(shortcut: Shortcut) -> Option<KeyChord> {
    let (physical, mods) = match shortcut {
        Shortcut::ClearSelection => (Physical::Code(Code::Escape), Modifiers::empty()),
        Shortcut::Delete => (Physical::Code(Code::KeyX), Modifiers::CTRL),
        Shortcut::SwitchTab(InspectorTab::Export) => {
            (Physical::Code(Code::Numpad1), Modifiers::empty())
        }
        Shortcut::SwitchTab(InspectorTab::Model3D) => {
            (Physical::Code(Code::Numpad2), Modifiers::empty())
        }
        Shortcut::SwitchTab(InspectorTab::Texture) => {
            (Physical::Code(Code::Numpad3), Modifiers::empty())
        }
        _ => return None,
    };
    Some(KeyChord::new(physical, mods))
}

pub fn chord_matches(chord: KeyChord, pressed_physical: Physical, pressed_mods: Modifiers) -> bool {
    if chord.physical != pressed_physical {
        return false;
    }
    let required = chord.modifiers;
    if required.contains(Modifiers::CTRL) != pressed_mods.contains(Modifiers::CTRL) {
        return false;
    }
    if required.contains(Modifiers::SHIFT) != pressed_mods.contains(Modifiers::SHIFT) {
        return false;
    }
    if required.contains(Modifiers::ALT) != pressed_mods.contains(Modifiers::ALT) {
        return false;
    }
    true
}

pub fn shortcut_matches(shortcut: Shortcut, pressed_physical: Physical, pressed_mods: Modifiers) -> bool {
    if chord_matches(shortcut_chord(shortcut), pressed_physical, pressed_mods) {
        return true;
    }
    alternate_chord(shortcut)
        .is_some_and(|chord| chord_matches(chord, pressed_physical, pressed_mods))
}

pub fn detect_pressed(pressed_physical: Physical, pressed_mods: Modifiers) -> Option<Shortcut> {
    all_shortcuts().into_iter()
        .find(|s| shortcut_matches(*s, pressed_physical, pressed_mods))
}

pub fn shortcut_display(shortcut: Shortcut) -> String {
    let chord = shortcut_chord(shortcut);
    let mut parts: Vec<String> = Vec::new();
    if chord.modifiers.contains(Modifiers::CTRL) {
        parts.push("Ctrl".to_string());
    }
    if chord.modifiers.contains(Modifiers::SHIFT) {
        parts.push("Shift".to_string());
    }
    if chord.modifiers.contains(Modifiers::ALT) {
        parts.push("Alt".to_string());
    }
    parts.push(label_for_physical(chord.physical));
    parts.join(" + ")
}

fn label_for_physical(physical: Physical) -> String {
    match physical {
        Physical::Code(Code::KeyA) => "A".into(),
        Physical::Code(Code::KeyB) => "B".into(),
        Physical::Code(Code::KeyC) => "C".into(),
        Physical::Code(Code::KeyD) => "D".into(),
        Physical::Code(Code::KeyE) => "E".into(),
        Physical::Code(Code::KeyF) => "F".into(),
        Physical::Code(Code::KeyG) => "G".into(),
        Physical::Code(Code::KeyH) => "H".into(),
        Physical::Code(Code::KeyI) => "I".into(),
        Physical::Code(Code::KeyJ) => "J".into(),
        Physical::Code(Code::KeyK) => "K".into(),
        Physical::Code(Code::KeyL) => "L".into(),
        Physical::Code(Code::KeyM) => "M".into(),
        Physical::Code(Code::KeyN) => "N".into(),
        Physical::Code(Code::KeyO) => "O".into(),
        Physical::Code(Code::KeyP) => "P".into(),
        Physical::Code(Code::KeyQ) => "Q".into(),
        Physical::Code(Code::KeyR) => "R".into(),
        Physical::Code(Code::KeyS) => "S".into(),
        Physical::Code(Code::KeyT) => "T".into(),
        Physical::Code(Code::KeyU) => "U".into(),
        Physical::Code(Code::KeyV) => "V".into(),
        Physical::Code(Code::KeyW) => "W".into(),
        Physical::Code(Code::KeyX) => "X".into(),
        Physical::Code(Code::KeyY) => "Y".into(),
        Physical::Code(Code::KeyZ) => "Z".into(),
        Physical::Code(Code::Delete) => "Del".into(),
        Physical::Code(Code::Enter) => "Enter".into(),
        Physical::Code(Code::Escape) => "Esc".into(),
        Physical::Code(Code::Space) => "Space".into(),
        Physical::Code(Code::Digit1) => "1".into(),
        Physical::Code(Code::Digit2) => "2".into(),
        Physical::Code(Code::Digit3) => "3".into(),
        Physical::Code(Code::Numpad1) => "Num 1".into(),
        Physical::Code(Code::Numpad2) => "Num 2".into(),
        Physical::Code(Code::Numpad3) => "Num 3".into(),
        Physical::Code(Code::Quote) => "'".into(),
        other => format!("{other:?}"),
    }
}

pub fn named_key_label(key: Key) -> Option<&'static str> {
    match key {
        Key::Named(Named::Enter) => Some("Enter"),
        Key::Named(Named::Escape) => Some("Escape"),
        Key::Named(Named::Backspace) => Some("Backspace"),
        Key::Named(Named::Delete) => Some("Delete"),
        Key::Named(Named::Tab) => Some("Tab"),
        _ => None,
    }
}

pub fn all_shortcuts() -> Vec<Shortcut> {
    vec![
        Shortcut::New,
        Shortcut::Open,
        Shortcut::Save,
        Shortcut::SaveAs,
        Shortcut::Close,
        Shortcut::Import,
        Shortcut::ImportReplace,
        Shortcut::ExportAll,
        Shortcut::ExportSelected,
        Shortcut::SelectAll,
        Shortcut::InvertSelection,
        Shortcut::ClearSelection,
        Shortcut::Delete,
        Shortcut::FocusSearch,
        Shortcut::CheckUpdates,
        Shortcut::SwitchTab(InspectorTab::Export),
        Shortcut::SwitchTab(InspectorTab::Model3D),
        Shortcut::SwitchTab(InspectorTab::Texture),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_ctrl_s() {
        let detected = detect_pressed(Physical::Code(Code::KeyS), Modifiers::CTRL);
        assert_eq!(detected, Some(Shortcut::Save));
    }

    #[test]
    fn detect_ctrl_shift_s() {
        let detected = detect_pressed(
            Physical::Code(Code::KeyS),
            Modifiers::CTRL | Modifiers::SHIFT,
        );
        assert_eq!(detected, Some(Shortcut::SaveAs));
    }

    #[test]
    fn detect_plain_delete() {
        let detected = detect_pressed(Physical::Code(Code::Delete), Modifiers::empty());
        assert_eq!(detected, Some(Shortcut::Delete));
    }

    #[test]
    fn detect_ctrl_x_deletes() {
        let detected = detect_pressed(Physical::Code(Code::KeyX), Modifiers::CTRL);
        assert_eq!(detected, Some(Shortcut::Delete));
    }

    #[test]
    fn detect_escape_clears_selection() {
        let detected = detect_pressed(Physical::Code(Code::Escape), Modifiers::empty());
        assert_eq!(detected, Some(Shortcut::ClearSelection));
    }

    #[test]
    fn detect_ctrl_d_clears_selection() {
        let detected = detect_pressed(Physical::Code(Code::KeyD), Modifiers::CTRL);
        assert_eq!(detected, Some(Shortcut::ClearSelection));
    }

    #[test]
    fn display_shows_ctrl_d_for_clear_selection() {
        let display = shortcut_display(Shortcut::ClearSelection);
        assert!(display.contains("Ctrl"));
        assert!(display.contains("D"));
    }

    #[test]
    fn digits_switch_inspector_tabs() {
        assert_eq!(
            detect_pressed(Physical::Code(Code::Digit1), Modifiers::empty()),
            Some(Shortcut::SwitchTab(InspectorTab::Export))
        );
        assert_eq!(
            detect_pressed(Physical::Code(Code::Digit2), Modifiers::empty()),
            Some(Shortcut::SwitchTab(InspectorTab::Model3D))
        );
        assert_eq!(
            detect_pressed(Physical::Code(Code::Digit3), Modifiers::empty()),
            Some(Shortcut::SwitchTab(InspectorTab::Texture))
        );
    }

    #[test]
    fn numpad_digits_are_alternate_tab_chords() {
        assert_eq!(
            detect_pressed(Physical::Code(Code::Numpad1), Modifiers::empty()),
            Some(Shortcut::SwitchTab(InspectorTab::Export))
        );
        assert_eq!(
            detect_pressed(Physical::Code(Code::Numpad2), Modifiers::empty()),
            Some(Shortcut::SwitchTab(InspectorTab::Model3D))
        );
        assert_eq!(
            detect_pressed(Physical::Code(Code::Numpad3), Modifiers::empty()),
            Some(Shortcut::SwitchTab(InspectorTab::Texture))
        );
    }

    #[test]
    fn ctrl_modifier_required() {
        let detected = detect_pressed(Physical::Code(Code::KeyS), Modifiers::empty());
        assert_eq!(detected, None);
    }

    #[test]
    fn display_includes_modifiers() {
        let display = shortcut_display(Shortcut::Save);
        assert!(display.contains("Ctrl"));
        assert!(display.contains("S"));
    }
}
