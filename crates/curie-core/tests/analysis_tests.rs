//! Integration tests for the statistical fits.

use curie_core::{
    BinWidthMode, DecayBackground, DecaySample, ExponentialDecayFit, GaussianFit, GoodnessOfFit,
    IntegerHistogram, PoissonFit,
};

/// Deterministic xorshift generator with a Poisson sampler, for reproducible synthetic runs.
struct TestRng(u64);

impl TestRng {
    fn uniform(&mut self) -> f64 {
        self.0 ^= self.0 << 13_u32;
        self.0 ^= self.0 >> 7_u32;
        self.0 ^= self.0 << 17_u32;
        #[expect(
            clippy::as_conversions,
            clippy::cast_precision_loss,
            reason = "53 random bits mapped to [0, 1)"
        )]
        let unit = (self.0 >> 11_u32) as f64 / 9_007_199_254_740_992.0_f64;
        unit
    }

    /// Knuth's multiplication method; fine for the means used here (< 500).
    fn poisson(&mut self, mean: f64) -> u32 {
        let limit = (-mean).exp();
        let mut count = 0_u32;
        let mut product = self.uniform();
        loop {
            if product <= limit {
                return count;
            }
            count = count.saturating_add(1_u32);
            product *= self.uniform();
        }
    }
}

/// Synthetic Pa-234m run: T1/2 = 70.2 s, 80 counts/s at t = 2.5 s, 0.6 counts/s background,
/// 5 s gates for 15 minutes (measuring out to > 10 half-lives).
fn synthetic_decay_run(rng: &mut TestRng) -> Vec<DecaySample> {
    let lambda = core::f64::consts::LN_2 / 70.2_f64;
    (0_u32..180_u32)
        .map(|gate_index| {
            let time = f64::from(gate_index).mul_add(5.0_f64, 2.5_f64);
            let mean = 5.0_f64 * 80.0_f64.mul_add((-lambda * (time - 2.5_f64)).exp(), 0.6_f64);
            DecaySample::new(time, 5.0_f64, rng.poisson(mean))
        })
        .collect()
}

#[test]
fn test_decay_fit_is_unbiased_when_the_tail_reaches_background() {
    let mut rng = TestRng(0x1234_5678_9ABC_DEF1_u64);
    let mut half_lives = Vec::new();
    let mut reported_errors = Vec::new();
    for _ in 0_u32..40_u32 {
        let samples = synthetic_decay_run(&mut rng);
        let fit = ExponentialDecayFit::fit(&samples, DecayBackground::Free, 0.0_f64)
            .expect("a full synthetic decay has a fit");
        assert!(fit.converged, "the minimiser converges");
        assert_eq!(fit.points_used, 180_usize);
        half_lives.push(fit.half_life_s);
        reported_errors.push(fit.half_life_error);
    }
    let runs = 40.0_f64;
    let mean = half_lives.iter().sum::<f64>() / runs;
    let spread = (half_lives
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f64>()
        / (runs - 1.0_f64))
        .sqrt();
    let mean_reported = reported_errors.iter().sum::<f64>() / runs;

    // The old unweighted log-linear fit returned ~96 s here.
    assert!(
        (mean - 70.2_f64).abs() < 1.5_f64,
        "mean fitted half-life {mean:.2} s must match 70.2 s"
    );
    assert!(
        mean_reported > 0.6_f64 * spread && mean_reported < 1.6_f64 * spread,
        "reported error {mean_reported:.2} s must describe the run-to-run spread {spread:.2} s"
    );
}

#[test]
fn test_decay_fit_with_fixed_background() {
    let mut rng = TestRng(0x0BAD_F00D_DEAD_BEEF_u64);
    let samples = synthetic_decay_run(&mut rng);
    let fit = ExponentialDecayFit::fit(&samples, DecayBackground::Fixed(0.6_f64), 0.0_f64)
        .expect("decay with a known background has a fit");

    assert!(!fit.background_fitted);
    assert_eq!(fit.background_error.to_bits(), 0.0_f64.to_bits());
    assert_eq!(fit.degrees_of_freedom, 178_u32, "two fitted parameters");
    assert!((fit.half_life_s - 70.2_f64).abs() < 5.0_f64 * fit.half_life_error);
    assert!(
        fit.reduced_chi_squared
            .is_some_and(|reduced| reduced > 0.5_f64 && reduced < 2.0_f64),
        "a correct model gives a reduced chi-squared near one"
    );
}

