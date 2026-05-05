use fundsp::{numeric_array::ArrayLength, prelude::*};

const MIXER_ID: u64 = crate::util::hash_str(concat!(module_path!(), "::Mixer"));

/// 1/√2 ≈ 0.707 — used for phantom-center and M/S decomposition.
const INV_SQRT_2: f32 = std::f32::consts::FRAC_1_SQRT_2;

#[derive(Clone)]
pub struct Mixer<
    S: Float + Real + 'static,
    I: Size<S> + ArrayLength + Send + Sync,
    O: Size<S> + ArrayLength + Send + Sync,
>(std::marker::PhantomData<(S, I, O)>);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum NumChannels {
    Mono = 1,
    Stereo = 2,
    Surround3 = 3,
    Surround4 = 4,
    Surround5 = 5,
    Surround5_1 = 6,
    Surround7_1 = 8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Channel {
    FrontLeft,
    FrontRight,
    Center,
    SideLeft,
    SideRight,
    RearLeft,
    RearRight,
    SubWoofer,
}

impl NumChannels {
    fn layout(&self) -> &[Channel] {
        match self {
            NumChannels::Mono => &[Channel::Center],
            NumChannels::Stereo => &[Channel::FrontLeft, Channel::FrontRight],
            NumChannels::Surround3 => &[Channel::FrontLeft, Channel::FrontRight, Channel::Center],
            NumChannels::Surround4 => &[
                Channel::FrontLeft,
                Channel::Center,
                Channel::FrontRight,
                Channel::Center,
            ],
            NumChannels::Surround5 => &[
                Channel::FrontLeft,
                Channel::FrontRight,
                Channel::Center,
                Channel::RearLeft,
                Channel::RearRight,
            ],
            NumChannels::Surround5_1 => &[
                Channel::FrontLeft,
                Channel::Center,
                Channel::FrontRight,
                Channel::RearLeft,
                Channel::RearRight,
                Channel::SubWoofer,
            ],
            NumChannels::Surround7_1 => &[
                Channel::FrontLeft,
                Channel::Center,
                Channel::FrontRight,
                Channel::SideLeft,
                Channel::SideRight,
                Channel::RearLeft,
                Channel::RearRight,
                Channel::SubWoofer,
            ],
        }
    }

    fn channel_index(&self, channel: &Channel) -> Option<usize> {
        let layout = self.layout();
        layout.iter().position(|ch| ch == channel)
    }
}

impl From<isize> for NumChannels {
    fn from(value: isize) -> Self {
        match value {
            1 => Self::Mono,
            2 => Self::Stereo,
            3 => Self::Surround3,
            4 => Self::Surround4,
            5 => Self::Surround5,
            6 => Self::Surround5_1,
            8 => Self::Surround7_1,
            _ => panic!("Unsupported number of channels: {}", value),
        }
    }
}

impl<
        S: Float + Real + 'static,
        I: Size<S> + ArrayLength + Send + Sync,
        O: Size<S> + ArrayLength + Send + Sync,
    > AudioNode for Mixer<S, I, O>
{
    const ID: u64 = MIXER_ID;

    type Inputs = I;

    type Outputs = O;

    fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        let mut output = Frame::<f32, Self::Outputs>::default();

        match (NumChannels::from(I::ISIZE), NumChannels::from(O::ISIZE)) {
            (a, b) if a == b => {
                // Pass through
                output.clone_from_slice(input.as_slice());
            }
            (a, b) if a > b => {
                // Down-mix: copy directly-matched channels and normalise by contributor count
                // for energy preservation (e.g. stereo → mono produces (L+R)/2 not just L).
                let mut contributor_count = [0u8; 8];
                for ch in a.layout() {
                    if let Some((at_b, at_a)) = b.channel_index(ch).zip(a.channel_index(ch)) {
                        output[at_b] += input[at_a];
                        contributor_count[at_b] += 1;
                    }
                }
                // Normalise multi-contributor slots.
                for i in 0..O::ISIZE as usize {
                    if contributor_count[i] > 1 {
                        output[i] /= contributor_count[i] as f32;
                    }
                }
                // For output slots that received no direct match (e.g. Center in a Mono output
                // when the input is Stereo), fold all inputs with energy preservation.
                let num_inputs = I::ISIZE as usize;
                let input_avg: f32 =
                    (0..num_inputs).map(|j| input[j]).sum::<f32>() / num_inputs as f32;
                for i in 0..O::ISIZE as usize {
                    if contributor_count[i] == 0 {
                        output[i] = input_avg;
                    }
                }
            }
            (a, b) => {
                // Up-mix using a Mid/Side spatial spread.
                //
                // The stereo input is decomposed into:
                //   M (mid)  = (FL + FR) × 1/√2  — phantom center / mono sum
                //   S (side) = (FL − FR) × 1/√2  — stereo width / decorrelation signal
                //
                // Output channel assignment:
                //   FrontLeft  → FL  (direct)
                //   FrontRight → FR  (direct)
                //   Center     → M
                //   SideLeft   → +S
                //   SideRight  → −S   (anti-phase for decorrelation)
                //   RearLeft   → M×0.5 + S×0.25
                //   RearRight  → M×0.5 − S×0.25
                //   SubWoofer  → M×0.5
                let fl = input[a.channel_index(&Channel::FrontLeft).unwrap_or(0)];
                let fr = input[a.channel_index(&Channel::FrontRight).unwrap_or(0)];
                let mid = (fl + fr) * INV_SQRT_2;
                let side = (fl - fr) * INV_SQRT_2;
                for ch in b.layout() {
                    if let Some(at_b) = b.channel_index(ch) {
                        output[at_b] = match ch {
                            Channel::FrontLeft => fl,
                            Channel::FrontRight => fr,
                            Channel::Center => mid,
                            Channel::SideLeft => side,
                            Channel::SideRight => -side,
                            Channel::RearLeft => mid * 0.5 + side * 0.25,
                            Channel::RearRight => mid * 0.5 - side * 0.25,
                            Channel::SubWoofer => mid * 0.5,
                        };
                    }
                }
            }
        }

        output
    }
}

