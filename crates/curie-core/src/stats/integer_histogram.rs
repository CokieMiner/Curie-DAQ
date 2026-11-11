//! Integer-aligned frequency histogram for raw counting data.
//!
//! Counts per gate are integers, so every bin covers a whole number of integer values
//! (`width` of them) and is aligned to a multiple of the width. Nothing is clamped into an
//! edge bin: every observation lands in the bin that contains it.

use alloc::vec::Vec;

/// Largest number of bins an [`IntegerHistogram`] allocates; wider data gets wider bins.
pub const INTEGER_HISTOGRAM_MAX_BINS: u32 = 4_096_u32;

/// How the bin width of an [`IntegerHistogram`] is chosen.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BinWidthMode {
    /// Smallest integer width giving at most the requested number of bins.
    #[default]
    Auto,
    /// Fixed number of integer counts per bin.
    Fixed(u32),
}

/// Frequency histogram of integer counts with integer-aligned bins.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IntegerHistogram {
    /// Lowest count covered by the first bin (a multiple of `width`).
    pub origin: u32,
    /// Number of integer values covered by each bin.
    pub width: u32,
    /// Observed frequency of every bin, lowest bin first.
    pub frequencies: Vec<u32>,
    /// Total number of observations.
    pub total: u64,
}

/// One bin of an [`IntegerHistogram`].
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HistogramBin {
    /// Lowest integer count in the bin.
    pub low: u32,
    /// Highest integer count in the bin.
    pub high: u32,
    /// Centre of the bin on the count axis.
    pub center: f64,
    /// Number of observations in the bin.
    pub frequency: u32,
}

impl IntegerHistogram {
    /// Builds a histogram of `counts`.
    ///
    /// [`BinWidthMode::Fixed`] uses that many integers per bin; [`BinWidthMode::Auto`]
    /// picks the smallest integer width giving at most `target_bins` bins. Bins start at
    /// the largest multiple of the width not above the minimum count. If the range would
    /// need more than [`INTEGER_HISTOGRAM_MAX_BINS`] bins, the width is increased. Returns
    /// `None` for an empty slice or when allocation fails.
    #[must_use]
    pub fn from_counts(counts: &[u32], mode: BinWidthMode, target_bins: u32) -> Option<Self> {
        let min = counts.iter().copied().min()?;
        let max = counts.iter().copied().max()?;
        let span = u64::from(max.saturating_sub(min)).saturating_add(1_u64);

        let requested = match mode {
            BinWidthMode::Fixed(width) => u64::from(width),
            BinWidthMode::Auto => span.div_ceil(u64::from(target_bins.max(1_u32))),
        };
        let max_bins = u64::from(INTEGER_HISTOGRAM_MAX_BINS);
        let width = requested
            .max(span.div_ceil(max_bins.saturating_sub(1_u64)))
            .max(1_u64);
        let bin_width = u32::try_from(width).unwrap_or(u32::MAX);

        let origin = min.saturating_sub(min.checked_rem(bin_width).unwrap_or(0_u32));
        let bin_count = max
            .saturating_sub(origin)
            .checked_div(bin_width)
            .unwrap_or(0_u32)
            .saturating_add(1_u32);
        let bin_count_usize = usize::try_from(bin_count).ok()?;

        let mut frequencies = Vec::new();
        frequencies.try_reserve_exact(bin_count_usize).ok()?;
        frequencies.resize(bin_count_usize, 0_u32);

        let mut total = 0_u64;
        for &value in counts {
            let index = value
                .saturating_sub(origin)
                .checked_div(bin_width)
                .unwrap_or(0_u32);
            if let Some(slot) = usize::try_from(index)
                .ok()
                .and_then(|i| frequencies.get_mut(i))
            {
                *slot = slot.saturating_add(1_u32);
                total = total.saturating_add(1_u64);
            }
        }

        Some(Self {
            origin,
            width: bin_width,
            frequencies,
            total,
        })
    }

    /// Bounds, centre and frequency of bin `index`, or `None` past the last bin.
    #[must_use]
    pub fn bin(&self, index: usize) -> Option<HistogramBin> {
        let frequency = *self.frequencies.get(index)?;
        let low = self
            .origin
            .checked_add(u32::try_from(index).ok()?.checked_mul(self.width)?)?;
        let high = low.checked_add(self.width.saturating_sub(1_u32))?;
        Some(HistogramBin {
            low,
            high,
            center: f64::midpoint(f64::from(low), f64::from(high)),
            frequency,
        })
    }
}
