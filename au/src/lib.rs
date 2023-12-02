#[cxx::bridge]
mod ffi {
    extern "Rust" {
        fn process_event(data: &[u8]) -> Vec<u8>;
        fn handle_response(uuid: &[u8], data: &[u8]) -> Vec<u8>;
        fn view() -> Vec<u8>;
        fn log_init();
    }
}

pub fn process_event(data: &[u8]) -> Vec<u8> {
    shared::process_event(data)
}
pub fn handle_response(uuid: &[u8], data: &[u8]) -> Vec<u8> {
    shared::handle_response(uuid, data)
}
pub fn view() -> Vec<u8> {
    shared::view()
}
pub fn log_init() {
    shared::log_init()
}