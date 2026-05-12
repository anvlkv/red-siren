use mint::Point2;

use super::{NodePhysics, MASS_VOLUME_SAMPLES};

impl NodePhysics {
    fn revolved_solid_volume_m3<F>(&self, num_samples: usize, profile_point: F) -> f64
    where
        F: Fn(Point2<f64>, Point2<f64>) -> Point2<f64>,
    {
        let samples = num_samples.max(1);
        let (prev_outer, prev_inner) = self.sample_outer_inner_at(0.0);
        let mut prev_profile = profile_point(prev_outer, prev_inner);
        let mut volume_m3 = 0.0;

        for i in 1..=samples {
            let u = Self::u_at_index(i, samples);
            let (outer, inner) = self.sample_outer_inner_at(u);
            let profile = profile_point(outer, inner);
            let dy = profile.y - prev_profile.y;
            let prev_area = std::f64::consts::PI * prev_profile.x.max(0.0).powi(2);
            let area = std::f64::consts::PI * profile.x.max(0.0).powi(2);

            volume_m3 += 0.5 * (prev_area + area) * dy;

            prev_profile = profile;
        }

        volume_m3.abs()
    }

    /// Computes the volume of the node's inner cavity in cubic meters, excluding material thickness.
    ///
    /// Takes into account the fact that inner profile is only 1/2 of the cross-section and the node is revolved around the y-axis.
    pub fn inner_volume_m3(&self, num_samples: usize) -> f64 {
        self.revolved_solid_volume_m3(num_samples, |_, inner| inner)
    }

    /// Computes the volume of the node's material in cubic meters.
    ///
    /// Takes into account the fact that inner profile is only 1/2 of the cross-section and the node is revolved around the y-axis.
    ///
    /// Only accounts for the actual material ie thickness.
    pub fn material_volume_m3(&self, num_samples: usize) -> f64 {
        let outer_volume_m3 = self.revolved_solid_volume_m3(num_samples, |outer, _| outer);
        (outer_volume_m3 - self.inner_volume_m3(num_samples)).max(0.0)
    }

    pub fn mass_kg(&self) -> f64 {
        self.material_density_kg_per_m3.max(0.0) * self.material_volume_m3(MASS_VOLUME_SAMPLES)
    }
}
