use crux_core::typegen::TypeGen;
use shared::{
    geometry::{Line, Rect},
    instrument::{Config, Instrument, Layout, InstrumentEV},
    intro::IntroEV,
    tuner::TunerEV,
    navigate::NavigateOperation,
    Activity, Intro, RedSiren, Tuner,
};
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=../shared");

    let mut gen = TypeGen::new();

    gen.register_type::<Activity>().expect("register activity");
    gen.register_type::<Line>().expect("register line");
    gen.register_type::<Rect>().expect("register rect");
    gen.register_type::<Config>().expect("register config");
    gen.register_type::<Layout>().expect("register layout");

    gen.register_type::<IntroEV>().expect("register intro ev");
    gen.register_type::<TunerEV>().expect("register tuner ev");
    gen.register_type::<InstrumentEV>().expect("register instrument ev");
    
    
    gen.register_app::<RedSiren>().expect("register RedSiren");

    let output_root = PathBuf::from("./generated");

    gen.swift("SharedTypes", output_root.join("swift"))
        .expect("swift type gen failed");

    gen.java("com.anvlkv.redsiren.shared_types", output_root.join("java"))
        .expect("java type gen failed");

    gen.typescript("shared_types", output_root.join("typescript"))
        .expect("typescript type gen failed");
}
