use eframe::egui;

pub fn run_native_app<T, F>(
    title: &'static str,
    inner_size: [f32; 2],
    app_creator: F,
) -> eframe::Result<()>
where
    T: eframe::App + 'static,
    F: FnOnce(&eframe::CreationContext<'_>) -> T + 'static,
{
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(title)
            .with_inner_size(inner_size),
        ..Default::default()
    };

    eframe::run_native(
        title,
        options,
        Box::new(move |cc| Ok(Box::new(app_creator(cc)))),
    )
}
