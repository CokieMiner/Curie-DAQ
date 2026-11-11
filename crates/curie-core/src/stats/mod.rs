//! Counting statistics: running sample moments and integer-aligned histograms.

mod integer_histogram;
mod moments;

pub use integer_histogram::{
    BinWidthMode, HistogramBin, INTEGER_HISTOGRAM_MAX_BINS, IntegerHistogram,
};
pub use moments::SampleMoments;
