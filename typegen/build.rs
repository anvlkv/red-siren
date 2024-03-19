use std::path::PathBuf;

use crux_core::typegen::TypeGen;

fn main() -> anyhow::Result<()> {
    println!("cargo:rerun-if-changed=../app_core");
    println!("cargo:rerun-if-changed=../au_core");

    use app_core::{
        Activity, Alignment, Box2D, ObjectId, ObjectStyle, Paint, Point2D, RedSiren, Rgba, Shapes,
        Size2D, Stroke, Text, UnitState, ViewObject, VisualEV, VisualVM,
    };

    let mut gen = TypeGen::new();

    // external types
    gen.register_type_with_samples::<ObjectId>(vec![ObjectId::default(), ObjectId::default()])?;
    gen.register_type::<Rgba>()?;
    gen.register_type::<Point2D<f64>>()?;
    gen.register_type::<Box2D<f64>>()?;
    gen.register_type::<Size2D<f64>>()?;

    // internal types
    gen.register_type::<ViewObject>()?;
    gen.register_type::<ObjectStyle>()?;
    gen.register_type::<Alignment>()?;
    gen.register_type::<Activity>()?;
    gen.register_type::<Text>()?;
    gen.register_type::<Stroke>()?;
    gen.register_type::<Shapes>()?;
    gen.register_type::<Paint>()?;
    gen.register_type::<UnitState>()?;
    gen.register_type::<VisualVM>()?;
    gen.register_type::<VisualEV>()?;
    // app type
    gen.register_app::<RedSiren>()?;

    let output_root = PathBuf::from("./generated");

    gen.swift("CoreTypes", output_root.join("swift"))?;

    gen.java("com.anvlkv.redsiren.core.typegen", output_root.join("java"))?;

    gen.typescript("typegen", output_root.join("typescript"))?;

    Ok(())
}
