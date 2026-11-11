//! Integration tests for integer-aligned histograms and running sample moments.

use curie_core::{BinWidthMode, INTEGER_HISTOGRAM_MAX_BINS, IntegerHistogram, SampleMoments};

#[test]
fn test_unit_width_histogram_has_one_bin_per_count_and_no_clamping() {
    // 54 distinct values: the old 32-bin histogram piled the tail into its last bin.
    let counts: Vec<u32> = (35_u32..=88_u32).collect();
    let histogram = IntegerHistogram::from_counts(&counts, BinWidthMode::Fixed(1_u32), 32_u32)
        .expect("non-empty counts");

    assert_eq!(histogram.origin, 35_u32);
    assert_eq!(histogram.width, 1_u32);
    assert_eq!(
        histogram.frequencies.len(),
        54_usize,
        "one bin per integer value"
    );
    assert!(
        histogram
            .frequencies
            .iter()
            .all(|&frequency| frequency == 1_u32),
        "every value lands in its own bin"
    );
    assert_eq!(histogram.total, 54_u64);
    assert_eq!(histogram.bin(53_usize).map(|bin| bin.low), Some(88_u32));
    assert!(histogram.bin(54_usize).is_none(), "no bin past the end");
}

#[test]
fn test_wide_bins_are_aligned_and_centred() {
    let counts = [7_u32, 8_u32, 9_u32, 10_u32, 14_u32];
    let histogram = IntegerHistogram::from_counts(&counts, BinWidthMode::Fixed(5_u32), 32_u32)
        .expect("non-empty counts");

    assert_eq!(
        histogram.origin, 5_u32,
        "bins start at a multiple of the width"
    );
    assert_eq!(histogram.frequencies, [3_u32, 2_u32]);
    let first = histogram.bin(0_usize).expect("first bin exists");
    assert_eq!(
        (first.low, first.high, first.frequency),
        (5_u32, 9_u32, 3_u32)
    );
    assert_eq!(
        first.center.to_bits(),
        7.0_f64.to_bits(),
        "centre of 5..=9 is 7, not the left edge"
    );
}

#[test]
fn test_auto_width_and_bin_cap() {
    let narrow: Vec<u32> = (0_u32..10_u32).collect();
    let auto_narrow = IntegerHistogram::from_counts(&narrow, BinWidthMode::Auto, 30_u32)
        .expect("non-empty counts");
    assert_eq!(auto_narrow.width, 1_u32);

    let wide: Vec<u32> = (0_u32..300_u32).collect();
    let auto_wide =
        IntegerHistogram::from_counts(&wide, BinWidthMode::Auto, 30_u32).expect("non-empty counts");
    assert_eq!(auto_wide.width, 10_u32);
    assert_eq!(auto_wide.frequencies.len(), 30_usize);

    let extreme = [0_u32, 10_000_000_u32];
    let capped = IntegerHistogram::from_counts(&extreme, BinWidthMode::Fixed(1_u32), 30_u32)
        .expect("non-empty counts");
    assert!(
        u32::try_from(capped.frequencies.len()).expect("bin count fits u32")
            <= INTEGER_HISTOGRAM_MAX_BINS,
        "a huge range widens the bins instead of allocating millions"
    );
    assert_eq!(capped.total, 2_u64, "no observation is lost");
    assert!(IntegerHistogram::from_counts(&[], BinWidthMode::Auto, 30_u32).is_none());
}

#[test]
fn test_moments_stay_exact_for_a_large_mean() {
    // 10^8 +- 1: the naive sum-of-squares formula loses this variance.
    let mut moments = SampleMoments::default();
    for count in [99_999_999_u32, 100_000_000_u32, 100_000_001_u32] {
        moments.push(f64::from(count));
    }

    assert_eq!(moments.count.to_bits(), 3.0_f64.to_bits());
    assert!((moments.mean - 1e8_f64).abs() < 1e-6_f64, "mean is exact");
    assert!(
        (moments.variance() - 1.0_f64).abs() < 1e-9_f64,
        "unbiased variance is 1"
    );
    assert!(
        moments
            .fano_factor()
            .is_some_and(|fano| (fano - 1e-8_f64).abs() < 1e-15_f64),
        "Fano factor = variance / mean"
    );
    assert!((moments.std_error_of_mean() - (1.0_f64 / 3.0_f64).sqrt()).abs() < 1e-12_f64);
}

#[test]
fn test_moments_handle_too_few_values() {
    let empty = SampleMoments::default();
    assert_eq!(empty.variance().to_bits(), 0.0_f64.to_bits());
    assert!(empty.fano_factor().is_none());

    let mut single = SampleMoments::default();
    single.push(5.0_f64);
    assert_eq!(single.mean.to_bits(), 5.0_f64.to_bits());
    assert!(single.fano_factor().is_none(), "variance needs two values");
}
