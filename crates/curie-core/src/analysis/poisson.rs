//! Maximum-likelihood Poisson fit of counts per gate.

use alloc::vec::Vec;

use crate::{F64Math, IntegerHistogram, SampleMoments};

use super::goodness::GoodnessOfFit;

/// Relative size below which a Poisson tail term stops the outward summation.
const TAIL_TOLERANCE: f64 = 1e-18_f64;

/// Poisson fit of a series of counts per gate.
///
/// The maximum-likelihood estimate of the Poisson mean is the sample mean, computed exactly
/// from the raw counts (not from histogram bin centres). The chi-squared test compares the
/// observed histogram with the expected frequencies summed over every integer in each bin.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct PoissonFit {
    /// Number of gates.
    pub samples: f64,
    /// Estimated Poisson mean `lambda` (counts per gate).
    pub lambda: f64,
    /// Standard error of `lambda`, `sqrt(lambda / n)`.
    pub lambda_error: f64,
    /// Unbiased sample variance of the counts (equals `lambda` for a Poisson process).
    pub sample_variance: f64,
    /// Fano factor `variance / mean`, if defined.
    pub fano_factor: Option<f64>,
    /// Expected frequency of every histogram bin (tails folded into the end bins).
    pub expected_per_bin: Vec<f64>,
    /// Pearson chi-squared against the histogram (one fitted parameter), if computable.
    pub goodness: Option<GoodnessOfFit>,
}

impl PoissonFit {
    /// Fits a Poisson distribution to raw counts and tests it on `histogram`.
    ///
    /// `histogram` should be built from the same counts. Returns `None` for no counts or
    /// when allocation fails.
    #[must_use]
    pub fn from_counts(counts: &[u32], histogram: &IntegerHistogram) -> Option<Self> {
        let mut moments = SampleMoments::default();
        for &count in counts {
            moments.push(f64::from(count));
        }
        if moments.count < 1.0_f64 {
            return None;
        }
        let lambda = moments.mean;

        let bins = histogram.frequencies.len();
        let mut probabilities = Vec::new();
        probabilities.try_reserve_exact(bins).ok()?;
        for index in 0..bins {
            let bin = histogram.bin(index)?;
            let mut probability = 0.0_f64;
            for k in bin.low..=bin.high {
                probability += F64Math::poisson_pmf(k, lambda);
            }
            if index == 0_usize {
                // P(X < low), summed downwards until the terms are negligible.
                let mut k = bin.low;
                while k > 0_u32 {
                    k = k.saturating_sub(1_u32);
                    let term = F64Math::poisson_pmf(k, lambda);
                    probability += term;
                    if f64::from(k) < lambda && term <= TAIL_TOLERANCE * probability {
                        break;
                    }
                }
            }
            if index.saturating_add(1_usize) == bins {
                // P(X > high), summed upwards until the terms are negligible.
                let mut k = bin.high;
                while k < u32::MAX {
                    k = k.saturating_add(1_u32);
                    let term = F64Math::poisson_pmf(k, lambda);
                    probability += term;
                    if f64::from(k) > lambda && term <= TAIL_TOLERANCE * probability {
                        break;
                    }
                }
            }
            probabilities.push(probability);
        }

        let goodness = GoodnessOfFit::pearson(histogram, &probabilities, 1_u32);
        for probability in &mut probabilities {
            *probability *= moments.count;
        }
        Some(Self {
            samples: moments.count,
            lambda,
            lambda_error: F64Math::sqrt(lambda / moments.count),
            sample_variance: moments.variance(),
            fano_factor: moments.fano_factor(),
            expected_per_bin: probabilities,
            goodness,
        })
    }
}
