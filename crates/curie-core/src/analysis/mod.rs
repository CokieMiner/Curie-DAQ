//! Statistical fits for decay curves and counting distributions.
//!
//! All fits work in `f64` on the raw counts (never on rounded rates or histogram centres)
//! and report parameter uncertainties and a goodness of fit. They are strictly `#![no_std]`
//! with no external dependencies (see [`crate::F64Math`]).

mod decay;
mod gaussian;
mod goodness;
mod poisson;

pub use decay::{DecayBackground, DecaySample, ExponentialDecayFit};
pub use gaussian::GaussianFit;
pub use goodness::GoodnessOfFit;
pub use poisson::PoissonFit;
