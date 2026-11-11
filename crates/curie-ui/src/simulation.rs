//! Synthetic counter for practising without a source or hardware.

use curie_core::{Device, DeviceCommand, DeviceEvent, F64Math};

/// A simulated counter that times its own gates, like the Cobra: one Poisson draw per gate.
///
/// The source decays with `half_life_s` from `source_start_ms`. During a sweep, `bench`
/// holds the operator's `(distance_mm, angle_deg, state_index)` and the rate follows an
/// attenuation bench: inverse square with distance, a Gaussian angular profile and a
/// transmission factor per state (state 1 = no filter).
#[derive(Debug, Clone, Copy)]
pub struct SimulatedCounter {
    /// Xorshift generator state (never zero).
    pub state: u64,
    /// Ambient background (counts/s).
    pub bg_rate_cps: f64,
    /// Source rate at `source_start_ms` (counts/s).
    pub base_rate_cps: f64,
    /// Half-life of the source in seconds; 0 keeps the rate constant.
    pub half_life_s: f64,
    /// When the source was prepared (start of the run), on the app clock.
    pub source_start_ms: u64,
    /// Bench position of a sweep step, or `None` for a gate series.
    pub bench: Option<(u16, i16, usize)>,
    /// Running gates: `(start of the next gate, gate length, repeat)`.
    pub job: Option<(u64, u32, bool)>,
}

impl SimulatedCounter {
    pub const fn new(seed: u64) -> Self {
        Self {
            state: seed | 1,
            bg_rate_cps: 0.6,
            base_rate_cps: 24.0,
            half_life_s: 0.0,
            source_start_ms: 0,
            bench: None,
            job: None,
        }
    }

    /// Counts in one gate of `gate_s` seconds centred `t_mid_s` after the source start.
    fn gate_counts(&mut self, gate_s: f64, t_mid_s: f64) -> u32 {
        let source_rate = match self.bench {
            None if self.half_life_s > 0.0 => {
                self.base_rate_cps
                    * F64Math::exp(-core::f64::consts::LN_2 * t_mid_s / self.half_life_s)
            }
            None => self.base_rate_cps,
            Some((distance_mm, angle_deg, state_index)) => {
                let d_norm = (f64::from(distance_mm) / 20.0).max(0.1);
                let angle = f64::from(angle_deg) / 40.0;
                let transmission = [1.0, 0.35, 0.18, 0.08]
                    .get(state_index)
                    .copied()
                    .unwrap_or(0.02);
                120.0 / f64::mul_add(d_norm, d_norm, 1.0)
                    * F64Math::exp(-angle * angle)
                    * transmission
            }
        };
        let mean = (source_rate + self.bg_rate_cps).max(0.0) * gate_s;

        // Knuth's method for small means, normal approximation (rounded) above 60.
        let mut uniforms = core::iter::from_fn(|| {
            self.state ^= self.state << 13;
            self.state ^= self.state >> 7;
            self.state ^= self.state << 17;
            // 52 random mantissa bits under the exponent of 1.0 give a uniform in [1, 2).
            Some(f64::from_bits(0x3FF0_0000_0000_0000 | (self.state >> 12)) - 1.0)
        });
        if mean > 60.0 {
            let u1 = uniforms.next().unwrap_or(0.5).max(1e-300);
            let u2 = uniforms.next().unwrap_or(0.5);
            let normal =
                F64Math::sqrt(-2.0 * F64Math::ln(u1)) * (core::f64::consts::TAU * u2).cos();
            #[expect(
                clippy::as_conversions,
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "rounded, non-negative count far below u32::MAX"
            )]
            let count = F64Math::sqrt(mean).mul_add(normal, mean).round().max(0.0) as u32;
            return count;
        }
        let limit = F64Math::exp(-mean);
        let mut count = 0_u32;
        let mut product = uniforms.next().unwrap_or(0.5);
        loop {
            if product <= limit {
                return count;
            }
            count = count.saturating_add(1);
            product *= uniforms.next().unwrap_or(0.5);
        }
    }
}

impl Device for SimulatedCounter {
    fn send(&mut self, command: DeviceCommand, now_ms: u64) {
        self.job = match command {
            DeviceCommand::StartGates { gate_ms, repeat } => Some((now_ms, gate_ms, repeat)),
            DeviceCommand::StopGates => None,
        };
    }

    /// Gates have fixed deadlines, so a stalled UI catches up with correctly sized gates.
    fn poll(&mut self, now_ms: u64) -> Option<DeviceEvent> {
        let (started_ms, gate_ms, repeat) = self.job?;
        let at_ms = started_ms.saturating_add(u64::from(gate_ms));
        if now_ms < at_ms {
            return None;
        }
        self.job = repeat.then_some((at_ms, gate_ms, repeat));
        let gate_seconds = f64::from(gate_ms) / 1000.0;
        let since_start_ms = started_ms.saturating_sub(self.source_start_ms);
        let t_mid_s =
            core::time::Duration::from_millis(since_start_ms).as_secs_f64() + gate_seconds / 2.0;
        let count = self.gate_counts(gate_seconds, t_mid_s);
        Some(DeviceEvent::Gate {
            started_ms,
            at_ms,
            gate_ms,
            count,
        })
    }
}
