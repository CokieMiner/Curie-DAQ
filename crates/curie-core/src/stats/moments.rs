//! Numerically stable running sample moments (Welford's algorithm) in `f64`.

use crate::F64Math;

/// Running mean and variance of a sample, updated one value at a time.
///
/// Uses Welford's algorithm so the variance stays accurate even when the mean is large
/// compared with the spread (for example 10 000 ± 100 counts per gate), which the naive
/// `sum(x²) - sum(x)²/n` formula cannot guarantee. Start from `SampleMoments::default()`.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SampleMoments {
    /// Number of observations (exact as `f64` up to 2^53).
    pub count: f64,
    /// Sample mean, or zero when empty.
    pub mean: f64,
    /// Sum of squared deviations from the running mean (Welford's `M2`).
    pub sum_squared_deviations: f64,
}

impl SampleMoments {
    /// Adds one observation.
    pub fn push(&mut self, value: f64) {
        self.count += 1.0_f64;
        let delta = value - self.mean;
        self.mean += delta / self.count;
        let delta_after = value - self.mean;
        self.sum_squared_deviations += delta * delta_after;
    }

    /// Unbiased sample variance (divides by `n - 1`), or zero with fewer than two values.
    #[must_use]
    pub fn variance(&self) -> f64 {
        if self.count < 2.0_f64 {
            0.0_f64
        } else {
            (self.sum_squared_deviations / (self.count - 1.0_f64)).max(0.0_f64)
        }
    }

    /// Standard error of the mean, `s / sqrt(n)`, or zero when undefined.
    #[must_use]
    pub fn std_error_of_mean(&self) -> f64 {
        if self.count < 2.0_f64 {
            0.0_f64
        } else {
            F64Math::sqrt(self.variance() / self.count)
        }
    }

    /// Fano factor `variance / mean` (1 for a Poisson process), or `None` when undefined.
    #[must_use]
    pub fn fano_factor(&self) -> Option<f64> {
        if self.count < 2.0_f64 || self.mean <= 0.0_f64 {
            None
        } else {
            Some(self.variance() / self.mean)
        }
    }
}
