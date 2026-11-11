#![no_std]

//! # Curie-DAQ Core Library
//!
//! `#![no_std] + alloc` core of Curie-DAQ, with no dependencies: the PHYWE Cobra protocol,
//! gate series and manual sweeps, datasets, counting statistics and fits.
//!
//! - `acquisition`: gate series with fixed software deadlines or hardware-timed gates.
//! - `analysis`: Poisson / Gaussian distribution fits and the decay fit.
//! - `hardware`: the `Device` contract, the Cobra protocol, an MCU counter, detector profiles.
//! - `math`: dependency-free double-precision special functions.
//! - `stats`: running sample moments and integer-aligned histograms.
//! - `storage`: datasets and the saved-dataset registry.
//! - `sweep`: guided distance / angle sweeps with several states per position.

extern crate alloc;

mod acquisition;
mod analysis;
mod hardware;
mod math;
mod stats;
mod storage;
mod sweep;

pub use acquisition::{CurieSession, DaqState, DataRecord};
pub use analysis::{
    DecayBackground, DecaySample, ExponentialDecayFit, GaussianFit, GoodnessOfFit, PoissonFit,
};
pub use hardware::{
    COBRA_COUNTER_CHANNEL, COBRA_MAX_GATE_US, COBRA_MIN_GATE_US, COBRA_TERMINATOR, CobraCommand,
    CobraEdge, CobraLineReader, CobraReply, DeadTimeBasis, DetectorProfile, Device, DeviceCommand,
    DeviceEvent, HardwareConfig, McuPulseCounter,
};
pub use math::F64Math;
pub use stats::{
    BinWidthMode, HistogramBin, INTEGER_HISTOGRAM_MAX_BINS, IntegerHistogram, SampleMoments,
};
pub use storage::{Dataset, DatasetRegistry, DatasetType, StoredPoint};
pub use sweep::{MAX_SWEEP_STATES, SweepAxis, SweepGroup, SweepRecord, SweepSession};
