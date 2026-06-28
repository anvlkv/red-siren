use crate::dsp::siren::chamber::Chamber;

#[derive(Clone, Copy, Debug)]
pub struct SirenConfig {
    resolution: i32,
    n_chambers: i32,
    fib_n_start: i32,
    base_opening_width: i32,
}

impl SirenConfig {
    pub fn chambers(&self) -> Vec<Chamber> {
        let resolution = self.resolution as usize;
        let n_chambers = self.n_chambers as usize;

        let initial_space_samples = resolution / n_chambers;

        (0..n_chambers)
            .map(|n| {
                let opening_width = self.nth_chamber_opening_width(n as i32) as usize;
                let x = self.nth_chamber_x_openings(n as i32) as usize;
                let gap_width = (resolution - opening_width * x) / x;
                let initial_window_size = opening_width;
                Chamber::new(
                    resolution,
                    (n, n_chambers),
                    n.is_multiple_of(2),
                    initial_window_size as u32,
                    initial_space_samples as u32,
                    opening_width,
                    gap_width,
                )
            })
            .collect()
    }

    pub fn configs_for_resolution(resolution: u32) -> Vec<Self> {
        let resolution_i = resolution as i32;

        let mut configs = Vec::new();

        let fibs = (1..resolution_i)
            .map(|n| (fibonacci(n as u32) as i32, n))
            .take_while(|(f, _)| *f <= resolution_i)
            .collect::<Vec<_>>();

        let n_chambers = fibs.len() as i32;

        for (f, n) in fibs {
            for bow in 1..=resolution_i / f {
                let config = Self {
                    resolution: resolution_i,
                    n_chambers: n_chambers - n,
                    fib_n_start: n,
                    base_opening_width: bow,
                };

                if config.is_valid() {
                    configs.push(config);
                }
            }
        }

        configs
    }

    /// Configs are scored based on the following criteria:
    ///
    ///
    pub fn score(&self) -> f32 {
        let chamber_scores = (0..self.n_chambers)
            .map(|n| {
                let x_openings = self.nth_chamber_x_openings(n);
                let opening_width = self.nth_chamber_opening_width(n);
                let space = self.resolution - opening_width * x_openings;
                let gap_width = space / x_openings;

                (opening_width as f32 / gap_width as f32) * x_openings as f32
                    + if opening_width % 2 == 0 { 0.0 } else { 1.0 }
            })
            .sum::<f32>()
            / self.n_chambers as f32;

        (self.fib_n_start as f32).powf(chamber_scores)
    }

    fn nth_chamber_x_openings(&self, n: i32) -> i32 {
        fibonacci((self.fib_n_start + n) as u32) as i32
    }

    fn nth_chamber_opening_width(&self, n: i32) -> i32 {
        let fib = fibonacci((self.fib_n_start + n) as u32) as i32;
        self.base_opening_width / fib
    }

    fn is_valid(&self) -> bool {
        if self.resolution <= 0
            || self.n_chambers <= 0
            || self.base_opening_width <= 0
            || self.fib_n_start <= 0
        {
            return false;
        }

        let mut previous_opening_width = 0;
        for n in 0..self.n_chambers {
            let x_openings = self.nth_chamber_x_openings(n);

            let opening_width = self.nth_chamber_opening_width(n);

            if opening_width == previous_opening_width || opening_width <= 0 || x_openings <= 0 {
                return false;
            }
            previous_opening_width = opening_width;

            let space = self.resolution - opening_width * x_openings;
            let gap_width = space / x_openings;

            if space <= 0 || gap_width <= 0 {
                return false;
            }

            if self
                .resolution
                .checked_sub((opening_width + gap_width) * x_openings)
                .is_none_or(|d| d < 0)
            {
                return false;
            }
        }

        true
    }
}

