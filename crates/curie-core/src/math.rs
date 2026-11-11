//! Dependency-free `#![no_std]` double-precision special functions for the analysis code.
//!
//! `core` does not expose `f64::sqrt`, `f64::exp` or `f64::ln` without `std`, and the
//! crate deliberately has no dependencies, so the fits use these implementations.
//!
//! Every function except `sqrt` is evaluated internally in double-double arithmetic (a
//! value is never held in more than two `f64` words; Dekker's error-free transformations,
//! no FMA required) and rounded to `f64` exactly once, so results are within about
//! 0.5 ULP of the exact value:
//!
//! - [`F64Math::sqrt`] is correctly rounded (integer square root of the mantissa).
//! - [`F64Math::exp`], [`F64Math::ln`]: ≈ 0.5 ULP.
//! - [`F64Math::normal_cdf`] (via a double-double `erfc`): ≈ 0.5 ULP.
//! - [`F64Math::poisson_pmf`] (via a double-double `ln k!`): ≈ 0.5 ULP.
//!
//! Subnormal results are also rounded once. The errors are checked against `mpmath`
//! reference values in `tests/f64_math_tests.rs`.

/// `ln 2` as a double-double.
const LN2: Dd = Dd::new(core::f64::consts::LN_2, 2.319_046_813_846_299_6e-17_f64);
/// `2 / sqrt(pi)` as a double-double.
const FRAC_2_SQRT_PI: Dd = Dd::new(
    core::f64::consts::FRAC_2_SQRT_PI,
    1.533_545_961_316_588e-17_f64,
);
/// `1 / sqrt(pi)` as a double-double.
const FRAC_1_SQRT_PI: Dd = Dd::new(0.564_189_583_547_756_3_f64, 7.667_729_806_582_94e-18_f64);
/// `1 / sqrt(2)` as a double-double.
const FRAC_1_SQRT_2: Dd = Dd::new(
    core::f64::consts::FRAC_1_SQRT_2,
    -4.833_646_656_726_457e-17_f64,
);
/// `ln(2 pi) / 2` as a double-double.
const HALF_LN_2PI: Dd = Dd::new(
    0.918_938_533_204_672_8_f64,
    -3.878_294_158_067_241_4e-17_f64,
);
/// Reciprocals `1/3, 1/5, ..., 1/43` as double-doubles for the `atanh` series.
const INVERSE_ODD: [Dd; 21] = [
    Dd::new(0.333_333_333_333_333_3_f64, 1.850_371_707_708_594e-17_f64),
    Dd::new(0.2_f64, -1.110_223_024_625_156_6e-17_f64),
    Dd::new(0.142_857_142_857_142_85_f64, 7.930_164_461_608_26e-18_f64),
    Dd::new(0.111_111_111_111_111_1_f64, 6.167_905_692_361_980_4e-18_f64),
    Dd::new(0.090_909_090_909_090_91_f64, -2.523_234_146_875_356e-18_f64),
    Dd::new(0.076_923_076_923_076_93_f64, -4.270_088_556_250_602e-18_f64),
    Dd::new(0.066_666_666_666_666_67_f64, 9.251_858_538_542_971e-19_f64),
    Dd::new(0.058_823_529_411_764_705_f64, 8.163_404_592_832_033e-19_f64),
    Dd::new(0.052_631_578_947_368_42_f64, 2.921_639_538_487_254e-18_f64),
    Dd::new(0.047_619_047_619_047_616_f64, 2.643_388_153_869_42e-18_f64),
    Dd::new(0.043_478_260_869_565_216_f64, 1.206_764_157_201_257e-18_f64),
    Dd::new(0.04_f64, -8.326_672_684_688_674e-19_f64),
    Dd::new(0.037_037_037_037_037_035_f64, 2.055_968_564_120_66e-18_f64),
    Dd::new(0.034_482_758_620_689_655_f64, 4.785_444_071_660_157e-19_f64),
    Dd::new(0.032_258_064_516_129_03_f64, 8.953_411_488_912_552e-19_f64),
    Dd::new(
        0.030_303_030_303_030_304_f64,
        -8.410_780_489_584_519e-19_f64,
    ),
    Dd::new(0.028_571_428_571_428_57_f64, 8.921_435_019_309_293e-19_f64),
    Dd::new(0.027_027_027_027_027_03_f64, -1.500_301_384_628_59e-18_f64),
    Dd::new(0.025_641_025_641_025_64_f64, 8.896_017_825_522_087e-19_f64),
    Dd::new(0.024_390_243_902_439_025_f64, -8.462_065_736_472_23e-19_f64),
    Dd::new(
        0.023_255_813_953_488_372_f64,
        3.227_392_513_445_222_5e-19_f64,
    ),
];
/// `log2(e)`, only used to pick the range-reduction integer.
const LOG2_E: f64 = core::f64::consts::LOG2_E;
/// Largest argument whose exponential is finite.
const EXP_OVERFLOW: f64 = 709.782_712_893_384_f64;
/// Below this argument the exponential is zero even as a subnormal.
const EXP_UNDERFLOW: f64 = -745.133_219_101_941_1_f64;
/// Dekker splitting constant `2^27 + 1`.
const SPLITTER: f64 = 134_217_729.0_f64;
/// Factorials 0! ..= 20! (all exact in `u64`).
const SMALL_FACTORIALS: [u64; 21] = [
    1,
    1,
    2,
    6,
    24,
    120,
    720,
    5_040,
    40_320,
    362_880,
    3_628_800,
    39_916_800,
    479_001_600,
    6_227_020_800,
    87_178_291_200,
    1_307_674_368_000,
    20_922_789_888_000,
    355_687_428_096_000,
    6_402_373_705_728_000,
    121_645_100_408_832_000,
    2_432_902_008_176_640_000,
];
/// Stirling series coefficients `B_2m / (2m (2m - 1))` for m = 1..=8, as double-doubles.
const STIRLING_COEFFICIENTS: [Dd; 8] = [
    Dd::new(0.083_333_333_333_333_33_f64, 4.625_929_269_271_485e-18_f64),
    Dd::new(
        -0.002_777_777_777_777_778_f64,
        1.060_108_790_874_715_4e-19_f64,
    ),
    Dd::new(
        0.000_793_650_793_650_793_7_f64,
        6.883_823_317_368_282e-22_f64,
    ),
    Dd::new(
        -0.000_595_238_095_238_095_3_f64,
        5.369_382_187_547_26e-20_f64,
    ),
    Dd::new(
        0.000_841_750_841_750_841_7_f64,
        3.687_017_488_923_769_4e-20_f64,
    ),
    Dd::new(
        -0.001_917_526_917_526_917_6_f64,
        1.067_570_277_687_247_5e-19_f64,
    ),
    Dd::new(
        0.006_410_256_410_256_41_f64,
        2.224_004_456_380_521_7e-19_f64,
    ),
    Dd::new(
        -0.029_550_653_594_771_242_f64,
        4.861_760_957_508_855e-19_f64,
    ),
];

