
pub fn main() {
  #[cfg(feature = "browser")] {
      use fs_extra::copy_items;
      use fs_extra::dir::{create_all, CopyOptions};

      
      let mut options = CopyOptions::new();
      options.overwrite = true;
  
      println!("cargo:rerun-if-changed=../worklet/dist/");
  

      create_all("../target/site/pkg/worklet", true).expect("create pkg/worklet");
      copy_items(&["../worklet/dist"], "../target/site/pkg/worklet", &options).expect("copy worklet");
    }
}
