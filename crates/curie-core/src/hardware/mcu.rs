//! Bare-metal microcontroller pulse counter.

use super::{Device, DeviceCommand, DeviceEvent};

/// Pulse counter fed by a pin interrupt or a timer capture register.
///
/// It streams pulses: every [`Device::poll`] reports the pulses counted since the previous
/// one, and the host times the gates.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct McuPulseCounter {
    counter: u32,
}

impl McuPulseCounter {
    /// Counts one pulse; call it from the interrupt service routine.
    pub const fn on_interrupt(&mut self) {
        self.counter = self.counter.saturating_add(1_u32);
    }

    /// Replaces the count with a value read from a hardware timer-capture register.
    pub const fn set_hardware_counter(&mut self, value: u32) {
        self.counter = value;
    }
}

impl Device for McuPulseCounter {
    fn send(&mut self, _command: DeviceCommand, _now_ms: u64) {}

    fn poll(&mut self, now_ms: u64) -> Option<DeviceEvent> {
        let count = core::mem::take(&mut self.counter);
        (count > 0_u32).then_some(DeviceEvent::Pulses {
            at_ms: now_ms,
            count,
        })
    }
}
