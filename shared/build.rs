fn main() {
    #[cfg(not(feature = "cargo-clippy"))]
    uniffi::generate_scaffolding("./src/shared.udl").unwrap();
}