pub fn create_mixer<
    S: Float + Real + 'static,
    I: Size<S> + ArrayLength + Send + Sync,
    O: Size<S> + ArrayLength + Send + Sync,
>() -> An<Mixer<S, I, O>> {
    An(Mixer(std::marker::PhantomData))
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta_fun::prelude::*;

    const SNAP_LEN: usize = 128;

    fn snapshot_config() -> SnapshotConfig {
        SnapshotConfigBuilder::default()
            .sample_rate(100.0)
            .num_samples(SNAP_LEN)
            .build()
            .unwrap()
    }

    /// Stereo input: left = sine, right = cosine (L ≠ R so M and S are non-trivial).
    fn stereo_input() -> InputSource {
        let left: Vec<f32> = (0..SNAP_LEN)
            .map(|i| (2.0 * std::f32::consts::PI * i as f32 / SNAP_LEN as f32).sin())
            .collect();
        let right: Vec<f32> = (0..SNAP_LEN)
            .map(|i| (2.0 * std::f32::consts::PI * i as f32 / SNAP_LEN as f32).cos())
            .collect();
        InputSource::VecByChannel(vec![left, right])
    }

    #[test]
    fn mixer_stereo_to_mono() {
        assert_audio_unit_snapshot!(
            "mixer_stereo_to_mono",
            create_mixer::<f32, U2, U1>(),
            stereo_input(),
            snapshot_config()
        );
    }

    #[test]
    fn mixer_stereo_passthru() {
        assert_audio_unit_snapshot!(
            "mixer_stereo_passthru",
            create_mixer::<f32, U2, U2>(),
            stereo_input(),
            snapshot_config()
        );
    }

    #[test]
    fn mixer_stereo_to_3ch() {
        assert_audio_unit_snapshot!(
            "mixer_stereo_to_3ch",
            create_mixer::<f32, U2, U3>(),
            stereo_input(),
            snapshot_config()
        );
    }

    #[test]
    fn mixer_stereo_to_4ch() {
        assert_audio_unit_snapshot!(
            "mixer_stereo_to_4ch",
            create_mixer::<f32, U2, U4>(),
            stereo_input(),
            snapshot_config()
        );
    }

    #[test]
    fn mixer_stereo_to_5ch() {
        assert_audio_unit_snapshot!(
            "mixer_stereo_to_5ch",
            create_mixer::<f32, U2, U5>(),
            stereo_input(),
            snapshot_config()
        );
    }

    #[test]
    fn mixer_stereo_to_5_1() {
        assert_audio_unit_snapshot!(
            "mixer_stereo_to_5_1",
            create_mixer::<f32, U2, U6>(),
            stereo_input(),
            snapshot_config()
        );
    }

    #[test]
    fn mixer_stereo_to_7_1() {
        assert_audio_unit_snapshot!(
            "mixer_stereo_to_7_1",
            create_mixer::<f32, U2, U8>(),
            stereo_input(),
            snapshot_config()
        );
    }
}