#[test]
fn test_decay_fit_recovers_noiseless_parameters() {
    let lambda = 0.02_f64;
    let samples: Vec<DecaySample> = (0_u32..120_u32)
        .map(|gate_index| {
            let time = f64::from(gate_index).mul_add(2.0_f64, 1.0_f64);
            let expected = 2.0_f64 * 400.0_f64.mul_add((-lambda * (time - 1.0_f64)).exp(), 3.0_f64);
            #[expect(
                clippy::as_conversions,
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "Rounded positive expected counts below 1000"
            )]
            let counts = expected.round() as u32;
            DecaySample::new(time, 2.0_f64, counts)
        })
        .collect();
    let fit = ExponentialDecayFit::fit(&samples, DecayBackground::Free, 0.0_f64)
        .expect("noiseless decay has a fit");
    assert!(
        (fit.lambda - lambda).abs() < 2e-4_f64,
        "lambda = {}",
        fit.lambda
    );
    assert!((fit.rate_at_start - 400.0_f64).abs() < 2.0_f64);
    assert!((fit.background_cps - 3.0_f64).abs() < 0.2_f64);
}

#[test]
fn test_decay_fit_rejects_too_few_points() {
    let samples = [
        DecaySample::new(1.0_f64, 1.0_f64, 10_u32),
        DecaySample::new(2.0_f64, 1.0_f64, 5_u32),
        DecaySample::new(3.0_f64, 1.0_f64, 3_u32),
    ];
    assert!(ExponentialDecayFit::fit(&samples, DecayBackground::Free, 0.0_f64).is_none());
}

#[test]
fn test_poisson_fit_uses_exact_sample_mean() {
    let mut rng = TestRng(0x5EED_5EED_5EED_5EED_u64);
    let counts: Vec<u32> = (0_u32..500_u32).map(|_| rng.poisson(3.9_f64)).collect();
    let histogram = IntegerHistogram::from_counts(&counts, BinWidthMode::Fixed(1_u32), 64_u32)
        .expect("non-empty counts");
    let fit = PoissonFit::from_counts(&counts, &histogram).expect("counts have a Poisson fit");

    let total: u32 = counts.iter().sum();
    assert!((fit.lambda - f64::from(total) / 500.0_f64).abs() < 1e-12_f64);
    assert!((fit.lambda_error - (fit.lambda / 500.0_f64).sqrt()).abs() < 1e-12_f64);
    let goodness = fit
        .goodness
        .expect("a populated histogram has a chi-squared");
    assert!(
        goodness
            .reduced_chi_squared
            .is_some_and(|reduced| reduced < 2.5_f64),
        "Poisson data fit a Poisson model"
    );
    let expected_total: f64 = fit.expected_per_bin.iter().sum();
    assert!(
        (expected_total - 500.0_f64).abs() < 1e-6_f64,
        "expected frequencies sum to n"
    );
}

#[test]
fn test_gaussian_fit_matches_sample_moments_for_large_counts() {
    // The old log-parabola fit returned None for means around 1000.
    let counts: Vec<u32> = (0_u32..400_u32)
        .map(|index| 1_000_u32 + index % 41_u32)
        .collect();
    let histogram = IntegerHistogram::from_counts(&counts, BinWidthMode::Fixed(2_u32), 64_u32)
        .expect("non-empty counts");
    let fit = GaussianFit::from_counts(&counts, &histogram).expect("spread counts have a fit");

    // 9 full cycles of 0..=40 (mean 20) plus 0..=30 (mean 15): 1000 + 7845 / 400.
    assert!(
        (fit.mean - 1_019.612_5_f64).abs() < 1e-9_f64,
        "mean = {}",
        fit.mean
    );
    assert!(fit.sigma > 11.0_f64 && fit.sigma < 13.0_f64);
    assert!(
        2.354_820_045_030_949_3_f64
            .mul_add(-fit.sigma, fit.fwhm)
            .abs()
            < 1e-12_f64
    );
    assert!(fit.mean_error > 0.0_f64 && fit.sigma_error > 0.0_f64);
    assert!(
        GaussianFit::from_counts(&[5_u32, 5_u32], &histogram).is_none(),
        "zero spread"
    );
}

#[test]
fn test_goodness_of_fit_merges_sparse_bins() {
    let counts = [0_u32, 1_u32, 1_u32, 2_u32, 2_u32, 2_u32, 3_u32, 9_u32];
    let histogram = IntegerHistogram::from_counts(&counts, BinWidthMode::Fixed(1_u32), 64_u32)
        .expect("non-empty counts");
    let bins = histogram.frequencies.len();
    let uniform = vec![1.0_f64 / f64::from(u32::try_from(bins).expect("few bins")); bins];
    let goodness =
        GoodnessOfFit::pearson(&histogram, &uniform, 1_u32).expect("histogram has a test");

    assert_eq!(
        goodness.merged_groups, 1_u32,
        "8 events in 10 bins form one group of >= 5"
    );
    assert_eq!(goodness.degrees_of_freedom, 0_u32);
    assert!(goodness.reduced_chi_squared.is_none());
    assert!(GoodnessOfFit::pearson(&histogram, &[1.0_f64], 1_u32).is_none());
}
