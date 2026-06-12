pub struct Widget;

impl Widget {
    pub fn centered_label(ui: &mut eframe::egui::Ui, text: &str) {
        ui.with_layout(
            eframe::egui::Layout::centered_and_justified(eframe::egui::Direction::TopDown),
            |ui| {
                ui.label(text);
            },
        );
    }
}
