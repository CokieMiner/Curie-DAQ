//! Acquisition engine, saved runs and the decay-fit worker of `CurieApp`.

use core::time::Duration;
use std::path::PathBuf;
use std::time::Instant;

use curie_core::{DatasetType, DeviceEvent, StoredPoint};
use eframe::CreationContext;

use crate::app::{CurieApp, FitModel, GateOwner};
use crate::device::DeviceKind;
use crate::serial::SerialProtocol;

/// An app on `device`, treated as connected and ready, autosaving to a temporary folder.
fn test_app(device: DeviceKind, tag: &str) -> CurieApp {
    let ctx = eframe::egui::Context::default();
    let mut app = CurieApp::new(&CreationContext::_new_kittest(ctx));
    app.select_device(device);
    app.device_ready = true;
    app.autosave_dir = std::env::temp_dir()
        .join(format!("curie_test_{tag}_{}", std::process::id()))
        .display()
        .to_string();
    app
}

#[test]
fn stalled_ui_still_records_correct_gates_and_autosaves() {
    let mut app = test_app(DeviceKind::Simulator, "stall");
    app.sample_interval_s = 0.2;
    app.start_acquisition();
    // The window was hidden for 10 s: no frames at all.
    app.clock = app
        .clock
        .checked_sub(Duration::from_secs(10))
        .expect("clock rewind");
    app.pump();

    let points = app.active_dataset.points;
    assert!((49..=51).contains(&points.len()), "{} gates", points.len());
    assert_eq!(app.active_dataset.gate_ms, 200);
    let first = points.first().expect("gates recorded");
    assert!(
        (first.x - 0.1).abs() < 0.03,
        "first gate centre {}",
        first.x
    );
    let spacing = points.get(1).map_or(0.0, |p| p.x - first.x);
    assert!(
        (spacing - 0.2).abs() < 1e-3,
        "gates are back to back: {spacing}"
    );

    let log = app.run_log.as_ref().expect("autosave active");
    let text = std::fs::read_to_string(&log.path).expect("autosave readable");
    let rows = text.lines().filter(|line| !line.starts_with('#')).count();
    assert_eq!(rows, points.len() + 1, "header row + one row per gate");
    drop(std::fs::remove_dir_all(&app.autosave_dir));
}

#[test]
fn pause_keeps_the_time_axis_running() {
    let mut app = test_app(DeviceKind::Simulator, "pause");
    app.sample_interval_s = 1.0;
    app.start_acquisition();
    app.clock = app
        .clock
        .checked_sub(Duration::from_millis(3_500))
        .expect("clock rewind");
    app.pump();
    assert_eq!(app.active_dataset.points.len(), 3);
    app.pause_acquisition();
    app.clock = app
        .clock
        .checked_sub(Duration::from_secs(20))
        .expect("clock rewind");
    app.pump();
    assert_eq!(
        app.active_dataset.points.len(),
        3,
        "nothing counted while paused"
    );
    app.start_acquisition();
    app.clock = app
        .clock
        .checked_sub(Duration::from_millis(1_100))
        .expect("clock rewind");
    app.pump();
    let last = app.active_dataset.points.last().expect("gate after resume");
    // Paused at 3.5 s, resumed at 23.5 s: the first new gate is centred at 24 s.
    assert!(
        (last.x - 24.0).abs() < 0.05,
        "time keeps running through the pause: {}",
        last.x
    );
    drop(std::fs::remove_dir_all(&app.autosave_dir));
}

#[test]
fn stream_pulses_land_in_the_gate_of_their_timestamp() {
    let mut app = test_app(DeviceKind::Serial(SerialProtocol::AsciiText), "stream");
    app.sample_interval_s = 0.1;
    app.start_acquisition();
    let start = app.session.session_start_ms;
    for (offset_ms, count) in [(50, 5), (150, 3), (160, 1), (250, 7)] {
        app.handle_device_event(DeviceEvent::Pulses {
            at_ms: start.saturating_add(offset_ms),
            count,
        });
    }
    // Delivered late (UI stalled), then closed past the grace period.
    app.clock = app
        .clock
        .checked_sub(Duration::from_secs(1))
        .expect("clock rewind");
    app.pump();
    let counts: Vec<u32> = app
        .active_dataset
        .points
        .iter()
        .take(3)
        .map(|p| p.raw_count)
        .collect();
    assert_eq!(counts, [5, 4, 7]);
    drop(std::fs::remove_dir_all(&app.autosave_dir));
}

#[test]
fn cobra_gate_is_recorded_at_its_centre() {
    let mut app = test_app(DeviceKind::Serial(SerialProtocol::PhyweCobra), "cobra");
    app.sample_interval_s = 10.0;
    app.start_acquisition();
    assert_eq!(app.gate_owner, GateOwner::Acquire);
    let start = app.session.session_start_ms;
    app.handle_device_event(DeviceEvent::Gate {
        started_ms: start.saturating_add(30),
        at_ms: start.saturating_add(10_090),
        gate_ms: 10_000,
        count: 123,
    });
    let point = app
        .active_dataset
        .points
        .first()
        .copied()
        .expect("gate recorded");
    assert_eq!(point.raw_count, 123);
    assert!(
        (point.x - 5.03).abs() < 1e-3,
        "centre = start + 5 s: {}",
        point.x
    );
    assert!((point.y - 12.3).abs() < 1e-4);
    drop(std::fs::remove_dir_all(&app.autosave_dir));
}

