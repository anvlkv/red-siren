use serde::{Deserialize, Serialize};

use crate::Segment;

use super::MODAL_EPSILON;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ShapeProfileBuilder {
    pub height_m: f64,
    pub radius_0_m: f64,
    pub radius_1_m: f64,
    pub radius_2_m: f64,
    pub radius_3_m: f64,
    pub radius_4_m: f64,
    pub radius_5_m: f64,
    pub profile_bias: f64,
    pub segment_bias_0: f64,
    pub segment_bias_1: f64,
    pub segment_bias_2: f64,
    pub segment_bias_3: f64,
    pub segment_bias_4: f64,
    pub sampling_density: f64,
}

impl Default for ShapeProfileBuilder {
    fn default() -> Self {
        Self {
            height_m: 0.75,
            radius_0_m: 0.06,
            radius_1_m: 0.085,
            radius_2_m: 0.16,
            radius_3_m: 0.185,
            radius_4_m: 0.215,
            radius_5_m: 0.22,
            profile_bias: 0.0,
            segment_bias_0: 0.0,
            segment_bias_1: 0.0,
            segment_bias_2: 0.0,
            segment_bias_3: 0.0,
            segment_bias_4: 0.0,
            sampling_density: 22.0,
        }
    }
}

impl ShapeProfileBuilder {
    pub fn build_profile(&self) -> Result<[Segment; 5], String> {
        let radii = self.tuned_target_radii();
        build_profile_from_targets(self.height_m, radii, self.sampling_density)
    }

    fn tuned_target_radii(&self) -> [f64; 6] {
        let base = [
            self.radius_0_m,
            self.radius_1_m,
            self.radius_2_m,
            self.radius_3_m,
            self.radius_4_m,
            self.radius_5_m,
        ];

        let profile_bias = self.profile_bias.clamp(-1.0, 1.0);
        let seg_bias = [
            self.segment_bias_0.clamp(-1.0, 1.0),
            self.segment_bias_1.clamp(-1.0, 1.0),
            self.segment_bias_2.clamp(-1.0, 1.0),
            self.segment_bias_3.clamp(-1.0, 1.0),
            self.segment_bias_4.clamp(-1.0, 1.0),
        ];

        let mut tuned = [0.0; 6];
        for (i, radius) in base.iter().enumerate() {
            let u = i as f64 / 5.0;
            let global_gain = 1.0 + profile_bias * (u - 0.5) * 0.35;
            let local = if i == 0 {
                seg_bias[0]
            } else if i == 5 {
                seg_bias[4]
            } else {
                (seg_bias[i - 1] + seg_bias[i]) * 0.5
            };
            let local_gain = 1.0 + local * 0.2;
            tuned[i] = (radius * global_gain * local_gain).max(MODAL_EPSILON);
        }

        tuned
    }
}

fn build_profile_from_targets(
    height_m: f64,
    radii: [f64; 6],
    sampling_density: f64,
) -> Result<[Segment; 5], String> {
    let height = height_m.max(0.2);
    let dt = height / 5.0;
    let density = sampling_density.max(2.0);
    let slope0 = (radii[1] - radii[0]) / dt.max(MODAL_EPSILON);

    let seg0 = Segment::start_line(dt, slope0, radii[0].max(MODAL_EPSILON))
        .map_err(|err| err.to_string())?;
    let seg1 = Segment::continue_parabolic(&seg0, dt * 2.0, radii[2].max(MODAL_EPSILON), density)
        .map_err(|err| err.to_string())?;
    let seg2 = Segment::continue_parabolic(&seg1, dt * 3.0, radii[3].max(MODAL_EPSILON), density)
        .map_err(|err| err.to_string())?;
    let seg3 = Segment::continue_parabolic(&seg2, dt * 4.0, radii[4].max(MODAL_EPSILON), density)
        .map_err(|err| err.to_string())?;
    let seg4 = Segment::continue_parabolic(&seg3, height, radii[5].max(MODAL_EPSILON), density)
        .map_err(|err| err.to_string())?;

    Ok([seg0, seg1, seg2, seg3, seg4])
}
