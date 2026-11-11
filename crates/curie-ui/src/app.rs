//! Application state, measurement control, saved runs and analysis caching.

use alloc::collections::{BTreeMap, BTreeSet};
use core::time::Duration;
use std::path::{Path, PathBuf};
use std::thread::JoinHandle;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use curie_core::{
    BinWidthMode, CurieSession, DaqState, DataRecord, Dataset, DatasetRegistry, DatasetType,
    DecayBackground, DecaySample, DetectorProfile, Device, DeviceCommand, DeviceEvent,
    ExponentialDecayFit, GaussianFit, HardwareConfig, IntegerHistogram, PoissonFit, SampleMoments,
    StoredPoint, SweepAxis, SweepRecord, SweepSession,
};
use eframe::egui::{Align, Button, Frame, Layout, Margin, RichText, Stroke, Ui};
use eframe::{App, CreationContext, Frame as EframeFrame};

use crate::device::DeviceKind;
use crate::export::{
    GATE_COLUMNS, RunLog, SAVED_RUN_PREFIX, SWEEP_COLUMNS, csv_path, export_dataset_csv_to,
    export_sweep_csv_to, gate_row, load_saved_run, sweep_row, utc_timestamp,
};
use crate::modals::{ExportModal, ExportTarget, SaveModal, render_export_modal, render_save_modal};
use crate::pages::{Page, acquire, analysis, settings, sweep};
use crate::serial::{SerialConfig, SerialConnection, SerialProtocol, default_port};
use crate::simulation::SimulatedCounter;
use crate::theme::{
    ACCENT_AMBER, ACCENT_LILAC, PANEL_BG, PANEL_BORDER, STATE_DONE, STATE_IDLE, STATE_PAUSED,
    STATE_RUNNING, TAB_ACTIVE_BG, TAB_INACTIVE_BG, TEXT_MUTED, configure_theme, status_badge,
};

/// Pulses from a streaming device are assigned to gates by timestamp; gates are closed only
/// this long after their deadline so pulses still in flight are not missed.
const STREAM_GRACE_MS: u64 = 150;
/// Sub-folder of the autosave folder holding the runs saved with SAVE RUN.
const SAVED_RUNS_FOLDER: &str = "saved_runs";

/// Which measurement currently owns the counting gates (only one can run at a time).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GateOwner {
    #[default]
    None,
    Acquire,
    Sweep,
}

/// Plot shown in 1:ACQUIRE.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcquireViewMode {
    /// Rate of every gate against time.
    Temporal,
    /// Histogram of the counts per gate.
    Distribution,
}

/// Fit shown in 3:ANALYSIS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FitModel {
    /// Decay with background, fitted to the gate series.
    Exponential,
    /// Gaussian fit of the counts per gate.
    Gaussian,
    /// Poisson fit of the counts per gate.
    Poisson,
}

/// Background handling selected in the Analysis tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BackgroundChoice {
    #[default]
    Fit,
    Fixed,
}

/// Cached distribution fits for one dataset and one set of histogram parameters.
pub struct AnalysisResult {
    /// Inputs these results were computed from.
    pub key: String,
    pub histogram: Option<IntegerHistogram>,
    pub gaussian: Option<GaussianFit>,
    pub poisson: Option<PoissonFit>,
}

/// Decay fit running on a worker thread for one set of inputs.
pub struct DecayJob {
    /// Inputs of the fit being computed.
    pub key: String,
    /// Gates inside the fit window.
    pub points: usize,
    /// Worker computing the fit.
    pub handle: JoinHandle<Option<ExponentialDecayFit>>,
}

/// Metadata of a run saved with SAVE RUN.
pub struct SavedRun {
    /// Header lines of the measurement, written at the top of its CSV files.
    pub header: Vec<String>,
    /// Copy in the saved-runs folder; deleted together with the run.
    pub file: Option<PathBuf>,
}

#[expect(
    clippy::struct_excessive_bools,
    reason = "independent UI toggles and link flags, not an encoded state machine"
)]
pub struct CurieApp {
    pub page: Page,
    pub session: CurieSession,
    pub sweep: SweepSession,
    pub sweep_labels: Vec<String>,
    pub state_names_open: bool,
    pub registry: DatasetRegistry,
    /// Header and file of every saved dataset, by dataset id.
    pub saved_runs: BTreeMap<u32, SavedRun>,
    pub hardware: HardwareConfig,
    pub device_kind: DeviceKind,
    pub simulator: SimulatedCounter,
    pub serial_config: SerialConfig,
    pub serial: Option<SerialConnection>,
    /// Link state shown in 4:CONFIG (opening, ready, or why it failed).
    pub device_status: String,
    /// The device answered its initialisation and can count.
    pub device_ready: bool,
    pub gate_owner: GateOwner,
    pub active_dataset: Dataset,
    pub active_header: Vec<String>,
    pub live_moments: SampleMoments,
    pub live_epoch: u64,
    pub run_log: Option<RunLog>,
    pub sweep_log: Option<RunLog>,
    pub autosave_dir: String,
    pub view_mode: AcquireViewMode,

