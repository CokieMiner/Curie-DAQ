//! Exponential decay with background, fitted by Poisson maximum likelihood.
//!
//! Model for the counts of gate `i` (centred at `t_i`, duration `g_i`):
//!
//! `mu_i = g_i (A exp(-lambda (t_i - t0)) + B)`
//!
//! where `A` is the net rate at the first fitted gate `t0`, `lambda` the decay constant and
//! `B` the background rate (fitted, or fixed to a measured "zero effect"). The parameters
//! maximise the Poisson likelihood of the raw counts, which stays unbiased when the tail
//! drops to the background level (a log-linear fit does not: it discards or distorts the
//! points where `rate - B <= 0`). Uncertainties come from the inverse Fisher information.

use alloc::vec::Vec;

use crate::F64Math;

/// Maximum Levenberg-Marquardt iterations.
const MAX_ITERATIONS: u32 = 500_u32;
/// Relative deviance change below which the fit is considered converged.
const CONVERGENCE_TOLERANCE: f64 = 1e-12_f64;
/// Dead-time correction factors below this are treated as saturated and dropped.
const MIN_LIVE_FRACTION: f64 = 0.1_f64;

/// One counting gate of a decay measurement.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DecaySample {
    /// Centre of the gate, in seconds since the start of the run.
    pub time_s: f64,
    /// Gate duration in seconds.
    pub gate_s: f64,
    /// Counts recorded in the gate.
    pub counts: u32,
}

impl DecaySample {
    /// Creates a gate sample.
    #[must_use]
    pub const fn new(time_s: f64, gate_s: f64, counts: u32) -> Self {
        Self {
            time_s,
            gate_s,
            counts,
        }
    }
}

/// How the background rate is treated.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum DecayBackground {
    /// Fit the background together with the decay (needs data reaching the plateau).
    #[default]
    Free,
    /// Use a separately measured background ("zero effect") in counts per second.
    Fixed(f64),
}

/// Result of a Poisson maximum-likelihood decay fit.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExponentialDecayFit {
    /// Time of the first fitted gate centre, `t0` (s).
    pub start_time_s: f64,
    /// Net rate at `t0`, `A` (counts/s, dead-time corrected).
    pub rate_at_start: f64,
    /// Standard error of `A`.
    pub rate_at_start_error: f64,
    /// Decay constant `lambda` (1/s).
    pub lambda: f64,
    /// Standard error of `lambda`.
    pub lambda_error: f64,
    /// Half-life `ln 2 / lambda` (s).
    pub half_life_s: f64,
    /// Standard error of the half-life.
    pub half_life_error: f64,
    /// Background rate `B` (counts/s).
    pub background_cps: f64,
    /// Standard error of `B` (zero when it was fixed).
    pub background_error: f64,
    /// Whether `B` was fitted (`false` when fixed by the caller).
    pub background_fitted: bool,
    /// Pearson chi-squared `sum (n_i - mu_i)^2 / mu_i`.
    pub chi_squared: f64,
    /// Degrees of freedom: gates used - fitted parameters.
    pub degrees_of_freedom: u32,
    /// `chi_squared / degrees_of_freedom`, if defined.
    pub reduced_chi_squared: Option<f64>,
    /// Number of gates used.
    pub points_used: usize,
    /// Whether the minimiser met the convergence tolerance.
    pub converged: bool,
}

