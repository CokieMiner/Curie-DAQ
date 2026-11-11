//! Guided step-by-step sweep engine for manual radiation attenuation measurements.
//!
//! The operator performs every physical change by hand. At each position a fixed number of
//! measurement states (filter in or out, absorber thickness, ...) is measured in order.
//! When a timed measurement completes, the session advances to the next state, and moves
//! the active axis (distance or angle) by one step after the final state.

use alloc::vec::Vec;

/// Largest number of measurement states per position.
pub const MAX_SWEEP_STATES: usize = 64;
/// Largest angle magnitude accepted for angular sweeps, in degrees.
const MAX_SWEEP_ANGLE_DEG: i16 = 180_i16;

/// Position coordinate a sweep steps through.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SweepAxis {
    /// Source-to-detector distance in millimetres (attenuation in air).
    #[default]
    Distance,
    /// Detection angle in degrees (angular distribution, magnetic deflection).
    Angle,
}

/// A single measurement of one state at one sweep position.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SweepRecord {
    /// Step sequence index (1-based).
    pub step_index: u16,
    /// Distance from source to detector in millimeters.
    pub distance_mm: u16,
    /// Detection angle relative to normal in degrees.
    pub angle_deg: i16,
    /// Index of the measurement state at this position.
    pub state_index: u8,
    /// Pulses detected during the sample window.
    pub pulses: u32,
    /// Duration of the sample window in milliseconds.
    pub duration_ms: u32,
    /// Measured count rate in counts per second.
    pub cps: f32,
}

/// The most recent record of each measurement state at one `(distance, angle)` position.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SweepGroup {
    /// Source-to-detector distance in millimetres.
    pub distance_mm: u16,
    /// Detection angle relative to normal in degrees.
    pub angle_deg: i16,
    /// Latest record of each state index at this position (`None` while not measured).
    pub records: [Option<SweepRecord>; MAX_SWEEP_STATES],
}

/// Session manager for guided manual spatial sweeps.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct SweepSession {
    points: Vec<SweepRecord>,
    target_step_index: u16,
    target_distance_mm: u16,
    target_angle_deg: i16,
    axis: SweepAxis,
    state_count: usize,
    state_index: usize,
    step_duration_ms: u32,
    step_size_mm: u16,
    measuring_start_ms: Option<u64>,
    current_pulses: u32,
}

impl SweepSession {
    /// Creates an empty distance sweep with two states per position.
    #[must_use]
    pub const fn new(step_duration_ms: u32) -> Self {
        Self {
            points: Vec::new(),
            target_step_index: 1_u16,
            target_distance_mm: 0_u16,
            target_angle_deg: 0_i16,
            axis: SweepAxis::Distance,
            state_count: 2_usize,
            state_index: 0_usize,
            step_duration_ms,
            step_size_mm: 10_u16,
            measuring_start_ms: None,
            current_pulses: 0_u32,
        }
    }

    /// Sets the number of states per position, clamped to `1..=MAX_SWEEP_STATES`.
    ///
    /// Any in-progress counting window is cancelled so a record cannot be attributed to the
    /// wrong configuration; a state index outside the new range returns to the first state.
    pub fn set_state_count(&mut self, count: usize) {
        self.state_count = count.clamp(1_usize, MAX_SWEEP_STATES);
        if self.state_index >= self.state_count {
            self.state_index = 0_usize;
        }
        self.cancel_measurement();
    }

    /// Number of measurement states per position.
    #[must_use]
    pub const fn state_count(&self) -> usize {
        self.state_count
    }

    /// Current measurement-state index, zero-based.
    #[must_use]
    pub const fn state_index(&self) -> usize {
        self.state_index
    }

    /// Selects a measurement state directly; out-of-range indices are ignored.
    ///
    /// Changing the state cancels any in-progress counting window, because the accumulated
    /// pulses belong to the previous physical configuration.
    pub const fn set_state_index(&mut self, index: usize) {
        if index < self.state_count {
            if index != self.state_index {
                self.cancel_measurement();
            }
            self.state_index = index;
        }
    }

    /// Target distance in millimeters for the upcoming point.
    #[must_use]
    pub const fn target_distance_mm(&self) -> u16 {
        self.target_distance_mm
    }

    /// Sets the target distance in millimeters.
    pub const fn set_target_distance_mm(&mut self, distance_mm: u16) {
        self.target_distance_mm = distance_mm;
    }

    /// Target detection angle in degrees.
    #[must_use]
    pub const fn target_angle_deg(&self) -> i16 {
        self.target_angle_deg
    }

    /// Sets the target detection angle in degrees, clamped to ±180°.
    pub fn set_target_angle_deg(&mut self, angle_deg: i16) {
        self.target_angle_deg = angle_deg.clamp(-MAX_SWEEP_ANGLE_DEG, MAX_SWEEP_ANGLE_DEG);
    }

    /// Coordinate advanced after the last state of each position.
    #[must_use]
    pub const fn axis(&self) -> SweepAxis {
        self.axis
    }

    /// Selects which coordinate the sweep steps through.
    ///
    /// The step size is read as millimetres for [`SweepAxis::Distance`] and as degrees for
    /// [`SweepAxis::Angle`].
    pub const fn set_axis(&mut self, axis: SweepAxis) {
        self.axis = axis;
    }

    /// Configured step increment (millimetres or degrees).
    #[must_use]
    pub const fn step_size_mm(&self) -> u16 {
        self.step_size_mm
    }

    /// Sets the step increment (at least 1).
    pub fn set_step_size_mm(&mut self, size_mm: u16) {
        self.step_size_mm = size_mm.max(1_u16);
    }

