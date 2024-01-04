use crux_core::bridge::Bridge;
use lazy_static::lazy_static;

use super::*;

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
pub fn log_init(lvl: log::LevelFilter) {
    #[cfg(feature = "android")]
    {
        android_logger::init_once(
            android_logger::Config::default()
                .with_max_level(lvl)
                .with_tag("red_siren::core"),
        );
    }

    #[cfg(feature = "ios")]
    {
        oslog::OsLogger::new("com.anvlkv.RedSiren.Shared")
            .level_filter(lvl)
            .init()
            .unwrap();
    }

    log::info!("init logging")
}