impl ExponentialDecayFit {
    /// Fits `samples` with the decay model.
    ///
    /// Counts are dead-time corrected (non-paralysable, `dead_time_s`) before fitting.
    /// Returns `None` with fewer gates than parameters + 1, when the data show no decay, or
    /// when the information matrix is singular.
    #[must_use]
    pub fn fit(
        samples: &[DecaySample],
        background: DecayBackground,
        dead_time_s: f64,
    ) -> Option<Self> {
        // Prepared gates: (time since t0, gate length, corrected counts).
        let start_time_s = samples
            .iter()
            .filter(|sample| sample.gate_s > 0.0_f64 && sample.time_s.is_finite())
            .map(|sample| sample.time_s)
            .fold(f64::INFINITY, f64::min);
        let mut gates: Vec<(f64, f64, f64)> = Vec::new();
        gates.try_reserve_exact(samples.len()).ok()?;
        for sample in samples {
            if sample.gate_s <= 0.0_f64 || !sample.time_s.is_finite() {
                continue;
            }
            let counts = f64::from(sample.counts);
            let live_fraction = 1.0_f64 - counts / sample.gate_s * dead_time_s.max(0.0_f64);
            if live_fraction < MIN_LIVE_FRACTION {
                continue;
            }
            gates.push((
                sample.time_s - start_time_s,
                sample.gate_s,
                counts / live_fraction,
            ));
        }

        let (fixed_background, parameters) = match background {
            DecayBackground::Fixed(rate) => (Some(rate.max(0.0_f64)), 2_u32),
            DecayBackground::Free => (None, 3_u32),
        };
        let parameter_count = usize::try_from(parameters).ok()?;
        if gates.len() <= parameter_count {
            return None;
        }

        let (amplitude, lambda, rate_b, converged) = minimise(&gates, fixed_background)?;

        let (information, _) =
            information_and_gradient(&gates, amplitude, lambda, rate_b, fixed_background);
        let [variance_amp, _, _, variance_decay, _, variance_back] =
            invert_symmetric3(information)?;
        let lambda_error = F64Math::sqrt(variance_decay.max(0.0_f64));
        let ln2 = core::f64::consts::LN_2;
        let lambda_squared = lambda * lambda;

        let mut chi_squared = 0.0_f64;
        for &(time, gate, counts) in &gates {
            let expected = gate * (amplitude * F64Math::exp(-lambda * time) + rate_b);
            chi_squared += (counts - expected) * (counts - expected) / expected;
        }
        let degrees_of_freedom = u32::try_from(gates.len())
            .unwrap_or(u32::MAX)
            .saturating_sub(parameters);

        Some(Self {
            start_time_s,
            rate_at_start: amplitude,
            rate_at_start_error: F64Math::sqrt(variance_amp.max(0.0_f64)),
            lambda,
            lambda_error,
            half_life_s: ln2 / lambda,
            half_life_error: ln2 * lambda_error / lambda_squared,
            background_cps: rate_b,
            background_error: if fixed_background.is_some() {
                0.0_f64
            } else {
                F64Math::sqrt(variance_back.max(0.0_f64))
            },
            background_fitted: fixed_background.is_none(),
            chi_squared,
            degrees_of_freedom,
            reduced_chi_squared: (degrees_of_freedom > 0_u32)
                .then(|| chi_squared / f64::from(degrees_of_freedom)),
            points_used: gates.len(),
            converged,
        })
    }
}

/// Levenberg-Marquardt minimisation of the Poisson deviance (Fisher scoring with damping).
///
/// Returns `(amplitude, lambda, background, converged)`.
fn minimise(
    gates: &[(f64, f64, f64)],
    fixed_background: Option<f64>,
) -> Option<(f64, f64, f64, bool)> {
    let (mut amplitude, mut lambda, mut rate_b) = initial_guess(gates, fixed_background)?;
    let mut deviance = poisson_deviance(gates, amplitude, lambda, rate_b)?;
    let mut damping = 1e-3_f64;
    let mut converged = false;

    for _ in 0..MAX_ITERATIONS {
        let (information, gradient) =
            information_and_gradient(gates, amplitude, lambda, rate_b, fixed_background);
        let [
            amp_amp,
            amp_decay,
            amp_back,
            decay_decay,
            decay_back,
            back_back,
        ] = information;
        let damped = [
            amp_amp * (1.0_f64 + damping),
            amp_decay,
            amp_back,
            decay_decay * (1.0_f64 + damping),
            decay_back,
            back_back * (1.0_f64 + damping),
        ];
        let Some(inverse) = invert_symmetric3(damped) else {
            damping *= 10.0_f64;
            if damping > 1e12_f64 {
                break;
            }
            continue;
        };
        let [
            cov_amp,
            cov_amp_decay,
            cov_amp_back,
            cov_decay,
            cov_decay_back,
            cov_back,
        ] = inverse;
        let [grad_amp, grad_decay, grad_back] = gradient;
        // Newton step on the negative log-likelihood: delta = -I^-1 grad.
        let new_amplitude = amplitude
            - (cov_amp * grad_amp + cov_amp_decay * grad_decay + cov_amp_back * grad_back);
        let new_lambda = lambda
            - (cov_amp_decay * grad_amp + cov_decay * grad_decay + cov_decay_back * grad_back);
        let new_background = if fixed_background.is_some() {
            rate_b
        } else {
            rate_b - (cov_amp_back * grad_amp + cov_decay_back * grad_decay + cov_back * grad_back)
        };
        let trial = if new_amplitude > 0.0_f64 && new_lambda > 0.0_f64 && new_background >= 0.0_f64
        {
            poisson_deviance(gates, new_amplitude, new_lambda, new_background)
        } else {
            None
        };
        match trial {
            Some(trial_deviance) if trial_deviance <= deviance => {
                let improvement = deviance - trial_deviance;
                amplitude = new_amplitude;
                lambda = new_lambda;
                rate_b = new_background;
                deviance = trial_deviance;
                damping = (damping * 0.1_f64).max(1e-12_f64);
                if improvement <= CONVERGENCE_TOLERANCE * (1.0_f64 + deviance) {
                    converged = true;
                    break;
                }
            }
            _ => {
                damping *= 10.0_f64;
                if damping > 1e12_f64 {
                    // No downhill step exists at any damping: we are at the minimum.
                    converged = true;
                    break;
                }
            }
        }
    }
    Some((amplitude, lambda, rate_b, converged))
}

