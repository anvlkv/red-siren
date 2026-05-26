use eframe::egui;
use std::sync::Once;

fn init_terminal_logger() {
    static LOGGER_INIT: Once = Once::new();

    LOGGER_INIT.call_once(|| {
        let mut builder =
            env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(concat!(
                "warn,",
                env!("CARGO_PKG_NAME"),
                "=trace"
            )));
        builder
            .format_timestamp_millis()
            .format_target(true)
            .target(env_logger::Target::Stderr);

        let _ = builder.try_init();
    });
}

pub fn run_native_app<T, F>(
    title: &'static str,
    inner_size: [f32; 2],
    app_creator: F,
) -> eframe::Result<()>
where
    T: eframe::App + 'static,
    F: FnOnce(&eframe::CreationContext<'_>) -> T + 'static,
{
    init_terminal_logger();
    log::info!(
        "starting native app title={title} inner_size={}x{}",
        inner_size[0],
        inner_size[1]
    );

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
