//! Integration tests for the manual sweep engine.

use curie_core::{SweepAxis, SweepGroup, SweepRecord, SweepSession};

const STEP_MS: u32 = 1_000_u32;

fn cps(group: &SweepGroup, state_index: usize) -> Option<f32> {
    group
        .records
        .get(state_index)
        .copied()
        .flatten()
        .map(|record| record.cps)
}

fn measure(sweep: &mut SweepSession, start_ms: u64, pulses: u32) -> SweepRecord {
    sweep.start_step(start_ms);
    sweep.ingest_pulses(pulses);
    sweep
        .tick(start_ms.saturating_add(u64::from(STEP_MS)))
        .expect("step completes")
}

#[test]
fn test_timed_measurements_advance_to_the_next_state() {
    let mut sweep = SweepSession::new(STEP_MS);
    assert_eq!(sweep.state_count(), 2_usize);
    assert_eq!(sweep.state_index(), 0_usize);

    sweep.set_target_distance_mm(25_u16);
    sweep.set_step_size_mm(10_u16);

    // Pulses arriving before START are ignored.
    sweep.ingest_pulses(999_u32);
    sweep.start_step(0_u64);
    assert!(sweep.is_measuring());

    sweep.ingest_pulses(1_000_u32);
    assert_eq!(sweep.tick(500_u64), None);

    let bare = sweep.tick(1_000_u64).expect("bare step completes");
    assert_eq!(bare.step_index, 1_u16);
    assert_eq!(bare.distance_mm, 25_u16);
    assert_eq!(bare.state_index, 0_u8);
    assert_eq!(bare.pulses, 1_000_u32);
    assert!((bare.cps - 1_000.0_f32).abs() < 0.01_f32);

    // Completing a measurement automatically advances to the next state.
    assert_eq!(sweep.state_index(), 1_usize);
    assert_eq!(sweep.target_distance_mm(), 25_u16);

    let filtered = measure(&mut sweep, 1_000_u64, 150_u32);
    assert_eq!(filtered.step_index, 2_u16);
    assert_eq!(filtered.distance_mm, 25_u16);
    assert_eq!(filtered.state_index, 1_u8);
    assert_eq!(filtered.pulses, 150_u32);

    // Completing the final state wraps state to zero and advances the distance.
    assert_eq!(sweep.state_index(), 0_usize);
    assert_eq!(sweep.target_distance_mm(), 35_u16);
    assert_eq!(sweep.points().len(), 2_usize);
}

#[test]
fn test_cancel_discards_in_progress_attempt() {
    let mut sweep = SweepSession::new(STEP_MS);
    sweep.set_target_distance_mm(20_u16);

    sweep.start_step(0_u64);
    sweep.ingest_pulses(300_u32);
    sweep.cancel_measurement();

    assert!(!sweep.is_measuring());
    assert_eq!(
        sweep.tick(5_000_u64),
        None,
        "a cancelled step never records"
    );
    assert_eq!(sweep.points(), []);
    assert_eq!(sweep.target_distance_mm(), 20_u16);
    assert_eq!(sweep.state_index(), 0_usize);

    sweep.start_step(5_000_u64);
    let retaken = sweep.tick(6_000_u64).expect("retaken step completes");
    assert_eq!(
        retaken.pulses, 0_u32,
        "pulses of the cancelled attempt are dropped"
    );
}

#[test]
fn test_undo_restores_previous_target() {
    let mut sweep = SweepSession::new(STEP_MS);
    sweep.set_target_distance_mm(40_u16);

    let _ = measure(&mut sweep, 0_u64, 400_u32);
    let _ = measure(&mut sweep, 1_000_u64, 40_u32);

    let removed = sweep.pop_last_point().expect("filtered point is removed");
    assert_eq!(removed.distance_mm, 40_u16);
    assert_eq!(removed.state_index, 1_u8);
    assert_eq!(sweep.points().len(), 1_usize);
    assert_eq!(sweep.state_index(), 1_usize);
    assert_eq!(sweep.target_distance_mm(), 40_u16);

    let removed_bare = sweep.pop_last_point().expect("bare point is removed");
    assert_eq!(removed_bare.state_index, 0_u8);
    assert_eq!(sweep.points(), []);
    assert_eq!(sweep.state_index(), 0_usize);
    assert_eq!(sweep.target_distance_mm(), 40_u16);
    let retaken = measure(&mut sweep, 2_000_u64, 7_u32);
    assert_eq!(retaken.step_index, 1_u16, "the step number is restored too");
}

