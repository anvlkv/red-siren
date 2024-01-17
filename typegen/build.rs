use std::path::PathBuf;

use crux_core::typegen::TypeGen;

fn main() -> anyhow::Result<()> {
    println!("cargo:rerun-if-changed=../core");
    println!("cargo:rerun-if-changed=../aucore");

    {
        use app_core::instrument::{Config, Node};
        use aucore::RedSirenAU;

        let mut gen = TypeGen::new();
        gen.register_type::<Config>()?;
        gen.register_type::<Node>()?;
        gen.register_app::<RedSirenAU>()?;

        let output_root = PathBuf::from("./generated");

        gen.swift("AUTypes", output_root.join("swift"))?;

        gen.java(
            "com.anvlkv.redsiren.core.au_types",
            output_root.join("java"),
        )?;

        gen.typescript("au_types", output_root.join("typescript"))?;
    }

    {
        use app_core::{
            geometry::{Line, Rect},
            instrument::{layout::MenuPosition, Config, InstrumentEV, Layout, Node, PlaybackEV},
            intro::IntroEV,
            play::CaptureOutput,
            tuner::{TriggerState, TunerEV},
            Activity, RedSiren,
        };

        let mut gen = TypeGen::new();
        gen.register_type::<InstrumentEV>()?;
        gen.register_type::<IntroEV>()?;
        gen.register_type::<TunerEV>()?;
        gen.register_type::<PlaybackEV>()?;
        gen.register_type::<TriggerState>()?;
        gen.register_type_with_samples(vec![
            CaptureOutput::CaptureFFT(vec![(0.0, 0.0)]),
            CaptureOutput::CaptureData(vec![0.0]),
            CaptureOutput::CaptureFFT((0..64).map(|i| (i as f32, (i * 2) as f32 / 1.0)).collect()),
            CaptureOutput::CaptureData((0..64).map(|i| i as f32 / 1.0).collect()),
            CaptureOutput::CaptureNodesData(
                (1..=5)
                    .map(|f| (f, (0..64).map(|i| i as f32 / 1.0).collect::<Vec<_>>()))
                    .collect::<Vec<_>>(),
            ),
        ])?;

        gen.register_type::<Activity>()?;
        gen.register_type::<MenuPosition>()?;
        gen.register_type::<Line>()?;
        gen.register_type::<Rect>()?;
        gen.register_type::<Config>()?;
        gen.register_type::<Layout>()?;
        gen.register_type::<Node>()?;

        gen.register_app::<RedSiren>()?;

        let output_root = PathBuf::from("./generated");

        gen.swift("CoreTypes", output_root.join("swift"))?;

        gen.java("com.anvlkv.redsiren.core.typegen", output_root.join("java"))?;

        gen.typescript("typegen", output_root.join("typescript"))?;
    }

    Ok(())
}