    pub sample_interval_s: f64,
    pub target_samples: u32,
    pub histogram_bins: u32,
    pub bin_width_mode: BinWidthMode,
    pub fixed_bin_width: u32,
    pub temporal_plot_points: bool,
    pub acquire_log_scale: bool,

    pub fit_model: FitModel,
    pub analysis_dataset_is_live: bool,
    pub decay_skip_points: usize,
    pub decay_start_time_sec: f64,
    pub decay_background: BackgroundChoice,
    pub decay_background_cps: f64,
    pub decay_log_scale: bool,
    pub analysis_cache: Option<AnalysisResult>,
    pub live_cache: Option<AnalysisResult>,
    /// Latest finished decay fit (kept on screen while a newer one is computed).
    pub decay_fit: Option<ExponentialDecayFit>,
    /// Inputs `decay_fit` was computed from.
    pub decay_fit_key: String,
    /// Gates inside the fit window of `decay_fit`.
    pub decay_points_used: usize,
    /// Decay fit in progress, if any (at most one at a time).
    pub decay_job: Option<DecayJob>,

    pub export_notice: Option<String>,
    pub save_modal: SaveModal,
    pub export_modal: ExportModal,
    pub storage_selection: BTreeSet<u32>,
    pub clock: Instant,
}

impl CurieApp {
    pub fn new(creation: &CreationContext<'_>) -> Self {
        configure_theme(&creation.egui_ctx);

        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".to_owned());
        let autosave_dir = Path::new(&home).join("curie_runs");
        let export_modal = ExportModal {
            directory: autosave_dir.join("exports").display().to_string(),
            ..ExportModal::default()
        };

        Self {
            page: Page::Acquire,
            session: CurieSession::new(10_000),
            sweep: SweepSession::new(20_000),
            // States start unnamed; the operator names them in STATE NAMES.
            sweep_labels: vec![String::new(); 2],
            registry: DatasetRegistry::default(),
            saved_runs: BTreeMap::new(),
            hardware: HardwareConfig::new(DetectorProfile::PHYWE_09005_00),
            device_kind: DeviceKind::Serial(SerialProtocol::PhyweCobra),
            simulator: SimulatedCounter::new(0x5EED_CAFE_F00D_0001),
            serial_config: SerialConfig {
                port: default_port(),
                baud: 9600,
                init_cmd: String::new(),
            },
            serial: None,
            device_status: "Not connected".to_owned(),
            device_ready: false,
            gate_owner: GateOwner::None,
            active_dataset: Dataset::new("live_run", DatasetType::AcquireTemporal, 0),
            active_header: Vec::new(),
            live_moments: SampleMoments::default(),
            live_epoch: 0,
            run_log: None,
            sweep_log: None,
            autosave_dir: autosave_dir.display().to_string(),
            view_mode: AcquireViewMode::Temporal,
            sample_interval_s: 10.0,
            target_samples: 0,
            histogram_bins: 30,
            bin_width_mode: BinWidthMode::Auto,
            fixed_bin_width: 1,
            temporal_plot_points: true,
            acquire_log_scale: false,
            fit_model: FitModel::Exponential,
            analysis_dataset_is_live: true,
            decay_skip_points: 0,
            decay_start_time_sec: 0.0,
            decay_background: BackgroundChoice::Fit,
            decay_background_cps: 0.0,
            decay_log_scale: false,
            analysis_cache: None,
            live_cache: None,
            decay_fit: None,
            decay_fit_key: String::new(),
            decay_points_used: 0,
            decay_job: None,
            export_notice: None,
            save_modal: SaveModal::default(),
            export_modal,
            storage_selection: BTreeSet::new(),
            state_names_open: false,
            clock: Instant::now(),
        }
    }

