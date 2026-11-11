//! CSV rows, file names and timestamps.

use crate::export::{csv_path, gate_row, utc_timestamp};

#[test]
fn utc_timestamp_formats_known_dates() {
    assert_eq!(utc_timestamp(0).0, "1970-01-01T00:00:00Z");
    assert_eq!(utc_timestamp(951_782_400).0, "2000-02-29T00:00:00Z");
    assert_eq!(utc_timestamp(1_793_577_600).1, "20261102_000000");
}

#[test]
fn gate_row_reports_counts_rate_and_sigma() {
    assert_eq!(
        gate_row(3, 35.0, 10.0, 100, 0.0),
        "3,35.000,10,100,10.000000,1.000000,10.000000"
    );
    assert_eq!(
        gate_row(0, 5.0, 10.0, 0, 0.0),
        "0,5.000,10,0,0.000000,0.100000,0.000000",
        "an empty gate gets sigma = 1 count / gate"
    );
}

#[test]
fn csv_path_keeps_names_inside_the_folder() {
    let dir = std::env::temp_dir().join(format!("curie_test_csv_{}", std::process::id()));
    let path = csv_path(&dir, "../ra226, 10 s.csv", false).expect("folder created");
    assert_eq!(path, dir.join(".._ra226__10_s.csv"));
    drop(std::fs::remove_dir_all(&dir));
}
