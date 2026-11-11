//! Detector probes (presets and custom tubes) and the bias-voltage selection.

/// Provenance of the dead-time value stored in a [`DetectorProfile`].
///
/// A datasheet minimum is a lower bound, a typical value describes a population of tubes,
/// and a measured value is specific to the calibrated probe.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeadTimeBasis {
    /// The datasheet minimum; the true dead time is greater than or equal to this value.
    DatasheetMinimum,
    /// A typical or nominal value from the tube operating instructions.
    Typical,
    /// A value measured or calibrated for this specific probe in the laboratory.
    Measured,
}

impl DeadTimeBasis {
    /// Short display label for a status panel.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::DatasheetMinimum => "DATASHEET MIN",
            Self::Typical => "TYPICAL",
            Self::Measured => "MEASURED",
        }
    }
}

/// Operating limits and dead time of a radiation detector probe.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DetectorProfile {
    /// Human-readable probe model name (e.g. `PHYWE 09005.00`).
    pub name: &'static str,
    /// Dead time used for rate correction in seconds.
    pub dead_time_sec: f32,
    /// Provenance of [`Self::dead_time_sec`].
    pub dead_time_basis: DeadTimeBasis,
    /// Lower bound of the detector-safe bias plateau in Volts.
    pub min_voltage_v: u16,
    /// Recommended centre-of-plateau bias voltage in Volts.
    pub nominal_voltage_v: u16,
    /// Upper bound of the detector-safe bias plateau in Volts.
    pub max_voltage_v: u16,
}

impl DetectorProfile {
    /// PHYWE 09005.00 mica-window Geiger-Müller counter tube.
    ///
    /// Dead time about 100 µs; plateau 425–650 V, 500 V recommended.
    /// Source: PHYWE 09005-00 operating instructions (`0900500e.pdf`).
    pub const PHYWE_09005_00: Self = Self {
        name: "PHYWE 09005.00",
        dead_time_sec: 0.000_100_f32,
        dead_time_basis: DeadTimeBasis::Typical,
        min_voltage_v: 425_u16,
        nominal_voltage_v: 500_u16,
        max_voltage_v: 650_u16,
    };

    /// Large-area pancake Geiger-Müller probe (LND 7311).
    ///
    /// Dead time 20 µs datasheet minimum (the real value depends on the quenching circuit);
    /// plateau 850–1000 V, 900 V recommended.
    /// Source: LND 7311 datasheet (`https://www.lndinc.com/products/geiger-mueller-tubes/7311/`).
    pub const PANCAKE_LND7311: Self = Self {
        name: "LND 7311 Pancake",
        dead_time_sec: 0.000_020_f32,
        dead_time_basis: DeadTimeBasis::DatasheetMinimum,
        min_voltage_v: 850_u16,
        nominal_voltage_v: 900_u16,
        max_voltage_v: 1000_u16,
    };

    /// The detectors offered for selection.
    pub const PRESETS: [Self; 2] = [Self::PHYWE_09005_00, Self::PANCAKE_LND7311];

    /// Name of every profile made with [`Self::custom`].
    pub const CUSTOM_NAME: &'static str = "Custom tube";

    /// Any other tube, with a dead time measured in the lab and a bias of 0–3000 V.
    #[must_use]
    pub const fn custom(dead_time_sec: f32) -> Self {
        Self {
            name: Self::CUSTOM_NAME,
            dead_time_sec,
            dead_time_basis: DeadTimeBasis::Measured,
            min_voltage_v: 0_u16,
            nominal_voltage_v: 500_u16,
            max_voltage_v: 3000_u16,
        }
    }
}

/// Selected detector and the tube bias noted for it.
///
/// The fields are private so that the bias can never leave the detector's plateau.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HardwareConfig {
    profile: DetectorProfile,
    bias_voltage_v: u16,
}

impl HardwareConfig {
    /// Configuration for `profile` at its nominal bias voltage.
    #[must_use]
    pub const fn new(profile: DetectorProfile) -> Self {
        Self {
            profile,
            bias_voltage_v: profile.nominal_voltage_v,
        }
    }

    /// The selected detector.
    #[must_use]
    pub const fn profile(&self) -> DetectorProfile {
        self.profile
    }

    /// The bias voltage, always inside the detector's plateau.
    #[must_use]
    pub const fn bias_voltage_v(&self) -> u16 {
        self.bias_voltage_v
    }

    /// Selects a detector and restores its nominal bias voltage.
    pub const fn set_profile(&mut self, profile: DetectorProfile) {
        *self = Self::new(profile);
    }

    /// Sets the bias voltage, clamped to the detector's plateau.
    pub fn set_bias_voltage(&mut self, voltage_v: u16) {
        self.bias_voltage_v =
            voltage_v.clamp(self.profile.min_voltage_v, self.profile.max_voltage_v);
    }
}