const fn fibonacci(n: u32) -> u32 {
    match n {
        0 => 0,
        1 => 1,
        _ => fibonacci(n - 1) + fibonacci(n - 2),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn test_configs_for_resolution() {
        let resolution = 576;
        let configs = SirenConfig::configs_for_resolution(resolution);
        assert!(!configs.is_empty());
    }

    #[test]
    fn test_configs_for_primes() {
        let mut configs = [
            2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83,
            89, 97, 101, 103, 107, 109, 113, 127, 131, 137, 139, 149, 151, 157, 163, 167, 173, 179,
            181, 191, 193, 197, 199, 211, 223, 227, 229, 233, 239, 241, 251, 257, 263, 269, 271,
            277, 281, 283, 293, 307, 311, 313, 317, 331, 337, 347, 349, 353, 359, 367, 373, 379,
            383, 389, 397, 401, 409, 419, 421, 431, 433, 439, 443, 449, 457, 461, 463, 467, 479,
            487, 491, 499, 503, 509, 521, 523, 541, 547, 557, 563, 569, 571, 577, 587, 593, 599,
            601, 607, 613, 617, 619, 631, 641, 643, 647, 653, 659, 661, 673, 677, 683, 691, 701,
            709, 719, 727, 733, 739, 743, 751, 757, 761, 769, 773, 787, 797, 809, 811, 821, 823,
            827, 829, 839, 853, 857, 859, 863, 877, 881, 883, 887, 907, 911, 919, 929, 937, 941,
            947, 953, 967, 971, 977, 983, 991, 997,
        ]
        .into_iter()
        .flat_map(SirenConfig::configs_for_resolution)
        .collect::<Vec<_>>();

        configs.sort_by(|a, b| {
            a.score()
                .partial_cmp(&b.score())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let resolutions: BTreeSet<_> = BTreeSet::from_iter(configs.iter().map(|c| c.resolution));

        println!(
            "Configs for prime resolutions: {}. Resolutions: {:?}",
            configs.len(),
            resolutions
        );

        println!(
            "Top 10 configs by score: {:#?}",
            configs.iter().rev().take(10).collect::<Vec<_>>()
        );

        println!(
            "Bottom 10 configs by score: {:#?}",
            configs.iter().take(10).collect::<Vec<_>>()
        );

        println!(
            "Max by fib_n_start: {:#?}",
            configs.iter().max_by_key(|c| c.fib_n_start)
        );

        assert!(!configs.is_empty());
    }

    #[test]
    fn test_configs_for_powers_of_two() {
        let mut configs = (1..=10)
            .map(|n| 2_u32.pow(n))
            .flat_map(SirenConfig::configs_for_resolution)
            .collect::<Vec<_>>();

        configs.sort_by(|a, b| {
            a.score()
                .partial_cmp(&b.score())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let resolutions: BTreeSet<_> = BTreeSet::from_iter(configs.iter().map(|c| c.resolution));

        println!(
            "Configs for power-of-two resolutions: {}. Resolutions: {:?}",
            configs.len(),
            resolutions
        );

        println!(
            "Top 10 configs by score: {:#?}",
            configs.iter().rev().take(10).collect::<Vec<_>>()
        );

        println!(
            "Bottom 10 configs by score: {:#?}",
            configs.iter().take(10).collect::<Vec<_>>()
        );

        println!(
            "Max by fib_n_start: {:#?}",
            configs.iter().max_by_key(|c| c.fib_n_start)
        );

        assert!(!configs.is_empty());
    }

    // #[test]
    // fn test_configs_delta_rate() {
    //     let config1 = SirenConfig {
    //         resolution: 8,
    //         n_chambers: 2,
    //         nth_chamber_opening_increment: 1,
    //         nth_openings_increment: 1,
    //         base_m_openings: 1,
    //         base_opening: 1,
    //     };

    //     assert!(config1.is_valid(), "config1 is not valid");

    //     let config2 = SirenConfig {
    //         resolution: 8,
    //         n_chambers: 2,
    //         nth_chamber_opening_increment: 2,
    //         nth_openings_increment: 1,
    //         base_m_openings: 1,
    //         base_opening: 1,
    //     };

    //     assert!(config2.is_valid(), "config2 is not valid");

    //     let score_delta = config2.score() - config1.score();
    //     assert_eq!(score_delta, 0.3125);
    // }

    // #[test]
    // fn test_is_valid() {
    //     let valid_config = SirenConfig {
    //         resolution: 8,
    //         n_chambers: 2,
    //         nth_chamber_opening_increment: 1,
    //         nth_openings_increment: 1,
    //         base_m_openings: 1,
    //         base_opening: 1,
    //     };
    //     assert!(valid_config.is_valid());
    // }
}