/// Namespace for the double-precision special functions.
#[non_exhaustive]
#[derive(Debug, Clone, Copy)]
pub struct F64Math;

impl F64Math {
    /// Correctly rounded square root. Returns NaN for negative input.
    #[must_use]
    pub fn sqrt(x: f64) -> f64 {
        if x.is_nan() || x < 0.0_f64 {
            return f64::NAN;
        }
        if x == 0.0_f64 || x.is_infinite() {
            return x;
        }

        let bits = x.to_bits();
        let mut exponent = i32::try_from((bits >> 52_u32) & 0x7FF_u64).unwrap_or(0_i32);
        let mut mantissa = bits & 0x000F_FFFF_FFFF_FFFF_u64;
        if exponent == 0_i32 {
            // Subnormal: normalise so the implicit bit is set.
            while mantissa & 0x0010_0000_0000_0000_u64 == 0_u64 {
                mantissa <<= 1_u32;
                exponent = exponent.saturating_sub(1_i32);
            }
            exponent = exponent.saturating_add(1_i32);
        } else {
            mantissa |= 0x0010_0000_0000_0000_u64;
        }
        // x = mantissa * 2^(exponent - 1075); make the power even.
        let mut power = exponent.saturating_sub(1075_i32);
        if power & 1_i32 != 0_i32 {
            mantissa <<= 1_u32;
            power = power.saturating_sub(1_i32);
        }

        // sqrt(x) = sqrt(mantissa * 2^52) * 2^((power - 52) / 2), with a 53-bit root.
        // Integer square root by Newton's method, starting above the root.
        let radicand = u128::from(mantissa) << 52_u32;
        let bit_length = 128_u32.saturating_sub(radicand.leading_zeros());
        let mut root = 1_u128 << bit_length.div_ceil(2_u32);
        loop {
            let next = root.saturating_add(radicand.checked_div(root).unwrap_or(0_u128)) >> 1_u32;
            if next >= root {
                break;
            }
            root = next;
        }
        let remainder = radicand.saturating_sub(root.saturating_mul(root));
        // Round to nearest: the exact root is root + d with d in [0, 1); round up when
        // (root + 0.5)^2 < radicand, i.e. remainder > root. Ties are impossible.
        if remainder > root {
            root = root.saturating_add(1_u128);
        }
        // power - 52 is even, so the arithmetic shift is an exact halving.
        let mut result_power = power.saturating_sub(52_i32) >> 1_i32;
        if root >= (1_u128 << 53_u32) {
            root >>= 1_u32;
            result_power = result_power.saturating_add(1_i32);
        }
        let fraction = u64::try_from(root).unwrap_or(0_u64) & 0x000F_FFFF_FFFF_FFFF_u64;
        let biased = u64::try_from(result_power.saturating_add(52_i32).saturating_add(1023_i32))
            .unwrap_or(0_u64);
        f64::from_bits((biased << 52_u32) | fraction)
    }

