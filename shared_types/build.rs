use std::path::PathBuf;

use crux_core::typegen::TypeGen;

fn main() -> anyhow::Result<()> {
    println!("cargo:rerun-if-changed=../shared");
    println!("cargo:rerun-if-changed=../aucore");

    {
        use aucore::RedSirenAU;
        use shared::instrument::{Config, Node};

        let mut gen = TypeGen::new();
        gen.register_type::<Config>()?;
        gen.register_type::<Node>()?;
        gen.register_app::<RedSirenAU>()?;

        let output_root = PathBuf::from("./generated");

        gen.swift("AUTypes", output_root.join("swift"))?;

        gen.java(
            "com.anvlkv.redsiren.shared.au_types",
            output_root.join("java"),
        )?;

        gen.typescript("au_types", output_root.join("typescript"))?;
    }

    {
        use shared::{
            geometry::{Line, Rect},
            instrument::{layout::MenuPosition, Config, InstrumentEV, Layout, Node, PlaybackEV},
            intro::IntroEV,
            tuner::TunerEV,
            Activity, RedSiren,
        };

        let mut gen = TypeGen::new();
        gen.register_type::<InstrumentEV>()?;
        gen.register_type::<IntroEV>()?;
        gen.register_type::<TunerEV>()?;
        gen.register_type::<PlaybackEV>()?;

        gen.register_type::<Activity>()?;
        gen.register_type::<MenuPosition>()?;
        gen.register_type::<Line>()?;
        gen.register_type::<Rect>()?;
        gen.register_type::<Config>()?;
        gen.register_type::<Layout>()?;
        gen.register_type::<Node>()?;

        gen.register_app::<RedSiren>()?;

        let output_root = PathBuf::from("./generated");

        gen.swift("SharedTypes", output_root.join("swift"))?;

        gen.java(
            "com.anvlkv.redsiren.shared.shared_types",
            output_root.join("java"),
        )?;

        gen.typescript("shared_types", output_root.join("typescript"))?;
    }

    Ok(())
}
