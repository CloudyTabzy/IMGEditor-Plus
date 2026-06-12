#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hotkey {
    primary: egui::Key,
    secondary: Option<egui::Key>,
}

impl Hotkey {
    pub const fn new(primary: egui::Key, secondary: Option<egui::Key>) -> Self {
        Self { primary, secondary }
    }

    pub fn pressed(&self, input: &egui::InputState, modifiers: egui::Modifiers) -> bool {
        input.modifiers == modifiers
            && input.key_pressed(self.primary)
            && self.secondary.map_or(true, |key| input.key_pressed(key))
    }
}