#[test]
fn test_groups_collect_latest_record_per_state_and_position() {
    let mut sweep = SweepSession::new(STEP_MS);
    sweep.set_target_distance_mm(20_u16);
    sweep.set_step_size_mm(10_u16);

    // First position: bare then filtered at 20 mm.
    let _ = measure(&mut sweep, 0_u64, 100_u32);
    assert_eq!(sweep.groups().len(), 1_usize);
    let incomplete = sweep.groups().first().copied().expect("group exists");
    assert_eq!(incomplete.distance_mm, 20_u16);
    assert_eq!(incomplete.records.iter().flatten().count(), 1_usize);
    assert_eq!(cps(&incomplete, 0_usize), Some(100.0_f32));
    assert_eq!(cps(&incomplete, 1_usize), None);

    let _ = measure(&mut sweep, 1_000_u64, 40_u32);
    assert_eq!(
        sweep
            .groups()
            .first()
            .copied()
            .expect("group exists")
            .records
            .iter()
            .flatten()
            .count(),
        2_usize
    );

    // Second position: bare then filtered at 30 mm.
    assert_eq!(sweep.target_distance_mm(), 30_u16);
    let _ = measure(&mut sweep, 2_000_u64, 80_u32);
    let _ = measure(&mut sweep, 3_000_u64, 20_u32);

    assert_eq!(sweep.groups().len(), 2_usize);

    let first = sweep.groups().first().copied().expect("first group exists");
    assert_eq!(first.distance_mm, 20_u16);
    assert_eq!(cps(&first, 0_usize), Some(100.0_f32));
    assert_eq!(cps(&first, 1_usize), Some(40.0_f32));

    let second = sweep
        .groups()
        .get(1_usize)
        .copied()
        .expect("second group exists");
    assert_eq!(second.distance_mm, 30_u16);
    assert_eq!(cps(&second, 0_usize), Some(80.0_f32));
    assert_eq!(cps(&second, 1_usize), Some(20.0_f32));
    assert!(sweep.groups().get(2_usize).is_none());
}

#[test]
fn test_group_keeps_most_recent_state_at_position() {
    let mut sweep = SweepSession::new(STEP_MS);
    sweep.set_target_distance_mm(20_u16);
    sweep.set_step_size_mm(10_u16);

    // First bare/filtered round at 20 mm.
    let _ = measure(&mut sweep, 0_u64, 100_u32);
    let _ = measure(&mut sweep, 1_000_u64, 40_u32);

    // Return to the same position and repeat both states.
    sweep.set_target_distance_mm(20_u16);
    let _ = measure(&mut sweep, 2_000_u64, 200_u32);
    let _ = measure(&mut sweep, 3_000_u64, 10_u32);

    assert_eq!(
        sweep.groups().len(),
        1_usize,
        "repeating a position must not create a second group"
    );
    let group = sweep.groups().first().copied().expect("group exists");
    assert_eq!(group.distance_mm, 20_u16);
    assert_eq!(
        cps(&group, 0_usize),
        Some(200.0_f32),
        "the latest bare measurement must replace the earlier one"
    );
    assert_eq!(
        cps(&group, 1_usize),
        Some(10.0_f32),
        "the latest filtered measurement must replace the earlier one"
    );
}

#[test]
fn test_custom_plan_cycles_three_states_at_one_distance() {
    let mut sweep = SweepSession::new(STEP_MS);
    sweep.set_state_count(3_usize);
    sweep.set_target_distance_mm(50_u16);
    sweep.set_step_size_mm(5_u16);

    assert_eq!(sweep.state_count(), 3_usize);

    for (index, pulses) in [100_u32, 40_u32, 20_u32].into_iter().enumerate() {
        assert_eq!(sweep.state_index(), index);
        let start_ms = u64::try_from(index)
            .unwrap_or_default()
            .saturating_mul(u64::from(STEP_MS));
        let record = measure(&mut sweep, start_ms, pulses);
        assert_eq!(record.state_index, u8::try_from(index).unwrap_or_default());
        let expected_next = if index + 1 < 3 { index + 1 } else { 0 };
        assert_eq!(
            sweep.state_index(),
            expected_next,
            "completing a measurement must auto-advance to the next state"
        );
        let expected_distance = if index + 1 < 3 { 50_u16 } else { 55_u16 };
        assert_eq!(sweep.target_distance_mm(), expected_distance);
    }

    assert_eq!(sweep.state_index(), 0_usize);
    assert_eq!(sweep.target_distance_mm(), 55_u16);
    assert_eq!(sweep.points().len(), 3_usize);

    let group = sweep.groups().first().copied().expect("group exists");
    assert_eq!(group.records.iter().flatten().count(), 3_usize);
    assert_eq!(cps(&group, 0_usize), Some(100.0_f32));
    assert_eq!(cps(&group, 1_usize), Some(40.0_f32));
    assert_eq!(cps(&group, 2_usize), Some(20.0_f32));
    assert_eq!(cps(&group, 3_usize), None);
}

