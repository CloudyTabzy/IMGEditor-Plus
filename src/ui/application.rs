pub fn run(app: impl eframe::App + 'static) -> eframe::Result<()> {
    let native_options = eframe::NativeOptions::default();

    eframe::run_native(
        "Grinch_'s IMG Editor",
        native_options,
        Box::new(|_| Box::new(app)),
    )
}
