use iced::keyboard::Key;
use iced::keyboard::Modifiers;
use iced::keyboard::key::{Code, Named, Physical};

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
    Delete,
    FocusSearch,
    CheckUpdates,
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
            Shortcut::Delete => "Delete selected",
            Shortcut::FocusSearch => "Focus search",
            Shortcut::CheckUpdates => "Check for updates",
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
        Shortcut::SaveAs => (Physical::Code(Code::KeyS), Modifiers::CTRL | Modifiers::SHIFT),
        Shortcut::Close => (Physical::Code(Code::KeyX), Modifiers::SHIFT),
        Shortcut::Import => (Physical::Code(Code::KeyI), Modifiers::CTRL),
        Shortcut::ImportReplace => (Physical::Code(Code::KeyI), Modifiers::CTRL | Modifiers::SHIFT),
        Shortcut::ExportAll => (Physical::Code(Code::KeyE), Modifiers::CTRL),
        Shortcut::ExportSelected => (Physical::Code(Code::KeyE), Modifiers::CTRL | Modifiers::SHIFT),
        Shortcut::SelectAll => (Physical::Code(Code::KeyA), Modifiers::CTRL),
        Shortcut::InvertSelection => (Physical::Code(Code::KeyA), Modifiers::CTRL | Modifiers::SHIFT),
        Shortcut::Delete => (Physical::Code(Code::Delete), Modifiers::empty()),
        Shortcut::FocusSearch => (Physical::Code(Code::KeyF), Modifiers::CTRL),
        Shortcut::CheckUpdates => (Physical::Code(Code::KeyU), Modifiers::CTRL),
    };
    KeyChord::new(physical, mods)
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

pub fn detect_pressed(pressed_physical: Physical, pressed_mods: Modifiers) -> Option<Shortcut> {
    let all = [
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
        Shortcut::Delete,
        Shortcut::FocusSearch,
        Shortcut::CheckUpdates,
    ];
    all.into_iter()
        .find(|s| chord_matches(shortcut_chord(*s), pressed_physical, pressed_mods))
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
        Shortcut::Delete,
        Shortcut::FocusSearch,
        Shortcut::CheckUpdates,
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
