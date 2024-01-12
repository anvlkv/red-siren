pub use crux_core::{Core, Request};

pub use app::*;


pub mod app;
mod resolve;
mod capture;
pub mod system;


cfg_if::cfg_if! {if #[cfg(feature="browser")] {
    mod instance;
    pub use instance::*;
} else {
    mod streamer;
    pub use streamer::*;
}}


pub fn log_init(lvl: log::LevelFilter) {

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