#[test]
fn saved_runs_survive_a_restart_and_are_deleted_with_their_file() {
    let mut app = test_app(DeviceKind::Simulator, "saved");
    app.sample_interval_s = 0.5;
    app.start_acquisition();
    app.clock = app
        .clock
        .checked_sub(Duration::from_secs(5))
        .expect("clock rewind");
    app.pump();
    app.save_named_run_to_registry("ra226, 10 s", false);
    app.sweep.start_step(0);
    let _first = app.sweep.finish_step(40, 20_000);
    app.sweep.start_step(0);
    let _second = app.sweep.finish_step(10, 20_000);
    app.save_named_run_to_registry("bench", true);
    assert_eq!(app.registry.datasets().len(), 2);

    let mut restarted = test_app(DeviceKind::Simulator, "x");
    restarted.autosave_dir.clone_from(&app.autosave_dir);
    restarted.load_saved_runs();
    let loaded = restarted.registry.datasets();
    assert_eq!(loaded.len(), 2, "{:?}", restarted.export_notice);
    let gates = loaded.first().expect("gate series reloaded");
    let sweep = loaded.get(1).expect("sweep reloaded");
    assert_eq!(gates.name, "ra226, 10 s");
    assert_eq!(gates.kind, DatasetType::AcquireTemporal);
    assert_eq!(gates.gate_ms, 500);
    let original: Vec<(u32, f32)> = app
        .registry
        .datasets()
        .first()
        .expect("run saved")
        .points
        .iter()
        .map(|point| (point.raw_count, point.x))
        .collect();
    let reloaded: Vec<(u32, f32)> = gates
        .points
        .iter()
        .map(|point| (point.raw_count, point.x))
        .collect();
    assert_eq!(original, reloaded, "counts and gate centres round-trip");
    assert_eq!(sweep.kind, DatasetType::SweepMatrix);
    assert_eq!(sweep.gate_ms, 20_000);
    let steps: Vec<(u32, f32)> = sweep
        .points
        .iter()
        .map(|point| (point.raw_count, point.aux))
        .collect();
    assert_eq!(steps, [(40, 0.0), (10, 1.0)]);
    let header = &restarted
        .saved_runs
        .get(&gates.id)
        .expect("header kept")
        .header;
    assert!(header.iter().any(|line| line.starts_with("gate_s: 0.5")));

    let files: Vec<PathBuf> = restarted
        .saved_runs
        .values()
        .filter_map(|run| run.file.clone())
        .collect();
    let all = restarted
        .registry
        .datasets()
        .iter()
        .map(|dataset| dataset.id)
        .collect();
    restarted.delete_runs(&all);
    assert_eq!(restarted.registry.datasets(), []);
    assert!(files.iter().all(|file| !file.exists()), "copies removed");
    drop(std::fs::remove_dir_all(&app.autosave_dir));
}

#[test]
fn decay_fit_runs_on_a_worker_thread() {
    let mut app = test_app(DeviceKind::Simulator, "decay");
    app.active_dataset.gate_ms = 5_000;
    let lambda = core::f64::consts::LN_2 / 70.2;
    app.active_dataset.points = (0..180_u32)
        .map(|index| {
            let t_mid = 5.0_f32.mul_add(f32::from(u16::try_from(index).unwrap_or(0)), 2.5);
            let expected = 5.0 * 40.0_f64.mul_add((-lambda * f64::from(t_mid)).exp(), 0.5);
            // Noiseless "counts": the expected value rounded to an integer.
            let counts = (0..u32::MAX)
                .find(|&count| f64::from(count) + 0.5 > expected)
                .unwrap_or(0);
            StoredPoint::new(t_mid, 0.0, 0.0, counts)
        })
        .collect();
    app.analysis_dataset_is_live = true;
    app.fit_model = FitModel::Exponential;
    app.refresh_analysis(false);
    assert!(
        app.decay_job.is_some(),
        "the fit is started in the background"
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    while app.decay_job.is_some() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
        app.refresh_analysis(false);
    }
    let fit = app.decay_fit.as_ref().expect("fit finished");
    assert!(
        (fit.half_life_s - 70.2).abs() < 1.0,
        "T1/2 = {}",
        fit.half_life_s
    );
    assert_eq!(app.decay_points_used, 180);

    app.refresh_analysis(false);
    assert!(app.decay_job.is_none(), "same inputs: no new fit");
    app.decay_skip_points = 10;
    app.refresh_analysis(false);
    assert!(app.decay_job.is_some(), "new inputs start a new fit");
    assert!(app.decay_fit.is_some(), "the previous result stays visible");
}

#[test]
fn simulated_sweep_step_is_timed_by_the_device() {
    let mut app = test_app(DeviceKind::Simulator, "sweep");
    app.sweep.set_step_duration_ms(2_000);
    app.sweep.set_target_distance_mm(40);
    app.start_sweep_step();
    assert_eq!(app.gate_owner, GateOwner::Sweep);
    app.pump();
    assert_eq!(app.sweep.points(), [], "the gate is still running");

    app.clock = app
        .clock
        .checked_sub(Duration::from_millis(2_100))
        .expect("clock rewind");
    app.pump();
    let record = app.sweep.points().first().copied().expect("step recorded");
    assert_eq!(record.distance_mm, 40);
    assert_eq!(record.duration_ms, 2_000);
    assert_eq!(app.gate_owner, GateOwner::None, "the gates are released");
    assert!(!app.sweep.is_measuring());
    drop(std::fs::remove_dir_all(&app.autosave_dir));
}