    /// Natural exponential `e^x` (≈ 0.5 ULP).
    #[must_use]
    pub fn exp(x: f64) -> f64 {
        if x.is_nan() {
            return f64::NAN;
        }
        let value = exp_dd(Dd::new(x, 0.0_f64));
        value.hi + value.lo
    }

    /// Natural logarithm (≈ 0.5 ULP). NaN for negative input, `-inf` for zero.
    #[must_use]
    pub fn ln(x: f64) -> f64 {
        if x.is_nan() || x < 0.0_f64 {
            return f64::NAN;
        }
        if x == 0.0_f64 {
            return f64::NEG_INFINITY;
        }
        if x.is_infinite() {
            return x;
        }
        let value = ln_dd(x);
        value.hi + value.lo
    }

    /// Poisson probability `P(k; lambda) = lambda^k e^-lambda / k!` (≈ 0.5 ULP).
    #[must_use]
    pub fn poisson_pmf(k: u32, lambda: f64) -> f64 {
        if lambda.is_nan() || lambda < 0.0_f64 || lambda.is_infinite() {
            return f64::NAN;
        }
        if lambda == 0.0_f64 {
            return if k == 0_u32 { 1.0_f64 } else { 0.0_f64 };
        }
        let log_probability = ln_dd(lambda)
            .mul_f64(f64::from(k))
            .add_f64(-lambda)
            .sub(ln_factorial_dd(k));
        let value = exp_dd(log_probability);
        value.hi + value.lo
    }

    /// Standard normal cumulative distribution function `Φ(z) = erfc(-z / sqrt 2) / 2`.
    #[must_use]
    pub fn normal_cdf(z: f64) -> f64 {
        if z.is_nan() {
            return f64::NAN;
        }
        let value = erfc_dd(FRAC_1_SQRT_2.mul_f64(-z), -1_i32);
        value.hi + value.lo
    }
}

/// Unevaluated sum `hi + lo` with `|lo| <= ulp(hi) / 2`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Dd {
    hi: f64,
    lo: f64,
}

impl Dd {
    const fn new(hi: f64, lo: f64) -> Self {
        Self { hi, lo }
    }

    /// Knuth's two-sum: `a + b` exactly as a double-double.
    fn two_sum(a: f64, b: f64) -> Self {
        let sum = a + b;
        let b_virtual = sum - a;
        let a_virtual = sum - b_virtual;
        Self::new(sum, (a - a_virtual) + (b - b_virtual))
    }

    /// Fast two-sum, valid when `|a| >= |b|`.
    fn quick_two_sum(a: f64, b: f64) -> Self {
        let sum = a + b;
        Self::new(sum, b - (sum - a))
    }

