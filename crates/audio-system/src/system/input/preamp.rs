use fundsp::prelude::*;

/*
Tuner input gain calibration

Given sparse calibration points (frequency -> dB offset), provides:
- gain_db(f): interpolated dB gain
- gain_linear(f): linear amplitude multiplier

Interpolation is piecewise-linear in log-frequency space (perceptually smoother).

Calibration table (Hz -> dB):
20      -> -2.7
100     -> +0.7
1000    -> +0.5
10000   -> +3.5

Frequencies outside table range are clamped to nearest endpoint.
*/

pub type PreampType<S> = Pipe<
    Pipe<
        Pipe<
            Pipe<
                DCBlock<S>,
                Bus<
                    Bus<
                        Bus<
                            Bus<FixedSvf<S, BellMode<S>>, FixedSvf<S, BellMode<S>>>,
                            FixedSvf<S, BellMode<S>>,
                        >,
                        FixedSvf<S, BellMode<S>>,
                    >,
                    Pass,
                >,
            >,
            Binop<FrameMul<U1>, MultiPass<U1>, Constant<U1>>,
        >,
        Stack<Stack<Pass, Var>, Var>,
    >,
    super::new_york::NewYork<S>,
>;

pub fn create_sensors_preamp<S>(ny_threshold: An<Var>, ny_wet_ratio: An<Var>) -> An<PreampType<S>>
where
    S: Real + Float,
{
    // Calibration points from the tuner input gain calibration table
    // Converting dB to linear amplitude: gain = 10^(dB/20)

    // Frequency calibration points and their gains
    let freq_20hz = S::from_f32(20.0);
    let gain_20hz_db = S::from_f32(-2.7);
    let gain_20hz_linear = db_amp(gain_20hz_db);

    let freq_100hz = S::from_f32(100.0);
    let gain_100hz_db = S::from_f32(0.7);
    let gain_100hz_linear = db_amp(gain_100hz_db);

    let freq_1khz = S::from_f32(1000.0);
    let gain_1khz_db = S::from_f32(0.5);
    let gain_1khz_linear = db_amp(gain_1khz_db);

    let freq_10khz = S::from_f32(10000.0);
    let gain_10khz_db = S::from_f32(3.5);
    let gain_10khz_linear = db_amp(gain_10khz_db);

    // Calculate Q factors for non-overlapping bands
    // Given frequency ratios: 100/20=5, 1000/100=10, 10000/1000=10
    // For wide frequency spacing, we use moderate Q values to ensure
    // focused correction without excessive overlap or gaps

    // Q calculation based on frequency ratios:
    // For ratio r=5: Q_optimal = sqrt(5)/(5-1) ≈ 0.56
    // For ratio r=10: Q_optimal = sqrt(10)/(10-1) ≈ 0.35
    //
    // However, these very low Q values would create extremely wide bands.
    // For better frequency selectivity with our wide spacing, we use Q ≈ 1.5-2.0
    // This provides focused correction while avoiding overlap between bands.

    let q_20hz = S::from_f32(1.5); // Lower Q for bass frequencies (wider band)
    let q_100hz = S::from_f32(1.8); // Slightly higher Q
    let q_1khz = S::from_f32(2.0); // Standard Q for midrange
    let q_10khz = S::from_f32(2.0); // Consistent Q for high frequencies

    // Create cascaded bell filters for frequency-selective amplification
    // Each bell filter applies gain at its center frequency with specified Q
    dcblock::<S>()
        >> (bell_hz::<S>(freq_20hz, q_20hz, gain_20hz_linear)
            & bell_hz::<S>(freq_100hz, q_100hz, gain_100hz_linear)
            & bell_hz::<S>(freq_1khz, q_1khz, gain_1khz_linear)
            & bell_hz::<S>(freq_10khz, q_10khz, gain_10khz_linear)
            & pass())
        >> mul(0.5)
        >> (pass() | ny_threshold | ny_wet_ratio)
        >> super::new_york::new_york::<S>()
}

#[cfg(test)]
mod tests {
    use common::tuner::Config;

    use super::*;

    #[test]
    fn test_preamp_creation() {
        let cfg = Config::default();
        let threshold = shared(cfg.ny_threshold);
        let wet_ratio = shared(cfg.ny_wet_ratio);

        let mut preamp = create_sensors_preamp::<f32>(var(&threshold), var(&wet_ratio));

        // Test that preamp has correct I/O configuration
        assert_eq!(preamp.inputs(), 1);
        assert_eq!(preamp.outputs(), 1);

        // Test sample rate setting
        preamp.set_sample_rate(48000.0);

        // Test processing doesn't panic
        let input = Frame::from_slice(&[0.5f32]);
        let output = preamp.tick(input);

        // Output should be modified by the preamp filters
        assert!(output[0].is_finite());
    }
}
