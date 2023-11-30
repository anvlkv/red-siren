use fs_extra::copy_items;
use fs_extra::dir::{create_all, CopyOptions};
use std::env;
use std::path::PathBuf;
use std::process::Command;

pub fn main() {
    #[cfg(feature = "browser")] {
      let mut options = CopyOptions::new();
      options.overwrite = true;
      let this = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
      let root = this.parent().unwrap();
      let wk_dir = root.join("worklet");
      let root = root.to_str().unwrap();
  
      println!("cargo:rerun-if-changed={root}/shared/src/");
      println!("cargo:rerun-if-changed={root}/worklet/src/");
      println!("cargo:rerun-if-changed={root}/web-leptos/build.rs");

      Command::new("pnpm")
          .arg("build")
          .current_dir(wk_dir)
          .status()
          .unwrap();
  

      create_all("../target/site/pkg/worklet", false).expect("create pkg/worklet");
      copy_items(&["../worklet/dist"], "../target/site/pkg/worklet", &options).expect("copy worklet");
    }
}