    /// Dekker's two-product: `a * b` exactly as a double-double (via 26-bit splits).
    fn two_prod(a: f64, b: f64) -> Self {
        let product = a * b;
        let a_scaled = SPLITTER * a;
        let a_high = a_scaled - (a_scaled - a);
        let a_low = a - a_high;
        let b_scaled = SPLITTER * b;
        let b_high = b_scaled - (b_scaled - b);
        let b_low = b - b_high;
        let error = ((a_high * b_high - product) + a_high * b_low + a_low * b_high) + a_low * b_low;
        Self::new(product, error)
    }

    fn add(self, other: Self) -> Self {
        let sum = Self::two_sum(self.hi, other.hi);
        let tail = Self::two_sum(self.lo, other.lo);
        let partial = Self::quick_two_sum(sum.hi, sum.lo + tail.hi);
        Self::quick_two_sum(partial.hi, partial.lo + tail.lo)
    }

    fn sub(self, other: Self) -> Self {
        let difference = Self::two_sum(self.hi, -other.hi);
        let tail = Self::two_sum(self.lo, -other.lo);
        let partial = Self::quick_two_sum(difference.hi, difference.lo + tail.hi);
        Self::quick_two_sum(partial.hi, partial.lo + tail.lo)
    }

    fn add_f64(self, value: f64) -> Self {
        let sum = Self::two_sum(self.hi, value);
        Self::quick_two_sum(sum.hi, sum.lo + self.lo)
    }

    fn mul(self, other: Self) -> Self {
        let product = Self::two_prod(self.hi, other.hi);
        let cross = self.hi * other.lo + self.lo * other.hi;
        Self::quick_two_sum(product.hi, product.lo + cross)
    }

    fn mul_f64(self, value: f64) -> Self {
        let product = Self::two_prod(self.hi, value);
        Self::quick_two_sum(product.hi, product.lo + self.lo * value)
    }

    /// Long division with three correction steps.
    fn div(self, other: Self) -> Self {
        let first = self.hi / other.hi;
        let remainder = self.sub(other.mul_f64(first));
        let second = remainder.hi / other.hi;
        let remainder_two = remainder.sub(other.mul_f64(second));
        let third = remainder_two.hi / other.hi;
        Self::quick_two_sum(first, second).add_f64(third)
    }
}

/// `e^a` in double-double (relative error about 2^-100).
fn exp_dd(a: Dd) -> Dd {
    if a.hi > EXP_OVERFLOW {
        return Dd::new(f64::INFINITY, 0.0_f64);
    }
    if a.hi < EXP_UNDERFLOW {
        return Dd::new(0.0_f64, 0.0_f64);
    }

    let scaled = a.hi * LOG2_E;
    let rounded = if scaled >= 0.0_f64 {
        scaled + 0.5_f64
    } else {
        scaled - 0.5_f64
    };
    #[expect(
        clippy::as_conversions,
        clippy::cast_possible_truncation,
        reason = "|a * log2(e)| < 1100 after the range checks above"
    )]
    let k = rounded as i32;
    let reduced = a.sub(LN2.mul_f64(f64::from(k)));
    // e^r = (e^(r/16))^16 with |r / 16| <= 0.0217 (scaling by 2^-4 is exact).
    let small = Dd::new(reduced.hi * 0.0625_f64, reduced.lo * 0.0625_f64);

    // u = e^small - 1 = small (1 + small/2 (1 + small/3 (1 + ...))), 15 terms.
    let mut series = Dd::new(1.0_f64, 0.0_f64);
    for order in (2_u8..=16_u8).rev() {
        series = small
            .mul(series)
            .div(Dd::new(f64::from(order), 0.0_f64))
            .add_f64(1.0_f64);
    }
    let mut expm1 = small.mul(series);
    // (1 + u)^2 - 1 = 2u + u^2, four times.
    for _ in 0_u8..4_u8 {
        expm1 = expm1.mul_f64(2.0_f64).add(expm1.mul(expm1));
    }
    let value = expm1.add_f64(1.0_f64);

    let high = scale_pow2(value.hi, k);
    if k >= -1_000_i32 || high.abs() >= f64::MIN_POSITIVE {
        // Normal result: scaling by a power of two is exact. A low word that would become
        // subnormal is dropped: `hi` is already the rounded value, and a quantised `lo` could
        // land exactly on half an ULP and flip the final rounding.
        let low = scale_pow2(value.lo, k);
        let kept_low = if low.abs() < f64::MIN_POSITIVE {
            0.0_f64
        } else {
            low
        };
        return Dd::new(high, kept_low);
    }
    // Subnormal result: `high` was rounded to a multiple of 2^-1074. Fold the dropped bits
    // and the low word back in so the value is rounded exactly once.
    let residual = (value.hi - scale_pow2(high, k.saturating_neg())) + value.lo;
    let half_quantum = scale_pow2(1.0_f64, (-1_075_i32).saturating_sub(k));
    let quantum = f64::from_bits(1_u64);
    let corrected = if residual > half_quantum {
        high + quantum
    } else if residual < -half_quantum {
        high - quantum
    } else {
        high
    };
    Dd::new(corrected, 0.0_f64)
}

