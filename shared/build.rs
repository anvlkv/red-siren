use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    generate_instrument_config_consts_file().expect("Failed to generate consts");
}

fn generate_instrument_config_consts_file() -> Result<(), Box<dyn std::error::Error>> {
    // Frequency bounds
    const MIN_FREQ_HZ: f32 = 20.0;
    const SOFT_MIN_FREQ_HZ: f32 = 60.0;
    const SOFT_MAX_FREQ_HZ: f32 = 14_000.0;
    const MAX_FREQ_HZ: f32 = 20_000.0;

    // Volume bounds
    const MAX_DBS: usize = 70;

    // Inspiring values
    /// `#e30022`
    /// Crimson Red
    ///
    /// Wave length in nanometers
    pub const CRIMSON_RED_WAVE_LENGTH_NM: f32 = 664.1;
    /// `#e44d2e`
    /// Cinnabar
    ///
    /// Wave length in nanometers
    pub const CINNABAR_RED_WAVE_LENGTH_NM: f32 = 659.5;
    /// Speed of light in m/s
    const SPEED_OF_LIGHT_M_S: f32 = 299_792_458.0;
    /// Speed of sound in m/s
    const SPEED_OF_SOUND_M_S: f32 = 340.29;

    // Get the output directory
    let out_dir = env::var_os("OUT_DIR").ok_or("No OUT_DIR")?;
    let dest_path = Path::new(&out_dir).join("instrument_constants_gen.rs");

    // Generate the constants file
    let mut f = File::create(&dest_path)?;

    writeln!(f, "// Auto-generated constants from build.rs")?;

    writeln!(f, "pub const MIN_FREQ_HZ: f32 = {MIN_FREQ_HZ:?};")?;
    writeln!(f, "pub const SOFT_MIN_FREQ_HZ: f32 = {SOFT_MIN_FREQ_HZ:?};")?;
    writeln!(f, "pub const SOFT_MAX_FREQ_HZ: f32 = {SOFT_MAX_FREQ_HZ:?};")?;
    writeln!(f, "pub const MAX_FREQ_HZ: f32 = {MAX_FREQ_HZ:?};")?;
    writeln!(f, "pub const MAX_DBS: usize = {MAX_DBS:?};")?;
    
    

    Ok(())
}
