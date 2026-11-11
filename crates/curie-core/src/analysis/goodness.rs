//! Pearson chi-squared goodness of fit on an integer histogram.

use crate::IntegerHistogram;

/// Smallest expected frequency a group of bins must reach before it enters the chi-squared
/// sum (the usual rule of thumb for Pearson's test).
const MIN_EXPECTED_PER_GROUP: f64 = 5.0_f64;

/// Pearson chi-squared summary of a fitted distribution against a histogram.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GoodnessOfFit {
    /// Pearson chi-squared `sum (O - E)^2 / E` over the merged bin groups.
    pub chi_squared: f64,
    /// Degrees of freedom: merged groups - 1 - estimated parameters (zero if undefined).
    pub degrees_of_freedom: u32,
    /// `chi_squared / degrees_of_freedom`, or `None` without degrees of freedom.
    pub reduced_chi_squared: Option<f64>,
    /// Number of bin groups after merging neighbours until each expects at least 5 events.
    pub merged_groups: u32,
}

impl GoodnessOfFit {
    /// Computes Pearson's chi-squared for `histogram` given the model probability of each bin.
    ///
    /// `bin_probabilities` must have one entry per histogram bin, with the model tails folded
    /// into the first and last bins so the entries sum to one. Adjacent bins are merged from
    /// the low end until each group expects at least five events; a short remainder at the
    /// high end is merged into the previous group. Returns `None` for an empty histogram or
    /// mismatched lengths.
    #[must_use]
    pub fn pearson(
        histogram: &IntegerHistogram,
        bin_probabilities: &[f64],
        fitted_parameters: u32,
    ) -> Option<Self> {
        if histogram.frequencies.is_empty()
            || bin_probabilities.len() != histogram.frequencies.len()
        {
            return None;
        }
        #[expect(
            clippy::as_conversions,
            clippy::cast_precision_loss,
            reason = "Observation counts stay far below 2^53"
        )]
        let total = histogram.total as f64;

        let mut chi_squared = 0.0_f64;
        let mut groups = 0_u32;
        let mut observed_acc = 0.0_f64;
        let mut expected_acc = 0.0_f64;
        // Last closed group, kept open so a short tail can be merged back into it.
        let mut last_group: Option<(f64, f64)> = None;

        for (&frequency, &probability) in histogram.frequencies.iter().zip(bin_probabilities.iter())
        {
            observed_acc += f64::from(frequency);
            expected_acc += total * probability.max(0.0_f64);
            if expected_acc >= MIN_EXPECTED_PER_GROUP {
                if let Some((observed, expected)) = last_group.take() {
                    chi_squared += (observed - expected) * (observed - expected) / expected;
                    groups = groups.saturating_add(1_u32);
                }
                last_group = Some((observed_acc, expected_acc));
                observed_acc = 0.0_f64;
                expected_acc = 0.0_f64;
            }
        }
        match last_group {
            Some((observed, expected)) => {
                let merged_observed = observed + observed_acc;
                let merged_expected = expected + expected_acc;
                chi_squared += (merged_observed - merged_expected)
                    * (merged_observed - merged_expected)
                    / merged_expected;
                groups = groups.saturating_add(1_u32);
            }
            None if expected_acc > 0.0_f64 => {
                chi_squared +=
                    (observed_acc - expected_acc) * (observed_acc - expected_acc) / expected_acc;
                groups = groups.saturating_add(1_u32);
            }
            None => return None,
        }

        let degrees_of_freedom = groups
            .saturating_sub(1_u32)
            .saturating_sub(fitted_parameters);
        let reduced_chi_squared =
            (degrees_of_freedom > 0_u32).then(|| chi_squared / f64::from(degrees_of_freedom));
        Some(Self {
            chi_squared,
            degrees_of_freedom,
            reduced_chi_squared,
            merged_groups: groups,
        })
    }
}