/// Starting point: background from the tail, then a weighted log-linear fit of the net rate.
fn initial_guess(
    gates: &[(f64, f64, f64)],
    fixed_background: Option<f64>,
) -> Option<(f64, f64, f64)> {
    let tail_len = gates.len().div_ceil(5_usize).max(2_usize);
    let tail_start = gates.len().saturating_sub(tail_len);
    let (tail_counts, tail_time) = gates
        .iter()
        .skip(tail_start)
        .fold((0.0_f64, 0.0_f64), |(counts, time), &(_, gate, n)| {
            (counts + n, time + gate)
        });
    let tail_rate = if tail_time > 0.0_f64 {
        tail_counts / tail_time
    } else {
        0.0_f64
    };
    let background = fixed_background.unwrap_or(tail_rate);

    // Weighted least squares of ln(net rate) on time; weight = 1 / var(ln net).
    let mut weight_total = 0.0_f64;
    let mut weighted_time = 0.0_f64;
    let mut weighted_log = 0.0_f64;
    let mut weighted_time_squared = 0.0_f64;
    let mut weighted_time_log = 0.0_f64;
    for &(time, gate, counts) in gates {
        let net = counts / gate - background;
        let sigma_rate = F64Math::sqrt(counts.max(1.0_f64)) / gate;
        if net <= 2.0_f64 * sigma_rate {
            continue;
        }
        let weight = (net * gate) * (net * gate) / counts.max(1.0_f64);
        let log_net = F64Math::ln(net);
        weight_total += weight;
        weighted_time += weight * time;
        weighted_log += weight * log_net;
        weighted_time_squared += weight * time * time;
        weighted_time_log += weight * time * log_net;
    }
    let span = gates
        .iter()
        .map(|&(time, _, _)| time)
        .fold(0.0_f64, f64::max);
    // Centred normal equations (better conditioned than the raw sums).
    let mean_time = weighted_time / weight_total;
    let mean_log = weighted_log / weight_total;
    let spread = weighted_time_squared - mean_time * weighted_time;
    let (amplitude, lambda) = if weight_total > 0.0_f64 && spread > 0.0_f64 {
        let slope = (weighted_time_log - mean_time * weighted_log) / spread;
        let intercept = mean_log - slope * mean_time;
        let lambda = if slope < 0.0_f64 {
            -slope
        } else {
            3.0_f64 / span.max(f64::MIN_POSITIVE)
        };
        (F64Math::exp(intercept), lambda)
    } else {
        let first = gates.first()?;
        let net = (first.2 / first.1 - background).max(f64::MIN_POSITIVE);
        (net, 3.0_f64 / span.max(f64::MIN_POSITIVE))
    };
    (amplitude > 0.0_f64 && lambda > 0.0_f64 && amplitude.is_finite() && lambda.is_finite())
        .then_some((amplitude, lambda, background))
}

/// Poisson deviance `2 sum (mu - n + n ln(n / mu))`, or `None` if any `mu <= 0`.
fn poisson_deviance(
    gates: &[(f64, f64, f64)],
    amplitude: f64,
    lambda: f64,
    background: f64,
) -> Option<f64> {
    let mut deviance = 0.0_f64;
    for &(time, gate, counts) in gates {
        let expected = gate * (amplitude * F64Math::exp(-lambda * time) + background);
        if expected <= 0.0_f64 || !expected.is_finite() {
            return None;
        }
        deviance += expected - counts;
        if counts > 0.0_f64 {
            deviance += counts * F64Math::ln(counts / expected);
        }
    }
    Some(2.0_f64 * deviance)
}

