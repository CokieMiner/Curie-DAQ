//! Datasets of a run or a sweep, and the registry of saved datasets.

use alloc::{string::String, vec::Vec};

/// Kind of experiment stored in a dataset.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DatasetType {
    /// Series of counting gates (`x` = gate centre in s, `y` = rate, `raw_count` = counts).
    #[default]
    AcquireTemporal,
    /// Manual position-by-state sweep (`x` = distance in mm or angle in degrees).
    SweepMatrix,
}

/// One stored point of a dataset.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct StoredPoint {
    /// Independent variable (gate centre in s, distance in mm or angle in degrees).
    pub x: f32,
    /// Dependent variable (count rate in counts/s).
    pub y: f32,
    /// Auxiliary value (rate uncertainty for gates, state index for sweeps).
    pub aux: f32,
    /// Exact pulse count, kept so histograms and fits never work on rounded rates.
    pub raw_count: u32,
}

impl StoredPoint {
    /// Creates a point.
    #[must_use]
    pub const fn new(x: f32, y: f32, aux: f32, raw_count: u32) -> Self {
        Self {
            x,
            y,
            aux,
            raw_count,
        }
    }
}

/// A run or a sweep with its points.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Dataset {
    /// Identifier assigned by the [`DatasetRegistry`] (zero until registered).
    pub id: u32,
    /// Human-readable name.
    pub name: String,
    /// Creation time in seconds since the Unix epoch.
    pub timestamp_secs: u64,
    /// Kind of experiment represented by this dataset.
    pub kind: DatasetType,
    /// Counting gate of every point in milliseconds (zero if not a gated series).
    pub gate_ms: u32,
    /// Stored points in acquisition order.
    pub points: Vec<StoredPoint>,
}

impl Dataset {
    /// Creates an empty, unregistered dataset.
    #[must_use]
    pub fn new(name: &str, kind: DatasetType, timestamp_secs: u64) -> Self {
        Self {
            name: String::from(name),
            timestamp_secs,
            kind,
            ..Self::default()
        }
    }
}

/// Saved datasets with unique identifiers and one selected entry.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DatasetRegistry {
    datasets: Vec<Dataset>,
    selected_index: usize,
    next_id: u32,
}

impl DatasetRegistry {
    /// The saved datasets in insertion order.
    #[must_use]
    pub fn datasets(&self) -> &[Dataset] {
        &self.datasets
    }

    /// Index of the selected dataset.
    #[must_use]
    pub const fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// The selected dataset, if any.
    #[must_use]
    pub fn selected(&self) -> Option<&Dataset> {
        self.datasets.get(self.selected_index)
    }

    /// Selects a dataset by index; out-of-range indices are ignored.
    pub const fn select_index(&mut self, index: usize) {
        if index < self.datasets.len() {
            self.selected_index = index;
        }
    }

    /// Adds a dataset, assigns it the next identifier (starting at 1) and selects it.
    pub fn add(&mut self, mut dataset: Dataset) -> u32 {
        self.next_id = self.next_id.saturating_add(1_u32);
        dataset.id = self.next_id;
        self.datasets.push(dataset);
        self.selected_index = self.datasets.len().saturating_sub(1_usize);
        self.next_id
    }

    /// Removes the dataset at `index`, keeping the selection on the same dataset when possible.
    pub fn remove_at(&mut self, index: usize) -> bool {
        if index >= self.datasets.len() {
            return false;
        }
        drop(self.datasets.remove(index));
        if index < self.selected_index || self.selected_index >= self.datasets.len() {
            self.selected_index = self.selected_index.saturating_sub(1_usize);
        }
        true
    }
}