    /// Configured counting window per measurement in milliseconds.
    #[must_use]
    pub const fn step_duration_ms(&self) -> u32 {
        self.step_duration_ms
    }

    /// Sets the counting window per measurement in milliseconds.
    pub const fn set_step_duration_ms(&mut self, duration_ms: u32) {
        self.step_duration_ms = duration_ms;
    }

    /// Returns `true` if a step measurement is currently accumulating pulses.
    #[must_use]
    pub const fn is_measuring(&self) -> bool {
        self.measuring_start_ms.is_some()
    }

    /// All recorded measurements in order.
    #[must_use]
    pub fn points(&self) -> &[SweepRecord] {
        &self.points
    }

    /// Recorded positions in order of their first measurement, each with the latest record
    /// of every state (repeating a measurement replaces the earlier value).
    #[must_use]
    pub fn groups(&self) -> Vec<SweepGroup> {
        let mut groups: Vec<SweepGroup> = Vec::new();
        for record in &self.points {
            let position = groups.iter().position(|group| {
                group.distance_mm == record.distance_mm && group.angle_deg == record.angle_deg
            });
            let index = position.unwrap_or_else(|| {
                groups.push(SweepGroup {
                    distance_mm: record.distance_mm,
                    angle_deg: record.angle_deg,
                    records: [None; MAX_SWEEP_STATES],
                });
                groups.len().saturating_sub(1_usize)
            });
            if let Some(slot) = groups
                .get_mut(index)
                .and_then(|group| group.records.get_mut(usize::from(record.state_index)))
            {
                *slot = Some(*record);
            }
        }
        groups
    }

    /// Starts measuring the current state.
    ///
    /// The operator is responsible for setting the physical position and configuration first.
    pub const fn start_step(&mut self, now_ms: u64) {
        self.current_pulses = 0_u32;
        self.measuring_start_ms = Some(now_ms);
    }

    /// Adds pulses to the running step (ignored when idle).
    pub const fn ingest_pulses(&mut self, pulses: u32) {
        if self.measuring_start_ms.is_some() {
            self.current_pulses = self.current_pulses.saturating_add(pulses);
        }
    }

    /// Elapsed time of the current step in milliseconds, or zero when idle.
    #[must_use]
    pub const fn step_elapsed_ms(&self, now_ms: u64) -> u64 {
        match self.measuring_start_ms {
            Some(start_ms) => now_ms.saturating_sub(start_ms),
            None => 0_u64,
        }
    }

    /// Removes the most recent measurement and restores its position, step and state.
    pub fn pop_last_point(&mut self) -> Option<SweepRecord> {
        let record = self.points.pop()?;
        self.target_step_index = record.step_index;
        self.target_distance_mm = record.distance_mm;
        self.target_angle_deg = record.angle_deg;
        if usize::from(record.state_index) < self.state_count {
            self.state_index = usize::from(record.state_index);
        }
        self.cancel_measurement();
        Some(record)
    }

    /// Finishes the running step once its duration has elapsed.
    ///
    /// For software-timed counting, call this with the timestamp of each batch of pulses
    /// *before* ingesting it, so pulses arriving after the deadline are not counted.
    pub fn tick(&mut self, now_ms: u64) -> Option<SweepRecord> {
        let start_ms = self.measuring_start_ms?;
        if now_ms.saturating_sub(start_ms) < u64::from(self.step_duration_ms) {
            return None;
        }
        self.finish_step(self.current_pulses, self.step_duration_ms)
    }

    /// Records the running step with an externally measured count and gate duration.
    ///
    /// Used for hardware-timed gates (the Cobra counts the gate itself). Stores the point,
    /// advances to the next state (and position after the last state), and returns `None`
    /// if no step is running.
    pub fn finish_step(&mut self, pulses: u32, duration_ms: u32) -> Option<SweepRecord> {
        let _started_ms = self.measuring_start_ms.take()?;
        #[expect(
            clippy::as_conversions,
            clippy::cast_precision_loss,
            reason = "Step milliseconds and pulses converted to float for the rate"
        )]
        let cps = if duration_ms > 0_u32 {
            (pulses as f32) / ((duration_ms as f32) / 1000.0_f32)
        } else {
            0.0_f32
        };
        let record = SweepRecord {
            step_index: self.target_step_index,
            distance_mm: self.target_distance_mm,
            angle_deg: self.target_angle_deg,
            state_index: u8::try_from(self.state_index).unwrap_or_default(),
            pulses,
            duration_ms,
            cps,
        };
        self.points.push(record);
        self.current_pulses = 0_u32;
        self.target_step_index = self.target_step_index.saturating_add(1_u16);
        self.state_index = self.state_index.saturating_add(1_usize);
        if self.state_index >= self.state_count {
            self.state_index = 0_usize;
            match self.axis {
                SweepAxis::Distance => {
                    self.target_distance_mm =
                        self.target_distance_mm.saturating_add(self.step_size_mm);
                }
                SweepAxis::Angle => {
                    let step = i16::try_from(self.step_size_mm).unwrap_or(MAX_SWEEP_ANGLE_DEG);
                    self.target_angle_deg = self
                        .target_angle_deg
                        .saturating_add(step)
                        .min(MAX_SWEEP_ANGLE_DEG);
                }
            }
        }
        Some(record)
    }

    /// Cancels any in-progress counting window and clears its partial pulse count.
    pub const fn cancel_measurement(&mut self) {
        self.measuring_start_ms = None;
        self.current_pulses = 0_u32;
    }
}
