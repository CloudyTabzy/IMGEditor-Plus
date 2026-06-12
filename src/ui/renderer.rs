pub use eframe::egui;

#[derive(Default)]
pub struct MainWindow;

impl eframe::App for MainWindow {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    ui.label("New");
                    ui.label("Open");
                    ui.label("Save");
                });
                ui.menu_button("Edit", |ui| {
                    ui.label("Import");
                    ui.label("Export");
                });
                ui.menu_button("Help", |ui| {
                    ui.label("About");
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Grinch_'s IMG Editor");
            ui.label("Rust scaffold");
        });
    }
}
