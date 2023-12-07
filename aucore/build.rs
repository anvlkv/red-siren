fn main() {
  #[cfg(not(feature = "cargo-clippy"))]
  uniffi::generate_scaffolding("./src/aucore.udl").unwrap();
}