/// `ln x` in double-double for finite `x > 0` (relative error about 2^-100).
fn ln_dd(input: f64) -> Dd {
    let (value, exponent_adjust) = if input < f64::MIN_POSITIVE {
        (input * 18_014_398_509_481_984.0_f64, -54_i32) // 2^54
    } else {
        (input, 0_i32)
    };
    let bits = value.to_bits();
    let biased = i32::try_from((bits >> 52_u32) & 0x7FF_u64).unwrap_or(1023_i32);
    let mut exponent = biased
        .saturating_sub(1023_i32)
        .saturating_add(exponent_adjust);
    let mut mantissa =
        f64::from_bits((bits & 0x000F_FFFF_FFFF_FFFF_u64) | 0x3FF0_0000_0000_0000_u64);
    if mantissa > core::f64::consts::SQRT_2 {
        mantissa *= 0.5_f64;
        exponent = exponent.saturating_add(1_i32);
    }

    // ln m = 2 atanh(s), s = f / (2 + f), f = m - 1 (exact by Sterbenz).
    let fraction_part = mantissa - 1.0_f64;
    let atanh_arg = Dd::new(fraction_part, 0.0_f64).div(Dd::two_sum(2.0_f64, fraction_part));
    // atanh(s) = s + s^3 Q(s^2), Q(z) = 1/3 + z/5 + z^2/7 + ... (21 terms).
    let arg_squared = atanh_arg.mul(atanh_arg);
    let mut poly = Dd::new(0.0_f64, 0.0_f64);
    for &coefficient in INVERSE_ODD.iter().rev() {
        poly = coefficient.add(arg_squared.mul(poly));
    }
    let atanh = atanh_arg.add(atanh_arg.mul(arg_squared).mul(poly));
    LN2.mul_f64(f64::from(exponent)).add(atanh.mul_f64(2.0_f64))
}

/// `ln(k!)` in double-double.
fn ln_factorial_dd(k: u32) -> Dd {
    if let Some(&factorial) = usize::try_from(k)
        .ok()
        .and_then(|index| SMALL_FACTORIALS.get(index))
    {
        if factorial <= 1_u64 {
            return Dd::new(0.0_f64, 0.0_f64);
        }
        // Exact u64 -> double-double conversion.
        #[expect(
            clippy::as_conversions,
            clippy::cast_precision_loss,
            reason = "Rounded high word; the exact remainder is kept in the low word"
        )]
        let high = factorial as f64;
        #[expect(
            clippy::as_conversions,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "high is within one ULP of a u64 value below 2^62"
        )]
        let high_int = high as u64;
        let low = i128::from(factorial).saturating_sub(i128::from(high_int));
        #[expect(
            clippy::as_conversions,
            clippy::cast_precision_loss,
            reason = "The remainder is below 2^10 and exactly representable"
        )]
        let low_f64 = low as f64;
        // ln(high + low) = ln(high) + low / high to well below 2^-100.
        return ln_dd(high).add_f64(low_f64 / high);
    }

    // Stirling series for ln Γ(n), n = k + 1 >= 22.
    let n = f64::from(k) + 1.0_f64;
    let inverse = Dd::new(1.0_f64, 0.0_f64).div(Dd::new(n, 0.0_f64));
    let inverse_squared = inverse.mul(inverse);
    let mut correction = Dd::new(0.0_f64, 0.0_f64);
    for &coefficient in STIRLING_COEFFICIENTS.iter().rev() {
        correction = coefficient.add(inverse_squared.mul(correction));
    }

    ln_dd(n)
        .mul_f64(n - 0.5_f64)
        .add_f64(-n)
        .add(HALF_LN_2PI)
        .add(correction.mul(inverse))
}

