use std::path::PathBuf;

use crux_core::typegen::TypeGen;

fn main() -> anyhow::Result<()> {
    println!("cargo:rerun-if-changed=../app_core");
    
    use app_core::{
        Activity, RedSiren,
    };

    let mut gen = TypeGen::new();

    gen.register_app::<RedSiren>()?;

    let output_root = PathBuf::from("./generated");

    gen.swift("CoreTypes", output_root.join("swift"))?;

    gen.java("com.anvlkv.redsiren.core.typegen", output_root.join("java"))?;

    gen.typescript("typegen", output_root.join("typescript"))?;

    Ok(())
}
