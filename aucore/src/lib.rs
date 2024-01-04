pub use crux_core::{Core, Request};

pub use app::*;
pub use streamer::*;

pub mod app;
mod resolve;
pub mod system;

mod streamer;

cfg_if::cfg_if! {if #[cfg(feature="browser")] {
    mod instance;
    pub use instance::*;
}}

pub fn au_log_init(lvl: log::LevelFilter) {

    #[cfg(feature = "browser")]
    {
        _ = console_log::init_with_level(lvl.to_level().unwrap());
        console_error_panic_hook::set_once();
    }

    #[cfg(feature = "android")]
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(lvl)
            .with_tag("red_siren::core"),
    );

    #[cfg(feature = "ios")]
    match oslog::OsLogger::new("com.anvlkv.RedSiren.AUCore")
        .level_filter(lvl)
        .init()
    {
        Ok(_) => {}
        Err(e) => {
            log::error!("already initialized: {e:?}");
        }
    }

    log::info!("init logging {lvl:?}");
}