/// `erfc(x) * 2^log2_scale` in double-double.
///
/// The power-of-two scale is applied before the final rounding, so a scaled subnormal
/// result (for example `normal_cdf` deep in the tail) is still rounded only once.
fn erfc_dd(x: Dd, log2_scale: i32) -> Dd {
    let scale = scale_pow2(1.0_f64, log2_scale);
    if x.hi < 0.0_f64 {
        return Dd::new(2.0_f64, 0.0_f64)
            .sub(erfc_dd(Dd::new(-x.hi, -x.lo), 0_i32))
            .mul_f64(scale);
    }
    if x.hi > 27.3_f64 {
        return Dd::new(0.0_f64, 0.0_f64);
    }
    let x_squared = x.mul(x);
    let one = Dd::new(1.0_f64, 0.0_f64);

    if x.hi < 0.5_f64 {
        // erf(x) = 2/sqrt(pi) sum (-1)^n x^(2n+1) / (n! (2n+1))
        let mut power_term = x;
        let mut sum = x;
        for index in 1_u8..=20_u8 {
            let n = f64::from(index);
            power_term = power_term.mul(x_squared).div(Dd::new(-n, 0.0_f64));
            sum = sum.add(power_term.div(Dd::new(2.0_f64 * n + 1.0_f64, 0.0_f64)));
        }
        return one.sub(sum.mul(FRAC_2_SQRT_PI)).mul_f64(scale);
    }

    let minus_x_squared = Dd::new(-x_squared.hi, -x_squared.lo);
    let gaussian = exp_dd(minus_x_squared);

    if x.hi < 4.0_f64 {
        // erf(x) = 2/sqrt(pi) e^(-x^2) sum (2x^2)^n x / (1*3*...*(2n+1)); all terms positive.
        let two_x_squared = x_squared.mul_f64(2.0_f64);
        let mut term = x;
        let mut sum = x;
        for index in 1_u8..=200_u8 {
            term = term
                .mul(two_x_squared)
                .div(Dd::new(2.0_f64 * f64::from(index) + 1.0_f64, 0.0_f64));
            sum = sum.add(term);
            if term.hi < sum.hi * 1e-34_f64 {
                break;
            }
        }
        return one
            .sub(sum.mul(gaussian).mul(FRAC_2_SQRT_PI))
            .mul_f64(scale);
    }

    // erfc(x) = e^(-x^2) / sqrt(pi) * 1 / (x + (1/2) / (x + 1 / (x + (3/2) / (x + ...))))
    let mut fraction = x;
    for index in (1_u8..=60_u8).rev() {
        fraction = x.add(Dd::new(0.5_f64 * f64::from(index), 0.0_f64).div(fraction));
    }
    let ratio = FRAC_1_SQRT_PI.div(fraction);
    if gaussian.hi < f64::from_bits(0x07B0_0000_0000_0000_u64) {
        // Below 2^-900 the low words of the intermediate products would become subnormal and
        // lose bits, so combine everything in the exponent and round the result once.
        // ln(hi + lo) = ln(hi) + lo/hi - (lo/hi)^2/2 + ...; the square is below 2^-106.
        let ln_ratio = ln_dd(ratio.hi).add_f64(ratio.lo / ratio.hi);
        let ln_scale = LN2.mul_f64(f64::from(log2_scale));
        return exp_dd(minus_x_squared.add(ln_ratio).add(ln_scale));
    }
    gaussian.mul(ratio).mul_f64(scale)
}

/// Multiplies `value` by `2^power`, splitting the scale to avoid intermediate overflow.
fn scale_pow2(value: f64, power: i32) -> f64 {
    let mut result = value;
    let mut remaining = power;
    while remaining > 1_000_i32 {
        result *= f64::from_bits(0x7E70_0000_0000_0000_u64); // 2^1000
        remaining = remaining.saturating_sub(1_000_i32);
    }
    while remaining < -1_000_i32 {
        result *= f64::from_bits(0x0170_0000_0000_0000_u64); // 2^-1000
        remaining = remaining.saturating_add(1_000_i32);
    }
    let biased = u64::try_from(remaining.saturating_add(1023_i32)).unwrap_or(0_u64);
    result * f64::from_bits(biased << 52_u32)
}
