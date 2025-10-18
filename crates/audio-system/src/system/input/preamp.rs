use fundsp::hacker32::prelude::*;

use crate::util::S;

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

pub type PreampType = Pipe<
    Pipe<
        Pipe<
            Pipe<DCBlock<S>, Split<U5>>,
            Stack<
                Stack<
                    Stack<
                        Stack<FixedSvf<S, BellMode<S>>, FixedSvf<S, BellMode<S>>>,
                        FixedSvf<S, BellMode<S>>,
                    >,
                    FixedSvf<S, BellMode<S>>,
                >,
                Pass,
            >,
        >,
        Join<U5>,
    >,
    Binop<FrameMul<U1>, MultiPass<U1>, Constant<U1>>,
>;

pub fn create_sensors_preamp() -> An<PreampType> {
    // Calibration points from the tuner input gain calibration table
    // Converting dB to linear amplitude: gain = 10^(dB/20)

    // Frequency calibration points and their gains
    let freq_20hz = 20.0;
    let gain_20hz_db = -2.7;
    let gain_20hz_linear = 10.0_f32.powf(gain_20hz_db / 20.0);

    let freq_100hz = 100.0;
    let gain_100hz_db = 0.7;
    let gain_100hz_linear = 10.0_f32.powf(gain_100hz_db / 20.0);

    let freq_1khz = 1000.0;
    let gain_1khz_db = 0.5;
    let gain_1khz_linear = 10.0_f32.powf(gain_1khz_db / 20.0);

    let freq_10khz = 10000.0;
    let gain_10khz_db = 3.5;
    let gain_10khz_linear = 10.0_f32.powf(gain_10khz_db / 20.0);

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

    let q_20hz = 1.5; // Lower Q for bass frequencies (wider band)
    let q_100hz = 1.8; // Slightly higher Q
    let q_1khz = 2.0; // Standard Q for midrange
    let q_10khz = 2.0; // Consistent Q for high frequencies

    // Create cascaded bell filters for frequency-selective amplification
    // Each bell filter applies gain at its center frequency with specified Q
    dcblock()
        >> split::<U5>()
        >> (bell_hz(freq_20hz, q_20hz, gain_20hz_linear)
            | bell_hz(freq_100hz, q_100hz, gain_100hz_linear)
            | bell_hz(freq_1khz, q_1khz, gain_1khz_linear)
            | bell_hz(freq_10khz, q_10khz, gain_10khz_linear)
            | pass())
        >> join::<U5>()
        >> mul(1.0 / 5.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preamp_creation() {
        let mut preamp = create_sensors_preamp();

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