    /// Header lines describing the current setup, written at the top of every CSV.
    pub fn metadata_header(&self, title: &str, gate_ms: u32) -> Vec<String> {
        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs());
        let timing = if self.device_kind.times_gates() {
            "gates timed by the device"
        } else {
            "gates timed by the PC"
        };
        let source = match self.device_kind {
            DeviceKind::Simulator => format!("simulator, synthetic Poisson data ({timing})"),
            DeviceKind::Serial(_) => format!(
                "{} on {} @ {} baud ({timing})",
                self.device_kind.label(),
                self.serial_config.port,
                self.serial_config.baud,
            ),
        };
        let profile = self.hardware.profile();
        vec![
            format!("Curie-DAQ {title}"),
            format!("started_utc: {}", utc_timestamp(now_secs).0),
            format!("source: {source}"),
            format!("gate_s: {}", f64::from(gate_ms) / 1000.0),
            format!(
                "detector: {} | dead_time_us: {:.1} | tube_voltage_V (operator note): {}",
                profile.name,
                profile.dead_time_sec * 1.0e6,
                self.hardware.bias_voltage_v()
            ),
            if title == "sweep" {
                let axis = if self.sweep.axis() == SweepAxis::Angle {
                    "angle (deg)"
                } else {
                    "distance (mm)"
                };
                let states: Vec<String> = self
                    .sweep_labels
                    .iter()
                    .zip(1_usize..)
                    .map(|(name, number)| {
                        if name.is_empty() {
                            format!("S{number}")
                        } else {
                            format!("S{number} = {name}")
                        }
                    })
                    .collect();
                format!(
                    "sweep axis: {axis}; states: {}; rate_sigma = sqrt(max(counts,1))/gate_s",
                    states.join(" | ")
                )
            } else {
                "t_mid_s = gate centre since start; rate = counts/gate_s; rate_sigma = sqrt(max(counts,1))/gate_s".to_owned()
            },
        ]
    }

    // ------------------------------------------------------------------
    // Acquisition (gate series)
    // ------------------------------------------------------------------

    pub fn start_acquisition(&mut self) {
        if self.sweep.is_measuring() {
            self.export_notice = Some("Finish or cancel the sweep step first.".to_owned());
            return;
        }
        if !self.device_ready {
            self.export_notice = Some("The device is not ready (see 4:CONFIG).".to_owned());
            return;
        }
        #[expect(
            clippy::as_conversions,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "rounded and clamped to 100 ms ... 4294 s, the Cobra gate range"
        )]
        let gate_ms = (self.sample_interval_s * 1000.0)
            .round()
            .clamp(100.0, 4_294_000.0) as u32;
        let now = u64::try_from(self.clock.elapsed().as_millis()).unwrap_or(u64::MAX);
        if matches!(self.session.state, DaqState::Idle | DaqState::Completed) {
            self.session.sample_interval_ms = gate_ms;
            self.session.target_samples = self.target_samples;
            let now_secs = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_secs());
            let mut dataset = Dataset::new("live_run", DatasetType::AcquireTemporal, now_secs);
            dataset.gate_ms = gate_ms;
            self.active_dataset = dataset;
            self.active_header = self.metadata_header("gate series", gate_ms);
            self.live_moments = SampleMoments::default();
            self.live_epoch = self.live_epoch.wrapping_add(1);
            self.simulator.source_start_ms = now;
            let stem = format!("run_{}", utc_timestamp(now_secs).1);
            match RunLog::create(&self.autosave_dir, &stem, &self.active_header, GATE_COLUMNS) {
                Ok(log) => {
                    self.export_notice = None;
                    self.run_log = Some(log);
                }
                Err(error) => {
                    self.export_notice = Some(format!("AUTOSAVE OFF: {error}"));
                    self.run_log = None;
                }
            }
        }
        self.session.start(now);
        self.gate_owner = GateOwner::Acquire;
        self.simulator.bench = None;
        let run_gate_ms = self.session.sample_interval_ms;
        if self.device_kind.times_gates()
            && let Some(device) = self.device()
        {
            device.send(
                DeviceCommand::StartGates {
                    gate_ms: run_gate_ms,
                    repeat: true,
                },
                now,
            );
        }
    }

    pub fn pause_acquisition(&mut self) {
        self.session.pause();
        self.release_gates(GateOwner::Acquire);
    }

    pub fn reset_acquisition(&mut self) {
        self.session.reset();
        self.release_gates(GateOwner::Acquire);
        self.run_log = None;
        self.live_moments = SampleMoments::default();
        self.live_epoch = self.live_epoch.wrapping_add(1);
        self.active_dataset = Dataset::new("live_run", DatasetType::AcquireTemporal, 0);
        self.export_notice = None;
    }

    /// Stops the device's gates if `owner` holds them.
    pub fn release_gates(&mut self, owner: GateOwner) {
        if self.gate_owner == owner {
            self.gate_owner = GateOwner::None;
            let now = u64::try_from(self.clock.elapsed().as_millis()).unwrap_or(u64::MAX);
            if self.device_kind.times_gates()
                && let Some(device) = self.device()
            {
                device.send(DeviceCommand::StopGates, now);
            }
        }
    }

    /// Stores a completed gate: dataset point, live statistics and autosave row.
    fn record_live_gate(&mut self, record: DataRecord, t_mid_s: f64) {
        let gate_s = f64::from(record.gate_ms) / 1000.0;
        let counts = record.interval_pulses;
        #[expect(
            clippy::as_conversions,
            clippy::cast_possible_truncation,
            reason = "stored points are f32 by design; the exact count is kept in raw_count"
        )]
        let point = StoredPoint::new(
            t_mid_s as f32,
            (f64::from(counts) / gate_s) as f32,
            (f64::from(counts.max(1)).sqrt() / gate_s) as f32,
            counts,
        );
        self.active_dataset.points.push(point);
        self.live_moments.push(f64::from(counts));
        let dead_time = f64::from(self.hardware.profile().dead_time_sec);
        if let Some(log) = &mut self.run_log
            && let Err(error) = log.append(&gate_row(log.rows, t_mid_s, gate_s, counts, dead_time))
        {
            self.export_notice = Some(error);
            self.run_log = None;
        }
        if self.session.state == DaqState::Completed {
            self.release_gates(GateOwner::Acquire);
            self.export_notice = Some("Target number of gates reached.".to_owned());
        }
    }

    // ------------------------------------------------------------------
    // Event pump: runs every frame, but timing never depends on the frame rate
    // ------------------------------------------------------------------

    /// The selected device, if it is connected.
    pub fn device(&mut self) -> Option<&mut dyn Device> {
        match self.device_kind {
            DeviceKind::Simulator => Some(&mut self.simulator),
            DeviceKind::Serial(_) => self
                .serial
                .as_mut()
                .map(|connection| -> &mut dyn Device { connection }),
        }
    }

    pub fn pump(&mut self) {
        let now = u64::try_from(self.clock.elapsed().as_millis()).unwrap_or(u64::MAX);
        let mut events = Vec::new();
        if let Some(device) = self.device() {
            events.extend(core::iter::from_fn(|| device.poll(now)));
        }
        for event in events {
            self.handle_device_event(event);
        }
        if !self.device_kind.times_gates() {
            self.close_stream_gates(now.saturating_sub(STREAM_GRACE_MS));
        }
    }

    /// Closes software gates (streaming devices) whose deadline is before `until_ms`.
    fn close_stream_gates(&mut self, until_ms: u64) {
        while let Some(record) = self.session.tick(until_ms) {
            let t_mid_s = Duration::from_millis(record.timestamp_ms).as_secs_f64()
                - f64::from(record.gate_ms) / 2000.0;
            self.record_live_gate(record, t_mid_s);
        }
        if let Some(record) = self.sweep.tick(until_ms) {
            self.on_sweep_record(record);
        }
    }

    pub fn handle_device_event(&mut self, event: DeviceEvent) {
        match event {
            DeviceEvent::Ready => {
                self.device_ready = true;
                self.device_status = format!("{} ready", self.device_kind.label());
            }
            DeviceEvent::Pulses { at_ms, count } => {
                self.close_stream_gates(at_ms);
                self.session.ingest_pulses(count);
                self.sweep.ingest_pulses(count);
            }
            DeviceEvent::Gate {
                started_ms,
                at_ms,
                gate_ms,
                count,
            } => match self.gate_owner {
                GateOwner::Acquire => {
                    // Gate centre on the run clock: start of the gate plus half its length.
                    let since_start = started_ms.saturating_sub(self.session.session_start_ms);
                    let t_mid_s = Duration::from_millis(since_start).as_secs_f64()
                        + f64::from(gate_ms) / 2000.0;
                    if let Some(record) = self.session.ingest_gate(at_ms, gate_ms, count) {
                        self.record_live_gate(record, t_mid_s);
                    }
                }
                GateOwner::Sweep => {
                    self.gate_owner = GateOwner::None;
                    if let Some(record) = self.sweep.finish_step(count, gate_ms) {
                        self.on_sweep_record(record);
                    }
                }
                GateOwner::None => {}
            },
            DeviceEvent::Warning(message) => self.export_notice = Some(message),
            DeviceEvent::Failed(message) => {
                self.device_status = format!("DISCONNECTED: {message}");
                self.export_notice = Some(self.device_status.clone());
                self.device_ready = false;
                self.serial = None;
                self.session.pause();
                self.sweep.cancel_measurement();
                self.gate_owner = GateOwner::None;
            }
        }
    }

    // ------------------------------------------------------------------
    // Device selection and serial link
    // ------------------------------------------------------------------

    /// Switches to another device, closing the current link (the simulator is always ready).
    pub fn select_device(&mut self, kind: DeviceKind) {
        self.disconnect_serial();
        self.device_kind = kind;
        if kind == DeviceKind::Simulator {
            self.device_ready = true;
            "Simulator ready".clone_into(&mut self.device_status);
        }
    }

    pub fn connect_serial(&mut self) {
        self.serial = None;
        self.device_ready = false;
        let DeviceKind::Serial(protocol) = self.device_kind else {
            return;
        };
        match SerialConnection::open(&self.serial_config, protocol, self.clock) {
            Ok(connection) => {
                self.device_status = format!(
                    "Opened {}; waiting for the {}...",
                    self.serial_config.port,
                    self.device_kind.label()
                );
                self.serial = Some(connection);
            }
            Err(error) => {
                self.device_status.clone_from(&error);
                self.export_notice = Some(error);
            }
        }
    }

    pub fn disconnect_serial(&mut self) {
        self.pause_acquisition();
        self.release_gates(GateOwner::Sweep);
        self.sweep.cancel_measurement();
        self.serial = None;
        self.device_ready = false;
        "Not connected".clone_into(&mut self.device_status);
    }

    // ------------------------------------------------------------------
    // Sweep
    // ------------------------------------------------------------------

    pub fn start_sweep_step(&mut self) {
        if self.session.state == DaqState::Acquiring {
            self.export_notice = Some("Pause the gate series in 1:ACQUIRE first.".to_owned());
            return;
        }
        if !self.device_ready {
            self.export_notice = Some("The device is not ready (see 4:CONFIG).".to_owned());
            return;
        }
        if self.sweep_log.is_none() {
            let now_secs = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_secs());
            let header = self.metadata_header("sweep", self.sweep.step_duration_ms());
            let stem = format!("sweep_{}", utc_timestamp(now_secs).1);
            match RunLog::create(&self.autosave_dir, &stem, &header, SWEEP_COLUMNS) {
                Ok(log) => self.sweep_log = Some(log),
                Err(error) => self.export_notice = Some(format!("AUTOSAVE OFF: {error}")),
            }
        }
        let now = u64::try_from(self.clock.elapsed().as_millis()).unwrap_or(u64::MAX);
        self.sweep.start_step(now);
        self.simulator.bench = Some((
            self.sweep.target_distance_mm(),
            self.sweep.target_angle_deg(),
            self.sweep.state_index(),
        ));
        if self.device_kind.times_gates() {
            self.gate_owner = GateOwner::Sweep;
            let gate_ms = self.sweep.step_duration_ms();
            if let Some(device) = self.device() {
                device.send(
                    DeviceCommand::StartGates {
                        gate_ms,
                        repeat: false,
                    },
                    now,
                );
            }
        }
    }

    fn on_sweep_record(&mut self, record: SweepRecord) {
        let label = self
            .sweep_labels
            .get(usize::from(record.state_index))
            .cloned()
            .unwrap_or_default();
        let dead_time = f64::from(self.hardware.profile().dead_time_sec);
        if let Some(log) = &mut self.sweep_log
            && let Err(error) =
                log.append(&sweep_row(&record, self.sweep.axis(), &label, dead_time))
        {
            self.export_notice = Some(error);
            self.sweep_log = None;
            return;
        }
        let position = if self.sweep.axis() == SweepAxis::Angle {
            format!("{} deg", record.angle_deg)
        } else {
            format!("{} mm", record.distance_mm)
        };
        let number = u16::from(record.state_index).saturating_add(1);
        let state = if label.is_empty() {
            format!("S{number}")
        } else {
            format!("S{number} {label}")
        };
        self.export_notice = Some(format!(
            "Recorded {position} [{state}]: {} counts ({:.2} counts/s)",
            record.pulses, record.cps
        ));
    }

    pub fn repeat_sweep_step(&mut self) {
        if !self.sweep.is_measuring() && self.sweep.points().is_empty() {
            return;
        }
        self.release_gates(GateOwner::Sweep);
        if self.sweep.is_measuring() {
            self.sweep.cancel_measurement();
        } else {
            let _ = self.sweep.pop_last_point();
            self.export_notice = Some("Last point discarded; measuring it again.".to_owned());
        }
        self.start_sweep_step();
    }

    pub fn undo_sweep_point(&mut self) {
        self.release_gates(GateOwner::Sweep);
        if self.sweep.is_measuring() {
            self.sweep.cancel_measurement();
            self.export_notice = Some("Measurement in progress cancelled.".to_owned());
        } else if self.sweep.pop_last_point().is_some() {
            self.export_notice =
                Some("Removed the last point (it stays in the autosave file).".to_owned());
        }
    }

    pub fn reset_sweep(&mut self) {
        self.release_gates(GateOwner::Sweep);
        let mut fresh = SweepSession::new(self.sweep.step_duration_ms());
        fresh.set_state_count(self.sweep.state_count());
        fresh.set_axis(self.sweep.axis());
        fresh.set_step_size_mm(self.sweep.step_size_mm());
        self.sweep = fresh;
        self.sweep_log = None;
    }

    pub fn set_sweep_state_count(&mut self, count: usize) {
        self.sweep.set_state_count(count);
        self.sweep_labels
            .resize(self.sweep.state_count(), String::new());
    }

    // ------------------------------------------------------------------
    // Storage
    // ------------------------------------------------------------------

    pub fn open_save_modal(&mut self, is_sweep: bool) {
        self.save_modal.is_open = true;
        self.save_modal.is_sweep = is_sweep;
        let next_idx = self.registry.datasets().len().saturating_add(1);
        self.save_modal.dataset_name = if is_sweep {
            format!("sweep_run_{next_idx}")
        } else {
            format!("run_{next_idx}")
        };
    }

    /// Adds the live run (or the sweep) to the saved runs and writes its copy to the
    /// saved-runs folder, so it is listed again the next time the app starts.
    pub fn save_named_run_to_registry(&mut self, name: &str, is_sweep: bool) {
        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs());
        let (mut dataset, header) = if is_sweep {
            let mut dataset = Dataset::new(name, DatasetType::SweepMatrix, now_secs);
            dataset.gate_ms = self.sweep.step_duration_ms();
            dataset.points = self
                .sweep
                .points()
                .iter()
                .map(|record| {
                    let position = if self.sweep.axis() == SweepAxis::Angle {
                        f32::from(record.angle_deg)
                    } else {
                        f32::from(record.distance_mm)
                    };
                    StoredPoint::new(
                        position,
                        record.cps,
                        f32::from(record.state_index),
                        record.pulses,
                    )
                })
                .collect();
            (
                dataset,
                self.metadata_header("sweep", self.sweep.step_duration_ms()),
            )
        } else {
            (self.active_dataset.clone(), self.active_header.clone())
        };
        if dataset.points.is_empty() {
            self.export_notice = Some("Nothing recorded yet.".to_owned());
            return;
        }
        name.clone_into(&mut dataset.name);
        dataset.timestamp_secs = now_secs;

        let id = self.registry.add(dataset);
        // `<UTC time>_<id>_<name>.csv`: listing the folder in name order is chronological.
        let stem = format!("{}_{id:05}_{name}", utc_timestamp(now_secs).1);
        let mut file_header = vec![format!("{SAVED_RUN_PREFIX}{name}")];
        file_header.extend(header.iter().cloned());
        let dead_time = f64::from(self.hardware.profile().dead_time_sec);
        let folder = Path::new(&self.autosave_dir).join(SAVED_RUNS_FOLDER);
        let written = self
            .registry
            .datasets()
            .last()
            .ok_or_else(|| "saved run missing".to_owned())
            .and_then(|saved| {
                csv_path(&folder, &stem, true)
                    .and_then(|path| export_dataset_csv_to(saved, &file_header, dead_time, &path))
            });
        self.export_notice = Some(match &written {
            Ok(path) => format!("Saved as dataset #{id} ('{name}') in {}", path.display()),
            Err(error) => format!("Saved #{id} for this session only: {error}"),
        });
        let _previous = self.saved_runs.insert(
            id,
            SavedRun {
                header,
                file: written.ok(),
            },
        );
    }

    /// Lists the runs in the saved-runs folder (oldest first), as left by earlier sessions.
    pub fn load_saved_runs(&mut self) {
        let folder = Path::new(&self.autosave_dir).join(SAVED_RUNS_FOLDER);
        let Ok(entries) = std::fs::read_dir(&folder) else {
            return;
        };
        let mut paths: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("csv"))
            })
            .collect();
        paths.sort();
        let mut unreadable = Vec::new();
        for path in paths {
            match load_saved_run(&path) {
                Ok((dataset, header)) => {
                    let id = self.registry.add(dataset);
                    let _previous = self.saved_runs.insert(
                        id,
                        SavedRun {
                            header,
                            file: Some(path),
                        },
                    );
                }
                Err(error) => unreadable.push(error),
            }
        }
        if let Some(first) = unreadable.first() {
            self.export_notice = Some(format!(
                "{} saved run(s) could not be read, e.g. {first}",
                unreadable.len()
            ));
        }
    }

    pub fn export_target_csv(&mut self, dir_path: &str, file_name: &str, target: ExportTarget) {
        let dead_time = f64::from(self.hardware.profile().dead_time_sec);
        let mut exported = Vec::new();
        let mut errors = Vec::new();
        let folder = Path::new(dir_path);
        let mut export_one =
            |dataset: &Dataset, header: &[String], name: &str| match csv_path(folder, name, false)
                .and_then(|path| export_dataset_csv_to(dataset, header, dead_time, &path))
            {
                Ok(path) => exported.push(path.display().to_string()),
                Err(error) => errors.push(error),
            };
        match target {
            ExportTarget::ActiveDataset => {
                export_one(&self.active_dataset, &self.active_header, file_name);
            }
            ExportTarget::Sweep => {
                let header = self.metadata_header("sweep", self.sweep.step_duration_ms());
                match csv_path(folder, file_name, false).and_then(|path| {
                    export_sweep_csv_to(&self.sweep, &self.sweep_labels, &header, dead_time, &path)
                }) {
                    Ok(path) => exported.push(path.display().to_string()),
                    Err(error) => errors.push(error),
                }
            }
            ExportTarget::RegistryDataset(index) => {
                if let Some(dataset) = self.registry.datasets().get(index) {
                    let header = self
                        .saved_runs
                        .get(&dataset.id)
                        .map_or(&[][..], |run| run.header.as_slice());
                    export_one(dataset, header, file_name);
                }
            }
            ExportTarget::MassAll | ExportTarget::Selected => {
                for dataset in self.registry.datasets() {
                    if target == ExportTarget::Selected
                        && !self.storage_selection.contains(&dataset.id)
                    {
                        continue;
                    }
                    let header = self
                        .saved_runs
                        .get(&dataset.id)
                        .map_or(&[][..], |run| run.header.as_slice());
                    export_one(
                        dataset,
                        header,
                        &format!("{}_{}.csv", dataset.id, dataset.name),
                    );
                }
            }
        }
        self.export_notice = Some(match (exported.len(), errors.first()) {
            (_, Some(error)) => format!("CSV export error: {error}"),
            (1, None) => format!("Exported to {}", exported.join("")),
            (count, None) => format!("Exported {count} files to '{dir_path}'"),
        });
    }

    /// Removes saved runs and their copies in the saved-runs folder. The autosave logs of
    /// the original measurements are never touched.
    pub fn delete_runs(&mut self, ids: &BTreeSet<u32>) {
        let mut removed = 0_usize;
        let mut failures = Vec::new();
        for index in (0..self.registry.datasets().len()).rev() {
            let Some(id) = self
                .registry
                .datasets()
                .get(index)
                .map(|dataset| dataset.id)
            else {
                continue;
            };
            if !ids.contains(&id) || !self.registry.remove_at(index) {
                continue;
            }
            removed = removed.saturating_add(1);
            let _was_selected = self.storage_selection.remove(&id);
            if let Some(file) = self.saved_runs.remove(&id).and_then(|run| run.file)
                && let Err(error) = std::fs::remove_file(&file)
            {
                failures.push(format!("{}: {error}", file.display()));
            }
        }
        self.export_notice = Some(failures.first().map_or_else(
            || format!("Deleted {removed} saved run(s); the autosave logs are kept."),
            |first| format!("Deleted {removed} run(s), but could not remove {first}"),
        ));
    }

    // ------------------------------------------------------------------
    // Analysis (cached: the fits are recomputed only when their inputs change)
    // ------------------------------------------------------------------

    /// Whether 3:ANALYSIS shows the live run: chosen explicitly, or because the registry
    /// selection is not a gate series (saving a sweep selects it).
    pub fn analysis_shows_live(&self) -> bool {
        self.analysis_dataset_is_live
            || self
                .registry
                .selected()
                .is_none_or(|dataset| dataset.kind != DatasetType::AcquireTemporal)
    }

    /// Fits for the dataset selected in the Analysis tab (or the live run on `live_view`).
    ///
    /// Distribution fits are cheap and computed here; the decay fit runs on a worker thread
    /// (one at a time, latest inputs win) and lands in `decay_fit` when it finishes.
    pub fn refresh_analysis(&mut self, live_view: bool) {
        let use_live = live_view || self.analysis_shows_live();
        let dataset = if use_live {
            &self.active_dataset
        } else {
            self.registry.selected().unwrap_or(&self.active_dataset)
        };
        let dead_time = f64::from(self.hardware.profile().dead_time_sec);
        let dataset_key = format!(
            "{use_live}|{}|{}|{}",
            self.live_epoch,
            dataset.id,
            dataset.points.len()
        );

        if !live_view && self.fit_model == FitModel::Exponential {
            if self
                .decay_job
                .as_ref()
                .is_some_and(|job| job.handle.is_finished())
                && let Some(job) = self.decay_job.take()
            {
                self.decay_fit = job.handle.join().unwrap_or_default();
                self.decay_points_used = job.points;
                self.decay_fit_key = job.key;
            }
            let decay_key = format!(
                "{dataset_key}|{}|{}|{:?}|{}|{dead_time}",
                self.decay_skip_points,
                self.decay_start_time_sec,
                self.decay_background,
                self.decay_background_cps
            );
            if self.decay_job.is_none() && self.decay_fit_key != decay_key {
                let gate_s = f64::from(dataset.gate_ms) / 1000.0;
                let samples: Vec<DecaySample> = dataset
                    .points
                    .iter()
                    .enumerate()
                    .filter(|(index, point)| {
                        *index >= self.decay_skip_points
                            && f64::from(point.x) >= self.decay_start_time_sec
                    })
                    .map(|(_, point)| DecaySample::new(f64::from(point.x), gate_s, point.raw_count))
                    .collect();
                let background = match self.decay_background {
                    BackgroundChoice::Fit => DecayBackground::Free,
                    BackgroundChoice::Fixed => DecayBackground::Fixed(self.decay_background_cps),
                };
                let points = if gate_s > 0.0 { samples.len() } else { 0 };
                self.decay_job = Some(DecayJob {
                    key: decay_key,
                    points,
                    handle: std::thread::spawn(move || {
                        (gate_s > 0.0)
                            .then(|| ExponentialDecayFit::fit(&samples, background, dead_time))
                            .flatten()
                    }),
                });
            }
            return;
        }

        let key = format!(
            "{dataset_key}|{:?}|{:?}|{}|{live_view}",
            self.fit_model, self.bin_width_mode, self.histogram_bins
        );
        let cache = if live_view {
            &self.live_cache
        } else {
            &self.analysis_cache
        };
        if cache.as_ref().is_some_and(|cached| cached.key == key) {
            return;
        }
        let counts: Vec<u32> = dataset.points.iter().map(|point| point.raw_count).collect();
        let histogram =
            IntegerHistogram::from_counts(&counts, self.bin_width_mode, self.histogram_bins);
        let (gaussian, poisson) = histogram.as_ref().map_or((None, None), |bins| {
            (
                GaussianFit::from_counts(&counts, bins),
                PoissonFit::from_counts(&counts, bins),
            )
        });
        let result = AnalysisResult {
            key,
            histogram,
            gaussian,
            poisson,
        };
        if live_view {
            self.live_cache = Some(result);
        } else {
            self.analysis_cache = Some(result);
        }
    }

    #[expect(
        unused_results,
        clippy::shadow_reuse,
        reason = "egui widgets return a Response that layout-only calls ignore, and egui closures rebind `ui` to the child Ui"
    )]
    fn ui_top_bar(&mut self, ui: &mut Ui) {
        let (badge_text, badge_color) = match self.session.state {
            _ if self.sweep.is_measuring() => ("RUNNING", STATE_RUNNING),
            DaqState::Acquiring => ("RUNNING", STATE_RUNNING),
            DaqState::Paused => ("PAUSED", STATE_PAUSED),
            DaqState::Completed => ("DONE", STATE_DONE),
            DaqState::Idle | _ => ("IDLE", STATE_IDLE),
        };
        let device = self.device_kind.label();
        let (source_text, source_color) = match self.device_kind {
            DeviceKind::Simulator => (device.to_owned(), TEXT_MUTED),
            DeviceKind::Serial(_) if self.device_ready => (
                format!("{device} @ {} ONLINE", self.serial_config.port),
                STATE_RUNNING,
            ),
            DeviceKind::Serial(_) => (format!("{device} OFFLINE"), ACCENT_AMBER),
        };
        // Autosave is always on; only a run whose CSV could not be written is worth a flag.
        let unsaved = (self.session.state != DaqState::Idle && self.run_log.is_none())
            || ((self.sweep.is_measuring() || !self.sweep.points().is_empty())
                && self.sweep_log.is_none());

        Frame::new()
            .fill(PANEL_BG)
            .stroke(Stroke::new(1.0, PANEL_BORDER))
            .inner_margin(Margin::symmetric(14, 8))
            .show(ui, |ui| {
                // Fixed row height so the logo, tabs and status are centred on one line.
                let row = Layout::left_to_right(Align::Center);
                ui.allocate_ui_with_layout(
                    eframe::egui::vec2(ui.available_width(), 30.0),
                    row,
                    |ui| {
                        ui.label(
                            RichText::new("CURIE-DAQ")
                                .monospace()
                                .size(16.5)
                                .strong()
                                .color(ACCENT_LILAC),
                        );
                        ui.add_space(16.0);
                        ui.separator();
                        ui.add_space(10.0);
                        for page in Page::ALL {
                            let is_active = self.page == page;
                            let (fill, accent) = if is_active {
                                (TAB_ACTIVE_BG, ACCENT_LILAC)
                            } else {
                                (TAB_INACTIVE_BG, PANEL_BORDER)
                            };
                            let text_color = if is_active { ACCENT_LILAC } else { TEXT_MUTED };
                            let label = RichText::new(page.label())
                                .monospace()
                                .size(13.5)
                                .strong()
                                .color(text_color);
                            let button = Button::new(label)
                                .fill(fill)
                                .stroke(Stroke::new(1.0, accent));
                            if ui.add_sized([115.0, 30.0], button).clicked() {
                                self.page = page;
                            }
                            ui.add_space(4.0);
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            status_badge(ui, badge_text, badge_color);
                            ui.add_space(12.0);
                            let source = RichText::new(source_text).monospace().size(12.0);
                            ui.label(source.color(source_color));
                            ui.add_space(10.0);
                            if unsaved {
                                let note = RichText::new("NOT AUTOSAVING").monospace().size(12.0);
                                ui.label(note.color(ACCENT_AMBER)).on_hover_text(
                                    "The CSV in the autosave folder could not be written; \
                                     check the folder in 4:CONFIG and export before closing.",
                                );
                            }
                        });
                    },
                );
            });
    }
}

impl App for CurieApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut EframeFrame) {
        self.pump();

        let ctx = ui.ctx().clone();
        if self.session.state == DaqState::Acquiring
            || self.sweep.is_measuring()
            || self.serial.is_some()
        {
            ctx.request_repaint_after(Duration::from_millis(50));
        }

        self.ui_top_bar(ui);
        ui.add_space(6.0);
        match self.page {
            Page::Acquire => acquire::render(self, ui),
            Page::Sweep => sweep::render(self, ui),
            Page::Analysis => analysis::render(self, ui),
            Page::Settings => settings::render(self, ui),
        }

        let mut save_action = None;
        let mut export_action = None;
        render_save_modal(&ctx, &mut self.save_modal, |name, is_sweep| {
            save_action = Some((name, is_sweep));
        });
        render_export_modal(&ctx, &mut self.export_modal, |dir, file, target| {
            export_action = Some((dir, file, target));
        });
        if let Some((name, is_sweep)) = save_action {
            self.save_named_run_to_registry(&name, is_sweep);
        }
        if let Some((dir, file, target)) = export_action {
            self.export_target_csv(&dir, &file, target);
        }
    }
}
