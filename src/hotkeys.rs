#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hotkey {
    primary: egui::Key,
    secondary: Option<egui::Key>,
}

impl Hotkey {
    pub const fn new(primary: egui::Key, secondary: Option<egui::Key>) -> Self {
        Self { primary, secondary }
    }

    pub fn pressed(&self, input: &egui::InputState, required: egui::Modifiers) -> bool {
        input.key_pressed(self.primary)
            && self.secondary.map_or(true, |key| input.key_pressed(key))
            && modifiers_match(input.modifiers, required)
    }
}

fn modifiers_match(input: egui::Modifiers, required: egui::Modifiers) -> bool {
    (!required.command || input.command)
        && (!required.ctrl || input.ctrl)
        && (!required.shift || input.shift)
        && (!required.alt || input.alt)
        && (!required.mac_cmd || input.mac_cmd)
}
