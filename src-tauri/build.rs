fn main() {
    #[cfg(target_os = "android")]
    {
        // AAudio is typically available as part of the Android system
        // Just declare the link without specifying a path
        println!("cargo:rustc-link-lib=aaudio");

        // These are usually available
        println!("cargo:rustc-link-lib=log");
        println!("cargo:rustc-link-lib=android");
    }

    tauri_build::build()
}
