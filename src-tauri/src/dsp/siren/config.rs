use std::iter;

use const_primes::Primes;

use crate::dsp::siren::chamber::Chamber;

pub const N_CHAMBERS_PRIMES: Primes<20> = Primes::new();

#[derive(Clone, Copy, Debug)]
pub struct SirenConfig {
    pub resolution: u32,
    pub n_chambers: u32,
}

impl SirenConfig {
    pub fn from_resolution(resolution: u32) -> Option<Self> {
        let fibs = iter::from_fn({
            let mut nth_fib = 1;
            move || {
                let result = fibonacci(nth_fib);
                if result > resolution {
                    None
                } else {
                    nth_fib += 1;
                    Some(result)
                }
            }
        })
        .collect::<Vec<_>>();

        if fibs.is_empty() {
            return None;
        }

        let prime_count = N_CHAMBERS_PRIMES
            .into_iter()
            .take_while(|p| *p as usize <= fibs.len())
            .last()?;

        let primed_fibs = fibs[..prime_count as usize].to_vec();

        Some(Self {
            resolution,
            n_chambers: primed_fibs.len() as u32,
        })
    }

    pub fn chambers(&self) -> Vec<Chamber> {
        let mut is_left_channel = false;
        (1..=self.n_chambers)
            .map(|n| {
                let fib = fibonacci(n);
                let is_prime = N_CHAMBERS_PRIMES.is_prime(n).unwrap_or_default();
                let modeling_space = self.resolution / fib;
                let remainder = self.resolution % fib;

                let opening = if is_prime {
                    is_left_channel = !is_left_channel;

                    modeling_space.isqrt()
                } else {
                    modeling_space / 2
                };
                let gap = modeling_space - opening;

                Chamber::new(
                    (self.resolution - remainder) as usize,
                    is_left_channel,
                    opening,
                    opening as usize,
                    gap as usize,
                )
            })
            .collect()
    }

    // pub fn configs_for_resolution(resolution: u32) -> Vec<Self> {
    //     todo!()
    // }

    // pub fn score(&self) -> f32 {
    //     todo!()
    // }

    // fn nth_chamber_x_openings(&self, n: i32) -> i32 {
    //     todo!()
    // }

    // fn nth_chamber_opening_width(&self, n: i32) -> i32 {
    //     todo!()
    // }

    // fn is_valid(&self) -> bool {
    //     todo!()
    // }
}

const fn fibonacci(n: u32) -> u32 {
    match n {
        0 => 0,
        1 => 1,
        _ => fibonacci(n - 1) + fibonacci(n - 2),
    }
}

#[cfg(test)]
mod tests {}
