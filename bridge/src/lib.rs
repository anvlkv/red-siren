mod bridge;

pub use crux_core::{bridge::Bridge, capability::Operation, Core, Request};

pub use app_core::{self};
pub use au_core::{self};
pub use bridge::*;

lazy_static::lazy_static! {
    static ref CORE: Bridge<Effect, RedSirenBridge> =
        Bridge::new(Core::new::<RedSirenBridgeCapabilities>());
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
pub fn platform_init_once() {
    let lvl = log::LevelFilter::Trace;

    cfg_if::cfg_if! { if #[cfg(target_arch = "wasm32")] {
        let lvl = lvl.to_level().unwrap();

        _ = console_log::init_with_level(lvl);
        console_error_panic_hook::set_once();
    } else if #[cfg(target_os = "android")] {
        mod init_android_ctx;
        android_logger::init_once(
            android_logger::Config::default()
                .with_max_level(lvl)
                .with_tag("red_siren::core"),
        );
        unsafe {
            init_android_ctx::initialize_android_context()
        }
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

uniffi::include_scaffolding!("bridge");
