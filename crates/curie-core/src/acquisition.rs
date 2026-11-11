//! Gate-series acquisition: fixed-deadline software gates and hardware-timed gates.

/// Current operating state of the data acquisition system.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DaqState {
    /// System is standby, not collecting data.
    #[default]
    Idle,
    /// System is actively counting pulses and logging.
    Acquiring,
    /// System paused the current experiment.
    Paused,
    /// Preset measurement limit has been reached; counting gate closed.
    Completed,
}

/// One completed counting gate.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataRecord {
    /// End of the counting gate, in milliseconds since experiment start.
    pub timestamp_ms: u64,
    /// Duration of the counting gate in milliseconds.
    pub gate_ms: u32,
    /// Number of pulses counted in the gate.
    pub interval_pulses: u32,
}

/// Gate series of one run: back-to-back software gates or gates timed by the hardware.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CurieSession {
    /// Current DAQ operational state.
    pub state: DaqState,
    /// Configured gate length in milliseconds.
    pub sample_interval_ms: u32,
    /// Number of gates after which the run stops by itself (0 = continuous).
    pub target_samples: u32,
    /// Number of gates completed in the active run.
    pub completed_samples: u32,
    /// Monotonic timestamp of session start in milliseconds.
    pub session_start_ms: u64,
    /// Start of the current software-timed gate, in milliseconds.
    ///
    /// Gates are back to back with fixed deadlines (`start + n * interval`), so a late
    /// [`Self::tick`] never stretches a gate or shifts later ones.
    pub last_tick_ms: u64,
    /// Pulses accumulated during the current software-timed gate.
    pub interval_pulses: u32,
}

impl CurieSession {
    /// Creates an idle session with the given gate length.
    #[must_use]
    pub const fn new(sample_interval_ms: u32) -> Self {
        Self {
            state: DaqState::Idle,
            sample_interval_ms,
            target_samples: 0_u32,
            completed_samples: 0_u32,
            session_start_ms: 0_u64,
            last_tick_ms: 0_u64,
            interval_pulses: 0_u32,
        }
    }

    /// Starts a new run, or resumes a paused one with a fresh gate at `now_ms`.
    ///
    /// Resuming keeps the run clock, so the time axis stays physical time.
    pub const fn start(&mut self, now_ms: u64) {
        if matches!(self.state, DaqState::Idle | DaqState::Completed) {
            self.session_start_ms = now_ms;
            self.completed_samples = 0_u32;
        }
        self.last_tick_ms = now_ms;
        self.interval_pulses = 0_u32;
        self.state = DaqState::Acquiring;
    }

    /// Pauses the run and discards the partially counted gate, so every recorded gate has
    /// its full duration.
    pub fn pause(&mut self) {
        if self.state == DaqState::Acquiring {
            self.interval_pulses = 0_u32;
            self.state = DaqState::Paused;
        }
    }

    /// Returns to Idle and clears the counters.
    pub const fn reset(&mut self) {
        self.state = DaqState::Idle;
        self.interval_pulses = 0_u32;
        self.completed_samples = 0_u32;
    }

    /// Adds pulses to the current software-timed gate (ignored unless acquiring).
    pub const fn ingest_pulses(&mut self, pulses: u32) {
        if matches!(self.state, DaqState::Acquiring) {
            self.interval_pulses = self.interval_pulses.saturating_add(pulses);
        }
    }

    /// Closes the current software-timed gate if its deadline has passed.
    ///
    /// Gates have fixed deadlines, so call this in a loop until it returns `None` to catch
    /// up after a delay. To keep pulses in the right gate, call it with the timestamp of
    /// each batch of pulses *before* ingesting that batch.
    pub fn tick(&mut self, now_ms: u64) -> Option<DataRecord> {
        let gate_end_ms = self
            .last_tick_ms
            .saturating_add(u64::from(self.sample_interval_ms));
        if self.state != DaqState::Acquiring || now_ms < gate_end_ms {
            return None;
        }
        let pulses = self.interval_pulses;
        self.interval_pulses = 0_u32;
        self.last_tick_ms = gate_end_ms;
        Some(self.record_gate(gate_end_ms, self.sample_interval_ms, pulses))
    }

    /// Records a gate timed by the hardware (for example the Cobra `Z` gate).
    ///
    /// `gate_end_ms` is when the count was read back, on the same clock as [`Self::start`].
    /// Ignored unless the session is acquiring.
    pub fn ingest_gate(
        &mut self,
        gate_end_ms: u64,
        gate_ms: u32,
        pulses: u32,
    ) -> Option<DataRecord> {
        if self.state != DaqState::Acquiring {
            return None;
        }
        self.last_tick_ms = gate_end_ms;
        Some(self.record_gate(gate_end_ms, gate_ms, pulses))
    }

    /// Counts a completed gate towards the target and builds its record.
    const fn record_gate(&mut self, gate_end_ms: u64, gate_ms: u32, pulses: u32) -> DataRecord {
        self.completed_samples = self.completed_samples.saturating_add(1_u32);
        if self.target_samples > 0_u32 && self.completed_samples >= self.target_samples {
            self.state = DaqState::Completed;
        }
        DataRecord {
            timestamp_ms: gate_end_ms.saturating_sub(self.session_start_ms),
            gate_ms,
            interval_pulses: pulses,
        }
    }
}
