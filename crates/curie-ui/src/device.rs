//! The counting devices the app can measure with.
//!
//! Every device implements [`curie_core::Device`]: it either times gates itself or streams
//! timestamped pulses. The engine in `app.rs` only asks [`DeviceKind::times_gates`]; adding
//! hardware means a new variant here and its `Device` implementation.

use crate::serial::SerialProtocol;

/// A counting device selectable in 4:CONFIG.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKind {
    /// Synthetic Poisson counts (see `simulation.rs`), for practice without a source.
    Simulator,
    /// A counter on a serial port.
    Serial(SerialProtocol),
}

impl DeviceKind {
    pub const ALL: [Self; 4] = [
        Self::Serial(SerialProtocol::PhyweCobra),
        Self::Serial(SerialProtocol::AsciiText),
        Self::Serial(SerialProtocol::RawBytePulses),
        Self::Simulator,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Simulator => "SIMULATOR",
            Self::Serial(SerialProtocol::PhyweCobra) => "PHYWE COBRA",
            Self::Serial(SerialProtocol::AsciiText) => "ASCII LINES",
            Self::Serial(SerialProtocol::RawBytePulses) => "RAW BYTES",
        }
    }

    /// Whether the device times each gate itself (the app asks for gates and receives
    /// counts) instead of streaming pulses that the PC sorts into gates.
    pub const fn times_gates(self) -> bool {
        matches!(
            self,
            Self::Simulator | Self::Serial(SerialProtocol::PhyweCobra)
        )
    }
}