/// Fisher information (upper triangle `[AA, AL, AB, LL, LB, BB]` for amplitude, lambda,
/// background) and the gradient of the negative log-likelihood. With a fixed background the
/// background row is replaced by the identity.
fn information_and_gradient(
    gates: &[(f64, f64, f64)],
    amplitude: f64,
    lambda: f64,
    background: f64,
    fixed_background: Option<f64>,
) -> ([f64; 6], [f64; 3]) {
    let mut information = [0.0_f64; 6];
    let mut gradient = [0.0_f64; 3];
    for &(time, gate, counts) in gates {
        let decay = F64Math::exp(-lambda * time);
        let expected = gate * (amplitude * decay + background);
        if expected <= 0.0_f64 {
            continue;
        }
        let derivatives = [gate * decay, -gate * amplitude * time * decay, gate];
        let [deriv_amp, deriv_decay, deriv_back] = derivatives;
        let inverse = 1.0_f64 / expected;
        let products = [
            deriv_amp * deriv_amp,
            deriv_amp * deriv_decay,
            deriv_amp * deriv_back,
            deriv_decay * deriv_decay,
            deriv_decay * deriv_back,
            deriv_back * deriv_back,
        ];
        for (slot, product) in information.iter_mut().zip(products) {
            *slot += product * inverse;
        }
        let residual = 1.0_f64 - counts * inverse;
        for (slot, derivative) in gradient.iter_mut().zip(derivatives) {
            *slot += residual * derivative;
        }
    }
    if fixed_background.is_some() {
        let [amp_amp, amp_decay, _, decay_decay, _, _] = information;
        let [grad_amp, grad_decay, _] = gradient;
        (
            [amp_amp, amp_decay, 0.0_f64, decay_decay, 0.0_f64, 1.0_f64],
            [grad_amp, grad_decay, 0.0_f64],
        )
    } else {
        (information, gradient)
    }
}

/// Inverse of a symmetric positive-definite 3x3 matrix given as its upper triangle
/// `[m11, m12, m13, m22, m23, m33]`.
///
/// The matrix is first scaled to unit diagonal (Jacobi preconditioning) so parameters of
/// very different magnitude (rates of 100 /s against decay constants of 0.01 /s) do not
/// destroy the cofactor arithmetic. Returns `None` if it is not positive definite.
fn invert_symmetric3(matrix: [f64; 6]) -> Option<[f64; 6]> {
    let [
        first,
        first_second,
        first_third,
        second,
        second_third,
        third,
    ] = matrix;
    if first <= 0.0_f64 || second <= 0.0_f64 || third <= 0.0_f64 {
        return None;
    }
    let scales = [
        1.0_f64 / F64Math::sqrt(first),
        1.0_f64 / F64Math::sqrt(second),
        1.0_f64 / F64Math::sqrt(third),
    ];
    let [scale_one, scale_two, scale_three] = scales;
    let corr_one_two = first_second * scale_one * scale_two;
    let corr_one_three = first_third * scale_one * scale_three;
    let corr_two_three = second_third * scale_two * scale_three;

    // Cofactors of the unit-diagonal correlation matrix.
    let cof_one = 1.0_f64 - corr_two_three * corr_two_three;
    let cof_one_two = corr_one_three * corr_two_three - corr_one_two;
    let cof_one_three = corr_one_two * corr_two_three - corr_one_three;
    let cof_two = 1.0_f64 - corr_one_three * corr_one_three;
    let cof_two_three = corr_one_two * corr_one_three - corr_two_three;
    let cof_three = 1.0_f64 - corr_one_two * corr_one_two;
    let determinant = cof_one + corr_one_two * cof_one_two + corr_one_three * cof_one_three;
    if determinant <= 1e-14_f64 || !determinant.is_finite() {
        return None;
    }
    let inverse_det = 1.0_f64 / determinant;
    Some([
        cof_one * inverse_det * scale_one * scale_one,
        cof_one_two * inverse_det * scale_one * scale_two,
        cof_one_three * inverse_det * scale_one * scale_three,
        cof_two * inverse_det * scale_two * scale_two,
        cof_two_three * inverse_det * scale_two * scale_three,
        cof_three * inverse_det * scale_three * scale_three,
    ])
}
