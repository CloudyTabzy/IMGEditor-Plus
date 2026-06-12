pub fn run(app: impl eframe::App + 'static) -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Grinch_'s IMG Editor")
            .with_inner_size([900.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Grinch_'s IMG Editor",
        native_options,
        Box::new(|_| Box::new(app)),
    )
}
