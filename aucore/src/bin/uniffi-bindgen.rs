fn main() {   
    #[cfg(not(feature = "cargo-clippy"))]
    uniffi::uniffi_bindgen_main()
}
