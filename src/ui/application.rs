pub fn run(app: impl eframe::App + 'static, options: eframe::NativeOptions) -> eframe::Result<()> {
    eframe::run_native("Grinch_'s IMG Editor", options, Box::new(|_| Box::new(app)))
}
