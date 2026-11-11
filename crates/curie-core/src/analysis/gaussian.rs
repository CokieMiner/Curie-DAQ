//! Maximum-likelihood Gaussian fit of counts per gate.

use alloc::vec::Vec;

use crate::{F64Math, IntegerHistogram, SampleMoments};

use super::goodness::GoodnessOfFit;

/// `2 sqrt(2 ln 2)`: FWHM of a Gaussian in units of sigma.
const FWHM_PER_SIGMA: f64 = 2.354_820_045_030_949_3_f64;

/// Gaussian fit of a series of counts per gate.
///
/// For a normal model the maximum-likelihood estimates are the sample mean and standard
/// deviation, so they are computed directly from the raw counts (no binning bias). The
/// chi-squared test integrates the Gaussian over each integer bin with a continuity
/// correction (`[low - 0.5, high + 0.5]`), with the tails folded into the end bins.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct GaussianFit {
    /// Number of gates.
    pub samples: f64,
    /// Mean count `mu`.
    pub mean: f64,
    /// Standard error of the mean, `s / sqrt(n)`.
    pub mean_error: f64,
    /// Standard deviation `sigma` (unbiased sample estimate).
    pub sigma: f64,
    /// Approximate standard error of `sigma`, `s / sqrt(2 (n - 1))`.
    pub sigma_error: f64,
    /// Full width at half maximum, `2 sqrt(2 ln 2) sigma`.
    pub fwhm: f64,
    /// Standard error of the FWHM.
    pub fwhm_error: f64,
    /// Expected frequency of every histogram bin (tails folded into the end bins).
    pub expected_per_bin: Vec<f64>,
    /// Pearson chi-squared against the histogram (two fitted parameters), if computable.
    pub goodness: Option<GoodnessOfFit>,
}

impl GaussianFit {
    /// Fits a Gaussian to raw counts and tests it on `histogram`.
    ///
    /// `histogram` should be built from the same counts. Returns `None` with fewer than two
    /// counts, zero spread, or when allocation fails.
    #[must_use]
    pub fn from_counts(counts: &[u32], histogram: &IntegerHistogram) -> Option<Self> {
        let mut moments = SampleMoments::default();
        for &count in counts {
            moments.push(f64::from(count));
        }
        let sigma = F64Math::sqrt(moments.variance());
        if moments.count < 2.0_f64 || sigma <= 0.0_f64 {
            return None;
        }
        let sigma_error = sigma / F64Math::sqrt(2.0_f64 * (moments.count - 1.0_f64));

        let bins = histogram.frequencies.len();
        let mut probabilities = Vec::new();
        probabilities.try_reserve_exact(bins).ok()?;
        for index in 0..bins {
            let bin = histogram.bin(index)?;
            let lower = if index == 0_usize {
                0.0_f64
            } else {
                F64Math::normal_cdf((f64::from(bin.low) - 0.5_f64 - moments.mean) / sigma)
            };
            let upper = if index.saturating_add(1_usize) == bins {
                1.0_f64
            } else {
                F64Math::normal_cdf((f64::from(bin.high) + 0.5_f64 - moments.mean) / sigma)
            };
            probabilities.push(upper - lower);
        }

        let goodness = GoodnessOfFit::pearson(histogram, &probabilities, 2_u32);
        for probability in &mut probabilities {
            *probability *= moments.count;
        }
        Some(Self {
            samples: moments.count,
            mean: moments.mean,
            mean_error: moments.std_error_of_mean(),
            sigma,
            sigma_error,
            fwhm: FWHM_PER_SIGMA * sigma,
            fwhm_error: FWHM_PER_SIGMA * sigma_error,
            expected_per_bin: probabilities,
            goodness,
        })
    }
}