#[test]
fn test_state_count_and_state_changes_cancel_the_running_window() {
    let mut sweep = SweepSession::new(STEP_MS);
    assert_eq!(sweep.state_count(), 2_usize, "two states by default");
    sweep.set_state_count(0_usize);
    assert_eq!(sweep.state_count(), 1_usize);
    sweep.set_state_count(1_000_usize);
    assert_eq!(sweep.state_count(), 64_usize);

    sweep.set_state_count(3_usize);
    sweep.set_state_index(2_usize);
    assert_eq!(sweep.state_index(), 2_usize);

    sweep.start_step(0_u64);
    sweep.ingest_pulses(12_u32);
    sweep.set_state_index(0_usize);
    assert_eq!(sweep.state_index(), 0_usize);
    assert!(
        !sweep.is_measuring(),
        "changing state must cancel an in-progress window"
    );

    sweep.set_state_index(2_usize);
    sweep.start_step(0_u64);
    sweep.set_state_count(2_usize);
    assert_eq!(sweep.state_index(), 0_usize);
    assert!(
        !sweep.is_measuring(),
        "changing the state count must cancel an in-progress window"
    );
}

#[test]
fn test_distance_advance_is_clamped() {
    let mut sweep = SweepSession::new(STEP_MS);
    sweep.set_target_distance_mm(u16::MAX - 5_u16);
    sweep.set_step_size_mm(100_u16);

    sweep.start_step(1_000_u64);
    assert_eq!(sweep.step_elapsed_ms(1_250_u64), 250_u64);
    sweep.ingest_pulses(10_u32);
    let bare = sweep.tick(2_000_u64).expect("bare step completes");
    assert_eq!(bare.distance_mm, u16::MAX - 5_u16);
    assert_eq!(sweep.state_index(), 1_usize);
    assert_eq!(
        sweep.target_distance_mm(),
        u16::MAX - 5_u16,
        "the measurement must stay at the same distance"
    );
    assert_eq!(sweep.step_elapsed_ms(2_500_u64), 0_u64);

    let _ = measure(&mut sweep, 2_000_u64, 5_u32);
    // Completing the final state wraps and clamps the distance at u16::MAX.
    assert_eq!(sweep.state_index(), 0_usize);
    assert_eq!(sweep.target_distance_mm(), u16::MAX);
}

#[test]
fn test_hardware_gate_finishes_the_step() {
    let mut sweep = SweepSession::new(STEP_MS);
    assert_eq!(
        sweep.finish_step(5_u32, 20_000_u32),
        None,
        "no step running"
    );

    sweep.start_step(0_u64);
    let record = sweep
        .finish_step(400_u32, 20_000_u32)
        .expect("running step is finished by the hardware count");
    assert_eq!(record.pulses, 400_u32);
    assert_eq!(record.duration_ms, 20_000_u32);
    assert!((record.cps - 20.0_f32).abs() < 1e-6_f32);
    assert!(!sweep.is_measuring());
    assert_eq!(sweep.state_index(), 1_usize, "advanced to the next state");
}

#[test]
fn test_angle_axis_steps_the_angle() {
    let mut sweep = SweepSession::new(STEP_MS);
    sweep.set_state_count(1_usize);
    sweep.set_axis(SweepAxis::Angle);
    sweep.set_step_size_mm(10_u16);
    sweep.set_target_angle_deg(-60_i16);
    sweep.set_target_distance_mm(80_u16);

    let first = measure(&mut sweep, 0_u64, 30_u32);
    assert_eq!(first.angle_deg, -60_i16);
    assert_eq!(
        sweep.target_angle_deg(),
        -50_i16,
        "angle advanced by the step"
    );
    assert_eq!(sweep.target_distance_mm(), 80_u16, "distance untouched");

    sweep.set_target_angle_deg(500_i16);
    assert_eq!(sweep.target_angle_deg(), 180_i16, "clamped to +-180");
}
