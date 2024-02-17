pub use crux_core::{bridge::Bridge, Core, Request};
pub use crux_http as http;
pub use crux_kv as key_value;
use lazy_static::lazy_static;

pub use app::*;

pub mod app;

lazy_static! {
    static ref CORE: Bridge<Effect, RedSiren> = Bridge::new(Core::new::<RedSirenCapabilities>());
}

pub fn process_event(data: &[u8]) -> Vec<u8> {
    CORE.process_event(data)
}

pub fn handle_response(uuid: &[u8], data: &[u8]) -> Vec<u8> {
    CORE.handle_response(uuid, data)
}

pub fn view() -> Vec<u8> {
    CORE.view()
}

#[allow(unused_variables)]
pub fn log_init() {
    let lvl = log::LevelFilter::Trace;

    cfg_if::cfg_if! { if #[cfg(feature="browser")] {
        let lvl = lvl.to_level().unwrap();

        _ = console_log::init_with_level(lvl);
        console_error_panic_hook::set_once();
    } else if #[cfg(target_os = "android")] {
        android_logger::init_once(
            android_logger::Config::default()
                .with_max_level(lvl)
                .with_tag("red_siren::core"),
        );
    }
    else if #[cfg(target_os = "ios")] {
        oslog::OsLogger::new("com.anvlkv.RedSiren.Core")
            .level_filter(lvl)
            .init()
            .unwrap();
    }
    else {
        let lvl = lvl.to_level().unwrap();

        simple_logger::init_with_level(lvl).expect("couldn't initialize logging");
    }}

    log::info!("init logging")
}

uniffi::include_scaffolding!("core");
