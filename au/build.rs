fn main() {
  cxx_build::bridge("src/lib.rs")  // returns a cc::Build
        .file("src/lib.cc")
        .flag_if_supported("-std=c++14")
        .compile("au_lib");

  println!("cargo:rerun-if-changed=../shared/src/");
  println!("cargo:rerun-if-changed=./src/");
}