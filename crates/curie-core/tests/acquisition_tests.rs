//! Integration tests for the gate-series session.

use curie_core::{CurieSession, DaqState};

#[test]
fn test_acquisition_target_samples_auto_completion() {
    let mut session = CurieSession::new(1000_u32);
    session.target_samples = 3;
    session.start(0);
    assert_eq!(session.state, DaqState::Acquiring);

    for (now_ms, pulses) in [(1000_u64, 25_u32), (2000, 30), (3000, 20)] {
        session.ingest_pulses(pulses);
        assert!(session.tick(now_ms).is_some());
    }
    assert_eq!(session.completed_samples, 3);
    assert_eq!(session.state, DaqState::Completed);

    session.ingest_pulses(50);
    assert!(
        session.tick(4000).is_none(),
        "a completed run records nothing"
    );

    session.start(5000);
    assert_eq!(session.state, DaqState::Acquiring);
    assert_eq!(session.completed_samples, 0, "a new run starts from zero");
}

#[test]
fn test_late_ticks_close_gates_on_fixed_deadlines() {
    let mut session = CurieSession::new(1_000_u32);
    session.start(0_u64);

    // Pulses timestamped inside gate 1, delivered together with a tick 3.4 s late.
    session.ingest_pulses(7_u32);
    let records: Vec<_> = core::iter::from_fn(|| session.tick(3_400_u64)).collect();
    let ends: Vec<u64> = records.iter().map(|record| record.timestamp_ms).collect();
    assert_eq!(
        ends,
        [1_000_u64, 2_000_u64, 3_000_u64],
        "deadlines do not drift"
    );
    assert!(records.iter().all(|record| record.gate_ms == 1_000_u32));
    assert_eq!(
        records.first().map(|record| record.interval_pulses),
        Some(7_u32)
    );
    assert_eq!(
        session.last_tick_ms, 3_000_u64,
        "the next gate started at 3 s, not 3.4 s"
    );
}

#[test]
fn test_pause_discards_the_partial_gate() {
    let mut session = CurieSession::new(1_000_u32);
    session.start(0_u64);
    session.ingest_pulses(50_u32);
    session.pause();
    session.ingest_pulses(99_u32);

    session.start(10_000_u64);
    session.ingest_pulses(4_u32);
    let record = session
        .tick(11_000_u64)
        .expect("one full gate after resuming");
    assert_eq!(
        record.interval_pulses, 4_u32,
        "partial gate and pulses while paused dropped"
    );
    assert_eq!(
        record.timestamp_ms, 11_000_u64,
        "run clock keeps running through the pause"
    );
}

#[test]
fn test_hardware_gates_are_recorded_with_their_own_duration() {
    let mut session = CurieSession::new(250_u32);
    session.target_samples = 2_u32;
    session.start(1_000_u64);

    let first = session
        .ingest_gate(11_050_u64, 10_000_u32, 120_u32)
        .expect("acquiring session records the gate");
    assert_eq!(first.timestamp_ms, 10_050_u64);
    assert_eq!(
        first.gate_ms, 10_000_u32,
        "the hardware gate length is kept"
    );
    assert_eq!(first.interval_pulses, 120_u32);

    let _second = session.ingest_gate(21_110_u64, 10_000_u32, 100_u32);
    assert_eq!(session.state, DaqState::Completed);
    assert!(session.ingest_gate(31_000_u64, 10_000_u32, 1_u32).is_none());
}
