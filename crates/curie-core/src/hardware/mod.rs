//! Hardware abstraction: counting devices and detector probes.
//!
//! - [`Device`]: the contract between a counting front-end and the acquisition engine.
//! - [`detector`]: detector profiles and the safe bias-voltage selection.
//! - [`phywe`]: PHYWE Cobra serial protocol (commands and reply parsing).
//! - [`mcu`]: bare-metal microcontroller pulse counter.
//!
//! Sweep positioning and filter changes are manual operator actions; the core provides
//! step bookkeeping only and does not drive actuators.

mod detector;
mod mcu;
mod phywe;

pub use detector::{DeadTimeBasis, DetectorProfile, HardwareConfig};
pub use mcu::McuPulseCounter;
pub use phywe::{
    COBRA_COUNTER_CHANNEL, COBRA_MAX_GATE_US, COBRA_MIN_GATE_US, COBRA_TERMINATOR, CobraCommand,
    CobraEdge, CobraLineReader, CobraReply,
};

use alloc::string::String;

/// Request from the acquisition engine to a device.
#[expect(
    clippy::exhaustive_enums,
    reason = "a closed protocol: every device must handle every command"
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceCommand {
    /// Count gates of `gate_ms`; with `repeat`, the next gate starts as soon as one ends.
    /// Devices that stream pulses ignore it (the host times their gates).
    StartGates {
        /// Gate length in milliseconds.
        gate_ms: u32,
        /// Keep counting gates back to back until [`DeviceCommand::StopGates`].
        repeat: bool,
    },
    /// Abandon the running gate and stop.
    StopGates,
}

/// Something a device observed. Times are milliseconds on the host clock.
#[expect(
    clippy::exhaustive_enums,
    reason = "a closed protocol: the engine must handle every event"
)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceEvent {
    /// The device answered its initialisation and can count.
    Ready,
    /// Pulses from a streaming device, observed at `at_ms` (the host times the gates).
    Pulses {
        /// When the pulses were received.
        at_ms: u64,
        /// Number of pulses.
        count: u32,
    },
    /// A gate timed by the device itself has finished.
    Gate {
        /// When the gate was started.
        started_ms: u64,
        /// When its count was read back.
        at_ms: u64,
        /// Gate length in milliseconds.
        gate_ms: u32,
        /// Pulses counted in the gate.
        count: u32,
    },
    /// A non-fatal device message (firmware error, odd reply).
    Warning(String),
    /// The link failed; the device delivers nothing more.
    Failed(String),
}

/// A counting front-end, seen from the acquisition engine.
///
/// A device either times gates itself (it answers [`DeviceCommand::StartGates`] with
/// [`DeviceEvent::Gate`], like the PHYWE Cobra) or streams timestamped
/// [`DeviceEvent::Pulses`] and leaves the gate timing to the host (see
/// [`crate::CurieSession::tick`]). Supporting new hardware means implementing this trait;
/// the engine does not change.
pub trait Device {
    /// Handles a command sent at `now_ms`.
    fn send(&mut self, command: DeviceCommand, now_ms: u64);

    /// Returns the next event observed up to `now_ms`, or `None` when there is none yet.
    fn poll(&mut self, now_ms: u64) -> Option<DeviceEvent>;
}
